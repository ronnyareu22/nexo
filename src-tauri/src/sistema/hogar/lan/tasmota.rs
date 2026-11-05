//! Tasmota y OpenBeken: aparatos (muchos de Tuya) con firmware abierto. Los dos entienden `GET /cm?cmnd=…` y
//! contestan JSON con la misma forma (https://tasmota.github.io/docs/Commands/ y los `docs/commands.md` y
//! `src/httpserver/json_interface.c` de OpenBK7231T_App).
//!
//! - Ninguno se anuncia de fábrica (Tasmota tiene el mDNS apagado y OpenBeken no carga el suyo): se prueba con
//!   lo que hay en la red con chip de aparato inteligente (Espressif, Tuya, Beken…) o nombre `tasmota-…`/`obk…`.
//! - Una sola pregunta sirve para los dos: `Status 2` → `StatusFWR.Version` ("15.1.0(release-tasmota)") en
//!   Tasmota; en OpenBeken `Version` empieza con "Open" ("OpenBK7231N_1.18.x") y `SDK` es "obk".
//! - Siempre va `Referer: http://<ip>/`: desde Tasmota 15.4.0.2 la API rechaza lo que no lo trae (SetOption128).
//! - Con contraseña: Basic con usuario `admin` (Tasmota contesta 401 con `{"WARNING":…}`; OpenBeken, 401 con
//!   `WWW-Authenticate: Basic realm="OpenBeken HTTP Server"`).
//! - `Status 5` da la MAC; `Status` el nombre; `State` cómo está: `POWER` o `POWER1…n` ("ON"/"OFF"), `Dimmer`
//!   0–100, `CT` 153–500 (más alto, más cálido), `HSBColor` "h,s,b" y `Channel` (uno por canal de luz).
//! - Órdenes: `Power2 On`, `Dimmer 40`, `CT 300`, `HSBColor 120,100,50`, varias juntas con `Backlog a;b`.

use super::super::modelo::{
    Cambio, Capacidades, Clase, EstadoAparato, FormaLuz, FormaVinculo, Hallazgo, Hsv, Manejo, ModoLuz, Proveedor, Via, a_rango, de_rango,
    mac_normal,
};
use super::web;
use serde_json::Value;
use std::net::Ipv4Addr;
use std::time::Duration;

const PLAZO: Duration = Duration::from_millis(1500);
const USUARIO: &str = "admin";
/// CT en mireds: 153 es el blanco más frío y 500 el más cálido.
const CT: (i64, i64) = (153, 500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Firmware {
    Tasmota,
    OpenBeken,
}

impl Firmware {
    pub fn marca(self) -> &'static str {
        match self {
            Firmware::Tasmota => "Tasmota",
            Firmware::OpenBeken => "OpenBeken",
        }
    }
}

/// Qué firmware es y su versión, por `Status 2`.
pub fn firmware_de(v: &Value) -> Option<(Firmware, String)> {
    let fwr = v.get("StatusFWR")?;
    let version = fwr.get("Version")?.as_str()?.trim().to_string();
    let sdk = fwr.get("SDK").and_then(Value::as_str).unwrap_or("");
    if sdk.eq_ignore_ascii_case("obk") || version.starts_with("Open") {
        return Some((Firmware::OpenBeken, version));
    }
    // "15.1.0(release-tasmota)", "14.2.0.3(tasmota)".
    let numeros = version.split('(').next().unwrap_or("");
    let parece = numeros.split('.').count() >= 3 && numeros.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    parece.then_some((Firmware::Tasmota, version))
}

/// Lo que contestó la primera pregunta.
#[derive(Debug, Clone, PartialEq)]
pub enum Sondeo {
    Hallado(Firmware, String),
    /// Tiene contraseña.
    Clave(Firmware),
    Nada,
}

pub fn sondeo_de(r: &web::Respuesta) -> Sondeo {
    if r.estado == 401 {
        let obk = r.cabecera("www-authenticate").is_some_and(|a| a.contains("OpenBeken"));
        let tasmota = r.json().is_some_and(|v| v.get("WARNING").is_some());
        return match (obk, tasmota) {
            (true, _) => Sondeo::Clave(Firmware::OpenBeken),
            (_, true) => Sondeo::Clave(Firmware::Tasmota),
            _ => Sondeo::Nada,
        };
    }
    match r.json().as_ref().and_then(firmware_de) {
        Some((f, v)) if r.ok() => Sondeo::Hallado(f, v),
        _ => Sondeo::Nada,
    }
}

