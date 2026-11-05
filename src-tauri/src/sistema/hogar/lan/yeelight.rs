//! Yeelight por la red local (su "Inter-Operation Specification"). Necesita «Control LAN» encendido en la app
//! de Yeelight: sin eso, la luz no contesta la búsqueda.
//!
//! - Buscar: `M-SEARCH` a 239.255.255.250:1982 con `ST: wifi_bulb`; cada luz contesta con cabeceras (Location
//!   yeelight://ip:55443, id, model, support, power, bright, color_mode, ct, rgb, hue, sat, name).
//! - Manejar y leer: TCP al 55443, una línea JSON por orden (`set_power`, `set_bright`, `set_ct_abx`, `set_hsv`,
//!   `get_prop`), con transición suave de 300 ms. Entre medio pueden llegar avisos `props`: se saltan.

use super::super::modelo::{
    Cambio, Capacidades, Clase, EstadoAparato, FormaLuz, Hallazgo, Hsv, Manejo, ModoLuz, Proveedor, Via, a_kelvin,
    de_kelvin, hsv_de,
};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

pub const PUERTO_BUSQUEDA: u16 = 1982;
pub const PUERTO_ORDENES: u16 = 55443;
const MULTIDIFUSION: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);

pub const BUSQUEDA: &str = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1982\r\nMAN: \"ssdp:discover\"\r\nST: wifi_bulb\r\n";

/// Las propiedades que se leen, en este orden.
const PROPIEDADES: [&str; 7] = ["power", "bright", "ct", "rgb", "hue", "sat", "color_mode"];

/// Las cabeceras de una respuesta, con la clave en minúsculas.
pub fn cabeceras(texto: &str) -> Vec<(String, String)> {
    texto
        .lines()
        .skip(1)
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect()
}

fn dato<'a>(c: &'a [(String, String)], k: &str) -> Option<&'a str> {
    c.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str()).filter(|v| !v.is_empty())
}

/// Lo que dijo una luz al buscarla.
#[derive(Debug, Clone, PartialEq)]
pub struct Luz {
    pub id: String,
    pub ip: Ipv4Addr,
    pub puerto: u16,
    pub modelo: String,
    pub soporte: Vec<String>,
    pub nombre: Option<String>,
    pub version: Option<String>,
    pub estado: EstadoAparato,
}

/// La respuesta a la búsqueda. `None` si no es de una Yeelight.
pub fn luz_de(texto: &str) -> Option<Luz> {
    if !texto.starts_with("HTTP/1.1 200") && !texto.starts_with("NOTIFY") {
        return None;
    }
    let c = cabeceras(texto);
    let lugar = dato(&c, "location")?.strip_prefix("yeelight://")?;
    let (ip, puerto) = match lugar.rsplit_once(':') {
        Some((ip, p)) => (ip.parse::<Ipv4Addr>().ok()?, p.parse::<u16>().ok()?),
        None => (lugar.parse::<Ipv4Addr>().ok()?, PUERTO_ORDENES),
    };
    let id = dato(&c, "id")?.to_ascii_lowercase();
    if !id.starts_with("0x") || id.len() > 24 || !id[2..].chars().all(|x| x.is_ascii_hexdigit()) {
        return None;
    }
    let modelo = dato(&c, "model").unwrap_or("").to_string();
    let rango = rango_de(&modelo);
    let num = |k: &str| dato(&c, k).and_then(|v| v.parse::<i64>().ok());
    let estado = estado_de(
        dato(&c, "power"),
        num("bright"),
        num("color_mode"),
        num("ct"),
        num("rgb"),
        num("hue"),
        num("sat"),
        rango,
    );
    Some(Luz {
        id,
        ip,
        puerto,
        modelo,
        soporte: dato(&c, "support").unwrap_or("").split_whitespace().map(str::to_string).collect(),
        nombre: dato(&c, "name").map(str::to_string),
        version: dato(&c, "fw_ver").map(str::to_string),
        estado,
    })
}

/// El rango de blancos por modelo (el de python-yeelight): las de color y las tiras llegan a 1700 K.
pub fn rango_de(modelo: &str) -> (u32, u32) {
    let m = modelo.to_ascii_lowercase();
    if m.starts_with("color") || m.starts_with("strip") || m.starts_with("bslamp") || m == "stripe" {
        (1700, 6500)
    } else {
        (2700, 6500)
    }
}

