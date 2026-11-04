//! La nube de Tuya / Smart Life: un proyecto de la plataforma IoT de Tuya con la cuenta de la app vinculada.
//!
//! - Token: `GET /v1.0/token?grant_type=1` (2 h) y se renueva con `GET /v1.0/token/{refresh_token}`. Solo en memoria.
//! - Aparatos de la cuenta vinculada: `GET /v1.0/iot-01/associated-users/devices` (trae nombre, categoría, si está en
//!   línea y su estado en una sola llamada). Si el proyecto no tiene esa API: `GET /v2.0/cloud/thing/device` y el
//!   estado por tandas de 20 con `GET /v1.0/iot-03/devices/status`.
//! - Qué sabe hacer cada uno: `GET /v1.0/iot-03/devices/{id}/specification` (se guarda en `casa.json`).
//! - Órdenes: `POST /v1.0/iot-03/devices/{id}/commands` (o `/v1.0/devices/{id}/commands`).
//! - Los puntos de datos de una luz cambian según el modelo (switch_led, bright_value_v2 10–1000 o bright_value
//!   25–255, temp_value_v2 0–1000, colour_data_v2 {h,s,v}, work_mode…): se leen de su especificación y se pasan
//!   a los valores de Nexo (`modelo`).
//! - La versión de prueba del proyecto da unas 26 000 llamadas al mes: Nexo pregunta solo con la Casa o Mi setup
//!   a la vista y recuerda la respuesta 15 s.

use super::firma;
use super::http;
use super::modelo::{
    Capacidades, Cambio, Clase, EstadoAparato, FormaLuz, Hsv, ModoLuz, a_rango, brillo_a_rango, brillo_de_rango, de_rango,
    normalizar,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Los centros de datos: (clave, nombre, dirección). "Detectar" los prueba en este orden.
pub const REGIONES: &[(&str, &str, &str)] = &[
    ("us", "América (oeste)", "https://openapi.tuyaus.com"),
    ("us-e", "América (este)", "https://openapi-ueaz.tuyaus.com"),
    ("eu", "Europa central", "https://openapi.tuyaeu.com"),
    ("eu-w", "Europa occidental", "https://openapi-weaz.tuyaeu.com"),
    ("in", "India", "https://openapi.tuyain.com"),
    ("sg", "Singapur", "https://openapi-sg.iotbing.com"),
    ("cn", "China", "https://openapi.tuyacn.com"),
];

const PLAZO: Duration = Duration::from_secs(8);

pub fn region_valida(r: &str) -> bool {
    REGIONES.iter().any(|(k, _, _)| *k == r)
}

fn base(region: &str) -> Option<&'static str> {
    REGIONES.iter().find(|(k, _, _)| *k == region).map(|(_, _, u)| *u)
}

/// Lo que hace falta para hablar con el proyecto. El secreto vive solo en memoria y en el llavero.
#[derive(Clone)]
pub struct Credenciales {
    pub id: String,
    pub secreto: String,
    pub region: String,
}

/// Un error de Tuya: el código (si lo dio) y qué pasó, en palabras.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorTuya {
    pub codigo: Option<i64>,
    pub texto: String,
}

impl ErrorTuya {
    fn red(texto: String) -> Self {
        ErrorTuya { codigo: None, texto }
    }
    /// El token venció o no sirve: se pide otro y se reintenta.
    fn token_vencido(&self) -> bool {
        matches!(self.codigo, Some(1010..=1012))
    }
    /// El proyecto no tiene esa API (o esa ruta ya no existe): se prueba la otra forma.
    fn sin_api(&self) -> bool {
        matches!(self.codigo, Some(1108 | 28841101 | 28841105 | 28841106 | 60009001))
    }
    /// Las credenciales no son (no tiene sentido seguir probando).
    pub fn credenciales(&self) -> bool {
        matches!(self.codigo, Some(1001 | 1004 | 1005))
    }
}

/// Lo que dice Tuya, en palabras para cualquiera.
pub fn traducir(codigo: i64, msg: &str) -> String {
    match codigo {
        1001 | 1004 | 1005 => {
            "El Access ID o el Access Secret no son correctos. Cópialos de nuevo desde tu proyecto de Tuya (Overview).".into()
        }
        1010..=1012 => "Tuya cerró la sesión. Vuelve a intentarlo.".into(),
        1013 => "La hora de este equipo no coincide con la de Tuya. Revisa la fecha y la hora del sistema.".into(),
        1106 => "Tuya no da permiso. Revisa que tu cuenta de Smart Life esté vinculada al proyecto y que el centro de datos sea el de tu cuenta.".into(),
        1114 => "Tu proyecto de Tuya no acepta consultas desde esta red.".into(),
        1199 | 28841004 | 28841104 => {
            "Se acabaron las consultas del mes de tu proyecto de Tuya. Vuelven el mes que viene, o amplía el plan en iot.tuya.com.".into()
        }
        2007 => "Ese no es el centro de datos de tu cuenta. Elige otro o usa Detectar.".into(),
        2001 | 40000801 => "está sin conexión. Revisa que tenga corriente y Wi-Fi.".into(),
        2008 | 40000308 | 40000003 => "no acepta ese cambio.".into(),
        2010 => "Tuya ya no encuentra ese aparato. Busca de nuevo.".into(),
        2047 => "está bloqueado en la app Smart Life.".into(),
        28841001 | 28841002 => {
            "Venció el plan de prueba de tu proyecto de Tuya. Renuévalo gratis en iot.tuya.com (Cloud › tu proyecto › Service API).".into()
        }
        28841101 | 28841102 | 28841105 | 28841106 | 60009001 => {
            "A tu proyecto de Tuya le falta el servicio «IoT Core». Súmalo en iot.tuya.com (tu proyecto › Service API).".into()
        }
        otro => format!("Tuya respondió con un error ({otro}: {msg})."),
    }
}

