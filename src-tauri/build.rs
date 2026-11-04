//! Compilación: lo de Tauri y el sello de esta compilación.
//!
//! - `NEXO_COMPILADO`: segundos desde 1970 (UTC). Lo pone `scripts/publicar-windows.sh`, así el instalador y su
//!   info.json dicen lo mismo; si no viene, es la hora de ahora.
//! - `NEXO_BUILD`: un id corto: el commit de git (con "+" si había cambios sin guardar) o, sin git, el sello en base 36.
//!
//! La versión sigue en 0.1.0: Nexo en Windows sabe si la Mac ofrece algo más nuevo comparando el sello.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // Cada publicación trae un sello nuevo: con esto se vuelve a compilar con él.
    println!("cargo:rerun-if-env-changed=NEXO_COMPILADO");
    let compilado = std::env::var("NEXO_COMPILADO")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or_else(|| SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
    let id = commit().unwrap_or_else(|| base36(compilado));
    println!("cargo:rustc-env=NEXO_COMPILADO={compilado}");
    println!("cargo:rustc-env=NEXO_BUILD={id}");
    tauri_build::build()
}

/// El commit actual en 7 letras, con "+" si había cambios sin guardar. `None` si no hay git.
fn commit() -> Option<String> {
    let git = |args: &[&str]| -> Option<String> {
        let salida = Command::new("git").args(args).output().ok()?;
        salida.status.success().then(|| String::from_utf8_lossy(&salida.stdout).trim().to_string())
    };
    let corto = git(&["rev-parse", "--short=7", "HEAD"]).filter(|c| !c.is_empty() && c.len() <= 12)?;
    let cambios = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    Some(if cambios { format!("{corto}+") } else { corto })
}

/// El sello en base 36 ("t4x9k2"): corto y distinto en cada compilación.
fn base36(mut n: u64) -> String {
    const CIFRAS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut texto = Vec::new();
    loop {
        texto.push(CIFRAS[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    texto.reverse();
    String::from_utf8(texto).unwrap_or_default()
}
