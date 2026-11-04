//! Bluetooth de bajo consumo: buscar luces cercanas y manejar las tiras con protocolo abierto.
//!
//! - `buscar`: una búsqueda corta (≈6 s); al terminar se detiene y no queda nada escuchando. Solo devuelve luces
//!   (o lo que parece una luz): ni teléfonos, ni audífonos, ni teclados.
//! - `cambiar`: conecta, escribe y se desconecta (≈8 s como mucho). Maneja ELK-BLEDOM / duoCo / Lotus Lantern y
//!   Triones / Happy Lighting / Magic Home. Lo cifrado (Tuya sin puerta de enlace, Xiaomi, Govee) solo se reconoce.
//! - Mac: CoreBluetooth en una cola propia (`mac`). Windows: WinRT (`win`). En otros sistemas, nada.
//! - Estas tiras no dicen cómo están: se recuerda lo último que se les mandó (mientras Nexo esté abierto).

mod tramas;

#[cfg(target_os = "macos")]
mod mac;
#[cfg(windows)]
mod win;

use super::modelo::{Cambio, Hallazgo, Manejo, Proveedor};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tramas::{Protocolo, Recuerdo};

/// Lo último que se mandó a cada luz (por su dirección), para el brillo de Triones.
static RECUERDOS: Mutex<Option<HashMap<String, Recuerdo>>> = Mutex::new(None);

/// Busca luces Bluetooth cercanas durante `plazo`. Error en palabras si el Bluetooth está apagado o sin permiso.
pub fn buscar(plazo: Duration) -> Result<Vec<Hallazgo>, String> {
    let anuncios = plataforma_buscar(plazo)?;
    let mut lista: Vec<Hallazgo> = anuncios.iter().filter_map(tramas::clasificar).collect();
    // Primero las que Nexo maneja; luego, la señal más fuerte.
    lista.sort_by_key(|h| (!matches!(h.manejo, Manejo::Listo), -(h.senal.unwrap_or(-127) as i32)));
    Ok(lista)
}

/// Manda un cambio a una luz Bluetooth. `direccion` es la que dio `buscar`.
pub fn cambiar(proveedor: Proveedor, direccion: &str, cambio: &Cambio) -> Result<(), String> {
    let protocolo = Protocolo::de(proveedor).ok_or("Nexo no maneja esta luz por Bluetooth.")?;
    let direccion = direccion.trim();
    if direccion.is_empty() {
        return Err("Falta la dirección Bluetooth de la luz: búscala de nuevo.".into());
    }
    if cambio.vacio() {
        return Ok(());
    }
    let clave = direccion.to_lowercase();
    let recuerdo = {
        let guardia = RECUERDOS.lock().unwrap_or_else(|e| e.into_inner());
        guardia.as_ref().and_then(|m| m.get(&clave).copied()).unwrap_or_default()
    };
    plataforma_escribir(direccion, protocolo, *cambio, recuerdo)?;
    let mut guardia = RECUERDOS.lock().unwrap_or_else(|e| e.into_inner());
    guardia.get_or_insert_with(HashMap::new).entry(clave).or_default().aplicar(cambio);
    Ok(())
}

#[cfg(target_os = "macos")]
fn plataforma_buscar(plazo: Duration) -> Result<Vec<tramas::Anuncio>, String> {
    mac::buscar(plazo)
}

#[cfg(target_os = "macos")]
fn plataforma_escribir(direccion: &str, p: Protocolo, c: Cambio, r: Recuerdo) -> Result<(), String> {
    mac::escribir(direccion, p, c, r)
}

#[cfg(windows)]
fn plataforma_buscar(plazo: Duration) -> Result<Vec<tramas::Anuncio>, String> {
    win::buscar(plazo)
}

#[cfg(windows)]
fn plataforma_escribir(direccion: &str, p: Protocolo, c: Cambio, r: Recuerdo) -> Result<(), String> {
    win::escribir(direccion, p, c, r)
}

#[cfg(not(any(target_os = "macos", windows)))]
fn plataforma_buscar(_plazo: Duration) -> Result<Vec<tramas::Anuncio>, String> {
    Err("Buscar luces por Bluetooth todavía no está disponible en este sistema.".into())
}

#[cfg(not(any(target_os = "macos", windows)))]
fn plataforma_escribir(_direccion: &str, _p: Protocolo, _c: Cambio, _r: Recuerdo) -> Result<(), String> {
    Err("Manejar luces por Bluetooth todavía no está disponible en este sistema.".into())
}

/// Los mensajes comunes de los dos sistemas.
#[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
mod textos {
    pub const APAGADO: &str = "El Bluetooth está apagado. Enciéndelo y vuelve a buscar.";
    pub const NO_ENCONTRADA: &str = "No encontré la luz cerca. Acércate o revisa que esté encendida.";
    pub const SIN_ESCRITURA: &str = "La luz respondió, pero no tiene lo que Nexo necesita para manejarla.";
    pub const NO_RESPONDE: &str = "La luz no respondió a tiempo. Acércate y vuelve a intentar.";
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn ble_cambiar_revisa_antes_de_conectar() {
        let c = Cambio { encendido: Some(true), ..Cambio::default() };
        assert!(cambiar(Proveedor::Wiz, "AABBCCDDEEFF", &c).is_err());
        assert!(cambiar(Proveedor::BleElk, "  ", &c).is_err());
        // Un cambio vacío no conecta.
        assert_eq!(cambiar(Proveedor::BleElk, "AABBCCDDEEFF", &Cambio::default()), Ok(()));
    }
}