#[allow(clippy::too_many_arguments)]
fn estado_de(
    power: Option<&str>,
    bright: Option<i64>,
    modo: Option<i64>,
    ct: Option<i64>,
    rgb: Option<i64>,
    hue: Option<i64>,
    sat: Option<i64>,
    rango: (u32, u32),
) -> EstadoAparato {
    let brillo = bright.map(|b| b.clamp(1, 100) as u8);
    let mut e = EstadoAparato { encendido: power.map(|p| p == "on"), brillo, ..Default::default() };
    match modo {
        Some(1) => {
            if let Some(v) = rgb {
                let v = v.clamp(0, 0xFF_FFFF);
                let c = hsv_de((v >> 16) as u8, (v >> 8) as u8, v as u8);
                e.color = Some(Hsv { v: brillo.unwrap_or(c.v), ..c });
            }
            e.modo = Some(ModoLuz::Color);
        }
        Some(3) => {
            e.color = Some(Hsv::nuevo(hue.unwrap_or(0), sat.unwrap_or(0), brillo.unwrap_or(100) as i64));
            e.modo = Some(ModoLuz::Color);
        }
        Some(2) => {
            e.temperatura = ct.map(|k| de_kelvin(k.clamp(0, 20000) as u32, rango.0, rango.1));
            e.modo = Some(ModoLuz::Blanco);
        }
        // 4 es un flujo de colores y 5 la luz de noche: una escena.
        Some(4 | 5) => e.modo = Some(ModoLuz::Escena),
        _ => {}
    }
    e
}

pub fn capacidades(soporte: &[String]) -> Capacidades {
    let tiene = |m: &str| soporte.iter().any(|s| s == m);
    Capacidades::luz(tiene("set_bright"), tiene("set_ct_abx"), tiene("set_rgb") || tiene("set_hsv"))
}

fn forma_de(modelo: &str) -> FormaLuz {
    let m = modelo.to_ascii_lowercase();
    if m.starts_with("strip") || m == "stripe" {
        FormaLuz::Tira
    } else if m.starts_with("lamp") || m.starts_with("bslamp") || m.starts_with("ceil") {
        FormaLuz::Lampara
    } else {
        FormaLuz::Foco
    }
}

pub fn hallazgo(l: &Luz) -> Hallazgo {
    let forma = forma_de(&l.modelo);
    let generico = match forma {
        FormaLuz::Tira => "Tira Yeelight",
        FormaLuz::Lampara => "Lámpara Yeelight",
        _ => "Foco Yeelight",
    };
    let direccion = if l.puerto == PUERTO_ORDENES { l.ip.to_string() } else { format!("{}:{}", l.ip, l.puerto) };
    Hallazgo {
        id: format!("yeelight:{}", l.id),
        via: Via::Wifi,
        proveedor: Proveedor::Yeelight,
        clase: Clase::Luz,
        forma: Some(forma),
        marca: "Yeelight".into(),
        nombre: l.nombre.clone().unwrap_or_else(|| generico.into()),
        modelo: Some(l.modelo.clone()).filter(|m| !m.is_empty()),
        direccion: Some(direccion),
        mac: None,
        senal: None,
        version: l.version.clone(),
        capacidades: capacidades(&l.soporte),
        estado: Some(l.estado),
        manejo: Manejo::Listo,
    }
}

pub fn buscar(fin: Instant) -> Vec<Hallazgo> {
    let Ok(s) = super::abrir_udp(0) else { return Vec::new() };
    let _ = s.set_multicast_ttl_v4(2);
    let destino = SocketAddr::from((MULTIDIFUSION, PUERTO_BUSQUEDA));
    let mut luces: Vec<Luz> = Vec::new();
    // Dos envíos: al empezar y a los 1,5 s (UDP puede perderse).
    let mut envios = [Instant::now(), Instant::now() + Duration::from_millis(1500)].into_iter().peekable();
    while Instant::now() < fin {
        if envios.peek().is_some_and(|t| Instant::now() >= *t) {
            envios.next();
            let _ = s.send_to(BUSQUEDA.as_bytes(), destino);
        }
        let hasta = envios.peek().copied().unwrap_or(fin).min(fin);
        super::recibir(&s, hasta, 200, |datos, origen| {
            if !super::es_local(origen.ip()) {
                return;
            }
            if let Some(l) = std::str::from_utf8(datos).ok().and_then(luz_de)
                && !luces.iter().any(|x| x.id == l.id)
            {
                luces.push(l);
            }
        });
    }
    luces.iter().map(hallazgo).collect()
}

// ---------------------------------------------------------------- Órdenes por TCP

/// Una línea de orden: `{"id":1,"method":"set_power","params":["on","smooth",300]}\r\n`.
pub fn linea(id: u32, metodo: &str, params: Value) -> String {
    format!("{}\r\n", json!({ "id": id, "method": metodo, "params": params }))
}

