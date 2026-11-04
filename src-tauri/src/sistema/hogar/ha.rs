//! Home Assistant de la casa (gratis y en tu red; nunca por Nabu Casa): todo lo que ya maneja, en Nexo.
//!
//! - Se conecta con su dirección (`http://homeassistant.local:8123` o su IP de Tailscale) y un token de acceso de
//!   larga duración (Perfil › Seguridad). El token va al llavero, nunca a `ajustes.json`.
//! - `GET /api/` comprueba la conexión; `GET /api/states` trae todo; `POST /api/template` (una sola vez por lectura)
//!   trae el área de cada entidad y los identificadores de su aparato, para usar las áreas como habitaciones y no
//!   repetir lo que Nexo ya maneja directo. Sin WebSocket: no queda nada abierto.
//! - Órdenes: `POST /api/services/<dominio>/<servicio>` (`light.turn_on`, `media_player.volume_set`, `lock.unlock`
//!   solo con confirmación, `cover.open_cover`…).
//! - Clases: light → luz · switch → enchufe · media_player → tele · lock → cerradura · binary_sensor de puerta,
//!   ventana o portón → sensor de puerta · cover → cortina · climate → clima. Lo demás va oculto ("Mostrar todo").

use super::http;
use super::modelo::{
    Accion, Cambio, Capacidades, Clase, EstadoAparato, FormaLuz, Hsv, ModoLuz, a_kelvin, de_kelvin, normalizar,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Duration;

const PLAZO: Duration = Duration::from_secs(8);
/// Lo que no es de la casa (automatizaciones, zonas, ajustes internos): ni siquiera con "Mostrar todo".
const INTERNOS: &[&str] = &[
    "automation", "script", "scene", "zone", "sun", "update", "persistent_notification", "conversation", "tts", "stt",
    "wake_word", "event", "timer", "counter", "schedule", "todo", "calendar", "image", "input_boolean", "input_number",
    "input_select", "input_text", "input_datetime", "input_button", "number", "select", "text", "button", "datetime",
    "date", "time", "person", "device_tracker", "weather", "ai_task", "notify", "assist_satellite",
];
/// Como mucho, tantas entidades (un Home Assistant grande tiene miles de sensores).
const MAX_ENTIDADES: usize = 400;

/// Dónde está y con qué entrar. El token vive solo en memoria y en el llavero.
#[derive(Clone)]
pub struct Conexion {
    pub url: String,
    pub token: String,
}

/// La dirección limpia: con http(s)://, sin "/" al final y sin rutas. `None` si no sirve.
pub fn url_limpia(texto: &str) -> Option<String> {
    let t = texto.trim().trim_end_matches('/');
    if t.is_empty() || t.len() > 200 || t.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"') {
        return None;
    }
    let con = if t.starts_with("http://") || t.starts_with("https://") { t.to_string() } else { format!("http://{t}") };
    let resto = con.split_once("://")?.1;
    if resto.is_empty() || resto.contains('/') {
        return None;
    }
    Some(con)
}

fn cabeceras(c: &Conexion, json: bool) -> Vec<(String, String)> {
    let mut v = vec![("Authorization".to_string(), format!("Bearer {}", c.token))];
    if json {
        v.push(("Content-Type".to_string(), "application/json".to_string()));
    }
    v
}

fn revisar(estado: u16, url: &str) -> Result<(), String> {
    match estado {
        200..=299 => Ok(()),
        401 | 403 => Err("Home Assistant no aceptó el token. Crea uno nuevo en tu perfil (Seguridad › Tokens de acceso de larga duración).".into()),
        404 => Err(format!("En {url} no hay un Home Assistant que responda. Revisa la dirección.")),
        otro => Err(format!("Home Assistant respondió con un error ({otro}).")),
    }
}

fn pedir(c: &Conexion, metodo: &str, ruta: &str, cuerpo: Option<&Value>) -> Result<Vec<u8>, String> {
    let bytes = cuerpo.map(|v| serde_json::to_vec(v).unwrap_or_default());
    let r = http::pedir(metodo, &format!("{}{ruta}", c.url), &cabeceras(c, cuerpo.is_some()), bytes.as_deref(), PLAZO)
        .map_err(|e| format!("No pude hablar con Home Assistant en {}. {e}", c.url))?;
    revisar(r.estado, &c.url)?;
    Ok(r.cuerpo)
}

