//! Una sola pregunta mDNS (RFC 6762 y 6763) para ver qué más hay en la red: Home Assistant (con su dirección, para
//! vincularlo), puentes Hue, teles (Google Cast, Android TV, AirPlay), HomeKit, Matter, Shelly y ESPHome. Fuera de
//! Home Assistant, nada se maneja todavía: se muestra con el motivo en palabras.
//!
//! - Se manda una consulta PTR con todos los servicios a 224.0.0.251:5353 desde un puerto cualquiera: así cada
//!   aparato contesta directo a este equipo ("consulta de una sola vez", RFC 6762 §5.1 y §6.7). No se escucha el
//!   5353 ni queda nada anunciado.
//! - De cada respuesta se leen los PTR (servicio → instancia), SRV (instancia → equipo), TXT (modelo y nombre) y A.

use super::super::modelo::{Capacidades, Clase, Hallazgo, Manejo, Proveedor, Via, normalizar};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

const MULTIDIFUSION: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const PUERTO: u16 = 5353;

pub const SERVICIOS: [&str; 9] = [
    "_home-assistant._tcp.local",
    "_hue._tcp.local",
    "_googlecast._tcp.local",
    "_androidtvremote2._tcp.local",
    "_airplay._tcp.local",
    "_hap._tcp.local",
    "_matter._tcp.local",
    "_shelly._tcp.local",
    "_esphomelib._tcp.local",
];

const TIPO_A: u16 = 1;
const TIPO_PTR: u16 = 12;
const TIPO_TXT: u16 = 16;
const TIPO_SRV: u16 = 33;

/// La consulta: una pregunta PTR por servicio, pidiendo respuesta directa (bit QU).
pub fn consulta(servicios: &[&str]) -> Vec<u8> {
    let mut p = vec![0, 0, 0, 0];
    p.extend_from_slice(&(servicios.len() as u16).to_be_bytes());
    p.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    for s in servicios {
        for etiqueta in s.split('.') {
            p.push(etiqueta.len() as u8);
            p.extend_from_slice(etiqueta.as_bytes());
        }
        p.push(0);
        p.extend_from_slice(&TIPO_PTR.to_be_bytes());
        p.extend_from_slice(&0x8001u16.to_be_bytes());
    }
    p
}

/// Un registro de una respuesta. Los nombres van por etiquetas (una instancia puede tener puntos en su nombre).
#[derive(Debug, Clone, PartialEq)]
pub enum Registro {
    Ptr { nombre: Vec<String>, destino: Vec<String> },
    Srv { nombre: Vec<String>, equipo: Vec<String>, puerto: u16 },
    Txt { nombre: Vec<String>, pares: Vec<(String, String)> },
    A { nombre: Vec<String>, ip: Ipv4Addr },
}

