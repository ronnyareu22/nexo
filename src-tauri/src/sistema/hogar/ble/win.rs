//! Bluetooth en Windows: WinRT.
//!
//! - `buscar`: un BluetoothLEAdvertisementWatcher activo durante el plazo; al terminar se detiene y se suelta.
//! - `escribir`: abre la luz por su dirección, busca la característica de escritura (GATT), manda las tramas y la
//!   cierra. Va en un hilo propio con plazo: las llamadas de WinRT no tienen tiempo límite.

use super::textos;
use super::tramas::{self, Anuncio, ESCRITURA, Protocolo, Recuerdo, uuid_corto};
use crate::sistema::hogar::modelo::Cambio;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use windows::Devices::Bluetooth::Advertisement::{
    BluetoothLEAdvertisementReceivedEventArgs, BluetoothLEAdvertisementWatcher, BluetoothLEAdvertisementWatcherStatus,
    BluetoothLEScanningMode,
};
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristic, GattCharacteristicProperties, GattCommunicationStatus, GattDeviceService, GattWriteOption,
};
use windows::Devices::Bluetooth::{BluetoothAdapter, BluetoothLEDevice};
use windows::Devices::Radios::RadioState;
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::{DataReader, DataWriter, IBuffer};
use windows::core::GUID;

/// Pausa entre tramas sin respuesta (`elkbledom` espera 0,1–0,3 s).
const PAUSA: Duration = Duration::from_millis(150);
const PLAZO_ESCRIBIR: Duration = Duration::from_secs(8);
const NO_PUDE_BUSCAR: &str = "No pude buscar por Bluetooth. Revisa que esté encendido y vuelve a intentar.";
/// Datos de servicio con UUID de 16 bits (tipo AD 0x16).
const DATOS_SERVICIO_16: u8 = 0x16;

/// El Bluetooth de este equipo existe, es de bajo consumo y está encendido.
fn adaptador_listo() -> Result<(), String> {
    let adaptador = BluetoothAdapter::GetDefaultAsync()
        .and_then(|op| op.join())
        .map_err(|_| "Este equipo no tiene Bluetooth.".to_string())?;
    if !adaptador.IsLowEnergySupported().unwrap_or(false) {
        return Err("El Bluetooth de este equipo no sirve para luces (no es de bajo consumo).".into());
    }
    if let Ok(radio) = adaptador.GetRadioAsync().and_then(|op| op.join())
        && radio.State().ok() != Some(RadioState::On)
    {
        return Err(textos::APAGADO.into());
    }
    Ok(())
}

/// Un UUID de 16 bits ("FFF0") como GUID de Bluetooth.
fn guid(corto: &str) -> GUID {
    let n = u32::from_str_radix(corto, 16).unwrap_or(0);
    GUID::from_values(n, 0x0000, 0x1000, [0x80, 0x00, 0x00, 0x80, 0x5f, 0x9b, 0x34, 0xfb])
}

/// Un GUID en texto, en corto si es de los de 16 bits.
fn texto_guid(g: &GUID) -> String {
    let d = g.data4;
    uuid_corto(&format!(
        "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        g.data1, g.data2, g.data3, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
    ))
}

fn bytes(b: &IBuffer) -> Vec<u8> {
    let Ok(lector) = DataReader::FromBuffer(b) else { return Vec::new() };
    let n = lector.UnconsumedBufferLength().unwrap_or(0) as usize;
    let mut v = vec![0u8; n.min(256)];
    if lector.ReadBytes(&mut v).is_err() {
        return Vec::new();
    }
    v
}