/// Comprueba la dirección y el token (`GET /api/` responde "API running.").
pub fn probar(c: &Conexion) -> Result<(), String> {
    let r = pedir(c, "GET", "/api/", None)?;
    let v: Value = serde_json::from_slice(&r).unwrap_or(Value::Null);
    if v["message"].as_str().is_some_and(|m| m.contains("API running")) {
        Ok(())
    } else {
        Err(format!("En {} responde algo que no es Home Assistant.", c.url))
    }
}

/// Todas las entidades con su estado.
pub fn estados(c: &Conexion) -> Result<Vec<Value>, String> {
    let r = pedir(c, "GET", "/api/states", None)?;
    serde_json::from_slice::<Vec<Value>>(&r).map_err(|_| "Home Assistant respondió algo que no entiendo.".into())
}

/// Una línea por entidad: [id, área, identificadores del aparato, conexiones (MAC)].
const PLANTILLA: &str = "{% for s in states if s.domain in ['light','switch','media_player','lock','binary_sensor','cover','climate'] %}{{ [s.entity_id, area_name(s.entity_id) or '', (device_attr(s.entity_id, 'identifiers') or []) | list, (device_attr(s.entity_id, 'connections') or []) | list] | tojson }}\n{% endfor %}";

/// Área e identificadores de cada entidad. Si la plantilla falla (versión vieja), vacío: van a "Sin cuarto".
pub fn areas(c: &Conexion) -> BTreeMap<String, (String, Vec<String>)> {
    pedir(c, "POST", "/api/template", Some(&json!({ "template": PLANTILLA })))
        .map(|r| leer_areas(&String::from_utf8_lossy(&r)))
        .unwrap_or_default()
}

/// Lee lo que devuelve la plantilla. Los identificadores quedan como texto comparable ("bf12ab…", "a8bb50…").
pub fn leer_areas(texto: &str) -> BTreeMap<String, (String, Vec<String>)> {
    let comparable = |s: &str| s.to_lowercase().replace([':', '-'], "");
    let mut m = BTreeMap::new();
    for linea in texto.lines().map(str::trim).filter(|l| l.starts_with('[')) {
        let Ok(v) = serde_json::from_str::<Value>(linea) else { continue };
        let Some(id) = v[0].as_str() else { continue };
        let area = v[1].as_str().unwrap_or("").trim().to_string();
        let mut enlaces: Vec<String> = Vec::new();
        for grupo in [&v[2], &v[3]] {
            for par in grupo.as_array().into_iter().flatten() {
                if let Some(valor) = par.get(1).and_then(Value::as_str) {
                    let c = comparable(valor);
                    if !c.is_empty() && !enlaces.contains(&c) {
                        enlaces.push(c);
                    }
                }
            }
        }
        m.insert(id.to_string(), (area, enlaces));
    }
    m
}

/// Llama un servicio (`light.turn_on` con sus datos).
pub fn servicio(c: &Conexion, dominio: &str, nombre: &str, datos: &Value) -> Result<(), String> {
    pedir(c, "POST", &format!("/api/services/{dominio}/{nombre}"), Some(datos)).map(|_| ())
}

// ---------------------------------------------------------------- De una entidad a Nexo

/// Una entidad de Home Assistant ya en el idioma de Nexo.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Entidad {
    /// "light.escritorio".
    pub id: String,
    pub nombre: String,
    pub clase: Clase,
    pub forma: Option<FormaLuz>,
    pub capacidades: Capacidades,
    pub estado: EstadoAparato,
    pub en_linea: bool,
    /// No es de las clases de la casa: solo con "Mostrar todo".
    pub oculto: bool,
    /// Tele: sus entradas y la de ahora.
    pub entradas: Vec<String>,
    pub entrada: Option<String>,
    /// Luz: su rango de blancos en Kelvin.
    pub kelvin: Option<(u32, u32)>,
    /// El área de Home Assistant ("Sala"), si tiene.
    pub area: Option<String>,
    /// Identificadores de su aparato (para no repetir lo que Nexo maneja directo).
    pub enlaces: Vec<String>,
}

fn atributo<'a>(e: &'a Value, clave: &str) -> &'a Value {
    &e["attributes"][clave]
}

