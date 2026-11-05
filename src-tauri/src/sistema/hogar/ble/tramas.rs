//! Bluetooth sin tocar el aparato: reconocer luces por lo que anuncian y armar las tramas de cada protocolo.
//!
//! - ELK-BLEDOM / duoCo / Lotus Lantern (servicio FFF0, escritura FFF3): tramas de 9 bytes `7e … ef`.
//!   Fuentes: la tabla de modelos de la integración `elkbledom` de Home Assistant (dave-code-ruiz/elkbledom,
//!   `models.json`) y el protocolo de FergusInLondon/ELK-BLEDOM (`PROTCOL.md`).
//! - Triones / Happy Lighting / Magic Home (servicio FFD5, escritura FFD9): `cc 23 33`, `56 r g b 00 f0 aa`…
//!   Fuentes: madhead/saberlight (`protocols/Triones/protocol.md`) y sysofwan/ha-triones (`triones.py`).
//! - Lo que tiene cifrado propio (Tuya, Xiaomi, Govee) solo se reconoce: Nexo no lo maneja por Bluetooth.

use crate::sistema::hogar::modelo::{
    Cambio, Capacidades, Clase, FormaLuz, Hallazgo, Hsv, Manejo, Proveedor, Via, normalizar, rgb_de,
};

/// Lo que se oyó de un aparato al buscar (ya juntado: nombre, servicios y datos de todos sus anuncios).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Anuncio {
    /// Mac: el identificador que le da el sistema; Windows: la dirección en 12 cifras hexadecimales.
    pub direccion: String,
    pub nombre: Option<String>,
    /// dBm.
    pub senal: Option<i16>,
    /// UUID de los servicios anunciados (cortos o completos, como lleguen).
    pub servicios: Vec<String>,
    /// (id del fabricante, datos).
    pub fabricante: Vec<(u16, Vec<u8>)>,
    /// (UUID del servicio, datos).
    pub datos_servicio: Vec<(String, Vec<u8>)>,
}

impl Anuncio {
    /// Suma lo de otro anuncio del mismo aparato (en Windows, el nombre suele llegar en la respuesta del escaneo).
    pub fn juntar(&mut self, otro: Anuncio) {
        if self.nombre.as_deref().is_none_or(|n| n.trim().is_empty()) {
            self.nombre = otro.nombre;
        }
        self.senal = match (self.senal, otro.senal) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        for s in otro.servicios {
            if !self.servicios.contains(&s) {
                self.servicios.push(s);
            }
        }
        for f in otro.fabricante {
            if !self.fabricante.contains(&f) {
                self.fabricante.push(f);
            }
        }
        for d in otro.datos_servicio {
            if !self.datos_servicio.contains(&d) {
                self.datos_servicio.push(d);
            }
        }
    }
}

/// Un UUID de Bluetooth en corto y en mayúsculas: "0000fff0-0000-1000-8000-00805f9b34fb" → "FFF0".
pub fn uuid_corto(uuid: &str) -> String {
    let u = uuid.trim().to_ascii_uppercase();
    let base = "-0000-1000-8000-00805F9B34FB";
    if let Some(inicio) = u.strip_suffix(base)
        && inicio.len() == 8
        && inicio.starts_with("0000")
    {
        return inicio[4..].to_string();
    }
    if u.len() == 8 && u.starts_with("0000") && u.chars().all(|c| c.is_ascii_hexdigit()) {
        return u[4..].to_string();
    }
    u
}

/// El UUID completo de uno corto ("FFF0"), para pedírselo al sistema.
#[cfg(test)]
pub fn uuid_largo(corto: &str) -> String {
    format!("0000{}-0000-1000-8000-00805F9B34FB", corto.to_ascii_uppercase())
}

// ---------------------------------------------------------------- Reconocer

/// Las tiras ELK-BLEDOM y sus parientes (Lotus Lantern, duoCo Strip): nombres de `elkbledom/models.json`.
const NOMBRES_ELK: &[&str] =
    &["ELK-", "MELK", "MODELX", "LEDBLE", "DUOCO", "JACKYLED", "XROCKER", "XSL-", "LED LIGHT STRIP", "DMRRBA-"];