fn u16_be(d: &[u8], i: usize) -> Option<u16> {
    d.get(i..i + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}

/// Un nombre desde `pos`, siguiendo los punteros de compresión (con tope, por si vienen en círculo).
fn nombre(d: &[u8], mut pos: usize) -> Option<(Vec<String>, usize)> {
    let mut etiquetas = Vec::new();
    let mut siguiente = None;
    for _ in 0..64 {
        let largo = *d.get(pos)? as usize;
        if largo == 0 {
            return Some((etiquetas, siguiente.unwrap_or(pos + 1)));
        }
        if largo & 0xC0 == 0xC0 {
            let destino = ((largo & 0x3F) << 8) | *d.get(pos + 1)? as usize;
            siguiente.get_or_insert(pos + 2);
            pos = destino;
            continue;
        }
        let texto = d.get(pos + 1..pos + 1 + largo)?;
        etiquetas.push(String::from_utf8_lossy(texto).into_owned());
        pos += 1 + largo;
    }
    None
}

/// Los pares clave=valor de un TXT.
fn pares(d: &[u8]) -> Vec<(String, String)> {
    let mut v = Vec::new();
    let mut i = 0;
    while let Some(&largo) = d.get(i) {
        let Some(trozo) = d.get(i + 1..i + 1 + largo as usize) else { break };
        let texto = String::from_utf8_lossy(trozo);
        if let Some((k, val)) = texto.split_once('=') {
            v.push((k.to_ascii_lowercase(), val.to_string()));
        }
        i += 1 + largo as usize;
    }
    v
}

/// Los registros de un paquete de respuesta (respuestas, autoridad y adicionales).
pub fn leer(d: &[u8]) -> Option<Vec<Registro>> {
    let preguntas = u16_be(d, 4)? as usize;
    let registros = u16_be(d, 6)? as usize + u16_be(d, 8)? as usize + u16_be(d, 10)? as usize;
    // Las respuestas tienen el bit QR.
    if d.get(2)? & 0x80 == 0 {
        return None;
    }
    let mut pos = 12;
    for _ in 0..preguntas {
        pos = nombre(d, pos)?.1 + 4;
    }
    let mut v = Vec::new();
    for _ in 0..registros.min(200) {
        let (dueno, p) = nombre(d, pos)?;
        let tipo = u16_be(d, p)?;
        let largo = u16_be(d, p + 8)? as usize;
        let datos = p + 10;
        let cuerpo = d.get(datos..datos + largo)?;
        match tipo {
            TIPO_PTR => {
                if let Some((destino, _)) = nombre(d, datos) {
                    v.push(Registro::Ptr { nombre: dueno, destino });
                }
            }
            TIPO_SRV if largo >= 7 => {
                if let Some((equipo, _)) = nombre(d, datos + 6) {
                    v.push(Registro::Srv { nombre: dueno, equipo, puerto: u16_be(d, datos + 4).unwrap_or(0) });
                }
            }
            TIPO_TXT => v.push(Registro::Txt { nombre: dueno, pares: pares(cuerpo) }),
            TIPO_A if largo == 4 => v.push(Registro::A { nombre: dueno, ip: Ipv4Addr::new(cuerpo[0], cuerpo[1], cuerpo[2], cuerpo[3]) }),
            _ => {}
        }
        pos = datos + largo;
    }
    Some(v)
}

fn igual(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}

fn etiquetas(s: &str) -> Vec<String> {
    s.split('.').map(str::to_string).collect()
}

/// Una instancia de un servicio, con lo que se juntó de ella.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Instancia {
    pub servicio: String,
    pub nombre: Vec<String>,
    pub txt: Vec<(String, String)>,
    pub equipo: Option<Vec<String>>,
    pub puerto: Option<u16>,
    pub ip: Option<Ipv4Addr>,
}

impl Instancia {
    fn txt(&self, k: &str) -> Option<&str> {
        self.txt.iter().find(|(c, _)| c == k).map(|(_, v)| v.as_str()).filter(|v| !v.is_empty())
    }
    fn etiqueta(&self) -> &str {
        self.nombre.first().map(String::as_str).unwrap_or("")
    }
}

/// Junta registros de varias respuestas en instancias. `origen` es quien mandó cada paquete (si no hay registro A).
pub fn juntar(paquetes: &[(Vec<Registro>, Option<Ipv4Addr>)]) -> Vec<Instancia> {
    let mut v: Vec<Instancia> = Vec::new();
    let todos: Vec<(&Registro, Option<Ipv4Addr>)> = paquetes.iter().flat_map(|(r, o)| r.iter().map(move |x| (x, *o))).collect();
    for (r, origen) in &todos {
        if let Registro::Ptr { nombre, destino } = r
            && let Some(s) = SERVICIOS.iter().find(|s| igual(nombre, &etiquetas(s)))
            && !v.iter().any(|i| igual(&i.nombre, destino))
        {
            v.push(Instancia { servicio: s.to_string(), nombre: destino.clone(), ip: *origen, ..Default::default() });
        }
    }
    for i in v.iter_mut() {
        for (r, _) in &todos {
            match r {
                Registro::Txt { nombre, pares } if igual(nombre, &i.nombre) && i.txt.is_empty() => i.txt = pares.clone(),
                Registro::Srv { nombre, equipo, puerto } if igual(nombre, &i.nombre) => {
                    i.equipo = Some(equipo.clone());
                    i.puerto = Some(*puerto).filter(|p| *p != 0);
                }
                _ => {}
            }
        }
        if let Some(eq) = &i.equipo
            && let Some(ip) = todos.iter().find_map(|(r, _)| match r {
                Registro::A { nombre, ip } if igual(nombre, eq) => Some(*ip),
                _ => None,
            })
        {
            i.ip = Some(ip);
        }
    }
    v
}

