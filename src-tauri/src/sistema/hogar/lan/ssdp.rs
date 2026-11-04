//! Teles por SSDP (UPnP): Roku, Samsung, LG y lo que habla DIAL (Google TV, Android TV, Fire TV…). Solo se
//! detectan: manejarlas llega más adelante.
//!
//! - Se manda `M-SEARCH` a 239.255.255.250:1900 solo por los tipos de tele (así no aparecen routers ni impresoras).
//! - Si queda tiempo, se lee la descripción de cada una (su `LOCATION`, por HTTP en la red local, con plazo corto)
//!   para su nombre, marca y modelo.

use super::super::modelo::{Capacidades, Clase, Hallazgo, Manejo, Proveedor, Via};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

const MULTIDIFUSION: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const PUERTO: u16 = 1900;

pub const OBJETIVOS: [&str; 4] = [
    "roku:ecp",
    "urn:dial-multiscreen-org:service:dial:1",
    "urn:samsung.com:device:RemoteControlReceiver:1",
    "urn:lge-com:service:webos-second-screen:1",
];

pub fn busqueda(st: &str) -> String {
    format!("M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: {st}\r\n\r\n")
}

/// Lo que dijo una tele.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Respuesta {
    pub st: String,
    pub usn: String,
    pub servidor: String,
    pub ubicacion: Option<String>,
}

pub fn respuesta(texto: &str) -> Option<Respuesta> {
    if !texto.starts_with("HTTP/1.1 200") {
        return None;
    }
    let mut r = Respuesta::default();
    for l in texto.lines().skip(1) {
        let Some((k, v)) = l.split_once(':') else { continue };
        let v = v.trim().to_string();
        match k.trim().to_ascii_lowercase().as_str() {
            "st" => r.st = v,
            "usn" => r.usn = v,
            "server" => r.servidor = v,
            "location" => r.ubicacion = Some(v),
            _ => {}
        }
    }
    (!r.usn.is_empty()).then_some(r)
}

/// El identificador estable: lo que va tras "uuid:" y antes de "::".
pub fn uuid_de(usn: &str) -> String {
    let sin = usn.strip_prefix("uuid:").unwrap_or(usn);
    sin.split("::").next().unwrap_or(sin).chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ':').take(64).collect()
}

/// La marca por lo que pidió y quién contestó. Vacío: una tele o un aparato DIAL sin marca conocida.
pub fn marca_de(r: &Respuesta) -> &'static str {
    let s = r.servidor.to_lowercase();
    if r.st == OBJETIVOS[0] || s.contains("roku") {
        "Roku"
    } else if r.st == OBJETIVOS[2] || s.contains("samsung") {
        "Samsung"
    } else if r.st == OBJETIVOS[3] || s.contains("webos") || s.contains("lge") {
        "LG"
    } else {
        ""
    }
}

/// Lo que dice su descripción (UPnP device description).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Descripcion {
    pub nombre: Option<String>,
    pub fabricante: Option<String>,
    pub modelo: Option<String>,
}

fn etiqueta(xml: &str, nombre: &str) -> Option<String> {
    let abre = format!("<{nombre}>");
    let i = xml.find(&abre)? + abre.len();
    let j = xml[i..].find("</")? + i;
    let t: String = xml[i..j].chars().filter(|c| !c.is_control()).take(60).collect();
    let t = t.replace("&amp;", "&").replace("&apos;", "'").replace("&quot;", "\"");
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

pub fn descripcion(xml: &str) -> Descripcion {
    Descripcion {
        nombre: etiqueta(xml, "friendlyName"),
        fabricante: etiqueta(xml, "manufacturer"),
        modelo: etiqueta(xml, "modelName"),
    }
}

/// `http://192.168.1.9:8060/dial/dd.xml` → (IP, puerto, ruta). Solo si es la misma IP que contestó.
fn partes(ubicacion: &str, ip: Ipv4Addr) -> Option<(SocketAddr, String, String)> {
    let resto = ubicacion.strip_prefix("http://")?;
    let (anfitrion, ruta) = resto.split_once('/').map_or((resto, "/".to_string()), |(a, r)| (a, format!("/{r}")));
    let (host, puerto) = anfitrion.rsplit_once(':').map_or((anfitrion, Some(80)), |(h, p)| (h, p.parse::<u16>().ok()));
    let destino: Ipv4Addr = host.parse().ok()?;
    if destino != ip || !(destino.is_private() || destino.is_link_local()) || ruta.len() > 200 {
        return None;
    }
    Some((SocketAddr::from((destino, puerto?)), anfitrion.to_string(), ruta))
}

/// Lee la descripción por HTTP, con todo dentro de `fin`.
fn pedir_descripcion(ubicacion: &str, ip: Ipv4Addr, fin: Instant) -> Option<Descripcion> {
    let (destino, anfitrion, ruta) = partes(ubicacion, ip)?;
    let queda = fin.saturating_duration_since(Instant::now());
    if queda < Duration::from_millis(100) {
        return None;
    }
    let mut tcp = TcpStream::connect_timeout(&destino, queda.min(Duration::from_millis(700))).ok()?;
    let _ = tcp.set_write_timeout(Some(Duration::from_millis(300)));
    tcp.write_all(format!("GET {ruta} HTTP/1.1\r\nHost: {anfitrion}\r\nConnection: close\r\n\r\n").as_bytes()).ok()?;
    let mut cuerpo = Vec::new();
    let mut buf = [0u8; 4096];
    while cuerpo.len() < 64 * 1024 {
        let queda = fin.saturating_duration_since(Instant::now());
        if queda.is_zero() {
            break;
        }
        let _ = tcp.set_read_timeout(Some(queda.max(Duration::from_millis(1))));
        match tcp.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => cuerpo.extend_from_slice(&buf[..n]),
        }
    }
    Some(descripcion(&String::from_utf8_lossy(&cuerpo)))
}

