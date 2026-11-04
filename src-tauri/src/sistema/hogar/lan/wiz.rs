//! WiZ por la red local: UDP 38899, JSON sin clave (como pywizlight).
//!
//! - Buscar: "registration" a toda la red (con `register: false`, así el foco no guarda nada) y, a cada foco que
//!   responde, `getSystemConfig` (el módulo dice qué sabe hacer), `getModelConfig` (su rango de blancos) y
//!   `getPilot` (cómo está).
//! - Manejar: `setPilot` con `state`, `dimming` (10–100), `temp` (Kelvin) o `r`, `g`, `b`. Leer: `getPilot`.
//! - El módulo se lee así: ESP01_SHRGB1C_31 → "RGB" color y blancos; "TW" blancos; "SOCKET" enchufe; lo demás
//!   (DW) solo brillo.

use super::super::modelo::{
    Cambio, Capacidades, Clase, EstadoAparato, FormaLuz, Hallazgo, Hsv, Manejo, ModoLuz, Proveedor, Via, a_kelvin,
    brillo_a_rango, brillo_de_rango, de_kelvin, hsv_de, rgb_de,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const PUERTO: u16 = 38899;

/// El mismo de pywizlight: `register: false` le dice al foco que no guarde este "teléfono".
const REGISTRO: &str =
    r#"{"method":"registration","params":{"phoneMac":"AAAAAAAAAAAA","register":false,"phoneIp":"1.2.3.4","id":"1"}}"#;

/// Lo mínimo de `dimming` que acepta un foco WiZ (pywizlight: DEFAULT_MIN_DIMMING).
const DIMMING_MIN: i64 = 10;

/// El rango de blancos de cada foco ya visto (IP → Kelvin), para no preguntarlo en cada orden.
static RANGOS: Mutex<Option<HashMap<Ipv4Addr, (u32, u32)>>> = Mutex::new(None);

/// Qué clase de foco es, por su módulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    /// Color y blancos.
    Rgb,
    /// Blanco cálido a frío.
    Tw,
    /// Solo brillo.
    Dw,
    Enchufe,
}

pub fn tipo_de(modulo: Option<&str>) -> Tipo {
    let parte = modulo.and_then(|m| m.split('_').nth(1)).unwrap_or("").to_ascii_uppercase();
    if parte.contains("RGB") {
        Tipo::Rgb
    } else if parte.contains("TW") {
        Tipo::Tw
    } else if parte.contains("SOCKET") {
        Tipo::Enchufe
    } else {
        Tipo::Dw
    }
}

pub fn capacidades(t: Tipo) -> Capacidades {
    match t {
        Tipo::Rgb => Capacidades::luz(true, true, true),
        Tipo::Tw => Capacidades::luz(true, true, false),
        Tipo::Dw => Capacidades::luz(true, false, false),
        Tipo::Enchufe => Capacidades::enchufe(),
    }
}

/// El rango de blancos de fábrica cuando el foco no lo dice.
fn rango_de_fabrica(t: Tipo) -> (u32, u32) {
    if t == Tipo::Rgb { (2200, 6500) } else { (2700, 6500) }
}

/// El rango que dice `getModelConfig` (o `getUserConfig`): `extRange`, `whiteRange` o `cctRange`.
pub fn rango_de(resultado: &Value) -> Option<(u32, u32)> {
    let mut valores: Vec<u32> = ["extRange", "whiteRange", "cctRange"]
        .iter()
        .filter_map(|k| resultado.get(k)?.as_array().cloned())
        .flatten()
        .filter_map(|v| v.as_f64())
        .filter(|k| (1000.0..=10000.0).contains(k))
        .map(|k| k as u32)
        .collect();
    valores.sort_unstable();
    match (valores.first(), valores.last()) {
        (Some(&a), Some(&b)) if b > a => Some((a, b)),
        _ => None,
    }
}

/// Cómo está, según `getPilot`.
pub fn estado_de(r: &Value, rango: (u32, u32)) -> EstadoAparato {
    let num = |k: &str| r.get(k).and_then(Value::as_i64);
    let dimming = num("dimming");
    let brillo = dimming.map(|d| brillo_de_rango(d, DIMMING_MIN, 100));
    let mut e = EstadoAparato {
        encendido: r.get("state").and_then(Value::as_bool),
        brillo,
        ..Default::default()
    };
    let escena = num("sceneId").unwrap_or(0);
    if let (Some(rr), Some(g), Some(b)) = (num("r"), num("g"), num("b")) {
        let c = hsv_de(rr.clamp(0, 255) as u8, g.clamp(0, 255) as u8, b.clamp(0, 255) as u8);
        e.color = Some(Hsv { v: brillo.unwrap_or(c.v), ..c });
        e.modo = Some(ModoLuz::Color);
    } else if let Some(k) = num("temp") {
        e.temperatura = Some(de_kelvin(k.clamp(0, 20000) as u32, rango.0, rango.1));
        e.modo = Some(ModoLuz::Blanco);
    }
    if escena != 0 {
        e.modo = Some(ModoLuz::Escena);
    }
    e
}

