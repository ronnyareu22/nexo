//! Tiras WLED por la red local: API JSON sin clave (https://kno.wled.ge/interfaces/json-api/).
//!
//! - Se anuncian por mDNS como `_wled._tcp` (puerto 80; su TXT `mac` son 12 hex sin ":").
//! - `GET /json/info`: `name`, `mac`, `ver` y `leds.lc`, lo que sabe hacer la tira (bit 0 color RGB, bit 1 canal
//!   blanco, bit 2 blanco cálido a frío; los demás bits pueden venir con cualquier cosa).
//! - `GET /json/state`: `on`, `bri` (1–255) y el primer segmento con `col` ([[r,g,b(,w)]…]), `cct` (0 cálido –
//!   255 frío) y `fx` (0 es color fijo; otro, un efecto).
//! - `POST /json/state` con lo que cambia: `{"on":true,"bri":128,"seg":[{"col":[[255,0,0]],"fx":0}]}`.

use super::super::modelo::{
    Cambio, Capacidades, Clase, EstadoAparato, FormaLuz, Hallazgo, Hsv, Manejo, ModoLuz, Proveedor, Via, a_rango, brillo_a_rango,
    brillo_de_rango, de_rango, hsv_de, mac_normal, rgb_de,
};
use super::web;
use serde_json::{Value, json};
use std::net::Ipv4Addr;
use std::time::Duration;

const PLAZO: Duration = Duration::from_millis(1500);

/// Lo que sabe hacer por `leds.lc`: (color, blanco cálido a frío).
pub fn sabe(info: &Value) -> (bool, bool) {
    let lc = info.pointer("/leds/lc").and_then(Value::as_u64).unwrap_or(1) & 0x07;
    (lc & 1 != 0, lc & 4 != 0)
}

pub fn capacidades(info: &Value) -> Capacidades {
    let (color, cct) = sabe(info);
    Capacidades::luz(true, cct, color)
}

pub fn estado_de(v: &Value) -> EstadoAparato {
    let brillo = v.get("bri").and_then(Value::as_i64).map(|b| brillo_de_rango(b, 1, 255));
    let mut e = EstadoAparato { encendido: v.get("on").and_then(Value::as_bool), brillo, ..Default::default() };
    let seg = v.get("seg").and_then(Value::as_array).and_then(|s| s.first());
    let efecto = seg.and_then(|s| s.get("fx")).and_then(Value::as_u64).unwrap_or(0);
    let rgb = seg.and_then(|s| s.get("col")).and_then(Value::as_array).and_then(|c| c.first()).and_then(Value::as_array).and_then(|c| {
        Some((c.first()?.as_u64()? as u8, c.get(1)?.as_u64()? as u8, c.get(2)?.as_u64()? as u8))
    });
    if let Some(k) = seg.and_then(|s| s.get("cct")).and_then(Value::as_i64) {
        // 0–255, o Kelvin (1900–10091) si viene mayor a 255.
        let t = if k > 255 { de_rango(k, 1900, 10091) } else { de_rango(k, 0, 255) };
        e.temperatura = Some(t);
    }
    match rgb {
        Some((r, g, b)) if (r, g, b) != (0, 0, 0) => {
            let c = hsv_de(r, g, b);
            e.color = Some(Hsv { v: brillo.unwrap_or(100), ..c });
            e.modo = Some(ModoLuz::Color);
        }
        _ => e.modo = Some(ModoLuz::Blanco),
    }
    if efecto != 0 {
        e.modo = Some(ModoLuz::Escena);
    }
    e
}

/// El cuerpo de `POST /json/state`. Elegir color o blanco deja la tira en color fijo (`fx` 0).
pub fn cuerpo(c: &Cambio, info_color: bool) -> Value {
    let mut p = serde_json::Map::new();
    if let Some(on) = c.encendido {
        p.insert("on".into(), json!(on));
    }
    if c.encendido != Some(false) {
        if let Some(color) = c.color {
            let (r, g, b) = rgb_de(Hsv { v: 100, ..color });
            p.insert("bri".into(), json!(brillo_a_rango(color.v.max(1), 1, 255)));
            p.insert("seg".into(), json!([{ "col": [[r, g, b]], "fx": 0 }]));
        } else {
            if let Some(t) = c.temperatura {
                // Con color, el blanco va en los tres canales a pleno para que se vea blanco.
                let seg = if info_color { json!([{ "cct": a_rango(t, 0, 255), "col": [[255, 255, 255, 255]], "fx": 0 }]) } else { json!([{ "cct": a_rango(t, 0, 255) }]) };
                p.insert("seg".into(), seg);
            }
            if let Some(b) = c.brillo {
                p.insert("bri".into(), json!(brillo_a_rango(b, 1, 255)));
            }
        }
    }
    Value::Object(p)
}