/// Algo que solo suena (un parlante con DIAL), no una tele.
fn es_parlante(d: &Descripcion) -> bool {
    let t = format!("{} {}", d.modelo.clone().unwrap_or_default(), d.nombre.clone().unwrap_or_default()).to_lowercase();
    ["speaker", "audio", "sonos", "nest mini", "home mini", "soundbar", "barra de sonido"].iter().any(|x| t.contains(x))
}

pub fn hallazgo(r: &Respuesta, ip: Ipv4Addr, d: Option<&Descripcion>) -> Hallazgo {
    let marca = match marca_de(r) {
        "" => d.and_then(|d| d.fabricante.clone()).unwrap_or_default(),
        m => m.to_string(),
    };
    let parlante = d.is_some_and(es_parlante);
    let nombre = d.and_then(|d| d.nombre.clone()).unwrap_or_else(|| {
        if marca.is_empty() { "Tele".to_string() } else { format!("Tele {marca}") }
    });
    let motivo = if parlante {
        "Es un parlante. Por ahora Nexo solo lo muestra."
    } else {
        "Es una tele. Pronto podrás manejarla desde Nexo."
    };
    Hallazgo {
        id: format!("ssdp:{}", uuid_de(&r.usn)),
        via: Via::Wifi,
        proveedor: Proveedor::Ninguno,
        clase: if parlante { Clase::Otro } else { Clase::Tv },
        forma: None,
        marca,
        nombre,
        modelo: d.and_then(|d| d.modelo.clone()),
        direccion: Some(ip.to_string()),
        senal: None,
        version: None,
        capacidades: Capacidades::ninguna(),
        estado: None,
        manejo: Manejo::Pronto { motivo: motivo.into() },
    }
}

