//! Lo común a todos los dispositivos y los comandos que usa el panel.
//!
//! Cada familia de aparatos tiene su propio módulo (hoy Bluetooth; luego monitores y luces).
//! Todos devuelven el mismo `Dispositivo`, así el panel los muestra igual.

// El menú del ícono (sistema/bandeja.rs) también usa el sonido.
pub mod audio;
mod bluetooth;
pub mod historial;
pub mod pantallas;
/// Windows: COM, nodos PnP y puntos de sonido que comparten Bluetooth y sonido.
#[cfg(windows)]
mod win;

use historial::{Automatico, Historial, Registro};
use pantallas::{ModoPantalla, Pantalla, Pantallas};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Monitor y Luz se usan cuando lleguen sus módulos.
pub enum Tipo {
    Audifonos,
    Diadema,
    Telefono,
    Teclado,
    Mouse,
    Parlante,
    Monitor,
    Luz,
    Otro,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dispositivo {
    /// Identificador estable (en Bluetooth, su dirección).
    pub id: String,
    pub nombre: String,
    pub tipo: Tipo,
    pub conectado: bool,
    pub bateria: Option<u8>,
    pub bateria_izquierda: Option<u8>,
    pub bateria_derecha: Option<u8>,
    pub bateria_estuche: Option<u8>,
    pub fabricante: Option<String>,
    pub firmware: Option<String>,
    /// Marca y modelo en hexadecimal (por ejemplo 0x054C y 0x0EAD); sirven para elegir su foto.
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    /// Nombre que la persona le puso dentro de Nexo (el del aparato no cambia). Vacío si no tiene.
    pub alias: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Estado {
    pub bluetooth_encendido: bool,
    pub dispositivos: Vec<Dispositivo>,
}

/// Lista los dispositivos con su estado actual (y el alias que tenga cada uno en Nexo).
#[tauri::command]
pub async fn listar_dispositivos(historial: State<'_, Arc<Historial>>) -> Result<Estado, String> {
    let h = historial.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut estado = bluetooth::listar()?;
        h.poner_alias_en(&mut estado.dispositivos);
        Ok(estado)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Pone (o quita, si llega vacío) el nombre con el que Nexo muestra un aparato.
#[tauri::command]
pub fn poner_alias(historial: State<'_, Arc<Historial>>, id: String, alias: Option<String>) {
    historial.poner_alias(&id, alias);
}

/// Olvida (desempareja) un aparato Bluetooth. Si macOS no lo permite, el error pide hacerlo en Ajustes.
#[tauri::command]
pub async fn olvidar_dispositivo(id: String) -> Result<(), String> {
    if crate::sistema::permisos::bluetooth() != "concedido" {
        return Err("Nexo necesita el permiso de Bluetooth. Actívalo en Ajustes › Permisos.".into());
    }
    tauri::async_runtime::spawn_blocking(move || bluetooth::olvidar(&id))
        .await
        .map_err(|e| e.to_string())?
}

/// Conecta (`conectar = true`) o desconecta un dispositivo Bluetooth emparejado.
#[tauri::command]
pub async fn cambiar_conexion(id: String, conectar: bool) -> Result<(), String> {
    // Sin el permiso de Bluetooth, macOS deja la llamada esperando su aviso (y la app "girando" sin fin):
    // mejor decirlo enseguida.
    if crate::sistema::permisos::bluetooth() != "concedido" {
        return Err("Nexo necesita el permiso de Bluetooth. Actívalo en Ajustes › Permisos.".into());
    }
    tauri::async_runtime::spawn_blocking(move || bluetooth::cambiar_conexion(&id, conectar))
        .await
        .map_err(|e| e.to_string())?
}

/// Volumen, silencio y micrófono de un aparato de sonido.
#[tauri::command]
pub async fn sonido(id: String) -> Result<audio::Sonido, String> {
    tauri::async_runtime::spawn_blocking(move || audio::estado(&id))
        .await
        .map_err(|e| e.to_string())?
}

/// Aplica un cambio de sonido y devuelve cómo quedó.
#[tauri::command]
pub async fn ajustar_sonido(id: String, cambio: audio::Cambio) -> Result<audio::Sonido, String> {
    tauri::async_runtime::spawn_blocking(move || {
        audio::ajustar(&id, cambio)?;
        audio::estado(&id)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- Audio del sistema (página Audio)

/// Todos los aparatos de sonido del sistema (integrados, Bluetooth, USB, HDMI…), no solo Bluetooth.
#[tauri::command]
pub async fn dispositivos_audio() -> Result<Vec<audio::DispositivoAudio>, String> {
    tauri::async_runtime::spawn_blocking(audio::listar).await.map_err(|e| e.to_string())
}

/// Usa un aparato como salida (los avisos del sistema lo siguen) o, con `entrada`, como micrófono.
/// Devuelve la lista actualizada.
#[tauri::command]
pub async fn usar_dispositivo_audio(uid: String, entrada: bool) -> Result<Vec<audio::DispositivoAudio>, String> {
    let cambio = if entrada { audio::Cambio::UsarMicrofono } else { audio::Cambio::UsarSalida };
    cambiar_audio(uid, cambio).await
}

/// Cambia el volumen, el silencio o el volumen del micrófono del aparato con ese UID. Devuelve la lista actualizada.
#[tauri::command]
pub async fn ajustar_dispositivo_audio(uid: String, cambio: audio::Cambio) -> Result<Vec<audio::DispositivoAudio>, String> {
    cambiar_audio(uid, cambio).await
}

async fn cambiar_audio(uid: String, cambio: audio::Cambio) -> Result<Vec<audio::DispositivoAudio>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        audio::ajustar_por_uid(&uid, cambio)?;
        Ok(audio::listar())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// "Probar sonido": un aviso corto por la salida en uso (Mac: Tink; Windows: el aviso del sistema).
#[tauri::command]
pub async fn probar_sonido() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(audio::probar).await.map_err(|e| e.to_string())?
}

/// Mac: el volumen de avisos y efectos (0–100). En Windows no hay uno aparte: `None`.
#[tauri::command]
pub async fn volumen_avisos() -> Option<u8> {
    tauri::async_runtime::spawn_blocking(audio::volumen_avisos).await.ok().flatten()
}

/// Mac: cambia el volumen de avisos y efectos.
#[tauri::command]
pub async fn poner_volumen_avisos(valor: u8) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || audio::poner_volumen_avisos(valor)).await.map_err(|e| e.to_string())?
}

/// Pantallas conectadas y su brillo.
#[tauri::command]
pub async fn listar_pantallas(pantallas: State<'_, Arc<Pantallas>>) -> Result<Vec<Pantalla>, String> {
    let p = pantallas.inner().clone();
    tauri::async_runtime::spawn_blocking(move || p.listar()).await.map_err(|e| e.to_string())
}

/// Cambia el brillo (0–100) de una pantalla y devuelve el nivel final.
#[tauri::command]
pub async fn poner_brillo(pantallas: State<'_, Arc<Pantallas>>, id: u32, valor: u8) -> Result<u8, String> {
    let p = pantallas.inner().clone();
    tauri::async_runtime::spawn_blocking(move || p.poner_brillo(id, valor))
        .await
        .map_err(|e| e.to_string())?
}

/// Brillo actual de cada pantalla, al instante: [(id, 0–100)]. El panel lo lee seguido mientras está abierto.
#[tauri::command]
pub fn brillos(pantallas: State<'_, Arc<Pantallas>>) -> Vec<(u32, u8)> {
    pantallas.brillos()
}

/// Resoluciones y frecuencias que se pueden elegir en una pantalla (la actual viene marcada).
#[tauri::command]
pub async fn modos_pantalla(id: u32) -> Vec<ModoPantalla> {
    tauri::async_runtime::spawn_blocking(move || pantallas::modos(id)).await.unwrap_or_default()
}

/// Cambia la resolución y la frecuencia de una pantalla. El panel pregunta si se queda
/// y, si no, vuelve a llamar con el modo anterior.
#[tauri::command]
pub async fn cambiar_modo_pantalla(id: u32, modo: i32) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || pantallas::cambiar_modo(id, modo))
        .await
        .map_err(|e| e.to_string())?
}

/// Hace principal a una pantalla (la que tiene la barra de menús), sin cambiar la distribución.
#[tauri::command]
pub async fn hacer_principal(id: u32) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || pantallas::hacer_principal(id))
        .await
        .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- Historial y automático