/// Los errores que hablan de un aparato ("está sin conexión") llevan su nombre delante.
pub fn con_nombre(e: &ErrorTuya, nombre: &str) -> String {
    match e.texto.chars().next() {
        Some(c) if c.is_lowercase() => format!("{nombre} {}", e.texto),
        _ => e.texto.clone(),
    }
}

// ---------------------------------------------------------------- Peticiones firmadas

struct Sesion {
    /// "<access id>@<región>": si cambian, el token no sirve.
    de: String,
    token: String,
    renovar: String,
    vence: Instant,
}

static SESION: Mutex<Option<Sesion>> = Mutex::new(None);

pub fn olvidar_sesion() {
    *SESION.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn milisegundos() -> String {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0).to_string()
}

/// Codifica un valor de la consulta (la firma va sin codificar).
fn codificar(v: &str) -> String {
    let mut s = String::with_capacity(v.len());
    for b in v.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b',') {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

fn llamar(
    c: &Credenciales,
    token: &str,
    metodo: &str,
    ruta: &str,
    parametros: &[(&str, String)],
    cuerpo: Option<&Value>,
) -> Result<Value, ErrorTuya> {
    let base = base(&c.region).ok_or_else(|| ErrorTuya::red("Elige el centro de datos de tu cuenta.".into()))?;
    let firmada = firma::ruta_firmada(ruta, parametros);
    let bytes = cuerpo.map(|v| serde_json::to_vec(v).unwrap_or_default()).unwrap_or_default();
    let t = milisegundos();
    let texto = firma::texto_a_firmar(metodo, &bytes, &[], &firmada);
    let sign = firma::firmar(&c.id, token, &t, "", &texto, &c.secreto);
    let mut cabeceras = vec![
        ("client_id".to_string(), c.id.clone()),
        ("sign".to_string(), sign),
        ("t".to_string(), t),
        ("sign_method".to_string(), "HMAC-SHA256".to_string()),
        ("lang".to_string(), "en".to_string()),
    ];
    if !token.is_empty() {
        cabeceras.push(("access_token".to_string(), token.to_string()));
    }
    if cuerpo.is_some() {
        cabeceras.push(("Content-Type".to_string(), "application/json".to_string()));
    }
    let mut p: Vec<&(&str, String)> = parametros.iter().collect();
    p.sort_by(|a, b| a.0.cmp(b.0));
    let consulta: Vec<String> = p.iter().map(|(k, v)| format!("{k}={}", codificar(v))).collect();
    let url = if consulta.is_empty() { format!("{base}{ruta}") } else { format!("{base}{ruta}?{}", consulta.join("&")) };

    let r = http::pedir(metodo, &url, &cabeceras, (!bytes.is_empty()).then_some(bytes.as_slice()), PLAZO)
        .map_err(|e| ErrorTuya::red(format!("No pude hablar con Tuya. {e}")))?;
    let v: Value = serde_json::from_slice(&r.cuerpo)
        .map_err(|_| ErrorTuya::red(format!("Tuya respondió algo que no entiendo (HTTP {}).", r.estado)))?;
    if v["success"].as_bool() == Some(true) {
        return Ok(v["result"].clone());
    }
    let codigo = v["code"].as_i64().unwrap_or(0);
    Err(ErrorTuya { codigo: Some(codigo), texto: traducir(codigo, v["msg"].as_str().unwrap_or("")) })
}

/// El token: el que hay si sirve; si venció, se renueva; si no, uno nuevo.
fn token(c: &Credenciales) -> Result<String, ErrorTuya> {
    let de = format!("{}@{}", c.id, c.region);
    let renovar = {
        let s = SESION.lock().unwrap_or_else(|e| e.into_inner());
        match s.as_ref().filter(|s| s.de == de) {
            Some(s) if s.vence > Instant::now() + Duration::from_secs(60) => return Ok(s.token.clone()),
            Some(s) => Some(s.renovar.clone()),
            None => None,
        }
    };
    let r = match renovar.filter(|r| !r.is_empty()) {
        Some(r) => llamar(c, "", "GET", &format!("/v1.0/token/{r}"), &[], None)
            .or_else(|_| llamar(c, "", "GET", "/v1.0/token", &[("grant_type", "1".into())], None)),
        None => llamar(c, "", "GET", "/v1.0/token", &[("grant_type", "1".into())], None),
    }?;
    let token = r["access_token"].as_str().unwrap_or_default().to_string();
    if token.is_empty() {
        return Err(ErrorTuya::red("Tuya no dio un token.".into()));
    }
    let segundos = r["expire_time"].as_u64().unwrap_or(7200).clamp(120, 86_400);
    *SESION.lock().unwrap_or_else(|e| e.into_inner()) = Some(Sesion {
        de,
        token: token.clone(),
        renovar: r["refresh_token"].as_str().unwrap_or_default().to_string(),
        vence: Instant::now() + Duration::from_secs(segundos),
    });
    Ok(token)
}

/// Una llamada con token; si Tuya dice que venció, se pide otro y se intenta una vez más.
fn con_token(
    c: &Credenciales,
    metodo: &str,
    ruta: &str,
    parametros: &[(&str, String)],
    cuerpo: Option<&Value>,
) -> Result<Value, ErrorTuya> {
    let t = token(c)?;
    match llamar(c, &t, metodo, ruta, parametros, cuerpo) {
        Err(e) if e.token_vencido() => {
            olvidar_sesion();
            let t = token(c)?;
            llamar(c, &t, metodo, ruta, parametros, cuerpo)
        }
        r => r,
    }
}

// ---------------------------------------------------------------- Aparatos

/// Un aparato de la cuenta, tal como lo da Tuya.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DispositivoTuya {
    pub id: String,
    pub nombre: String,
    pub categoria: String,
    pub producto: Option<String>,
    pub en_linea: bool,
    /// Los puntos de datos tal como llegan (código → valor).
    pub crudo: BTreeMap<String, Value>,
}

