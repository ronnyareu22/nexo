//! Nanoleaf (Light Panels, Canvas, Shapes, Elements, Lines y Essentials por Wi-Fi) por la red local, según la
//! "Nanoleaf Light Panels Open API" (nanoleaf.atlassian.net, espacio nlapid).
//!
//! - Se anuncian por mDNS como `_nanoleafapi._tcp` (puerto 16021 de fábrica, pero se usa el del anuncio; TXT
//!   `md` es el modelo e `id` cambia si se restablece).
//! - Vincular: se mantiene el botón de encendido de 5 a 7 s hasta que parpadea y, dentro de 30 s,
//!   `POST /api/v1/new` devuelve `{"auth_token":"…"}` (403 mientras no esté en modo de vincular). El token va al
//!   llavero.
//! - `GET /api/v1/<token>/`: `name`, `serialNo`, `model`, `firmwareVersion` y `state` (`on.value`,
//!   `brightness.value` 0–100, `hue.value` 0–360, `sat.value` 0–100, `ct.value` con su `min` y `max`, `colorMode`).
//! - `PUT /api/v1/<token>/state` con `{"on":{"value":true}}`, `{"brightness":{"value":50}}`,
//!   `{"hue":{"value":120},"sat":{"value":100}}` o `{"ct":{"value":4000}}`; contesta 204.

use super::super::modelo::{Cambio, Capacidades, EstadoAparato, Hsv, ModoLuz, a_kelvin, de_kelvin};
use super::web;
use serde_json::{Value, json};
use std::net::Ipv4Addr;
use std::time::Duration;

pub const PUERTO: u16 = 16021;
const PLAZO: Duration = Duration::from_millis(1800);
/// El rango de blancos de fábrica (Lines, Shapes, Canvas, Light Panels); se usa el que dice cada una.
pub const KELVIN: (u32, u32) = (1200, 6500);

/// El resultado de un intento de vincular.
#[derive(Debug, Clone, PartialEq)]
pub enum Vinculo {
    Token(String),
    /// Todavía no está en modo de vincular (no se mantuvo el botón).
    Esperando,
}

pub fn token_de(r: &web::Respuesta) -> Result<Vinculo, String> {
    match r.estado {
        200 | 204 => r
            .json()
            .and_then(|v| v.get("auth_token")?.as_str().map(str::to_string))
            .filter(|t| !t.is_empty() && t.len() <= 128 && t.chars().all(|c| c.is_ascii_alphanumeric()))
            .map(Vinculo::Token)
            .ok_or_else(|| "La Nanoleaf contestó sin clave.".into()),
        401 | 403 => Ok(Vinculo::Esperando),
        otro => Err(format!("La Nanoleaf contestó con un error ({otro}).")),
    }
}

/// Un intento de vincular.
pub fn vincular(ip: Ipv4Addr, puerto: u16) -> Result<Vinculo, String> {
    let r = web::pedir(ip, puerto, "POST", "/api/v1/new", &[], None, PLAZO)?;
    token_de(&r)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub nombre: String,
    pub serie: Option<String>,
    pub modelo: Option<String>,
    pub version: Option<String>,
    pub kelvin: (u32, u32),
}

pub fn info_de(v: &Value) -> Option<Info> {
    let texto = |k: &str| v.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let rango = v
        .pointer("/state/ct")
        .and_then(|ct| Some((ct.get("min")?.as_u64()? as u32, ct.get("max")?.as_u64()? as u32)))
        .filter(|(a, b)| *a >= 1000 && b > a)
        .unwrap_or(KELVIN);
    Some(Info { nombre: texto("name")?, serie: texto("serialNo"), modelo: texto("model"), version: texto("firmwareVersion"), kelvin: rango })
}

pub fn capacidades() -> Capacidades {
    Capacidades::luz(true, true, true)
}

pub fn estado_de(v: &Value, kelvin: (u32, u32)) -> EstadoAparato {
    let s = v.get("state").unwrap_or(v);
    let valor = |k: &str| s.get(k).and_then(|x| x.get("value")).and_then(Value::as_f64);
    let brillo = valor("brightness").map(|b| b.round().clamp(1.0, 100.0) as u8);
    let mut e = EstadoAparato { encendido: s.pointer("/on/value").and_then(Value::as_bool), brillo, ..Default::default() };
    match s.get("colorMode").and_then(Value::as_str) {
        Some("hs") => {
            e.color = Some(Hsv::nuevo(valor("hue").unwrap_or(0.0) as i64, valor("sat").unwrap_or(0.0) as i64, brillo.unwrap_or(100) as i64));
            e.modo = Some(ModoLuz::Color);
        }
        Some("ct") => {
            e.temperatura = valor("ct").map(|k| de_kelvin(k as u32, kelvin.0, kelvin.1));
            e.modo = Some(ModoLuz::Blanco);
        }
        Some(_) => e.modo = Some(ModoLuz::Escena),
        None => {}
    }
    e
}

