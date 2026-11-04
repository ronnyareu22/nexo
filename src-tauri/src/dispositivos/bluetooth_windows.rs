//! Bluetooth en Windows.
//!
//! - Lista: los clásicos (audífonos, parlantes, teléfonos…) con BluetoothFindFirstDevice, sin buscar
//!   aparatos nuevos (tarda pocos milisegundos), y los Bluetooth LE emparejados con WinRT.
//!   Marca, modelo y batería salen de sus nodos PnP; el estado del Bluetooth, de Windows.Devices.Radios.
//! - Conectar y desconectar: Windows no tiene una forma pública. Para aparatos de sonido se hace
//!   lo mismo que Configuración: se pide KSPROPERTY_ONESHOT_RECONNECT o _DISCONNECT al filtro de
//!   sonido Bluetooth (IKsControl), al que se llega desde sus salidas y micrófonos. Los demás aparatos
//!   se conectan desde Configuración › Bluetooth.
//! - Olvidar: BluetoothRemoveDevice y, si no basta, WinRT (UnpairAsync). Después se confirma.

use std::time::Duration;

use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::{DeviceInformation, DeviceUnpairingResultStatus};
use windows::Devices::Radios::{Radio, RadioKind, RadioState};
use windows::Win32::Devices::Bluetooth::{
    BLUETOOTH_ADDRESS, BLUETOOTH_ADDRESS_0, BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS, BluetoothFindDeviceClose,
    BluetoothFindFirstDevice, BluetoothFindNextDevice, BluetoothRemoveDevice,
};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Media::Audio::{DEVICE_STATEMASK_ALL, IDeviceTopology, IMMDevice, IMMDeviceEnumerator, IPart};
use windows::Win32::Media::KernelStreaming::{
    IKsControl, KSIDENTIFIER, KSIDENTIFIER_0, KSIDENTIFIER_0_0, KSPROPERTY_ONESHOT_DISCONNECT, KSPROPERTY_ONESHOT_RECONNECT,
    KSPROPERTY_TYPE_GET, KSPROPSETID_BtAudio,
};
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::core::{HSTRING, Interface};

use crate::dispositivos::win::{self, NodosBluetooth};
use crate::dispositivos::{Dispositivo, Estado, Tipo};

const NO_SE_PUDO_OLVIDAR: &str = "No se pudo olvidar desde Nexo. Hazlo en Configuración › Bluetooth.";

/// Un aparato emparejado, antes de completar sus datos.
struct Emparejado {
    direccion: u64,
    nombre: String,
    tipo: Tipo,
    conectado: bool,
}

pub fn listar() -> Result<Estado, String> {
    win::iniciar_com();
    let encendido = radio_encendida();
    let nodos = NodosBluetooth::leer();

    let mut emparejados = clasicos();
    for le in emparejados_le() {
        // Algunos aparatos se emparejan por las dos vías: se muestran una sola vez.
        if !emparejados.iter().any(|c| c.direccion == le.direccion || c.nombre == le.nombre) {
            emparejados.push(le);
        }
    }

    let mut lista: Vec<Dispositivo> = emparejados.into_iter().map(|e| crear(e, &nodos)).collect();
    // Primero los conectados; dentro de cada grupo, por nombre.
    lista.sort_by(|a, b| b.conectado.cmp(&a.conectado).then(a.nombre.cmp(&b.nombre)));
    // Si hay algo conectado, el Bluetooth está encendido aunque Windows no muestre la radio.
    let encendido = encendido || lista.iter().any(|d| d.conectado);
    Ok(Estado { bluetooth_encendido: encendido, dispositivos: lista })
}

