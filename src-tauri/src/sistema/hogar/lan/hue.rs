//! Philips Hue por el puente, en la red local: la API v1 por HTTPS con el certificado del puente fijado.
//!
//! - Por qué v1 y no v2: desde 2025 el puente deja de contestar por HTTP (las dos versiones van por HTTPS), así
//!   que el certificado hay que manejarlo igual en las dos. La v1 da lo mismo para luces y grupos con JSON más
//!   simple (tono y saturación directos, sin pasar a xy) y también sirve en el puente redondo (BSB001), que no
//!   recibe más actualizaciones y sigue por HTTP.
//! - El certificado es del puente (firmado por Signify, que no está en el sistema, o propio en los viejos): se
//!   fija el primero que se ve (`http::pedir_fijado`) y desde ahí solo se acepta ese.
//! - Se anuncia por mDNS como `_hue._tcp` (TXT `bridgeid` y `modelid`).
//! - Vincular: se presiona el botón redondo del puente y, dentro de 30 s, `POST /api` con
//!   `{"devicetype":"nexo#mac"}` devuelve `[{"success":{"username":"…"}}]`; antes de presionarlo,
//!   `[{"error":{"type":101,…}}]`. La clave va al llavero.
//! - Luces: `GET /api/<clave>/lights` (`state.on`, `bri` 1–254, `hue` 0–65535, `sat` 0–254, `ct` en mireds según
//!   `capabilities.control.ct`, `reachable`; `type`, `uniqueid`, `config.archetype`). Cambiar:
//!   `PUT /api/<clave>/lights/<n>/state`. Habitaciones: `GET /api/<clave>/groups` (`type` "Room").

use super::super::http;
use super::super::modelo::{
    Cambio, Capacidades, Clase, EstadoAparato, FormaLuz, FormaVinculo, Hallazgo, Hsv, Manejo, ModoLuz, Proveedor, Via, a_kelvin,
    brillo_a_rango, brillo_de_rango, de_kelvin, mac_normal,
};
use super::web;
use serde_json::{Value, json};
use std::net::Ipv4Addr;
use std::time::Duration;

const PLAZO: Duration = Duration::from_secs(4);
/// El rango de blancos de fábrica (153–500 mireds ≈ 6500–2000 K).
const MIREDS: (u32, u32) = (153, 500);

/// Cómo llegar al puente.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Puente {
    pub ip: Ipv4Addr,
    /// HTTPS con el certificado fijado (todos menos el redondo, BSB001).
    pub seguro: bool,
    pub huella: Option<[u8; 32]>,
}

impl Puente {
    pub fn nuevo(ip: Ipv4Addr, modelo: Option<&str>, huella: Option<[u8; 32]>) -> Self {
        Puente { ip, seguro: modelo != Some("BSB001"), huella }
    }
}

/// Una petición al puente: (JSON, huella del certificado si es HTTPS).
fn pedir(p: &Puente, metodo: &str, ruta: &str, cuerpo: Option<&Value>) -> Result<(Value, Option<[u8; 32]>), String> {
    let bytes = cuerpo.map(|c| serde_json::to_vec(c).unwrap_or_default());
    let (estado, datos, huella) = if p.seguro {
        let url = format!("https://{}{ruta}", p.ip);
        let cab = vec![("Content-Type".to_string(), "application/json".to_string())];
        let (r, h) = http::pedir_fijado(metodo, &url, &cab, bytes.as_deref(), PLAZO, p.huella)?;
        (r.estado, r.cuerpo, Some(h))
    } else {
        let r = web::pedir(p.ip, 80, metodo, ruta, &[], bytes.as_deref(), PLAZO)?;
        (r.estado, r.cuerpo, None)
    };
    if !(200..300).contains(&estado) {
        return Err(format!("El puente Hue contestó con un error ({estado})."));
    }
    let v = serde_json::from_slice(&datos).map_err(|_| "El puente Hue contestó algo que no entiendo.".to_string())?;
    Ok((v, huella))
}