fn bandera(e: &Value, bit: u64) -> bool {
    atributo(e, "supported_features").as_u64().is_some_and(|f| f & bit != 0)
}

/// Convierte una entidad (`/api/states`). `None` si es de lo interno (automatizaciones, zonas…).
pub fn mapear(e: &Value) -> Option<Entidad> {
    let id = e["entity_id"].as_str()?.to_string();
    let (dominio, _) = id.split_once('.')?;
    if INTERNOS.contains(&dominio) {
        return None;
    }
    let estado_texto = e["state"].as_str().unwrap_or("unknown");
    let nombre = atributo(e, "friendly_name").as_str().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&id).to_string();
    let mut x = Entidad {
        id: id.clone(),
        nombre,
        en_linea: !matches!(estado_texto, "unavailable"),
        ..Entidad::default()
    };
    let encendido = match estado_texto {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    };
    match dominio {
        "light" => {
            let modos: Vec<&str> = atributo(e, "supported_color_modes")
                .as_array()
                .map(|l| l.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let color = modos.iter().any(|m| matches!(*m, "hs" | "xy" | "rgb" | "rgbw" | "rgbww"));
            let temperatura = modos.contains(&"color_temp");
            let brillo = modos.iter().any(|m| *m != "onoff") || atributo(e, "brightness").is_u64();
            let kmin = atributo(e, "min_color_temp_kelvin").as_u64().unwrap_or(2000) as u32;
            let kmax = atributo(e, "max_color_temp_kelvin").as_u64().unwrap_or(6500) as u32;
            let kelvin = (kmax > kmin).then_some((kmin, kmax));
            let b = atributo(e, "brightness").as_f64().map(|b| ((b * 100.0 / 255.0).round() as i64).clamp(1, 100) as u8);
            let modo = match atributo(e, "color_mode").as_str() {
                Some("hs" | "xy" | "rgb" | "rgbw" | "rgbww") => Some(ModoLuz::Color),
                Some(_) => Some(ModoLuz::Blanco),
                None => None,
            };
            let hs = atributo(e, "hs_color").as_array().and_then(|a| Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)));
            x.clase = Clase::Luz;
            x.forma = Some(super::tuya::forma_de("", &x.nombre, &id));
            x.capacidades = Capacidades::luz(brillo, temperatura && kelvin.is_some(), color);
            x.kelvin = kelvin;
            x.estado = EstadoAparato {
                encendido,
                brillo: b,
                temperatura: match (atributo(e, "color_temp_kelvin").as_u64(), kelvin) {
                    (Some(k), Some((a, z))) => Some(de_kelvin(k as u32, a, z)),
                    _ => None,
                },
                color: hs.map(|(h, s)| Hsv::nuevo(h.round() as i64, s.round() as i64, b.unwrap_or(100) as i64)),
                modo: modo.or(Some(ModoLuz::Blanco)).filter(|_| brillo || color || temperatura),
                ..EstadoAparato::default()
            };
        }
        "switch" => {
            x.clase = Clase::Enchufe;
            x.capacidades = Capacidades::enchufe();
            x.estado.encendido = encendido;
        }
        "media_player" => {
            x.clase = Clase::Tv;
            let mut c = Capacidades::ninguna();
            c.encender = bandera(e, 128) || bandera(e, 256);
            c.volumen = bandera(e, 4);
            c.silencio = bandera(e, 8);
            c.entradas = bandera(e, 2048);
            c.informa_estado = true;
            x.capacidades = c;
            x.estado.encendido = match estado_texto {
                "off" | "standby" | "unavailable" | "unknown" => Some(false),
                _ => Some(true),
            };
            x.estado.volumen = atributo(e, "volume_level").as_f64().map(|v| (v * 100.0).round().clamp(0.0, 100.0) as u8);
            x.estado.silenciado = atributo(e, "is_volume_muted").as_bool();
            x.entradas = atributo(e, "source_list")
                .as_array()
                .map(|l| l.iter().filter_map(|s| s.as_str().map(str::to_string)).take(40).collect())
                .unwrap_or_default();
            x.entrada = atributo(e, "source").as_str().map(str::to_string);
        }
        "lock" => {
            x.clase = Clase::Cerradura;
            let mut c = Capacidades::ninguna();
            c.abrir = true;
            c.requiere_confirmacion = true;
            c.informa_estado = true;
            x.capacidades = c;
            x.estado.abierto = match estado_texto {
                "locked" | "locking" => Some(false),
                "unlocked" | "unlocking" | "open" | "opening" => Some(true),
                _ => None,
            };
        }
        "binary_sensor" => {
            let clase = atributo(e, "device_class").as_str().unwrap_or("");
            if matches!(clase, "door" | "window" | "garage_door" | "opening") {
                x.clase = Clase::SensorPuerta;
                let mut c = Capacidades::ninguna();
                c.solo_lectura = true;
                c.informa_estado = true;
                x.capacidades = c;
                x.estado.abierto = encendido;
            } else {
                x.clase = Clase::Otro;
                x.oculto = true;
            }
        }
        "cover" => {
            x.clase = Clase::Cortina;
            let mut c = Capacidades::ninguna();
            c.abrir = true;
            c.posicion = bandera(e, 4);
            c.informa_estado = true;
            x.capacidades = c;
            x.estado.abierto = match estado_texto {
                "open" | "opening" => Some(true),
                "closed" | "closing" => Some(false),
                _ => None,
            };
            x.estado.posicion = atributo(e, "current_position").as_u64().map(|p| p.min(100) as u8);
        }
        "climate" => {
            x.clase = Clase::Clima;
            let mut c = Capacidades::ninguna();
            c.encender = true;
            c.informa_estado = true;
            x.capacidades = c;
            x.estado.encendido = Some(!matches!(estado_texto, "off" | "unavailable" | "unknown"));
            x.estado.grados = atributo(e, "current_temperature").as_f64().map(|g| g as f32);
        }
        _ => {
            x.clase = Clase::Otro;
            x.oculto = true;
            if atributo(e, "device_class").as_str() == Some("temperature") {
                x.estado.grados = estado_texto.parse::<f32>().ok();
            }
        }
    }
    x.estado.bateria = atributo(e, "battery_level").as_f64().map(|b| b.clamp(0.0, 100.0) as u8);
    Some(x)
}