/// Triones / Happy Lighting / Magic Home: los nombres que reconoce `led_ble` de Home Assistant (sin LEDnetWF,
/// que usa otro protocolo).
const NOMBRES_TRIONES: &[&str] = &["TRIONES", "LEDBLUE", "DREAM~", "QHM-", "AP-", "BLE-LED", "LD-0003"];
/// Servicios propios de Triones.
const SERVICIOS_TRIONES: &[&str] = &["FFD5", "FFD0", "FFE5"];
/// Tuya: servicio FD50 e id de fabricante 0x07D0 (SDK de Tuya BLE).
const TUYA_SERVICIO: &str = "FD50";
const TUYA_FABRICANTE: u16 = 0x07D0;
/// Xiaomi: MiBeacon va en los datos del servicio FE95.
const XIAOMI_SERVICIO: &str = "FE95";
/// Govee anuncia sus luces como ihoment_H6xxx_XXXX, Govee_H6xxx_XXXX, GBK_H6xxx o Minger_H6xxx.
const NOMBRES_GOVEE: &[&str] = &["IHOMENT_", "GOVEE_", "GBK_", "MINGER_"];
/// Palabras que hacen pensar en una luz (nombre partido en palabras, sin tildes).
const PALABRAS_LUZ: &[&str] =
    &["light", "lights", "lamp", "lampara", "bulb", "strip", "luz", "luces", "foco", "led", "leds", "rgb", "rgbw", "bar"];

fn empieza(nombre: &str, prefijos: &[&str]) -> bool {
    let n = nombre.trim().to_ascii_uppercase();
    prefijos.iter().any(|p| n.starts_with(p))
}

fn parece_luz(nombre: &str) -> bool {
    let n = normalizar(nombre);
    n.split(' ').any(|p| PALABRAS_LUZ.contains(&p) || p.starts_with("yeelight") || p.starts_with("yeelink"))
}

fn tiene_servicio(a: &Anuncio, buscado: &str) -> bool {
    a.servicios.iter().any(|s| uuid_corto(s) == buscado) || a.datos_servicio.iter().any(|(s, _)| uuid_corto(s) == buscado)
}

/// Govee: el modelo va después del prefijo; las luces son H6… y H7…, los termómetros H5… (no van).
fn es_luz_govee(nombre: &str) -> bool {
    let n = nombre.trim().to_ascii_uppercase();
    let Some(resto) = NOMBRES_GOVEE.iter().find_map(|p| n.strip_prefix(p)) else { return false };
    resto.starts_with("H6") || resto.starts_with("H7") || resto.starts_with("H8")
}

