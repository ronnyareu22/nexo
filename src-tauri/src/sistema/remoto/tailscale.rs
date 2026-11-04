//! Tailscale: si está conectado y cómo se llama cada equipo de la red ("innotech1017").
//! Se le pregunta a su programa (`tailscale status --json`) con un límite de 4 s, y lo leído se recuerda
//! un rato para no abrirlo seguido. Si no está instalado, se dice así.
//! Nexo Drop usa además este equipo (sus direcciones y su dueño) y los demás equipos del mismo dueño.

// En Windows solo se usa el estado (los nombres sirven en la Mac, para decir desde qué PC entra la sesión).
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Un equipo de la red de Tailscale.
#[derive(Clone, Debug, Default)]
pub struct Nodo {
    /// "innotech1017" (la primera parte del nombre DNS) o, si no hay, el nombre del equipo.
    pub nombre: String,
    pub ips: Vec<String>,
    /// El dueño en Tailscale. Los equipos compartidos por otra persona o con etiquetas tienen otro.
    pub usuario: i64,
    /// "macOS", "windows", "linux", "iOS"…
    pub sistema: String,
    pub en_linea: bool,
}

impl Nodo {
    /// Su dirección IPv4 de Tailscale (100.x.y.z).
    pub fn ipv4(&self) -> Option<Ipv4Addr> {
        self.ips.iter().find_map(|i| i.parse::<Ipv4Addr>().ok())
    }
}

#[derive(Clone, Debug)]
pub struct Info {
    /// "activo", "detenido" o "no instalado".
    pub estado: &'static str,
    /// (dirección, nombre) de cada equipo de la red.
    nombres: Vec<(String, String)>,
    /// Este equipo, si Tailscale lo informó.
    pub propio: Option<Nodo>,
    /// Los demás equipos de la red.
    pub pares: Vec<Nodo>,
}

impl Info {
    /// El nombre del equipo que tiene esa dirección de Tailscale.
    pub fn nombre_de(&self, ip: &str) -> Option<String> {
        self.nombres.iter().find(|(d, _)| d == ip).map(|(_, n)| n.clone())
    }

    /// Si esa dirección es de este equipo o de otro equipo del mismo dueño. `None` si no se conoce.
    pub fn es_del_dueno(&self, ip: &str) -> Option<bool> {
        let propio = self.propio.as_ref()?;
        if propio.ips.iter().any(|i| i == ip) {
            return Some(true);
        }
        let par = self.pares.iter().find(|p| p.ips.iter().any(|i| i == ip))?;
        Some(propio.usuario != 0 && par.usuario == propio.usuario)
    }

    /// Los demás equipos del mismo dueño.
    pub fn pares_del_dueno(&self) -> Vec<Nodo> {
        let Some(propio) = &self.propio else { return Vec::new() };
        if propio.usuario == 0 {
            return Vec::new();
        }
        self.pares.iter().filter(|p| p.usuario == propio.usuario).cloned().collect()
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
        return Info { estado: "no instalado", nombres: Vec::new(), propio: None, pares: Vec::new() };
    };
    super::ejecutar(&programa, &["status", "--json"], Duration::from_secs(4))
        .and_then(|salida| analizar(&salida))
        .unwrap_or(Info { estado: "detenido", nombres: Vec::new(), propio: None, pares: Vec::new() })
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
    let mut pares = Vec::new();
    if let Some(lista) = v.get("Peer").and_then(|p| p.as_object()) {
        for p in lista.values() {
            let Some(nodo) = nodo(p) else { continue };
            for ip in &nodo.ips {
                nombres.push((ip.clone(), nodo.nombre.clone()));
            }
            pares.push(nodo);
        }
    }
    let propio = v.get("Self").and_then(nodo);
    Some(Info { estado, nombres, propio, pares })
}

/// Un equipo de la lista de Tailscale. `None` si no tiene nombre.
fn nodo(p: &serde_json::Value) -> Option<Nodo> {
    // "innotech1017.tail1234.ts.net." → "innotech1017"; si no hay, el nombre del equipo.
    let dns = p.get("DNSName").and_then(|d| d.as_str()).and_then(|d| d.split('.').next()).filter(|d| !d.is_empty());
    let host = p.get("HostName").and_then(|h| h.as_str()).filter(|h| !h.is_empty());
    let nombre = dns.or(host)?.to_string();
    let ips = p
        .get("TailscaleIPs")
        .and_then(|i| i.as_array())
        .into_iter()
        .flatten()
        .filter_map(|ip| ip.as_str().map(str::to_string))
        .collect();
    Some(Nodo {
        nombre,
        ips,
        usuario: p.get("UserID").and_then(|u| u.as_i64()).unwrap_or(0),
        sistema: p.get("OS").and_then(|o| o.as_str()).unwrap_or_default().to_string(),
        en_linea: p.get("Online").and_then(|o| o.as_bool()).unwrap_or(false),
    })
}
