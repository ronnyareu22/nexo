//! Nexo Drop: funciones puras para nombres y direcciones.
//! - Limpiar la ruta relativa que llega del otro equipo ("Fotos/viaje/a.jpg"): sin "..", sin rutas absolutas,
//!   sin letras de unidad y sin caracteres que Windows no acepta.
//! - Nombres libres ante un choque: "foto (2).png".
//! - Codificar y decodificar textos en la dirección (%XX).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Cuántas carpetas puede tener una ruta relativa.
const MAX_PARTES: usize = 32;
/// Largo máximo de cada nombre, en bytes (los sistemas de archivos aceptan 255).
const MAX_NOMBRE: usize = 200;

/// Parte una ruta relativa recibida en nombres seguros. `None` si no queda ninguno usable.
pub fn partes_seguras(ruta: &str) -> Option<Vec<String>> {
    let mut partes = Vec::new();
    for (i, parte) in ruta.split(['/', '\\']).enumerate() {
        // "C:" al principio es una letra de unidad: se descarta.
        if i == 0 && parte.len() == 2 && parte.as_bytes()[0].is_ascii_alphabetic() && parte.ends_with(':') {
            continue;
        }
        if parte == "." || parte == ".." {
            continue;
        }
        let Some(limpia) = nombre_seguro(parte) else { continue };
        partes.push(limpia);
        if partes.len() > MAX_PARTES {
            return None;
        }
    }
    (!partes.is_empty()).then_some(partes)
}

/// Un nombre de archivo o carpeta seguro en Mac y en Windows. `None` si queda vacío.
pub fn nombre_seguro(nombre: &str) -> Option<String> {
    let limpio: String = nombre
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' | '/' | '\\' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    // Windows no acepta puntos ni espacios al final.
    let limpio = limpio.trim().trim_end_matches(['.', ' ']).to_string();
    if limpio.is_empty() || limpio == "." || limpio == ".." {
        return None;
    }
    let limpio = if reservado_en_windows(&limpio) { format!("_{limpio}") } else { limpio };
    Some(acortar(&limpio))
}

/// CON, PRN, AUX, NUL, COM1…COM9 y LPT1…LPT9 (también con extensión) no se pueden usar en Windows.
fn reservado_en_windows(nombre: &str) -> bool {
    let base = nombre.split('.').next().unwrap_or(nombre).trim_end().to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base.as_bytes()[3].is_ascii_digit()
            && base.as_bytes()[3] != b'0')
}

/// Deja el nombre en `MAX_NOMBRE` bytes como mucho, conservando la extensión.
fn acortar(nombre: &str) -> String {
    if nombre.len() <= MAX_NOMBRE {
        return nombre.to_string();
    }
    let (base, ext) = separar(nombre);
    let mut corto = String::new();
    for c in base.chars() {
        if corto.len() + c.len_utf8() + ext.len() > MAX_NOMBRE {
            break;
        }
        corto.push(c);
    }
    format!("{corto}{ext}")
}

/// ("foto", ".png"). Un punto al principio (".bashrc") no cuenta como extensión.
fn separar(nombre: &str) -> (&str, &str) {
    match nombre.rfind('.') {
        Some(i) if i > 0 && i < nombre.len() - 1 => (&nombre[..i], &nombre[i..]),
        _ => (nombre, ""),
    }
}

/// El nombre con un número: "foto.png" → "foto (2).png"; "Fotos" → "Fotos (2)".
pub fn con_numero(nombre: &str, n: u32, es_carpeta: bool) -> String {
    if es_carpeta {
        return format!("{nombre} ({n})");
    }
    let (base, ext) = separar(nombre);
    format!("{base} ({n}){ext}")
}

/// La primera ruta libre dentro de `carpeta` para ese nombre: el mismo, o con (2), (3)…
pub fn ruta_libre(carpeta: &Path, nombre: &str, es_carpeta: bool) -> PathBuf {
    let mut ruta = carpeta.join(nombre);
    let mut n = 2;
    while ruta.symlink_metadata().is_ok() && n < 10_000 {
        ruta = carpeta.join(con_numero(nombre, n, es_carpeta));
        n += 1;
    }
    ruta
}

/// Codifica un texto para la dirección: deja letras, números y `-_.~`; lo demás va en %XX (UTF-8).
pub fn codificar(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len() * 3 / 2);
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            salida.push(b as char);
        } else {
            salida.push_str(&format!("%{b:02X}"));
        }
    }
    salida
}

/// Decodifica %XX (y "+" como espacio). `None` si viene mal formado o no es UTF-8.
pub fn decodificar(texto: &str) -> Option<String> {
    let bytes = texto.as_bytes();
    let mut salida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let alto = (*bytes.get(i + 1)? as char).to_digit(16)?;
                let bajo = (*bytes.get(i + 2)? as char).to_digit(16)?;
                salida.push((alto * 16 + bajo) as u8);
                i += 3;
            }
            b'+' => {
                salida.push(b' ');
                i += 1;
            }
            b => {
                salida.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(salida).ok()
}

/// Los parámetros de la dirección ("a=1&b=dos") ya decodificados. Los mal formados se ignoran.
pub fn consulta(texto: &str) -> HashMap<String, String> {
    texto
        .split('&')
        .filter_map(|par| {
            let (clave, valor) = par.split_once('=').unwrap_or((par, ""));
            Some((decodificar(clave)?, decodificar(valor)?))
        })
        .collect()
}

