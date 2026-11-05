//! "Tu red": todo lo que hay en la misma red, no solo lo de la casa, y qué puede hacer Nexo con cada cosa.
//!
//! - Quién está: la tabla de vecinos del sistema (`vecinos`), después de despertarla con un datagrama corto a cada
//!   IP de la /24. También el router (la puerta de enlace) y este equipo.
//! - La marca por la MAC (`oui.rs`, generado del registro de la IEEE por `scripts/tablas-red.py`). Una MAC
//!   "administrada localmente" (el bit 0x02 del primer byte) es una dirección privada: la cambian solos los
//!   teléfonos y las computadoras, así que no dice la marca.
//! - El nombre: el de Nexo, el que dio al buscarlo, lo que contestó por mDNS y el DNS inverso (`nombres`).
//! - Qué es (router, teléfono, computadora, tele, parlante, luz, enchufe, cámara, impresora, consola o no se sabe) y
//!   en qué grupo va: "Se manejan con Nexo", "Se pueden agregar" u "Otros equipos en tu red" (con el porqué).
//! - Solo a pedido; lo que se encuentra queda en memoria.

pub mod nombres;
mod oui;
mod vecinos;

pub use vecinos::{Red, despertar, red, vecinos};

use super::modelo::{Clase, Manejo, Proveedor, normalizar};
use super::{Aparato, HallazgoVista};
use serde::Serialize;
use std::net::Ipv4Addr;

/// La marca y su pista de tipo por los 3 primeros bytes de la MAC.
pub fn marca(mac: &str) -> Option<(&'static str, &'static str)> {
    let hex: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).take(6).collect();
    let p = u32::from_str_radix(&hex, 16).ok().filter(|_| hex.len() == 6)?;
    let i = oui::PREFIJOS.binary_search_by_key(&p, |x| x >> 8).ok()?;
    oui::MARCAS.get((oui::PREFIJOS[i] & 0xff) as usize).copied()
}

/// Una MAC privada (administrada localmente, como las que inventan iOS, Android, Windows y macOS por red).
pub fn privada(mac: &str) -> bool {
    let hex: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).take(2).collect();
    u8::from_str_radix(&hex, 16).is_ok_and(|b| b & 0x02 != 0)
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TipoEquipo {
    Router,
    EsteEquipo,
    Telefono,
    Computadora,
    Tele,
    Parlante,
    Luz,
    Enchufe,
    Camara,
    Impresora,
    Consola,
    Desconocido,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum GrupoRed {
    Nexo,
    Agregar,
    Otros,
}

/// Un equipo de la red, listo para la interfaz.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EquipoRed {
    pub ip: String,
    pub mac: Option<String>,
    pub privada: bool,
    pub nombre: String,
    pub nombre_red: Option<String>,
    pub marca: Option<String>,
    pub tipo: TipoEquipo,
    pub grupo: GrupoRed,
    pub motivo: Option<String>,
    pub hallazgo: Option<HallazgoVista>,
    pub aparatos: Vec<String>,
}

/// Lo que se sabe de una IP.
#[derive(Debug)]
pub struct Entrada<'a> {
    pub ip: Ipv4Addr,
    pub mac: Option<String>,
    pub propia: bool,
    pub router: bool,
    /// De mDNS: nombres ("Sala"), "modelo:…" y "equipo:…"; y el DNS inverso.
    pub pistas: Vec<String>,
    /// El nombre por DNS inverso.
    pub inverso: Option<String>,
    pub hallazgo: Option<&'a HallazgoVista>,
    /// Los aparatos de Nexo en esta IP.
    pub aparatos: Vec<&'a Aparato>,
}

impl Default for Entrada<'_> {
    fn default() -> Self {
        Entrada {
            ip: Ipv4Addr::UNSPECIFIED,
            mac: None,
            propia: false,
            router: false,
            pistas: Vec::new(),
            inverso: None,
            hallazgo: None,
            aparatos: Vec::new(),
        }
    }
}

fn contiene(t: &str, palabras: &[&str]) -> bool {
    let n = normalizar(t);
    palabras.iter().any(|p| if p.contains(' ') { n.contains(p) } else { n.split(' ').any(|x| x.starts_with(p)) })
}

