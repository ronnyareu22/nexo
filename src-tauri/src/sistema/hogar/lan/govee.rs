//! Govee por la red local (su "LAN API"). Necesita «LAN Control» encendido en la app Govee Home.
//!
//! - Buscar: `scan` a 239.255.255.250:4001; las luces contestan al puerto 4002 de este equipo (fijo: si otra
//!   app lo tiene en Windows, Govee se salta).
//! - Manejar: al puerto 4003 de la luz, `turn` (0/1), `brightness` (1–100) y `colorwc` (r, g, b con Kelvin en 0,
//!   o r, g, b en 0 con Kelvin de 2000 a 9000). No contesta las órdenes: se mandan y listo.
//! - Leer: `devStatus` al 4003; la respuesta llega al 4002.

use super::super::modelo::{
    Cambio, Capacidades, Clase, EstadoAparato, FormaLuz, Hallazgo, Hsv, Manejo, ModoLuz, Proveedor, Via, a_kelvin,
    de_kelvin, hsv_de, rgb_de,
};
use serde_json::{Value, json};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

pub const PUERTO_BUSQUEDA: u16 = 4001;
pub const PUERTO_RESPUESTAS: u16 = 4002;
pub const PUERTO_ORDENES: u16 = 4003;
const MULTIDIFUSION: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
/// El rango de blancos de la LAN API.
const KELVIN: (u32, u32) = (2000, 9000);

pub fn mensaje(cmd: &str, data: Value) -> String {
    json!({ "msg": { "cmd": cmd, "data": data } }).to_string()
}

pub fn busqueda() -> String {
    mensaje("scan", json!({ "account_topic": "reserve" }))
}

/// Una respuesta: (orden, datos).
pub fn respuesta(datos: &[u8]) -> Option<(String, Value)> {
    let v: Value = serde_json::from_slice(datos).ok()?;
    let msg = v.get("msg")?;
    Some((msg.get("cmd")?.as_str()?.to_string(), msg.get("data").cloned().unwrap_or(Value::Null)))
}

/// Lo que contestó una luz a la búsqueda.
#[derive(Debug, Clone, PartialEq)]
pub struct Luz {
    pub ip: Ipv4Addr,
    pub id: String,
    pub sku: String,
    pub version: Option<String>,
}

pub fn luz_de(data: &Value, origen: Ipv4Addr) -> Option<Luz> {
    let texto = |k: &str| data.get(k).and_then(Value::as_str).filter(|t| !t.is_empty());
    let id = texto("device")?;
    if id.len() > 40 || !id.chars().all(|c| c.is_ascii_hexdigit() || c == ':') {
        return None;
    }
    // Se usa la IP de quien contestó (la del JSON podría ser de otra red).
    Some(Luz {
        ip: origen,
        id: id.to_ascii_uppercase(),
        sku: texto("sku").unwrap_or("").to_string(),
        version: texto("wifiVersionSoft").map(str::to_string),
    })
}

/// El estado según `devStatus`.
pub fn estado_de(d: &Value) -> EstadoAparato {
    let num = |v: Option<&Value>| v.and_then(Value::as_i64);
    let brillo = num(d.get("brightness")).map(|b| b.clamp(1, 100) as u8);
    let mut e = EstadoAparato { encendido: num(d.get("onOff")).map(|x| x == 1), brillo, ..Default::default() };
    let k = num(d.get("colorTemInKelvin")).unwrap_or(0);
    let color = d.get("color");
    let canal = |c: &str| num(color.and_then(|x| x.get(c))).unwrap_or(0).clamp(0, 255) as u8;
    if k > 0 {
        e.temperatura = Some(de_kelvin(k.clamp(0, 20000) as u32, KELVIN.0, KELVIN.1));
        e.modo = Some(ModoLuz::Blanco);
    } else if color.is_some() {
        let c = hsv_de(canal("r"), canal("g"), canal("b"));
        e.color = Some(Hsv { v: brillo.unwrap_or(c.v), ..c });
        e.modo = Some(ModoLuz::Color);
    }
    e
}