fn estado_crudo(lista: &Value) -> BTreeMap<String, Value> {
    lista
        .as_array()
        .map(|l| {
            l.iter()
                .filter_map(|x| Some((x["code"].as_str()?.to_string(), x["value"].clone())))
                .collect()
        })
        .unwrap_or_default()
}

fn texto(v: &Value) -> String {
    v.as_str().unwrap_or_default().trim().to_string()
}

/// Cómo se leyó la lista la última vez (así se vuelve a leer igual).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fuente {
    /// `associated-users/devices`: una llamada trae todo.
    #[default]
    Asociados,
    /// `cloud/thing/device` y el estado aparte.
    Proyecto,
}

/// La lista de aparatos con su estado, y por dónde se leyó.
pub fn listar(c: &Credenciales) -> Result<(Vec<DispositivoTuya>, Fuente), ErrorTuya> {
    match listar_asociados(c) {
        Ok(l) => Ok((l, Fuente::Asociados)),
        Err(e) if e.sin_api() => listar_proyecto(c).map(|l| (l, Fuente::Proyecto)),
        Err(e) => Err(e),
    }
}

fn listar_asociados(c: &Credenciales) -> Result<Vec<DispositivoTuya>, ErrorTuya> {
    let mut lista = Vec::new();
    let mut desde = String::new();
    for _ in 0..10 {
        let mut p = vec![("size", "50".to_string())];
        if !desde.is_empty() {
            p.push(("last_row_key", desde.clone()));
        }
        let r = con_token(c, "GET", "/v1.0/iot-01/associated-users/devices", &p, None)?;
        for d in r["devices"].as_array().into_iter().flatten() {
            lista.push(DispositivoTuya {
                id: texto(&d["id"]),
                nombre: texto(&d["name"]),
                categoria: texto(&d["category"]),
                producto: Some(texto(&d["product_name"])).filter(|p| !p.is_empty()),
                en_linea: d["online"].as_bool().unwrap_or(false),
                crudo: estado_crudo(&d["status"]),
            });
        }
        desde = texto(&r["last_row_key"]);
        if r["has_more"].as_bool() != Some(true) || desde.is_empty() {
            break;
        }
    }
    lista.retain(|d| !d.id.is_empty());
    Ok(lista)
}

fn listar_proyecto(c: &Credenciales) -> Result<Vec<DispositivoTuya>, ErrorTuya> {
    let mut lista: Vec<DispositivoTuya> = Vec::new();
    for _ in 0..15 {
        let mut p = vec![("page_size", "20".to_string())];
        if let Some(u) = lista.last() {
            p.push(("last_id", u.id.clone()));
        }
        let r = con_token(c, "GET", "/v2.0/cloud/thing/device", &p, None)?;
        let pagina = r.as_array().cloned().unwrap_or_default();
        for d in &pagina {
            lista.push(DispositivoTuya {
                id: texto(&d["id"]),
                nombre: Some(texto(&d["customName"])).filter(|n| !n.is_empty()).unwrap_or_else(|| texto(&d["name"])),
                categoria: texto(&d["category"]),
                producto: Some(texto(&d["productName"])).filter(|p| !p.is_empty()),
                en_linea: d["isOnline"].as_bool().unwrap_or(false),
                crudo: BTreeMap::new(),
            });
        }
        if pagina.len() < 20 {
            break;
        }
    }
    lista.retain(|d| !d.id.is_empty());
    let ids: Vec<String> = lista.iter().map(|d| d.id.clone()).collect();
    for (id, crudo) in estados(c, &ids)? {
        if let Some(d) = lista.iter_mut().find(|d| d.id == id) {
            d.crudo = crudo;
        }
    }
    Ok(lista)
}

