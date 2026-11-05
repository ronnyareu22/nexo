//! El modelo común de la casa: qué es cada aparato, qué sabe hacer, cómo está y qué se le puede pedir.
//!
//! - Todo proveedor (la nube de Tuya, WiZ, Yeelight, Govee, las tiras Bluetooth…) traduce lo suyo a estos tipos,
//!   así la interfaz, las escenas, el menú del ícono y la voz hablan un solo idioma.
//! - Valores de Nexo: brillo 1–100, temperatura 0 (cálida) – 100 (fría), color con tono 0–360 y saturación y
//!   brillo 0–100. Cada proveedor los pasa a sus rangos.
//! - Clases de hoy y de mañana: luces y enchufes se manejan; tele, cerradura, sensor de puerta, cortina y clima ya
//!   tienen su lugar (capacidades, estado y acciones) para sumarlos sin rehacer nada.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------- Qué es

/// La clase de aparato.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub enum Clase {
    #[default]
    Luz,
    Enchufe,
    Tv,
    Cerradura,
    SensorPuerta,
    Cortina,
    Clima,
    Otro,
}

impl Clase {
    /// En palabras, para los mensajes ("la luz", "el enchufe").
    pub fn nombre(self) -> &'static str {
        match self {
            Clase::Luz => "luz",
            Clase::Enchufe => "enchufe",
            Clase::Tv => "tele",
            Clase::Cerradura => "cerradura",
            Clase::SensorPuerta => "sensor de puerta",
            Clase::Cortina => "cortina",
            Clase::Clima => "clima",
            Clase::Otro => "aparato",
        }
    }
}

/// Cómo es una luz por fuera (para dibujarla y nombrarla). Solo en la clase `Luz`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum FormaLuz {
    /// Tira LED.
    Tira,
    /// Foco o bombilla.
    Foco,
    /// Lámpara de monitor (la barra que va encima de la pantalla).
    Barra,
    /// Lámpara de mesa o de pie.
    Lampara,
}

/// Quién maneja (o manejaría) el aparato. Cada proveedor dice qué clases entiende.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Proveedor {
    /// La nube de Tuya / Smart Life (Access ID y Secret en Ajustes).
    Tuya,
    /// WiZ por la red local (UDP 38899, sin clave).
    Wiz,
    /// Yeelight por la red local (necesita «Control LAN» en su app).
    Yeelight,
    /// Govee por la red local (necesita «LAN Control» en su app).
    Govee,
    /// Puente Philips Hue por la red local (HTTPS con su certificado fijado; se vincula con su botón).
    Hue,
    /// LIFX por la red local (UDP 56700, sin clave).
    Lifx,
    /// Shelly por la red local (HTTP; Gen1 con Basic, Gen2 en adelante con Digest si tiene contraseña).
    Shelly,
    /// Nanoleaf por la red local (HTTP 16021; se vincula manteniendo su botón de encendido).
    Nanoleaf,
    /// Elgato Key Light y Light Strip por la red local (HTTP 9123, sin clave).
    Elgato,
    /// Tiras WLED por la red local (API JSON, sin clave).
    Wled,
    /// Tasmota y OpenBeken (aparatos Tuya con firmware abierto): `cm?cmnd=` por HTTP.
    Tasmota,
    /// Tiras Bluetooth tipo ELK-BLEDOM / duoCo / Lotus Lantern (servicio FFF0).
    BleElk,
    /// Tiras Bluetooth tipo Triones / Happy Lighting / Magic Home (servicio FFD5).
    BleTriones,
    /// Home Assistant de la casa (gratis, en tu red): luces, enchufes, teles, cerraduras, puertas, cortinas y clima.
    HomeAssistant,
    /// Lo que se ve pero nadie maneja todavía (teles, Matter, HomeKit, Bluetooth con cifrado propio…).
    Ninguno,
    /// Datos de ejemplo (solo para revisar el diseño).
    Ejemplo,
}