pub fn hallazgo(ip: Ipv4Addr, info: &Value, estado: Option<&Value>) -> Option<Hallazgo> {
    let mac = mac_normal(info.get("mac")?.as_str()?)?;
    let nombre = info.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).unwrap_or("WLED");
    Some(Hallazgo {
        id: format!("wled:{}", mac.replace(':', "")),
        via: Via::Wifi,
        proveedor: Proveedor::Wled,
        clase: Clase::Luz,
        forma: Some(FormaLuz::Tira),
        marca: "WLED".into(),
        nombre: nombre.chars().take(40).collect(),
        modelo: info.get("arch").and_then(Value::as_str).map(|a| format!("WLED {a}")),
        direccion: Some(ip.to_string()),
        mac: Some(mac),
        senal: None,
        version: info.get("ver").and_then(Value::as_str).map(str::to_string),
        capacidades: capacidades(info),
        estado: estado.map(estado_de),
        manejo: Manejo::Listo,
    })
}

pub fn probar(ip: Ipv4Addr) -> Option<Hallazgo> {
    let info = web::json(ip, 80, "/json/info", &[], PLAZO).ok()?;
    let estado = web::json(ip, 80, "/json/state", &[], PLAZO).ok();
    hallazgo(ip, &info, estado.as_ref())
}

pub fn leer(ip: Ipv4Addr) -> Result<EstadoAparato, String> {
    web::json(ip, 80, "/json/state", &[], PLAZO).map(|v| estado_de(&v))
}

pub fn cambiar(ip: Ipv4Addr, con_color: bool, c: &Cambio) -> Result<(), String> {
    let cuerpo = serde_json::to_vec(&cuerpo(c, con_color)).unwrap_or_default();
    let r = web::pedir(ip, 80, "POST", "/json/state", &[], Some(&cuerpo), PLAZO)?;
    let ok = r.ok() && r.json().and_then(|v| v.get("success").and_then(Value::as_bool)).unwrap_or(true);
    if ok { Ok(()) } else { Err("La tira WLED no aceptó el cambio.".into()) }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lan_wled_lo_que_sabe() {
        assert_eq!(sabe(&json!({"leds":{"lc":1}})), (true, false));
        assert_eq!(sabe(&json!({"leds":{"lc":7}})), (true, true));
        // Bits altos con cualquier cosa: solo cuentan los tres primeros.
        assert_eq!(sabe(&json!({"leds":{"lc":0xF3}})), (true, false));
        assert_eq!(capacidades(&json!({"leds":{"lc":6}})), Capacidades::luz(true, true, false));
        let info = json!({"ver":"0.15.0","name":"Escritorio","mac":"60019423b441","arch":"esp32","leds":{"count":60,"lc":1}});
        let h = hallazgo(Ipv4Addr::new(192, 168, 1, 80), &info, None).unwrap_or_else(|| panic!("sin hallazgo"));
        assert_eq!((h.id.as_str(), h.nombre.as_str(), h.mac.as_deref()), ("wled:60019423b441", "Escritorio", Some("60:01:94:23:b4:41")));
    }

    #[test]
    fn lan_wled_estado_y_cuerpo() {
        let e = estado_de(&json!({"on":true,"bri":128,"seg":[{"id":0,"col":[[255,0,0],[0,0,0],[0,0,0]],"fx":0,"cct":127}]}));
        assert_eq!((e.encendido, e.brillo, e.modo), (Some(true), Some(51), Some(ModoLuz::Color)));
        assert_eq!(e.color.map(|c| (c.h, c.s)), Some((0, 100)));
        let efecto = estado_de(&json!({"on":true,"bri":255,"seg":[{"col":[[0,0,255]],"fx":9}]}));
        assert_eq!(efecto.modo, Some(ModoLuz::Escena));
        assert_eq!(cuerpo(&Cambio { encendido: Some(false), ..Default::default() }, true), json!({"on":false}));
        assert_eq!(
            cuerpo(&Cambio { encendido: Some(true), color: Some(Hsv { h: 120, s: 100, v: 100 }), ..Default::default() }, true),
            json!({"on":true,"bri":255,"seg":[{"col":[[0,255,0]],"fx":0}]})
        );
        assert_eq!(cuerpo(&Cambio { temperatura: Some(100), brillo: Some(1), ..Default::default() }, false), json!({"seg":[{"cct":255}],"bri":1}));
    }
}