/// Todas las entidades de la casa (las internas no; las demás, ocultas), con su área si se sabe.
pub fn entidades(estados: &[Value], areas: &BTreeMap<String, (String, Vec<String>)>) -> Vec<Entidad> {
    let mut v: Vec<Entidad> = estados.iter().filter_map(mapear).collect();
    // Primero lo de la casa; lo oculto, hasta el tope.
    v.sort_by_key(|e| e.oculto);
    v.truncate(MAX_ENTIDADES);
    for e in &mut v {
        if let Some((area, enlaces)) = areas.get(&e.id) {
            e.area = Some(area.clone()).filter(|a| !a.is_empty());
            e.enlaces = enlaces.clone();
        }
    }
    v
}

/// El servicio para una acción: (dominio, servicio, datos). Abrir una cerradura pide `confirmado`.
pub fn servicio_de(e: &Entidad, accion: &Accion, confirmado: bool) -> Result<(String, String, Value), String> {
    let dominio = e.id.split_once('.').map(|(d, _)| d).unwrap_or("");
    let datos = |extra: Value| {
        let mut d = json!({ "entity_id": e.id });
        if let (Some(m), Some(x)) = (d.as_object_mut(), extra.as_object()) {
            m.extend(x.clone());
        }
        d
    };
    let s = |d: &str, n: &str, v: Value| Ok((d.to_string(), n.to_string(), v));
    let no = || Err(format!("{} no se maneja así desde Nexo.", e.nombre));
    if e.capacidades.solo_lectura && !accion.es_consulta() {
        return Err(format!("{} solo informa: no recibe órdenes.", e.nombre));
    }
    match (dominio, accion) {
        (_, Accion::Consultar) => Err(String::new()),
        ("light", _) => {
            let c = super::modelo::cambio_de(accion, &e.estado, &e.capacidades, &e.nombre)?;
            let (dominio, nombre, extra) = luz(e, &c);
            s(&dominio, &nombre, datos(extra))
        }
        ("switch", Accion::Encender) => s("switch", "turn_on", datos(json!({}))),
        ("switch", Accion::Apagar) => s("switch", "turn_off", datos(json!({}))),
        ("switch", Accion::Alternar) => s("switch", "toggle", datos(json!({}))),
        ("media_player", Accion::Encender) => s("media_player", "turn_on", datos(json!({}))),
        ("media_player", Accion::Apagar) => s("media_player", "turn_off", datos(json!({}))),
        ("media_player", Accion::Alternar) => s("media_player", "toggle", datos(json!({}))),
        ("media_player", Accion::Volumen { valor }) if e.capacidades.volumen => {
            s("media_player", "volume_set", datos(json!({ "volume_level": (*valor).min(100) as f64 / 100.0 })))
        }
        ("media_player", Accion::Subir { .. }) => s("media_player", "volume_up", datos(json!({}))),
        ("media_player", Accion::Bajar { .. }) => s("media_player", "volume_down", datos(json!({}))),
        ("media_player", Accion::Silenciar { activo }) if e.capacidades.silencio => {
            s("media_player", "volume_mute", datos(json!({ "is_volume_muted": activo })))
        }
        ("media_player", Accion::Entrada { nombre }) if e.capacidades.entradas => {
            if !e.entradas.is_empty() && !e.entradas.iter().any(|x| x == nombre) {
                return Err(format!("{} no tiene la entrada «{nombre}».", e.nombre));
            }
            s("media_player", "select_source", datos(json!({ "source": nombre })))
        }
        ("lock", Accion::Cerrar) => s("lock", "lock", datos(json!({}))),
        ("lock", Accion::Abrir) => {
            if !confirmado {
                return Err(format!("Para abrir {}, confírmalo en Nexo.", e.nombre));
            }
            s("lock", "unlock", datos(json!({})))
        }
        ("cover", Accion::Abrir) => s("cover", "open_cover", datos(json!({}))),
        ("cover", Accion::Cerrar) => s("cover", "close_cover", datos(json!({}))),
        ("cover", Accion::Posicion { valor }) if e.capacidades.posicion => {
            s("cover", "set_cover_position", datos(json!({ "position": (*valor).min(100) })))
        }
        ("climate", Accion::Encender) => s("climate", "turn_on", datos(json!({}))),
        ("climate", Accion::Apagar) => s("climate", "turn_off", datos(json!({}))),
        _ => no(),
    }
}