/// `[{"error":{"type":1,…}}]` en palabras; `Ok` si no hay errores.
pub fn sin_errores(v: &Value) -> Result<(), String> {
    let Some(lista) = v.as_array() else { return Ok(()) };
    for x in lista {
        if let Some(e) = x.get("error") {
            return Err(match e.get("type").and_then(Value::as_i64) {
                Some(1) => "El puente Hue ya no acepta a Nexo. Quítalo y vincúlalo de nuevo.".into(),
                Some(3) => "El puente Hue ya no tiene esa luz.".into(),
                Some(201) => "La luz está apagada: enciéndela primero.".into(),
                _ => format!("El puente Hue no aceptó el cambio ({}).", e.get("description").and_then(Value::as_str).unwrap_or("error")),
            });
        }
    }
    Ok(())
}

/// Lo que dice `GET /api/config` sin clave: (id del puente, nombre, MAC, modelo).
pub fn identidad(v: &Value) -> Option<(String, String, Option<String>, Option<String>)> {
    let id = v.get("bridgeid")?.as_str()?.trim().to_ascii_lowercase();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let nombre = v.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Puente Hue").to_string();
    Some((id, nombre, v.get("mac").and_then(Value::as_str).and_then(mac_normal), v.get("modelid").and_then(Value::as_str).map(str::to_string)))
}

pub fn config(p: &Puente) -> Result<(Value, Option<[u8; 32]>), String> {
    pedir(p, "GET", "/api/config", None)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Vinculo {
    Clave(String),
    /// Todavía no se presionó el botón.
    Esperando,
}

pub fn vinculo_de(v: &Value) -> Result<Vinculo, String> {
    let primero = v.as_array().and_then(|a| a.first()).ok_or("El puente Hue contestó algo que no entiendo.")?;
    if let Some(u) = primero.pointer("/success/username").and_then(Value::as_str) {
        if u.is_empty() || u.len() > 128 || !u.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err("El puente Hue contestó una clave rara.".into());
        }
        return Ok(Vinculo::Clave(u.to_string()));
    }
    match primero.pointer("/error/type").and_then(Value::as_i64) {
        Some(101) => Ok(Vinculo::Esperando),
        _ => Err(format!(
            "El puente Hue no dejó vincular ({}).",
            primero.pointer("/error/description").and_then(Value::as_str).unwrap_or("error")
        )),
    }
}

/// Un intento de vincular.
pub fn vincular(p: &Puente) -> Result<(Vinculo, Option<[u8; 32]>), String> {
    let equipo = if cfg!(windows) { "nexo#pc" } else { "nexo#mac" };
    let (v, h) = pedir(p, "POST", "/api", Some(&json!({ "devicetype": equipo })))?;
    Ok((vinculo_de(&v)?, h))
}

/// Una luz del puente.
#[derive(Debug, Clone, PartialEq)]
pub struct Luz {
    /// El número que le da el puente ("3").
    pub numero: String,
    /// `uniqueid` en corto ("0017880100f07118-0b"): no cambia aunque se restablezca el puente.
    pub unico: Option<String>,
    pub nombre: String,
    pub clase: Clase,
    pub forma: Option<FormaLuz>,
    pub capacidades: Capacidades,
    /// En Kelvin, por los mireds que acepta.
    pub kelvin: Option<(u32, u32)>,
    pub modelo: Option<String>,
    pub estado: EstadoAparato,
    pub alcanzable: bool,
}

fn forma_de(arquetipo: &str, producto: &str) -> FormaLuz {
    let a = format!("{arquetipo} {}", producto.to_ascii_lowercase());
    if a.contains("lightstrip") || a.contains("strip") {
        FormaLuz::Tira
    } else if a.contains("hueplay") || a.contains("play") || a.contains("lightbar") || a.contains("signe") {
        FormaLuz::Barra
    } else if ["bulb", "candle", "spot", "filament", "sultan", "classic", "vintage", "flood"].iter().any(|x| a.contains(x)) {
        FormaLuz::Foco
    } else {
        FormaLuz::Lampara
    }
}

/// Mireds a Kelvin (más mireds, más cálido).
fn kelvin_de_mireds(min: u32, max: u32) -> (u32, u32) {
    (1_000_000 / max.max(1), 1_000_000 / min.max(1))
}