impl Proveedor {
    /// Las clases que Nexo sabe manejar con este proveedor hoy.
    pub fn clases(self) -> &'static [Clase] {
        match self {
            Proveedor::Tuya | Proveedor::Ejemplo => &[Clase::Luz, Clase::Enchufe],
            Proveedor::HomeAssistant => &[
                Clase::Luz,
                Clase::Enchufe,
                Clase::Tv,
                Clase::Cerradura,
                Clase::SensorPuerta,
                Clase::Cortina,
                Clase::Clima,
            ],
            Proveedor::Wiz | Proveedor::Hue | Proveedor::Shelly | Proveedor::Tasmota => &[Clase::Luz, Clase::Enchufe],
            Proveedor::Yeelight
            | Proveedor::Govee
            | Proveedor::Lifx
            | Proveedor::Nanoleaf
            | Proveedor::Elgato
            | Proveedor::Wled
            | Proveedor::BleElk
            | Proveedor::BleTriones => &[Clase::Luz],
            Proveedor::Ninguno => &[],
        }
    }

    /// Lo maneja por la red local o por Bluetooth (sin nube).
    pub fn es_local(self) -> bool {
        matches!(
            self,
            Proveedor::Wiz
                | Proveedor::Yeelight
                | Proveedor::Govee
                | Proveedor::Hue
                | Proveedor::Lifx
                | Proveedor::Shelly
                | Proveedor::Nanoleaf
                | Proveedor::Elgato
                | Proveedor::Wled
                | Proveedor::Tasmota
                | Proveedor::BleElk
                | Proveedor::BleTriones
        )
    }

    pub fn por_bluetooth(self) -> bool {
        matches!(self, Proveedor::BleElk | Proveedor::BleTriones)
    }
}

/// Por dónde se encontró.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Via {
    Nube,
    Wifi,
    Bluetooth,
}

// ---------------------------------------------------------------- Qué sabe hacer

/// Lo que un aparato deja hacer. Lo que no está, la interfaz no lo muestra.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Capacidades {
    /// Encender y apagar (luces, enchufes, teles).
    pub encender: bool,
    pub brillo: bool,
    /// Blanco cálido a frío.
    pub temperatura: bool,
    pub color: bool,
    /// Tele: volumen, silencio y entrada (HDMI, apps).
    pub volumen: bool,
    pub silencio: bool,
    pub entradas: bool,
    /// Para después: cerradura o cortina (abrir y cerrar).
    pub abrir: bool,
    /// Para después: cortina (0–100 %).
    pub posicion: bool,
    /// Solo informa (un sensor de puerta): no recibe órdenes.
    pub solo_lectura: bool,
    /// Nunca con un toque ni por voz sin confirmar (abrir una cerradura).
    pub requiere_confirmacion: bool,
    /// Sabe decir cómo está (si no, Nexo recuerda lo último que le mandó).
    pub informa_estado: bool,
}

impl Capacidades {
    pub const fn luz(brillo: bool, temperatura: bool, color: bool) -> Self {
        Capacidades {
            encender: true,
            brillo,
            temperatura,
            color,
            volumen: false,
            silencio: false,
            entradas: false,
            abrir: false,
            posicion: false,
            solo_lectura: false,
            requiere_confirmacion: false,
            informa_estado: true,
        }
    }

    pub const fn enchufe() -> Self {
        Capacidades::luz(false, false, false)
    }

    /// Algo que solo se ve (todavía sin control).
    pub const fn ninguna() -> Self {
        Capacidades {
            encender: false,
            brillo: false,
            temperatura: false,
            color: false,
            volumen: false,
            silencio: false,
            entradas: false,
            abrir: false,
            posicion: false,
            solo_lectura: false,
            requiere_confirmacion: false,
            informa_estado: false,
        }
    }
}

// ---------------------------------------------------------------- Cómo está

/// Un color: tono 0–360, saturación 0–100 y brillo 0–100.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hsv {
    pub h: u16,
    pub s: u8,
    pub v: u8,
}

