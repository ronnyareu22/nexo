//! Sonido en Windows: WASAPI (IMMDeviceEnumerator e IAudioEndpointVolume).
//!
//! - Por dirección Bluetooth: las salidas y micrófonos del aparato se encuentran por el nodo
//!   del que salen o por su contenedor (ver `win.rs`). Un audífono suele tener dos salidas:
//!   la estéreo (A2DP) y la de manos libres; se prefiere la que está en uso y, si no, la estéreo.
//! - Por UID (página Audio): el UID es el ID del punto de sonido de Windows.
//! - Elegir salida o micrófono: IPolicyConfig, la interfaz sin documentar que usa el propio
//!   Windows. Se aplica a los tres usos (consola, multimedia y comunicaciones).

use std::ffi::c_void;

use windows::Win32::Devices::FunctionDiscovery::{PKEY_Device_DeviceDesc, PKEY_Device_EnumeratorName, PKEY_Device_FriendlyName};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, DigitalAudioDisplayDevice, EDataFlow, ERole, Handset, Headset, IMMDevice, IMMDeviceEnumerator,
    PKEY_AudioEndpoint_FormFactor, PKEY_AudioEndpoint_JackSubType, eAll, eCapture, eCommunications, eConsole, eMultimedia, eRender,
};
use windows::Win32::Media::KernelStreaming::{KSNODETYPE_DISPLAYPORT_INTERFACE, KSNODETYPE_HDMI_INTERFACE};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::core::{GUID, HRESULT, HSTRING, IUnknown, IUnknown_Vtbl, Interface, PCWSTR};

use super::{Cambio, DispositivoAudio, Sonido};
use crate::dispositivos::win::{self, Punto};

// ---------------------------------------------------------------- IPolicyConfig

/// CPolicyConfigClient: la clase de Windows que cambia la salida y la entrada predeterminadas.
const CLASE_POLITICA: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

/// IPolicyConfig (sin documentar, Windows 7 a 11). Se declara a mano porque la macro
/// `interface` pide el crate `windows-core` directo; solo se usa SetDefaultEndpoint.
#[repr(transparent)]
#[derive(Clone)]
struct PoliticaDeSonido(IUnknown);

unsafe impl Interface for PoliticaDeSonido {
    type Vtable = PoliticaVtbl;
    const IID: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
}

#[repr(C)]
#[allow(non_snake_case, dead_code)]
struct PoliticaVtbl {
    base: IUnknown_Vtbl,
    // GetMixFormat, GetDeviceFormat, ResetDeviceFormat, SetDeviceFormat, GetProcessingPeriod,
    // SetProcessingPeriod, GetShareMode, SetShareMode, GetPropertyValue, SetPropertyValue.
    sin_usar: [*const c_void; 10],
    SetDefaultEndpoint: unsafe extern "system" fn(*mut c_void, PCWSTR, ERole) -> HRESULT,
    SetEndpointVisibility: unsafe extern "system" fn(*mut c_void, PCWSTR, i32) -> HRESULT,
}

/// Usa ese punto de sonido como predeterminado. true si Windows lo aceptó.
fn predeterminar(id: &str) -> bool {
    let Ok(politica) = (unsafe { CoCreateInstance::<_, PoliticaDeSonido>(&CLASE_POLITICA, None, CLSCTX_ALL) }) else {
        return false;
    };
    let texto = win::ancho(id);
    [eConsole, eMultimedia, eCommunications]
        .into_iter()
        .map(|rol| unsafe { (politica.vtable().SetDefaultEndpoint)(politica.as_raw(), PCWSTR(texto.as_ptr()), rol) }.is_ok())
        .fold(false, |a, b| a | b)
}

// ---------------------------------------------------------------- Volumen

fn control(aparato: &IMMDevice) -> Option<IAudioEndpointVolume> {
    unsafe { aparato.Activate(CLSCTX_ALL, None) }.ok()
}

fn porcentaje(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 100.0).round() as u8
}

fn volumen(aparato: &IMMDevice) -> Option<u8> {
    control(aparato).and_then(|c| unsafe { c.GetMasterVolumeLevelScalar() }.ok()).map(porcentaje)
}

fn silenciado(aparato: &IMMDevice) -> Option<bool> {
    control(aparato).and_then(|c| unsafe { c.GetMute() }.ok()).map(|m| m.as_bool())
}

fn poner_volumen(aparato: &IMMDevice, valor: u8) -> bool {
    let nivel = valor.min(100) as f32 / 100.0;
    control(aparato).is_some_and(|c| unsafe { c.SetMasterVolumeLevelScalar(nivel, std::ptr::null()) }.is_ok())
}

fn silenciar(aparato: &IMMDevice, si: bool) -> bool {
    control(aparato).is_some_and(|c| unsafe { c.SetMute(si, std::ptr::null()) }.is_ok())
}

/// El ID de la salida o el micrófono predeterminado (el de consola, el que muestra Windows).
fn predeterminado(e: &IMMDeviceEnumerator, sentido: EDataFlow) -> Option<String> {
    let aparato = unsafe { e.GetDefaultAudioEndpoint(sentido, eConsole) }.ok()?;
    win::id_de(&aparato)
}