fn crear(e: Emparejado, nodos: &NodosBluetooth) -> Dispositivo {
    let clave = win::direccion_en_ids(e.direccion);
    let marca_y_modelo = nodos.marca_y_modelo(&clave);
    let marca = marca_y_modelo.map(|(m, _)| m);
    Dispositivo {
        id: win::numero_a_direccion(e.direccion),
        fabricante: fabricante(marca, &e.nombre),
        // La batería guardada puede ser vieja: solo cuenta si está conectado.
        bateria: if e.conectado { nodos.bateria(&clave) } else { None },
        bateria_izquierda: None,
        bateria_derecha: None,
        bateria_estuche: None,
        firmware: None,
        vendor_id: marca.map(|m| format!("0x{m:04X}")),
        product_id: marca_y_modelo.map(|(_, p)| format!("0x{p:04X}")),
        alias: None, // lo completa el historial
        nombre: e.nombre,
        tipo: e.tipo,
        conectado: e.conectado,
    }
}

/// El Bluetooth está encendido si alguna radio Bluetooth está en On. Sin radio, apagado.
fn radio_encendida() -> bool {
    let Ok(radios) = Radio::GetRadiosAsync().and_then(|op| op.join()) else {
        return false;
    };
    (&radios).into_iter().any(|r| r.Kind().ok() == Some(RadioKind::Bluetooth) && r.State().ok() == Some(RadioState::On))
}

// ---------------------------------------------------------------- Clásicos

fn info_vacia() -> BLUETOOTH_DEVICE_INFO {
    BLUETOOTH_DEVICE_INFO { dwSize: std::mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32, ..Default::default() }
}

/// Los aparatos clásicos emparejados, con su estado de conexión. No busca aparatos nuevos.
fn clasicos() -> Vec<Emparejado> {
    let busqueda = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: std::mem::size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnAuthenticated: true.into(),
        fReturnRemembered: true.into(),
        fReturnUnknown: false.into(),
        fReturnConnected: true.into(),
        fIssueInquiry: false.into(),
        cTimeoutMultiplier: 0,
        hRadio: HANDLE::default(),
    };
    let mut info = info_vacia();
    let Ok(busca) = (unsafe { BluetoothFindFirstDevice(&busqueda, &mut info) }) else {
        return Vec::new();
    };
    let mut lista = Vec::new();
    loop {
        // Un aparato conectado que nunca se emparejó no entra en la lista.
        if info.fRemembered.as_bool() || info.fAuthenticated.as_bool() {
            let direccion = unsafe { info.Address.Anonymous.ullLong } & 0xFFFF_FFFF_FFFF;
            let largo = info.szName.iter().position(|&c| c == 0).unwrap_or(info.szName.len());
            let nombre = String::from_utf16_lossy(&info.szName[..largo]).trim().to_string();
            let nombre = if nombre.is_empty() { win::numero_a_direccion(direccion) } else { nombre };
            lista.push(Emparejado {
                tipo: tipo_por_clase(info.ulClassofDevice, &nombre),
                conectado: info.fConnected.as_bool(),
                direccion,
                nombre,
            });
        }
        info = info_vacia();
        if unsafe { BluetoothFindNextDevice(busca, &mut info) }.is_err() {
            break;
        }
    }
    let _ = unsafe { BluetoothFindDeviceClose(busca) };
    lista
}

/// El tipo según la clase del aparato (Class of Device) que anuncia el Bluetooth clásico.
fn tipo_por_clase(clase: u32, nombre: &str) -> Tipo {
    let mayor = (clase >> 8) & 0x1F;
    let menor = (clase >> 2) & 0x3F;
    match mayor {
        0x02 => Tipo::Telefono,
        0x04 => match menor {
            0x01 | 0x02 => Tipo::Diadema,                 // con micrófono, manos libres
            0x06 => Tipo::Audifonos,                      // audífonos
            0x05 | 0x07 | 0x08 | 0x0A => Tipo::Parlante,  // parlante, portátil, auto, equipo
            _ => tipo_por_nombre(nombre),
        },
        0x05 if clase & 0x40 != 0 => Tipo::Teclado,
        0x05 if clase & 0x80 != 0 => Tipo::Mouse,
        _ => tipo_por_nombre(nombre),
    }
}