pub fn estado_de(s: &Value, kelvin: Option<(u32, u32)>) -> EstadoAparato {
    let num = |k: &str| s.get(k).and_then(Value::as_i64);
    let brillo = num("bri").map(|b| brillo_de_rango(b, 1, 254));
    let mut e = EstadoAparato { encendido: s.get("on").and_then(Value::as_bool), brillo, ..Default::default() };
    match s.get("colormode").and_then(Value::as_str) {
        Some("hs") | Some("xy") => {
            let h = (num("hue").unwrap_or(0) * 360 + 32768) / 65536;
            let sat = (num("sat").unwrap_or(0) * 100 + 127) / 254;
            e.color = Some(Hsv::nuevo(h, sat, brillo.unwrap_or(100) as i64));
            e.modo = Some(ModoLuz::Color);
        }
        Some("ct") => {
            if let (Some(ct), Some((min, max))) = (num("ct"), kelvin) {
                e.temperatura = Some(de_kelvin(1_000_000 / ct.max(1) as u32, min, max));
            }
            e.modo = Some(ModoLuz::Blanco);
        }
        _ => {
            if brillo.is_some() {
                e.modo = Some(ModoLuz::Blanco);
            }
        }
    }
    e
}

pub fn luces_de(v: &Value) -> Vec<Luz> {
    let Some(o) = v.as_object() else { return Vec::new() };
    let mut lista: Vec<Luz> = o
        .iter()
        .filter_map(|(numero, l)| {
            let tipo = l.get("type").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
            let texto = |k: &str| l.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
            let (clase, capacidades) = if tipo.contains("plug") || tipo.contains("on/off") {
                (Clase::Enchufe, Capacidades::enchufe())
            } else if tipo == "extended color light" {
                (Clase::Luz, Capacidades::luz(true, true, true))
            } else if tipo == "color light" {
                (Clase::Luz, Capacidades::luz(true, false, true))
            } else if tipo == "color temperature light" {
                (Clase::Luz, Capacidades::luz(true, true, false))
            } else if tipo == "dimmable light" {
                (Clase::Luz, Capacidades::luz(true, false, false))
            } else if l.get("state").and_then(|s| s.get("on")).is_some() {
                (Clase::Luz, Capacidades::luz(l.pointer("/state/bri").is_some(), l.pointer("/state/ct").is_some(), l.pointer("/state/hue").is_some()))
            } else {
                return None;
            };
            let mireds = l
                .pointer("/capabilities/control/ct")
                .and_then(|ct| Some((ct.get("min")?.as_u64()? as u32, ct.get("max")?.as_u64()? as u32)))
                .filter(|(a, b)| *a > 0 && b > a)
                .unwrap_or(MIREDS);
            let kelvin = capacidades.temperatura.then(|| kelvin_de_mireds(mireds.0, mireds.1));
            let arquetipo = l.pointer("/config/archetype").and_then(Value::as_str).unwrap_or("");
            let estado = l.get("state").map(|s| estado_de(s, kelvin)).unwrap_or_default();
            Some(Luz {
                numero: numero.clone(),
                unico: texto("uniqueid").map(|u| u.to_ascii_lowercase().chars().filter(|c| c.is_ascii_hexdigit() || *c == '-').collect()),
                nombre: texto("name").unwrap_or("Luz Hue").chars().take(40).collect(),
                clase,
                forma: (clase == Clase::Luz).then(|| forma_de(arquetipo, texto("productname").unwrap_or(""))),
                capacidades,
                kelvin,
                modelo: texto("productname").or(texto("modelid")).map(str::to_string),
                alcanzable: l.pointer("/state/reachable").and_then(Value::as_bool).unwrap_or(true),
                estado,
            })
        })
        .collect();
    lista.sort_by_key(|l| l.numero.parse::<u32>().unwrap_or(u32::MAX));
    lista
}

/// Las habitaciones del puente: (nombre, números de sus luces).
pub type Salas = Vec<(String, Vec<String>)>;

pub fn salas_de(v: &Value) -> Salas {
    let Some(o) = v.as_object() else { return Vec::new() };
    o.values()
        .filter(|g| g.get("type").and_then(Value::as_str) == Some("Room"))
        .filter_map(|g| {
            let nombre = g.get("name")?.as_str()?.trim().chars().take(40).collect::<String>();
            let luces = g.get("lights")?.as_array()?.iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
            (!nombre.is_empty()).then_some((nombre, luces))
        })
        .collect()
}