/// Los cuerpos de `PUT …/state`: uno solo con todo lo que cambia.
pub fn cuerpo(c: &Cambio, kelvin: (u32, u32)) -> Value {
    let mut p = serde_json::Map::new();
    if let Some(on) = c.encendido {
        p.insert("on".into(), json!({ "value": on }));
    }
    if c.encendido != Some(false) {
        if let Some(color) = c.color {
            p.insert("hue".into(), json!({ "value": color.h % 360 }));
            p.insert("sat".into(), json!({ "value": color.s.min(100) }));
            p.insert("brightness".into(), json!({ "value": color.v.clamp(1, 100) }));
        } else {
            if let Some(t) = c.temperatura {
                p.insert("ct".into(), json!({ "value": a_kelvin(t, kelvin.0, kelvin.1) }));
            }
            if let Some(b) = c.brillo {
                p.insert("brightness".into(), json!({ "value": b.clamp(1, 100) }));
            }
        }
    }
    Value::Object(p)
}

fn ruta(token: &str, resto: &str) -> String {
    format!("/api/v1/{}/{resto}", web::codificar(token))
}

/// Su info completa (con el token).
pub fn info(ip: Ipv4Addr, puerto: u16, token: &str) -> Result<Value, String> {
    web::json(ip, puerto, &ruta(token, ""), &[], PLAZO)
}

pub fn leer(ip: Ipv4Addr, puerto: u16, token: &str, kelvin: (u32, u32)) -> Result<EstadoAparato, String> {
    web::json(ip, puerto, &ruta(token, "state"), &[], PLAZO).map(|v| estado_de(&v, kelvin))
}

pub fn cambiar(ip: Ipv4Addr, puerto: u16, token: &str, kelvin: (u32, u32), c: &Cambio) -> Result<(), String> {
    let cuerpo = serde_json::to_vec(&cuerpo(c, kelvin)).unwrap_or_default();
    let r = web::pedir(ip, puerto, "PUT", &ruta(token, "state"), &[], Some(&cuerpo), PLAZO)?;
    match r.estado {
        200..=299 => Ok(()),
        401 | 403 => Err("La Nanoleaf ya no acepta a Nexo. Quítala y vincúlala de nuevo.".into()),
        otro => Err(format!("La Nanoleaf no aceptó el cambio ({otro}).")),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn resp(estado: u16, cuerpo: &str) -> web::Respuesta {
        web::Respuesta { estado, cabeceras: Vec::new(), cuerpo: cuerpo.as_bytes().to_vec() }
    }

    #[test]
    fn lan_nanoleaf_vincular() {
        assert_eq!(token_de(&resp(200, r#"{"auth_token":"dTgL2KiPsH1hWNXnZtRk2Rr6gVtYZMB2"}"#)), Ok(Vinculo::Token("dTgL2KiPsH1hWNXnZtRk2Rr6gVtYZMB2".into())));
        assert_eq!(token_de(&resp(403, "")), Ok(Vinculo::Esperando));
        assert!(token_de(&resp(200, r#"{"auth_token":"../x"}"#)).is_err());
        assert!(token_de(&resp(500, "")).is_err());
    }

    #[test]
    fn lan_nanoleaf_info_estado_y_cuerpo() {
        // El ejemplo de la documentación (§4.1), con un ct de verdad.
        let v = json!({"name":"Light Panels Name","serialNo":"S16331A0217","manufacturer":"Nanoleaf","firmwareVersion":"1.5.0","model":"NL22",
            "state":{"on":{"value":true},"brightness":{"value":100,"max":100,"min":0},"hue":{"value":0,"max":360,"min":0},"sat":{"value":0,"max":100,"min":0},
            "ct":{"value":4000,"max":6500,"min":1200},"colorMode":"ct"}});
        let i = info_de(&v).unwrap_or_else(|| panic!("sin info"));
        assert_eq!((i.serie.as_deref(), i.modelo.as_deref(), i.kelvin), (Some("S16331A0217"), Some("NL22"), (1200, 6500)));
        let e = estado_de(&v, i.kelvin);
        assert_eq!((e.encendido, e.brillo, e.modo, e.temperatura), (Some(true), Some(100), Some(ModoLuz::Blanco), Some(53)));
        let efecto = estado_de(&json!({"on":{"value":true},"colorMode":"effect"}), KELVIN);
        assert_eq!(efecto.modo, Some(ModoLuz::Escena));
        assert_eq!(cuerpo(&Cambio { encendido: Some(false), brillo: Some(3), ..Default::default() }, KELVIN), json!({"on":{"value":false}}));
        assert_eq!(
            cuerpo(&Cambio { encendido: Some(true), color: Some(Hsv { h: 120, s: 90, v: 40 }), ..Default::default() }, KELVIN),
            json!({"on":{"value":true},"hue":{"value":120},"sat":{"value":90},"brightness":{"value":40}})
        );
        assert_eq!(cuerpo(&Cambio { temperatura: Some(0), ..Default::default() }, (2700, 6500)), json!({"ct":{"value":2700}}));
    }
}
