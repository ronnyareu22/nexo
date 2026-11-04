//! La primera vez en Windows, Nexo queda listo sin tocar nada.
//!
//! Lo de siempre ya viene encendido en los ajustes: widget de escritorio, recibir por XDrop y traducir los atajos
//! para RustDesk. Lo único que vive fuera de ajustes.json es abrir Nexo al iniciar sesión (HKCU\…\Run): si todavía
//! no hay ajustes guardados (recién instalado), se activa aquí y se guardan los ajustes de siempre. Así queda
//! aunque se cierre la bienvenida a medias; al terminarla, se respeta lo que elijas en su interruptor.
//!
//! Las teclas de brillo (Ctrl+Alt+F1 / F2) llegaron después: se encienden una sola vez, también en las instalaciones
//! que ya tenían ajustes (antes no había cómo usarlas en Windows). Si después las apagas, quedan apagadas.

use super::{ajustes, arranque};
use tauri::{AppHandle, Manager};

/// Al abrir, antes de que nada guarde ajustes.
pub fn aplicar(app: &AppHandle) {
    if !cfg!(windows) {
        return;
    }
    let Ok(carpeta) = app.path().app_config_dir() else { return };
    if !carpeta.join("ajustes.json").exists() {
        match arranque::usar(true) {
            Ok(true) => {}
            Ok(false) => eprintln!("Nexo: el arranque no quedó activo"),
            Err(e) => eprintln!("Nexo: {e}"),
        }
        // Con ajustes.json creado, esto no vuelve a pasar.
        if let Err(e) = ajustes::cambiar(app, |_| {}) {
            eprintln!("Nexo: no se pudieron guardar los ajustes iniciales ({e})");
        }
    }
    // Teclas de brillo encendidas, una sola vez. Solo si ajustes.json se leyó bien: si no, no se toca.
    if ajustes::leer(app).is_some_and(|a| !a.teclas_brillo_revisadas) {
        let cambio = ajustes::cambiar(app, |a| {
            a.teclas_brillo = true;
            a.teclas_brillo_revisadas = true;
        });
        if let Err(e) = cambio {
            eprintln!("Nexo: no se pudieron encender las teclas de brillo ({e})");
        }
    }
}