/// Qué es lo que se oyó: una luz que Nexo maneja, una que solo reconoce, o nada (teléfonos, audífonos…).
pub fn clasificar(a: &Anuncio) -> Option<Hallazgo> {
    let nombre = a.nombre.as_deref().map(str::trim).filter(|n| !n.is_empty());
    let n = nombre.unwrap_or("");
    let triones_por_servicio = SERVICIOS_TRIONES.iter().any(|s| tiene_servicio(a, s));

    // Las que Nexo maneja. "LEDBLE" lo usan las dos familias: decide el servicio anunciado.
    let familia = if empieza(n, NOMBRES_TRIONES) || (empieza(n, &["LEDBLE"]) && triones_por_servicio) {
        Some(Proveedor::BleTriones)
    } else if empieza(n, NOMBRES_ELK) || (tiene_servicio(a, "FFF0") && parece_luz(n)) {
        Some(Proveedor::BleElk)
    } else if triones_por_servicio && parece_luz(n) {
        Some(Proveedor::BleTriones)
    } else {
        None
    };
    if let Some(proveedor) = familia {
        let alto = n.to_ascii_uppercase();
        let forma = if alto.contains("BULB") {
            FormaLuz::Foco
        } else if alto.contains("LAMP") {
            FormaLuz::Lampara
        } else {
            FormaLuz::Tira
        };
        let marca = match proveedor {
            Proveedor::BleTriones => "Triones",
            _ if alto.starts_with("MELK") => "MELK",
            _ => "ELK-BLEDOM",
        };
        // Escriben, pero no dicen cómo están: Nexo recuerda lo último que mandó.
        let capacidades = Capacidades { informa_estado: false, ..Capacidades::luz(true, true, true) };
        return Some(hallazgo(a, nombre, proveedor, forma, marca, capacidades, Manejo::Listo));
    }

    let solo_ver = |marca: &str, forma: FormaLuz, motivo: &str| {
        Some(hallazgo(a, nombre, Proveedor::Ninguno, forma, marca, Capacidades::ninguna(), Manejo::Pronto { motivo: motivo.into() }))
    };
    if tiene_servicio(a, TUYA_SERVICIO) || a.fabricante.iter().any(|(id, _)| *id == TUYA_FABRICANTE) {
        return solo_ver(
            "Tuya",
            FormaLuz::Foco,
            "Usa el cifrado de Tuya: necesita una puerta de enlace (gateway) Tuya para manejarse desde la nube.",
        );
    }
    if es_luz_govee(n) {
        return solo_ver(
            "Govee",
            FormaLuz::Tira,
            "Govee por Bluetooth usa un protocolo cerrado. Si tiene Wi-Fi, activa «LAN Control» en la app de Govee y búscala de nuevo.",
        );
    }
    // Xiaomi anuncia así muchos aparatos (termómetros, cepillos): solo los que se llaman como una luz.
    if tiene_servicio(a, XIAOMI_SERVICIO) && parece_luz(n) {
        let forma = if normalizar(n).contains("bar") || normalizar(n).contains("monitor") { FormaLuz::Barra } else { FormaLuz::Lampara };
        return solo_ver("Xiaomi", forma, "Usa el cifrado propio de Xiaomi; Nexo no puede manejarla por Bluetooth.");
    }
    if parece_luz(n) {
        return solo_ver("", FormaLuz::Foco, "Nexo todavía no conoce cómo se maneja esta luz por Bluetooth.");
    }
    None
}

fn hallazgo(
    a: &Anuncio,
    nombre: Option<&str>,
    proveedor: Proveedor,
    forma: FormaLuz,
    marca: &str,
    capacidades: Capacidades,
    manejo: Manejo,
) -> Hallazgo {
    Hallazgo {
        id: format!("ble:{}", a.direccion.to_lowercase()),
        via: Via::Bluetooth,
        proveedor,
        clase: Clase::Luz,
        forma: Some(forma),
        marca: marca.into(),
        nombre: nombre.map(str::to_string).unwrap_or_else(|| "Luz Bluetooth".into()),
        modelo: nombre.map(str::to_string),
        direccion: Some(a.direccion.clone()),
        mac: None,
        senal: a.senal,
        version: None,
        capacidades,
        estado: None,
        manejo,
    }
}

// ---------------------------------------------------------------- Tramas

/// Cómo se escribe a una luz Bluetooth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocolo {
    Elk,
    Triones,
}

impl Protocolo {
    pub fn de(p: Proveedor) -> Option<Protocolo> {
        match p {
            Proveedor::BleElk => Some(Protocolo::Elk),
            Proveedor::BleTriones => Some(Protocolo::Triones),
            _ => None,
        }
    }
}

/// Servicio y característica de escritura de cada protocolo, en el orden en que se buscan.
/// LEDBLE (de Lotus Lantern) escribe en FFE1; Magic Home, en FFE9.
pub const ESCRITURA: &[(Protocolo, &str, &str)] = &[
    (Protocolo::Elk, "FFF0", "FFF3"),
    (Protocolo::Elk, "FFE0", "FFE1"),
    (Protocolo::Triones, "FFD5", "FFD9"),
    (Protocolo::Triones, "FFE5", "FFE9"),
];

/// Las variantes de ELK que cambian sus tramas (según `elkbledom/models.json`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarianteElk {
    /// ELK-BLEDOM, ELK-BLEDDM, duoCo, Lotus Lantern…: el byte 1 en 0.
    Generica,
    /// ELK-BLEDOB.
    Bledob,
    /// ELK-BULB y LEDBLE: encender con `04 01`.
    Bulb,
    /// MELK y MODELX: además, un saludo antes de las órdenes.
    Melk,
}

