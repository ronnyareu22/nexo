//! Pantallas en Windows.
//!
//! - Lista: EnumDisplayMonitors + GetMonitorInfoW. El nombre real (EDID) y si es la integrada salen de
//!   QueryDisplayConfig + DisplayConfigGetDeviceInfo.
//! - Brillo de los monitores externos: DDC/CI con dxva2 (código VCP 0x10). Es lento (decenas de ms), así que
//!   solo lo toca el hilo de pantallas, que es dueño de los manejadores de monitor físico.
//! - Brillo de la pantalla del portátil: WMI (root\wmi, WmiMonitorBrightness). Si falla, no es controlable.
//! - Resolución, frecuencia y pantalla principal: EnumDisplaySettingsExW y ChangeDisplaySettingsExW
//!   (funciones sueltas: no tocan DDC, así que no pasan por el hilo).
//!
//! - Ajustes del monitor (contraste, color, entrada…): los mismos manejadores DDC, más su lista de capacidades
//!   (GetCapabilitiesStringLength + CapabilitiesRequestAndCapabilitiesReply), leída una vez por monitor.
//!
//! - Teclas de brillo del portátil: su hilo escucha WmiMonitorBrightnessEvent. Si el mouse está sobre un monitor
//!   externo, ese cambio pasa al monitor y el portátil vuelve a su nivel (ver `vigilar_portatil`).
//!
//! El `id` de cada pantalla es una huella de la ruta del monitor que da Windows
//! (\\?\DISPLAY#...): no cambia mientras siga conectado al mismo puerto.

use super::{Avisos, ModoPantalla, NO_RESPONDE, Orden, Pantalla, Pantallas, Sistema, minimo_en, vcp};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL,
    DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED, DISPLAYCONFIG_PATH_INFO,
    CapabilitiesRequestAndCapabilitiesReply, DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME,
    DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY, DestroyPhysicalMonitors, DisplayConfigGetDeviceInfo, GetCapabilitiesStringLength,
    GetDisplayConfigBufferSizes,
    GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR, GetVCPFeatureAndVCPFeatureReply,
    PHYSICAL_MONITOR, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig, SetVCPFeature,
};
use windows::Win32::Foundation::{
    ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, HANDLE, LPARAM, POINT, POINTL, RECT, RPC_E_CHANGED_MODE,
};
use windows::Win32::Graphics::Gdi::{
    CDS_NORESET, CDS_SET_PRIMARY, CDS_TEST, CDS_TYPE, CDS_UPDATEREGISTRY, ChangeDisplaySettingsExW, DEVMODEW,
    DISP_CHANGE, DISP_CHANGE_BADDUALVIEW, DISP_CHANGE_BADMODE, DISP_CHANGE_FAILED, DISP_CHANGE_NOTUPDATED,
    DISP_CHANGE_RESTART, DISP_CHANGE_SUCCESSFUL, DM_BITSPERPEL, DM_DISPLAYFREQUENCY, DM_INTERLACED, DM_PELSHEIGHT,
    DM_PELSWIDTH, DM_POSITION, ENUM_CURRENT_SETTINGS, ENUM_DISPLAY_SETTINGS_FLAGS, ENUM_DISPLAY_SETTINGS_MODE,
    EnumDisplayMonitors, EnumDisplaySettingsExW, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MONITORINFOEXW, MonitorFromPoint,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoSetProxyBlanket, CoUninitialize,
    EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE,
};
use windows::Win32::System::Variant::{VARENUM, VARIANT, VT_BOOL, VT_BSTR, VT_I2, VT_I4, VT_UI1, VariantClear};
use windows::Win32::System::Wmi::{
    IWbemClassObject, IWbemLocator, IWbemServices, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY,
    WBEM_GENERIC_FLAG_TYPE, WbemLocator,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, MONITORINFOF_PRIMARY};
use windows::core::{BOOL, BSTR, PCWSTR, w};

const BRILLO_VCP: u8 = vcp::BRILLO;
/// Un monitor que no respondió a DDC se vuelve a probar cada tanto (la persona puede activar DDC/CI en su menú).
const REINTENTO_DDC: Duration = Duration::from_secs(30);
/// Si WMI falla o no da el brillo, no se insiste en cada lectura.
const REINTENTO_WMI: Duration = Duration::from_secs(60);
// RPC_C_AUTHN_WINNT y RPC_C_AUTHZ_NONE (están en Win32_System_Rpc; no vale sumar la función entera por dos números).
const AUTENTICACION_WINNT: u32 = 10;
const AUTORIZACION_NINGUNA: u32 = 0;

// ---------------------------------------------------------------- Pantallas activas

/// Una pantalla activa, leída en el momento (no guarda nada del sistema abierto).
struct Monitor {
    id: u32,
    hmonitor: HMONITOR,
    /// Nombre GDI de la salida (\\.\DISPLAY1), terminado en cero: lo piden las funciones de modos.
    dispositivo: [u16; 32],
    /// Nombre del monitor en su EDID ("MNT-027"), si lo da.
    nombre: Option<String>,
    /// Ruta del monitor en minúsculas (\\?\display#...), para reconocerlo en WMI.
    ruta: String,
    interna: bool,
    /// Su salida también muestra la pantalla del portátil (pantallas duplicadas, o es ella).
    con_integrada: bool,
    principal: bool,
    /// Dónde está en el escritorio, en píxeles (para ordenar de izquierda a derecha y ubicar widgets).
    rect: RECT,
    /// Zona útil (sin la barra de tareas), en píxeles.
    trabajo: RECT,
}

/// Lo que dice QueryDisplayConfig de cada monitor conectado a una salida.
struct Destino {
    nombre: String,
    ruta: String,
    interna: bool,
}

fn texto(buf: &[u16]) -> String {
    let fin = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..fin]).trim().to_string()
}

/// Huella FNV-1a de 32 bits (nunca 0).
fn huella(texto: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in texto.to_lowercase().bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h.max(1)
}

fn es_integrada(t: DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY) -> bool {
    t == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL
        || t == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED
        || t == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED
        || t == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS
}

/// Monitores de cada salida GDI (\\.\display1 → sus monitores). Si se duplican pantallas, una salida tiene varios.
fn destinos() -> HashMap<String, Vec<Destino>> {
    let mut mapa: HashMap<String, Vec<Destino>> = HashMap::new();
    let mut rutas: Vec<DISPLAYCONFIG_PATH_INFO> = Vec::new();
    // Si algo cambia entre medir y leer, Windows pide un búfer más grande: se reintenta.
    for _ in 0..3 {
        let (mut n_rutas, mut n_modos) = (0u32, 0u32);
        if unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut n_rutas, &mut n_modos) } != ERROR_SUCCESS {
            return mapa;
        }
        rutas = vec![DISPLAYCONFIG_PATH_INFO::default(); n_rutas as usize];
        let mut modos = vec![DISPLAYCONFIG_MODE_INFO::default(); n_modos as usize];
        let r = unsafe {
            QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut n_rutas, rutas.as_mut_ptr(), &mut n_modos, modos.as_mut_ptr(), None)
        };
        if r == ERROR_SUCCESS {
            rutas.truncate(n_rutas as usize);
            break;
        }
        rutas.clear();
        if r != ERROR_INSUFFICIENT_BUFFER {
            return mapa;
        }
    }

    for ruta in &rutas {
        let mut fuente = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
        fuente.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
        fuente.header.size = size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
        fuente.header.adapterId = ruta.sourceInfo.adapterId;
        fuente.header.id = ruta.sourceInfo.id;
        let puntero = &mut fuente as *mut DISPLAYCONFIG_SOURCE_DEVICE_NAME as *mut DISPLAYCONFIG_DEVICE_INFO_HEADER;
        if unsafe { DisplayConfigGetDeviceInfo(puntero) } != 0 {
            continue;
        }

        let mut destino = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
        destino.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
        destino.header.size = size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
        destino.header.adapterId = ruta.targetInfo.adapterId;
        destino.header.id = ruta.targetInfo.id;
        let puntero = &mut destino as *mut DISPLAYCONFIG_TARGET_DEVICE_NAME as *mut DISPLAYCONFIG_DEVICE_INFO_HEADER;
        let leido = unsafe { DisplayConfigGetDeviceInfo(puntero) } == 0;

        let gdi = texto(&fuente.viewGdiDeviceName).to_lowercase();
        mapa.entry(gdi).or_default().push(Destino {
            nombre: if leido { texto(&destino.monitorFriendlyDeviceName) } else { String::new() },
            ruta: if leido { texto(&destino.monitorDevicePath).to_lowercase() } else { String::new() },
            interna: es_integrada(ruta.targetInfo.outputTechnology) || (leido && es_integrada(destino.outputTechnology)),
        });
    }
    mapa
}