/// Un texto que viene de la red, limpio y corto.
fn limpio(t: &str) -> String {
    let s: String = t.chars().filter(|c| !c.is_control()).take(60).collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

const MARCAS_TV: [&str; 10] = ["samsung", "lg", "sony", "vizio", "tcl", "hisense", "roku", "panasonic", "philips", "sharp"];

fn pronto(motivo: &str) -> Manejo {
    Manejo::Pronto { motivo: motivo.into() }
}

const TELE: &str = "Es una tele. Pronto podrás manejarla desde Nexo.";
const PARLANTE: &str = "Es un parlante. Por ahora Nexo solo lo muestra.";
const PRONTO: &str = "Pronto podrás manejarlo desde Nexo.";

/// Lo que se muestra de una instancia, o `None` si no vale la pena (una Mac o un iPhone por AirPlay).
pub fn hallazgo(i: &Instancia) -> Option<Hallazgo> {
    let ip = i.ip?;
    let etiqueta = limpio(i.etiqueta());
    let corto = i.servicio.split('.').next().unwrap_or("").trim_start_matches('_').to_string();
    let base = |clase: Clase, marca: &str, nombre: String, manejo: Manejo| Hallazgo {
        id: format!("mdns:{corto}:{}", normalizar(&etiqueta).replace(' ', "-")),
        via: Via::Wifi,
        proveedor: Proveedor::Ninguno,
        clase,
        forma: None,
        marca: marca.into(),
        nombre,
        modelo: None,
        direccion: Some(ip.to_string()),
        senal: None,
        version: None,
        capacidades: Capacidades::ninguna(),
        estado: None,
        manejo,
    };
    let con_modelo = |mut h: Hallazgo, m: Option<&str>| {
        h.modelo = m.map(limpio).filter(|m| !m.is_empty());
        h
    };
    let h = match corto.as_str() {
        "home-assistant" => return Some(home_assistant(i, ip)),
        "hue" => {
            let mut h = base(Clase::Otro, "Philips Hue", etiqueta.clone(), pronto("Es el puente de Philips Hue. Pronto podrás vincularlo con Nexo."));
            h.proveedor = Proveedor::Hue;
            con_modelo(h, i.txt("modelid"))
        }
        "googlecast" => {
            let md = i.txt("md").unwrap_or("");
            let m = md.to_lowercase();
            let tele = !m.contains("audio") && ["chromecast", "google tv", "tv", "bravia", "shield", "android"].iter().any(|x| m.contains(x));
            let nombre = i.txt("fn").map(limpio).unwrap_or(etiqueta.clone());
            let marca = if m.contains("chromecast") || m.contains("google") || m.contains("nest") { "Google" } else { "Google Cast" };
            let h = if tele { base(Clase::Tv, marca, nombre, pronto(TELE)) } else { base(Clase::Otro, marca, nombre, pronto(PARLANTE)) };
            con_modelo(h, Some(md))
        }
        "androidtvremote2" => base(Clase::Tv, "Android TV", etiqueta.clone(), pronto(TELE)),
        "airplay" => {
            let modelo = i.txt("model").unwrap_or("");
            let fabricante = i.txt("manufacturer").unwrap_or("");
            let m = modelo.to_lowercase();
            let apple_propio = ["mac", "imac", "macbook", "macmini", "macpro", "iphone", "ipad", "ipod", "virtualmac"];
            if apple_propio.iter().any(|x| m.starts_with(x)) {
                return None;
            }
            let h = if m.starts_with("appletv") {
                base(Clase::Tv, "Apple", etiqueta.clone(), pronto(TELE))
            } else if m.starts_with("audioaccessory") {
                base(Clase::Otro, "Apple", etiqueta.clone(), pronto(PARLANTE))
            } else {
                let texto = format!("{} {}", fabricante.to_lowercase(), m);
                match MARCAS_TV.iter().find(|x| texto.split(|c: char| !c.is_alphanumeric()).any(|p| p == **x)) {
                    Some(marca) => {
                        let marca = if fabricante.is_empty() { primera_mayuscula(marca) } else { limpio(fabricante) };
                        base(Clase::Tv, &marca, etiqueta.clone(), pronto(TELE))
                    }
                    None => base(Clase::Otro, &limpio(fabricante), etiqueta.clone(), pronto(PARLANTE)),
                }
            };
            con_modelo(h, Some(modelo))
        }
        "hap" => {
            // Las categorías de HomeKit (HAP-NodeJS, `Categories`).
            let ci = i.txt("ci").and_then(|c| c.parse::<u16>().ok()).unwrap_or(1);
            let clase = match ci {
                5 => Clase::Luz,
                7 | 8 => Clase::Enchufe,
                6 => Clase::Cerradura,
                14 => Clase::Cortina,
                3 | 9 | 19..=23 => Clase::Clima,
                24 | 31 | 35 | 36 => Clase::Tv,
                _ => Clase::Otro,
            };
            let h = base(clase, "HomeKit", etiqueta.clone(), pronto("Es un accesorio de HomeKit. Pronto podrás manejarlo desde Nexo."));
            con_modelo(h, i.txt("md"))
        }
        "matter" => base(Clase::Otro, "Matter", "Aparato Matter".into(), pronto("Es un aparato Matter. Pronto podrás manejarlo desde Nexo.")),
        "shelly" => {
            let n = etiqueta.to_lowercase();
            let clase = if n.contains("plug") {
                Clase::Enchufe
            } else if ["bulb", "rgbw", "dimmer", "duo", "vintage"].iter().any(|x| n.contains(x)) {
                Clase::Luz
            } else {
                Clase::Otro
            };
            con_modelo(base(clase, "Shelly", etiqueta.clone(), pronto(PRONTO)), i.txt("app"))
        }
        "esphomelib" => {
            let nombre = i.txt("friendly_name").map(limpio).unwrap_or(etiqueta.clone());
            base(Clase::Otro, "ESPHome", nombre, pronto(PRONTO))
        }
        _ => return None,
    };
    (!h.nombre.is_empty()).then_some(h)
}

/// Un enlace que se puede guardar: http o https, corto y sin caracteres raros.
fn enlace(t: Option<&str>) -> Option<String> {
    let t = t?.trim().trim_end_matches('/');
    let ok = (t.starts_with("http://") || t.starts_with("https://"))
        && t.len() <= 200
        && !t.chars().any(|c| c.is_control() || c.is_whitespace() || c == '"' || c == '<' || c == '>');
    ok.then(|| t.to_string())
}

/// Home Assistant: su nombre de casa, su versión y por dónde entrar (la dirección interna, la de base o la IP con su
/// puerto, 8123 de fábrica). Siempre se muestra, aunque en esa IP haya otras cosas: es un puente a todo lo demás.
fn home_assistant(i: &Instancia, ip: Ipv4Addr) -> Hallazgo {
    let uuid: Option<String> = i
        .txt("uuid")
        .map(|u| u.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(64).collect::<String>())
        .filter(|u| !u.is_empty());
    let url = enlace(i.txt("internal_url"))
        .or_else(|| enlace(i.txt("base_url")))
        .unwrap_or_else(|| format!("http://{ip}:{}", i.puerto.unwrap_or(8123)));
    Hallazgo {
        id: format!("ha:{}", uuid.unwrap_or_else(|| ip.to_string())),
        via: Via::Wifi,
        proveedor: Proveedor::HomeAssistant,
        clase: Clase::Otro,
        forma: None,
        marca: "Home Assistant".into(),
        nombre: i.txt("location_name").map(limpio).filter(|n| !n.is_empty()).unwrap_or_else(|| "Home Assistant".into()),
        modelo: None,
        direccion: Some(ip.to_string()),
        senal: None,
        version: i.txt("version").map(limpio).filter(|v| !v.is_empty()),
        capacidades: Capacidades::ninguna(),
        estado: None,
        manejo: Manejo::HomeAssistant { url },
    }
}

/// "lg" → "LG", "samsung" → "Samsung".
fn primera_mayuscula(t: &str) -> String {
    if t.len() <= 3 {
        return t.to_uppercase();
    }
    let mut c = t.chars();
    c.next().map(|p| p.to_uppercase().chain(c).collect()).unwrap_or_default()
}

pub fn buscar(fin: Instant) -> Vec<Hallazgo> {
    let Ok(s) = super::abrir_udp(0) else { return Vec::new() };
    let _ = s.set_multicast_ttl_v4(255);
    let destino = SocketAddr::from((MULTIDIFUSION, PUERTO));
    let propia = super::ip_propia();
    let paquete = consulta(&SERVICIOS);
    let mut paquetes: Vec<(Vec<Registro>, Option<Ipv4Addr>)> = Vec::new();
    let mut envios = [Instant::now(), Instant::now() + Duration::from_millis(1200)].into_iter().peekable();
    while Instant::now() < fin {
        if envios.peek().is_some_and(|t| Instant::now() >= *t) {
            envios.next();
            let _ = s.send_to(&paquete, destino);
        }
        let hasta = envios.peek().copied().unwrap_or(fin).min(fin);
        super::recibir(&s, hasta, 200, |datos, origen| {
            let IpAddr::V4(ip) = origen.ip() else { return };
            if !super::es_local(origen.ip()) || Some(origen.ip()) == propia || paquetes.len() >= 300 {
                return;
            }
            if let Some(r) = leer(datos) {
                paquetes.push((r, Some(ip)));
            }
        });
    }
    juntar(&paquetes)
        .iter()
        .filter(|i| i.ip.map(IpAddr::V4) != propia)
        .filter_map(hallazgo)
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Escribe un nombre sin compresión.
    fn n(t: &str) -> Vec<u8> {
        let mut v = Vec::new();
        for e in t.split('.') {
            v.push(e.len() as u8);
            v.extend_from_slice(e.as_bytes());
        }
        v.push(0);
        v
    }

    fn registro(dueno: Vec<u8>, tipo: u16, datos: Vec<u8>) -> Vec<u8> {
        let mut v = dueno;
        v.extend_from_slice(&tipo.to_be_bytes());
        v.extend_from_slice(&1u16.to_be_bytes());
        v.extend_from_slice(&120u32.to_be_bytes());
        v.extend_from_slice(&(datos.len() as u16).to_be_bytes());
        v.extend(datos);
        v
    }

    #[test]
    fn lan_mdns_arma_la_consulta() {
        let c = consulta(&["_hue._tcp.local"]);
        let mut esperado = vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        esperado.extend(n("_hue._tcp.local"));
        esperado.extend_from_slice(&[0, 12, 0x80, 0x01]);
        assert_eq!(c, esperado);
        assert_eq!(u16_be(&consulta(&SERVICIOS), 4), Some(9));
    }

    #[test]
    fn lan_mdns_lee_una_respuesta_de_cast() {
        // Respuesta: PTR del servicio a la instancia (con un puntero), TXT, SRV y A.
        let mut p = vec![0, 0, 0x84, 0, 0, 0, 0, 4, 0, 0, 0, 0];
        let inicio_servicio = p.len();
        let mut instancia = vec![17];
        instancia.extend_from_slice(b"Chromecast-1a2b3c");
        // La instancia apunta al nombre del servicio, que quedó al inicio.
        instancia.extend_from_slice(&[0xC0, inicio_servicio as u8]);
        let ptr = registro(n("_googlecast._tcp.local"), TIPO_PTR, instancia.clone());
        p.extend(ptr);
        let mut txt = Vec::new();
        for par in ["md=Chromecast", "fn=Tele de la sala", "id=abc"] {
            txt.push(par.len() as u8);
            txt.extend_from_slice(par.as_bytes());
        }
        p.extend(registro(instancia.clone(), TIPO_TXT, txt));
        let mut srv = vec![0, 0, 0, 0, 0x1f, 0x49];
        srv.extend(n("abc123.local"));
        p.extend(registro(instancia, TIPO_SRV, srv));
        p.extend(registro(n("abc123.local"), TIPO_A, vec![192, 168, 1, 50]));

        let r = leer(&p).unwrap_or_default();
        assert_eq!(r.len(), 4);
        assert!(matches!(&r[0], Registro::Ptr { destino, .. } if destino[0] == "Chromecast-1a2b3c" && destino.len() == 4));
        assert!(matches!(&r[2], Registro::Srv { puerto: 8009, .. }));
        let inst = juntar(&[(r, Some(Ipv4Addr::new(192, 168, 1, 99)))]);
        assert_eq!(inst.len(), 1);
        // La IP del registro A manda sobre la de quien mandó el paquete.
        assert_eq!(inst[0].ip, Some(Ipv4Addr::new(192, 168, 1, 50)));
        let h = hallazgo(&inst[0]).unwrap_or_else(|| panic!("sin hallazgo"));
        assert_eq!(h.clase, Clase::Tv);
        assert_eq!(h.nombre, "Tele de la sala");
        assert_eq!(h.marca, "Google");
        assert_eq!(h.id, "mdns:googlecast:chromecast-1a2b3c");
        assert!(matches!(h.manejo, Manejo::Pronto { .. }));
    }

    #[test]
    fn lan_mdns_encuentra_home_assistant() {
        let mut p = vec![0, 0, 0x84, 0, 0, 0, 0, 4, 0, 0, 0, 0];
        let instancia = n("Casa Ronny._home-assistant._tcp.local");
        p.extend(registro(n("_home-assistant._tcp.local"), TIPO_PTR, instancia.clone()));
        let mut txt = Vec::new();
        for par in [
            "location_name=Casa Ronny",
            "uuid=0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e",
            "version=2026.10.3",
            "internal_url=http://homeassistant.local:8123/",
            "base_url=http://192.168.1.3:8123",
            "requires_api_password=True",
        ] {
            txt.push(par.len() as u8);
            txt.extend_from_slice(par.as_bytes());
        }
        p.extend(registro(instancia.clone(), TIPO_TXT, txt));
        let mut srv = vec![0, 0, 0, 0];
        srv.extend_from_slice(&8123u16.to_be_bytes());
        srv.extend(n("homeassistant.local"));
        p.extend(registro(instancia, TIPO_SRV, srv));
        p.extend(registro(n("homeassistant.local"), TIPO_A, vec![192, 168, 1, 3]));

        let inst = juntar(&[(leer(&p).unwrap_or_default(), Some(Ipv4Addr::new(192, 168, 1, 3)))]);
        assert_eq!(inst.len(), 1);
        assert_eq!(inst[0].puerto, Some(8123));
        let h = hallazgo(&inst[0]).unwrap_or_else(|| panic!("sin hallazgo"));
        assert_eq!(h.id, "ha:0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e");
        assert_eq!(h.proveedor, Proveedor::HomeAssistant);
        assert_eq!(h.nombre, "Casa Ronny");
        assert_eq!(h.version.as_deref(), Some("2026.10.3"));
        assert_eq!(h.direccion.as_deref(), Some("192.168.1.3"));
        assert_eq!(h.manejo, Manejo::HomeAssistant { url: "http://homeassistant.local:8123".into() });

        // Sin direcciones en el TXT ni uuid: la IP con el puerto del SRV.
        let solo = Instancia {
            servicio: "_home-assistant._tcp.local".into(),
            nombre: vec!["Home".into()],
            txt: vec![("internal_url".into(), "".into()), ("base_url".into(), "javascript:alert(1)".into())],
            equipo: None,
            puerto: Some(8124),
            ip: Some(Ipv4Addr::new(192, 168, 1, 4)),
        };
        let h = hallazgo(&solo).unwrap_or_else(|| panic!("sin hallazgo"));
        assert_eq!(h.id, "ha:192.168.1.4");
        assert_eq!(h.nombre, "Home Assistant");
        assert_eq!(h.manejo, Manejo::HomeAssistant { url: "http://192.168.1.4:8124".into() });
    }

    #[test]
    fn lan_mdns_clasifica_cada_servicio() {
        let inst = |servicio: &str, etiqueta: &str, txt: &[(&str, &str)]| Instancia {
            servicio: servicio.into(),
            nombre: vec![etiqueta.into()],
            txt: txt.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            equipo: None,
            puerto: None,
            ip: Some(Ipv4Addr::new(192, 168, 1, 20)),
        };
        let hue = hallazgo(&inst("_hue._tcp.local", "Hue Bridge - 1A2B3C", &[("modelid", "BSB002")])).unwrap_or_else(|| panic!());
        assert_eq!(hue.proveedor, Proveedor::Hue);
        assert_eq!(hue.modelo.as_deref(), Some("BSB002"));
        let lampara = hallazgo(&inst("_hap._tcp.local", "Lámpara 7A3F", &[("ci", "5"), ("md", "LT01")])).unwrap_or_else(|| panic!());
        assert_eq!(lampara.clase, Clase::Luz);
        let cerradura = hallazgo(&inst("_hap._tcp.local", "Puerta", &[("ci", "6")])).unwrap_or_else(|| panic!());
        assert_eq!(cerradura.clase, Clase::Cerradura);
        // La Mac misma (AirPlay) no se muestra; un Apple TV y una tele Samsung sí.
        assert_eq!(hallazgo(&inst("_airplay._tcp.local", "MacBook Pro de Ronny", &[("model", "Mac16,7")])), None);
        let atv = hallazgo(&inst("_airplay._tcp.local", "Sala", &[("model", "AppleTV14,1")])).unwrap_or_else(|| panic!());
        assert_eq!((atv.clase, atv.marca.as_str()), (Clase::Tv, "Apple"));
        let samsung = hallazgo(&inst("_airplay._tcp.local", "[TV] Samsung Q60", &[("model", "QN55Q60BAFXZA"), ("manufacturer", "Samsung")])).unwrap_or_else(|| panic!());
        assert_eq!((samsung.clase, samsung.marca.as_str()), (Clase::Tv, "Samsung"));
        let homepod = hallazgo(&inst("_airplay._tcp.local", "HomePod", &[("model", "AudioAccessory5,1")])).unwrap_or_else(|| panic!());
        assert_eq!(homepod.clase, Clase::Otro);
        let shelly = hallazgo(&inst("_shelly._tcp.local", "shellyplusplugs-a8032ab1", &[])).unwrap_or_else(|| panic!());
        assert_eq!(shelly.clase, Clase::Enchufe);
        let matter = hallazgo(&inst("_matter._tcp.local", "A1B2C3D4E5F60718-0000000000000001", &[])).unwrap_or_else(|| panic!());
        assert_eq!(matter.nombre, "Aparato Matter");
        let esp = hallazgo(&inst("_esphomelib._tcp.local", "sensor-sala", &[("friendly_name", "Sensor de la sala")])).unwrap_or_else(|| panic!());
        assert_eq!(esp.nombre, "Sensor de la sala");
        let atvr = hallazgo(&inst("_androidtvremote2._tcp.local", "Sony TV", &[])).unwrap_or_else(|| panic!());
        assert_eq!(atvr.clase, Clase::Tv);
    }

    #[test]
    fn lan_mdns_no_se_cuelga_con_paquetes_rotos() {
        // Un puntero que apunta a sí mismo.
        let mut p = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        p.extend_from_slice(&[0xC0, 12]);
        assert_eq!(leer(&p), None);
        assert_eq!(leer(&[1, 2, 3]), None);
        // Una consulta (sin el bit de respuesta) no se lee.
        assert_eq!(leer(&consulta(&SERVICIOS)), None);
    }
}