pub fn variante_elk(nombre: Option<&str>) -> VarianteElk {
    let n = nombre.unwrap_or("").trim().to_ascii_uppercase();
    if n.starts_with("ELK-BLEDOB") {
        VarianteElk::Bledob
    } else if n.starts_with("MELK") || n.starts_with("MODELX") {
        VarianteElk::Melk
    } else if n.starts_with("ELK-BULB") || n.starts_with("LEDBLE") {
        VarianteElk::Bulb
    } else {
        VarianteElk::Generica
    }
}

/// Lo último que se mandó a una luz: estas tiras no dicen cómo están, y Triones no tiene brillo propio
/// (se aplica al color o al blanco que ya tenía).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Recuerdo {
    pub color: Option<Hsv>,
    /// Blanco: 0 cálido – 100 frío.
    pub temperatura: Option<u8>,
    pub brillo: Option<u8>,
}

impl Recuerdo {
    pub fn aplicar(&mut self, c: &Cambio) {
        if let Some(color) = c.color {
            self.color = Some(color);
            self.temperatura = None;
            self.brillo = Some(color.v.max(1));
        } else if let Some(t) = c.temperatura {
            self.temperatura = Some(t.min(100));
            self.color = None;
        }
        if c.color.is_none()
            && let Some(b) = c.brillo
        {
            self.brillo = Some(b.clamp(1, 100));
        }
    }
}

fn elk(bytes: [u8; 9]) -> Vec<u8> {
    bytes.to_vec()
}

fn elk_encender(v: VarianteElk) -> Vec<u8> {
    match v {
        VarianteElk::Generica => elk([0x7e, 0x00, 0x04, 0xf0, 0x00, 0x01, 0xff, 0x00, 0xef]),
        VarianteElk::Bledob => elk([0x7e, 0x07, 0x04, 0xff, 0x00, 0x01, 0x02, 0x01, 0xef]),
        VarianteElk::Bulb | VarianteElk::Melk => elk([0x7e, 0x00, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0xef]),
    }
}

fn elk_apagar(v: VarianteElk) -> Vec<u8> {
    match v {
        VarianteElk::Bledob => elk([0x7e, 0x07, 0x04, 0x00, 0x00, 0x00, 0x02, 0x01, 0xef]),
        _ => elk([0x7e, 0x00, 0x04, 0x00, 0x00, 0x00, 0xff, 0x00, 0xef]),
    }
}

/// Color a pleno: el brillo va en su propia trama (como en `elkbledom`).
fn elk_color(v: VarianteElk, r: u8, g: u8, b: u8) -> Vec<u8> {
    match v {
        VarianteElk::Bledob => elk([0x7e, 0x07, 0x05, 0x03, r, g, b, 0x0a, 0xef]),
        _ => elk([0x7e, 0x00, 0x05, 0x03, r, g, b, 0x00, 0xef]),
    }
}

/// Brillo 0–100 (el byte va de 0 a 100, no a 255).
fn elk_brillo(v: VarianteElk, brillo: u8) -> Vec<u8> {
    let i = brillo.min(100);
    match v {
        VarianteElk::Generica => elk([0x7e, 0x00, 0x01, i, 0xff, 0x00, 0xff, 0x00, 0xef]),
        VarianteElk::Bledob => elk([0x7e, 0x04, 0x01, i, 0x01, 0xff, 0x02, 0x01, 0xef]),
        VarianteElk::Bulb | VarianteElk::Melk => elk([0x7e, 0x04, 0x01, i, 0xff, 0x00, 0xff, 0x00, 0xef]),
    }
}

/// Blanco cálido a frío: tibio y frío suman 100 (FergusInLondon); 0 de Nexo es todo tibio.
fn elk_temperatura(v: VarianteElk, temperatura: u8) -> Vec<u8> {
    let frio = temperatura.min(100);
    let tibio = 100 - frio;
    match v {
        VarianteElk::Bledob => elk([0x7e, 0x07, 0x05, 0x02, tibio, frio, 0x00, 0x00, 0xef]),
        VarianteElk::Melk => elk([0x7e, 0x06, 0x05, 0x02, tibio, frio, 0xff, 0x08, 0xef]),
        _ => elk([0x7e, 0x00, 0x05, 0x02, tibio, frio, 0x00, 0x00, 0xef]),
    }
}