// ---------------------------------------------------------------- Por dirección Bluetooth

/// La de manos libres (llamadas) suena peor: se elige solo si no hay otra.
fn es_manos_libres(p: &Punto) -> bool {
    p.forma.is_some_and(|f| f == Headset.0 as u32 || f == Handset.0 as u32)
}

/// La salida (o el micrófono) del aparato que corresponde mostrar: la que está en uso o, si no, la estéreo.
fn elegir<'a>(puntos: &'a [Punto], salida: bool, actual: Option<&str>) -> Option<&'a Punto> {
    let mismos = || puntos.iter().filter(move |p| p.salida == salida);
    mismos()
        .find(|p| Some(p.id.as_str()) == actual)
        .or_else(|| mismos().find(|p| !es_manos_libres(p)))
        .or_else(|| mismos().next())
}

pub fn estado(direccion: &str) -> Result<Sonido, String> {
    win::iniciar_com();
    let (Some(numero), Some(e)) = (win::direccion_a_numero(direccion), win::enumerador()) else {
        return Ok(Sonido::default());
    };
    let puntos = win::puntos_bluetooth(&e, numero, DEVICE_STATE_ACTIVE.0);
    let salida_actual = predeterminado(&e, eRender);
    let entrada_actual = predeterminado(&e, eCapture);
    let salida = elegir(&puntos, true, salida_actual.as_deref());
    let entrada = elegir(&puntos, false, entrada_actual.as_deref());
    Ok(Sonido {
        tiene_salida: salida.is_some(),
        salida_en_uso: salida.is_some_and(|p| Some(p.id.as_str()) == salida_actual.as_deref()),
        volumen: salida.and_then(|p| volumen(&p.aparato)),
        silenciado: salida.and_then(|p| silenciado(&p.aparato)),
        tiene_microfono: entrada.is_some(),
        microfono_en_uso: entrada.is_some_and(|p| Some(p.id.as_str()) == entrada_actual.as_deref()),
        volumen_microfono: entrada.and_then(|p| volumen(&p.aparato)),
    })
}

pub fn ajustar(direccion: &str, cambio: Cambio) -> Result<(), String> {
    win::iniciar_com();
    let numero = win::direccion_a_numero(direccion).ok_or("No encontré ese aparato de sonido")?;
    let e = win::enumerador().ok_or("No se pudo abrir el sonido de Windows")?;
    let puntos = win::puntos_bluetooth(&e, numero, DEVICE_STATE_ACTIVE.0);
    // Para usarlo como salida se elige la estéreo, aunque ahora suene por manos libres.
    let salida_actual = if matches!(cambio, Cambio::UsarSalida) { None } else { predeterminado(&e, eRender) };
    let entrada_actual = predeterminado(&e, eCapture);
    let salida = elegir(&puntos, true, salida_actual.as_deref());
    let entrada = elegir(&puntos, false, entrada_actual.as_deref());
    aplicar(salida.map(|p| (&p.aparato, p.id.as_str())), entrada.map(|p| (&p.aparato, p.id.as_str())), cambio)
}

/// Aplica un cambio a la salida o al micrófono de un aparato ya encontrado.
fn aplicar(salida: Option<(&IMMDevice, &str)>, entrada: Option<(&IMMDevice, &str)>, cambio: Cambio) -> Result<(), String> {
    let sin_salida = || "Este aparato no tiene salida de sonido activa".to_string();
    let sin_microfono = || "Este aparato no tiene micrófono activo".to_string();
    let ok = match cambio {
        Cambio::UsarSalida => predeterminar(salida.ok_or_else(sin_salida)?.1),
        Cambio::Volumen(v) => poner_volumen(salida.ok_or_else(sin_salida)?.0, v),
        Cambio::Silenciar(s) => silenciar(salida.ok_or_else(sin_salida)?.0, s),
        Cambio::UsarMicrofono => predeterminar(entrada.ok_or_else(sin_microfono)?.1),
        Cambio::VolumenMicrofono(v) => poner_volumen(entrada.ok_or_else(sin_microfono)?.0, v),
        Cambio::SilenciarMicrofono(s) => silenciar(entrada.ok_or_else(sin_microfono)?.0, s),
    };
    if ok { Ok(()) } else { Err("Windows no aceptó el cambio de sonido".into()) }
}

// ---------------------------------------------------------------- Todos los aparatos (página Audio)