/// Lo que usa el vigía de lib.rs: la lista actual y, si toca, usar un aparato como salida de sonido.
pub fn listar_ahora() -> Result<Estado, String> {
    bluetooth::listar()
}

pub fn usar_como_salida(id: &str) -> Result<(), String> {
    audio::ajustar(id, audio::Cambio::UsarSalida)
}

/// Sesiones de uso y muestras de batería de un aparato.
#[tauri::command]
pub fn historial(historial: State<'_, Arc<Historial>>, id: String) -> Registro {
    historial.registro(&id)
}

/// Borra el uso y la batería guardados de todos los aparatos (los ajustes automáticos y los alias se quedan).
#[tauri::command]
pub fn borrar_historial(historial: State<'_, Arc<Historial>>) {
    historial.borrar();
}

#[tauri::command]
pub fn automatico(historial: State<'_, Arc<Historial>>, id: String) -> Automatico {
    historial.automatico(&id)
}

#[tauri::command]
pub fn guardar_automatico(historial: State<'_, Arc<Historial>>, id: String, ajustes: Automatico) -> Automatico {
    historial.guardar_automatico(&id, ajustes)
}

/// Abre Ajustes del Sistema › Bluetooth.
#[tauri::command]
pub fn abrir_ajustes_bluetooth() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("/usr/bin/open").arg("x-apple.systempreferences:com.apple.BluetoothSettings").spawn();
    #[cfg(windows)]
    // explorer abre la Configuración sin que se asome una ventana de consola.
    let r = std::process::Command::new("explorer").arg("ms-settings:bluetooth").spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// Abre los ajustes de sonido del sistema (Ajustes del Sistema › Sonido, o Configuración › Sonido).