/// El cuerpo de `PUT …/lights/<n>/state`.
pub fn cuerpo(c: &Cambio, kelvin: Option<(u32, u32)>) -> Value {
    let mut p = serde_json::Map::new();
    if let Some(on) = c.encendido {
        p.insert("on".into(), json!(on));
    }
    if c.encendido != Some(false) {
        if let Some(color) = c.color {
            p.insert("hue".into(), json!(((color.h % 360) as u32 * 65536 / 360).min(65535)));
            p.insert("sat".into(), json!((color.s.min(100) as u32 * 254 + 50) / 100));
            p.insert("bri".into(), json!(brillo_a_rango(color.v.max(1), 1, 254)));
        } else {
            if let (Some(t), Some((min, max))) = (c.temperatura, kelvin) {
                p.insert("ct".into(), json!((1_000_000 / a_kelvin(t, min, max).max(1)).clamp(MIREDS.0 - 10, MIREDS.1 + 50)));
            }
            if let Some(b) = c.brillo {
                p.insert("bri".into(), json!(brillo_a_rango(b, 1, 254)));
            }
        }
    }
    Value::Object(p)
}

/// Luces y habitaciones (dos preguntas al puente).
pub fn leer_todo(p: &Puente, clave: &str) -> Result<(Vec<Luz>, Salas), String> {
    let base = format!("/api/{}", web::codificar(clave));
    let (luces, _) = pedir(p, "GET", &format!("{base}/lights"), None)?;
    sin_errores(&luces)?;
    let salas = pedir(p, "GET", &format!("{base}/groups"), None).map(|(g, _)| salas_de(&g)).unwrap_or_default();
    Ok((luces_de(&luces), salas))
}

pub fn cambiar(p: &Puente, clave: &str, numero: &str, kelvin: Option<(u32, u32)>, c: &Cambio) -> Result<(), String> {
    let ruta = format!("/api/{}/lights/{}/state", web::codificar(clave), web::codificar(numero));
    let (v, _) = pedir(p, "PUT", &ruta, Some(&cuerpo(c, kelvin)))?;
    sin_errores(&v)
}