/// Qué es, por lo que dicen sus nombres y modelos.
fn tipo_por_nombres(textos: &[&str]) -> Option<TipoEquipo> {
    let todo = textos.join(" ");
    let t = todo.as_str();
    if contiene(t, &["iphone", "ipad", "android", "galaxy", "pixel", "redmi", "xiaomi", "oneplus", "motorola", "moto", "huawei", "honor"]) {
        return Some(TipoEquipo::Telefono);
    }
    if contiene(t, &["appletv", "apple tv", "tv", "roku", "chromecast", "firetv", "fire tv", "bravia", "webos", "tizen", "shield"]) {
        return Some(TipoEquipo::Tele);
    }
    if contiene(t, &["homepod", "audioaccessory", "sonos", "echo", "nest mini", "nest audio", "google home", "speaker", "parlante", "bose"]) {
        return Some(TipoEquipo::Parlante);
    }
    if contiene(t, &["playstation", "ps4", "ps5", "xbox", "nintendo"]) {
        return Some(TipoEquipo::Consola);
    }
    if contiene(t, &["printer", "impresora", "laserjet", "deskjet", "officejet", "envy", "epson", "brother", "canon", "pixma"]) {
        return Some(TipoEquipo::Impresora);
    }
    if contiene(t, &["camera", "camara", "cam", "doorbell", "timbre"]) {
        return Some(TipoEquipo::Camara);
    }
    if contiene(t, &["macbook", "imac", "mac mini", "macmini", "macpro", "mac16", "mac15", "mac14", "desktop", "laptop", "pc", "windows", "ubuntu", "raspberrypi", "nas", "synology"]) {
        return Some(TipoEquipo::Computadora);
    }
    None
}

fn tipo_por_pista(p: &str) -> Option<TipoEquipo> {
    Some(match p {
        "luz" => TipoEquipo::Luz,
        "enchufe" => TipoEquipo::Enchufe,
        "parlante" => TipoEquipo::Parlante,
        "tele" => TipoEquipo::Tele,
        "camara" => TipoEquipo::Camara,
        "impresora" => TipoEquipo::Impresora,
        "consola" => TipoEquipo::Consola,
        "router" => TipoEquipo::Router,
        "computadora" => TipoEquipo::Computadora,
        "telefono" => TipoEquipo::Telefono,
        _ => return None,
    })
}

fn tipo_por_clase(c: Clase) -> Option<TipoEquipo> {
    match c {
        Clase::Luz => Some(TipoEquipo::Luz),
        Clase::Enchufe => Some(TipoEquipo::Enchufe),
        Clase::Tv => Some(TipoEquipo::Tele),
        _ => None,
    }
}

fn motivo_de(tipo: TipoEquipo, privada: bool, iot: bool, h: Option<&HallazgoVista>) -> String {
    if let Some(h) = h {
        match &h.hallazgo.manejo {
            Manejo::Pronto { motivo } => return motivo.clone(),
            Manejo::CuentaTuya => return "Es un aparato Tuya: funciona solo con la nube de Tuya.".into(),
            _ => {}
        }
    }
    match tipo {
        TipoEquipo::Router => "Es tu router: reparte internet en tu casa.".into(),
        TipoEquipo::EsteEquipo => "Es este equipo.".into(),
        TipoEquipo::Telefono => "Es un teléfono o una tableta.".into(),
        TipoEquipo::Computadora => "Es una computadora.".into(),
        TipoEquipo::Tele => "Es una tele. Pronto podrás manejarla desde Nexo.".into(),
        TipoEquipo::Parlante => "Es un parlante. Por ahora Nexo solo lo muestra.".into(),
        TipoEquipo::Impresora => "Es una impresora.".into(),
        TipoEquipo::Consola => "Es una consola de juegos.".into(),
        TipoEquipo::Camara => "Es una cámara. Por ahora Nexo solo la muestra.".into(),
        TipoEquipo::Luz | TipoEquipo::Enchufe => "No contestó a ninguna forma local que Nexo conozca.".into(),
        TipoEquipo::Desconocido if privada => "Dirección privada: suele ser un teléfono o una computadora.".into(),
        TipoEquipo::Desconocido if iot => "Parece un aparato inteligente, pero no contestó a nada que Nexo conozca.".into(),
        TipoEquipo::Desconocido => "No sé qué es: no contestó a nada que Nexo conozca.".into(),
    }
}