/// Los puntos de datos de cada aparato: (id, código → valor).
pub type Estados = Vec<(String, BTreeMap<String, Value>)>;

/// El estado de varios aparatos, de a 20 por llamada.
pub fn estados(c: &Credenciales, ids: &[String]) -> Result<Estados, ErrorTuya> {
    let mut salida = Vec::new();
    for tanda in ids.chunks(20) {
        let r = con_token(c, "GET", "/v1.0/iot-03/devices/status", &[("device_ids", tanda.join(","))], None)?;
        for d in r.as_array().into_iter().flatten() {
            salida.push((texto(&d["id"]), estado_crudo(&d["status"])));
        }
    }
    Ok(salida)
}

/// Una función de un aparato (lo que acepta como orden), como la da su especificación.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Funcion {
    pub code: String,
    #[serde(rename = "type", default)]
    pub tipo: String,
    #[serde(default)]
    pub values: String,
}

/// Lo que acepta un aparato. Vacío si Tuya no la da (entonces se deduce de su estado).
pub fn funciones(c: &Credenciales, id: &str) -> Result<Vec<Funcion>, ErrorTuya> {
    let r = match con_token(c, "GET", &format!("/v1.0/iot-03/devices/{id}/specification"), &[], None) {
        Err(e) if e.sin_api() => con_token(c, "GET", &format!("/v1.0/devices/{id}/specifications"), &[], None)?,
        r => r?,
    };
    Ok(serde_json::from_value(r["functions"].clone()).unwrap_or_default())
}

/// Manda varias órdenes juntas a un aparato.
pub fn enviar(c: &Credenciales, id: &str, comandos: &[(String, Value)]) -> Result<(), ErrorTuya> {
    if comandos.is_empty() {
        return Ok(());
    }
    let cuerpo = json!({ "commands": comandos.iter().map(|(code, value)| json!({ "code": code, "value": value })).collect::<Vec<_>>() });
    match con_token(c, "POST", &format!("/v1.0/iot-03/devices/{id}/commands"), &[], Some(&cuerpo)) {
        Err(e) if e.sin_api() => con_token(c, "POST", &format!("/v1.0/devices/{id}/commands"), &[], Some(&cuerpo)).map(|_| ()),
        r => r.map(|_| ()),
    }
}

/// Prueba una región: si da token y lista. `Ok` con la lista, o el error de ese centro de datos.
pub fn probar(c: &Credenciales) -> Result<(Vec<DispositivoTuya>, Fuente), ErrorTuya> {
    olvidar_sesion();
    token(c)?;
    listar(c)
}

/// "Detectar": prueba los centros de datos en orden. El primero con aparatos gana; si ninguno tiene, el primero
/// que conectó. Si las credenciales no son, se dice eso.
pub fn detectar(id: &str, secreto: &str) -> Result<(String, Vec<DispositivoTuya>, Fuente), ErrorTuya> {
    let mut vacio: Option<(String, Vec<DispositivoTuya>, Fuente)> = None;
    let mut primer_error: Option<ErrorTuya> = None;
    for (region, _, _) in REGIONES {
        let c = Credenciales { id: id.into(), secreto: secreto.into(), region: (*region).into() };
        match probar(&c) {
            Ok((l, f)) if !l.is_empty() => return Ok(((*region).into(), l, f)),
            Ok((l, f)) => {
                if vacio.is_none() {
                    vacio = Some(((*region).into(), l, f));
                }
            }
            Err(e) if e.codigo.is_none() && primer_error.is_none() => return Err(e),
            Err(e) => {
                // Unas credenciales que no son se prefieren a otros errores, que pueden ser del centro equivocado.
                if primer_error.as_ref().is_none_or(|p| !p.credenciales() && e.credenciales()) {
                    primer_error = Some(e);
                }
            }
        }
    }
    olvidar_sesion();
    vacio.ok_or_else(|| primer_error.unwrap_or_else(|| ErrorTuya::red("No pude conectar con ningún centro de datos.".into())))
}

// ---------------------------------------------------------------- De los puntos de datos a Nexo

/// Un número con su rango ("bright_value_v2" de 10 a 1000).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rango {
    pub codigo: String,
    pub min: i64,
    pub max: i64,
}

/// El color: rangos de saturación y brillo (el tono va de 0 a 360).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RangoColor {
    pub codigo: String,
    pub s_max: i64,
    pub v_max: i64,
}

/// Cómo se habla con un aparato de Tuya: qué códigos tiene y sus rangos.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Puntos {
    pub encendido: Option<String>,
    pub brillo: Option<Rango>,
    pub temperatura: Option<Rango>,
    pub color: Option<RangoColor>,
    /// `work_mode` y sus valores ("white", "colour", "scene"…).
    pub modo: Option<String>,
    pub modos: Vec<String>,
}

