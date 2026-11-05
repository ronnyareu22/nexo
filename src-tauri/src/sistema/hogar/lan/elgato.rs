//! Elgato Key Light, Key Light Air, Ring Light y Light Strip por la red local: HTTP en el puerto 9123, sin clave.
//!
//! Elgato no publica esta API; se sigue lo que usa Home Assistant (github.com/frenck/python-elgato y la
//! integración `elgato`):
//! - Se anuncian por mDNS como `_elg._tcp` (su TXT `id` es la MAC).
//! - `GET /elgato/accessory-info`: `productName`, `serialNumber`, `macAddress`, `displayName`, `firmwareVersion`.
//! - `GET /elgato/lights`: `{"numberOfLights":1,"lights":[{"on":1,"brightness":21,"temperature":297}]}`; las de
//!   color (Light Strip) traen `hue` y `saturation` en vez de `temperature`.
//! - `PUT /elgato/lights` con lo que cambia. `on` va como 0 o 1; brillo 0–100; temperatura en mireds 143–344
//!   (unos 7000–2900 K); tono 0–360 y saturación 0–100.

use super::super::modelo::{Cambio, Capacidades, Clase, EstadoAparato, FormaLuz, Hallazgo, Hsv, Manejo, ModoLuz, Proveedor, Via, a_rango, de_rango, mac_normal};
use super::web;
use serde_json::{Value, json};
use std::net::Ipv4Addr;
use std::time::Duration;

pub const PUERTO: u16 = 9123;
const PLAZO: Duration = Duration::from_millis(1500);
/// Mireds: 344 es el más cálido (≈2900 K) y 143 el más frío (≈7000 K).
const MIREDS: (i64, i64) = (143, 344);

#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub nombre: String,
    pub producto: String,
    pub serie: Option<String>,
    pub mac: Option<String>,
    pub version: Option<String>,
}

pub fn info_de(v: &Value) -> Option<Info> {
    let texto = |k: &str| v.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let producto = texto("productName")?;
    Some(Info {
        nombre: texto("displayName").unwrap_or_else(|| producto.clone()),
        serie: texto("serialNumber"),
        mac: texto("macAddress").and_then(|m| mac_normal(&m)),
        version: texto("firmwareVersion"),
        producto,
    })
}

/// Las de color (Light Strip) no tienen blanco cálido a frío como las Key Light.
pub fn es_de_color(producto: &str, luz: Option<&Value>) -> bool {
    producto.to_ascii_lowercase().contains("strip") || luz.is_some_and(|l| l.get("hue").is_some())
}

fn luz(v: &Value) -> Option<&Value> {
    v.get("lights")?.as_array()?.first()
}

pub fn capacidades(color: bool) -> Capacidades {
    Capacidades::luz(true, !color, color)
}

pub fn estado_de(v: &Value) -> Option<EstadoAparato> {
    let l = luz(v)?;
    let num = |k: &str| l.get(k).and_then(Value::as_f64);
    let mut e = EstadoAparato {
        encendido: num("on").map(|x| x > 0.0),
        brillo: num("brightness").map(|b| b.round().clamp(1.0, 100.0) as u8),
        ..Default::default()
    };
    if let (Some(h), Some(s)) = (num("hue"), num("saturation")) {
        e.color = Some(Hsv::nuevo(h.round() as i64, s.round() as i64, e.brillo.unwrap_or(100) as i64));
        e.modo = Some(ModoLuz::Color);
    } else if let Some(m) = num("temperature") {
        // Más mireds, más cálido: 344 → 0 y 143 → 100.
        e.temperatura = Some(100 - de_rango(m.round() as i64, MIREDS.0, MIREDS.1));
        e.modo = Some(ModoLuz::Blanco);
    }
    Some(e)
}

/// El cuerpo de `PUT /elgato/lights` con lo que cambia.
pub fn cuerpo(c: &Cambio) -> Value {
    let mut l = serde_json::Map::new();
    if let Some(on) = c.encendido {
        l.insert("on".into(), json!(u8::from(on)));
    }
    if c.encendido != Some(false) {
        if let Some(color) = c.color {
            l.insert("hue".into(), json!(color.h % 360));
            l.insert("saturation".into(), json!(color.s.min(100)));
            l.insert("brightness".into(), json!(color.v.clamp(1, 100)));
        } else {
            if let Some(t) = c.temperatura {
                l.insert("temperature".into(), json!(a_rango(100 - t.min(100), MIREDS.0, MIREDS.1)));
            }
            if let Some(b) = c.brillo {
                l.insert("brightness".into(), json!(b.clamp(1, 100)));
            }
        }
    }
    json!({ "numberOfLights": 1, "lights": [Value::Object(l)] })
}