/// "Equipo Apple", "Router TP-Link", "Equipo desconocido".
fn nombre_generico(tipo: TipoEquipo, marca: Option<&str>) -> String {
    let que = match tipo {
        TipoEquipo::Router => "Router",
        TipoEquipo::EsteEquipo => "Este equipo",
        TipoEquipo::Telefono => "Teléfono",
        TipoEquipo::Computadora => "Computadora",
        TipoEquipo::Tele => "Tele",
        TipoEquipo::Parlante => "Parlante",
        TipoEquipo::Luz => "Luz",
        TipoEquipo::Enchufe => "Enchufe",
        TipoEquipo::Camara => "Cámara",
        TipoEquipo::Impresora => "Impresora",
        TipoEquipo::Consola => "Consola",
        TipoEquipo::Desconocido => "Equipo",
    };
    match (tipo, marca) {
        (TipoEquipo::EsteEquipo, _) => que.into(),
        (_, Some(m)) => format!("{que} {m}"),
        (TipoEquipo::Desconocido, None) => "Equipo desconocido".into(),
        (_, None) => que.into(),
    }
}

/// Arma la fila de una IP.
pub fn equipo(e: &Entrada) -> EquipoRed {
    let privada = e.mac.as_deref().is_some_and(privada);
    let (marca_mac, pista) = e.mac.as_deref().filter(|_| !privada).and_then(marca).unzip();
    let h = e.hallazgo;
    let nombres: Vec<&str> = e.pistas.iter().filter(|p| !p.contains(':')).map(String::as_str).collect();
    let modelos: Vec<&str> = e.pistas.iter().filter_map(|p| p.split_once(':').map(|(_, v)| v)).collect();
    let equipo_red = e.pistas.iter().find_map(|p| p.strip_prefix("equipo:")).map(str::to_string).or_else(|| e.inverso.clone());

    let mut textos: Vec<&str> = nombres.iter().chain(modelos.iter()).copied().collect();
    if let Some(n) = equipo_red.as_deref() {
        textos.push(n);
    }
    if let Some(h) = h {
        textos.push(&h.hallazgo.nombre);
        textos.push(&h.hallazgo.marca);
    }
    let tipo = if e.router {
        TipoEquipo::Router
    } else if e.propia {
        TipoEquipo::EsteEquipo
    } else if let Some(t) = e.aparatos.iter().find_map(|a| tipo_por_clase(a.clase)) {
        t
    } else if let Some(t) = h.and_then(|h| tipo_por_clase(h.hallazgo.clase)) {
        t
    } else if h.is_some_and(|h| h.hallazgo.proveedor == Proveedor::HomeAssistant) {
        TipoEquipo::Computadora
    } else if let Some(t) = h.filter(|h| h.hallazgo.proveedor == Proveedor::Hue).map(|_| TipoEquipo::Luz) {
        t
    } else if let Some(t) = tipo_por_nombres(&textos) {
        t
    } else if let Some(t) = pista.and_then(tipo_por_pista) {
        t
    } else if e.pistas.iter().any(|p| p.starts_with("modelo:") && contiene(p, &["ipp", "printer"])) {
        TipoEquipo::Impresora
    } else {
        TipoEquipo::Desconocido
    };

    let marca = h.map(|h| h.hallazgo.marca.clone()).filter(|m| !m.is_empty()).or(marca_mac.map(str::to_string));
    let nexo = !e.aparatos.is_empty() || h.is_some_and(|h| h.agregado && h.hallazgo.manejo == Manejo::Listo);
    let agregable = h.filter(|h| !h.agregado).filter(|h| {
        matches!(h.hallazgo.manejo, Manejo::Listo | Manejo::Vincular { .. } | Manejo::Activar { .. } | Manejo::HomeAssistant { .. })
    });
    let (grupo, motivo) = if nexo {
        (GrupoRed::Nexo, None)
    } else if let Some(h) = agregable {
        let m = match &h.hallazgo.manejo {
            Manejo::Activar { paso } => Some(paso.clone()),
            Manejo::Vincular { paso, .. } => Some(paso.clone()),
            _ => None,
        };
        (GrupoRed::Agregar, m)
    } else {
        (GrupoRed::Otros, Some(motivo_de(tipo, privada, pista == Some("iot"), h)))
    };

    // El nombre: el de Nexo (uno o el puente), el del hallazgo, el de mDNS, el del DNS o el genérico.
    let nombre = if e.propia {
        "Este equipo".to_string()
    } else if let [a] = e.aparatos.as_slice() {
        a.nombre.clone()
    } else if let Some(h) = h {
        h.hallazgo.nombre.clone()
    } else if let Some(n) = nombres.first() {
        n.to_string()
    } else if let Some(n) = &equipo_red {
        n.clone()
    } else {
        nombre_generico(tipo, marca.as_deref())
    };
    let nombre_red = equipo_red.filter(|n| normalizar(n) != normalizar(&nombre));
    EquipoRed {
        ip: e.ip.to_string(),
        mac: e.mac.clone(),
        privada,
        nombre,
        nombre_red,
        marca,
        tipo,
        grupo,
        motivo,
        hallazgo: h.cloned(),
        aparatos: e.aparatos.iter().map(|a| a.id.clone()).collect(),
    }
}

