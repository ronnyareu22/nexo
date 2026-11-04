//! Captura liviana (reemplaza la app aparte "Captura remota"): eliges una zona con `screencapture -i`,
//! se pasa de Retina a 1x (4 veces menos píxeles, así RustDesk no se congela al mandarla), va al portapapeles
//! como PNG y se guarda una copia en Imágenes › Capturas remotas. Una a la vez, siempre en un hilo aparte.

use super::{Captura, imagen, portapapeles};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

static CAPTURANDO: AtomicBool = AtomicBool::new(false);

/// Suelta el turno al terminar, pase lo que pase.
struct Turno;
impl Drop for Turno {
    fn drop(&mut self) {
        CAPTURANDO.store(false, Ordering::SeqCst);
    }
}

/// Imágenes › Capturas remotas (en disco la carpeta se llama Pictures).
pub fn carpeta() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Pictures").join("Capturas remotas"))
}

/// Desde la tecla F13: en su propio hilo, sin esperar. Si falla (por ejemplo, falta el permiso), lo avisa
/// con una notificación, que se ve también desde el PC.
pub fn lanzar() {
    if CAPTURANDO.load(Ordering::SeqCst) {
        return;
    }
    let Some(app) = super::mac::APP.get().cloned() else { return };
    let _ = std::thread::Builder::new().name("nexo-captura".into()).spawn(move || {
        if let Err(e) = capturar(&app) {
            let _ = app.notification().builder().title("No se pudo capturar").body(e).show();
        }
    });
}

/// Hace la captura y espera a que termine. `None` si se canceló con Esc.
pub fn capturar(app: &AppHandle) -> Result<Option<Captura>, String> {
    let Some((png, ancho, alto)) = tomar()? else { return Ok(None) };

    // Al portapapeles va solo el PNG liviano (marcado: el portapapeles liviano no lo vuelve a tocar).
    let copiada = portapapeles::escribir_imagen(&png);
    let archivo = guardar(&png);
    let captura = Captura { ancho, alto, kb: (png.len() / 1024) as u32, archivo, copiada };
    let _ = app.emit("remoto-captura", &captura);
    Ok(Some(captura))
}

/// Eliges una zona y queda en 1x: el PNG con su ancho y alto, sin tocar el portapapeles.
/// `None` si se canceló con Esc. También la usa XDrop ("Enviar captura").
pub fn tomar() -> Result<Option<(Vec<u8>, u32, u32)>, String> {
    if CAPTURANDO.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una captura en curso.".into());
    }
    let _turno = Turno;
    if !super::pantalla_permitida() {
        super::pedir_pantalla();
        return Err(
            "Falta el permiso de Grabación de pantalla. Actívalo para Nexo en Ajustes del Sistema › Privacidad y seguridad."
                .into(),
        );
    }

    let temporal = std::env::temp_dir().join(format!("nexo-captura-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&temporal);
    // -i: eliges la zona (Esc cancela) · -x: sin sonido. Se espera a que termine: no queda nada abierto.
    Command::new("/usr/sbin/screencapture")
        .args(["-i", "-x", "-t", "png"])
        .arg(&temporal)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("No pude abrir la captura de macOS ({e})."))?;
    let datos = std::fs::read(&temporal).unwrap_or_default();
    let _ = std::fs::remove_file(&temporal);
    if datos.is_empty() {
        return Ok(None); // cancelada
    }

    // Retina → 1x. La escala sale de los ppp que anota screencapture (144 = 2x), así sirve en cualquier pantalla.
    let medida = imagen::medir(&datos).ok_or("La captura salió dañada.")?;
    let escala = (medida.dpi / 72.0).max(1.0);
    let lado = ((medida.ancho.max(medida.alto) as f64) / escala).round().max(1.0) as u32;
    let lista = if escala > 1.05 {
        imagen::achicar_png(&datos, lado).unwrap_or((datos, medida.ancho, medida.alto))
    } else {
        (datos, medida.ancho, medida.alto)
    };
    Ok(Some(lista))
}

/// El nombre de una captura nueva, sin la extensión: "Captura 2026-11-03 a las 14.05.33".
pub fn nombre() -> String {
    format!("Captura {}", fecha())
}

/// Guarda la copia como "Captura 2026-11-03 a las 14.05.33.png". Devuelve dónde quedó.
fn guardar(png: &[u8]) -> Option<String> {
    let carpeta = carpeta()?;
    std::fs::create_dir_all(&carpeta).ok()?;
    let archivo = nombre_libre(&carpeta, &nombre());
    std::fs::write(&archivo, png).ok()?;
    Some(archivo.display().to_string())
}

fn nombre_libre(carpeta: &Path, base: &str) -> PathBuf {
    let mut ruta = carpeta.join(format!("{base}.png"));
    let mut n = 2;
    while ruta.exists() && n < 100 {
        ruta = carpeta.join(format!("{base} ({n}).png"));
        n += 1;
    }
    ruta
}

/// La fecha y hora local: "2026-11-03 a las 14.05.33".
fn fecha() -> String {
    // SAFETY: time y localtime_r solo escriben en las variables que se les pasan.
    unsafe {
        let ahora = libc::time(std::ptr::null_mut());
        let mut t: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&ahora, &mut t).is_null() {
            return ahora.to_string();
        }
        format!(
            "{:04}-{:02}-{:02} a las {:02}.{:02}.{:02}",
            t.tm_year + 1900,
            t.tm_mon + 1,
            t.tm_mday,
            t.tm_hour,
            t.tm_min,
            t.tm_sec
        )
    }
}