pub fn hallazgo(ip: Ipv4Addr, i: &Info, mac_txt: Option<&str>, estado: Option<&Value>) -> Hallazgo {
    let color = es_de_color(&i.producto, estado.and_then(luz));
    let mac = i.mac.clone().or_else(|| mac_txt.and_then(mac_normal));
    let clave = mac.as_deref().map(|m| m.replace(':', "")).or_else(|| i.serie.clone()).unwrap_or_else(|| ip.to_string());
    Hallazgo {
        id: format!("elgato:{}", clave.to_ascii_lowercase()),
        via: Via::Wifi,
        proveedor: Proveedor::Elgato,
        clase: Clase::Luz,
        forma: Some(if color { FormaLuz::Tira } else { FormaLuz::Barra }),
        marca: "Elgato".into(),
        nombre: i.nombre.clone(),
        modelo: Some(i.producto.clone()),
        direccion: Some(ip.to_string()),
        mac,
        senal: None,
        version: i.version.clone(),
        capacidades: capacidades(color),
        estado: estado.and_then(estado_de),
        manejo: Manejo::Listo,
    }
}

/// Lee su info y cómo está (para la búsqueda).
pub fn probar(ip: Ipv4Addr, puerto: u16, mac_txt: Option<&str>) -> Option<Hallazgo> {
    let info = info_de(&web::json(ip, puerto, "/elgato/accessory-info", &[], PLAZO).ok()?)?;
    let estado = web::json(ip, puerto, "/elgato/lights", &[], PLAZO).ok();
    Some(hallazgo(ip, &info, mac_txt, estado.as_ref()))
}

pub fn leer(ip: Ipv4Addr) -> Result<EstadoAparato, String> {
    let v = web::json(ip, PUERTO, "/elgato/lights", &[], PLAZO)?;
    estado_de(&v).ok_or_else(|| "La luz Elgato contestó algo que no entiendo.".into())
}

pub fn cambiar(ip: Ipv4Addr, c: &Cambio) -> Result<(), String> {
    let cuerpo = serde_json::to_vec(&cuerpo(c)).unwrap_or_default();
    let r = web::pedir(ip, PUERTO, "PUT", "/elgato/lights", &[], Some(&cuerpo), PLAZO)?;
    if r.ok() { Ok(()) } else { Err(format!("La luz Elgato no aceptó el cambio ({}).", r.estado)) }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lan_elgato_info_y_estado() {
        // Los ejemplos de python-elgato (tests/fixtures).
        let info = info_de(&json!({"productName":"Elgato Key Light","hardwareBoardType":53,"macAddress":"AA:BB:CC:DD:EE:FF","firmwareBuildNumber":218,"firmwareVersion":"1.0.3","serialNumber":"CN11A1A00001","displayName":"Frenck","features":["lights"]}))
            .unwrap_or_else(|| panic!("sin info"));
        assert_eq!((info.nombre.as_str(), info.mac.as_deref()), ("Frenck", Some("aa:bb:cc:dd:ee:ff")));
        let blanca = json!({"numberOfLights":1,"lights":[{"on":1,"brightness":21,"temperature":297}]});
        let h = hallazgo(Ipv4Addr::new(192, 168, 1, 70), &info, None, Some(&blanca));
        assert_eq!((h.id.as_str(), h.forma, h.capacidades.temperatura, h.capacidades.color), ("elgato:aabbccddeeff", Some(FormaLuz::Barra), true, false));
        let e = h.estado.unwrap_or_default();
        assert_eq!((e.encendido, e.brillo, e.modo), (Some(true), Some(21), Some(ModoLuz::Blanco)));
        assert_eq!(e.temperatura, Some(23));
        let tira = json!({"numberOfLights":1,"lights":[{"on":1,"hue":358.0,"saturation":6.0,"brightness":50}]});
        assert!(es_de_color("Elgato Light Strip", None));
        assert_eq!(estado_de(&tira).and_then(|e| e.color), Some(Hsv { h: 358, s: 6, v: 50 }));
        assert_eq!(info_de(&json!({"displayName":"x"})), None);
    }

    #[test]
    fn lan_elgato_cuerpos() {
        assert_eq!(cuerpo(&Cambio { encendido: Some(false), brillo: Some(10), ..Default::default() }), json!({"numberOfLights":1,"lights":[{"on":0}]}));
        assert_eq!(
            cuerpo(&Cambio { encendido: Some(true), temperatura: Some(0), brillo: Some(55), ..Default::default() }),
            json!({"numberOfLights":1,"lights":[{"on":1,"temperature":344,"brightness":55}]})
        );
        assert_eq!(
            cuerpo(&Cambio { color: Some(Hsv { h: 200, s: 80, v: 30 }), ..Default::default() }),
            json!({"numberOfLights":1,"lights":[{"hue":200,"saturation":80,"brightness":30}]})
        );
    }
}