/// El `setPilot` de un cambio. Con color, el brillo va en `dimming` y el color a pleno.
pub fn mensaje(cambio: &Cambio, rango: (u32, u32)) -> Value {
    if cambio.encendido == Some(false) {
        return json!({ "method": "setPilot", "params": { "state": false } });
    }
    let mut p = serde_json::Map::new();
    if cambio.encendido == Some(true) {
        p.insert("state".into(), json!(true));
    }
    if let Some(c) = cambio.color {
        let (r, g, b) = rgb_de(Hsv { v: 100, ..c });
        p.insert("r".into(), json!(r));
        p.insert("g".into(), json!(g));
        p.insert("b".into(), json!(b));
        p.insert("dimming".into(), json!(brillo_a_rango(c.v.max(1), DIMMING_MIN, 100)));
    } else {
        if let Some(t) = cambio.temperatura {
            p.insert("temp".into(), json!(a_kelvin(t, rango.0, rango.1)));
        }
        if let Some(b) = cambio.brillo {
            p.insert("dimming".into(), json!(brillo_a_rango(b, DIMMING_MIN, 100)));
        }
    }
    json!({ "method": "setPilot", "params": Value::Object(p) })
}

/// Lo que se juntó de un foco durante la búsqueda.
#[derive(Debug, Default, Clone)]
struct Foco {
    ip: Option<Ipv4Addr>,
    mac: String,
    modulo: Option<String>,
    version: Option<String>,
    rango: Option<(u32, u32)>,
    piloto: Option<Value>,
    senal: Option<i16>,
}

impl Foco {
    fn completo(&self) -> bool {
        self.modulo.is_some() && self.piloto.is_some()
    }
}

/// Lee una respuesta: (método, resultado). Un error del foco viene como `Err` con su mensaje.
pub fn respuesta(datos: &[u8]) -> Option<(String, Result<Value, String>)> {
    let v: Value = serde_json::from_slice(datos).ok()?;
    let metodo = v.get("method")?.as_str()?.to_string();
    if let Some(e) = v.get("error") {
        let texto = e.get("message").and_then(Value::as_str).unwrap_or("error").to_string();
        return Some((metodo, Err(texto)));
    }
    Some((metodo, Ok(v.get("result").cloned().unwrap_or(Value::Null))))
}

fn hallazgo(f: &Foco) -> Option<Hallazgo> {
    let ip = f.ip?;
    let tipo = tipo_de(f.modulo.as_deref());
    let rango = f.rango.unwrap_or(rango_de_fabrica(tipo));
    let (clase, forma, nombre) = match tipo {
        Tipo::Enchufe => (Clase::Enchufe, None, "Enchufe WiZ"),
        Tipo::Rgb => (Clase::Luz, Some(FormaLuz::Foco), "Foco WiZ de color"),
        _ => (Clase::Luz, Some(FormaLuz::Foco), "Foco WiZ"),
    };
    let forma = if f.modulo.as_deref().is_some_and(|m| m.to_ascii_uppercase().contains("STRIP")) {
        Some(FormaLuz::Tira)
    } else {
        forma
    };
    Some(Hallazgo {
        id: format!("wiz:{}", f.mac.to_ascii_lowercase()),
        via: Via::Wifi,
        proveedor: Proveedor::Wiz,
        clase,
        forma,
        marca: "WiZ".into(),
        nombre: nombre.into(),
        modelo: f.modulo.clone(),
        direccion: Some(ip.to_string()),
        senal: f.senal,
        version: f.version.clone(),
        capacidades: capacidades(tipo),
        estado: f.piloto.as_ref().map(|p| estado_de(p, rango)),
        manejo: Manejo::Listo,
    })
}