/// Los mensajes de un cambio, en orden.
pub fn mensajes(cambio: &Cambio) -> Vec<String> {
    if cambio.encendido == Some(false) {
        return vec![mensaje("turn", json!({ "value": 0 }))];
    }
    let mut v = Vec::new();
    if cambio.encendido == Some(true) {
        v.push(mensaje("turn", json!({ "value": 1 })));
    }
    if let Some(c) = cambio.color {
        let (r, g, b) = rgb_de(Hsv { v: 100, ..c });
        v.push(mensaje("colorwc", json!({ "color": { "r": r, "g": g, "b": b }, "colorTemInKelvin": 0 })));
        v.push(mensaje("brightness", json!({ "value": c.v.clamp(1, 100) })));
    } else {
        if let Some(t) = cambio.temperatura {
            let k = a_kelvin(t, KELVIN.0, KELVIN.1);
            v.push(mensaje("colorwc", json!({ "color": { "r": 0, "g": 0, "b": 0 }, "colorTemInKelvin": k })));
        }
        if let Some(b) = cambio.brillo {
            v.push(mensaje("brightness", json!({ "value": b.clamp(1, 100) })));
        }
    }
    v
}

fn forma_de(sku: &str) -> FormaLuz {
    let s = sku.to_ascii_uppercase();
    // H61xx son tiras; H60xx, focos y lámparas.
    if s.starts_with("H61") || s.starts_with("H70") { FormaLuz::Tira } else { FormaLuz::Foco }
}

pub fn hallazgo(l: &Luz, estado: Option<EstadoAparato>) -> Hallazgo {
    let forma = forma_de(&l.sku);
    let base = if forma == FormaLuz::Tira { "Tira Govee" } else { "Luz Govee" };
    Hallazgo {
        id: format!("govee:{}", l.id),
        via: Via::Wifi,
        proveedor: Proveedor::Govee,
        clase: Clase::Luz,
        forma: Some(forma),
        marca: "Govee".into(),
        nombre: if l.sku.is_empty() { base.into() } else { format!("{base} {}", l.sku) },
        modelo: Some(l.sku.clone()).filter(|s| !s.is_empty()),
        direccion: Some(l.ip.to_string()),
        mac: None,
        senal: None,
        version: l.version.clone(),
        capacidades: Capacidades::luz(true, true, true),
        estado,
        manejo: Manejo::Listo,
    }
}

pub fn buscar(fin: Instant) -> Vec<Hallazgo> {
    // Las respuestas llegan al 4002: sin ese puerto no hay nada que escuchar.
    let Ok(s) = super::abrir_udp(PUERTO_RESPUESTAS) else { return Vec::new() };
    let _ = s.set_multicast_ttl_v4(2);
    let destino = SocketAddr::from((MULTIDIFUSION, PUERTO_BUSQUEDA));
    let mut luces: Vec<(Luz, Option<EstadoAparato>)> = Vec::new();
    let mut envios = [Instant::now(), Instant::now() + Duration::from_millis(1500)].into_iter().peekable();
    while Instant::now() < fin {
        if envios.peek().is_some_and(|t| Instant::now() >= *t) {
            envios.next();
            let _ = s.send_to(busqueda().as_bytes(), destino);
            // Y a las que ya contestaron sin estado, se les pide otra vez.
            for (l, e) in &luces {
                if e.is_none() {
                    let _ = s.send_to(mensaje("devStatus", json!({})).as_bytes(), (l.ip, PUERTO_ORDENES));
                }
            }
        }
        let hasta = envios.peek().copied().unwrap_or(fin).min(fin);
        super::recibir(&s, hasta, 200, |datos, origen| {
            let IpAddr::V4(ip) = origen.ip() else { return };
            if !super::es_local(origen.ip()) {
                return;
            }
            match respuesta(datos) {
                Some((cmd, data)) if cmd == "scan" => {
                    if let Some(l) = luz_de(&data, ip)
                        && !luces.iter().any(|(x, _)| x.id == l.id)
                    {
                        let _ = s.send_to(mensaje("devStatus", json!({})).as_bytes(), (ip, PUERTO_ORDENES));
                        luces.push((l, None));
                    }
                }
                Some((cmd, data)) if cmd == "devStatus" => {
                    if let Some((_, e)) = luces.iter_mut().find(|(x, _)| x.ip == ip) {
                        *e = Some(estado_de(&data));
                    }
                }
                _ => {}
            }
        });
    }
    luces.iter().map(|(l, e)| hallazgo(l, *e)).collect()
}

pub fn leer(ip: &Ipv4Addr) -> Result<EstadoAparato, String> {
    let s = super::abrir_udp(PUERTO_RESPUESTAS).map_err(|_| {
        "Otra app está usando el puerto de Govee (4002) en este equipo: ciérrala y vuelve a intentar.".to_string()
    })?;
    let fin = Instant::now() + super::PLAZO_ORDEN;
    let mut estado = None;
    for intento in 0..2 {
        if s.send_to(mensaje("devStatus", json!({})).as_bytes(), (*ip, PUERTO_ORDENES)).is_err() {
            break;
        }
        let hasta = (Instant::now() + Duration::from_millis(if intento == 0 { 700 } else { 800 })).min(fin);
        super::recibir(&s, hasta, 800, |datos, origen| {
            if estado.is_none()
                && origen.ip() == IpAddr::V4(*ip)
                && let Some((cmd, data)) = respuesta(datos)
                && cmd == "devStatus"
            {
                estado = Some(estado_de(&data));
            }
        });
        if estado.is_some() {
            break;
        }
    }
    estado.ok_or_else(|| {
        format!("La luz no responde en {ip}. Revisa que esté encendida, en la misma red y con «LAN Control» activo en la app de Govee.")
    })
}