fn cabeceras(ip: Ipv4Addr, clave: Option<&str>) -> Vec<(&'static str, String)> {
    let mut v = vec![("Referer", format!("http://{ip}/"))];
    if let Some(c) = clave {
        v.push(("Authorization", web::basic(USUARIO, c)));
    }
    v
}

/// Un comando (`Status 5`, `Backlog Power On;Dimmer 40`…) y su JSON.
pub fn comando(ip: Ipv4Addr, cmnd: &str, clave: Option<&str>) -> Result<Value, String> {
    web::json(ip, 80, &format!("/cm?cmnd={}", web::codificar(cmnd)), &cabeceras(ip, clave), PLAZO)
}

/// La primera pregunta, con plazo corto (se prueba con varios a la vez).
pub fn sondear(ip: Ipv4Addr, plazo: Duration) -> Sondeo {
    match web::pedir(ip, 80, "GET", "/cm?cmnd=Status%202", &cabeceras(ip, None), None, plazo) {
        Ok(r) => sondeo_de(&r),
        Err(_) => Sondeo::Nada,
    }
}

/// Una salida: `canal` "" es `POWER` (una sola) y "2" es `POWER2`.
#[derive(Debug, Clone, PartialEq)]
pub struct Salida {
    pub canal: String,
    pub clase: Clase,
    pub capacidades: Capacidades,
}

/// Las salidas por `State`: relés (`POWERn`) y, si tiene `Dimmer`, una luz (la última salida).
pub fn salidas_de(estado: &Value) -> Vec<Salida> {
    let Some(o) = estado.as_object() else { return Vec::new() };
    let mut potencias: Vec<u32> =
        o.keys().filter_map(|k| k.strip_prefix("POWER")).filter_map(|n| if n.is_empty() { Some(0) } else { n.parse().ok() }).collect();
    potencias.sort_unstable();
    potencias.dedup();
    if potencias.is_empty() {
        return Vec::new();
    }
    let luz = o.contains_key("Dimmer");
    let color = o.contains_key("HSBColor");
    let ct = o.contains_key("CT");
    let ultima = *potencias.last().unwrap_or(&0);
    potencias
        .into_iter()
        .map(|n| {
            let canal = if n == 0 { String::new() } else { n.to_string() };
            if luz && n == ultima {
                Salida { canal, clase: Clase::Luz, capacidades: Capacidades::luz(true, ct, color) }
            } else {
                Salida { canal, clase: Clase::Enchufe, capacidades: Capacidades::enchufe() }
            }
        })
        .collect()
}

fn encendido(v: &Value, canal: &str) -> Option<bool> {
    let clave = format!("POWER{canal}");
    let x = v.get(&clave).or_else(|| if canal == "1" { v.get("POWER") } else { None }).or_else(|| if canal.is_empty() { v.get("POWER1") } else { None })?;
    match x {
        Value::String(s) => Some(s.eq_ignore_ascii_case("on") || s == "1"),
        Value::Number(n) => Some(n.as_i64().unwrap_or(0) != 0),
        _ => None,
    }
}

pub fn estado_de(v: &Value, canal: &str, luz: bool) -> EstadoAparato {
    let mut e = EstadoAparato { encendido: encendido(v, canal), ..Default::default() };
    if !luz {
        return e;
    }
    e.brillo = v.get("Dimmer").and_then(Value::as_f64).map(|d| d.round().clamp(1.0, 100.0) as u8);
    let hsb: Option<Vec<i64>> = v
        .get("HSBColor")
        .and_then(Value::as_str)
        .map(|t| t.split(',').filter_map(|p| p.trim().parse::<i64>().ok()).collect::<Vec<_>>())
        .filter(|p| p.len() == 3);
    // Con canales: si los tres primeros (RGB) están en 0 y hay blanco, está en blanco.
    let canales: Vec<i64> = v.get("Channel").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
    let rgb_apagado = canales.len() > 3 && canales[..3].iter().all(|c| *c == 0);
    let ct = v.get("CT").and_then(Value::as_i64);
    match hsb {
        Some(p) if !rgb_apagado && p[1] > 0 => {
            e.color = Some(Hsv::nuevo(p[0], p[1], e.brillo.unwrap_or(p[2].clamp(1, 100) as u8) as i64));
            e.modo = Some(ModoLuz::Color);
        }
        _ => {
            if let Some(c) = ct {
                e.temperatura = Some(100 - de_rango(c, CT.0, CT.1));
            }
            e.modo = Some(ModoLuz::Blanco);
        }
    }
    e
}