/// `light.turn_on` o `light.turn_off` con lo que pide el cambio.
fn luz(e: &Entidad, c: &Cambio) -> (String, String, Value) {
    if c.encendido == Some(false) {
        return ("light".into(), "turn_off".into(), json!({}));
    }
    let mut d = serde_json::Map::new();
    if let Some(col) = c.color {
        d.insert("hs_color".into(), json!([col.h % 360, col.s]));
        d.insert("brightness_pct".into(), json!(col.v.max(1)));
    } else {
        if let (Some(t), Some((a, z))) = (c.temperatura, e.kelvin) {
            d.insert("color_temp_kelvin".into(), json!(a_kelvin(t, a, z)));
        }
        if let Some(b) = c.brillo {
            d.insert("brightness_pct".into(), json!(b.clamp(1, 100)));
        }
    }
    ("light".into(), "turn_on".into(), Value::Object(d))
}

/// El servicio para un cambio de escena (encender con brillo, blanco o color).
pub fn servicio_de_cambio(e: &Entidad, c: &Cambio) -> Result<(String, String, Value), String> {
    let base = json!({ "entity_id": e.id });
    let junta = |extra: Value| {
        let mut d = base.clone();
        if let (Some(m), Some(x)) = (d.as_object_mut(), extra.as_object()) {
            m.extend(x.clone());
        }
        d
    };
    match e.clase {
        Clase::Luz => {
            let (d, n, extra) = luz(e, c);
            Ok((d, n, junta(extra)))
        }
        Clase::Enchufe | Clase::Tv | Clase::Clima => {
            let dominio = e.id.split_once('.').map(|(d, _)| d).unwrap_or("switch");
            let n = if c.encendido == Some(false) { "turn_off" } else { "turn_on" };
            Ok((dominio.into(), n.into(), base))
        }
        _ => Err(format!("{} no se enciende ni se apaga.", e.nombre)),
    }
}