/// En orden: por grupo, el router y este equipo primero en "otros", y luego por IP.
pub fn ordenar(v: &mut [EquipoRed]) {
    let peso = |e: &EquipoRed| match e.tipo {
        TipoEquipo::Router => 0,
        TipoEquipo::EsteEquipo => 1,
        _ => 2,
    };
    v.sort_by_key(|e| (e.grupo, peso(e), e.ip.parse::<Ipv4Addr>().map(u32::from).unwrap_or(u32::MAX)));
}

#[cfg(test)]
mod pruebas {
    use super::super::modelo::{Capacidades, FormaVinculo, Hallazgo, Via};
    use super::*;

    fn h(id: &str, proveedor: Proveedor, clase: Clase, manejo: Manejo, agregado: bool) -> HallazgoVista {
        HallazgoVista {
            hallazgo: Hallazgo {
                id: id.into(),
                via: Via::Wifi,
                proveedor,
                clase,
                forma: None,
                marca: "WLED".into(),
                nombre: "Tira".into(),
                modelo: None,
                direccion: Some("192.168.1.9".into()),
                mac: None,
                senal: None,
                version: None,
                capacidades: Capacidades::ninguna(),
                estado: None,
                manejo,
            },
            agregado,
        }
    }

    #[test]
    fn red_marcas_por_mac() {
        // 00:17:88 es de Philips (Signify); 24:0A:C4, de Espressif; F0:18:98, de Apple.
        assert_eq!(marca("00:17:88:01:02:03"), Some(("Philips Hue", "luz")));
        assert_eq!(marca("24:0a:c4:12:34:56").map(|m| m.1), Some("iot"));
        assert_eq!(marca("f0:18:98:00:00:01").map(|m| m.0), Some("Apple"));
        assert_eq!(marca("zz"), None);
        assert!(privada("da:a1:19:00:00:01"));
        assert!(!privada("00:17:88:01:02:03"));
    }

    #[test]
    fn red_arma_cada_fila() {
        let ip = Ipv4Addr::new(192, 168, 1, 1);
        let router = equipo(&Entrada { ip, mac: Some("00:17:88:00:00:01".into()), router: true, ..Default::default() });
        assert_eq!((router.tipo, router.grupo), (TipoEquipo::Router, GrupoRed::Otros));
        assert_eq!(router.motivo.as_deref(), Some("Es tu router: reparte internet en tu casa."));
        // Una dirección privada sin nada más: no se sabe la marca y se dice por qué.
        let tel = equipo(&Entrada { ip: Ipv4Addr::new(192, 168, 1, 30), mac: Some("da:a1:19:00:00:01".into()), ..Default::default() });
        assert_eq!((tel.privada, tel.marca.clone(), tel.tipo), (true, None, TipoEquipo::Desconocido));
        assert_eq!(tel.motivo.as_deref(), Some("Dirección privada: suele ser un teléfono o una computadora."));
        // Con nombre por DNS inverso, ya se sabe qué es.
        let mac = equipo(&Entrada { ip: Ipv4Addr::new(192, 168, 1, 31), inverso: Some("MacBook-Pro".into()), ..Default::default() });
        assert_eq!((mac.tipo, mac.nombre.as_str()), (TipoEquipo::Computadora, "MacBook-Pro"));
        // Un WLED encontrado y sin agregar: "Se pueden agregar".
        let w = h("wled:1", Proveedor::Wled, Clase::Luz, Manejo::Listo, false);
        let tira = equipo(&Entrada { ip: Ipv4Addr::new(192, 168, 1, 9), hallazgo: Some(&w), ..Default::default() });
        assert_eq!((tira.grupo, tira.tipo, tira.nombre.as_str()), (GrupoRed::Agregar, TipoEquipo::Luz, "Tira"));
        // Un Hue sin vincular: también, con el paso.
        let hue = h("hue:1", Proveedor::Hue, Clase::Otro, Manejo::Vincular { forma: FormaVinculo::Boton, paso: "Presiona el botón.".into() }, false);
        let puente = equipo(&Entrada { ip: Ipv4Addr::new(192, 168, 1, 2), hallazgo: Some(&hue), ..Default::default() });
        assert_eq!((puente.grupo, puente.motivo.as_deref()), (GrupoRed::Agregar, Some("Presiona el botón.")));
        // Tuya en la red: solo se ve.
        let t = h("tuya:1", Proveedor::Tuya, Clase::Otro, Manejo::CuentaTuya, false);
        let tuya = equipo(&Entrada { ip: Ipv4Addr::new(192, 168, 1, 40), hallazgo: Some(&t), ..Default::default() });
        assert_eq!((tuya.grupo, tuya.motivo.as_deref()), (GrupoRed::Otros, Some("Es un aparato Tuya: funciona solo con la nube de Tuya.")));
        let mut todo = vec![tuya, puente, router, tira];
        ordenar(&mut todo);
        assert_eq!(todo.iter().map(|e| e.ip.as_str()).collect::<Vec<_>>(), ["192.168.1.2", "192.168.1.9", "192.168.1.1", "192.168.1.40"]);
    }

