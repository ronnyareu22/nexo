//! Tailscale: si está conectado y cómo se llama cada equipo de la red ("innotech1017").
//! Se le pregunta a su programa (`tailscale status --json`) con un límite de 4 s, y lo leído se recuerda
//! un rato para no abrirlo seguido. Si no está instalado, se dice así.

// En Windows solo se usa el estado (los nombres sirven en la Mac, para decir desde qué PC entra la sesión).
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct Info {
    /// "activo", "detenido" o "no instalado".
    pub estado: &'static str,
    /// (dirección, nombre) de cada equipo de la red.
    nombres: Vec<(String, String)>,
}

impl Info {
    /// El nombre del equipo que tiene esa dirección de Tailscale.
    pub fn nombre_de(&self, ip: &str) -> Option<String> {
        self.nombres.iter().find(|(d, _)| d == ip).map(|(_, n)| n.clone())
    }
}

static MEMORIA: Mutex<Option<(Instant, Info)>> = Mutex::new(None);

/// Lo que dice Tailscale. Si lo último leído tiene menos de `max_edad`, no vuelve a preguntar (con cero, siempre).
pub fn leer(max_edad: Duration) -> Info {
    if !max_edad.is_zero() {
        if let Ok(m) = MEMORIA.lock() {
            if let Some((cuando, info)) = m.as_ref() {
                if cuando.elapsed() <= max_edad {
                    return info.clone();
                }
            }
        }
    }
    let info = consultar();
    if let Ok(mut m) = MEMORIA.lock() {
        *m = Some((Instant::now(), info.clone()));
    }
    info
}

fn consultar() -> Info {
    let Some(programa) = programa() else {
        return Info { estado: "no instalado", nombres: Vec::new() };
    };
    super::ejecutar(&programa, &["status", "--json"], Duration::from_secs(4))
        .and_then(|salida| analizar(&salida))
        .unwrap_or(Info { estado: "detenido", nombres: Vec::new() })
}

/// El programa de Tailscale, si está instalado.
pub fn programa() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let candidatos: Vec<PathBuf> = [
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
        "/usr/local/bin/tailscale",
        "/opt/homebrew/bin/tailscale",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    #[cfg(windows)]
    let candidatos: Vec<PathBuf> = {
        let mut v = Vec::new();
        if let Some(programas) = std::env::var_os("ProgramFiles") {
            v.push(PathBuf::from(programas).join("Tailscale").join("tailscale.exe"));
        }
        v.push(PathBuf::from(r"C:\Program Files\Tailscale\tailscale.exe"));
        v
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let candidatos: Vec<PathBuf> = Vec::new();
    candidatos.into_iter().find(|p| p.is_file())
}

/// Mac: si Tailscale quedó desconectado, `tailscale up` lo vuelve a conectar (espera hasta 8 s).
#[cfg(target_os = "macos")]
pub fn conectar() {
    if let Some(p) = programa() {
        let _ = super::ejecutar(&p, &["up"], Duration::from_secs(8));
    }
}

fn analizar(json: &[u8]) -> Option<Info> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    let estado = match v.get("BackendState")?.as_str()? {
        "Running" => "activo",
        _ => "detenido",
    };
    let mut nombres = Vec::new();
    if let Some(pares) = v.get("Peer").and_then(|p| p.as_object()) {
        for p in pares.values() {
            // "innotech1017.tail1234.ts.net." → "innotech1017"; si no hay, el nombre del equipo.
            let dns = p.get("DNSName").and_then(|d| d.as_str()).and_then(|d| d.split('.').next()).filter(|d| !d.is_empty());
            let host = p.get("HostName").and_then(|h| h.as_str()).filter(|h| !h.is_empty());
            let Some(nombre) = dns.or(host) else { continue };
            for ip in p.get("TailscaleIPs").and_then(|i| i.as_array()).into_iter().flatten() {
                if let Some(ip) = ip.as_str() {
                    nombres.push((ip.to_string(), nombre.to_string()));
                }
            }
        }
    }
    Some(Info { estado, nombres })
}