unsafe extern "system" fn juntar(h: HMONITOR, _: HDC, _: *mut RECT, datos: LPARAM) -> BOOL {
    // SAFETY: `datos` es el Vec que pasa `monitores`, vivo durante toda la enumeración.
    let lista = unsafe { &mut *(datos.0 as *mut Vec<HMONITOR>) };
    lista.push(h);
    BOOL(1)
}

/// Las pantallas activas ahora. Es barato (no toca DDC ni WMI).
fn monitores() -> Vec<Monitor> {
    let mut handles: Vec<HMONITOR> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(juntar), LPARAM(&mut handles as *mut Vec<HMONITOR> as isize));
    }
    let destinos = destinos();
    let mut lista = Vec::new();
    for h in handles {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if !unsafe { GetMonitorInfoW(h, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) }.as_bool() {
            continue;
        }
        let mut dispositivo = info.szDevice;
        dispositivo[31] = 0;
        let gdi = texto(&dispositivo).to_lowercase();
        // Al duplicar pantallas, el monitor externo manda (es el que se controla por DDC).
        let salida = destinos.get(&gdi);
        let destino = salida.and_then(|v| v.iter().find(|d| !d.interna).or(v.first()));
        let id = match destino {
            Some(d) if !d.ruta.is_empty() => huella(&d.ruta),
            _ => huella(&gdi),
        };
        lista.push(Monitor {
            id,
            hmonitor: h,
            dispositivo,
            nombre: destino.map(|d| d.nombre.clone()).filter(|n| !n.is_empty()),
            ruta: destino.map(|d| d.ruta.clone()).unwrap_or_default(),
            interna: destino.is_some_and(|d| d.interna),
            con_integrada: salida.is_some_and(|v| v.iter().any(|d| d.interna)),
            principal: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            rect: info.monitorInfo.rcMonitor,
            trabajo: info.monitorInfo.rcWork,
        });
    }
    lista
}

fn buscar(id: u32) -> Option<Monitor> {
    monitores().into_iter().find(|m| m.id == id)
}

fn monitor_bajo_el_cursor() -> Option<Monitor> {
    let mut punto = POINT::default();
    // Falla con la pantalla bloqueada o en el escritorio seguro: entonces no hay pantalla.
    unsafe { GetCursorPos(&mut punto) }.ok()?;
    let h = unsafe { MonitorFromPoint(punto, MONITOR_DEFAULTTONEAREST) };
    if h.is_invalid() {
        return None;
    }
    monitores().into_iter().find(|m| m.hmonitor == h)
}

/// La pantalla que está bajo el mouse (para las teclas de brillo). No toca DDC ni WMI.
pub fn id_bajo_el_cursor() -> Option<u32> {
    monitor_bajo_el_cursor().map(|m| m.id)
}

/// La pantalla del portátil, si está encendida (con la tapa cerrada no está).
pub fn id_integrada() -> Option<u32> {
    monitores().into_iter().find(|m| m.interna).map(|m| m.id)
}

/// Si la PC tiene pantalla de portátil encendida (para mostrar "teclas del portátil"). No toca DDC ni WMI.
pub fn hay_integrada() -> bool {
    id_integrada().is_some()
}

/// El monitor externo bajo el mouse. Nada si es la pantalla del portátil o si la duplica.
fn externa_bajo_el_cursor() -> Option<u32> {
    monitor_bajo_el_cursor().filter(|m| !m.interna && !m.con_integrada).map(|m| m.id)
}

/// Zona útil de una pantalla (sin la barra de tareas), en píxeles físicos, y su escala (1,0 = 96 ppp).
pub struct AreaTrabajo {
    pub izquierda: i32,
    pub arriba: i32,
    pub derecha: i32,
    pub abajo: i32,
    pub escala: f64,
}

/// Dónde está una pantalla en el escritorio, en píxeles físicos: x, y, ancho, alto (para el widget de brillo).
pub fn limites(id: u32) -> Option<(f64, f64, f64, f64)> {
    let r = buscar(id)?.rect;
    (r.right > r.left && r.bottom > r.top).then_some((
        r.left as f64,
        r.top as f64,
        (r.right - r.left) as f64,
        (r.bottom - r.top) as f64,
    ))
}

/// Para ubicar el aviso de brillo en la pantalla que cambió.
pub fn area_de_trabajo(id: u32) -> Option<AreaTrabajo> {
    let m = buscar(id)?;
    let (mut ppp_x, mut ppp_y) = (0u32, 0u32);
    let escala = match unsafe { GetDpiForMonitor(m.hmonitor, MDT_EFFECTIVE_DPI, &mut ppp_x, &mut ppp_y) } {
        Ok(()) if ppp_x > 0 => ppp_x as f64 / 96.0,
        _ => 1.0,
    };
    let t = m.trabajo;
    (t.right > t.left && t.bottom > t.top).then_some(AreaTrabajo {
        izquierda: t.left,
        arriba: t.top,
        derecha: t.right,
        abajo: t.bottom,
        escala,
    })
}

// ---------------------------------------------------------------- Brillo: DDC/CI y WMI

/// Manejadores de monitor físico de una pantalla; se destruyen al soltarlos.
struct Fisicos(Vec<PHYSICAL_MONITOR>);

impl Drop for Fisicos {
    fn drop(&mut self) {
        if !self.0.is_empty() {
            let _ = unsafe { DestroyPhysicalMonitors(&self.0) };
        }
    }
}

enum Via {
    /// Monitor externo por DDC/CI. `cual` es el monitor físico que respondió.
    Ddc { hmonitor: isize, fisicos: Fisicos, cual: usize },
    /// Pantalla del portátil por WMI. `ruta` es la instancia de WmiMonitorBrightnessMethods (__PATH).
    Wmi { ruta: String },
}

/// Una pantalla a la que Nexo le puede cambiar el brillo.
struct Control {
    via: Via,
    maximo: u32,
    actual: u32,
    /// Brillo mínimo (0–40 %): el 0 % de Nexo. En la pantalla del portátil es 0.
    minimo: u8,
}

impl Control {
    fn porcentaje(&self) -> u8 {
        vcp::a_porcentaje(self.actual, self.minimo, self.maximo)
    }

    fn manejador(&self) -> Option<HANDLE> {
        match &self.via {
            // Copia del campo (PHYSICAL_MONITOR es empaquetado: no se puede tomar una referencia a él).
            Via::Ddc { fisicos, cual, .. } => fisicos.0.get(*cual).map(|f| f.hPhysicalMonitor),
            Via::Wmi { .. } => None,
        }
    }
}

