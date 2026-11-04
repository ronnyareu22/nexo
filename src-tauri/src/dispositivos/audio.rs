//! Sonido: volumen, silencio, salida y micrófono.
//!
//! - Por dirección Bluetooth (detalle de un aparato): macOS nombra cada aparato de sonido
//!   Bluetooth con su dirección (por ejemplo `E8-9E-13-61-CE-B7:output`), así se une con la lista de Bluetooth.
//! - Por UID (página Audio): todos los aparatos de sonido del sistema, no solo Bluetooth.
//!
//! Mac: CoreAudio. Windows: WASAPI e IPolicyConfig (en `audio_windows.rs`).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Sonido {
    pub tiene_salida: bool,
    pub salida_en_uso: bool,
    pub volumen: Option<u8>,
    pub silenciado: Option<bool>,
    pub tiene_microfono: bool,
    pub microfono_en_uso: bool,
    pub volumen_microfono: Option<u8>,
}

/// Un aparato de sonido del sistema (integrado, Bluetooth, USB, HDMI…), para la página Audio.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
pub struct DispositivoAudio {
    /// Identificador estable del sistema (por ejemplo `BuiltInSpeakerDevice`).
    pub uid: String,
    /// Nombre para mostrar, tal como lo da el sistema ("Bocinas de MacBook Pro").
    pub nombre: String,
    /// Se puede elegir como salida o como entrada (igual que en Ajustes del Sistema).
    pub salida: bool,
    pub entrada: bool,
    pub salida_en_uso: bool,
    pub entrada_en_uso: bool,
    pub volumen: Option<u8>,
    pub silenciado: Option<bool>,
    pub volumen_entrada: Option<u8>,
    /// "integrado", "bluetooth", "usb", "hdmi", "displayport", "airplay", "virtual" u "otro".
    pub transporte: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "accion", content = "valor", rename_all = "camelCase")]
pub enum Cambio {
    UsarSalida,
    Volumen(u8),
    Silenciar(bool),
    UsarMicrofono,
    VolumenMicrofono(u8),
}

#[cfg(target_os = "macos")]
pub use mac::{ajustar, ajustar_por_uid, estado, listar};

#[cfg(windows)]
#[path = "audio_windows.rs"]
mod sistema;
#[cfg(windows)]
pub use sistema::{ajustar, ajustar_por_uid, estado, listar};