/// Las órdenes de un cambio, en un solo comando (con `Backlog` si son varias).
pub fn orden(canal: &str, c: &Cambio) -> Option<String> {
    let mut v: Vec<String> = Vec::new();
    if c.encendido == Some(false) {
        return Some(format!("Power{canal} Off"));
    }
    if let Some(color) = c.color {
        v.push(format!("HSBColor {},{},{}", color.h % 360, color.s.min(100), color.v.clamp(1, 100)));
    } else {
        if let Some(t) = c.temperatura {
            v.push(format!("CT {}", a_rango(100 - t.min(100), CT.0, CT.1)));
        }
        if let Some(b) = c.brillo {
            v.push(format!("Dimmer {}", b.clamp(1, 100)));
        }
    }
    if c.encendido == Some(true) {
        v.insert(0, format!("Power{canal} On"));
    }
    match v.len() {
        0 => None,
        1 => v.pop(),
        _ => Some(format!("Backlog {}", v.join(";"))),
    }
}

/// El nombre que le puso la persona (`Status` → `DeviceName` o el primer `FriendlyName`).
pub fn nombre_de(v: &Value) -> Option<String> {
    let s = v.get("Status")?;
    let n = s
        .get("DeviceName")
        .and_then(Value::as_str)
        .or_else(|| s.get("FriendlyName").and_then(Value::as_array).and_then(|a| a.first()).and_then(Value::as_str))?
        .trim();
    (!n.is_empty()).then(|| n.chars().take(40).collect())
}

pub fn mac_de(v: &Value) -> Option<String> {
    mac_normal(v.pointer("/StatusNET/Mac")?.as_str()?)
}

pub fn hallazgo(ip: Ipv4Addr, firmware: Firmware, version: Option<String>, mac: Option<String>, nombre: Option<String>, estado: Option<&Value>, clave: bool) -> Hallazgo {
    let salidas = estado.map(salidas_de).unwrap_or_default();
    let luz = salidas.iter().find(|s| s.clase == Clase::Luz);
    let clase = if luz.is_some() { Clase::Luz } else { Clase::Enchufe };
    let id = mac.as_deref().map(|m| m.replace(':', "")).unwrap_or_else(|| ip.to_string().replace('.', "-"));
    let manejo = if clave {
        Manejo::Vincular { forma: FormaVinculo::Clave, paso: format!("Este {} tiene contraseña. Escríbela para agregarlo.", firmware.marca()) }
    } else {
        Manejo::Listo
    };
    Hallazgo {
        id: format!("tasmota:{id}"),
        via: Via::Wifi,
        proveedor: Proveedor::Tasmota,
        clase,
        forma: luz.map(|_| FormaLuz::Foco),
        marca: firmware.marca().into(),
        nombre: nombre.unwrap_or_else(|| format!("Aparato {}", firmware.marca())),
        modelo: None,
        direccion: Some(ip.to_string()),
        mac,
        senal: None,
        version,
        capacidades: luz.map(|s| s.capacidades).unwrap_or_else(Capacidades::enchufe),
        estado: estado.map(|e| estado_de(e, salidas.first().map(|s| s.canal.as_str()).unwrap_or(""), luz.is_some())),
        manejo,
    }
}

/// Todo lo de un aparato ya reconocido (MAC, nombre y cómo está).
pub fn detalle(ip: Ipv4Addr, clave: Option<&str>) -> Result<(Option<String>, Option<String>, Value), String> {
    let red = comando(ip, "Status 5", clave)?;
    let nombre = comando(ip, "Status", clave).ok().as_ref().and_then(nombre_de);
    let estado = comando(ip, "State", clave)?;
    Ok((mac_de(&red), nombre, estado))
}

pub fn leer(ip: Ipv4Addr, canal: &str, luz: bool, clave: Option<&str>) -> Result<EstadoAparato, String> {
    comando(ip, "State", clave).map(|v| estado_de(&v, canal, luz))
}