/// MELK y MODELX piden este saludo después de conectar (`elkbledom`).
fn elk_saludo() -> Vec<Vec<u8>> {
    vec![vec![0x7e, 0x07, 0x83], vec![0x7e, 0x04, 0x04]]
}

const TRIONES_ENCENDER: [u8; 3] = [0xcc, 0x23, 0x33];
const TRIONES_APAGAR: [u8; 3] = [0xcc, 0x24, 0x33];

fn triones_color(r: u8, g: u8, b: u8) -> Vec<u8> {
    vec![0x56, r, g, b, 0x00, 0xf0, 0xaa]
}

/// El canal blanco de Triones (en los focos RGBW), 0–255.
fn triones_blanco(intensidad: u8) -> Vec<u8> {
    vec![0x56, 0x00, 0x00, 0x00, intensidad, 0x0f, 0xaa]
}

/// Triones no tiene blanco cálido o frío: se aproxima con RGB entre un tibio (≈2700 K) y un frío (≈6500 K).
fn blanco_rgb(temperatura: u8, brillo: u8) -> (u8, u8, u8) {
    const TIBIO: (f64, f64, f64) = (255.0, 160.0, 70.0);
    const FRIO: (f64, f64, f64) = (200.0, 220.0, 255.0);
    let t = temperatura.min(100) as f64 / 100.0;
    let b = brillo.clamp(1, 100) as f64 / 100.0;
    let mezcla = |a: f64, z: f64| ((a + (z - a) * t) * b).round().clamp(0.0, 255.0) as u8;
    (mezcla(TIBIO.0, FRIO.0), mezcla(TIBIO.1, FRIO.1), mezcla(TIBIO.2, FRIO.2))
}

/// Las tramas de un cambio, en orden. Apagar va sola; encender va antes que lo demás.
pub fn armar(p: Protocolo, nombre: Option<&str>, c: &Cambio, r: &Recuerdo) -> Vec<Vec<u8>> {
    match p {
        Protocolo::Elk => armar_elk(variante_elk(nombre), c),
        Protocolo::Triones => armar_triones(c, r),
    }
}

fn armar_elk(v: VarianteElk, c: &Cambio) -> Vec<Vec<u8>> {
    let mut t = if v == VarianteElk::Melk { elk_saludo() } else { Vec::new() };
    if c.encendido == Some(false) {
        t.push(elk_apagar(v));
        return t;
    }
    if c.encendido == Some(true) {
        t.push(elk_encender(v));
    }
    if let Some(color) = c.color {
        let (r, g, b) = rgb_de(Hsv { v: 100, ..color });
        t.push(elk_color(v, r, g, b));
        t.push(elk_brillo(v, color.v.max(1)));
    } else {
        if let Some(temp) = c.temperatura {
            t.push(elk_temperatura(v, temp));
        }
        if let Some(b) = c.brillo {
            t.push(elk_brillo(v, b.clamp(1, 100)));
        }
    }
    t
}