impl Hsv {
    pub fn nuevo(h: i64, s: i64, v: i64) -> Self {
        Hsv { h: h.rem_euclid(360) as u16, s: s.clamp(0, 100) as u8, v: v.clamp(0, 100) as u8 }
    }
}

/// En qué está una luz que tiene color.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ModoLuz {
    #[default]
    Blanco,
    Color,
    /// Una escena o música de su app: Nexo no la cambia hasta que se elija blanco o color.
    Escena,
}

/// Cómo está un aparato, en valores de Nexo. Cada clase usa lo suyo; lo que no se sabe va en `None`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct EstadoAparato {
    pub encendido: Option<bool>,
    /// 1–100.
    pub brillo: Option<u8>,
    /// 0 cálida – 100 fría.
    pub temperatura: Option<u8>,
    pub color: Option<Hsv>,
    pub modo: Option<ModoLuz>,
    /// Puerta, cortina o cerradura: abierta (true) o cerrada.
    pub abierto: Option<bool>,
    /// Cortina: 0–100 %.
    pub posicion: Option<u8>,
    /// Tele: 0–100.
    pub volumen: Option<u8>,
    pub silenciado: Option<bool>,
    pub bateria: Option<u8>,
    /// Lo que marca un termómetro o el clima, en °C.
    pub grados: Option<f32>,
}

// ---------------------------------------------------------------- Qué se le pide

/// Una acción, ya tipada. La voz, las escenas, el menú del ícono y el panel mandan esto.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum Accion {
    Encender,
    Apagar,
    Alternar,
    /// 1–100 (0 cuenta como 1: para apagar está `Apagar`).
    Brillo { valor: u8 },
    /// Sube el brillo (de fábrica, 20 puntos).
    Subir {
        #[serde(default)]
        paso: Option<u8>,
    },
    Bajar {
        #[serde(default)]
        paso: Option<u8>,
    },
    /// 0 cálida – 100 fría.
    Temperatura { valor: u8 },
    /// Tono 0–360, saturación y brillo 0–100.
    Color { h: u16, s: u8, v: u8 },
    /// Para después: cerraduras y cortinas. Abrir una cerradura pide `confirmado`.
    Abrir,
    Cerrar,
    /// Para después: cortina al 0–100 %.
    Posicion { valor: u8 },
    /// Tele: volumen 0–100, silencio y entrada (por su nombre, como la da la tele).
    Volumen { valor: u8 },
    Silenciar { activo: bool },
    Entrada { nombre: String },
    /// Solo pregunta cómo está ("¿está cerrada la puerta?"). No cambia nada.
    Consultar,
}

impl Accion {
    /// No cambia nada (sirve con aparatos de solo lectura).
    pub fn es_consulta(&self) -> bool {
        matches!(self, Accion::Consultar)
    }
}

/// A quién va una orden.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum Destino {
    /// Todo lo que entienda la acción; con `clase`, solo esa ("todas las luces").
    Todos {
        #[serde(default)]
        clase: Option<Clase>,
    },
    Aparato { id: String },
    Grupo { id: String },
    /// Lo de una habitación; con `clase`, solo eso ("las luces de la sala", sin la tele).
    Habitacion {
        id: String,
        #[serde(default)]
        clase: Option<Clase>,
    },
    /// Como lo diría alguien: "la luz del escritorio", "la tira". Se resuelve con `resolver_nombre`.
    Nombre {
        texto: String,
        #[serde(default)]
        clase: Option<Clase>,
    },
}

/// Una orden completa: a quién, qué y, para lo delicado (abrir una cerradura), si ya se confirmó.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Orden {
    pub destino: Destino,
    pub accion: Accion,
    #[serde(default)]
    pub confirmado: bool,
}