/// Un identificador de lote seguro: letras, números, "-" y "_", hasta 64.
pub fn lote_seguro(texto: &str) -> Option<String> {
    let ok = !texto.is_empty() && texto.len() <= 64 && texto.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    ok.then(|| texto.to_string())
}

/// Si el nombre parece una imagen (por su extensión).
pub fn es_imagen(nombre: &str) -> bool {
    let ext = separar(nombre).1.trim_start_matches('.').to_ascii_lowercase();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "heic" | "heif" | "webp" | "bmp" | "tif" | "tiff")
}

/// Base64 estándar (para las miniaturas en data URL).
pub fn base64(datos: &[u8]) -> String {
    const TABLA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let letra = |n: u32| TABLA[(n & 63) as usize] as char;
    let mut s = String::with_capacity(datos.len().div_ceil(3) * 4);
    for trozo in datos.chunks(3) {
        let b0 = trozo.first().copied().unwrap_or(0) as u32;
        let b1 = trozo.get(1).copied().unwrap_or(0) as u32;
        let b2 = trozo.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        s.push(letra(n >> 18));
        s.push(letra(n >> 12));
        s.push(if trozo.len() > 1 { letra(n >> 6) } else { '=' });
        s.push(if trozo.len() > 2 { letra(n) } else { '=' });
    }
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn limpia_rutas_peligrosas() {
        assert_eq!(partes_seguras("Fotos/viaje/a.jpg"), Some(vec!["Fotos".into(), "viaje".into(), "a.jpg".into()]));
        assert_eq!(partes_seguras("../../etc/passwd"), Some(vec!["etc".into(), "passwd".into()]));
        assert_eq!(partes_seguras("/Users/otro/.ssh/id"), Some(vec!["Users".into(), "otro".into(), ".ssh".into(), "id".into()]));
        assert_eq!(partes_seguras("C:\\Windows\\system32\\x.dll"), Some(vec!["Windows".into(), "system32".into(), "x.dll".into()]));
        assert_eq!(partes_seguras("a\\..\\b"), Some(vec!["a".into(), "b".into()]));
        assert_eq!(partes_seguras("..//.."), None);
        assert_eq!(partes_seguras(""), None);
        assert_eq!(partes_seguras("hola?.txt"), Some(vec!["hola_.txt".into()]));
        assert_eq!(partes_seguras("nota. . "), Some(vec!["nota".into()]));
        assert_eq!(partes_seguras("con.txt"), Some(vec!["_con.txt".into()]));
        assert_eq!(partes_seguras("COM1"), Some(vec!["_COM1".into()]));
        assert_eq!(partes_seguras("COM0"), Some(vec!["COM0".into()]));
        assert_eq!(partes_seguras("línea\nnueva.txt"), Some(vec!["línea_nueva.txt".into()]));
        let largo = format!("{}.png", "á".repeat(300));
        let p = partes_seguras(&largo).unwrap_or_default();
        assert_eq!(p.len(), 1);
        assert!(p[0].len() <= MAX_NOMBRE && p[0].ends_with(".png"));
    }

    #[test]
    fn numera_los_nombres() {
        assert_eq!(con_numero("foto.png", 2, false), "foto (2).png");
        assert_eq!(con_numero("archivo", 3, false), "archivo (3)");
        assert_eq!(con_numero(".bashrc", 2, false), ".bashrc (2)");
        assert_eq!(con_numero("Fotos.2024", 2, true), "Fotos.2024 (2)");
        assert_eq!(con_numero("a.tar.gz", 2, false), "a.tar (2).gz");
    }

    #[test]
    fn busca_un_nombre_libre() {
        let carpeta = std::env::temp_dir().join(format!("nexo-drop-prueba-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&carpeta);
        std::fs::create_dir_all(&carpeta).expect("carpeta de prueba");
        assert_eq!(ruta_libre(&carpeta, "foto.png", false), carpeta.join("foto.png"));
        std::fs::write(carpeta.join("foto.png"), b"1").expect("archivo de prueba");
        std::fs::write(carpeta.join("foto (2).png"), b"2").expect("archivo de prueba");
        assert_eq!(ruta_libre(&carpeta, "foto.png", false), carpeta.join("foto (3).png"));
        let _ = std::fs::remove_dir_all(&carpeta);
    }

    #[test]
    fn codifica_y_decodifica() {
        let texto = "Fotos/viaje 2024/ñandú & más+.png";
        assert_eq!(decodificar(&codificar(texto)).as_deref(), Some(texto));
        assert_eq!(decodificar("a+b%20c").as_deref(), Some("a b c"));
        assert_eq!(decodificar("%zz"), None);
        assert_eq!(decodificar("%4"), None);
        let q = consulta("nombre=foto%20(2).png&tamano=10&vacio=");
        assert_eq!(q.get("nombre").map(String::as_str), Some("foto (2).png"));
        assert_eq!(q.get("tamano").map(String::as_str), Some("10"));
        assert_eq!(q.get("vacio").map(String::as_str), Some(""));
        assert_eq!(lote_seguro("a1-b_2"), Some("a1-b_2".into()));
        assert_eq!(lote_seguro("../x"), None);
        assert!(es_imagen("Foto.JPG") && !es_imagen("informe.pdf") && !es_imagen("png"));
        assert_eq!(base64(b"Nexo"), "TmV4bw==");
        assert_eq!(base64(b"Ne"), "TmU=");
        assert_eq!(base64(b"Nex"), "TmV4");
    }
}