/// Lee un código VCP: (actual, máximo). Si el monitor no contesta a tiempo, se reintenta.
/// En el brillo el máximo tiene que ser mayor que 0; en los demás lo revisa `vcp`.
fn leer_vcp(h: HANDLE, codigo: u8, intentos: u32) -> Option<(u32, u32)> {
    for intento in 0..intentos {
        let (mut actual, mut maximo) = (0u32, 0u32);
        let ok = unsafe { GetVCPFeatureAndVCPFeatureReply(h, codigo, None, &mut actual, Some(&mut maximo as *mut u32)) };
        if ok != 0 && (codigo != BRILLO_VCP || maximo > 0) {
            return Some((if codigo == BRILLO_VCP { actual.min(maximo) } else { actual }, maximo));
        }
        if intento + 1 < intentos {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    None
}

fn escribir_vcp(h: HANDLE, codigo: u8, valor: u32) -> bool {
    for intento in 0..2 {
        if unsafe { SetVCPFeature(h, codigo, valor) } != 0 {
            return true;
        }
        if intento == 0 {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    false
}

/// La cadena de capacidades del monitor ("(prot(monitor)…vcp(02 04 10 12 14(05 08 0B)…)…)"). Es lenta: se pide una vez.
fn capacidades_ddc(h: HANDLE) -> Option<String> {
    for intento in 0..2 {
        let mut largo = 0u32;
        if unsafe { GetCapabilitiesStringLength(h, &mut largo) } != 0 && (1..=16_384).contains(&largo) {
            let mut buf = vec![0u8; largo as usize];
            if unsafe { CapabilitiesRequestAndCapabilitiesReply(h, &mut buf) } != 0 {
                let fin = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
                return Some(String::from_utf8_lossy(&buf[..fin]).into_owned());
            }
        }
        if intento == 0 {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    None
}

/// Abre los monitores físicos de una pantalla y se queda con el primero que responde a DDC.
/// Si ninguno responde (o DDC/CI está apagado en su menú), los manejadores se destruyen y da None.
fn abrir_ddc(h: HMONITOR, intentos: u32) -> Option<Control> {
    let mut n = 0u32;
    unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(h, &mut n) }.ok()?;
    if n == 0 || n > 8 {
        return None;
    }
    let mut lista: Vec<PHYSICAL_MONITOR> = (0..n).map(|_| PHYSICAL_MONITOR::default()).collect();
    unsafe { GetPhysicalMonitorsFromHMONITOR(h, &mut lista) }.ok()?;
    let fisicos = Fisicos(lista);
    let (cual, (actual, maximo)) = fisicos
        .0
        .iter()
        .enumerate()
        .find_map(|(i, f)| leer_vcp(f.hPhysicalMonitor, BRILLO_VCP, intentos).map(|l| (i, l)))?;
    Some(Control { via: Via::Ddc { hmonitor: h.0 as isize, fisicos, cual }, maximo, actual, minimo: 0 })
}

/// "\\?\display#boe0812#4&2d4a&0&uid8388688#{...}" → "display\boe0812\4&2d4a&0&uid8388688"
fn clave_ruta(ruta: &str) -> String {
    let sin_prefijo = ruta.trim_start_matches("\\\\?\\");
    sin_prefijo.split('#').take(3).collect::<Vec<_>>().join("\\").to_lowercase()
}

/// "DISPLAY\BOE0812\4&2d4a&0&UID8388688_0" → "display\boe0812\4&2d4a&0&uid8388688"
fn clave_instancia(instancia: &str) -> String {
    let i = instancia.to_lowercase();
    match i.rsplit_once('_') {
        Some((base, n)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => base.to_string(),
        _ => i,
    }
}

struct LecturaWmi {
    instancia: String,
    brillo: u8,
}

/// Conexión a WMI (root\wmi), abierta solo si hay una pantalla integrada.
struct Wmi {
    /// COM quedó listo en este hilo.
    com: bool,
    servicios: Option<IWbemServices>,
    /// Cuándo falló (o no dio nada) la última vez.
    fallo: Option<Instant>,
}

impl Wmi {
    fn conectar(&mut self) -> Option<IWbemServices> {
        if !self.com {
            return None;
        }
        if let Some(s) = &self.servicios {
            return Some(s.clone());
        }
        if self.fallo.is_some_and(|t| t.elapsed() < REINTENTO_WMI) {
            return None;
        }
        match abrir_wmi() {
            Ok(s) => {
                self.servicios = Some(s.clone());
                Some(s)
            }
            Err(_) => {
                self.fallo = Some(Instant::now());
                None
            }
        }
    }

    fn fallar(&mut self) {
        self.servicios = None;
        self.fallo = Some(Instant::now());
    }

    /// Brillo de cada pantalla integrada que WMI conoce (en una de escritorio no da nada).
    fn brillos(&mut self) -> Option<Vec<LecturaWmi>> {
        let s = self.conectar()?;
        let objetos = consultar(&s, "SELECT InstanceName, CurrentBrightness FROM WmiMonitorBrightness WHERE Active = TRUE");
        let lecturas: Vec<LecturaWmi> = objetos
            .iter()
            .filter_map(|o| {
                Some(LecturaWmi {
                    instancia: leer_texto(o, w!("InstanceName"))?,
                    brillo: leer_numero(o, w!("CurrentBrightness"))?.clamp(0, 100) as u8,
                })
            })
            .collect();
        if lecturas.is_empty() {
            self.fallar();
            return None;
        }
        self.fallo = None;
        Some(lecturas)
    }

    fn ruta_metodos(&mut self, instancia: &str) -> Option<String> {
        let s = self.conectar()?;
        ruta_metodos(&s, instancia)
    }

    /// WmiSetBrightness(Timeout = 0, Brightness = valor).
    fn poner(&mut self, ruta: &str, valor: u8) -> Result<(), String> {
        let s = self.conectar().ok_or("Windows no deja cambiar el brillo de esta pantalla")?;
        let r = poner_wmi(&s, ruta, valor);
        if r.is_err() {
            // Quizá la conexión quedó vieja: se abre de nuevo la próxima vez.
            self.servicios = None;
        }
        r.map_err(|e| {
            let detalle = e.message();
            let detalle = detalle.trim();
            if detalle.is_empty() {
                format!("Windows no aceptó el cambio de brillo (código {:#010x}).", e.code().0)
            } else {
                format!("Windows no aceptó el cambio de brillo ({detalle}).")
            }
        })
    }
}

fn abrir_wmi() -> windows::core::Result<IWbemServices> {
    unsafe {
        let localizador: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
        let vacio = BSTR::new();
        let s = localizador.ConnectServer(&BSTR::from("ROOT\\WMI"), &vacio, &vacio, &vacio, 0, &vacio, None)?;
        // Sin esto WMI puede negar el acceso: el nivel de suplantación por defecto del proceso no le alcanza.
        CoSetProxyBlanket(
            &s,
            AUTENTICACION_WINNT,
            AUTORIZACION_NINGUNA,
            PCWSTR::null(),
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )?;
        Ok(s)
    }
}

/// La ruta (__PATH) del objeto con el método WmiSetBrightness de esa pantalla.
fn ruta_metodos(s: &IWbemServices, instancia: &str) -> Option<String> {
    let objetos = consultar(s, "SELECT * FROM WmiMonitorBrightnessMethods WHERE Active = TRUE");
    let rutas: Vec<(String, String)> = objetos
        .iter()
        .filter_map(|o| Some((leer_texto(o, w!("InstanceName"))?, leer_texto(o, w!("__PATH"))?)))
        .collect();
    let elegida = rutas.iter().find(|(i, _)| i.eq_ignore_ascii_case(instancia)).or(rutas.first());
    elegida.map(|(_, r)| r.clone())
}

fn consultar(s: &IWbemServices, wql: &str) -> Vec<IWbemClassObject> {
    let mut lista = Vec::new();
    let banderas = WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY;
    let Ok(enumerador) = (unsafe { s.ExecQuery(&BSTR::from("WQL"), &BSTR::from(wql), banderas, None) }) else {
        return lista;
    };
    for _ in 0..16 {
        let mut objetos: [Option<IWbemClassObject>; 1] = [None];
        let mut n = 0u32;
        // Espera hasta 2 s por objeto. En una PC de escritorio aquí llega "no compatible" y se termina.
        let hr = unsafe { enumerador.Next(2000, &mut objetos, &mut n) };
        if hr.is_err() || n == 0 {
            break;
        }
        match objetos[0].take() {
            Some(o) => lista.push(o),
            None => break,
        }
    }
    lista
}

enum Valor {
    Numero(i64),
    Texto(String),
}

fn leer(o: &IWbemClassObject, nombre: PCWSTR) -> Option<Valor> {
    unsafe {
        let mut v = VARIANT::default();
        o.Get(nombre, 0, &mut v, None, None).ok()?;
        let valor = {
            let dentro = &v.Anonymous.Anonymous;
            let tipo: VARENUM = dentro.vt;
            if tipo == VT_UI1 {
                Some(Valor::Numero(dentro.Anonymous.bVal as i64))
            } else if tipo == VT_I4 {
                Some(Valor::Numero(dentro.Anonymous.lVal as i64))
            } else if tipo == VT_I2 {
                Some(Valor::Numero(dentro.Anonymous.iVal as i64))
            } else if tipo == VT_BOOL {
                Some(Valor::Numero((dentro.Anonymous.boolVal.0 != 0) as i64))
            } else if tipo == VT_BSTR {
                Some(Valor::Texto(dentro.Anonymous.bstrVal.to_string()))
            } else {
                None
            }
        };
        // Libera el texto (BSTR) que WMI dejó en la variante.
        let _ = VariantClear(&mut v);
        valor
    }
}

fn leer_texto(o: &IWbemClassObject, nombre: PCWSTR) -> Option<String> {
    match leer(o, nombre)? {
        Valor::Texto(t) if !t.is_empty() => Some(t),
        _ => None,
    }
}

fn leer_numero(o: &IWbemClassObject, nombre: PCWSTR) -> Option<i64> {
    match leer(o, nombre)? {
        Valor::Numero(n) => Some(n),
        Valor::Texto(_) => None,
    }
}

/// Una variante numérica (VT_UI1 para uint8, VT_I4 para uint32, como los pide WMI).
fn variante(tipo: VARENUM, valor: i32) -> VARIANT {
    let mut v = VARIANT::default();
    unsafe {
        let dentro = &mut *v.Anonymous.Anonymous;
        dentro.vt = tipo;
        if tipo == VT_UI1 {
            dentro.Anonymous.bVal = valor.clamp(0, 255) as u8;
        } else {
            dentro.Anonymous.lVal = valor;
        }
    }
    v
}

fn poner_wmi(s: &IWbemServices, ruta: &str, valor: u8) -> windows::core::Result<()> {
    unsafe {
        let mut clase: Option<IWbemClassObject> = None;
        s.GetObject(
            &BSTR::from("WmiMonitorBrightnessMethods"),
            WBEM_GENERIC_FLAG_TYPE(0),
            None,
            Some(&mut clase as *mut Option<IWbemClassObject>),
            None,
        )?;
        let clase = clase.ok_or_else(windows::core::Error::empty)?;
        let (mut entrada, mut salida): (Option<IWbemClassObject>, Option<IWbemClassObject>) = (None, None);
        clase.GetMethod(w!("WmiSetBrightness"), 0, &mut entrada, &mut salida)?;
        let parametros = entrada.ok_or_else(windows::core::Error::empty)?.SpawnInstance(0)?;
        parametros.Put(w!("Timeout"), 0, &variante(VT_I4, 0), 0)?;
        parametros.Put(w!("Brightness"), 0, &variante(VT_UI1, valor as i32), 0)?;
        s.ExecMethod(
            &BSTR::from(ruta),
            &BSTR::from("WmiSetBrightness"),
            WBEM_GENERIC_FLAG_TYPE(0),
            None,
            &parametros,
            None,
            None,
        )
    }
}

// ---------------------------------------------------------------- Hilo de pantallas

/// Lo que es del hilo de pantallas: los manejadores DDC, la conexión a WMI, lo leído de cada monitor y sus mínimos.
struct Estado {
    controles: HashMap<u32, Control>,
    /// Monitores que no respondieron a DDC y cuándo se probaron.
    fallidos: HashMap<u32, Instant>,
    wmi: Wmi,
    /// Capacidades y máximos de cada monitor (se leen una vez; sobreviven a reabrir sus manejadores).
    memorias: HashMap<u32, vcp::Memoria>,
    /// Brillo mínimo guardado de cada monitor externo.
    minimos: HashMap<u32, u8>,
}

const NO_ACEPTO: &str = "El monitor no aceptó el cambio. Revisa que DDC/CI siga activo en su menú.";

impl Estado {
    /// Mira qué pantallas hay. Olvida las desconectadas (destruye sus manejadores), abre DDC solo en las nuevas
    /// y relee el brillo de la integrada por WMI (es rápido; DDC no se relee).
    fn buscar(&mut self) -> Vec<Monitor> {
        let lista = monitores();
        let activos: HashSet<u32> = lista.iter().map(|m| m.id).collect();
        self.controles.retain(|id, _| activos.contains(id));
        self.fallidos.retain(|id, _| activos.contains(id));
        self.memorias.retain(|id, _| activos.contains(id));

        let mut internas: Vec<(u32, String)> = Vec::new();
        for m in &lista {
            if m.interna {
                internas.push((m.id, m.ruta.clone()));
                continue;
            }
            // Si Windows rehízo la pantalla (otro HMONITOR), los manejadores viejos ya no sirven.
            let viejo = matches!(
                self.controles.get(&m.id).map(|c| &c.via),
                Some(Via::Ddc { hmonitor, .. }) if *hmonitor != m.hmonitor.0 as isize
            );
            if viejo {
                self.controles.remove(&m.id);
            }
            if self.controles.contains_key(&m.id) {
                continue;
            }
            // La primera vez se insiste un poco más; después, una sola prueba cada 30 s.
            let intentos = match self.fallidos.get(&m.id) {
                Some(t) if t.elapsed() < REINTENTO_DDC => continue,
                Some(_) => 1,
                None => 2,
            };
            match abrir_ddc(m.hmonitor, intentos) {
                Some(mut c) => {
                    c.minimo = minimo_en(&self.minimos, m.id);
                    // Si quedó por debajo de su mínimo (ahí parpadea), sube al mínimo.
                    let piso = vcp::piso(c.minimo, c.maximo);
                    if c.actual < piso && c.manejador().is_some_and(|h| escribir_vcp(h, BRILLO_VCP, piso)) {
                        c.actual = piso;
                    }
                    self.controles.insert(m.id, c);
                    self.fallidos.remove(&m.id);
                }
                None => {
                    self.fallidos.insert(m.id, Instant::now());
                }
            }
        }
        if !internas.is_empty() {
            self.refrescar_internas(&internas);
        }
        lista
    }

    fn refrescar_internas(&mut self, internas: &[(u32, String)]) {
        // Si WMI falla, se queda el último valor conocido.
        let Some(lecturas) = self.wmi.brillos() else { return };
        for (id, ruta) in internas {
            let clave = clave_ruta(ruta);
            let lectura = lecturas
                .iter()
                .find(|l| !clave.is_empty() && clave_instancia(&l.instancia) == clave)
                .or_else(|| (internas.len() == 1).then(|| lecturas.first()).flatten());
            let Some(l) = lectura else { continue };
            if let Some(c) = self.controles.get_mut(id) {
                c.actual = l.brillo as u32;
            } else if let Some(r) = self.wmi.ruta_metodos(&l.instancia) {
                let c = Control { via: Via::Wmi { ruta: r }, maximo: 100, actual: l.brillo as u32, minimo: 0 };
                self.controles.insert(*id, c);
            }
        }
    }

    fn listar_de(&self, monitores: &[Monitor]) -> Vec<Pantalla> {
        let mut orden: Vec<(&Monitor, Pantalla)> = monitores
            .iter()
            .map(|m| {
                let c = self.controles.get(&m.id);
                let nombre = if m.interna {
                    "Pantalla del portátil".to_string()
                } else {
                    m.nombre.clone().unwrap_or_else(|| "Monitor externo".into())
                };
                let p = Pantalla {
                    id: m.id,
                    nombre,
                    interna: m.interna,
                    brillo: c.map(Control::porcentaje),
                    controlable: c.is_some(),
                    principal: m.principal,
                };
                (m, p)
            })
            .collect();
        // La integrada primero (como en Mac) y luego de izquierda a derecha.
        orden.sort_by_key(|(m, _)| (!m.interna, m.rect.left, m.rect.top));
        orden.into_iter().map(|(_, p)| p).collect()
    }

    /// Pone el brillo en unidades del monitor (0..=maximo), nunca por debajo de su mínimo. Devuelve el nivel de Nexo.
    fn escribir(&mut self, id: u32, crudo: u32) -> Result<u8, String> {
        let Some(c) = self.controles.get_mut(&id) else {
            let interna = buscar(id).is_some_and(|m| m.interna);
            return Err(if interna { "No se puede cambiar el brillo de esta pantalla".into() } else { NO_RESPONDE.into() });
        };
        let crudo = crudo.clamp(vcp::piso(c.minimo, c.maximo).min(c.maximo), c.maximo);
        let escrito = match &c.via {
            Via::Wmi { ruta } => {
                let ruta = ruta.clone();
                // Lo cambia Nexo: el vigía de las teclas del portátil no lo pasa al monitor.
                marcar_escrito_por_nexo();
                let r = self.wmi.poner(&ruta, crudo.min(100) as u8);
                marcar_escrito_por_nexo();
                r
            }
            Via::Ddc { .. } => match c.manejador() {
                Some(h) if escribir_vcp(h, BRILLO_VCP, crudo) => Ok(()),
                _ => Err(String::new()),
            },
        };
        match escrito {
            Ok(()) => {
                c.actual = crudo;
                return Ok(c.porcentaje());
            }
            Err(e) if matches!(c.via, Via::Wmi { .. }) => return Err(e),
            Err(_) => {}
        }

        // DDC falló: los manejadores pueden haber quedado viejos (cambió la configuración). Se piden de nuevo.
        let minimo = c.minimo;
        let anterior = vcp::a_porcentaje(crudo, minimo, c.maximo);
        self.controles.remove(&id);
        let reabierto = buscar(id).and_then(|m| abrir_ddc(m.hmonitor, 1));
        let Some(mut c) = reabierto else {
            self.fallidos.insert(id, Instant::now());
            return Err(NO_ACEPTO.into());
        };
        c.minimo = minimo;
        let crudo = vcp::a_crudo(anterior, minimo, c.maximo);
        let listo = c.manejador().is_some_and(|h| escribir_vcp(h, BRILLO_VCP, crudo));
        if listo {
            c.actual = crudo;
        }
        let final_ = c.porcentaje();
        self.controles.insert(id, c);
        if listo { Ok(final_) } else { Err(NO_ACEPTO.into()) }
    }

    /// El manejador DDC de un monitor externo que responde.
    fn manejador_ddc(&self, id: u32) -> Result<HANDLE, String> {
        match self.controles.get(&id) {
            Some(c) if matches!(c.via, Via::Ddc { .. }) => c.manejador().ok_or_else(|| NO_RESPONDE.to_string()),
            Some(_) => Err("La pantalla del portátil no tiene estos ajustes.".into()),
            None => Err(NO_RESPONDE.into()),
        }
    }
}

impl Sistema for Estado {
    fn refrescar(&mut self) {
        self.buscar();
    }

    fn listar(&mut self) -> Vec<Pantalla> {
        let vistas = self.buscar();
        self.listar_de(&vistas)
    }

    fn poner(&mut self, id: u32, valor: u8) -> Result<u8, String> {
        let (maximo, minimo) = self.controles.get(&id).map(|c| (c.maximo, c.minimo)).unwrap_or((100, 0));
        self.escribir(id, vcp::a_crudo(valor, minimo, maximo))
    }

    /// Sube o baja en pasos del tamaño elegido (el normal es 1/16, como en Mac). Devuelve el nivel final.
    fn paso(&mut self, id: u32, pasos: i32, tamano: u8) -> Option<u8> {
        let c = self.controles.get(&id)?;
        let crudo = vcp::a_crudo(vcp::paso(c.porcentaje(), pasos, tamano), c.minimo, c.maximo);
        self.escribir(id, crudo).ok()
    }

    fn poner_minimo(&mut self, id: u32, minimo: u8) -> Result<u8, String> {
        let minimo = minimo.min(vcp::MINIMO_TOPE);
        self.minimos.insert(id, minimo);
        self.manejador_ddc(id)?;
        let c = self.controles.get_mut(&id).ok_or(NO_RESPONDE)?;
        let nivel = c.porcentaje();
        c.minimo = minimo;
        let crudo = vcp::a_crudo(nivel, minimo, c.maximo);
        self.escribir(id, crudo)
    }

    fn ajustes(&mut self, id: u32) -> Result<vcp::AjustesMonitor, String> {
        let h = self.manejador_ddc(id)?;
        let minimo = minimo_en(&self.minimos, id);
        let memoria = self.memorias.entry(id).or_default();
        Ok(vcp::leer_ajustes(memoria, minimo, || capacidades_ddc(h), |codigo| leer_vcp(h, codigo, 2)))
    }

    fn poner_vcp(&mut self, id: u32, codigo: u8, valor: u16) -> Result<vcp::Valor, String> {
        let h = self.manejador_ddc(id)?;
        // La primera vez se lee qué tiene el monitor (para no mandar algo que no ofrece).
        if self.memorias.get(&id).is_none_or(|m| m.lista.is_none()) {
            self.ajustes(id)?;
        }
        let memoria = self.memorias.get(&id).ok_or(NO_RESPONDE)?;
        let crudo = vcp::validar(memoria, codigo, valor)?;
        let maximo = memoria.maximos.get(&codigo).copied().unwrap_or(0);
        if !escribir_vcp(h, codigo, crudo) {
            return Err(NO_ACEPTO.into());
        }
        if codigo == vcp::FABRICA {
            // El monitor tarda un momento en volver a sus valores; después se relee el brillo.
            std::thread::sleep(Duration::from_millis(500));
            if let (Some((actual, max)), Some(c)) = (leer_vcp(h, BRILLO_VCP, 2), self.controles.get_mut(&id)) {
                c.maximo = max;
                c.actual = actual;
            }
        }
        Ok(vcp::Valor { actual: crudo.min(u16::MAX as u32) as u16, maximo: maximo.min(u16::MAX as u32) as u16 })
    }

    fn niveles(&self) -> HashMap<u32, u8> {
        // También la pantalla del portátil: `brillos()` la lee al instante sin tocar DDC ni WMI.
        self.controles.iter().map(|(id, c)| (*id, c.porcentaje())).collect()
    }
}

pub(super) fn trabajador(
    rx: mpsc::Receiver<Orden>,
    controlables: Arc<Mutex<HashSet<u32>>>,
    brillos: Arc<Mutex<HashMap<u32, u8>>>,
    avisos: Avisos,
    minimos: HashMap<u32, u8>,
) {
    // COM para WMI en este hilo. S_FALSE (ya estaba) y RPC_E_CHANGED_MODE (otro modo) también sirven.
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let com_listo = com.is_ok() || com == RPC_E_CHANGED_MODE;

    let estado = Estado {
        controles: HashMap::new(),
        fallidos: HashMap::new(),
        wmi: Wmi { com: com_listo, servicios: None, fallo: None },
        memorias: HashMap::new(),
        minimos,
    };
    // `atender` es dueño del estado: al terminar lo suelta (destruye los manejadores) antes de cerrar COM.
    super::atender(rx, estado, controlables, brillos, avisos);
    if com.is_ok() {
        unsafe { CoUninitialize() };
    }
}

// ---------------------------------------------------------------- Teclas de brillo del portátil
//
// Las teclas de brillo del portátil (F6/F7, Fn+…) no llegan como teclas: el firmware cambia el brillo y Windows
// muestra su propio aviso. Por eso se escucha el cambio (WmiMonitorBrightnessEvent) y no la tecla.
// Si el mouse está sobre un monitor externo, cada cambio es un paso en ese monitor (1/16 por defecto, como en la Mac),
// con el aviso de Nexo allá, y el portátil vuelve a su nivel. El aviso de Windows sale igual: no se puede quitar.
// Límite: con el portátil en 0 % o 100 %, la tecla que sigue en esa dirección no cambia nada y WMI no avisa;
// entonces Nexo no se entera. No se mueve el portátil para evitarlo.

/// Si se escuchan las teclas del portátil (el ajuste "teclas de brillo"). La cambia `vigilar_portatil`.
static VIGILAR: AtomicBool = AtomicBool::new(false);
/// Si el hilo que escucha está vivo (hay uno solo).
static VIGIA_VIVO: AtomicBool = AtomicBool::new(false);
/// Cuándo puso Nexo el brillo del portátil desde el panel, las escenas o los widgets.
static ESCRITO_POR_NEXO: Mutex<Option<Instant>> = Mutex::new(None);

/// Un cambio que llega hasta 1 s después de que Nexo puso el brillo del portátil es de Nexo.
const ECO_NEXO: Duration = Duration::from_secs(1);
/// El aviso de que el portátil volvió a su nivel llega antes de esto.
const ECO_REGRESO: Duration = Duration::from_millis(1500);
/// Una tecla mueve el portátil de 3 a 25 puntos (Windows usa 10; algunos, 5). Menos es el brillo automático;
/// más, el plan de energía al enchufar o desenchufar. Esos no pasan al monitor.
const TOQUE: std::ops::RangeInclusive<i32> = 3..=25;
/// Si la escucha se corta, se vuelve a pedir unas pocas veces (con pausa), no sin parar.
const REINTENTOS_VIGIA: u32 = 3;

fn marcar_escrito_por_nexo() {
    if let Ok(mut t) = ESCRITO_POR_NEXO.lock() {
        *t = Some(Instant::now());
    }
}

fn escrito_por_nexo_hace_poco() -> bool {
    ESCRITO_POR_NEXO.lock().ok().and_then(|t| *t).is_some_and(|t| t.elapsed() < ECO_NEXO)
}

/// Enciende o apaga el paso de las teclas de brillo del portátil al monitor bajo el mouse (lo llama `teclas::activar`).
/// Al encender crea el hilo que escucha a WMI; al apagar, ese hilo termina solo en menos de 1 s.
pub fn vigilar_portatil(pantallas: Arc<Pantallas>, encendido: bool) {
    VIGILAR.store(encendido, Ordering::SeqCst);
    if !encendido || VIGIA_VIVO.swap(true, Ordering::SeqCst) {
        return;
    }
    let hilo = std::thread::Builder::new()
        .name("nexo-brillo-portatil".into())
        .stack_size(256 * 1024)
        .spawn(move || vigia_portatil(pantallas));
    if let Err(e) = hilo {
        VIGIA_VIVO.store(false, Ordering::SeqCst);
        eprintln!("Nexo: no se pudo escuchar el brillo del portátil ({e})");
    }
}

/// Cómo terminó una escucha.
enum Fin {
    /// Se apagó el ajuste.
    Apagado,
    /// No se pudo empezar: no se insiste.
    Fallo(String),
    /// Andaba y se cortó (quizá se reinició WMI): se puede volver a pedir.
    Cortado(String),
}

fn vigia_portatil(pantallas: Arc<Pantallas>) {
    // COM propio (MTA). S_FALSE (ya estaba) y RPC_E_CHANGED_MODE (otro modo) también sirven.
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let mut seguir = com.is_ok() || com == RPC_E_CHANGED_MODE;
    if !seguir {
        VIGIA_VIVO.store(false, Ordering::SeqCst);
        eprintln!("Nexo: no se pudo escuchar el brillo del portátil (COM {:#010x})", com.0);
    }
    let mut cortes = 0;
    while seguir {
        // Los objetos de WMI se sueltan dentro de `escuchar_portatil`, antes de cerrar COM.
        let apagado = match escuchar_portatil(&pantallas) {
            Fin::Apagado => true,
            Fin::Fallo(e) => {
                eprintln!("Nexo: no se puede escuchar el brillo del portátil ({e}); sus teclas no pasan al monitor");
                false
            }
            Fin::Cortado(e) if cortes < REINTENTOS_VIGIA => {
                cortes += 1;
                eprintln!("Nexo: se cortó la escucha del brillo del portátil ({e}); se vuelve a pedir en 10 s");
                // Pausa en pasos de 1 s: si apagan el ajuste, termina enseguida.
                for _ in 0..10 {
                    if !VIGILAR.load(Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
                if VIGILAR.load(Ordering::SeqCst) {
                    continue;
                }
                true
            }
            Fin::Cortado(e) => {
                eprintln!("Nexo: se cortó la escucha del brillo del portátil ({e}); no se insiste más");
                false
            }
        };
        VIGIA_VIVO.store(false, Ordering::SeqCst);
        // Si lo encendieron justo mientras se apagaba, este mismo hilo sigue.
        seguir = apagado && VIGILAR.load(Ordering::SeqCst) && !VIGIA_VIVO.swap(true, Ordering::SeqCst);
    }
    if com.is_ok() {
        unsafe { CoUninitialize() };
    }
}

/// Lo que el vigía sabe del brillo del portátil.
struct Portatil {
    servicios: IWbemServices,
    /// Último nivel que dio WMI.
    visto: Option<u8>,
    /// Nivel al que Nexo devolvió el portátil y cuándo (su aviso aún no llega).
    regreso: Option<(u8, Instant)>,
    /// Ruta del objeto con WmiSetBrightness (se busca la primera vez).
    ruta: Option<String>,
}

impl Portatil {
    /// Un aviso de WMI: el portátil quedó en `nuevo`.
    fn al_cambiar(&mut self, nuevo: u8, instancia: &str, pantallas: &Pantallas) {
        let antes = self.visto.replace(nuevo);
        // El portátil volvió al nivel que pidió Nexo: es su propio cambio.
        if let Some((nivel, cuando)) = self.regreso {
            if cuando.elapsed() > ECO_REGRESO {
                self.regreso = None;
            } else if nivel == nuevo {
                self.regreso = None;
                return;
            }
        }
        if escrito_por_nexo_hace_poco() || !VIGILAR.load(Ordering::Relaxed) {
            return;
        }
        let Some(antes) = antes else { return };
        let cambio = nuevo as i32 - antes as i32;
        if !TOQUE.contains(&cambio.abs()) {
            return;
        }
        let Some(id) = externa_bajo_el_cursor() else { return };
        // Ese monitor no responde a las teclas (su ajuste): el portátil queda como lo dejó la tecla.
        if !pantallas.teclas_de(id).responde {
            return;
        }
        if !pantallas.es_controlable(id) {
            // Quizá se acaba de conectar o se activó DDC/CI: la próxima vez ya estará. Ahora el portátil queda así.
            pantallas.refrescar();
            return;
        }
        // Un paso por aviso (del tamaño elegido para ese monitor); si llegan varios seguidos, el hilo de pantallas
        // los junta y muestra el aviso.
        pantallas.paso(id, cambio.signum());
        // El portátil vuelve al nivel de antes del toque (o al que ya se estaba devolviendo).
        let base = self.regreso.map(|(n, _)| n).unwrap_or(antes);
        self.devolver(base, instancia);
    }

    /// WmiSetBrightness al nivel de antes, marcado como de Nexo.
    fn devolver(&mut self, nivel: u8, instancia: &str) {
        if self.visto == Some(nivel) {
            return;
        }
        if self.ruta.is_none() {
            self.ruta = ruta_metodos(&self.servicios, instancia);
        }
        let Some(ruta) = self.ruta.as_deref() else { return };
        self.regreso = Some((nivel, Instant::now()));
        match poner_wmi(&self.servicios, ruta, nivel) {
            // Ya está ahí aunque su aviso llegue tarde (o no llegue): el próximo toque se mide desde aquí.
            Ok(()) => self.visto = Some(nivel),
            Err(e) => {
                self.regreso = None;
                self.ruta = None;
                eprintln!("Nexo: no se pudo devolver el brillo del portátil ({:#010x})", e.code().0);
            }
        }
    }
}

/// Se suscribe a WmiMonitorBrightnessEvent y atiende cada aviso hasta que se apague el ajuste.
fn escuchar_portatil(pantallas: &Pantallas) -> Fin {
    let servicios = match abrir_wmi() {
        Ok(s) => s,
        Err(e) => return Fin::Fallo(format!("WMI {:#010x}", e.code().0)),
    };
    let banderas = WBEM_FLAG_RETURN_IMMEDIATELY | WBEM_FLAG_FORWARD_ONLY;
    let consulta = BSTR::from("SELECT * FROM WmiMonitorBrightnessEvent");
    let eventos = match unsafe { servicios.ExecNotificationQuery(&BSTR::from("WQL"), &consulta, banderas, None) } {
        Ok(e) => e,
        Err(e) => return Fin::Fallo(format!("suscripción {:#010x}", e.code().0)),
    };
    // Igual que en la conexión: sin esto WMI puede negar el acceso al pedir los avisos.
    let _ = unsafe {
        CoSetProxyBlanket(
            &eventos,
            AUTENTICACION_WINNT,
            AUTORIZACION_NINGUNA,
            PCWSTR::null(),
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )
    };
    // Nivel de partida (en una PC de escritorio no hay: se toma del primer aviso).
    let visto = consultar(&servicios, "SELECT CurrentBrightness FROM WmiMonitorBrightness WHERE Active = TRUE")
        .first()
        .and_then(|o| leer_numero(o, w!("CurrentBrightness")))
        .map(|b| b.clamp(0, 100) as u8);
    let mut portatil = Portatil { servicios, visto, regreso: None, ruta: None };

    while VIGILAR.load(Ordering::SeqCst) {
        let mut objetos: [Option<IWbemClassObject>; 1] = [None];
        let mut n = 0u32;
        // Espera hasta 1 s (WBEM_S_TIMEDOUT si no hubo nada): así ve pronto si se apagó el ajuste.
        let hr = unsafe { eventos.Next(1000, &mut objetos, &mut n) };
        if hr.is_err() {
            return Fin::Cortado(format!("{:#010x}", hr.0));
        }
        let Some(o) = objetos[0].take().filter(|_| n > 0) else { continue };
        let Some(nuevo) = leer_numero(&o, w!("Brightness")) else { continue };
        let instancia = leer_texto(&o, w!("InstanceName")).unwrap_or_default();
        portatil.al_cambiar(nuevo.clamp(0, 100) as u8, &instancia, pantallas);
    }
    Fin::Apagado
}

// ---------------------------------------------------------------- Resolución y frecuencia

fn devmode() -> DEVMODEW {
    DEVMODEW { dmSize: size_of::<DEVMODEW>() as u16, ..Default::default() }
}

fn modo_actual(dispositivo: &[u16; 32]) -> Option<DEVMODEW> {
    let mut dm = devmode();
    let ok = unsafe {
        EnumDisplaySettingsExW(PCWSTR(dispositivo.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm, ENUM_DISPLAY_SETTINGS_FLAGS(0))
    };
    ok.as_bool().then_some(dm)
}

/// Todos los modos que da el controlador, con su número (iModeNum). Empezar en 0 hace que Windows
/// vuelva a leer la lista, así el número de un modo es el mismo en `modos` y en `cambiar_modo`.
fn modos_crudos(dispositivo: &[u16; 32]) -> Vec<(u32, DEVMODEW)> {
    let mut lista = Vec::new();
    for i in 0..4096u32 {
        let mut dm = devmode();
        let ok = unsafe {
            EnumDisplaySettingsExW(
                PCWSTR(dispositivo.as_ptr()),
                ENUM_DISPLAY_SETTINGS_MODE(i),
                &mut dm,
                ENUM_DISPLAY_SETTINGS_FLAGS(0),
            )
        };
        if !ok.as_bool() {
            break;
        }
        lista.push((i, dm));
    }
    lista
}

fn entrelazado(dm: &DEVMODEW) -> bool {
    unsafe { dm.Anonymous2.dmDisplayFlags & DM_INTERLACED.0 != 0 }
}

/// Windows dice 0 o 1 cuando usa "la frecuencia por defecto del aparato": se muestra como 60.
fn hz_visible(frecuencia: u32) -> f64 {
    if frecuencia > 1 { frecuencia as f64 } else { 60.0 }
}

/// Modos que se pueden elegir, sin repetidos: uno por (ancho, alto, Hz).
/// `ModoPantalla.id` es el número del modo en la lista del controlador (iModeNum de EnumDisplaySettingsExW);
/// `cambiar_modo` lo vuelve a buscar con ese número. En Windows los píxeles son la resolución y no hay Retina.
/// Ordenados de mayor a menor ancho y, dentro de cada uno, de mayor a menor frecuencia.
pub fn modos(id: u32) -> Vec<ModoPantalla> {
    let Some(m) = buscar(id) else { return Vec::new() };
    let actual = modo_actual(&m.dispositivo);
    let firma_actual = actual.map(|a| (a.dmPelsWidth, a.dmPelsHeight, a.dmDisplayFrequency));
    let bits = actual.map(|a| a.dmBitsPerPel).filter(|b| *b > 0).unwrap_or(32);
    let crudos = modos_crudos(&m.dispositivo);

    let juntar = |solo_bits: bool| {
        let mut lista: Vec<((u32, u32, u32), ModoPantalla)> = Vec::new();
        for (i, dm) in &crudos {
            let (ancho, alto, f) = (dm.dmPelsWidth, dm.dmPelsHeight, dm.dmDisplayFrequency);
            // Los colores de 8 y 16 bits, los entrelazados y los menores de 800×600 no sirven para el escritorio.
            if (solo_bits && dm.dmBitsPerPel != bits) || entrelazado(dm) || ancho.max(alto) < 800 || ancho.min(alto) < 600 {
                continue;
            }
            let clave = (ancho, alto, f);
            let es_actual = Some(clave) == firma_actual;
            let modo = ModoPantalla {
                id: *i as i32,
                ancho,
                alto,
                pixeles_ancho: ancho,
                pixeles_alto: alto,
                hz: hz_visible(f),
                retina: false,
                actual: es_actual,
            };
            match lista.iter_mut().find(|(k, _)| *k == clave) {
                Some((_, x)) if es_actual => *x = modo,
                Some(_) => {}
                None => lista.push((clave, modo)),
            }
        }
        lista
    };
    let mut lista = juntar(true);
    if lista.is_empty() {
        lista = juntar(false);
    }
    let mut lista: Vec<ModoPantalla> = lista.into_iter().map(|(_, m)| m).collect();
    lista.sort_by(|a, b| b.ancho.cmp(&a.ancho).then(b.hz.total_cmp(&a.hz)).then(b.alto.cmp(&a.alto)));
    lista
}

fn error_cambio(r: DISP_CHANGE) -> String {
    match r {
        DISP_CHANGE_RESTART => "Windows pide reiniciar la computadora para aplicar este cambio.".into(),
        DISP_CHANGE_BADMODE => "La pantalla no admite ese modo.".into(),
        DISP_CHANGE_FAILED => "El controlador de video no pudo aplicar el cambio.".into(),
        DISP_CHANGE_NOTUPDATED => "Windows no pudo guardar el ajuste de pantalla.".into(),
        DISP_CHANGE_BADDUALVIEW => "Esta pantalla está en modo DualView y no admite el cambio.".into(),
        otro => format!("Windows no aceptó el cambio de pantalla (código {}).", otro.0),
    }
}

/// Cambia la resolución y la frecuencia de una pantalla al modo `modo` (su número en la lista del controlador).
/// Primero lo prueba (CDS_TEST) y, si Windows lo acepta, lo aplica y lo guarda (CDS_UPDATEREGISTRY).
pub fn cambiar_modo(id: u32, modo: i32) -> Result<(), String> {
    let m = buscar(id).ok_or("No encontré esa pantalla. ¿Se desconectó?")?;
    let numero = u32::try_from(modo).map_err(|_| "Ese modo ya no está disponible en esta pantalla")?;
    let (_, elegido) = modos_crudos(&m.dispositivo)
        .into_iter()
        .find(|(i, _)| *i == numero)
        .ok_or("Ese modo ya no está disponible en esta pantalla")?;
    let (ancho, alto, f) = (elegido.dmPelsWidth, elegido.dmPelsHeight, elegido.dmDisplayFrequency);
    if modo_actual(&m.dispositivo).is_some_and(|a| (a.dmPelsWidth, a.dmPelsHeight, a.dmDisplayFrequency) == (ancho, alto, f)) {
        return Ok(()); // ya está en ese modo
    }

    let mut dm = devmode();
    dm.dmPelsWidth = ancho;
    dm.dmPelsHeight = alto;
    dm.dmBitsPerPel = elegido.dmBitsPerPel;
    dm.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT | DM_BITSPERPEL;
    if f > 1 {
        dm.dmDisplayFrequency = f;
        dm.dmFields |= DM_DISPLAYFREQUENCY;
    }
    let nombre = PCWSTR(m.dispositivo.as_ptr());
    let prueba = unsafe { ChangeDisplaySettingsExW(nombre, Some(&dm as *const DEVMODEW), None, CDS_TEST, None) };
    if prueba != DISP_CHANGE_SUCCESSFUL {
        return Err(error_cambio(prueba));
    }
    let r = unsafe { ChangeDisplaySettingsExW(nombre, Some(&dm as *const DEVMODEW), None, CDS_UPDATEREGISTRY, None) };
    if r == DISP_CHANGE_SUCCESSFUL { Ok(()) } else { Err(error_cambio(r)) }
}

// ---------------------------------------------------------------- Pantalla principal

fn posicion(dm: &DEVMODEW) -> POINTL {
    unsafe { dm.Anonymous1.Anonymous2.dmPosition }
}

fn con_posicion(dm: &DEVMODEW, x: i32, y: i32) -> DEVMODEW {
    let mut nuevo = *dm;
    let mut pantalla = unsafe { nuevo.Anonymous1.Anonymous2 };
    pantalla.dmPosition = POINTL { x, y };
    nuevo.Anonymous1.Anonymous2 = pantalla;
    nuevo.dmFields |= DM_POSITION;
    nuevo
}

/// Guarda (sin aplicar) la posición de cada pantalla; la que va con `principal` queda como principal.
fn guardar_posiciones(lista: &[(&Monitor, DEVMODEW)], principal: u32) -> Result<(), DISP_CHANGE> {
    // La nueva principal va primero.
    let mut orden: Vec<&(&Monitor, DEVMODEW)> = lista.iter().collect();
    orden.sort_by_key(|(m, _)| m.id != principal);
    for (m, dm) in orden {
        let mut banderas = CDS_UPDATEREGISTRY | CDS_NORESET;
        if m.id == principal {
            banderas |= CDS_SET_PRIMARY;
        }
        let r = unsafe {
            ChangeDisplaySettingsExW(PCWSTR(m.dispositivo.as_ptr()), Some(dm as *const DEVMODEW), None, banderas, None)
        };
        if r != DISP_CHANGE_SUCCESSFUL {
            return Err(r);
        }
    }
    Ok(())
}

/// Hace principal a una pantalla (la de la barra de tareas con la bandeja). En Windows la principal es la que
/// está en el origen (0, 0): se mueven todas lo mismo, así la distribución no cambia.
pub fn hacer_principal(id: u32) -> Result<(), String> {
    let lista = monitores();
    let nueva = lista.iter().find(|m| m.id == id).ok_or("No encontré esa pantalla. ¿Se desconectó?")?;
    if nueva.principal {
        return Ok(());
    }
    let anterior = lista.iter().find(|m| m.principal).map(|m| m.id);

    let mut actuales: Vec<(&Monitor, DEVMODEW)> = Vec::new();
    for m in &lista {
        let dm = modo_actual(&m.dispositivo).ok_or("Windows no dejó leer la posición de las pantallas.")?;
        actuales.push((m, dm));
    }
    let origen = actuales.iter().find(|(m, _)| m.id == id).map(|(_, dm)| posicion(dm)).unwrap_or_default();
    let movidas: Vec<(&Monitor, DEVMODEW)> = actuales
        .iter()
        .map(|(m, dm)| {
            let p = posicion(dm);
            (*m, con_posicion(dm, p.x - origen.x, p.y - origen.y))
        })
        .collect();

    if let Err(r) = guardar_posiciones(&movidas, id) {
        // Se devuelve lo guardado a como estaba (sin aplicar nada).
        if let Some(a) = anterior {
            let originales: Vec<(&Monitor, DEVMODEW)> =
                actuales.iter().map(|(m, dm)| (*m, con_posicion(dm, posicion(dm).x, posicion(dm).y))).collect();
            let _ = guardar_posiciones(&originales, a);
        }
        return Err(error_cambio(r));
    }
    // Aplica todo junto.
    let r = unsafe { ChangeDisplaySettingsExW(PCWSTR::null(), None, None, CDS_TYPE(0), None) };
    if r == DISP_CHANGE_SUCCESSFUL { Ok(()) } else { Err(error_cambio(r)) }
}
