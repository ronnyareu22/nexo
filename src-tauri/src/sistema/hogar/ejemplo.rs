//! Una casa de ejemplo, sin cuentas ni aparatos: para revisar Nexo y para las pruebas.
//! En la app solo se usa compilada para desarrollo y con `NEXO_CASA=ejemplo`.
//! - Tuya: una tira RGB ("Tira del escritorio"), dos focos blancos, la barra del monitor y un enchufe.
//! - Home Assistant: la tele de la sala, el sensor y la cerradura de la entrada, una cortina y dos luces.
//! - Al buscar: un foco WiZ, una tira Govee en la red, una tira Bluetooth ELK-BLEDOM, la tira de Tuya vista en la
//!   red, la barra de Xiaomi (solo se detecta), una tele Samsung y un Home Assistant.

use super::ha;
use super::modelo::{Capacidades, Clase, FormaLuz, Hallazgo, Manejo, Proveedor, Via};
use super::tuya::{DispositivoTuya, Funcion};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Solo en desarrollo y con `NEXO_CASA=ejemplo`.
pub fn activo() -> bool {
    cfg!(debug_assertions) && std::env::var("NEXO_CASA").is_ok_and(|v| v == "ejemplo")
}

fn f(code: &str, tipo: &str, values: &str) -> Funcion {
    Funcion { code: code.into(), tipo: tipo.into(), values: values.into() }
}

