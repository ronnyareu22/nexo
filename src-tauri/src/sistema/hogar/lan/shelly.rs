//! Shelly por la red local (HTTP), según https://shelly-api-docs.shelly.cloud (gen1 y gen2).
//!
//! - Quién es: `GET /shelly` (nunca pide clave). Gen1 trae `type`, `mac` y `auth`; Gen2 en adelante trae `gen`,
//!   `mac`, `model`, `app` y `auth_en`.
//! - Gen1 (REST): `/settings` dice sus salidas (`relays`, `lights`, `mode`); se maneja con `/relay/N?turn=on`,
//!   `/light/0?turn=on&brightness=…&temp=…` (Bulb, Duo, Vintage, Dimmer), `/color/0?red=…&gain=…` y `/white/N`
//!   (RGBW2). Con contraseña: Basic, usuario `admin`.
//! - Gen2 en adelante (JSON-RPC por `POST /rpc`): `Shelly.GetStatus` lista los componentes (`switch:0`,
//!   `light:0`, `rgb:0`, `rgbw:0`, `cct:0`, `rgbcct:0`); `Switch.Set`, `Light.Set`, `RGB.Set`, `RGBW.Set`,
//!   `CCT.Set` y `RGBCCT.Set` los manejan. Con contraseña: Digest con SHA-256, usuario `admin` (RFC 7616).
//! - Cada salida es un aparato en Nexo (`canal`: "relay/1" en Gen1, "switch:1" en Gen2).

use super::super::modelo::{
    Cambio, Capacidades, Clase, EstadoAparato, FormaLuz, FormaVinculo, Hallazgo, Hsv, Manejo, ModoLuz, Proveedor, Via, a_kelvin,
    de_kelvin, hsv_de, mac_normal, rgb_de,
};
use super::web;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Mutex;
use std::time::Duration;

const PLAZO: Duration = Duration::from_millis(1800);
const USUARIO: &str = "admin";
/// El rango de blancos de fábrica (CCT de Gen2, Duo de Gen1).
const KELVIN: (u32, u32) = (2700, 6500);
/// Bulb y Bulb RGBW de Gen1 en blanco.
const KELVIN_BULB: (u32, u32) = (3000, 6500);

/// El último reto Digest de cada Shelly (IP → reto y cuenta), para no pedirlo en cada orden.
static RETOS: Mutex<Option<HashMap<Ipv4Addr, (web::Reto, u32)>>> = Mutex::new(None);

/// Lo que dice `/shelly`.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub generacion: u8,
    /// "a8:03:2a:b1:c2:d3"
    pub mac: String,
    /// Gen1: el tipo ("SHPLG-S"); Gen2: la app ("PlusPlugS") o el modelo.
    pub modelo: String,
    pub nombre: Option<String>,
    pub clave: bool,
}