#[tauri::command]
pub async fn abrir_ajustes_sonido() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(target_os = "macos")]
        let r = std::process::Command::new("/usr/bin/open")
            .arg("x-apple.systempreferences:com.apple.Sound-Settings.extension")
            .status()
            .map(|_| ());
        #[cfg(windows)]
        let r = std::process::Command::new("explorer").arg("ms-settings:sound").spawn().map(|_| ());
        #[cfg(not(any(target_os = "macos", windows)))]
        let r: std::io::Result<()> = Ok(());
        r.map_err(|e| format!("No pude abrir los ajustes de sonido ({e})."))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- Cuenta del sistema

/// Nombre para el saludo de Inicio ("Hola, Ronny"): el que escribió la persona en la bienvenida
/// o, si no hay, el de pila de la cuenta. Vacío solo si el sistema no da ningún nombre.
#[tauri::command]
pub async fn nombre_usuario(app: tauri::AppHandle) -> String {
    tauri::async_runtime::spawn_blocking(move || crate::sistema::nombre_para_saludo(&app)).await.unwrap_or_default()
}

/// Nombre de pila de quien usa la cuenta del sistema.
pub fn nombre_de_pila() -> String {
    // Del nombre completo, la primera palabra ("Ronny Isaac Arellano" → "Ronny").
    let completo = nombre_completo().and_then(|n| n.split_whitespace().next().map(str::to_string));
    // Si no, el usuario de la cuenta ("ronny" → "Ronny").
    completo
        .or_else(|| {
            let cuenta = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok()?;
            let parte = cuenta.split(|c: char| !c.is_alphanumeric()).find(|p| !p.is_empty())?;
            let mut letras = parte.chars();
            letras.next().map(|primera| primera.to_uppercase().chain(letras).collect())
        })
        .unwrap_or_default()
}

/// Mac: el nombre completo de la cuenta (`id -F`).
#[cfg(target_os = "macos")]
fn nombre_completo() -> Option<String> {
    let salida = std::process::Command::new("/usr/bin/id").arg("-F").output().ok()?;
    salida.status.success().then(|| String::from_utf8_lossy(&salida.stdout).trim().to_string())
}

/// Windows: el nombre visible de la cuenta (GetUserNameExW). Suele fallar en cuentas locales;
/// entonces se usa USERNAME. Si viene como "Apellido, Nombre", se toma lo que va tras la coma.
#[cfg(windows)]
fn nombre_completo() -> Option<String> {
    #[link(name = "secur32")]
    unsafe extern "system" {
        fn GetUserNameExW(formato: u32, nombre: *mut u16, tam: *mut u32) -> u8;
    }
    const NOMBRE_VISIBLE: u32 = 3;
    let mut buf = [0u16; 256];
    let mut tam = buf.len() as u32;
    if unsafe { GetUserNameExW(NOMBRE_VISIBLE, buf.as_mut_ptr(), &mut tam) } == 0 {
        return None;
    }
    let nombre = String::from_utf16_lossy(&buf[..(tam as usize).min(buf.len())]);
    let nombre = nombre.split_once(',').map_or(nombre.as_str(), |(_, n)| n).trim().to_string();
    (!nombre.is_empty()).then_some(nombre)
}

#[cfg(not(any(target_os = "macos", windows)))]
fn nombre_completo() -> Option<String> {
    None
}