/// Las órdenes de un cambio, en orden: primero encender (la luz apagada no acepta lo demás).
pub fn ordenes(cambio: &Cambio, modelo: Option<&str>) -> Vec<(&'static str, Value)> {
    if cambio.encendido == Some(false) {
        return vec![("set_power", json!(["off", "smooth", 300]))];
    }
    let mut v = Vec::new();
    if cambio.encendido == Some(true) {
        v.push(("set_power", json!(["on", "smooth", 300])));
    }
    if let Some(c) = cambio.color {
        v.push(("set_hsv", json!([c.h.min(359), c.s.min(100), "smooth", 300])));
        v.push(("set_bright", json!([c.v.clamp(1, 100), "smooth", 300])));
    } else {
        if let Some(t) = cambio.temperatura {
            let (min, max) = rango_de(modelo.unwrap_or(""));
            v.push(("set_ct_abx", json!([a_kelvin(t, min, max), "smooth", 300])));
        }
        if let Some(b) = cambio.brillo {
            v.push(("set_bright", json!([b.clamp(1, 100), "smooth", 300])));
        }
    }
    v
}

/// La respuesta a la orden `id`: `Ok(resultado)`, `Err(mensaje)` o `None` si es otra cosa (un aviso `props`).
pub fn respuesta(linea: &str, id: u32) -> Option<Result<Value, String>> {
    let v: Value = serde_json::from_str(linea.trim()).ok()?;
    if v.get("id").and_then(Value::as_u64) != Some(id as u64) {
        return None;
    }
    if let Some(e) = v.get("error") {
        return Some(Err(e.get("message").and_then(Value::as_str).unwrap_or("error").to_string()));
    }
    Some(Ok(v.get("result").cloned().unwrap_or(Value::Null)))
}

/// IP y puerto de una dirección guardada ("192.168.1.9" o "192.168.1.9:55443").
fn destino_de(direccion: &str) -> Result<SocketAddr, String> {
    let ip = super::ip_de(direccion)?;
    let puerto = direccion.rsplit_once(':').and_then(|(_, p)| p.parse::<u16>().ok()).unwrap_or(PUERTO_ORDENES);
    Ok(SocketAddr::from((ip, puerto)))
}

/// Manda varias órdenes por una sola conexión y devuelve el resultado de la última.
fn mandar(direccion: &str, ordenes: &[(&str, Value)]) -> Result<Value, String> {
    let destino = destino_de(direccion)?;
    let ip = destino.ip().to_string();
    let fin = Instant::now() + super::PLAZO_ORDEN + Duration::from_millis(500);
    let mut tcp = TcpStream::connect_timeout(&destino, super::PLAZO_ORDEN).map_err(|_| {
        format!("La luz no responde en {ip}. Revisa que esté encendida, en la misma red y con «Control LAN» activo en la app de Yeelight.")
    })?;
    let _ = tcp.set_write_timeout(Some(super::PLAZO_ORDEN));
    let lector = tcp.try_clone().map_err(|_| super::no_responde(&ip))?;
    let mut lector = BufReader::new(lector);
    let mut ultimo = Value::Null;
    for (i, (metodo, params)) in ordenes.iter().enumerate() {
        let id = i as u32 + 1;
        tcp.write_all(linea(id, metodo, params.clone()).as_bytes()).map_err(|_| super::no_responde(&ip))?;
        loop {
            let queda = fin.saturating_duration_since(Instant::now());
            if queda.is_zero() {
                return Err(super::no_responde(&ip));
            }
            let _ = lector.get_ref().set_read_timeout(Some(queda.max(Duration::from_millis(1))));
            let mut texto = String::new();
            match lector.read_line(&mut texto) {
                Ok(0) | Err(_) => return Err(super::no_responde(&ip)),
                Ok(_) => match respuesta(&texto, id) {
                    Some(Ok(r)) => {
                        ultimo = r;
                        break;
                    }
                    Some(Err(e)) => return Err(format!("La luz Yeelight en {ip} no aceptó el cambio ({e}).")),
                    None => continue,
                },
            }
        }
    }
    Ok(ultimo)
}

/// El estado según `get_prop` (las propiedades llegan como textos, en el orden de `PROPIEDADES`).
pub fn estado_de_props(r: &Value, modelo: Option<&str>) -> EstadoAparato {
    let lista: Vec<&str> = r.as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or("")).collect()).unwrap_or_default();
    let texto = |i: usize| lista.get(i).copied().filter(|t| !t.is_empty());
    let num = |i: usize| texto(i).and_then(|t| t.parse::<i64>().ok());
    estado_de(texto(0), num(1), num(6), num(2), num(3), num(4), num(5), rango_de(modelo.unwrap_or("")))
}

pub fn leer(direccion: &str, modelo: Option<&str>) -> Result<EstadoAparato, String> {
    let r = mandar(direccion, &[("get_prop", json!(PROPIEDADES))])?;
    Ok(estado_de_props(&r, modelo))
}