pub fn info_de(v: &Value) -> Option<Info> {
    let mac = mac_normal(v.get("mac")?.as_str()?)?;
    let texto = |k: &str| v.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if let Some(g) = v.get("gen").and_then(Value::as_u64) {
        return Some(Info {
            generacion: g.clamp(2, 9) as u8,
            mac,
            modelo: texto("app").or_else(|| texto("model")).unwrap_or_else(|| "Shelly".into()),
            nombre: texto("name"),
            clave: v.get("auth_en").and_then(Value::as_bool).unwrap_or(false),
        });
    }
    Some(Info {
        generacion: 1,
        mac,
        modelo: texto("type")?,
        nombre: None,
        clave: v.get("auth").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn hex(mac: &str) -> String {
    mac.replace(':', "")
}

/// Qué parece por su modelo (antes de ver sus salidas).
fn clase_por_modelo(modelo: &str) -> (Clase, Option<FormaLuz>) {
    let m = modelo.to_ascii_lowercase();
    if m.contains("plug") || m.starts_with("shplg") {
        (Clase::Enchufe, None)
    } else if ["bulb", "duo", "vintage", "shblb", "shcb", "shbduo", "shvin"].iter().any(|x| m.contains(x)) {
        (Clase::Luz, Some(FormaLuz::Foco))
    } else if ["rgbw", "dimmer", "shdm", "light", "cct", "rgb"].iter().any(|x| m.contains(x)) {
        (Clase::Luz, Some(FormaLuz::Tira))
    } else {
        (Clase::Enchufe, None)
    }
}

pub fn hallazgo(ip: Ipv4Addr, i: &Info) -> Hallazgo {
    let (clase, forma) = clase_por_modelo(&i.modelo);
    let manejo = if i.clave {
        Manejo::Vincular { forma: FormaVinculo::Clave, paso: "Este Shelly tiene contraseña. Escríbela para agregarlo.".into() }
    } else {
        Manejo::Listo
    };
    Hallazgo {
        id: format!("shelly:{}", hex(&i.mac)),
        via: Via::Wifi,
        proveedor: Proveedor::Shelly,
        clase,
        forma,
        marca: "Shelly".into(),
        nombre: i.nombre.clone().unwrap_or_else(|| format!("Shelly {}", i.modelo)),
        modelo: Some(i.modelo.clone()),
        direccion: Some(ip.to_string()),
        mac: Some(i.mac.clone()),
        senal: None,
        version: Some(format!("Gen{}", i.generacion)),
        capacidades: if clase == Clase::Luz { Capacidades::luz(true, false, false) } else { Capacidades::enchufe() },
        estado: None,
        manejo,
    }
}

/// Una salida del Shelly que Nexo maneja como un aparato.
#[derive(Debug, Clone, PartialEq)]
pub struct Salida {
    /// "relay/0", "light/0", "color/0", "white/2" (Gen1) o "switch:0", "light:0", "rgbw:0"… (Gen2).
    pub canal: String,
    pub nombre: Option<String>,
    pub clase: Clase,
    pub forma: Option<FormaLuz>,
    pub capacidades: Capacidades,
    pub kelvin: Option<(u32, u32)>,
}

fn salida(canal: String, nombre: Option<String>, clase: Clase, capacidades: Capacidades, kelvin: Option<(u32, u32)>) -> Salida {
    let forma = (clase == Clase::Luz).then_some(FormaLuz::Tira);
    Salida { canal, nombre: nombre.filter(|n| !n.trim().is_empty()), clase, forma, capacidades, kelvin }
}

/// Las salidas de un Gen2 por `Shelly.GetStatus` (y sus nombres por `Shelly.GetConfig`, si llegó).
/// Un relé (`switch`) va como enchufe: se enciende y se apaga, sea un enchufe o la luz del techo.
pub fn salidas_gen2(estado: &Value, config: Option<&Value>) -> Vec<Salida> {
    let Some(o) = estado.as_object() else { return Vec::new() };
    let mut v: Vec<Salida> = o
        .keys()
        .filter_map(|k| {
            let (tipo, _) = k.split_once(':')?;
            let nombre = config.and_then(|c| c.get(k)).and_then(|c| c.get("name")).and_then(Value::as_str).map(str::to_string);
            let rango = config
                .and_then(|c| c.get(k))
                .and_then(|c| c.get("ct_range"))
                .and_then(Value::as_array)
                .and_then(|r| Some((r.first()?.as_u64()? as u32, r.get(1)?.as_u64()? as u32)))
                .filter(|(a, b)| b > a)
                .or(Some(KELVIN));
            Some(match tipo {
                "switch" => salida(k.clone(), nombre, Clase::Enchufe, Capacidades::enchufe(), None),
                "light" => salida(k.clone(), nombre, Clase::Luz, Capacidades::luz(true, false, false), None),
                "rgb" | "rgbw" => salida(k.clone(), nombre, Clase::Luz, Capacidades::luz(true, false, true), None),
                "cct" => salida(k.clone(), nombre, Clase::Luz, Capacidades::luz(true, true, false), rango),
                "rgbcct" => salida(k.clone(), nombre, Clase::Luz, Capacidades::luz(true, true, true), rango),
                _ => return None,
            })
        })
        .collect();
    v.sort_by(|a, b| a.canal.cmp(&b.canal));
    v
}

/// Las salidas de un Gen1 por su tipo y `/settings`.
pub fn salidas_gen1(tipo: &str, ajustes: &Value) -> Vec<Salida> {
    let t = tipo.to_ascii_uppercase();
    let nombres = |k: &str| -> Vec<Option<String>> {
        ajustes
            .get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|x| x.get("name").and_then(Value::as_str).map(str::to_string)).collect())
            .unwrap_or_default()
    };
    let modo = ajustes.get("mode").and_then(Value::as_str).unwrap_or("");
    let mut v = Vec::new();
    if t.starts_with("SHRGBW2") {
        let luces = nombres("lights");
        if modo == "white" {
            for (i, n) in luces.into_iter().enumerate().take(4) {
                v.push(salida(format!("white/{i}"), n, Clase::Luz, Capacidades::luz(true, false, false), None));
            }
        } else {
            v.push(salida("color/0".into(), luces.into_iter().next().flatten(), Clase::Luz, Capacidades::luz(true, false, true), None));
        }
        return v;
    }
    let luz = nombres("lights").into_iter().next().flatten();
    let foco = |mut s: Salida| {
        s.forma = Some(FormaLuz::Foco);
        s
    };
    if t.starts_with("SHBLB") || t.starts_with("SHCB") {
        v.push(foco(salida("light/0".into(), luz, Clase::Luz, Capacidades::luz(true, true, true), Some(KELVIN_BULB))));
    } else if t.starts_with("SHBDUO") {
        v.push(foco(salida("light/0".into(), luz, Clase::Luz, Capacidades::luz(true, true, false), Some(KELVIN))));
    } else if t.starts_with("SHVIN") {
        v.push(foco(salida("light/0".into(), luz, Clase::Luz, Capacidades::luz(true, false, false), None)));
    } else if t.starts_with("SHDM") {
        v.push(salida("light/0".into(), luz, Clase::Luz, Capacidades::luz(true, false, false), None));
    } else if !(t.starts_with("SHSW-2") && modo == "roller") {
        for (i, n) in nombres("relays").into_iter().enumerate().take(4) {
            v.push(salida(format!("relay/{i}"), n, Clase::Enchufe, Capacidades::enchufe(), None));
        }
    }
    v
}

/// Cómo está una salida, por lo que contestó (`<Comp>.GetStatus` en Gen2; `/relay/N`, `/light/0`… en Gen1).
pub fn estado_de(canal: &str, v: &Value, kelvin: Option<(u32, u32)>) -> EstadoAparato {
    let num = |k: &str| v.get(k).and_then(Value::as_f64);
    let encendido = v.get("output").or_else(|| v.get("ison")).and_then(Value::as_bool);
    let brillo = num("brightness").map(|b| b.round().clamp(1.0, 100.0) as u8);
    let mut e = EstadoAparato { encendido, ..Default::default() };
    if canal.starts_with("relay") || canal.starts_with("switch") {
        return e;
    }
    e.brillo = brillo;
    let rgb = v.get("rgb").and_then(Value::as_array).and_then(|a| {
        Some((a.first()?.as_u64()? as u8, a.get(1)?.as_u64()? as u8, a.get(2)?.as_u64()? as u8))
    });
    let rgb = rgb.or_else(|| Some((num("red")? as u8, num("green")? as u8, num("blue")? as u8)));
    let en_color = match v.get("mode").and_then(Value::as_str) {
        Some("color") | Some("rgb") => true,
        Some(_) => false,
        None => canal.starts_with("rgb") || canal.starts_with("color/"),
    };
    if en_color && let Some((r, g, b)) = rgb {
        // Gen1 da el brillo del color en `gain`.
        let v_ = num("gain").map(|g| g.round().clamp(1.0, 100.0) as u8).or(brillo).unwrap_or(100);
        let c = hsv_de(r, g, b);
        e.color = Some(Hsv { v: v_, ..c });
        e.brillo = Some(v_);
        e.modo = Some(ModoLuz::Color);
    } else if let (Some(k), Some((min, max))) = (num("ct").or_else(|| num("temp")), kelvin) {
        e.temperatura = Some(de_kelvin(k as u32, min, max));
        e.modo = Some(ModoLuz::Blanco);
    } else if brillo.is_some() {
        e.modo = Some(ModoLuz::Blanco);
    }
    e
}

/// La orden de Gen2: (método, parámetros).
pub fn orden_gen2(canal: &str, c: &Cambio, kelvin: Option<(u32, u32)>) -> Option<(String, Value)> {
    let (tipo, id) = canal.split_once(':')?;
    let id: u32 = id.parse().ok()?;
    let metodo = match tipo {
        "switch" => "Switch.Set",
        "light" => "Light.Set",
        "rgb" => "RGB.Set",
        "rgbw" => "RGBW.Set",
        "cct" => "CCT.Set",
        "rgbcct" => "RGBCCT.Set",
        _ => return None,
    };
    let mut p = serde_json::Map::new();
    p.insert("id".into(), json!(id));
    if let Some(on) = c.encendido {
        p.insert("on".into(), json!(on));
    }
    if c.encendido != Some(false) && tipo != "switch" {
        if let Some(color) = c.color.filter(|_| matches!(tipo, "rgb" | "rgbw" | "rgbcct")) {
            let (r, g, b) = rgb_de(Hsv { v: 100, ..color });
            p.insert("rgb".into(), json!([r, g, b]));
            p.insert("brightness".into(), json!(color.v.clamp(1, 100)));
            if tipo == "rgbcct" {
                p.insert("mode".into(), json!("rgb"));
            }
        } else {
            if let (Some(t), Some((min, max))) = (c.temperatura.filter(|_| matches!(tipo, "cct" | "rgbcct")), kelvin) {
                p.insert("ct".into(), json!(a_kelvin(t, min, max)));
                if tipo == "rgbcct" {
                    p.insert("mode".into(), json!("cct"));
                }
            }
            if let Some(b) = c.brillo {
                p.insert("brightness".into(), json!(b.clamp(1, 100)));
            }
        }
    }
    (p.len() > 1).then(|| (metodo.to_string(), Value::Object(p)))
}

/// La ruta de Gen1 con su consulta ("/light/0?turn=on&brightness=40").
pub fn ruta_gen1(canal: &str, c: &Cambio, kelvin: Option<(u32, u32)>, tipo: &str) -> String {
    let mut q: Vec<String> = Vec::new();
    if let Some(on) = c.encendido {
        q.push(format!("turn={}", if on { "on" } else { "off" }));
    }
    let t = tipo.to_ascii_uppercase();
    if c.encendido != Some(false) && !canal.starts_with("relay") {
        if let Some(color) = c.color.filter(|_| canal.starts_with("color") || t.starts_with("SHBLB") || t.starts_with("SHCB")) {
            let (r, g, b) = rgb_de(Hsv { v: 100, ..color });
            if canal.starts_with("light") {
                q.push("mode=color".into());
            }
            q.push(format!("red={r}&green={g}&blue={b}&white=0&gain={}", color.v.clamp(1, 100)));
        } else {
            if let (Some(tk), Some((min, max))) = (c.temperatura, kelvin) {
                if t.starts_with("SHBLB") || t.starts_with("SHCB") {
                    q.push("mode=white".into());
                }
                q.push(format!("temp={}", a_kelvin(tk, min, max)));
            }
            if let Some(b) = c.brillo {
                if canal.starts_with("color") {
                    q.push(format!("gain={}", b.clamp(1, 100)));
                } else {
                    q.push(format!("brightness={}", b.clamp(1, 100)));
                }
            }
        }
    }
    if q.is_empty() { format!("/{canal}") } else { format!("/{canal}?{}", q.join("&")) }
}

// ---------------------------------------------------------------- Por la red

/// `GET /shelly`: quién es (sin clave).
pub fn probar(ip: Ipv4Addr) -> Option<Info> {
    web::json(ip, 80, "/shelly", &[], Duration::from_millis(1200)).ok().as_ref().and_then(info_de)
}

/// GET de Gen1 con Basic si hay clave.
fn gen1(ip: Ipv4Addr, ruta: &str, clave: Option<&str>) -> Result<Value, String> {
    let cab: Vec<(&str, String)> = clave.map(|c| vec![("Authorization", web::basic(USUARIO, c))]).unwrap_or_default();
    web::json(ip, 80, ruta, &cab, PLAZO)
}

/// Una llamada RPC de Gen2 (`POST /rpc`). Con clave: Digest SHA-256, reusando el último reto de ese Shelly.
fn rpc(ip: Ipv4Addr, metodo: &str, params: Value, clave: Option<&str>) -> Result<Value, String> {
    let cuerpo = serde_json::to_vec(&json!({ "id": 1, "method": metodo, "params": params })).unwrap_or_default();
    let mandar = |cab: &[(&str, String)]| web::pedir(ip, 80, "POST", "/rpc", cab, Some(&cuerpo), PLAZO);
    let guardado = clave.and_then(|_| {
        let mut g = RETOS.lock().unwrap_or_else(|e| e.into_inner());
        let mapa = g.get_or_insert_with(HashMap::new);
        mapa.get_mut(&ip).map(|(r, n)| {
            *n += 1;
            (r.clone(), *n)
        })
    });
    let mut r = match (&guardado, clave) {
        (Some((reto, nc)), Some(c)) => mandar(&[("Authorization", web::digest(USUARIO, c, reto, "POST", "/rpc", *nc))])?,
        _ => mandar(&[])?,
    };
    if r.estado == 401 {
        let c = clave.ok_or(web::CLAVE)?;
        let reto = r.cabecera("www-authenticate").and_then(web::reto).ok_or(web::CLAVE)?;
        r = mandar(&[("Authorization", web::digest(USUARIO, c, &reto, "POST", "/rpc", 1))])?;
        if r.estado == 401 {
            return Err(web::CLAVE.into());
        }
        RETOS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(ip, (reto, 1));
    }
    let v = r.json().ok_or("El Shelly contestó algo que no entiendo.")?;
    if let Some(e) = v.get("error") {
        if e.get("code").and_then(Value::as_i64) == Some(401) {
            return Err(web::CLAVE.into());
        }
        return Err(format!("El Shelly no aceptó la orden ({}).", e.get("message").and_then(Value::as_str).unwrap_or("error")));
    }
    Ok(v.get("result").cloned().unwrap_or(Value::Null))
}

/// Las salidas que maneja Nexo (pide la clave si el Shelly la tiene).
pub fn salidas(ip: Ipv4Addr, i: &Info, clave: Option<&str>) -> Result<Vec<Salida>, String> {
    let v = if i.generacion >= 2 {
        let estado = rpc(ip, "Shelly.GetStatus", json!({}), clave)?;
        let config = rpc(ip, "Shelly.GetConfig", json!({}), clave).ok();
        salidas_gen2(&estado, config.as_ref())
    } else {
        let ajustes = gen1(ip, "/settings", clave)?;
        salidas_gen1(&i.modelo, &ajustes)
    };
    if v.is_empty() { Err("Este Shelly no tiene salidas que Nexo maneje todavía (por ejemplo, una cortina).".into()) } else { Ok(v) }
}

/// Cómo está una salida.
pub fn leer(ip: Ipv4Addr, canal: &str, clave: Option<&str>, kelvin: Option<(u32, u32)>) -> Result<EstadoAparato, String> {
    let v = match canal.split_once(':') {
        Some((tipo, id)) => {
            let metodo = match tipo {
                "switch" => "Switch.GetStatus",
                "light" => "Light.GetStatus",
                "rgb" => "RGB.GetStatus",
                "rgbw" => "RGBW.GetStatus",
                "cct" => "CCT.GetStatus",
                "rgbcct" => "RGBCCT.GetStatus",
                _ => return Err("Nexo no maneja esa salida del Shelly.".into()),
            };
            rpc(ip, metodo, json!({ "id": id.parse::<u32>().unwrap_or(0) }), clave)?
        }
        None => gen1(ip, &format!("/{canal}"), clave)?,
    };
    Ok(estado_de(canal, &v, kelvin))
}

pub fn cambiar(ip: Ipv4Addr, canal: &str, tipo: &str, clave: Option<&str>, kelvin: Option<(u32, u32)>, c: &Cambio) -> Result<(), String> {
    if canal.contains(':') {
        let (metodo, params) = orden_gen2(canal, c, kelvin).ok_or("Ese Shelly no acepta ese cambio.")?;
        rpc(ip, &metodo, params, clave).map(|_| ())
    } else {
        gen1(ip, &ruta_gen1(canal, c, kelvin, tipo), clave).map(|_| ())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lan_shelly_quien_es() {
        // Gen1 (ejemplo de la documentación) y Gen2.
        let g1 = info_de(&json!({"type":"SHSW-21","mac":"5ECF7F1632E8","auth":true,"fw":"20161223-111304/master@2bc16496","longid":1}))
            .unwrap_or_else(|| panic!("sin info"));
        assert_eq!((g1.generacion, g1.mac.as_str(), g1.modelo.as_str(), g1.clave), (1, "5e:cf:7f:16:32:e8", "SHSW-21", true));
        let g2 = info_de(&json!({"id":"shellyplusplugs-a8032ab1c2d3","mac":"A8032AB1C2D3","model":"SNPL-00112EU","gen":2,"app":"PlusPlugS","auth_en":false,"name":null}))
            .unwrap_or_else(|| panic!("sin info"));
        assert_eq!((g2.generacion, g2.modelo.as_str(), g2.clave), (2, "PlusPlugS", false));
        let h = hallazgo(Ipv4Addr::new(192, 168, 1, 50), &g2);
        assert_eq!((h.id.as_str(), h.clase, h.manejo.clone()), ("shelly:a8032ab1c2d3", Clase::Enchufe, Manejo::Listo));
        assert!(matches!(hallazgo(Ipv4Addr::new(192, 168, 1, 51), &g1).manejo, Manejo::Vincular { forma: FormaVinculo::Clave, .. }));
        assert_eq!(info_de(&json!({"mac":"x"})), None);
    }

    #[test]
    fn lan_shelly_salidas() {
        let estado = json!({"sys":{},"wifi":{},"switch:0":{"output":true},"switch:1":{"output":false},"input:0":{}});
        let config = json!({"switch:0":{"name":"Lámpara"},"switch:1":{"name":null}});
        let s = salidas_gen2(&estado, Some(&config));
        assert_eq!(s.iter().map(|x| x.canal.as_str()).collect::<Vec<_>>(), ["switch:0", "switch:1"]);
        assert_eq!((s[0].nombre.as_deref(), s[1].nombre.as_deref()), (Some("Lámpara"), None));
        let cct = salidas_gen2(&json!({"cct:0":{}}), Some(&json!({"cct:0":{"ct_range":[3000,6000]}})));
        assert_eq!((cct[0].capacidades.temperatura, cct[0].kelvin), (true, Some((3000, 6000))));
        let rgbw2 = salidas_gen1("SHRGBW2", &json!({"mode":"white","lights":[{"name":"A"},{},{},{}]}));
        assert_eq!(rgbw2.len(), 4);
        assert_eq!(rgbw2[0].canal, "white/0");
        let bulb = salidas_gen1("SHCB-1", &json!({"mode":"color","lights":[{"name":null}]}));
        assert_eq!((bulb[0].canal.as_str(), bulb[0].capacidades.color, bulb[0].forma), ("light/0", true, Some(FormaLuz::Foco)));
        let rele = salidas_gen1("SHSW-25", &json!({"mode":"relay","relays":[{"name":"Patio"},{"name":null}]}));
        assert_eq!(rele.iter().map(|x| x.canal.as_str()).collect::<Vec<_>>(), ["relay/0", "relay/1"]);
        assert!(salidas_gen1("SHSW-25", &json!({"mode":"roller","relays":[{},{}]})).is_empty());
    }

    #[test]
    fn lan_shelly_estado_y_ordenes() {
        let e = estado_de("switch:0", &json!({"id":0,"output":true,"apower":12.5}), None);
        assert_eq!((e.encendido, e.brillo), (Some(true), None));
        let rgbw = estado_de("rgbw:0", &json!({"output":true,"rgb":[255,0,0],"brightness":40,"white":0}), None);
        assert_eq!((rgbw.modo, rgbw.color), (Some(ModoLuz::Color), Some(Hsv { h: 0, s: 100, v: 40 })));
        let duo = estado_de("light/0", &json!({"ison":true,"brightness":70,"temp":2700}), Some(KELVIN));
        assert_eq!((duo.temperatura, duo.brillo), (Some(0), Some(70)));
        let (m, p) = orden_gen2("switch:1", &Cambio { encendido: Some(false), ..Default::default() }, None).unwrap_or_default();
        assert_eq!((m.as_str(), p), ("Switch.Set", json!({"id":1,"on":false})));
        let (m, p) = orden_gen2("rgbcct:0", &Cambio { encendido: Some(true), color: Some(Hsv { h: 240, s: 100, v: 30 }), ..Default::default() }, Some(KELVIN))
            .unwrap_or_default();
        assert_eq!((m.as_str(), p), ("RGBCCT.Set", json!({"id":0,"on":true,"rgb":[0,0,255],"brightness":30,"mode":"rgb"})));
        let (_, p) = orden_gen2("cct:0", &Cambio { temperatura: Some(100), brillo: Some(50), ..Default::default() }, Some(KELVIN)).unwrap_or_default();
        assert_eq!(p, json!({"id":0,"ct":6500,"brightness":50}));
        assert_eq!(ruta_gen1("relay/0", &Cambio { encendido: Some(true), ..Default::default() }, None, "SHPLG-S"), "/relay/0?turn=on");
        assert_eq!(
            ruta_gen1("light/0", &Cambio { encendido: Some(true), temperatura: Some(0), brillo: Some(40), ..Default::default() }, Some(KELVIN_BULB), "SHBLB-1"),
            "/light/0?turn=on&mode=white&temp=3000&brightness=40"
        );
        assert_eq!(
            ruta_gen1("color/0", &Cambio { color: Some(Hsv { h: 120, s: 100, v: 60 }), ..Default::default() }, None, "SHRGBW2"),
            "/color/0?red=0&green=255&blue=0&white=0&gain=60"
        );
    }
}