fn blanca() -> Vec<Funcion> {
    vec![
        f("switch_led", "Boolean", "{}"),
        f("work_mode", "Enum", r#"{"range":["white","scene"]}"#),
        f("bright_value_v2", "Integer", r#"{"min":10,"max":1000}"#),
        f("temp_value_v2", "Integer", r#"{"min":0,"max":1000}"#),
    ]
}

fn d(id: &str, nombre: &str, categoria: &str, producto: &str, pares: &[(&str, Value)]) -> DispositivoTuya {
    DispositivoTuya {
        id: id.into(),
        nombre: nombre.into(),
        categoria: categoria.into(),
        producto: Some(producto.into()),
        en_linea: true,
        crudo: pares.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<BTreeMap<_, _>>(),
    }
}

/// Los aparatos de Tuya de ejemplo, con lo que acepta cada uno.
pub fn tuya() -> Vec<(DispositivoTuya, Vec<Funcion>)> {
    let mut rgb = blanca();
    rgb[1] = f("work_mode", "Enum", r#"{"range":["white","colour","scene","music"]}"#);
    rgb.push(f("colour_data_v2", "Json", r#"{"h":{"min":0,"max":360},"s":{"min":0,"max":1000},"v":{"min":0,"max":1000}}"#));
    vec![
        (
            d("ejtira01", "Tira del escritorio", "dj", "RGB Strip Light", &[
                ("switch_led", json!(true)),
                ("work_mode", json!("colour")),
                ("bright_value_v2", json!(700)),
                ("temp_value_v2", json!(500)),
                ("colour_data_v2", json!(r#"{"h":190,"s":800,"v":700}"#)),
            ]),
            rgb,
        ),
        (
            d("ejbarra01", "Barra del monitor", "dj", "Screen Light Bar", &[
                ("switch_led", json!(true)),
                ("work_mode", json!("white")),
                ("bright_value_v2", json!(620)),
                ("temp_value_v2", json!(380)),
            ]),
            blanca(),
        ),
        (
            d("ejfoco01", "Foco de la sala", "dj", "Smart Bulb A60", &[
                ("switch_led", json!(true)),
                ("work_mode", json!("white")),
                ("bright_value_v2", json!(800)),
                ("temp_value_v2", json!(150)),
            ]),
            blanca(),
        ),
        (
            d("ejfoco02", "Foco del cuarto", "dj", "Smart Bulb A60", &[
                ("switch_led", json!(false)),
                ("work_mode", json!("white")),
                ("bright_value_v2", json!(400)),
                ("temp_value_v2", json!(0)),
            ]),
            blanca(),
        ),
        (
            d("ejenchufe01", "Enchufe del ventilador", "cz", "Smart Plug", &[("switch_1", json!(false))]),
            vec![f("switch_1", "Boolean", "{}")],
        ),
    ]
}

/// El Home Assistant de ejemplo: estados como los da `/api/states`.
pub fn home_assistant() -> Vec<ha::Entidad> {
    let estados = vec![
        json!({ "entity_id": "media_player.tele_sala", "state": "on", "attributes": {
            "friendly_name": "Tele de la sala", "supported_features": 128 | 256 | 4 | 8 | 2048,
            "volume_level": 0.3, "is_volume_muted": false, "source": "HDMI 1",
            "source_list": ["HDMI 1", "HDMI 2", "Netflix", "YouTube"] } }),
        json!({ "entity_id": "binary_sensor.puerta_entrada", "state": "off", "attributes": {
            "friendly_name": "Puerta de entrada", "device_class": "door", "battery_level": 90 } }),
        json!({ "entity_id": "lock.cerradura_entrada", "state": "locked", "attributes": {
            "friendly_name": "Cerradura de la entrada" } }),
        json!({ "entity_id": "cover.cortina_sala", "state": "open", "attributes": {
            "friendly_name": "Cortina de la sala", "supported_features": 15, "current_position": 70 } }),
        json!({ "entity_id": "light.lampara_sala", "state": "on", "attributes": {
            "friendly_name": "Lámpara de pie", "supported_color_modes": ["color_temp"], "color_mode": "color_temp",
            "brightness": 180, "color_temp_kelvin": 3000, "min_color_temp_kelvin": 2200, "max_color_temp_kelvin": 6500 } }),
        json!({ "entity_id": "light.foco_cocina", "state": "off", "attributes": {
            "friendly_name": "Foco de la cocina", "supported_color_modes": ["brightness"], "brightness": 200 } }),
        json!({ "entity_id": "sensor.temperatura_sala", "state": "22.5", "attributes": {
            "friendly_name": "Temperatura de la sala", "device_class": "temperature" } }),
    ];
    let areas = [
        ("media_player.tele_sala", "Sala"),
        ("binary_sensor.puerta_entrada", "Entrada"),
        ("lock.cerradura_entrada", "Entrada"),
        ("cover.cortina_sala", "Sala"),
        ("light.lampara_sala", "Sala"),
        ("light.foco_cocina", "Cocina"),
    ]
    .into_iter()
    .map(|(id, a)| (id.to_string(), (a.to_string(), Vec::new())))
    .collect();
    ha::entidades(&estados, &areas)
}

/// Dónde va cada aparato de ejemplo si nadie lo cambió.
pub fn habitacion_de(id: &str) -> Option<&'static str> {
    match id {
        "tuya:ejtira01" | "tuya:ejbarra01" => Some("escritorio"),
        "tuya:ejfoco01" => Some("sala"),
        "tuya:ejfoco02" | "tuya:ejenchufe01" => Some("cuarto"),
        _ => None,
    }
}

/// Lo que encuentra una búsqueda de ejemplo.
pub fn hallazgos() -> Vec<Hallazgo> {
    let h = |id: &str, via: Via, proveedor: Proveedor, clase: Clase, forma: Option<FormaLuz>, marca: &str, nombre: &str| Hallazgo {
        id: id.into(),
        via,
        proveedor,
        clase,
        forma,
        marca: marca.into(),
        nombre: nombre.into(),
        modelo: None,
        direccion: None,
        mac: None,
        senal: None,
        version: None,
        capacidades: Capacidades::ninguna(),
        estado: None,
        manejo: Manejo::Listo,
    };
    let mut wiz = h("wiz:a8bb50d2c3e4", Via::Wifi, Proveedor::Wiz, Clase::Luz, Some(FormaLuz::Foco), "WiZ", "Foco WiZ");
    wiz.modelo = Some("ESP01_SHRGB1C_31".into());
    wiz.direccion = Some("192.168.1.41".into());
    wiz.senal = Some(-58);
    wiz.capacidades = Capacidades::luz(true, true, true);
    let mut govee = h("govee:1f80c5323236724e", Via::Wifi, Proveedor::Govee, Clase::Luz, Some(FormaLuz::Tira), "Govee", "Tira Govee H6199");
    govee.modelo = Some("H6199".into());
    govee.direccion = Some("192.168.1.52".into());
    govee.capacidades = Capacidades::luz(true, true, true);
    let mut elk = h("ble:5f1c0a9e", Via::Bluetooth, Proveedor::BleElk, Clase::Luz, Some(FormaLuz::Tira), "ELK-BLEDOM", "ELK-BLEDOM");
    elk.direccion = Some("5F1C0A9E".into());
    elk.senal = Some(-67);
    elk.capacidades = Capacidades { informa_estado: false, ..Capacidades::luz(true, true, true) };
    let mut tuya = h("tuya:ejtira01", Via::Wifi, Proveedor::Tuya, Clase::Otro, None, "Tuya / Smart Life", "Aparato Tuya");
    tuya.direccion = Some("192.168.1.37".into());
    tuya.version = Some("3.4".into());
    tuya.manejo = Manejo::CuentaTuya;
    let mut xiaomi = h("ble:9c2e7b10", Via::Bluetooth, Proveedor::Ninguno, Clase::Luz, Some(FormaLuz::Barra), "Xiaomi", "Mi Monitor Light Bar");
    xiaomi.senal = Some(-74);
    xiaomi.manejo = Manejo::Pronto { motivo: "Usa el cifrado propio de Xiaomi; Nexo no puede manejarla por Bluetooth.".into() };
    let mut tele = h("ssdp:samsung-qn55", Via::Wifi, Proveedor::Ninguno, Clase::Tv, None, "Samsung", "Samsung QN55Q60");
    tele.direccion = Some("192.168.1.60".into());
    tele.manejo = Manejo::Pronto { motivo: "Detectado · pronto podrás manejarlo desde Nexo.".into() };
    let mut casa = h("ha:7f3e9a", Via::Wifi, Proveedor::HomeAssistant, Clase::Otro, None, "Home Assistant", "Casa");
    casa.direccion = Some("192.168.1.10".into());
    casa.version = Some("2026.10.2".into());
    casa.manejo = Manejo::HomeAssistant { url: "http://homeassistant.local:8123".into() };
    vec![wiz, govee, elk, tuya, xiaomi, tele, casa]
}