// En otros sistemas no hay sonido que controlar.
#[cfg(not(any(target_os = "macos", windows)))]
pub fn estado(_direccion: &str) -> Result<Sonido, String> {
    Ok(Sonido::default())
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn ajustar(_direccion: &str, _cambio: Cambio) -> Result<(), String> {
    Err("No disponible en este sistema".into())
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn listar() -> Vec<DispositivoAudio> {
    Vec::new()
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn ajustar_por_uid(_uid: &str, _cambio: Cambio) -> Result<(), String> {
    Err("No disponible en este sistema".into())
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{Cambio, DispositivoAudio, Sonido};
    use std::ffi::{c_char, c_void};

    type Objeto = u32;

    #[repr(C)]
    struct Direccion {
        selector: u32,
        alcance: u32,
        elemento: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    unsafe extern "C" {
        fn AudioObjectGetPropertyDataSize(id: Objeto, dir: *const Direccion, q: u32, qd: *const c_void, tam: *mut u32) -> i32;
        fn AudioObjectGetPropertyData(id: Objeto, dir: *const Direccion, q: u32, qd: *const c_void, tam: *mut u32, datos: *mut c_void) -> i32;
        fn AudioObjectSetPropertyData(id: Objeto, dir: *const Direccion, q: u32, qd: *const c_void, tam: u32, datos: *const c_void) -> i32;
        fn AudioObjectHasProperty(id: Objeto, dir: *const Direccion) -> u8;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringGetCString(texto: *const c_void, buf: *mut c_char, tam: isize, codificacion: u32) -> u8;
        fn CFRelease(objeto: *const c_void);
    }

    const fn cc(s: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*s)
    }
    const SISTEMA: Objeto = 1;
    const DISPOSITIVOS: u32 = cc(b"dev#");
    const SALIDA_PREDETERMINADA: u32 = cc(b"dOut");
    const ENTRADA_PREDETERMINADA: u32 = cc(b"dIn ");
    const SALIDA_AVISOS: u32 = cc(b"sOut");
    const UID: u32 = cc(b"uid ");
    const NOMBRE: u32 = cc(b"lnam");
    const TRANSPORTE: u32 = cc(b"tran");
    const OCULTO: u32 = cc(b"hidn");
    const VIVO: u32 = cc(b"livn");
    const PUEDE_SER_PREDETERMINADO: u32 = cc(b"dflt");
    const FLUJOS: u32 = cc(b"stm#");
    const VOLUMEN_VIRTUAL: u32 = cc(b"vmvc");
    const VOLUMEN: u32 = cc(b"volm");
    const SILENCIO: u32 = cc(b"mute");
    const GLOBAL: u32 = cc(b"glob");
    const SALIDA: u32 = cc(b"outp");
    const ENTRADA: u32 = cc(b"inpt");
    const UTF8: u32 = 0x0800_0100;

    fn dir(selector: u32, alcance: u32, elemento: u32) -> Direccion {
        Direccion { selector, alcance, elemento }
    }

    fn tiene(id: Objeto, d: &Direccion) -> bool {
        unsafe { AudioObjectHasProperty(id, d) != 0 }
    }

    fn leer<T: Copy + Default>(id: Objeto, d: &Direccion) -> Option<T> {
        let mut valor = T::default();
        let mut tam = std::mem::size_of::<T>() as u32;
        let r = unsafe { AudioObjectGetPropertyData(id, d, 0, std::ptr::null(), &mut tam, &mut valor as *mut T as *mut c_void) };
        (r == 0).then_some(valor)
    }

    fn escribir<T: Copy>(id: Objeto, d: &Direccion, valor: T) -> bool {
        let tam = std::mem::size_of::<T>() as u32;
        unsafe { AudioObjectSetPropertyData(id, d, 0, std::ptr::null(), tam, &valor as *const T as *const c_void) == 0 }
    }

    fn dispositivos() -> Vec<Objeto> {
        let d = dir(DISPOSITIVOS, GLOBAL, 0);
        let mut tam = 0u32;
        if unsafe { AudioObjectGetPropertyDataSize(SISTEMA, &d, 0, std::ptr::null(), &mut tam) } != 0 {
            return Vec::new();
        }
        let mut ids = vec![0 as Objeto; tam as usize / std::mem::size_of::<Objeto>()];
        let r = unsafe { AudioObjectGetPropertyData(SISTEMA, &d, 0, std::ptr::null(), &mut tam, ids.as_mut_ptr() as *mut c_void) };
        if r == 0 { ids } else { Vec::new() }
    }

    /// Una propiedad de texto (CFString) del aparato: su UID, su nombre…
    fn texto(id: Objeto, selector: u32) -> Option<String> {
        let texto: *const c_void = leer(id, &dir(selector, GLOBAL, 0)).filter(|p: &*const c_void| !p.is_null())?;
        let mut buf = [0 as c_char; 512];
        let ok = unsafe { CFStringGetCString(texto, buf.as_mut_ptr(), buf.len() as isize, UTF8) } != 0;
        unsafe { CFRelease(texto) };
        ok.then(|| unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned())
    }

    fn uid(id: Objeto) -> Option<String> {
        texto(id, UID)
    }

    fn tiene_flujos(id: Objeto, alcance: u32) -> bool {
        let mut tam = 0u32;
        let r = unsafe { AudioObjectGetPropertyDataSize(id, &dir(FLUJOS, alcance, 0), 0, std::ptr::null(), &mut tam) };
        r == 0 && tam > 0
    }

    /// Los aparatos de sonido de macOS que corresponden a una dirección Bluetooth: (salida, entrada).
    fn buscar(direccion: &str) -> (Option<Objeto>, Option<Objeto>) {
        let clave = direccion.replace(':', "-").to_uppercase();
        let mut salida = None;
        let mut entrada = None;
        for id in dispositivos() {
            let Some(u) = uid(id) else { continue };
            if !u.to_uppercase().contains(&clave) {
                continue;
            }
            if salida.is_none() && tiene_flujos(id, SALIDA) {
                salida = Some(id);
            }
            if entrada.is_none() && tiene_flujos(id, ENTRADA) {
                entrada = Some(id);
            }
        }
        (salida, entrada)
    }

    /// Volumen 0–1: primero el volumen general; si no existe, el promedio de los canales.
    fn volumen(id: Objeto, alcance: u32) -> Option<f32> {
        let virtual_ = dir(VOLUMEN_VIRTUAL, alcance, 0);
        if tiene(id, &virtual_) {
            if let Some(v) = leer::<f32>(id, &virtual_) {
                return Some(v);
            }
        }
        let principal = dir(VOLUMEN, alcance, 0);
        if tiene(id, &principal) {
            return leer::<f32>(id, &principal);
        }
        let canales: Vec<f32> = (1..=2).filter_map(|c| leer::<f32>(id, &dir(VOLUMEN, alcance, c))).collect();
        (!canales.is_empty()).then(|| canales.iter().sum::<f32>() / canales.len() as f32)
    }

    fn poner_volumen(id: Objeto, alcance: u32, valor: f32) -> bool {
        let valor = valor.clamp(0.0, 1.0);
        let virtual_ = dir(VOLUMEN_VIRTUAL, alcance, 0);
        if tiene(id, &virtual_) && escribir(id, &virtual_, valor) {
            return true;
        }
        let principal = dir(VOLUMEN, alcance, 0);
        if tiene(id, &principal) && escribir(id, &principal, valor) {
            return true;
        }
        (1..=2).map(|c| escribir(id, &dir(VOLUMEN, alcance, c), valor)).fold(false, |a, b| a | b)
    }

    fn porcentaje(v: f32) -> u8 {
        (v.clamp(0.0, 1.0) * 100.0).round() as u8
    }

    pub fn estado(direccion: &str) -> Result<Sonido, String> {
        let (salida, entrada) = buscar(direccion);
        let predeterminada_salida: Option<Objeto> = leer(SISTEMA, &dir(SALIDA_PREDETERMINADA, GLOBAL, 0));
        let predeterminada_entrada: Option<Objeto> = leer(SISTEMA, &dir(ENTRADA_PREDETERMINADA, GLOBAL, 0));
        Ok(Sonido {
            tiene_salida: salida.is_some(),
            salida_en_uso: salida.is_some() && salida == predeterminada_salida,
            volumen: salida.and_then(|id| volumen(id, SALIDA)).map(porcentaje),
            silenciado: salida.and_then(|id| leer::<u32>(id, &dir(SILENCIO, SALIDA, 0))).map(|m| m != 0),
            tiene_microfono: entrada.is_some(),
            microfono_en_uso: entrada.is_some() && entrada == predeterminada_entrada,
            volumen_microfono: entrada.and_then(|id| volumen(id, ENTRADA)).map(porcentaje),
        })
    }

    pub fn ajustar(direccion: &str, cambio: Cambio) -> Result<(), String> {
        let (salida, entrada) = buscar(direccion);
        aplicar(salida, entrada, cambio)
    }

    /// Aplica un cambio a la salida o a la entrada de un aparato ya encontrado.
    fn aplicar(salida: Option<Objeto>, entrada: Option<Objeto>, cambio: Cambio) -> Result<(), String> {
        let sin_salida = || "Este aparato no tiene salida de sonido activa".to_string();
        let sin_microfono = || "Este aparato no tiene micrófono activo".to_string();
        let ok = match cambio {
            Cambio::UsarSalida => {
                let id = salida.ok_or_else(sin_salida)?;
                escribir(SISTEMA, &dir(SALIDA_PREDETERMINADA, GLOBAL, 0), id)
            }
            Cambio::Volumen(v) => poner_volumen(salida.ok_or_else(sin_salida)?, SALIDA, v as f32 / 100.0),
            Cambio::Silenciar(s) => escribir(salida.ok_or_else(sin_salida)?, &dir(SILENCIO, SALIDA, 0), s as u32),
            Cambio::UsarMicrofono => {
                let id = entrada.ok_or_else(sin_microfono)?;
                escribir(SISTEMA, &dir(ENTRADA_PREDETERMINADA, GLOBAL, 0), id)
            }
            Cambio::VolumenMicrofono(v) => poner_volumen(entrada.ok_or_else(sin_microfono)?, ENTRADA, v as f32 / 100.0),
        };
        if ok { Ok(()) } else { Err("macOS no aceptó el cambio de sonido".into()) }
    }

    // ------------------------------------------------------------ Todos los aparatos (página Audio)

    /// Se puede elegir en ese sentido: tiene flujos y macOS lo deja ser predeterminado
    /// (el mismo filtro de Ajustes del Sistema). Si macOS no lo dice, se asume que sí.
    fn elegible(id: Objeto, alcance: u32) -> bool {
        tiene_flujos(id, alcance) && leer::<u32>(id, &dir(PUEDE_SER_PREDETERMINADO, alcance, 0)) != Some(0)
    }

    fn transporte(id: Objeto) -> &'static str {
        let tipo = leer::<u32>(id, &dir(TRANSPORTE, GLOBAL, 0)).unwrap_or(0);
        const NOMBRES: [(&[u8; 4], &str); 9] = [
            (b"bltn", "integrado"),
            (b"blue", "bluetooth"),
            (b"blea", "bluetooth"),
            (b"usb ", "usb"),
            (b"hdmi", "hdmi"),
            (b"dprt", "displayport"),
            (b"airp", "airplay"),
            (b"virt", "virtual"),
            (b"grup", "virtual"),
        ];
        NOMBRES.iter().find(|(c, _)| cc(c) == tipo).map_or("otro", |(_, n)| n)
    }

    /// El aparato con ese UID, si sigue conectado.
    fn por_uid(clave: &str) -> Option<Objeto> {
        dispositivos().into_iter().find(|&id| uid(id).as_deref() == Some(clave))
    }

    /// Todos los aparatos de sonido que tiene sentido mostrar: primero los que están en uso,
    /// luego los integrados y el resto por nombre.
    pub fn listar() -> Vec<DispositivoAudio> {
        let salida_actual: Option<Objeto> = leer(SISTEMA, &dir(SALIDA_PREDETERMINADA, GLOBAL, 0));
        let entrada_actual: Option<Objeto> = leer(SISTEMA, &dir(ENTRADA_PREDETERMINADA, GLOBAL, 0));
        let mut lista: Vec<DispositivoAudio> = dispositivos()
            .into_iter()
            .filter_map(|id| {
                let uid = uid(id)?;
                // Fuera lo que macOS esconde (como los agregados que arma para sí) y lo que ya no está.
                let oculto = leer::<u32>(id, &dir(OCULTO, GLOBAL, 0)).is_some_and(|h| h != 0);
                let desconectado = leer::<u32>(id, &dir(VIVO, GLOBAL, 0)) == Some(0);
                if oculto || desconectado || uid.starts_with("CADefaultDeviceAggregate") {
                    return None;
                }
                let salida = elegible(id, SALIDA);
                let entrada = elegible(id, ENTRADA);
                if !salida && !entrada {
                    return None;
                }
                Some(DispositivoAudio {
                    nombre: texto(id, NOMBRE).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| uid.clone()),
                    uid,
                    salida,
                    entrada,
                    salida_en_uso: salida && salida_actual == Some(id),
                    entrada_en_uso: entrada && entrada_actual == Some(id),
                    volumen: salida.then(|| volumen(id, SALIDA)).flatten().map(porcentaje),
                    silenciado: salida.then(|| leer::<u32>(id, &dir(SILENCIO, SALIDA, 0))).flatten().map(|m| m != 0),
                    volumen_entrada: entrada.then(|| volumen(id, ENTRADA)).flatten().map(porcentaje),
                    transporte: transporte(id).into(),
                })
            })
            .collect();
        lista.sort_by_cached_key(|d| (!(d.salida_en_uso || d.entrada_en_uso), d.transporte != "integrado", d.nombre.to_lowercase()));
        lista
    }

    /// Aplica un cambio al aparato con ese UID. Al elegirlo como salida, los sonidos de aviso
    /// del sistema también pasan a él.
    pub fn ajustar_por_uid(clave: &str, cambio: Cambio) -> Result<(), String> {
        let id = por_uid(clave).ok_or_else(|| "Ese aparato de sonido ya no está conectado".to_string())?;
        let usar_salida = matches!(cambio, Cambio::UsarSalida);
        aplicar(tiene_flujos(id, SALIDA).then_some(id), tiene_flujos(id, ENTRADA).then_some(id), cambio)?;
        if usar_salida {
            // Si el aparato no admite avisos (algunos HDMI), se queda solo como salida.
            let _ = escribir(SISTEMA, &dir(SALIDA_AVISOS, GLOBAL, 0), id);
        }
        Ok(())
    }
}