/// Cómo se conecta: por el nodo del que sale ("{1}.BTHENUM\…", "{1}.USB\…") y por su forma.
fn transporte(tienda: &IPropertyStore) -> &'static str {
    // Si falta la ruta, Windows también guarda el nombre del enumerador ("BTHENUM", "USB"…).
    let ruta = win::texto(tienda, &win::RUTA_DEL_APARATO)
        .or_else(|| win::texto(tienda, &PKEY_Device_EnumeratorName))
        .unwrap_or_default()
        .to_uppercase();
    let sin_prefijo = ruta.split_once("}.").map_or(ruta.as_str(), |(_, r)| r);
    let enumerador = sin_prefijo.split('\\').next().unwrap_or("");
    let forma = win::numero(tienda, &PKEY_AudioEndpoint_FormFactor);
    let conector = win::guid(tienda, &PKEY_AudioEndpoint_JackSubType);

    if enumerador.starts_with("BTH") {
        "bluetooth"
    } else if enumerador.starts_with("USB") {
        "usb"
    } else if conector == Some(KSNODETYPE_DISPLAYPORT_INTERFACE) {
        "displayport"
    } else if conector == Some(KSNODETYPE_HDMI_INTERFACE) || forma == Some(DigitalAudioDisplayDevice.0 as u32) {
        // El sonido de un monitor (HDMI o DisplayPort, sin más detalle): se muestra como HDMI.
        "hdmi"
    } else if ["HDAUDIO", "INTELAUDIO", "ACPI", "PCI", "SST", "AMDAUDIO"].contains(&enumerador) {
        "integrado"
    } else if ["ROOT", "SWD"].contains(&enumerador) {
        "virtual"
    } else {
        "otro"
    }
}

/// Todas las salidas y micrófonos activos. Primero los que están en uso, luego los integrados
/// y el resto por nombre (igual que en la Mac). Un aparato con salida y micrófono aparece dos veces.
pub fn listar() -> Vec<DispositivoAudio> {
    win::iniciar_com();
    let Some(e) = win::enumerador() else { return Vec::new() };
    let salida_actual = predeterminado(&e, eRender);
    let entrada_actual = predeterminado(&e, eCapture);
    let Ok(coleccion) = (unsafe { e.EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE) }) else {
        return Vec::new();
    };
    let cantidad = unsafe { coleccion.GetCount() }.unwrap_or(0);
    let mut lista: Vec<DispositivoAudio> = (0..cantidad)
        .filter_map(|i| {
            let aparato = unsafe { coleccion.Item(i) }.ok()?;
            let uid = win::id_de(&aparato)?;
            let salida = win::es_salida(&aparato)?;
            let tienda = win::tienda(&aparato);
            let nombre = tienda
                .as_ref()
                .and_then(|t| win::texto(t, &PKEY_Device_FriendlyName).or_else(|| win::texto(t, &PKEY_Device_DeviceDesc)))
                .unwrap_or_else(|| uid.clone());
            let control = control(&aparato);
            let nivel = control.as_ref().and_then(|c| unsafe { c.GetMasterVolumeLevelScalar() }.ok()).map(porcentaje);
            let mudo = control.as_ref().and_then(|c| unsafe { c.GetMute() }.ok()).map(|m| m.as_bool());
            Some(DispositivoAudio {
                salida_en_uso: salida && salida_actual.as_deref() == Some(uid.as_str()),
                entrada_en_uso: !salida && entrada_actual.as_deref() == Some(uid.as_str()),
                volumen: if salida { nivel } else { None },
                silenciado: if salida { mudo } else { None },
                volumen_entrada: if salida { None } else { nivel },
                silenciado_entrada: if salida { None } else { mudo },
                transporte: tienda.as_ref().map_or("otro", transporte).into(),
                salida,
                entrada: !salida,
                nombre,
                uid,
            })
        })
        .collect();
    lista.sort_by_cached_key(|d| (!(d.salida_en_uso || d.entrada_en_uso), d.transporte != "integrado", d.nombre.to_lowercase()));
    lista
}

/// Aplica un cambio al punto de sonido con ese UID.
pub fn ajustar_por_uid(uid: &str, cambio: Cambio) -> Result<(), String> {
    win::iniciar_com();
    let desconectado = || "Ese aparato de sonido ya no está conectado".to_string();
    let e = win::enumerador().ok_or_else(desconectado)?;
    let aparato = unsafe { e.GetDevice(&HSTRING::from(uid)) }.map_err(|_| desconectado())?;
    if unsafe { aparato.GetState() }.ok() != Some(DEVICE_STATE_ACTIVE) {
        return Err(desconectado());
    }
    let salida = win::es_salida(&aparato).ok_or_else(desconectado)?;
    let este = (&aparato, uid);
    aplicar(salida.then_some(este), (!salida).then_some(este), cambio)
}

// ---------------------------------------------------------------- Probar sonido y volumen de avisos

/// Suena el aviso "Predeterminado" de Windows por la salida en uso.
pub fn probar() -> Result<(), String> {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBeep(tipo: u32) -> i32;
    }
    const MB_OK: u32 = 0;
    if unsafe { MessageBeep(MB_OK) } != 0 { Ok(()) } else { Err("Windows no pudo sonar el aviso".into()) }
}

/// En Windows los avisos siguen al volumen general: no hay uno aparte.
pub fn volumen_avisos() -> Option<u8> {
    None
}

pub fn poner_volumen_avisos(_valor: u8) -> Result<(), String> {
    Err("En Windows los avisos siguen al volumen general.".into())
}
