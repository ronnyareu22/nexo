//! HTTP sin cifrar para los aparatos de la red local (Shelly, Elgato, WLED, Nanoleaf, Tasmota y OpenBeken).
//!
//! - HTTP/1.1 sobre TCP, una petición por conexión (`Connection: close`) y plazo corto: nada queda abierto.
//!   Solo a IPs de la red local (quien llama pasa por `ip_de`). Lo de la nube y Home Assistant sigue en `http.rs`.
//! - Devuelve el estado, las cabeceras (hace falta `WWW-Authenticate` para Shelly) y el cuerpo, con
//!   `Content-Length`, por pedazos (`chunked`) o hasta que el aparato cierra.
//! - Claves: Basic (Shelly Gen1, Tasmota, OpenBeken) y Digest con SHA-256 (Shelly Gen2 en adelante, RFC 7616).
//!   Nunca se escriben en un error ni en un registro.

use super::super::firma::hex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Como mucho 1 MB de respuesta (la info de un WLED o un Shelly ocupa unos pocos KB).
const MAX: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Respuesta {
    pub estado: u16,
    pub cabeceras: Vec<(String, String)>,
    pub cuerpo: Vec<u8>,
}

impl Respuesta {
    /// Una cabecera, sin importar mayúsculas.
    pub fn cabecera(&self, nombre: &str) -> Option<&str> {
        self.cabeceras.iter().find(|(k, _)| k.eq_ignore_ascii_case(nombre)).map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> Option<Value> {
        serde_json::from_slice(&self.cuerpo).ok()
    }

    pub fn ok(&self) -> bool {
        (200..300).contains(&self.estado)
    }
}

/// La petición en bytes.
pub fn armar(ip: Ipv4Addr, puerto: u16, metodo: &str, ruta: &str, cabeceras: &[(&str, String)], cuerpo: Option<&[u8]>) -> Vec<u8> {
    let anfitrion = if puerto == 80 { ip.to_string() } else { format!("{ip}:{puerto}") };
    let mut t = format!("{metodo} {ruta} HTTP/1.1\r\nHost: {anfitrion}\r\nConnection: close\r\nAccept: */*\r\nUser-Agent: Nexo\r\n");
    for (k, v) in cabeceras {
        // Sin saltos de línea: un valor no puede abrir otra cabecera.
        let v: String = v.chars().filter(|c| *c != '\r' && *c != '\n').collect();
        t.push_str(&format!("{k}: {v}\r\n"));
    }
    if let Some(c) = cuerpo {
        if !cabeceras.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-type")) {
            t.push_str("Content-Type: application/json\r\n");
        }
        t.push_str(&format!("Content-Length: {}\r\n", c.len()));
    } else if metodo != "GET" {
        t.push_str("Content-Length: 0\r\n");
    }
    t.push_str("\r\n");
    let mut v = t.into_bytes();
    if let Some(c) = cuerpo {
        v.extend_from_slice(c);
    }
    v
}

fn buscar(d: &[u8], aguja: &[u8]) -> Option<usize> {
    d.windows(aguja.len()).position(|w| w == aguja)
}

/// Un cuerpo por pedazos (`chunked`); `None` si está incompleto.
fn sin_pedazos(mut d: &[u8]) -> Option<Vec<u8>> {
    let mut v = Vec::new();
    loop {
        let fin = buscar(d, b"\r\n")?;
        let linea = std::str::from_utf8(&d[..fin]).ok()?;
        let tam = usize::from_str_radix(linea.split(';').next()?.trim(), 16).ok()?;
        d = &d[fin + 2..];
        if tam == 0 {
            return Some(v);
        }
        v.extend_from_slice(d.get(..tam)?);
        d = d.get(tam + 2..)?;
    }
}

