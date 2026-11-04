//! Si algo sale muy mal. La app final se compila con `panic = "abort"`: sin esto se cerraría sin decir nada.
//!
//! - `instalar`: antes de cerrarse por un pánico, anota el motivo en `nexo-errores.log`, en la carpeta de ajustes
//!   (`%APPDATA%\com.ronny.nexo` en Windows, `~/Library/Application Support/com.ronny.nexo` en la Mac).
//! - `fatal`: Nexo no pudo arrancar: lo anota, en Windows lo dice en un cuadro y sale sin abortar.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// La carpeta de ajustes de Nexo, sin depender de Tauri (sirve antes de que arranque).
const IDENTIFICADOR: &str = "com.ronny.nexo";
const MAX_LOG: u64 = 256 * 1024;

pub fn instalar() {
    let anterior = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        anotar(&format!("Pánico: {info}"));
        anterior(info);
    }));
}

/// No se pudo iniciar: se anota, se avisa (Windows) y se sale.
pub fn fatal(motivo: &str) -> ! {
    anotar(&format!("No se pudo iniciar: {motivo}"));
    eprintln!("No se pudo iniciar Nexo: {motivo}");
    #[cfg(windows)]
    mostrar(motivo);
    std::process::exit(1)
}

fn archivo() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"));
    #[cfg(not(any(windows, target_os = "macos")))]
    let base = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"));
    Some(base?.join(IDENTIFICADOR).join("nexo-errores.log"))
}

/// Agrega una línea con la hora (segundos UTC) y la compilación. Si el registro creció mucho, empieza de nuevo.
fn anotar(texto: &str) {
    let Some(ruta) = archivo() else { return };
    if let Some(carpeta) = ruta.parent() {
        let _ = std::fs::create_dir_all(carpeta);
    }
    let grande = std::fs::metadata(&ruta).is_ok_and(|m| m.len() > MAX_LOG);
    let abierto = std::fs::OpenOptions::new().create(true).append(!grande).write(true).truncate(grande).open(&ruta);
    if let Ok(mut f) = abierto {
        let ahora = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "[{ahora}] [{}] {texto}", super::actualizar::BUILD);
    }
}

#[cfg(windows)]
fn mostrar(motivo: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW};
    use windows::core::{HSTRING, w};
    let texto = HSTRING::from(format!(
        "Nexo no pudo abrirse.\n\n{motivo}\n\nSi vuelve a pasar, reinicia el equipo. El detalle quedó en %APPDATA%\\{IDENTIFICADOR}\\nexo-errores.log."
    ));
    let _ = unsafe { MessageBoxW(None, &texto, w!("Nexo"), MB_OK | MB_ICONERROR | MB_SETFOREGROUND) };
}