pub fn buscar(fin: Instant) -> Vec<Hallazgo> {
    let Ok(s) = super::abrir_udp(0) else { return Vec::new() };
    let _ = s.set_multicast_ttl_v4(2);
    let destino = SocketAddr::from((MULTIDIFUSION, PUERTO));
    // Se escucha hasta 0,9 s antes del final; ese rato queda para leer las descripciones.
    let fin_busqueda = fin.checked_sub(Duration::from_millis(900)).filter(|t| *t > Instant::now()).unwrap_or(fin);
    let mut vistas: Vec<(Respuesta, Ipv4Addr)> = Vec::new();
    let mut envios = [Instant::now(), Instant::now() + Duration::from_millis(1500)].into_iter().peekable();
    while Instant::now() < fin_busqueda {
        if envios.peek().is_some_and(|t| Instant::now() >= *t) {
            envios.next();
            for st in OBJETIVOS {
                let _ = s.send_to(busqueda(st).as_bytes(), destino);
            }
        }
        let hasta = envios.peek().copied().unwrap_or(fin_busqueda).min(fin_busqueda);
        super::recibir(&s, hasta, 200, |datos, origen| {
            let IpAddr::V4(ip) = origen.ip() else { return };
            if !super::es_local(origen.ip()) {
                return;
            }
            if let Some(r) = std::str::from_utf8(datos).ok().and_then(respuesta) {
                let id = uuid_de(&r.usn);
                // Una sola por aparato (contesta a varios tipos) y por IP.
                match vistas.iter().position(|(x, i)| uuid_de(&x.usn) == id || *i == ip) {
                    Some(i) => {
                        // Si antes contestó sin marca (DIAL) y ahora con ella (Roku, Samsung, LG), queda la de marca.
                        if marca_de(&vistas[i].0).is_empty() && !marca_de(&r).is_empty() {
                            vistas[i].0 = r;
                        }
                    }
                    None if vistas.len() < 16 => vistas.push((r, ip)),
                    None => {}
                }
            }
        });
    }
    drop(s);
    // Las descripciones, todas a la vez y dentro del plazo.
    let descripciones: Vec<Option<Descripcion>> = std::thread::scope(|ambito| {
        let hilos: Vec<_> = vistas
            .iter()
            .take(8)
            .map(|(r, ip)| {
                let ubicacion = r.ubicacion.clone();
                let ip = *ip;
                ambito.spawn(move || ubicacion.and_then(|u| pedir_descripcion(&u, ip, fin)))
            })
            .collect();
        hilos.into_iter().map(|h| h.join().ok().flatten()).collect()
    });
    vistas
        .iter()
        .enumerate()
        .map(|(i, (r, ip))| hallazgo(r, *ip, descripciones.get(i).and_then(Option::as_ref)))
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lan_ssdp_arma_la_busqueda() {
        assert_eq!(
            busqueda("roku:ecp"),
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: roku:ecp\r\n\r\n"
        );
    }

    #[test]
    fn lan_ssdp_lee_las_teles() {
        let roku = respuesta(
            "HTTP/1.1 200 OK\r\nCache-Control: max-age=3600\r\nST: roku:ecp\r\nLocation: http://192.168.1.134:8060/\r\nUSN: uuid:roku:ecp:P0A070000007\r\nServer: Roku/9.3.0 UPnP/1.0 Roku/9.3.0\r\n\r\n",
        )
        .unwrap_or_else(|| panic!("sin respuesta"));
        assert_eq!(marca_de(&roku), "Roku");
        assert_eq!(uuid_de(&roku.usn), "roku:ecp:P0A070000007");
        let h = hallazgo(&roku, Ipv4Addr::new(192, 168, 1, 134), None);
        assert_eq!((h.clase, h.nombre.as_str()), (Clase::Tv, "Tele Roku"));
        assert_eq!(h.id, "ssdp:roku:ecp:P0A070000007");

        let lg = respuesta(
            "HTTP/1.1 200 OK\r\nST: urn:lge-com:service:webos-second-screen:1\r\nUSN: uuid:a1b2c3d4-0000-1111-2222-334455667788::urn:lge-com:service:webos-second-screen:1\r\nSERVER: WebOS/4.1.0 UPnP/1.0\r\nLOCATION: http://192.168.1.60:1414/\r\n\r\n",
        )
        .unwrap_or_else(|| panic!("sin respuesta"));
        assert_eq!(marca_de(&lg), "LG");
        assert_eq!(uuid_de(&lg.usn), "a1b2c3d4-0000-1111-2222-334455667788");
        assert_eq!(respuesta("NOTIFY * HTTP/1.1\r\n"), None);
    }

    #[test]
    fn lan_ssdp_lee_la_descripcion() {
        let xml = "<?xml version=\"1.0\"?><root><device><friendlyName>[TV] Samsung Q60 Series (55)</friendlyName><manufacturer>Samsung Electronics</manufacturer><modelName>QN55Q60BA</modelName></device></root>";
        let d = descripcion(xml);
        assert_eq!(d.nombre.as_deref(), Some("[TV] Samsung Q60 Series (55)"));
        assert_eq!(d.modelo.as_deref(), Some("QN55Q60BA"));
        let r = Respuesta { st: OBJETIVOS[1].into(), usn: "uuid:x".into(), servidor: "Linux".into(), ubicacion: None };
        let h = hallazgo(&r, Ipv4Addr::new(192, 168, 1, 7), Some(&d));
        assert_eq!(h.marca, "Samsung Electronics");
        assert_eq!(h.clase, Clase::Tv);
        // Un parlante con DIAL no es una tele.
        let p = Descripcion { nombre: Some("Cocina".into()), fabricante: Some("Sonos, Inc.".into()), modelo: Some("Sonos One".into()) };
        assert_eq!(hallazgo(&r, Ipv4Addr::new(192, 168, 1, 7), Some(&p)).clase, Clase::Otro);
    }

    #[test]
    fn lan_ssdp_solo_lee_descripciones_de_la_misma_ip() {
        let ip = Ipv4Addr::new(192, 168, 1, 9);
        let ok = partes("http://192.168.1.9:8060/dial/dd.xml", ip);
        assert_eq!(ok.map(|(d, _, r)| (d.port(), r)), Some((8060, "/dial/dd.xml".to_string())));
        assert!(partes("http://192.168.1.10:8060/", ip).is_none());
        assert!(partes("http://8.8.8.8/", Ipv4Addr::new(8, 8, 8, 8)).is_none());
        assert!(partes("https://192.168.1.9/", ip).is_none());
        assert_eq!(partes("http://192.168.1.9", ip).map(|(d, _, r)| (d.port(), r)), Some((80, "/".to_string())));
    }
}