fn armar_triones(c: &Cambio, r: &Recuerdo) -> Vec<Vec<u8>> {
    if c.encendido == Some(false) {
        return vec![TRIONES_APAGAR.to_vec()];
    }
    let mut t = Vec::new();
    if c.encendido == Some(true) {
        t.push(TRIONES_ENCENDER.to_vec());
    }
    // Sin brillo propio: el color o el blanco se mandan ya con el brillo puesto.
    let brillo = c.brillo.or(r.brillo).unwrap_or(100).clamp(1, 100);
    if let Some(color) = c.color {
        let (rr, g, b) = rgb_de(color);
        t.push(triones_color(rr, g, b));
    } else if let Some(temp) = c.temperatura {
        let (rr, g, b) = blanco_rgb(temp, brillo);
        t.push(triones_color(rr, g, b));
    } else if c.brillo.is_some() {
        match (r.color, r.temperatura) {
            (Some(color), _) => {
                let (rr, g, b) = rgb_de(Hsv { v: brillo, ..color });
                t.push(triones_color(rr, g, b));
            }
            (None, Some(temp)) => {
                let (rr, g, b) = blanco_rgb(temp, brillo);
                t.push(triones_color(rr, g, b));
            }
            // Sin nada recordado: el canal blanco.
            (None, None) => t.push(triones_blanco(((brillo as u32 * 255 + 50) / 100) as u8)),
        }
    }
    t
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cambio(encendido: Option<bool>, brillo: Option<u8>, temperatura: Option<u8>, color: Option<Hsv>) -> Cambio {
        Cambio { encendido, brillo, temperatura, color }
    }

    fn anuncio(nombre: &str, servicios: &[&str]) -> Anuncio {
        Anuncio {
            direccion: "BE58303C1A2F".into(),
            nombre: (!nombre.is_empty()).then(|| nombre.to_string()),
            senal: Some(-61),
            servicios: servicios.iter().map(|s| s.to_string()).collect(),
            ..Anuncio::default()
        }
    }

    #[test]
    fn ble_uuid_corto_y_largo() {
        assert_eq!(uuid_corto("0000fff0-0000-1000-8000-00805f9b34fb"), "FFF0");
        assert_eq!(uuid_corto("fff3"), "FFF3");
        assert_eq!(uuid_corto("0000FD50"), "FD50");
        assert_eq!(uuid_corto("6E400001-B5A3-F393-E0A9-E50E24DCCA9E"), "6E400001-B5A3-F393-E0A9-E50E24DCCA9E");
        assert_eq!(uuid_largo("ffd9"), "0000FFD9-0000-1000-8000-00805F9B34FB");
    }

    #[test]
    fn ble_elk_encender_y_apagar() {
        // elkbledom/models.json, "ELK-BLEDOM" sin handle.
        let on = armar(Protocolo::Elk, Some("ELK-BLEDOM"), &cambio(Some(true), None, None, None), &Recuerdo::default());
        assert_eq!(on, vec![vec![0x7e, 0x00, 0x04, 0xf0, 0x00, 0x01, 0xff, 0x00, 0xef]]);
        let off = armar(Protocolo::Elk, Some("ELK-BLEDOM"), &cambio(Some(false), Some(50), None, None), &Recuerdo::default());
        assert_eq!(off, vec![vec![0x7e, 0x00, 0x04, 0x00, 0x00, 0x00, 0xff, 0x00, 0xef]]);
    }

    #[test]
    fn ble_elk_color_y_brillo() {
        // Color: 7e 00 05 03 r g b 00 ef (FergusInLondon y elkbledom); brillo 0–100: 7e 00 01 i ff 00 ff 00 ef.
        let rojo = cambio(Some(true), None, None, Some(Hsv { h: 0, s: 100, v: 40 }));
        let t = armar(Protocolo::Elk, None, &rojo, &Recuerdo::default());
        assert_eq!(
            t,
            vec![
                vec![0x7e, 0x00, 0x04, 0xf0, 0x00, 0x01, 0xff, 0x00, 0xef],
                vec![0x7e, 0x00, 0x05, 0x03, 0xff, 0x00, 0x00, 0x00, 0xef],
                vec![0x7e, 0x00, 0x01, 40, 0xff, 0x00, 0xff, 0x00, 0xef],
            ]
        );
        let brillo = armar(Protocolo::Elk, Some("duoCo Strip"), &cambio(None, Some(100), None, None), &Recuerdo::default());
        assert_eq!(brillo, vec![vec![0x7e, 0x00, 0x01, 0x64, 0xff, 0x00, 0xff, 0x00, 0xef]]);
    }

    #[test]
    fn ble_elk_temperatura() {
        // 7e 00 05 02 tibio frío 00 00 ef; los dos suman 100. Nexo 0 = cálido.
        let calido = armar(Protocolo::Elk, None, &cambio(None, None, Some(0), None), &Recuerdo::default());
        assert_eq!(calido, vec![vec![0x7e, 0x00, 0x05, 0x02, 100, 0, 0x00, 0x00, 0xef]]);
        let frio = armar(Protocolo::Elk, None, &cambio(None, Some(30), Some(80), None), &Recuerdo::default());
        assert_eq!(frio[0], vec![0x7e, 0x00, 0x05, 0x02, 20, 80, 0x00, 0x00, 0xef]);
        assert_eq!(frio[1], vec![0x7e, 0x00, 0x01, 30, 0xff, 0x00, 0xff, 0x00, 0xef]);
    }

    #[test]
    fn ble_elk_variantes() {
        assert_eq!(variante_elk(Some("ELK-BLEDOB")), VarianteElk::Bledob);
        assert_eq!(variante_elk(Some("MELK-OA10")), VarianteElk::Melk);
        assert_eq!(variante_elk(Some("LEDBLE-1A2B")), VarianteElk::Bulb);
        assert_eq!(variante_elk(None), VarianteElk::Generica);
        // ELK-BLEDOB: encender 7e 07 04 ff 00 01 02 01 ef.
        let on = armar(Protocolo::Elk, Some("ELK-BLEDOB"), &cambio(Some(true), None, None, None), &Recuerdo::default());
        assert_eq!(on, vec![vec![0x7e, 0x07, 0x04, 0xff, 0x00, 0x01, 0x02, 0x01, 0xef]]);
        // ELK-BULB: encender 7e 00 04 01 00 00 00 00 ef.
        let on = armar(Protocolo::Elk, Some("ELK-BULB"), &cambio(Some(true), None, None, None), &Recuerdo::default());
        assert_eq!(on, vec![vec![0x7e, 0x00, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0xef]]);
        // MELK: primero el saludo 7e 07 83 y 7e 04 04.
        let on = armar(Protocolo::Elk, Some("MELK-OC10"), &cambio(Some(true), None, None, None), &Recuerdo::default());
        assert_eq!(on[0], vec![0x7e, 0x07, 0x83]);
        assert_eq!(on[1], vec![0x7e, 0x04, 0x04]);
        assert_eq!(on.len(), 3);
    }

    #[test]
    fn ble_triones_tramas() {
        // madhead/saberlight: encender cc 23 33, apagar cc 24 33, rojo 56 ff 00 00 00 f0 aa.
        let r = Recuerdo::default();
        assert_eq!(armar(Protocolo::Triones, None, &cambio(Some(true), None, None, None), &r), vec![vec![0xcc, 0x23, 0x33]]);
        assert_eq!(armar(Protocolo::Triones, None, &cambio(Some(false), None, None, None), &r), vec![vec![0xcc, 0x24, 0x33]]);
        let rojo = armar(Protocolo::Triones, None, &cambio(None, None, None, Some(Hsv { h: 0, s: 100, v: 100 })), &r);
        assert_eq!(rojo, vec![vec![0x56, 0xff, 0x00, 0x00, 0x00, 0xf0, 0xaa]]);
        // Sin recuerdo, el brillo va al canal blanco: 56 00 00 00 ww 0f aa.
        let blanco = armar(Protocolo::Triones, None, &cambio(None, Some(50), None, None), &r);
        assert_eq!(blanco, vec![vec![0x56, 0x00, 0x00, 0x00, 128, 0x0f, 0xaa]]);
    }

    #[test]
    fn ble_triones_brillo_sobre_el_color_recordado() {
        let mut r = Recuerdo::default();
        r.aplicar(&cambio(Some(true), None, None, Some(Hsv { h: 240, s: 100, v: 100 })));
        let t = armar(Protocolo::Triones, None, &cambio(None, Some(50), None, None), &r);
        assert_eq!(t, vec![vec![0x56, 0x00, 0x00, 128, 0x00, 0xf0, 0xaa]]);
        // Blanco aproximado con RGB: cálido a pleno.
        let t = armar(Protocolo::Triones, None, &cambio(None, None, Some(0), None), &Recuerdo::default());
        assert_eq!(t, vec![vec![0x56, 255, 160, 70, 0x00, 0xf0, 0xaa]]);
        r.aplicar(&cambio(None, None, Some(100), None));
        assert_eq!((r.color, r.temperatura), (None, Some(100)));
    }

    #[test]
    fn ble_reconoce_las_que_maneja() {
        let h = clasificar(&anuncio("ELK-BLEDOM   ", &["FFF0"]));
        let h = h.as_ref();
        assert_eq!(h.map(|x| x.proveedor), Some(Proveedor::BleElk));
        assert_eq!(h.map(|x| x.manejo.clone()), Some(Manejo::Listo));
        assert_eq!(h.map(|x| x.id.as_str()), Some("ble:be58303c1a2f"));
        assert_eq!(h.map(|x| x.forma), Some(Some(FormaLuz::Tira)));
        assert_eq!(h.map(|x| x.capacidades.informa_estado), Some(false));
        assert_eq!(clasificar(&anuncio("Triones-A1B2", &[])).map(|x| x.proveedor), Some(Proveedor::BleTriones));
        assert_eq!(clasificar(&anuncio("QHM-0A12", &[])).map(|x| x.proveedor), Some(Proveedor::BleTriones));
        // LEDBLE: decide el servicio anunciado.
        assert_eq!(clasificar(&anuncio("LEDBLE-78AB", &["FFD5"])).map(|x| x.proveedor), Some(Proveedor::BleTriones));
        assert_eq!(clasificar(&anuncio("LEDBLE-78AB", &["FFE0"])).map(|x| x.proveedor), Some(Proveedor::BleElk));
        assert_eq!(clasificar(&anuncio("ELK-BULB", &[])).and_then(|x| x.forma), Some(FormaLuz::Foco));
    }

    #[test]
    fn ble_reconoce_las_que_solo_ve() {
        let pronto = |h: Option<Hallazgo>| h.map(|x| (x.proveedor, matches!(x.manejo, Manejo::Pronto { .. }), x.marca));
        let tuya = Anuncio { fabricante: vec![(0x07D0, vec![0, 3])], ..anuncio("", &[]) };
        assert_eq!(pronto(clasificar(&tuya)), Some((Proveedor::Ninguno, true, "Tuya".into())));
        assert_eq!(pronto(clasificar(&anuncio("TY", &["0000fd50-0000-1000-8000-00805f9b34fb"]))).map(|x| x.2), Some("Tuya".into()));
        assert_eq!(pronto(clasificar(&anuncio("ihoment_H6199_ABCD", &[]))).map(|x| x.2), Some("Govee".into()));
        let barra = Anuncio { datos_servicio: vec![("FE95".into(), vec![0x30, 0x58])], ..anuncio("Mi Monitor Light Bar", &[]) };
        let h = clasificar(&barra);
        assert_eq!(h.as_ref().map(|x| x.marca.as_str()), Some("Xiaomi"));
        assert_eq!(h.and_then(|x| x.forma), Some(FormaLuz::Barra));
        assert!(matches!(clasificar(&anuncio("Smart Bulb", &[])).map(|x| x.manejo), Some(Manejo::Pronto { .. })));
    }

    #[test]
    fn ble_deja_afuera_lo_que_no_es_luz() {
        // Termómetros Govee, Xiaomi que no es luz, una billetera Ledger, audífonos y teléfonos.
        assert!(clasificar(&anuncio("GVH5075_1234", &[])).is_none());
        assert!(clasificar(&anuncio("Govee_H5179_AB12", &[])).is_none());
        let termometro = Anuncio { datos_servicio: vec![("FE95".into(), vec![0x50])], ..anuncio("LYWSD03MMC", &[]) };
        assert!(clasificar(&termometro).is_none());
        assert!(clasificar(&anuncio("Ledger Nano X", &[])).is_none());
        assert!(clasificar(&anuncio("WH-CH520", &["FE2C"])).is_none());
        assert!(clasificar(&anuncio("", &[])).is_none());
    }

    #[test]
    fn ble_junta_anuncios_del_mismo_aparato() {
        let mut a = Anuncio { senal: Some(-80), ..anuncio("", &["FFF0"]) };
        a.juntar(Anuncio { senal: Some(-60), ..anuncio("ELK-BLEDOM", &["FFF0", "1812"]) });
        assert_eq!(a.nombre.as_deref(), Some("ELK-BLEDOM"));
        assert_eq!(a.senal, Some(-60));
        assert_eq!(a.servicios, vec!["FFF0".to_string(), "1812".to_string()]);
    }
}
