//! Nexo Drop en la Mac: el portapapeles (con el código de Remoto), miniaturas con ImageIO y abrir o mostrar
//! con `open` (Finder, el navegador o la app de cada archivo).

use super::super::remoto::{imagen, portapapeles};
use super::Copiado;
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};

/// Las imágenes que no son PNG se convierten con este lado como mucho (para pegar alcanza y sobra).
const LADO_PORTAPAPELES: u32 = 4096;
const LADO_MINIATURA: u32 = 128;

pub fn poner_texto(texto: &str) -> bool {
    portapapeles::poner_texto(texto)
}

/// Pone una imagen recibida en el portapapeles, en PNG.
pub fn poner_imagen(ruta: &Path) -> bool {
    let Ok(datos) = std::fs::read(ruta) else { return false };
    if datos.starts_with(b"\x89PNG\r\n\x1a\n") {
        return portapapeles::poner_png(&datos);
    }
    let Some(medida) = imagen::medir(&datos) else { return false };
    let lado = medida.ancho.max(medida.alto).min(LADO_PORTAPAPELES);
    imagen::achicar_png(&datos, lado).is_some_and(|(png, _, _)| portapapeles::poner_png(&png))
}

pub fn leer_copiado() -> Option<Copiado> {
    Some(match portapapeles::leer_copiado()? {
        portapapeles::Copiado::Texto { texto, oculto } => Copiado::Texto { texto, oculto },
        portapapeles::Copiado::Imagen(png) => Copiado::Imagen(png),
        portapapeles::Copiado::Archivos(rutas) => Copiado::Archivos(rutas),
    })
}

/// Miniatura de una imagen (PNG de 128 px como mucho) como data URL.
pub fn miniatura(ruta: &Path) -> Option<String> {
    let datos = std::fs::read(ruta).ok()?;
    let (png, _, _) = imagen::achicar_png(&datos, LADO_MINIATURA)?;
    Some(format!("data:image/png;base64,{}", super::rutas::base64(&png)))
}

/// Corre `open` con esos argumentos y espera a que termine (es inmediato).
fn open(args: &[&OsStr]) -> Result<(), String> {
    let estado = Command::new("/usr/bin/open")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if estado.success() { Ok(()) } else { Err("macOS no pudo abrirlo.".into()) }
}

/// Abre un archivo con su app, o una carpeta en Finder.
pub fn abrir(ruta: &Path) -> Result<(), String> {
    open(&[ruta.as_os_str()])
}

/// Muestra el archivo o la carpeta en Finder, ya elegido.
pub fn mostrar(ruta: &Path) -> Result<(), String> {
    open(&[OsStr::new("-R"), ruta.as_os_str()])
}

/// Abre un enlace (ya revisado: http o https) en el navegador.
pub fn abrir_enlace(url: &str) -> Result<(), String> {
    open(&[OsStr::new(url)])
}

/// El nombre del equipo ("demo-sige"), sin ".local".
pub fn nombre_equipo() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: gethostname escribe como mucho `buf.len()` bytes en el búfer.
    let r = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if r != 0 {
        return "Mac".into();
    }
    let fin = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    let nombre = String::from_utf8_lossy(&buf[..fin]).into_owned();
    let nombre = nombre.strip_suffix(".local").unwrap_or(&nombre).to_string();
    if nombre.is_empty() { "Mac".into() } else { nombre }
}