pub fn cambiar(ip: Ipv4Addr, canal: &str, clave: Option<&str>, c: &Cambio) -> Result<(), String> {
    let o = orden(canal, c).ok_or("Ese cambio no aplica a este aparato.")?;
    comando(ip, &o, clave).map(|_| ())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    #[test]
    fn lan_tasmota_reconoce_los_dos() {
        assert_eq!(
            firmware_de(&json!({"StatusFWR":{"Version":"15.1.0(release-tasmota)","BuildDateTime":"2025-09-01","Core":"2_7_8","SDK":"2.2.2-dev(38a443e)"}})),
            Some((Firmware::Tasmota, "15.1.0(release-tasmota)".into()))
        );
        assert_eq!(
            firmware_de(&json!({"StatusFWR":{"Version":"OpenBK7231N_1.18.141","SDK":"obk","Hardware":"BK7231N","Boot":7,"Core":"0.0"}})).map(|x| x.0),
            Some(Firmware::OpenBeken)
        );
        assert_eq!(firmware_de(&json!({"StatusFWR":{"Version":"hola"}})), None);
        let r = |estado: u16, cab: Vec<(String, String)>, cuerpo: &str| web::Respuesta { estado, cabeceras: cab, cuerpo: cuerpo.as_bytes().to_vec() };
        assert_eq!(sondeo_de(&r(401, vec![], r#"{"WARNING":"Need user=<username>&password=<password>"}"#)), Sondeo::Clave(Firmware::Tasmota));
        assert_eq!(
            sondeo_de(&r(401, vec![("WWW-Authenticate".into(), "Basic realm=\"OpenBeken HTTP Server\"".into())], "")),
            Sondeo::Clave(Firmware::OpenBeken)
        );
        assert_eq!(sondeo_de(&r(404, vec![], "Not found")), Sondeo::Nada);
        assert_eq!(sondeo_de(&r(401, vec![], "")), Sondeo::Nada);
    }

    #[test]
    fn lan_tasmota_salidas_y_estado() {
        let enchufe = json!({"Time":"2026-11-04T10:00:00","POWER":"ON","Wifi":{"RSSI":80}});
        let s = salidas_de(&enchufe);
        assert_eq!((s.len(), s[0].canal.as_str(), s[0].clase), (1, "", Clase::Enchufe));
        assert_eq!(estado_de(&enchufe, "", false).encendido, Some(true));
        let doble = json!({"POWER1":"ON","POWER2":"OFF"});
        assert_eq!(salidas_de(&doble).iter().map(|x| x.canal.as_str()).collect::<Vec<_>>(), ["1", "2"]);
        assert_eq!(estado_de(&doble, "2", false).encendido, Some(false));
        // Un foco RGBCCT en blanco: los canales RGB en 0.
        let foco = json!({"POWER":"ON","Dimmer":60,"Color":"000000FF66","HSBColor":"30,100,0","White":60,"CT":500,"Channel":[0,0,0,100,40]});
        let s = salidas_de(&foco);
        assert_eq!(s[0].capacidades, Capacidades::luz(true, true, true));
        let e = estado_de(&foco, "", true);
        assert_eq!((e.brillo, e.modo, e.temperatura), (Some(60), Some(ModoLuz::Blanco), Some(0)));
        let rojo = json!({"POWER":"ON","Dimmer":80,"HSBColor":"0,100,80","Channel":[80,0,0]});
        assert_eq!(estado_de(&rojo, "", true).color, Some(Hsv { h: 0, s: 100, v: 80 }));
        assert_eq!(nombre_de(&json!({"Status":{"Module":0,"DeviceName":"Enchufe tele","FriendlyName":["Tasmota"]}})).as_deref(), Some("Enchufe tele"));
        assert_eq!(mac_de(&json!({"StatusNET":{"Hostname":"tasmota-1A2B3C-1234","Mac":"A4:CF:12:1A:2B:3C"}})).as_deref(), Some("a4:cf:12:1a:2b:3c"));
    }

    #[test]
    fn lan_tasmota_ordenes() {
        assert_eq!(orden("", &Cambio { encendido: Some(false), brillo: Some(20), ..Default::default() }).as_deref(), Some("Power Off"));
        assert_eq!(orden("2", &Cambio { encendido: Some(true), ..Default::default() }).as_deref(), Some("Power2 On"));
        assert_eq!(
            orden("", &Cambio { encendido: Some(true), temperatura: Some(100), brillo: Some(40), ..Default::default() }).as_deref(),
            Some("Backlog Power On;CT 153;Dimmer 40")
        );
        assert_eq!(orden("", &Cambio { color: Some(Hsv { h: 120, s: 100, v: 50 }), ..Default::default() }).as_deref(), Some("HSBColor 120,100,50"));
        assert_eq!(orden("", &Cambio::default()), None);
        let h = hallazgo(Ipv4Addr::new(192, 168, 1, 90), Firmware::OpenBeken, None, Some("a4:cf:12:1a:2b:3c".into()), None, None, true);
        assert_eq!((h.id.as_str(), h.marca.as_str()), ("tasmota:a4cf121a2b3c", "OpenBeken"));
        assert!(matches!(h.manejo, Manejo::Vincular { forma: FormaVinculo::Clave, .. }));
    }
}