fn tipo_por_nombre(nombre: &str) -> Tipo {
    let minusculas = nombre.to_lowercase();
    if ["iphone", "galaxy", "pixel", "redmi", "xiaomi", "motorola"].iter().any(|m| minusculas.contains(m)) {
        Tipo::Telefono
    } else {
        Tipo::Otro
    }
}

/// El mismo criterio de la Mac: por la marca (vendor ID) y, para Beats, por el nombre.
fn fabricante(marca: Option<u16>, nombre: &str) -> Option<String> {
    let minusculas = nombre.to_lowercase();
    match marca {
        _ if minusculas.contains("beats") => Some("Beats"),
        Some(0x004C) | Some(0x05AC) => Some("Apple"),
        Some(0x054C) => Some("Sony"),
        Some(0x046D) => Some("Logitech"),
        Some(0x045E) => Some("Microsoft"),
        Some(0x0075) | Some(0x04E8) => Some("Samsung"),
        Some(0x0A12) => Some("JBL"),
        _ if minusculas.contains("iphone") => Some("Apple"),
        _ => None,
    }
    .map(str::to_owned)
}

/// ¿Está conectado ahora? None si ya no está entre los emparejados clásicos.
fn conectado_clasico(direccion: u64) -> Option<bool> {
    clasicos().into_iter().find(|c| c.direccion == direccion).map(|c| c.conectado)
}

/// Como clásico o, si no lo es, como Bluetooth LE. None si no se encuentra.
fn conectado_ahora(direccion: u64) -> Option<bool> {
    conectado_clasico(direccion).or_else(|| {
        let aparato = BluetoothLEDevice::FromBluetoothAddressAsync(direccion).and_then(|op| op.join()).ok()?;
        Some(aparato.ConnectionStatus().ok()? == BluetoothConnectionStatus::Connected)
    })
}

// ---------------------------------------------------------------- Bluetooth LE

/// Los aparatos Bluetooth LE emparejados (teclados, ratones…), con WinRT.
fn emparejados_le() -> Vec<Emparejado> {
    let Some(lista) = lista_le() else { return Vec::new() };
    (&lista)
        .into_iter()
        .filter_map(|info| {
            let id = info.Id().ok()?;
            let nombre = info.Name().map(|n| n.to_string().trim().to_string()).unwrap_or_default();
            let aparato = BluetoothLEDevice::FromIdAsync(&id).and_then(|op| op.join()).ok();
            let direccion = aparato
                .as_ref()
                .and_then(|a| a.BluetoothAddress().ok())
                .filter(|&d| d != 0)
                .or_else(|| direccion_de_id(&id.to_string()))?;
            let apariencia = aparato.as_ref().and_then(|a| a.Appearance().ok());
            let categoria = apariencia.as_ref().and_then(|a| a.Category().ok()).unwrap_or(0);
            let subcategoria = apariencia.as_ref().and_then(|a| a.SubCategory().ok()).unwrap_or(0);
            let conectado = aparato.as_ref().and_then(|a| a.ConnectionStatus().ok()) == Some(BluetoothConnectionStatus::Connected);
            let nombre = if nombre.is_empty() { win::numero_a_direccion(direccion) } else { nombre };
            Some(Emparejado { tipo: tipo_por_apariencia(categoria, subcategoria, &nombre), direccion, nombre, conectado })
        })
        .collect()
}

fn lista_le() -> Option<windows::Devices::Enumeration::DeviceInformationCollection> {
    let selector = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true).ok()?;
    DeviceInformation::FindAllAsyncAqsFilter(&selector).and_then(|op| op.join()).ok()
}