/// Lo que trae un anuncio: dirección, nombre, señal, servicios y datos.
fn anuncio(a: &BluetoothLEAdvertisementReceivedEventArgs) -> Option<Anuncio> {
    let direccion = format!("{:012X}", a.BluetoothAddress().ok()?);
    let ad = a.Advertisement().ok()?;
    let mut x = Anuncio {
        direccion,
        nombre: ad.LocalName().ok().map(|n| n.to_string().trim().to_string()).filter(|n| !n.is_empty()),
        senal: a.RawSignalStrengthInDBm().ok().filter(|s| (-127..0).contains(s)),
        ..Anuncio::default()
    };
    if let Ok(lista) = ad.ServiceUuids() {
        x.servicios = (&lista).into_iter().map(|g| texto_guid(&g)).collect();
    }
    if let Ok(lista) = ad.ManufacturerData() {
        for m in &lista {
            if let (Ok(id), Ok(datos)) = (m.CompanyId(), m.Data()) {
                x.fabricante.push((id, bytes(&datos)));
            }
        }
    }
    if let Ok(secciones) = ad.DataSections() {
        for s in &secciones {
            if s.DataType().ok() != Some(DATOS_SERVICIO_16) {
                continue;
            }
            let Ok(datos) = s.Data() else { continue };
            let b = bytes(&datos);
            if b.len() >= 2 {
                x.datos_servicio.push((format!("{:04X}", u16::from_le_bytes([b[0], b[1]])), b[2..].to_vec()));
            }
        }
    }
    Some(x)
}

// ---------------------------------------------------------------- Buscar

pub fn buscar(plazo: Duration) -> Result<Vec<Anuncio>, String> {
    let inicio = Instant::now();
    adaptador_listo()?;
    let vistos: Arc<Mutex<HashMap<String, Anuncio>>> = Arc::default();
    let vigia = BluetoothLEAdvertisementWatcher::new().map_err(|_| NO_PUDE_BUSCAR.to_string())?;
    // Activo: así llega también el nombre (va en la respuesta del escaneo).
    let _ = vigia.SetScanningMode(BluetoothLEScanningMode::Active);
    let destino = vistos.clone();
    let manejador = TypedEventHandler::<BluetoothLEAdvertisementWatcher, BluetoothLEAdvertisementReceivedEventArgs>::new(
        move |_, args| {
            if let Some(a) = args.as_ref().and_then(anuncio) {
                let mut mapa = destino.lock().unwrap_or_else(|e| e.into_inner());
                match mapa.get_mut(&a.direccion) {
                    Some(previo) => previo.juntar(a),
                    None => {
                        mapa.insert(a.direccion.clone(), a);
                    }
                }
            }
            Ok(())
        },
    );
    let token = vigia.Received(&manejador).map_err(|_| NO_PUDE_BUSCAR.to_string())?;
    if vigia.Start().is_err() {
        let _ = vigia.RemoveReceived(token);
        return Err(NO_PUDE_BUSCAR.into());
    }
    // Se escucha hasta el plazo (al menos 2 s); si Windows la corta (radio apagada), se avisa.
    let fin = (inicio + plazo).max(Instant::now() + Duration::from_secs(2));
    let mut cortada = false;
    while Instant::now() < fin {
        std::thread::sleep(Duration::from_millis(200));
        if vigia.Status().ok() == Some(BluetoothLEAdvertisementWatcherStatus::Aborted) {
            cortada = true;
            break;
        }
    }
    let _ = vigia.Stop();
    let _ = vigia.RemoveReceived(token);
    if cortada {
        return Err(NO_PUDE_BUSCAR.into());
    }
    let mut mapa = vistos.lock().unwrap_or_else(|e| e.into_inner());
    Ok(mapa.drain().map(|(_, a)| a).collect())
}

// ---------------------------------------------------------------- Manejar

pub fn escribir(direccion: &str, preferido: Protocolo, cambio: Cambio, recuerdo: Recuerdo) -> Result<(), String> {
    let limpia: String = direccion.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let numero = u64::from_str_radix(&limpia, 16).map_err(|_| textos::NO_ENCONTRADA.to_string())?;
    adaptador_listo()?;
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("nexo-hogar-ble".into())
        .spawn(move || {
            let _ = tx.send(escribir_en_hilo(numero, preferido, &cambio, &recuerdo));
        })
        .map_err(|_| textos::NO_RESPONDE.to_string())?;
    rx.recv_timeout(PLAZO_ESCRIBIR).unwrap_or_else(|_| Err(textos::NO_RESPONDE.into()))
}