/// Lee una respuesta completa (o lo que llegó si el aparato cerró). `None` si no es HTTP.
pub fn leer(d: &[u8]) -> Option<Respuesta> {
    let fin = buscar(d, b"\r\n\r\n")?;
    let cabeza = std::str::from_utf8(&d[..fin]).ok()?;
    let mut lineas = cabeza.split("\r\n");
    let primera = lineas.next()?;
    let mut partes = primera.split_whitespace();
    if !partes.next()?.starts_with("HTTP/") {
        return None;
    }
    let estado = partes.next()?.parse::<u16>().ok()?;
    let cabeceras: Vec<(String, String)> = lineas
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let resto = &d[fin + 4..];
    let por = |k: &str| cabeceras.iter().find(|(c, _)| c.eq_ignore_ascii_case(k)).map(|(_, v)| v.clone());
    let cuerpo = if por("transfer-encoding").is_some_and(|t| t.to_ascii_lowercase().contains("chunked")) {
        sin_pedazos(resto).unwrap_or_else(|| resto.to_vec())
    } else if let Some(n) = por("content-length").and_then(|n| n.parse::<usize>().ok()) {
        resto[..n.min(resto.len())].to_vec()
    } else {
        resto.to_vec()
    };
    Some(Respuesta { estado, cabeceras, cuerpo })
}

/// ¿Ya llegó todo? (para no esperar al cierre si el aparato deja la conexión abierta).
fn completa(d: &[u8]) -> bool {
    let Some(fin) = buscar(d, b"\r\n\r\n") else { return false };
    let cabeza = String::from_utf8_lossy(&d[..fin]).to_ascii_lowercase();
    let resto = &d[fin + 4..];
    if cabeza.contains("transfer-encoding: chunked") {
        return sin_pedazos(resto).is_some();
    }
    match cabeza.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|n| n.trim().parse::<usize>().ok()) {
        Some(n) => resto.len() >= n,
        None => false,
    }
}