/// Las direcciones LE emparejadas, sin abrir cada aparato (para confirmar que se olvidó).
fn direcciones_le() -> Vec<u64> {
    let Some(lista) = lista_le() else { return Vec::new() };
    (&lista).into_iter().filter_map(|info| direccion_de_id(&info.Id().ok()?.to_string())).collect()
}

/// "BluetoothLE#BluetoothLE00:1a:7d:da:71:13-d7:a0:ab:7a:1a:4e" → la dirección del aparato (la última).
fn direccion_de_id(id: &str) -> Option<u64> {
    win::direccion_a_numero(id.rsplit('-').next()?)
}

/// El tipo según la apariencia que anuncia el Bluetooth LE (categoría y subcategoría).
fn tipo_por_apariencia(categoria: u16, subcategoria: u16, nombre: &str) -> Tipo {
    match (categoria, subcategoria) {
        (0x001, _) => Tipo::Telefono,
        (0x00F, 0x01) => Tipo::Teclado,
        (0x00F, 0x02) | (0x00F, 0x09) => Tipo::Mouse, // ratón, panel táctil
        (0x021, _) => Tipo::Parlante,
        (0x025, 0x02) => Tipo::Diadema, // audífonos con micrófono
        (0x025, _) => Tipo::Audifonos,
        _ => tipo_por_nombre(nombre),
    }
}

// ---------------------------------------------------------------- Conectar y desconectar

pub fn cambiar_conexion(direccion: &str, conectar: bool) -> Result<(), String> {
    win::iniciar_com();
    let desde_configuracion = || {
        if conectar {
            "En Windows este aparato se conecta desde Configuración › Bluetooth.".to_string()
        } else {
            "En Windows este aparato se desconecta desde Configuración › Bluetooth.".to_string()
        }
    };
    let numero = win::direccion_a_numero(direccion).ok_or("No encontré ese dispositivo entre los emparejados")?;
    let e = win::enumerador().ok_or_else(desde_configuracion)?;

    // Los filtros de sonido Bluetooth del aparato (estéreo y manos libres), conectados o no.
    let filtros = filtros_bluetooth(&e, numero);
    if filtros.is_empty() {
        return Err(desde_configuracion());
    }
    let aceptados = filtros.iter().filter(|f| pedir(f, conectar)).count();

    // El pedido no espera a que termine: se mira el estado real hasta que cambie.
    let espera = if aceptados == 0 { 3 } else if conectar { 10 } else { 6 };
    for _ in 0..espera * 4 {
        std::thread::sleep(Duration::from_millis(250));
        match conectado_ahora(numero) {
            Some(ahora) if ahora == conectar => return Ok(()),
            None => return Err("No encontré ese dispositivo entre los emparejados".into()),
            _ => {}
        }
    }
    if conectar {
        Err("No se pudo conectar. Revisa que esté encendido y cerca.".into())
    } else {
        Err("Windows no terminó de desconectarlo. Prueba desde Configuración › Bluetooth.".into())
    }
}

/// Un IKsControl por cada filtro de sonido distinto del aparato.
fn filtros_bluetooth(e: &IMMDeviceEnumerator, direccion: u64) -> Vec<IKsControl> {
    let mut vistos: Vec<String> = Vec::new();
    let mut filtros = Vec::new();
    for punto in win::puntos_bluetooth(e, direccion, DEVICE_STATEMASK_ALL) {
        let Some(id) = filtro_de(&punto.aparato) else { continue };
        if vistos.contains(&id) {
            continue;
        }
        let Ok(aparato) = (unsafe { e.GetDevice(&HSTRING::from(id.as_str())) }) else { continue };
        let Ok(control) = (unsafe { aparato.Activate::<IKsControl>(CLSCTX_ALL, None) }) else { continue };
        vistos.push(id);
        filtros.push(control);
    }
    filtros
}