impl Puntos {
    pub fn capacidades(&self) -> Capacidades {
        let mut c = Capacidades::luz(self.brillo.is_some(), self.temperatura.is_some(), self.color.is_some());
        c.encender = self.encendido.is_some();
        c
    }
}

const ENCENDIDO: [&str; 5] = ["switch_led", "switch_led_1", "switch", "switch_1", "led_switch"];
const BRILLO: [&str; 3] = ["bright_value_v2", "bright_value", "bright_value_1"];
const TEMPERATURA: [&str; 3] = ["temp_value_v2", "temp_value", "temp_value_1"];
const COLOR: [&str; 2] = ["colour_data_v2", "colour_data"];

/// Un valor Json de Tuya: a veces llega como texto ("{\"h\":0,…}") y a veces como objeto.
fn json_de(v: &Value) -> Value {
    match v {
        Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
        otro => otro.clone(),
    }
}

fn rango_de(codigo: &str, valores: &Value, si_no: (i64, i64)) -> Rango {
    let min = valores["min"].as_i64().unwrap_or(si_no.0);
    let max = valores["max"].as_i64().unwrap_or(si_no.1);
    let (min, max) = if max > min { (min, max) } else { si_no };
    Rango { codigo: codigo.into(), min, max }
}

/// El rango de fábrica cuando la especificación no lo dice: los `_v2` van hasta 1000; los viejos hasta 255,
/// salvo que el valor de ahora ya pase de 255 (las tiras "dd" usan nombres viejos con rangos nuevos).
fn rango_supuesto(codigo: &str, ahora: Option<i64>, brillo: bool) -> (i64, i64) {
    let mil = codigo.ends_with("_v2") || codigo.ends_with("_1") || ahora.is_some_and(|v| v > 255);
    match (brillo, mil) {
        (true, true) => (10, 1000),
        (true, false) => (25, 255),
        (false, true) => (0, 1000),
        (false, false) => (0, 255),
    }
}

/// Lee los puntos de la especificación; lo que no esté ahí se deduce del estado (con rangos de fábrica).
pub fn puntos(funciones: &[Funcion], crudo: &BTreeMap<String, Value>) -> Puntos {
    let f = |codigos: &[&str], tipo: &str| {
        codigos.iter().find_map(|c| funciones.iter().find(|x| x.code == *c && x.tipo.eq_ignore_ascii_case(tipo)))
    };
    let en_estado = |codigos: &[&str]| codigos.iter().find(|c| crudo.contains_key(**c)).map(|c| c.to_string());
    let valores = |x: &Funcion| serde_json::from_str::<Value>(&x.values).unwrap_or(Value::Null);

    let encendido = f(&ENCENDIDO, "Boolean").map(|x| x.code.clone()).or_else(|| en_estado(&ENCENDIDO));
    let brillo = match f(&BRILLO, "Integer") {
        Some(x) => Some(rango_de(&x.code, &valores(x), rango_supuesto(&x.code, None, true))),
        None => en_estado(&BRILLO).map(|c| {
            let ahora = crudo.get(&c).and_then(Value::as_i64);
            let (min, max) = rango_supuesto(&c, ahora, true);
            Rango { codigo: c, min, max }
        }),
    };
    let temperatura = match f(&TEMPERATURA, "Integer") {
        Some(x) => Some(rango_de(&x.code, &valores(x), rango_supuesto(&x.code, None, false))),
        None => en_estado(&TEMPERATURA).map(|c| {
            let ahora = crudo.get(&c).and_then(Value::as_i64);
            let (min, max) = rango_supuesto(&c, ahora, false);
            Rango { codigo: c, min, max }
        }),
    };
    let color = match f(&COLOR, "Json") {
        Some(x) => {
            let v = valores(x);
            let mil = x.code.ends_with("_v2");
            let tope = |eje: &str| v[eje]["max"].as_i64().filter(|m| *m > 0).unwrap_or(if mil { 1000 } else { 255 });
            Some(RangoColor { codigo: x.code.clone(), s_max: tope("s"), v_max: tope("v") })
        }
        None => en_estado(&COLOR).map(|c| {
            let ahora = crudo.get(&c).map(json_de).unwrap_or(Value::Null);
            let mil = c.ends_with("_v2") || ahora["s"].as_i64().unwrap_or(0) > 255 || ahora["v"].as_i64().unwrap_or(0) > 255;
            let tope = if mil { 1000 } else { 255 };
            RangoColor { codigo: c, s_max: tope, v_max: tope }
        }),
    };
    let modo_f = funciones.iter().find(|x| x.code == "work_mode");
    let modos: Vec<String> = modo_f
        .map(|x| valores(x)["range"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let modo = modo_f.map(|x| x.code.clone()).or_else(|| crudo.contains_key("work_mode").then(|| "work_mode".to_string()));
    let modos = if modos.is_empty() && modo.is_some() { vec!["white".into(), "colour".into()] } else { modos };
    Puntos { encendido, brillo, temperatura, color, modo, modos }
}

/// El estado en valores de Nexo.
pub fn estado(p: &Puntos, crudo: &BTreeMap<String, Value>) -> EstadoAparato {
    let entero = |c: &str| crudo.get(c).and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())));
    let encendido = p.encendido.as_ref().and_then(|c| crudo.get(c)).and_then(Value::as_bool);
    let color = p.color.as_ref().and_then(|r| {
        let v = json_de(crudo.get(&r.codigo)?);
        let h = v["h"].as_i64()?;
        Some(Hsv::nuevo(h, de_rango(v["s"].as_i64()?, 0, r.s_max) as i64, de_rango(v["v"].as_i64()?, 0, r.v_max) as i64))
    });
    let modo = match p.modo.as_ref().and_then(|c| crudo.get(c)).and_then(Value::as_str) {
        Some("colour") => Some(ModoLuz::Color),
        Some("white") => Some(ModoLuz::Blanco),
        Some(_) => Some(ModoLuz::Escena),
        None if p.color.is_some() && p.brillo.is_none() && p.temperatura.is_none() => Some(ModoLuz::Color),
        None if p.brillo.is_some() || p.temperatura.is_some() || p.color.is_some() => Some(ModoLuz::Blanco),
        None => None,
    };
    let brillo = match (modo, color) {
        (Some(ModoLuz::Color), Some(c)) => Some(c.v.max(1)),
        _ => p.brillo.as_ref().and_then(|r| entero(&r.codigo).map(|v| brillo_de_rango(v, r.min, r.max))),
    };
    let temperatura = p.temperatura.as_ref().and_then(|r| entero(&r.codigo).map(|v| de_rango(v, r.min, r.max)));
    EstadoAparato {
        encendido,
        brillo,
        temperatura,
        color,
        modo,
        abierto: crudo.get("doorcontact_state").and_then(Value::as_bool),
        bateria: entero("battery_percentage").or_else(|| entero("residual_electricity")).map(|b| b.clamp(0, 100) as u8),
        ..EstadoAparato::default()
    }
}