fn escribir_en_hilo(numero: u64, preferido: Protocolo, c: &Cambio, r: &Recuerdo) -> Result<(), String> {
    let aparato = BluetoothLEDevice::FromBluetoothAddressAsync(numero)
        .and_then(|op| op.join())
        .map_err(|_| textos::NO_ENCONTRADA.to_string())?;
    let nombre = aparato.Name().ok().map(|n| n.to_string());
    let mut abiertos: Vec<GattDeviceService> = Vec::new();
    let resultado = escribir_aparato(&aparato, nombre.as_deref(), preferido, c, r, &mut abiertos);
    for s in abiertos {
        let _ = s.Close();
    }
    let _ = aparato.Close();
    resultado
}

fn escribir_aparato(
    aparato: &BluetoothLEDevice,
    nombre: Option<&str>,
    preferido: Protocolo,
    c: &Cambio,
    r: &Recuerdo,
    abiertos: &mut Vec<GattDeviceService>,
) -> Result<(), String> {
    // Primero los servicios del protocolo esperado.
    let mut candidatas: Vec<(Protocolo, &str, &str)> = ESCRITURA.to_vec();
    candidatas.sort_by_key(|(p, _, _)| *p != preferido);
    let mut inalcanzable = false;
    let mut elegida: Option<(Protocolo, GattCharacteristic)> = None;
    'buscar: for (protocolo, servicio, caracteristica) in candidatas {
        let Ok(res) = aparato.GetGattServicesForUuidAsync(guid(servicio)).and_then(|op| op.join()) else { continue };
        match res.Status() {
            Ok(GattCommunicationStatus::Success) => {}
            Ok(GattCommunicationStatus::Unreachable) => {
                inalcanzable = true;
                continue;
            }
            _ => continue,
        }
        let Ok(servicios) = res.Services() else { continue };
        for s in &servicios {
            let car = s
                .GetCharacteristicsForUuidAsync(guid(caracteristica))
                .and_then(|op| op.join())
                .ok()
                .filter(|rc| rc.Status().ok() == Some(GattCommunicationStatus::Success))
                .and_then(|rc| rc.Characteristics().ok())
                .and_then(|lista| lista.GetAt(0).ok());
            abiertos.push(s);
            if let Some(car) = car {
                elegida = Some((protocolo, car));
                break 'buscar;
            }
        }
    }
    let Some((protocolo, car)) = elegida else {
        return Err(if inalcanzable { textos::NO_ENCONTRADA } else { textos::SIN_ESCRITURA }.into());
    };
    let propiedades = car.CharacteristicProperties().unwrap_or(GattCharacteristicProperties::None);
    let sin_respuesta = propiedades.0 & GattCharacteristicProperties::WriteWithoutResponse.0 != 0;
    let opcion = if sin_respuesta { GattWriteOption::WriteWithoutResponse } else { GattWriteOption::WriteWithResponse };
    for trama in tramas::armar(protocolo, nombre, c, r) {
        let escritor = DataWriter::new().map_err(|_| textos::NO_RESPONDE.to_string())?;
        let buffer = escritor
            .WriteBytes(&trama)
            .and_then(|_| escritor.DetachBuffer())
            .map_err(|_| textos::NO_RESPONDE.to_string())?;
        let estado = car
            .WriteValueWithOptionAsync(&buffer, opcion)
            .and_then(|op| op.join())
            .map_err(|_| textos::NO_RESPONDE.to_string())?;
        match estado {
            GattCommunicationStatus::Success => {}
            GattCommunicationStatus::Unreachable => return Err(textos::NO_ENCONTRADA.into()),
            _ => return Err("La luz no aceptó la orden.".into()),
        }
        if sin_respuesta {
            std::thread::sleep(PAUSA);
        }
    }
    Ok(())
}