/// El ID del filtro de sonido (kernel streaming) al que está unido un punto de sonido.
fn filtro_de(punto: &IMMDevice) -> Option<String> {
    let topologia: IDeviceTopology = unsafe { punto.Activate(CLSCTX_ALL, None) }.ok()?;
    let conector = unsafe { topologia.GetConnector(0) }.ok()?;
    if let Some(id) = unsafe { conector.GetDeviceIdConnectedTo() }.ok().and_then(win::texto_com) {
        return Some(id);
    }
    // Otra vía al mismo dato: la parte del otro lado y su topología.
    let otro = unsafe { conector.GetConnectedTo() }.ok()?;
    let parte: IPart = otro.cast().ok()?;
    let topologia_filtro = unsafe { parte.GetTopologyObject() }.ok()?;
    unsafe { topologia_filtro.GetDeviceId() }.ok().and_then(win::texto_com)
}

/// Pide al filtro que se vuelva a conectar o que se desconecte. true si lo aceptó.
fn pedir(filtro: &IKsControl, conectar: bool) -> bool {
    let id = if conectar { KSPROPERTY_ONESHOT_RECONNECT } else { KSPROPERTY_ONESHOT_DISCONNECT };
    let propiedad = KSIDENTIFIER {
        Anonymous: KSIDENTIFIER_0 { Anonymous: KSIDENTIFIER_0_0 { Set: KSPROPSETID_BtAudio, Id: id.0 as u32, Flags: KSPROPERTY_TYPE_GET } },
    };
    let mut devueltos = 0u32;
    let tam = std::mem::size_of::<KSIDENTIFIER>() as u32;
    unsafe { filtro.KsProperty(&propiedad, tam, std::ptr::null_mut(), 0, &mut devueltos) }.is_ok()
}

// ---------------------------------------------------------------- Olvidar

fn emparejado(direccion: u64) -> bool {
    conectado_clasico(direccion).is_some() || direcciones_le().contains(&direccion)
}

pub fn olvidar(direccion: &str) -> Result<(), String> {
    win::iniciar_com();
    let numero = win::direccion_a_numero(direccion).ok_or(NO_SE_PUDO_OLVIDAR)?;
    if !emparejado(numero) {
        return Err("No encontré ese dispositivo entre los emparejados".into());
    }

    // Primero la vía de Win32; si no basta (algunos LE), la de WinRT.
    if (quitar_win32(numero) && salio(numero)) || (quitar_winrt(numero) && salio(numero)) {
        Ok(())
    } else {
        Err(NO_SE_PUDO_OLVIDAR.into())
    }
}

/// Windows tarda un momento en sacarlo de la lista: se espera hasta 3 s.
fn salio(direccion: u64) -> bool {
    for _ in 0..12 {
        if !emparejado(direccion) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

/// BluetoothRemoveDevice: quita el emparejamiento y lo que Windows guardaba del aparato.
fn quitar_win32(direccion: u64) -> bool {
    let d = BLUETOOTH_ADDRESS { Anonymous: BLUETOOTH_ADDRESS_0 { ullLong: direccion } };
    unsafe { BluetoothRemoveDevice(&d) == 0 }
}

/// WinRT: UnpairAsync, como clásico o como LE.
fn quitar_winrt(direccion: u64) -> bool {
    let por_clasico = BluetoothDevice::FromBluetoothAddressAsync(direccion)
        .and_then(|op| op.join())
        .and_then(|a| a.DeviceInformation());
    let por_le = || {
        BluetoothLEDevice::FromBluetoothAddressAsync(direccion)
            .and_then(|op| op.join())
            .and_then(|a| a.DeviceInformation())
    };
    let Ok(info) = por_clasico.or_else(|_| por_le()) else { return false };
    let estado = info.Pairing().and_then(|p| p.UnpairAsync()).and_then(|op| op.join()).and_then(|r| r.Status());
    estado.is_ok_and(|e| e == DeviceUnpairingResultStatus::Unpaired || e == DeviceUnpairingResultStatus::AlreadyUnpaired)
}