/// Las órdenes de Tuya para un cambio, en el orden en que se aplican (encender, modo y valor).
pub fn comandos(p: &Puntos, actual: &EstadoAparato, cambio: &Cambio) -> Vec<(String, Value)> {
    let mut v: Vec<(String, Value)> = Vec::new();
    if let (Some(on), Some(c)) = (cambio.encendido, &p.encendido) {
        v.push((c.clone(), json!(on)));
    }
    let modo = |v: &mut Vec<(String, Value)>, cual: &str| {
        if let Some(m) = &p.modo
            && p.modos.iter().any(|x| x == cual)
        {
            v.push((m.clone(), json!(cual)));
        }
    };
    let color_json = |r: &RangoColor, c: Hsv| {
        let texto = json!({ "h": c.h % 360, "s": a_rango(c.s, 0, r.s_max), "v": a_rango(c.v.max(1), 0, r.v_max) }).to_string();
        (r.codigo.clone(), Value::String(texto))
    };
    if let (Some(c), Some(r)) = (cambio.color, &p.color) {
        modo(&mut v, "colour");
        v.push(color_json(r, c));
    } else if let (Some(t), Some(r)) = (cambio.temperatura, &p.temperatura) {
        modo(&mut v, "white");
        v.push((r.codigo.clone(), json!(a_rango(t, r.min, r.max))));
        if let (Some(b), Some(rb)) = (cambio.brillo, &p.brillo) {
            v.push((rb.codigo.clone(), json!(brillo_a_rango(b, rb.min, rb.max))));
        }
    } else if let Some(b) = cambio.brillo {
        // En color, el brillo es el del color; en blanco, el suyo.
        match (actual.modo, actual.color, &p.color, &p.brillo) {
            (Some(ModoLuz::Color), Some(c), Some(r), _) => v.push(color_json(r, Hsv { v: b, ..c })),
            (_, _, _, Some(rb)) => v.push((rb.codigo.clone(), json!(brillo_a_rango(b, rb.min, rb.max)))),
            (_, _, Some(r), None) => v.push(color_json(r, Hsv { v: b, ..actual.color.unwrap_or(Hsv { h: 0, s: 0, v: b }) })),
            _ => {}
        }
    }
    v
}

/// Aplica las órdenes ya mandadas a los valores guardados (así se ve el cambio sin volver a preguntar).
pub fn aplicar(crudo: &mut BTreeMap<String, Value>, comandos: &[(String, Value)]) {
    for (c, v) in comandos {
        crudo.insert(c.clone(), v.clone());
    }
}