fn mac_valida(m: &str) -> bool {
    m.len() == 12 && m.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn buscar(fin: Instant) -> Vec<Hallazgo> {
    let Ok(s) = super::abrir_udp(0) else { return Vec::new() };
    let difusion = SocketAddr::from((Ipv4Addr::BROADCAST, PUERTO));
    let mut focos: Vec<Foco> = Vec::new();
    let mut proximo = Instant::now();
    while Instant::now() < fin {
        if Instant::now() >= proximo {
            let _ = s.send_to(REGISTRO.as_bytes(), difusion);
            // A los que todavía no contaron todo, se les vuelve a preguntar.
            for f in focos.iter().filter(|f| !f.completo()) {
                if let Some(ip) = f.ip {
                    preguntar(&s, ip);
                }
            }
            proximo = Instant::now() + Duration::from_millis(1000);
        }
        let hasta = proximo.min(fin);
        super::recibir(&s, hasta, 150, |datos, origen| {
            let std::net::IpAddr::V4(ip) = origen.ip() else { return };
            if !super::es_local(origen.ip()) {
                return;
            }
            let Some((metodo, Ok(r))) = respuesta(datos) else { return };
            let nuevo = !focos.iter().any(|f| f.ip == Some(ip));
            if nuevo {
                let mac = r.get("mac").and_then(Value::as_str).unwrap_or_default().to_string();
                if !mac_valida(&mac) {
                    return;
                }
                focos.push(Foco { ip: Some(ip), mac, ..Default::default() });
                preguntar(&s, ip);
            }
            let Some(f) = focos.iter_mut().find(|f| f.ip == Some(ip)) else { return };
            match metodo.as_str() {
                "getSystemConfig" => {
                    f.modulo = r.get("moduleName").and_then(Value::as_str).map(str::to_string);
                    f.version = r.get("fwVersion").and_then(Value::as_str).map(str::to_string);
                }
                "getModelConfig" => f.rango = rango_de(&r).or(f.rango),
                "getPilot" => {
                    f.senal = r.get("rssi").and_then(Value::as_i64).map(|x| x.clamp(-127, 0) as i16);
                    f.piloto = Some(r);
                }
                _ => {}
            }
        });
    }
    if let Ok(mut g) = RANGOS.lock() {
        let mapa = g.get_or_insert_with(HashMap::new);
        for f in &focos {
            if let (Some(ip), Some(r)) = (f.ip, f.rango) {
                mapa.insert(ip, r);
            }
        }
    }
    focos.iter().filter_map(hallazgo).collect()
}

fn preguntar(s: &std::net::UdpSocket, ip: Ipv4Addr) {
    let destino = SocketAddr::from((ip, PUERTO));
    for metodo in ["getSystemConfig", "getModelConfig", "getPilot"] {
        let _ = s.send_to(json!({ "method": metodo, "params": {} }).to_string().as_bytes(), destino);
    }
}

/// Manda un mensaje y espera la respuesta de ese mismo método (dos intentos: es UDP).
fn pedir(ip: Ipv4Addr, mensaje: &Value) -> Result<Value, String> {
    let s = super::abrir_udp(0).map_err(|_| super::no_responde(&ip.to_string()))?;
    let metodo = mensaje.get("method").and_then(Value::as_str).unwrap_or_default().to_string();
    let destino = SocketAddr::from((ip, PUERTO));
    let fin = Instant::now() + super::PLAZO_ORDEN;
    let mut resultado: Option<Result<Value, String>> = None;
    for intento in 0..2 {
        if s.send_to(mensaje.to_string().as_bytes(), destino).is_err() {
            break;
        }
        let hasta = (Instant::now() + Duration::from_millis(if intento == 0 { 700 } else { 800 })).min(fin);
        super::recibir(&s, hasta, 800, |datos, origen| {
            if resultado.is_none()
                && origen.ip() == ip
                && let Some((m, r)) = respuesta(datos)
                && m == metodo
            {
                resultado = Some(r);
            }
        });
        if resultado.is_some() {
            break;
        }
    }
    match resultado {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(format!("El foco WiZ en {ip} no aceptó el cambio ({e}).")),
        None => Err(super::no_responde(&ip.to_string())),
    }
}

fn rango_para(ip: Ipv4Addr, modelo: Option<&str>) -> (u32, u32) {
    RANGOS
        .lock()
        .ok()
        .and_then(|g| g.as_ref().and_then(|m| m.get(&ip).copied()))
        .unwrap_or_else(|| rango_de_fabrica(tipo_de(modelo)))
}

pub fn leer(ip: &Ipv4Addr, modelo: Option<&str>) -> Result<EstadoAparato, String> {
    let r = pedir(*ip, &json!({ "method": "getPilot", "params": {} }))?;
    Ok(estado_de(&r, rango_para(*ip, modelo)))
}

pub fn cambiar(ip: &Ipv4Addr, modelo: Option<&str>, cambio: &Cambio) -> Result<(), String> {
    let r = pedir(*ip, &mensaje(cambio, rango_para(*ip, modelo)))?;
    if r.get("success").and_then(Value::as_bool) == Some(false) {
        return Err(format!("El foco WiZ en {ip} no aceptó el cambio."));
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lan_wiz_lee_el_modulo() {
        assert_eq!(tipo_de(Some("ESP01_SHRGB1C_31")), Tipo::Rgb);
        assert_eq!(tipo_de(Some("ESP56_SHTW3_01")), Tipo::Tw);
        assert_eq!(tipo_de(Some("ESP10_SOCKET_06")), Tipo::Enchufe);
        assert_eq!(tipo_de(Some("ESP01_SHDW1_31")), Tipo::Dw);
        assert_eq!(tipo_de(None), Tipo::Dw);
        assert_eq!(capacidades(Tipo::Tw), Capacidades::luz(true, true, false));
    }

    #[test]
    fn lan_wiz_lee_getpilot() {
        let r = respuesta(
            br#"{"method":"getPilot","env":"pro","result":{"mac":"a8bb50d2c3e4","rssi":-62,"state":true,"sceneId":0,"temp":4350,"dimming":55}}"#,
        );
        let Some((m, Ok(res))) = r else { panic!("no se leyó") };
        assert_eq!(m, "getPilot");
        let e = estado_de(&res, (2200, 6500));
        assert_eq!(e.encendido, Some(true));
        assert_eq!(e.temperatura, Some(50));
        assert_eq!(e.modo, Some(ModoLuz::Blanco));
        assert_eq!(e.brillo, Some(brillo_de_rango(55, 10, 100)));
        // En color: r, g, b y el brillo en el color.
        let c: Value = serde_json::from_str(r#"{"state":true,"r":255,"g":0,"b":0,"dimming":100,"sceneId":0}"#).unwrap_or_default();
        let e = estado_de(&c, (2200, 6500));
        assert_eq!(e.color, Some(Hsv { h: 0, s: 100, v: 100 }));
        assert_eq!(e.modo, Some(ModoLuz::Color));
        // Una escena de su app.
        let c: Value = serde_json::from_str(r#"{"state":true,"sceneId":4,"dimming":40}"#).unwrap_or_default();
        assert_eq!(estado_de(&c, (2200, 6500)).modo, Some(ModoLuz::Escena));
    }

    #[test]
    fn lan_wiz_arma_setpilot() {
        let rango = (2200, 6500);
        let apagar = mensaje(&Cambio { encendido: Some(false), brillo: Some(40), ..Default::default() }, rango);
        assert_eq!(apagar, json!({"method":"setPilot","params":{"state":false}}));
        let blanco = mensaje(&Cambio { encendido: Some(true), temperatura: Some(0), brillo: Some(100), ..Default::default() }, rango);
        assert_eq!(blanco, json!({"method":"setPilot","params":{"state":true,"temp":2200,"dimming":100}}));
        let color = mensaje(
            &Cambio { encendido: Some(true), color: Some(Hsv { h: 240, s: 100, v: 1 }), ..Default::default() },
            rango,
        );
        assert_eq!(color, json!({"method":"setPilot","params":{"state":true,"r":0,"g":0,"b":255,"dimming":10}}));
        let solo_brillo = mensaje(&Cambio { brillo: Some(50), ..Default::default() }, rango);
        assert_eq!(solo_brillo["params"], json!({"dimming": brillo_a_rango(50, 10, 100)}));
    }

    #[test]
    fn lan_wiz_rangos_y_errores() {
        let r: Value = serde_json::from_str(r#"{"extRange":[2200,6500],"whiteRange":[2700,6500]}"#).unwrap_or_default();
        assert_eq!(rango_de(&r), Some((2200, 6500)));
        assert_eq!(rango_de(&json!({})), None);
        let e = respuesta(br#"{"method":"setPilot","id":1,"env":"pro","error":{"code":-32602,"message":"Invalid params"}}"#);
        assert_eq!(e, Some(("setPilot".into(), Err("Invalid params".into()))));
        assert_eq!(respuesta(b"no es json"), None);
        assert!(mac_valida("a8bb50d2c3e4"));
        assert!(!mac_valida("a8:bb:50"));
    }

    #[test]
    fn lan_wiz_hallazgo_de_un_foco() {
        let f = Foco {
            ip: Some(Ipv4Addr::new(192, 168, 1, 30)),
            mac: "A8BB50D2C3E4".into(),
            modulo: Some("ESP01_SHRGB1C_31".into()),
            version: Some("1.25.0".into()),
            rango: None,
            piloto: Some(json!({"state": false, "temp": 6500, "dimming": 100})),
            senal: Some(-58),
        };
        let h = hallazgo(&f).unwrap_or_else(|| panic!("sin hallazgo"));
        assert_eq!(h.id, "wiz:a8bb50d2c3e4");
        assert_eq!(h.capacidades, Capacidades::luz(true, true, true));
        assert_eq!(h.estado.and_then(|e| e.temperatura), Some(100));
        assert_eq!(h.manejo, Manejo::Listo);
        assert_eq!(h.direccion.as_deref(), Some("192.168.1.30"));
    }
}