pub fn cambiar(ip: &Ipv4Addr, cambio: &Cambio) -> Result<(), String> {
    let s = super::abrir_udp(0).map_err(|_| super::no_responde(&ip.to_string()))?;
    for (i, m) in mensajes(cambio).iter().enumerate() {
        if i > 0 {
            // Un respiro entre órdenes: algunas luces pierden la segunda si llega pegada.
            std::thread::sleep(Duration::from_millis(60));
        }
        s.send_to(m.as_bytes(), (*ip, PUERTO_ORDENES)).map_err(|_| super::no_responde(&ip.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lan_govee_arma_la_busqueda_y_lee_la_respuesta() {
        assert_eq!(busqueda(), r#"{"msg":{"cmd":"scan","data":{"account_topic":"reserve"}}}"#);
        let r = respuesta(
            br#"{"msg":{"cmd":"scan","data":{"ip":"192.168.1.23","device":"1f:80:c5:32:32:36:72:4e","sku":"H6159","bleVersionHard":"3.01.01","bleVersionSoft":"1.03.01","wifiVersionHard":"1.00.10","wifiVersionSoft":"1.02.03"}}}"#,
        );
        let Some((cmd, data)) = r else { panic!("no se leyó") };
        assert_eq!(cmd, "scan");
        let l = luz_de(&data, Ipv4Addr::new(192, 168, 1, 23)).unwrap_or_else(|| panic!("sin luz"));
        assert_eq!(l.id, "1F:80:C5:32:32:36:72:4E");
        let h = hallazgo(&l, None);
        assert_eq!(h.id, "govee:1F:80:C5:32:32:36:72:4E");
        assert_eq!(h.nombre, "Tira Govee H6159");
        assert_eq!(h.forma, Some(FormaLuz::Tira));
        assert_eq!(h.manejo, Manejo::Listo);
        assert_eq!(luz_de(&json!({"device":"<script>"}), Ipv4Addr::new(192, 168, 1, 2)), None);
    }

    #[test]
    fn lan_govee_lee_devstatus() {
        let d = json!({"onOff":1,"brightness":100,"color":{"r":255,"g":0,"b":0},"colorTemInKelvin":7200});
        let e = estado_de(&d);
        assert_eq!(e.encendido, Some(true));
        assert_eq!(e.modo, Some(ModoLuz::Blanco));
        assert_eq!(e.temperatura, Some(de_kelvin(7200, 2000, 9000)));
        let d = json!({"onOff":0,"brightness":40,"color":{"r":0,"g":0,"b":255},"colorTemInKelvin":0});
        let e = estado_de(&d);
        assert_eq!(e.encendido, Some(false));
        assert_eq!(e.color, Some(Hsv { h: 240, s: 100, v: 40 }));
    }

    #[test]
    fn lan_govee_arma_las_ordenes() {
        // Se comparan como JSON: el orden de las claves no importa.
        let leer = |l: Vec<String>| l.iter().map(|m| serde_json::from_str::<Value>(m).unwrap_or_default()).collect::<Vec<_>>();
        let m = mensajes(&Cambio { encendido: Some(true), color: Some(Hsv { h: 0, s: 100, v: 30 }), ..Default::default() });
        assert_eq!(
            leer(m),
            vec![
                json!({"msg":{"cmd":"turn","data":{"value":1}}}),
                json!({"msg":{"cmd":"colorwc","data":{"color":{"r":255,"g":0,"b":0},"colorTemInKelvin":0}}}),
                json!({"msg":{"cmd":"brightness","data":{"value":30}}}),
            ]
        );
        let blanco = mensajes(&Cambio { temperatura: Some(0), ..Default::default() });
        assert_eq!(leer(blanco), vec![json!({"msg":{"cmd":"colorwc","data":{"color":{"r":0,"g":0,"b":0},"colorTemInKelvin":2000}}})]);
        let apagar = mensajes(&Cambio { encendido: Some(false), ..Default::default() });
        assert_eq!(leer(apagar), vec![json!({"msg":{"cmd":"turn","data":{"value":0}}})]);
    }
}