pub fn cambiar(direccion: &str, modelo: Option<&str>, cambio: &Cambio) -> Result<(), String> {
    let lista = ordenes(cambio, modelo);
    if lista.is_empty() {
        return Ok(());
    }
    mandar(direccion, &lista).map(|_| ())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const RESPUESTA: &str = "HTTP/1.1 200 OK\r\nCache-Control: max-age=3600\r\nDate: \r\nExt: \r\nLocation: yeelight://192.168.1.239:55443\r\nServer: POSIX UPnP/1.0 YGLC/1\r\nid: 0x000000000015243f\r\nmodel: color\r\nfw_ver: 18\r\nsupport: get_prop set_default set_power toggle set_bright start_cf stop_cf set_scene cron_add cron_get cron_del set_ct_abx set_rgb\r\npower: on\r\nbright: 100\r\ncolor_mode: 2\r\nct: 4000\r\nrgb: 16711680\r\nhue: 100\r\nsat: 35\r\nname: Luz de la sala\r\n";

    #[test]
    fn lan_yeelight_lee_la_busqueda() {
        let l = luz_de(RESPUESTA).unwrap_or_else(|| panic!("no se leyó"));
        assert_eq!(l.id, "0x000000000015243f");
        assert_eq!(l.ip, Ipv4Addr::new(192, 168, 1, 239));
        assert_eq!(l.puerto, 55443);
        assert_eq!(l.estado.encendido, Some(true));
        assert_eq!(l.estado.brillo, Some(100));
        assert_eq!(l.estado.modo, Some(ModoLuz::Blanco));
        assert_eq!(l.estado.temperatura, Some(de_kelvin(4000, 1700, 6500)));
        let h = hallazgo(&l);
        assert_eq!(h.id, "yeelight:0x000000000015243f");
        assert_eq!(h.nombre, "Luz de la sala");
        assert_eq!(h.capacidades, Capacidades::luz(true, true, true));
        assert_eq!(h.direccion.as_deref(), Some("192.168.1.239"));
        assert_eq!(h.manejo, Manejo::Listo);
        // Lo que no es de Yeelight se salta.
        assert_eq!(luz_de("HTTP/1.1 200 OK\r\nLocation: http://192.168.1.1/x\r\n"), None);
        assert_eq!(luz_de(&RESPUESTA.replace("0x000000000015243f", "../x")), None);
    }

    #[test]
    fn lan_yeelight_arma_las_ordenes() {
        let c = Cambio { encendido: Some(true), color: Some(Hsv { h: 360, s: 80, v: 0 }), ..Default::default() };
        let o = ordenes(&c, Some("color"));
        assert_eq!(o[0], ("set_power", json!(["on", "smooth", 300])));
        assert_eq!(o[1], ("set_hsv", json!([359, 80, "smooth", 300])));
        assert_eq!(o[2], ("set_bright", json!([1, "smooth", 300])));
        let blanco = ordenes(&Cambio { temperatura: Some(100), brillo: Some(30), ..Default::default() }, Some("mono"));
        assert_eq!(blanco, vec![("set_ct_abx", json!([6500, "smooth", 300])), ("set_bright", json!([30, "smooth", 300]))]);
        assert_eq!(ordenes(&Cambio { encendido: Some(false), brillo: Some(3), ..Default::default() }, None).len(), 1);
        assert_eq!(linea(7, "set_power", json!(["off"])), "{\"id\":7,\"method\":\"set_power\",\"params\":[\"off\"]}\r\n");
    }

    #[test]
    fn lan_yeelight_lee_respuestas_y_props() {
        assert_eq!(respuesta(r#"{"id":1, "result":["ok"]}"#, 1), Some(Ok(json!(["ok"]))));
        assert_eq!(respuesta(r#"{"id":2, "error":{"code":-1, "message":"unsupported method"}}"#, 2), Some(Err("unsupported method".into())));
        // Un aviso de cambio no es la respuesta.
        assert_eq!(respuesta(r#"{"method":"props","params":{"power":"on"}}"#, 1), None);
        let props = json!(["on", "40", "2700", "255", "200", "70", "3"]);
        let e = estado_de_props(&props, Some("color"));
        assert_eq!(e.color, Some(Hsv { h: 200, s: 70, v: 40 }));
        assert_eq!(e.modo, Some(ModoLuz::Color));
        let rgb = estado_de_props(&json!(["off", "80", "", "65280", "", "", "1"]), Some("stripe"));
        assert_eq!(rgb.color, Some(Hsv { h: 120, s: 100, v: 80 }));
        assert_eq!(rgb.encendido, Some(false));
        assert_eq!(destino_de("192.168.1.9:55444").map(|d| d.port()), Ok(55444));
        assert!(destino_de("8.8.8.8").is_err());
    }
}