/// El puente encontrado por mDNS (`bridgeid` y `modelid` en su TXT).
pub fn hallazgo(ip: Ipv4Addr, bridgeid: Option<&str>, modelo: Option<&str>) -> Hallazgo {
    let id = bridgeid.map(|b| b.trim().to_ascii_lowercase()).filter(|b| !b.is_empty() && b.chars().all(|c| c.is_ascii_hexdigit()));
    Hallazgo {
        id: format!("hue:{}", id.unwrap_or_else(|| ip.to_string().replace('.', "-"))),
        via: Via::Wifi,
        proveedor: Proveedor::Hue,
        clase: Clase::Otro,
        forma: None,
        marca: "Philips Hue".into(),
        nombre: "Puente Hue".into(),
        modelo: modelo.map(str::to_string),
        direccion: Some(ip.to_string()),
        mac: None,
        senal: None,
        version: None,
        capacidades: Capacidades::ninguna(),
        estado: None,
        manejo: Manejo::Vincular {
            forma: FormaVinculo::Boton,
            paso: "Presiona el botón redondo del puente Hue. Nexo agrega sus luces solo.".into(),
        },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lan_hue_vincular() {
        assert_eq!(vinculo_de(&json!([{"success":{"username":"83b7780291a6ceffbe0bd049104df"}}])), Ok(Vinculo::Clave("83b7780291a6ceffbe0bd049104df".into())));
        assert_eq!(vinculo_de(&json!([{"error":{"type":101,"address":"","description":"link button not pressed"}}])), Ok(Vinculo::Esperando));
        assert!(vinculo_de(&json!([{"error":{"type":7,"description":"invalid value"}}])).is_err());
        assert!(vinculo_de(&json!([{"success":{"username":"../x"}}])).is_err());
        assert!(sin_errores(&json!([{"success":{"/lights/1/state/on":true}}])).is_ok());
        assert_eq!(sin_errores(&json!([{"error":{"type":1}}])), Err("El puente Hue ya no acepta a Nexo. Quítalo y vincúlalo de nuevo.".into()));
        let (id, nombre, mac, modelo) = identidad(&json!({"name":"Philips hue","bridgeid":"001788FFFE23BFC2","mac":"00:17:88:23:bf:c2","modelid":"BSB002","apiversion":"1.65.0"}))
            .unwrap_or_default();
        assert_eq!((id.as_str(), nombre.as_str(), mac.as_deref(), modelo.as_deref()), ("001788fffe23bfc2", "Philips hue", Some("00:17:88:23:bf:c2"), Some("BSB002")));
        assert!(!Puente::nuevo(Ipv4Addr::new(192, 168, 1, 2), Some("BSB001"), None).seguro);
        assert!(Puente::nuevo(Ipv4Addr::new(192, 168, 1, 2), Some("BSB002"), None).seguro);
    }

    #[test]
    fn lan_hue_luces_y_salas() {
        let v = json!({
            "1": {"state":{"on":true,"bri":254,"hue":21845,"sat":254,"ct":153,"colormode":"hs","reachable":true},
                  "type":"Extended color light","name":"Tira TV","modelid":"LST002","productname":"Hue lightstrip plus",
                  "uniqueid":"00:17:88:01:00:f0:71:18-0b","config":{"archetype":"huelightstrip"},
                  "capabilities":{"control":{"ct":{"min":153,"max":500}}}},
            "2": {"state":{"on":false,"bri":127,"ct":366,"colormode":"ct","reachable":false},"type":"Color temperature light","name":"Foco sala",
                  "uniqueid":"00:17:88:01:02:aa:bb:cc-0b","config":{"archetype":"classicbulb"},"capabilities":{"control":{"ct":{"min":153,"max":454}}}},
            "10": {"state":{"on":true,"reachable":true},"type":"On/Off plug-in unit","name":"Enchufe","uniqueid":"00:17:88:01:08:11:22:33-0b","config":{"archetype":"plug"}}
        });
        let l = luces_de(&v);
        assert_eq!(l.iter().map(|x| x.numero.as_str()).collect::<Vec<_>>(), ["1", "2", "10"]);
        assert_eq!((l[0].forma, l[0].capacidades.color, l[0].unico.as_deref()), (Some(FormaLuz::Tira), true, Some("0017880100f07118-0b")));
        assert_eq!(l[0].estado.color.map(|c| (c.h, c.s)), Some((120, 100)));
        assert_eq!((l[1].forma, l[1].alcanzable, l[1].kelvin), (Some(FormaLuz::Foco), false, Some((2202, 6535))));
        assert_eq!((l[1].estado.modo, l[1].estado.brillo), (Some(ModoLuz::Blanco), Some(50)));
        assert_eq!((l[2].clase, l[2].forma), (Clase::Enchufe, None));
        let salas = salas_de(&json!({"1":{"name":"Sala","type":"Room","lights":["1","2"]},"2":{"name":"Zona TV","type":"Zone","lights":["1"]}}));
        assert_eq!(salas, vec![("Sala".to_string(), vec!["1".to_string(), "2".to_string()])]);
    }

    #[test]
    fn lan_hue_cuerpos() {
        assert_eq!(cuerpo(&Cambio { encendido: Some(false), ..Default::default() }, None), json!({"on":false}));
        assert_eq!(
            cuerpo(&Cambio { encendido: Some(true), color: Some(Hsv { h: 180, s: 100, v: 100 }), ..Default::default() }, None),
            json!({"on":true,"hue":32768,"sat":254,"bri":254})
        );
        // Cálido del todo: el mínimo de Kelvin, el máximo de mireds.
        assert_eq!(cuerpo(&Cambio { temperatura: Some(0), brillo: Some(1), ..Default::default() }, Some((2000, 6500))), json!({"ct":500,"bri":1}));
        let h = hallazgo(Ipv4Addr::new(192, 168, 1, 2), Some("001788FFFE23BFC2"), Some("BSB002"));
        assert_eq!(h.id, "hue:001788fffe23bfc2");
        assert!(matches!(h.manejo, Manejo::Vincular { forma: FormaVinculo::Boton, .. }));
    }
}