    /// Un análisis de verdad de la red de este equipo, solo de lectura (no corre solo):
    /// `cargo test --lib red_analiza_en_vivo -- --ignored --nocapture`. Muestra solo cuántos hay de cada cosa.
    #[test]
    #[ignore]
    fn red_analiza_en_vivo() {
        use super::super::lan;
        use std::collections::BTreeMap;
        use std::time::{Duration, Instant};
        let inicio = Instant::now();
        let b = lan::buscar_todo(Duration::from_secs(4));
        let ips: Vec<Ipv4Addr> = b.vecinos.iter().map(|(ip, _)| *ip).collect();
        let inversos = nombres::inversos(&ips, b.red.router, Duration::from_millis(900));
        let vistas: Vec<HallazgoVista> = b.hallazgos.iter().map(|h| HallazgoVista { hallazgo: h.clone(), agregado: false }).collect();
        let mut todas = ips.clone();
        todas.extend(vistas.iter().filter_map(|v| lan::ip_de(v.hallazgo.direccion.as_deref()?).ok()));
        todas.extend(b.red.ip);
        todas.sort();
        todas.dedup();
        let equipos: Vec<EquipoRed> = todas
            .iter()
            .map(|ip| {
                let hallazgo = vistas.iter().find(|v| v.hallazgo.direccion.as_deref().and_then(|d| lan::ip_de(d).ok()) == Some(*ip));
                equipo(&Entrada {
                    ip: *ip,
                    mac: b.vecinos.iter().find(|(v, _)| v == ip).map(|(_, m)| m.clone()),
                    propia: Some(*ip) == b.red.ip,
                    router: Some(*ip) == b.red.router,
                    pistas: b.pistas.iter().filter(|(p, _)| p == ip).flat_map(|(_, v)| v.iter().cloned()).collect(),
                    inverso: inversos.get(ip).cloned(),
                    hallazgo,
                    aparatos: Vec::new(),
                })
            })
            .collect();
        let mut tipos: BTreeMap<String, usize> = BTreeMap::new();
        let mut grupos: BTreeMap<String, usize> = BTreeMap::new();
        for e in &equipos {
            *tipos.entry(format!("{:?}", e.tipo)).or_default() += 1;
            *grupos.entry(format!("{:?}", e.grupo)).or_default() += 1;
        }
        let mut proveedores: BTreeMap<String, usize> = BTreeMap::new();
        for h in &b.hallazgos {
            *proveedores.entry(format!("{:?}", h.proveedor)).or_default() += 1;
        }
        println!("red: prefijo {:?}, router {}, este equipo {}", b.red.prefijo, b.red.router.is_some(), b.red.ip.is_some());
        println!("vecinos en la tabla: {} · con nombre por DNS: {} · con pistas de mDNS: {}", b.vecinos.len(), inversos.len(), b.pistas.len());
        println!("equipos: {} · con marca: {} · MAC privada: {}", equipos.len(), equipos.iter().filter(|e| e.marca.is_some()).count(), equipos.iter().filter(|e| e.privada).count());
        println!("por tipo: {tipos:?}");
        println!("por grupo: {grupos:?}");
        println!("hallazgos por proveedor: {proveedores:?}");
        println!("tardó {:?}", inicio.elapsed());
        assert!(inicio.elapsed() < Duration::from_secs(7));
    }
}