/// Lo que hay que cambiar en un aparato, en valores de Nexo. Cada proveedor lo traduce a lo suyo.
/// Con `color`, la luz pasa a color (su `v` es el brillo); con `temperatura`, a blanco.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Cambio {
    pub encendido: Option<bool>,
    /// 1–100.
    pub brillo: Option<u8>,
    /// 0 cálida – 100 fría.
    pub temperatura: Option<u8>,
    pub color: Option<Hsv>,
}

impl Cambio {
    pub fn vacio(&self) -> bool {
        *self == Cambio::default()
    }

    /// Cómo queda el estado después del cambio (para mostrarlo enseguida, sin esperar a leerlo).
    pub fn aplicar(&self, e: &mut EstadoAparato) {
        if let Some(on) = self.encendido {
            e.encendido = Some(on);
        }
        if let Some(c) = self.color {
            e.color = Some(c);
            e.modo = Some(ModoLuz::Color);
            e.brillo = Some(c.v.max(1));
        } else if let Some(t) = self.temperatura {
            e.temperatura = Some(t.min(100));
            e.modo = Some(ModoLuz::Blanco);
        }
        if self.color.is_none()
            && let Some(b) = self.brillo
        {
            e.brillo = Some(b.clamp(1, 100));
            if e.modo == Some(ModoLuz::Color)
                && let Some(c) = e.color.as_mut()
            {
                c.v = b.clamp(1, 100);
            }
        }
    }
}

/// Lo que se quiere de una luz en una escena: encendida o apagada y, si se elige, brillo y blanco o color.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Objetivo {
    pub encendido: bool,
    #[serde(default)]
    pub brillo: Option<u8>,
    #[serde(default)]
    pub temperatura: Option<u8>,
    #[serde(default)]
    pub color: Option<Hsv>,
}

impl Objetivo {
    /// El cambio para un aparato con estas capacidades: lo que no sabe hacer se deja afuera, sin error.
    pub fn cambio(&self, c: &Capacidades) -> Cambio {
        if !self.encendido {
            return Cambio { encendido: Some(false), ..Cambio::default() };
        }
        let color = self.color.filter(|_| c.color);
        Cambio {
            encendido: Some(true),
            brillo: self.brillo.filter(|_| c.brillo && color.is_none()).map(|b| b.clamp(1, 100)),
            temperatura: self.temperatura.filter(|_| c.temperatura && color.is_none()).map(|t| t.min(100)),
            color: color.map(|h| match self.brillo {
                Some(b) => Hsv { v: b.clamp(1, 100), ..h },
                None => h,
            }),
        }
    }
}

/// Paso de fábrica de "sube" y "baja".
pub const PASO_BRILLO: u8 = 20;