/// Las áreas de Home Assistant que se ven ("Sala", "Cocina"), sin repetir.
pub fn nombres_de_areas(entidades: &[Entidad]) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for a in entidades.iter().filter_map(|e| e.area.as_deref()) {
        if !v.iter().any(|x| normalizar(x) == normalizar(a)) {
            v.push(a.to_string());
        }
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ent(v: Value) -> Entidad {
        mapear(&v).unwrap_or_default()
    }

    #[test]
    fn hogar_ha_luz_con_color() {
        let e = ent(json!({
            "entity_id": "light.tira_escritorio", "state": "on",
            "attributes": { "friendly_name": "Tira del escritorio", "supported_color_modes": ["color_temp", "hs"],
              "color_mode": "hs", "brightness": 128, "hs_color": [200.0, 80.0],
              "min_color_temp_kelvin": 2000, "max_color_temp_kelvin": 6500 }
        }));
        assert_eq!(e.clase, Clase::Luz);
        assert_eq!(e.forma, Some(FormaLuz::Tira));
        assert_eq!(e.capacidades, Capacidades::luz(true, true, true));
        assert_eq!(e.estado.brillo, Some(50));
        assert_eq!(e.estado.modo, Some(ModoLuz::Color));
        assert_eq!(e.estado.color, Some(Hsv { h: 200, s: 80, v: 50 }));
        assert_eq!(e.kelvin, Some((2000, 6500)));
        // Brillo, blanco y color, en servicios de Home Assistant.
        let (d, n, v) = servicio_de(&e, &Accion::Brillo { valor: 30 }, false).unwrap_or_default();
        assert_eq!((d.as_str(), n.as_str()), ("light", "turn_on"));
        assert_eq!(v, json!({ "entity_id": "light.tira_escritorio", "brightness_pct": 30 }));
        let (_, _, v) = servicio_de(&e, &Accion::Temperatura { valor: 100 }, false).unwrap_or_default();
        assert_eq!(v["color_temp_kelvin"], json!(6500));
        let (_, _, v) = servicio_de(&e, &Accion::Color { h: 10, s: 90, v: 60 }, false).unwrap_or_default();
        assert_eq!((v["hs_color"].clone(), v["brightness_pct"].clone()), (json!([10, 90]), json!(60)));
        let (_, n, _) = servicio_de(&e, &Accion::Apagar, false).unwrap_or_default();
        assert_eq!(n, "turn_off");
    }

    #[test]
    fn hogar_ha_tele() {
        let e = ent(json!({
            "entity_id": "media_player.tele_sala", "state": "playing",
            "attributes": { "friendly_name": "Tele de la sala", "supported_features": 128 | 256 | 4 | 8 | 2048,
              "volume_level": 0.25, "is_volume_muted": false, "source": "HDMI 1", "source_list": ["HDMI 1", "Netflix"] }
        }));
        assert_eq!(e.clase, Clase::Tv);
        assert!(e.capacidades.encender && e.capacidades.volumen && e.capacidades.silencio && e.capacidades.entradas);
        assert_eq!((e.estado.encendido, e.estado.volumen, e.estado.silenciado), (Some(true), Some(25), Some(false)));
        assert_eq!(e.entradas, ["HDMI 1", "Netflix"]);
        let (_, n, v) = servicio_de(&e, &Accion::Volumen { valor: 40 }, false).unwrap_or_default();
        assert_eq!((n.as_str(), v["volume_level"].clone()), ("volume_set", json!(0.4)));
        let (_, n, _) = servicio_de(&e, &Accion::Silenciar { activo: true }, false).unwrap_or_default();
        assert_eq!(n, "volume_mute");
        let (_, n, v) = servicio_de(&e, &Accion::Entrada { nombre: "Netflix".into() }, false).unwrap_or_default();
        assert_eq!((n.as_str(), v["source"].clone()), ("select_source", json!("Netflix")));
        assert!(servicio_de(&e, &Accion::Entrada { nombre: "VHS".into() }, false).is_err());
        let apagada = ent(json!({ "entity_id": "media_player.tv", "state": "standby", "attributes": {} }));
        assert_eq!(apagada.estado.encendido, Some(false));
    }

    #[test]
    fn hogar_ha_cerradura_pide_confirmacion() {
        let e = ent(json!({ "entity_id": "lock.puerta", "state": "locked", "attributes": { "friendly_name": "Puerta de entrada" } }));
        assert_eq!(e.clase, Clase::Cerradura);
        assert!(e.capacidades.requiere_confirmacion);
        assert_eq!(e.estado.abierto, Some(false));
        assert_eq!(servicio_de(&e, &Accion::Abrir, false), Err("Para abrir Puerta de entrada, confírmalo en Nexo.".into()));
        assert_eq!(servicio_de(&e, &Accion::Abrir, true).map(|x| x.1), Ok("unlock".into()));
        assert_eq!(servicio_de(&e, &Accion::Cerrar, false).map(|x| x.1), Ok("lock".into()));
        assert!(servicio_de(&e, &Accion::Encender, true).is_err());
    }

    #[test]
    fn hogar_ha_sensores_cortinas_y_clima() {
        let puerta = ent(json!({ "entity_id": "binary_sensor.puerta", "state": "on", "attributes": { "device_class": "door", "battery_level": 80 } }));
        assert_eq!((puerta.clase, puerta.estado.abierto, puerta.estado.bateria), (Clase::SensorPuerta, Some(true), Some(80)));
        assert!(puerta.capacidades.solo_lectura);
        assert!(servicio_de(&puerta, &Accion::Cerrar, true).is_err());
        let movimiento = ent(json!({ "entity_id": "binary_sensor.mov", "state": "off", "attributes": { "device_class": "motion" } }));
        assert!(movimiento.oculto);
        let cortina = ent(json!({ "entity_id": "cover.sala", "state": "open", "attributes": { "supported_features": 15, "current_position": 70 } }));
        assert_eq!((cortina.clase, cortina.estado.abierto, cortina.estado.posicion), (Clase::Cortina, Some(true), Some(70)));
        assert_eq!(servicio_de(&cortina, &Accion::Posicion { valor: 20 }, false).map(|x| x.2["position"].clone()), Ok(json!(20)));
        assert_eq!(servicio_de(&cortina, &Accion::Cerrar, false).map(|x| x.1), Ok("close_cover".into()));
        let clima = ent(json!({ "entity_id": "climate.cuarto", "state": "cool", "attributes": { "current_temperature": 23.5 } }));
        assert_eq!((clima.clase, clima.estado.encendido, clima.estado.grados), (Clase::Clima, Some(true), Some(23.5)));
        let enchufe = ent(json!({ "entity_id": "switch.ventilador", "state": "off", "attributes": {} }));
        assert_eq!(servicio_de(&enchufe, &Accion::Alternar, false).map(|x| x.1), Ok("toggle".into()));
        // Lo interno no entra; lo demás, oculto.
        assert!(mapear(&json!({ "entity_id": "automation.x", "state": "on" })).is_none());
        assert!(ent(json!({ "entity_id": "sensor.temp", "state": "21.5", "attributes": { "device_class": "temperature" } })).oculto);
    }

    #[test]
    fn hogar_ha_areas_e_identificadores() {
        let texto = "[\"light.tira\", \"Escritorio\", [[\"tuya\", \"bf12AB34\"]], [[\"mac\", \"A8:BB:50:01:02:03\"]]]\n\
                     [\"switch.x\", \"\", [], []]\nbasura\n";
        let m = leer_areas(texto);
        assert_eq!(m.get("light.tira"), Some(&("Escritorio".to_string(), vec!["bf12ab34".to_string(), "a8bb50010203".to_string()])));
        assert_eq!(m.get("switch.x"), Some(&(String::new(), vec![])));
        let estados = vec![json!({ "entity_id": "light.tira", "state": "off", "attributes": {} })];
        let v = entidades(&estados, &m);
        assert_eq!(v[0].area.as_deref(), Some("Escritorio"));
        assert_eq!(nombres_de_areas(&v), ["Escritorio"]);
    }

    #[test]
    fn hogar_ha_direcciones() {
        assert_eq!(url_limpia(" homeassistant.local:8123/ "), Some("http://homeassistant.local:8123".into()));
        assert_eq!(url_limpia("https://100.88.1.2:8123"), Some("https://100.88.1.2:8123".into()));
        assert_eq!(url_limpia("http://x/y"), None);
        assert_eq!(url_limpia("http://a b"), None);
        assert_eq!(url_limpia(""), None);
    }
}