/// La clase por la categoría de Tuya (y, si no se conoce, por lo que sabe hacer).
pub fn clase_de(categoria: &str, p: &Puntos) -> Clase {
    match categoria {
        "dj" | "dd" | "dc" | "fwd" | "xdd" | "tgq" | "tyndj" | "fsd" | "sxd" | "gyd" | "tgkg" | "dsd" | "mbd" | "hcdd" => {
            Clase::Luz
        }
        "cz" | "pc" | "kg" | "tdq" | "dlq" | "wkcz" | "znrb" => Clase::Enchufe,
        "ms" | "jtmspro" | "jtmsbh" | "videolock" | "bxx" | "gyms" | "hotelms" | "ms_category" => Clase::Cerradura,
        "mcs" => Clase::SensorPuerta,
        "cl" | "clkg" | "mc" | "ckmkzq" => Clase::Cortina,
        "kt" | "wk" | "wkf" | "qn" | "ktkzq" => Clase::Clima,
        _ if p.brillo.is_some() || p.color.is_some() || p.temperatura.is_some() => Clase::Luz,
        _ if p.encendido.is_some() => Clase::Enchufe,
        _ => Clase::Otro,
    }
}

/// Cómo es una luz por su categoría, su nombre y su modelo.
pub fn forma_de(categoria: &str, nombre: &str, producto: &str) -> FormaLuz {
    let t = normalizar(&format!("{nombre} {producto}"));
    let tiene = |palabras: &[&str]| palabras.iter().any(|p| t.contains(p));
    if tiene(&["barra", "monitor", "screen", "light bar", "lightbar", "pantalla"]) {
        FormaLuz::Barra
    } else if categoria == "dd" || tiene(&["tira", "strip", "cinta", "led tape"]) {
        FormaLuz::Tira
    } else if tiene(&["lampara", "lamp", "desk", "velador"]) {
        FormaLuz::Lampara
    } else {
        FormaLuz::Foco
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn f(code: &str, tipo: &str, values: &str) -> Funcion {
        Funcion { code: code.into(), tipo: tipo.into(), values: values.into() }
    }

    fn crudo(pares: &[(&str, Value)]) -> BTreeMap<String, Value> {
        pares.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    /// Una tira RGB "dj" con los puntos v2, como la da la especificación.
    fn tira() -> Vec<Funcion> {
        vec![
            f("switch_led", "Boolean", "{}"),
            f("work_mode", "Enum", r#"{"range":["white","colour","scene","music"]}"#),
            f("bright_value_v2", "Integer", r#"{"min":10,"max":1000,"scale":0,"step":1}"#),
            f("temp_value_v2", "Integer", r#"{"min":0,"max":1000,"scale":0,"step":1}"#),
            f("colour_data_v2", "Json", r#"{"h":{"min":0,"scale":0,"unit":"","max":360,"step":1},"s":{"min":0,"scale":0,"unit":"","max":1000,"step":1},"v":{"min":0,"scale":0,"unit":"","max":1000,"step":1}}"#),
        ]
    }

    #[test]
    fn hogar_tuya_puntos_de_una_tira_v2() {
        let e = crudo(&[
            ("switch_led", json!(true)),
            ("work_mode", json!("colour")),
            ("bright_value_v2", json!(500)),
            ("colour_data_v2", json!(r#"{"h":200,"s":1000,"v":500}"#)),
        ]);
        let p = puntos(&tira(), &e);
        assert_eq!(p.encendido.as_deref(), Some("switch_led"));
        assert_eq!(p.brillo, Some(Rango { codigo: "bright_value_v2".into(), min: 10, max: 1000 }));
        assert_eq!(p.color, Some(RangoColor { codigo: "colour_data_v2".into(), s_max: 1000, v_max: 1000 }));
        assert_eq!(p.modos, ["white", "colour", "scene", "music"]);
        assert_eq!(p.capacidades(), Capacidades::luz(true, true, true));
        let s = estado(&p, &e);
        assert_eq!(s.encendido, Some(true));
        assert_eq!(s.modo, Some(ModoLuz::Color));
        assert_eq!(s.color, Some(Hsv { h: 200, s: 100, v: 50 }));
        // En color, el brillo es el del color.
        assert_eq!(s.brillo, Some(50));
        assert_eq!(clase_de("dj", &p), Clase::Luz);
    }

    #[test]
    fn hogar_tuya_puntos_viejos_sin_especificacion() {
        // Un foco viejo: bright_value 25–255 y temp_value 0–255, sin especificación.
        let e = crudo(&[("switch_led", json!(false)), ("bright_value", json!(255)), ("temp_value", json!(0))]);
        let p = puntos(&[], &e);
        assert_eq!(p.brillo, Some(Rango { codigo: "bright_value".into(), min: 25, max: 255 }));
        assert_eq!(p.temperatura, Some(Rango { codigo: "temp_value".into(), min: 0, max: 255 }));
        let s = estado(&p, &e);
        assert_eq!((s.encendido, s.brillo, s.temperatura, s.modo), (Some(false), Some(100), Some(0), Some(ModoLuz::Blanco)));
        // Una tira "dd" con nombres viejos y valores de hasta 1000.
        let e = crudo(&[("switch_led", json!(true)), ("bright_value", json!(800))]);
        assert_eq!(puntos(&[], &e).brillo, Some(Rango { codigo: "bright_value".into(), min: 10, max: 1000 }));
    }

    #[test]
    fn hogar_tuya_enchufe() {
        let funciones = [f("switch_1", "Boolean", "{}"), f("countdown_1", "Integer", r#"{"min":0,"max":86400}"#)];
        let e = crudo(&[("switch_1", json!(true)), ("cur_power", json!(123))]);
        let p = puntos(&funciones, &e);
        assert_eq!(p.encendido.as_deref(), Some("switch_1"));
        assert_eq!(p.capacidades(), Capacidades::enchufe());
        assert_eq!(clase_de("cz", &p), Clase::Enchufe);
        assert_eq!(estado(&p, &e).encendido, Some(true));
        assert_eq!(comandos(&p, &estado(&p, &e), &Cambio { encendido: Some(false), ..Default::default() }), vec![("switch_1".into(), json!(false))]);
    }

    #[test]
    fn hogar_tuya_ordenes_de_cada_cambio() {
        let e = crudo(&[("switch_led", json!(false)), ("work_mode", json!("white")), ("bright_value_v2", json!(10)), ("colour_data_v2", json!(r#"{"h":0,"s":1000,"v":1000}"#))]);
        let p = puntos(&tira(), &e);
        let s = estado(&p, &e);
        // Brillo al 100 % y encender.
        let c = comandos(&p, &s, &Cambio { encendido: Some(true), brillo: Some(100), ..Default::default() });
        assert_eq!(c, vec![("switch_led".into(), json!(true)), ("bright_value_v2".into(), json!(1000))]);
        // Blanco cálido: pasa a blanco.
        let c = comandos(&p, &s, &Cambio { temperatura: Some(0), ..Default::default() });
        assert_eq!(c, vec![("work_mode".into(), json!("white")), ("temp_value_v2".into(), json!(0))]);
        // Color: pasa a color, con el JSON como texto (como lo manda Home Assistant).
        let c = comandos(&p, &s, &Cambio { color: Some(Hsv { h: 120, s: 50, v: 100 }), ..Default::default() });
        assert_eq!(c[0], ("work_mode".into(), json!("colour")));
        assert_eq!(c[1], ("colour_data_v2".into(), Value::String(r#"{"h":120,"s":500,"v":1000}"#.into())));
        // En color, el brillo cambia la `v` del color.
        let mut crudo2 = e.clone();
        aplicar(&mut crudo2, &c);
        let s2 = estado(&p, &crudo2);
        assert_eq!(s2.modo, Some(ModoLuz::Color));
        let c = comandos(&p, &s2, &Cambio { brillo: Some(30), ..Default::default() });
        assert_eq!(c, vec![("colour_data_v2".into(), Value::String(r#"{"h":120,"s":500,"v":300}"#.into()))]);
    }

    #[test]
    fn hogar_tuya_clases_y_formas() {
        let nada = Puntos::default();
        assert_eq!(clase_de("ms", &nada), Clase::Cerradura);
        assert_eq!(clase_de("mcs", &nada), Clase::SensorPuerta);
        assert_eq!(clase_de("cl", &nada), Clase::Cortina);
        assert_eq!(clase_de("kt", &nada), Clase::Clima);
        assert_eq!(clase_de("zzz", &nada), Clase::Otro);
        assert_eq!(forma_de("dj", "Barra del monitor", ""), FormaLuz::Barra);
        assert_eq!(forma_de("dj", "Desk light", "Screen Light Bar"), FormaLuz::Barra);
        assert_eq!(forma_de("dd", "Escritorio", ""), FormaLuz::Tira);
        assert_eq!(forma_de("dj", "LED Strip", ""), FormaLuz::Tira);
        assert_eq!(forma_de("dj", "Foco de la sala", "Smart Bulb A60"), FormaLuz::Foco);
        assert_eq!(forma_de("dj", "Lámpara", ""), FormaLuz::Lampara);
        // Un sensor de puerta: abierta y batería.
        let e = crudo(&[("doorcontact_state", json!(true)), ("battery_percentage", json!(87))]);
        let s = estado(&puntos(&[], &e), &e);
        assert_eq!((s.abierto, s.bateria), (Some(true), Some(87)));
    }

    #[test]
    fn hogar_tuya_errores_en_palabras() {
        assert!(traducir(1004, "sign invalid").starts_with("El Access ID o el Access Secret"));
        assert!(traducir(28841002, "").contains("plan de prueba"));
        assert!(traducir(28841105, "").contains("IoT Core"));
        assert_eq!(traducir(9999, "boom"), "Tuya respondió con un error (9999: boom).");
        let e = ErrorTuya { codigo: Some(2001), texto: traducir(2001, "") };
        assert_eq!(con_nombre(&e, "La tira"), "La tira está sin conexión. Revisa que tenga corriente y Wi-Fi.");
        assert!(ErrorTuya { codigo: Some(1010), texto: String::new() }.token_vencido());
        assert!(ErrorTuya { codigo: Some(1108), texto: String::new() }.sin_api());
        assert_eq!(codificar("a,b c/é"), "a,b%20c%2F%C3%A9");
        assert!(region_valida("us-e") && !region_valida("marte"));
    }
}