/// El cambio que pide una acción, sabiendo cómo está el aparato y qué deja hacer.
/// Mover el brillo, el blanco o el color también lo enciende (como en la app de Tuya y en Apple Casa).
pub fn cambio_de(accion: &Accion, actual: &EstadoAparato, c: &Capacidades, nombre: &str) -> Result<Cambio, String> {
    let no = |que: &str| format!("{nombre} no {que}.");
    let encender = |cambio: Cambio| Cambio { encendido: Some(true), ..cambio };
    match accion {
        Accion::Encender | Accion::Apagar | Accion::Alternar if !c.encender => Err(no("se enciende ni se apaga desde Nexo")),
        Accion::Encender => Ok(Cambio { encendido: Some(true), ..Cambio::default() }),
        Accion::Apagar => Ok(Cambio { encendido: Some(false), ..Cambio::default() }),
        Accion::Alternar => Ok(Cambio { encendido: Some(!actual.encendido.unwrap_or(false)), ..Cambio::default() }),
        Accion::Brillo { valor } => {
            if !c.brillo {
                return Err(no("cambia de brillo"));
            }
            Ok(encender(Cambio { brillo: Some((*valor).clamp(1, 100)), ..Cambio::default() }))
        }
        Accion::Subir { paso } | Accion::Bajar { paso } => {
            if !c.brillo {
                return Err(no("cambia de brillo"));
            }
            let paso = paso.unwrap_or(PASO_BRILLO).clamp(1, 100) as i16;
            let ahora = if actual.encendido == Some(false) { 0 } else { actual.brillo.unwrap_or(50) as i16 };
            let nuevo = if matches!(accion, Accion::Subir { .. }) { ahora + paso } else { ahora - paso };
            Ok(encender(Cambio { brillo: Some(nuevo.clamp(1, 100) as u8), ..Cambio::default() }))
        }
        Accion::Temperatura { valor } => {
            if !c.temperatura {
                return Err(no("cambia de blanco cálido a frío"));
            }
            Ok(encender(Cambio { temperatura: Some((*valor).min(100)), ..Cambio::default() }))
        }
        Accion::Color { h, s, v } => {
            if !c.color {
                return Err(no("cambia de color"));
            }
            Ok(encender(Cambio { color: Some(Hsv::nuevo(*h as i64, *s as i64, (*v).max(1) as i64)), ..Cambio::default() }))
        }
        Accion::Abrir | Accion::Cerrar if !c.abrir => Err(no("se abre ni se cierra desde Nexo")),
        Accion::Posicion { .. } if !c.posicion => Err(no("cambia de posición desde Nexo")),
        Accion::Volumen { .. } if !c.volumen => Err(no("cambia de volumen desde Nexo")),
        Accion::Silenciar { .. } if !c.silencio => Err(no("se silencia desde Nexo")),
        Accion::Entrada { .. } if !c.entradas => Err(no("cambia de entrada desde Nexo")),
        // No son cambios de luz: los hace el proveedor que los maneja (Home Assistant), no este camino.
        Accion::Abrir
        | Accion::Cerrar
        | Accion::Posicion { .. }
        | Accion::Volumen { .. }
        | Accion::Silenciar { .. }
        | Accion::Entrada { .. } => Err(format!("{nombre} no se maneja así.")),
        Accion::Consultar => Ok(Cambio::default()),
    }
}

// ---------------------------------------------------------------- Lo que se encuentra al buscar

/// Si Nexo puede manejar lo encontrado, en palabras para la persona.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum Manejo {
    /// Lista para usar: Nexo la maneja ya.
    Listo,
    /// Se maneja con la cuenta de Tuya / Smart Life (Ajustes › Casa).
    CuentaTuya,
    /// Falta activar algo en su app: "Activa «Control LAN» en la app de Yeelight".
    Activar { paso: String },
    /// Se ve, pero Nexo todavía no la maneja; el motivo en palabras.
    Pronto { motivo: String },
    /// Un Home Assistant en la red: se conecta en Ajustes › Casa con un token (la dirección ya va puesta).
    HomeAssistant { url: String },
    /// Se vincula una vez: con el botón del aparato (Hue, Nanoleaf) o con su contraseña (Shelly, Tasmota).
    /// `paso` es la instrucción en palabras.
    Vincular { forma: FormaVinculo, paso: String },
}

/// Cómo se vincula algo encontrado.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FormaVinculo {
    /// Se presiona (o se mantiene) un botón del aparato y Nexo pide la clave unos segundos.
    Boton,
    /// La persona escribe la contraseña que le puso al aparato.
    Clave,
}

/// Algo encontrado en una búsqueda (por la red, por Bluetooth o en la nube).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hallazgo {
    /// Estable entre búsquedas: "wiz:a8bb50d2c3e4", "tuya:<id>", "ble:<dirección>"…
    pub id: String,
    pub via: Via,
    pub proveedor: Proveedor,
    pub clase: Clase,
    #[serde(default)]
    pub forma: Option<FormaLuz>,
    /// "WiZ", "Govee", "Tuya / Smart Life", "Xiaomi"… o "" si no se sabe.
    pub marca: String,
    pub nombre: String,
    #[serde(default)]
    pub modelo: Option<String>,
    /// La IP ("192.168.1.40") o el Bluetooth (en la Mac, el identificador del sistema; en Windows, la dirección).
    #[serde(default)]
    pub direccion: Option<String>,
    /// La MAC ("a4:2b:b0:12:34:56"), si se sabe: con ella se vuelve a encontrar si cambia de IP.
    #[serde(default)]
    pub mac: Option<String>,
    /// Señal en dBm (Bluetooth y WiZ).
    #[serde(default)]
    pub senal: Option<i16>,
    #[serde(default)]
    pub version: Option<String>,
    pub capacidades: Capacidades,
    #[serde(default)]
    pub estado: Option<EstadoAparato>,
    pub manejo: Manejo,
}