/// Una petición a `ip:puerto`. Error en palabras si no contesta o no habla HTTP.
pub fn pedir(
    ip: Ipv4Addr,
    puerto: u16,
    metodo: &str,
    ruta: &str,
    cabeceras: &[(&str, String)],
    cuerpo: Option<&[u8]>,
    plazo: Duration,
) -> Result<Respuesta, String> {
    let no_responde = || format!("No responde en {ip}.");
    let limite = Instant::now() + plazo;
    let mut s = TcpStream::connect_timeout(&SocketAddr::from((ip, puerto)), plazo).map_err(|_| no_responde())?;
    let _ = s.set_nodelay(true);
    let _ = s.set_write_timeout(Some(plazo));
    s.write_all(&armar(ip, puerto, metodo, ruta, cabeceras, cuerpo)).map_err(|_| no_responde())?;
    let mut datos = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let queda = limite.saturating_duration_since(Instant::now());
        if queda.is_zero() {
            break;
        }
        if s.set_read_timeout(Some(queda.max(Duration::from_millis(5)))).is_err() {
            break;
        }
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                datos.extend_from_slice(&buf[..n]);
                if datos.len() > MAX {
                    return Err("La respuesta es demasiado grande.".into());
                }
                if completa(&datos) {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    leer(&datos).ok_or_else(no_responde)
}

/// GET que devuelve JSON (2xx). Con 401, el error dice que falta la clave.
pub fn json(ip: Ipv4Addr, puerto: u16, ruta: &str, cabeceras: &[(&str, String)], plazo: Duration) -> Result<Value, String> {
    let r = pedir(ip, puerto, "GET", ruta, cabeceras, None, plazo)?;
    if r.estado == 401 || r.estado == 403 {
        return Err(CLAVE.into());
    }
    if !r.ok() {
        return Err(format!("Contestó con un error ({}).", r.estado));
    }
    r.json().ok_or_else(|| "No contestó lo esperado.".into())
}

/// El error de una clave que falta o no sirve (quien llama lo reconoce para pedirla de nuevo).
pub const CLAVE: &str = "La contraseña no es correcta.";

// ---------------------------------------------------------------- Claves

/// Base64 estándar (RFC 4648), para Basic.
pub fn base64(d: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(d.len().div_ceil(3) * 4);
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

/// `Authorization: Basic …`
pub fn basic(usuario: &str, clave: &str) -> String {
    format!("Basic {}", base64(format!("{usuario}:{clave}").as_bytes()))
}

/// Lo que pide un `WWW-Authenticate: Digest …`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Reto {
    pub realm: String,
    pub nonce: String,
    pub qop: Option<String>,
    pub algoritmo: String,
    pub opaque: Option<String>,
}

/// Lee `Digest realm="…", nonce="…", qop="auth", algorithm=SHA-256`.
pub fn reto(cabecera: &str) -> Option<Reto> {
    let resto = cabecera.trim().strip_prefix("Digest").or_else(|| cabecera.trim().strip_prefix("digest"))?;
    let mut r = Reto::default();
    let mut texto = resto.trim();
    while !texto.is_empty() {
        let (clave, tras) = texto.split_once('=')?;
        let clave = clave.trim().trim_start_matches(',').trim().to_ascii_lowercase();
        let tras = tras.trim_start();
        let (valor, siguiente) = if let Some(t) = tras.strip_prefix('"') {
            let fin = t.find('"')?;
            (&t[..fin], t[fin + 1..].trim_start().trim_start_matches(',').trim_start())
        } else {
            match tras.find(',') {
                Some(i) => (tras[..i].trim(), tras[i + 1..].trim_start()),
                None => (tras.trim(), ""),
            }
        };
        match clave.as_str() {
            "realm" => r.realm = valor.to_string(),
            "nonce" => r.nonce = valor.to_string(),
            "qop" => r.qop = Some(valor.to_string()),
            "algorithm" => r.algoritmo = valor.to_string(),
            "opaque" => r.opaque = Some(valor.to_string()),
            _ => {}
        }
        texto = siguiente;
    }
    (!r.nonce.is_empty()).then_some(r)
}

fn sha(t: &str) -> String {
    hex(&Sha256::digest(t.as_bytes()))
}

/// La respuesta de Digest con SHA-256 (RFC 7616, qop=auth), como pide Shelly.
pub fn respuesta_digest(usuario: &str, clave: &str, r: &Reto, metodo: &str, uri: &str, nc: u32, cnonce: &str) -> String {
    let ha1 = sha(&format!("{usuario}:{}:{clave}", r.realm));
    let ha2 = sha(&format!("{metodo}:{uri}"));
    sha(&format!("{ha1}:{}:{nc:08x}:{cnonce}:auth:{ha2}", r.nonce))
}

/// `Authorization: Digest …` completo.
pub fn digest(usuario: &str, clave: &str, r: &Reto, metodo: &str, uri: &str, nc: u32) -> String {
    let semilla = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let cnonce = sha(&format!("{semilla}:{}", r.nonce))[..16].to_string();
    let respuesta = respuesta_digest(usuario, clave, r, metodo, uri, nc, &cnonce);
    let mut t = format!(
        "Digest username=\"{usuario}\", realm=\"{}\", nonce=\"{}\", uri=\"{uri}\", algorithm=SHA-256, response=\"{respuesta}\", qop=auth, nc={nc:08x}, cnonce=\"{cnonce}\"",
        r.realm, r.nonce
    );
    if let Some(o) = &r.opaque {
        t.push_str(&format!(", opaque=\"{o}\""));
    }
    t
}

/// Pone `%XX` a lo que no va tal cual en una consulta (`Power On` → `Power%20On`).
pub fn codificar(t: &str) -> String {
    let mut s = String::with_capacity(t.len() * 3);
    for b in t.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lan_web_arma_la_peticion() {
        let ip = Ipv4Addr::new(192, 168, 1, 30);
        let p = armar(ip, 80, "POST", "/rpc", &[("Authorization", "x\r\nMala: 1".into())], Some(br#"{"id":1}"#));
        let t = String::from_utf8(p).unwrap_or_default();
        assert!(t.starts_with("POST /rpc HTTP/1.1\r\nHost: 192.168.1.30\r\nConnection: close\r\n"));
        // Un salto de línea en un valor no abre otra cabecera.
        assert!(t.contains("Authorization: xMala: 1\r\n"));
        assert!(t.contains("Content-Type: application/json\r\nContent-Length: 8\r\n\r\n{\"id\":1}"));
        let g = String::from_utf8(armar(ip, 9123, "GET", "/elgato/lights", &[], None)).unwrap_or_default();
        assert!(g.contains("Host: 192.168.1.30:9123\r\n") && !g.contains("Content-Length"));
    }

    #[test]
    fn lan_web_lee_respuestas() {
        let r = leer(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"on\":true}extra")
            .unwrap_or_else(|| panic!("sin respuesta"));
        assert_eq!((r.estado, r.cuerpo.as_slice()), (200, &b"{\"on\":true}"[..]));
        assert_eq!(r.json().and_then(|v| v["on"].as_bool()), Some(true));
        let p = leer(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n3\r\n:1}\r\n0\r\n\r\n")
            .unwrap_or_else(|| panic!("sin respuesta"));
        assert_eq!(p.cuerpo, b"{\"a\":1}");
        assert!(completa(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n"));
        assert!(!completa(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nab"));
        let n = leer(b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Digest qop=\"auth\", realm=\"shellypro4pm-f008d1d8b8b8\", nonce=\"60dc59c6\", algorithm=SHA-256\r\nContent-Length: 0\r\n\r\n")
            .unwrap_or_else(|| panic!("sin respuesta"));
        assert_eq!(n.estado, 401);
        let reto = reto(n.cabecera("www-authenticate").unwrap_or("")).unwrap_or_default();
        assert_eq!(reto.realm, "shellypro4pm-f008d1d8b8b8");
        assert_eq!((reto.nonce.as_str(), reto.algoritmo.as_str(), reto.qop.as_deref()), ("60dc59c6", "SHA-256", Some("auth")));
        assert_eq!(leer(b"garbage"), None);
    }

    #[test]
    fn lan_web_claves() {
        // RFC 7617: "Aladdin:open sesame".
        assert_eq!(basic("Aladdin", "open sesame"), "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==");
        assert_eq!(base64(b"ab"), "YWI=");
        // Digest SHA-256 de Shelly: ha1 = sha256(admin:realm:clave), ha2 = sha256(POST:/rpc),
        // response = sha256(ha1:nonce:00000001:cnonce:auth:ha2). Valor calculado aparte con hashlib.
        let r = Reto { realm: "shellypro4pm-f008d1d8b8b8".into(), nonce: "60dc59c6".into(), qop: Some("auth".into()), algoritmo: "SHA-256".into(), opaque: None };
        assert_eq!(
            respuesta_digest("admin", "mypass", &r, "POST", "/rpc", 1, "313273957"),
            "0461778af550d4d2b0cf6b6a8b37eef8d2272adf1f8329ef62f806a3f72f2f6b"
        );
        let cabecera = digest("admin", "mypass", &r, "GET", "/rpc/Shelly.GetStatus", 2);
        assert!(cabecera.starts_with("Digest username=\"admin\", realm=\"shellypro4pm-f008d1d8b8b8\", nonce=\"60dc59c6\", uri=\"/rpc/Shelly.GetStatus\", algorithm=SHA-256"));
        assert!(cabecera.contains("nc=00000002"));
        assert_eq!(codificar("Status 0"), "Status%200");
        assert_eq!(codificar("Backlog Power On;Dimmer 40"), "Backlog%20Power%20On%3BDimmer%2040");
    }
}