// ---------------------------------------------------------------- Conversiones comunes

/// De 0–100 de Nexo a un rango (redondeado).
pub fn a_rango(pct: u8, min: i64, max: i64) -> i64 {
    let p = pct.min(100) as i64;
    min + ((max - min) * p + 50) / 100
}

/// De un rango a 0–100 de Nexo.
pub fn de_rango(valor: i64, min: i64, max: i64) -> u8 {
    if max <= min {
        return 0;
    }
    let v = valor.clamp(min, max);
    (((v - min) * 100 + (max - min) / 2) / (max - min)) as u8
}

/// El brillo de Nexo (1–100) en un rango: 1 es el mínimo de la luz y 100 su máximo.
pub fn brillo_a_rango(pct: u8, min: i64, max: i64) -> i64 {
    let p = pct.clamp(1, 100) as i64 - 1;
    min + ((max - min) * p + 49) / 99
}

/// De un rango al brillo de Nexo (1–100).
pub fn brillo_de_rango(valor: i64, min: i64, max: i64) -> u8 {
    if max <= min {
        return 100;
    }
    let v = valor.clamp(min, max);
    (1 + ((v - min) * 99 + (max - min) / 2) / (max - min)) as u8
}

/// La temperatura de Nexo (0 cálida – 100 fría) en Kelvin, con el rango de cada luz (WiZ 2200–6500…).
pub fn a_kelvin(temp: u8, min_k: u32, max_k: u32) -> u32 {
    a_rango(temp, min_k as i64, max_k as i64) as u32
}

pub fn de_kelvin(k: u32, min_k: u32, max_k: u32) -> u8 {
    de_rango(k as i64, min_k as i64, max_k as i64)
}

/// Un color de Nexo en RGB (0–255).
pub fn rgb_de(c: Hsv) -> (u8, u8, u8) {
    let h = (c.h % 360) as f64 / 60.0;
    let s = c.s.min(100) as f64 / 100.0;
    let v = c.v.min(100) as f64 / 100.0;
    let croma = v * s;
    let x = croma * (1.0 - ((h % 2.0) - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (croma, x, 0.0),
        1 => (x, croma, 0.0),
        2 => (0.0, croma, x),
        3 => (0.0, x, croma),
        4 => (x, 0.0, croma),
        _ => (croma, 0.0, x),
    };
    let m = v - croma;
    let a = |c: f64| ((c + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (a(r), a(g), a(b))
}

/// Un RGB (0–255) en un color de Nexo.
pub fn hsv_de(r: u8, g: u8, b: u8) -> Hsv {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    Hsv::nuevo(h.round() as i64, (s * 100.0).round() as i64, (max * 100.0).round() as i64)
}

/// Una MAC en la forma de Nexo ("a4:2b:b0:12:34:56"), venga como venga ("A42BB0123456", "A4-2B-…").
pub fn mac_normal(t: &str) -> Option<String> {
    let hex: String = t.chars().filter(|c| c.is_ascii_hexdigit()).collect::<String>().to_ascii_lowercase();
    let sobra = t.chars().any(|c| !c.is_ascii_hexdigit() && !matches!(c, ':' | '-' | '.' | ' '));
    if hex.len() != 12 || sobra || hex == "000000000000" || hex == "ffffffffffff" {
        return None;
    }
    Some(hex.as_bytes().chunks(2).map(|p| std::str::from_utf8(p).unwrap_or("00")).collect::<Vec<_>>().join(":"))
}

/// Sin tildes, en minúsculas y con un solo espacio: para comparar nombres ("Lámpara" = "lampara").
pub fn normalizar(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    for c in texto.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        s.push(c);
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn hogar_rangos_de_tuya() {
        // bright_value_v2 10–1000: 1 % es el mínimo y 100 % el máximo.
        assert_eq!(brillo_a_rango(1, 10, 1000), 10);
        assert_eq!(brillo_a_rango(100, 10, 1000), 1000);
        assert_eq!(brillo_de_rango(10, 10, 1000), 1);
        assert_eq!(brillo_de_rango(1000, 10, 1000), 100);
        assert_eq!(brillo_de_rango(brillo_a_rango(50, 25, 255), 25, 255), 50);
        // temp_value_v2 0–1000.
        assert_eq!(a_rango(0, 0, 1000), 0);
        assert_eq!(a_rango(73, 0, 1000), 730);
        assert_eq!(de_rango(255, 0, 255), 100);
        assert_eq!(a_kelvin(0, 2200, 6500), 2200);
        assert_eq!(a_kelvin(100, 2200, 6500), 6500);
        assert_eq!(de_kelvin(4350, 2200, 6500), 50);
    }

    #[test]
    fn hogar_colores() {
        assert_eq!(rgb_de(Hsv { h: 0, s: 100, v: 100 }), (255, 0, 0));
        assert_eq!(rgb_de(Hsv { h: 120, s: 100, v: 100 }), (0, 255, 0));
        assert_eq!(rgb_de(Hsv { h: 240, s: 100, v: 50 }), (0, 0, 128));
        assert_eq!(rgb_de(Hsv { h: 30, s: 0, v: 100 }), (255, 255, 255));
        assert_eq!(hsv_de(255, 128, 0), Hsv { h: 30, s: 100, v: 100 });
        assert_eq!(hsv_de(0, 0, 0), Hsv { h: 0, s: 0, v: 0 });
        assert_eq!(Hsv::nuevo(-30, 140, -2), Hsv { h: 330, s: 100, v: 0 });
    }

    #[test]
    fn hogar_cambios_de_cada_accion() {
        let c = Capacidades::luz(true, true, true);
        let apagada = EstadoAparato { encendido: Some(false), brillo: Some(60), ..Default::default() };
        let r = cambio_de(&Accion::Alternar, &apagada, &c, "La tira");
        assert_eq!(r, Ok(Cambio { encendido: Some(true), ..Default::default() }));
        // Subir con la luz apagada parte de 0 y la enciende.
        let r = cambio_de(&Accion::Subir { paso: None }, &apagada, &c, "La tira");
        assert_eq!(r, Ok(Cambio { encendido: Some(true), brillo: Some(20), ..Default::default() }));
        let r = cambio_de(&Accion::Brillo { valor: 0 }, &apagada, &c, "La tira");
        assert_eq!(r.map(|x| x.brillo), Ok(Some(1)));
        let r = cambio_de(&Accion::Color { h: 400, s: 80, v: 0 }, &apagada, &c, "La tira");
        assert_eq!(r.map(|x| x.color), Ok(Some(Hsv { h: 40, s: 80, v: 1 })));
        // Un enchufe no cambia de brillo, y lo de después (abrir) todavía no se hace.
        let e = Capacidades::enchufe();
        assert_eq!(cambio_de(&Accion::Brillo { valor: 50 }, &apagada, &e, "El enchufe"), Err("El enchufe no cambia de brillo.".into()));
        assert!(cambio_de(&Accion::Abrir, &apagada, &e, "La puerta").is_err());
        assert_eq!(cambio_de(&Accion::Consultar, &apagada, &Capacidades::ninguna(), "La puerta"), Ok(Cambio::default()));
    }

    #[test]
    fn hogar_objetivo_segun_lo_que_sabe() {
        let o = Objetivo { encendido: true, brillo: Some(40), temperatura: Some(0), color: None };
        // Una barra blanca: brillo y cálida. Un enchufe: solo encender.
        assert_eq!(
            o.cambio(&Capacidades::luz(true, true, false)),
            Cambio { encendido: Some(true), brillo: Some(40), temperatura: Some(0), color: None }
        );
        assert_eq!(o.cambio(&Capacidades::enchufe()), Cambio { encendido: Some(true), ..Default::default() });
        // Con color, el brillo va en el color.
        let o = Objetivo { encendido: true, brillo: Some(30), temperatura: None, color: Some(Hsv { h: 200, s: 90, v: 100 }) };
        assert_eq!(o.cambio(&Capacidades::luz(true, true, true)).color, Some(Hsv { h: 200, s: 90, v: 30 }));
        let apagar = Objetivo { encendido: false, brillo: Some(30), temperatura: None, color: None };
        assert_eq!(apagar.cambio(&Capacidades::luz(true, true, true)), Cambio { encendido: Some(false), ..Default::default() });
    }

    #[test]
    fn hogar_estado_tras_un_cambio() {
        let mut e = EstadoAparato { encendido: Some(true), brillo: Some(80), modo: Some(ModoLuz::Color), color: Some(Hsv { h: 10, s: 100, v: 80 }), ..Default::default() };
        Cambio { brillo: Some(30), ..Default::default() }.aplicar(&mut e);
        assert_eq!(e.color, Some(Hsv { h: 10, s: 100, v: 30 }));
        Cambio { temperatura: Some(20), ..Default::default() }.aplicar(&mut e);
        assert_eq!((e.modo, e.temperatura, e.brillo), (Some(ModoLuz::Blanco), Some(20), Some(30)));
    }

    #[test]
    fn hogar_macs() {
        assert_eq!(mac_normal("A8BB50D2C3E4").as_deref(), Some("a8:bb:50:d2:c3:e4"));
        assert_eq!(mac_normal("3C-6A-9D-26-6E-8C").as_deref(), Some("3c:6a:9d:26:6e:8c"));
        assert_eq!(mac_normal("0:1:2:3:4:5"), None);
        assert_eq!(mac_normal("ff:ff:ff:ff:ff:ff"), None);
        assert_eq!(mac_normal("shelly-a8bb50d2c3e4"), None);
    }

    #[test]
    fn hogar_nombres_sin_tildes() {
        assert_eq!(normalizar("  La LÁMPARA  del Escritorio!"), "la lampara del escritorio");
        assert_eq!(normalizar("Baño · Ñandú"), "bano nandu");
    }

    #[test]
    fn hogar_json_de_ordenes() {
        let o: Orden = serde_json::from_str(
            r#"{"destino":{"tipo":"nombre","texto":"la luz del escritorio"},"accion":{"tipo":"brillo","valor":40}}"#,
        )
        .unwrap_or(Orden { destino: Destino::Todos { clase: None }, accion: Accion::Consultar, confirmado: false });
        assert_eq!(o.accion, Accion::Brillo { valor: 40 });
        assert!(!o.confirmado);
        let todas: Destino = serde_json::from_str(r#"{"tipo":"todos","clase":"luz"}"#).unwrap_or(Destino::Todos { clase: None });
        assert_eq!(todas, Destino::Todos { clase: Some(Clase::Luz) });
        assert_eq!(serde_json::to_string(&Clase::SensorPuerta).unwrap_or_default(), r#""sensorPuerta""#);
        assert_eq!(serde_json::to_string(&Manejo::Listo).unwrap_or_default(), r#"{"tipo":"listo"}"#);
        let v = Manejo::Vincular { forma: FormaVinculo::Boton, paso: "Presiona el botón.".into() };
        assert_eq!(serde_json::to_string(&v).unwrap_or_default(), r#"{"tipo":"vincular","forma":"boton","paso":"Presiona el botón."}"#);
        assert_eq!(serde_json::to_string(&Proveedor::Wled).unwrap_or_default(), r#""wled""#);
    }
}
