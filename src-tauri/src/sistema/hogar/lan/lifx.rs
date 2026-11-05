//! LIFX por la red local (UDP 56700, sin clave), según https://lan.developer.lifx.com.
//!
//! - Buscar: `GetService` (2) a toda la red con `tagged`; cada luz contesta `StateService` (3) con su MAC en el
//!   encabezado. A cada una, directo: `GetVersion` (32 → `StateVersion` 33: el producto) y `Light::Get`
//!   (101 → `Light::State` 107: color, encendido y nombre).
//! - Qué sabe hacer sale del producto (`lifx_productos.rs`, de github.com/LIFX/products): color o solo blanco, su
//!   rango de Kelvin y si es un interruptor (esos solo se ven, por ahora).
//! - Manejar: `Light::SetColor` (102: tono, saturación, brillo y Kelvin juntos, por eso antes se lee cómo está) y
//!   `Light::SetPower` (117). Se pide acuse (`ack_required`) y se espera el `Acknowledgement` (45).
//! - Encabezado de 36 bytes en little endian: tamaño, protocolo 1024 con `addressable` y `tagged`, origen,
//!   destino (8 bytes; los 6 primeros son la MAC), reservado, banderas, secuencia, reservado, tipo, reservado.
//! - Tono 0–65535 = 0–360°, saturación y brillo 0–65535, encendido 0 o 65535.

use super::super::modelo::{
    Cambio, Capacidades, Clase, EstadoAparato, FormaLuz, Hallazgo, Hsv, Manejo, ModoLuz, Proveedor, Via, a_kelvin, brillo_a_rango,
    brillo_de_rango, de_kelvin, mac_normal,
};
use super::lifx_productos::PRODUCTOS;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

pub const PUERTO: u16 = 56700;
const TIPO_GET_SERVICE: u16 = 2;
const TIPO_STATE_SERVICE: u16 = 3;
const TIPO_STATE_LABEL: u16 = 25;
const TIPO_GET_VERSION: u16 = 32;
const TIPO_STATE_VERSION: u16 = 33;
const TIPO_ACUSE: u16 = 45;
const TIPO_LIGHT_GET: u16 = 101;
const TIPO_SET_COLOR: u16 = 102;
const TIPO_LIGHT_STATE: u16 = 107;
const TIPO_SET_POWER: u16 = 117;
/// Quién pregunta (cualquier número distinto de 0 y 1 hace que la luz conteste solo a este equipo).
const ORIGEN: u32 = 0x4E45_584F;
/// El blanco de fábrica cuando la luz no dice el suyo.
const KELVIN_NEUTRO: u16 = 3500;

static SECUENCIA: AtomicU8 = AtomicU8::new(1);

/// Un mensaje: a todas (`destino` en ceros, `tagged`) o a una MAC, con su carga. Pide respuesta o acuse.
pub fn mensaje_con(tipo: u16, destino: Option<[u8; 6]>, carga: &[u8], acuse: bool, secuencia: u8) -> Vec<u8> {
    let tamano = 36 + carga.len();
    let mut m = Vec::with_capacity(tamano);
    m.extend_from_slice(&(tamano as u16).to_le_bytes());
    let tagged: u16 = if destino.is_none() { 1 << 13 } else { 0 };
    m.extend_from_slice(&(1024u16 | (1 << 12) | tagged).to_le_bytes());
    m.extend_from_slice(&ORIGEN.to_le_bytes());
    let mut objetivo = [0u8; 8];
    if let Some(mac) = destino {
        objetivo[..6].copy_from_slice(&mac);
    }
    m.extend_from_slice(&objetivo);
    m.extend_from_slice(&[0; 6]);
    // Banderas: res_required (bit 0) para preguntar; ack_required (bit 1) para mandar.
    m.push(if acuse { 0b10 } else { 0b01 });
    m.push(secuencia);
    m.extend_from_slice(&[0; 8]);
    m.extend_from_slice(&tipo.to_le_bytes());
    m.extend_from_slice(&[0; 2]);
    m.extend_from_slice(carga);
    m
}

/// Un mensaje sin carga que pide respuesta.
pub fn mensaje(tipo: u16, destino: Option<[u8; 6]>) -> Vec<u8> {
    mensaje_con(tipo, destino, &[], false, 0)
}

/// (tipo, MAC, carga) de una respuesta.
pub fn leer(d: &[u8]) -> Option<(u16, [u8; 6], &[u8])> {
    if d.len() < 36 {
        return None;
    }
    let tamano = u16::from_le_bytes([d[0], d[1]]) as usize;
    let protocolo = u16::from_le_bytes([d[2], d[3]]) & 0x0FFF;
    if protocolo != 1024 || tamano < 36 || tamano > d.len() {
        return None;
    }
    let mut mac = [0u8; 6];
    mac.copy_from_slice(&d[8..14]);
    Some((u16::from_le_bytes([d[32], d[33]]), mac, &d[36..tamano]))
}

/// Un texto de 32 bytes en UTF-8, relleno con ceros.
pub fn nombre(carga: &[u8]) -> Option<String> {
    let crudo = carga.get(..32.min(carga.len()))?;
    let fin = crudo.iter().position(|&b| b == 0).unwrap_or(crudo.len());
    let t: String = String::from_utf8_lossy(&crudo[..fin]).chars().filter(|c| !c.is_control()).collect();
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

fn mac_texto(mac: &[u8; 6]) -> String {
    mac.iter().map(|b| format!("{b:02x}")).collect()
}

/// La MAC de un id "lifx:d073d5aabbcc".
pub fn mac_de_id(id: &str) -> Option<[u8; 6]> {
    let h = id.strip_prefix("lifx:")?;
    if h.len() != 12 {
        return None;
    }
    let mut m = [0u8; 6];
    for (i, b) in m.iter_mut().enumerate() {
        *b = u8::from_str_radix(h.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(m)
}

/// HSBK y encendido de un `Light::State`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub tono: u16,
    pub saturacion: u16,
    pub brillo: u16,
    pub kelvin: u16,
    pub encendida: bool,
}

/// `Light::State` (52 bytes): HSBK, reservado, encendido y nombre.
pub fn estado_crudo(carga: &[u8]) -> Option<(Color, Option<String>)> {
    if carga.len() < 44 {
        return None;
    }
    let u = |i: usize| u16::from_le_bytes([carga[i], carga[i + 1]]);
    let c = Color { tono: u(0), saturacion: u(2), brillo: u(4), kelvin: u(6), encendida: u(10) != 0 };
    Some((c, nombre(&carga[12..44])))
}

/// Lo que sabe hacer un producto: (color, rango de Kelvin, es un interruptor). Si no está en la tabla, una luz de
/// color común (2500–9000 K).
pub fn producto(pid: u32) -> (bool, Option<(u32, u32)>, bool) {
    match PRODUCTOS.binary_search_by_key(&pid, |p| p.0 as u32) {
        Ok(i) => {
            let (_, color, kmin, kmax, reles) = PRODUCTOS[i];
            let rango = (kmax > kmin).then_some((kmin as u32, kmax as u32));
            (color, rango, reles)
        }
        Err(_) => (true, Some((2500, 9000)), false),
    }
}

pub fn capacidades(color: bool, rango: Option<(u32, u32)>) -> Capacidades {
    Capacidades::luz(true, rango.is_some(), color)
}

/// Cómo está, en valores de Nexo.
pub fn estado_de(c: &Color, rango: Option<(u32, u32)>) -> EstadoAparato {
    let brillo = brillo_de_rango(c.brillo as i64, 0, 65535).max(1);
    let mut e = EstadoAparato { encendido: Some(c.encendida), brillo: Some(brillo), ..Default::default() };
    if c.saturacion > 0 {
        let h = ((c.tono as u32 * 360 + 32768) / 65536) % 360;
        let s = (c.saturacion as u32 * 100 + 32767) / 65535;
        e.color = Some(Hsv { h: h as u16, s: s as u8, v: brillo });
        e.modo = Some(ModoLuz::Color);
    } else {
        if let Some((min, max)) = rango {
            e.temperatura = Some(de_kelvin(c.kelvin as u32, min, max));
        }
        e.modo = Some(ModoLuz::Blanco);
    }
    e
}

/// El color nuevo tras un cambio (lo que no cambia queda como estaba).
pub fn color_nuevo(actual: &Color, cambio: &Cambio, rango: Option<(u32, u32)>) -> Color {
    let mut c = *actual;
    if c.kelvin == 0 {
        c.kelvin = KELVIN_NEUTRO;
    }
    if let Some(color) = cambio.color {
        c.tono = (((color.h % 360) as u32 * 65536 + 180) / 360 % 65536) as u16;
        c.saturacion = ((color.s.min(100) as u32 * 65535 + 50) / 100) as u16;
        c.brillo = brillo_a_rango(color.v.max(1), 0, 65535) as u16;
    } else {
        if let Some(t) = cambio.temperatura {
            c.saturacion = 0;
            if let Some((min, max)) = rango {
                c.kelvin = a_kelvin(t, min, max) as u16;
            }
        }
        if let Some(b) = cambio.brillo {
            c.brillo = brillo_a_rango(b, 0, 65535) as u16;
        }
    }
    c
}

/// La carga de `Light::SetColor`: reservado, HSBK y duración (ms).
pub fn carga_color(c: &Color) -> Vec<u8> {
    let mut v = vec![0u8];
    for x in [c.tono, c.saturacion, c.brillo, c.kelvin] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(&0u32.to_le_bytes());
    v
}

/// La carga de `Light::SetPower`: nivel (0 o 65535) y duración (ms).
pub fn carga_encendido(on: bool) -> Vec<u8> {
    let mut v = if on { 65535u16 } else { 0 }.to_le_bytes().to_vec();
    v.extend_from_slice(&0u32.to_le_bytes());
    v
}

pub fn hallazgo(mac: &[u8; 6], ip: Ipv4Addr, nombre: Option<String>, pid: Option<u32>, color: Option<Color>) -> Hallazgo {
    let (con_color, rango, interruptor) = pid.map(producto).unwrap_or((true, Some((2500, 9000)), false));
    let (capacidades, manejo) = if interruptor || pid.is_none() {
        let motivo = if interruptor {
            "Es un interruptor LIFX. Pronto podrás manejarlo desde Nexo."
        } else {
            "Es una luz LIFX, pero no contestó qué modelo es. Busca de nuevo."
        };
        (Capacidades::ninguna(), Manejo::Pronto { motivo: motivo.into() })
    } else {
        (capacidades(con_color, rango), Manejo::Listo)
    };
    Hallazgo {
        id: format!("lifx:{}", mac_texto(mac)),
        via: Via::Wifi,
        proveedor: Proveedor::Lifx,
        clase: if interruptor { Clase::Enchufe } else { Clase::Luz },
        forma: (!interruptor).then_some(FormaLuz::Foco),
        marca: "LIFX".into(),
        nombre: nombre.unwrap_or_else(|| "Luz LIFX".into()),
        modelo: pid.map(|p| format!("Producto {p}")),
        direccion: Some(ip.to_string()),
        mac: mac_normal(&mac_texto(mac)),
        senal: None,
        version: None,
        capacidades,
        estado: color.filter(|_| !interruptor).map(|c| estado_de(&c, rango)),
        manejo,
    }
}

/// El rango de Kelvin por el modelo guardado ("Producto 27").
pub fn rango_de_modelo(modelo: Option<&str>) -> Option<(u32, u32)> {
    let pid = modelo?.strip_prefix("Producto ")?.trim().parse::<u32>().ok()?;
    producto(pid).1
}

pub fn buscar(fin: Instant) -> Vec<Hallazgo> {
    let Ok(s) = super::abrir_udp(0) else { return Vec::new() };
    let difusion = SocketAddr::from((Ipv4Addr::BROADCAST, PUERTO));
    struct Vista {
        mac: [u8; 6],
        ip: Ipv4Addr,
        nombre: Option<String>,
        pid: Option<u32>,
        color: Option<Color>,
    }
    let mut luces: Vec<Vista> = Vec::new();
    let mut envios = [Instant::now(), Instant::now() + Duration::from_millis(1500)].into_iter().peekable();
    while Instant::now() < fin {
        if envios.peek().is_some_and(|t| Instant::now() >= *t) {
            envios.next();
            let _ = s.send_to(&mensaje(TIPO_GET_SERVICE, None), difusion);
            // Las que no contestaron su modelo o su color, otra vez.
            for l in luces.iter().filter(|l| l.pid.is_none() || l.color.is_none()) {
                let _ = s.send_to(&mensaje(TIPO_GET_VERSION, Some(l.mac)), (l.ip, PUERTO));
                let _ = s.send_to(&mensaje(TIPO_LIGHT_GET, Some(l.mac)), (l.ip, PUERTO));
            }
        }
        let hasta = envios.peek().copied().unwrap_or(fin).min(fin);
        super::recibir(&s, hasta, 200, |datos, origen| {
            let IpAddr::V4(ip) = origen.ip() else { return };
            if !super::es_local(origen.ip()) {
                return;
            }
            let Some((tipo, mac, carga)) = leer(datos) else { return };
            match tipo {
                TIPO_STATE_SERVICE if !luces.iter().any(|l| l.mac == mac) && luces.len() < 32 => {
                    luces.push(Vista { mac, ip, nombre: None, pid: None, color: None });
                    let _ = s.send_to(&mensaje(TIPO_GET_VERSION, Some(mac)), (ip, PUERTO));
                    let _ = s.send_to(&mensaje(TIPO_LIGHT_GET, Some(mac)), (ip, PUERTO));
                }
                TIPO_STATE_VERSION if carga.len() >= 8 => {
                    if let Some(l) = luces.iter_mut().find(|l| l.mac == mac) {
                        l.pid = Some(u32::from_le_bytes([carga[4], carga[5], carga[6], carga[7]]));
                    }
                }
                TIPO_LIGHT_STATE => {
                    if let (Some(l), Some((c, n))) = (luces.iter_mut().find(|l| l.mac == mac), estado_crudo(carga)) {
                        l.color = Some(c);
                        l.nombre = n.or(l.nombre.take());
                    }
                }
                TIPO_STATE_LABEL => {
                    if let Some(l) = luces.iter_mut().find(|l| l.mac == mac) {
                        l.nombre = nombre(carga);
                    }
                }
                _ => {}
            }
        });
    }
    luces.into_iter().map(|l| hallazgo(&l.mac, l.ip, l.nombre, l.pid, l.color)).collect()
}

/// Manda un mensaje a una luz y espera la respuesta del tipo pedido (o el acuse), con un reintento.
fn preguntar(ip: Ipv4Addr, mac: [u8; 6], tipo: u16, carga: &[u8], esperado: u16) -> Result<Vec<u8>, String> {
    let s = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|_| super::no_responde(&ip.to_string()))?;
    for _ in 0..2 {
        let sec = SECUENCIA.fetch_add(1, Ordering::Relaxed);
        let m = mensaje_con(tipo, Some(mac), carga, esperado == TIPO_ACUSE, sec);
        let _ = s.send_to(&m, (ip, PUERTO));
        let fin = Instant::now() + Duration::from_millis(700);
        let mut hallado = None;
        super::recibir(&s, fin, 700, |datos, origen| {
            if hallado.is_some() || origen.ip() != IpAddr::V4(ip) {
                return;
            }
            if let Some((t, m, c)) = leer(datos)
                && t == esperado
                && m == mac
                && (esperado != TIPO_ACUSE || datos.get(23) == Some(&sec))
            {
                hallado = Some(c.to_vec());
            }
        });
        if let Some(c) = hallado {
            return Ok(c);
        }
    }
    Err(super::no_responde(&ip.to_string()))
}

/// Cómo está una luz.
pub fn leer_luz(ip: Ipv4Addr, mac: [u8; 6], rango: Option<(u32, u32)>) -> Result<EstadoAparato, String> {
    let c = preguntar(ip, mac, TIPO_LIGHT_GET, &[], TIPO_LIGHT_STATE)?;
    let (color, _) = estado_crudo(&c).ok_or("La luz LIFX contestó algo que no entiendo.")?;
    Ok(estado_de(&color, rango))
}

/// Manda un cambio: el color (si cambia algo de él) y luego el encendido.
pub fn cambiar(ip: Ipv4Addr, mac: [u8; 6], rango: Option<(u32, u32)>, cambio: &Cambio) -> Result<(), String> {
    if cambio.encendido == Some(false) {
        preguntar(ip, mac, TIPO_SET_POWER, &carga_encendido(false), TIPO_ACUSE)?;
        return Ok(());
    }
    if cambio.color.is_some() || cambio.temperatura.is_some() || cambio.brillo.is_some() {
        let c = preguntar(ip, mac, TIPO_LIGHT_GET, &[], TIPO_LIGHT_STATE)?;
        let (actual, _) = estado_crudo(&c).ok_or("La luz LIFX contestó algo que no entiendo.")?;
        let nuevo = color_nuevo(&actual, cambio, rango);
        preguntar(ip, mac, TIPO_SET_COLOR, &carga_color(&nuevo), TIPO_ACUSE)?;
    }
    if cambio.encendido == Some(true) {
        preguntar(ip, mac, TIPO_SET_POWER, &carga_encendido(true), TIPO_ACUSE)?;
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lan_lifx_arma_getservice() {
        let m = mensaje(TIPO_GET_SERVICE, None);
        assert_eq!(m.len(), 36);
        // Tamaño 36, protocolo 1024 + addressable + tagged = 0x3400.
        assert_eq!(&m[0..4], &[36, 0, 0x00, 0x34]);
        assert_eq!(&m[8..16], &[0; 8]);
        assert_eq!(&m[32..34], &[2, 0]);
        let a_una = mensaje(TIPO_LIGHT_GET, Some([0xd0, 0x73, 0xd5, 1, 2, 3]));
        assert_eq!(&a_una[2..4], &[0x00, 0x14]);
        assert_eq!(&a_una[8..14], &[0xd0, 0x73, 0xd5, 1, 2, 3]);
        assert_eq!(&a_una[32..34], &[101, 0]);
    }

    #[test]
    fn lan_lifx_lee_las_respuestas() {
        // StateService: el encabezado con la MAC y la carga (servicio 1, puerto 56700).
        let mut r = mensaje(TIPO_STATE_SERVICE, Some([0xd0, 0x73, 0xd5, 0xaa, 0xbb, 0xcc]));
        r.extend_from_slice(&[1, 0x7c, 0xdd, 0, 0]);
        r[0..2].copy_from_slice(&41u16.to_le_bytes());
        let (tipo, mac, carga) = leer(&r).unwrap_or_else(|| panic!("no se leyó"));
        assert_eq!((tipo, mac_texto(&mac), carga.len()), (3, "d073d5aabbcc".to_string(), 5));
        let mut etiqueta = [0u8; 32];
        let texto = "Lámpara sala".as_bytes();
        etiqueta[..texto.len()].copy_from_slice(texto);
        assert_eq!(nombre(&etiqueta).as_deref(), Some("Lámpara sala"));
        assert_eq!(leer(&[0; 10]), None);
        let h = hallazgo(&mac, Ipv4Addr::new(192, 168, 1, 8), None, Some(27), None);
        assert_eq!((h.id.as_str(), h.nombre.as_str(), h.manejo.clone()), ("lifx:d073d5aabbcc", "Luz LIFX", Manejo::Listo));
        assert_eq!(h.mac.as_deref(), Some("d0:73:d5:aa:bb:cc"));
        assert_eq!(mac_de_id(&h.id), Some(mac));
    }

    #[test]
    fn lan_lifx_productos() {
        // 27 = LIFX A19 (color, 2500–9000 K); 51 = Mini W (solo brillo); 70 = LIFX Switch.
        assert_eq!(producto(27), (true, Some((2500, 9000)), false));
        assert_eq!(producto(51), (false, None, false));
        assert!(producto(70).2);
        let interruptor = hallazgo(&[0xd0, 0x73, 0xd5, 1, 2, 3], Ipv4Addr::new(192, 168, 1, 9), None, Some(70), None);
        assert!(matches!(interruptor.manejo, Manejo::Pronto { .. }));
        assert_eq!(capacidades(false, None), Capacidades::luz(true, false, false));
        assert_eq!(rango_de_modelo(Some("Producto 27")), Some((2500, 9000)));
    }

    #[test]
    fn lan_lifx_color_y_encendido() {
        // Light::State: tono 120°, saturación plena, brillo 50 %, 3500 K, encendida, "Escritorio".
        let mut carga = Vec::new();
        for x in [21845u16, 65535, 32768, 3500, 0, 65535] {
            carga.extend_from_slice(&x.to_le_bytes());
        }
        let mut etiqueta = [0u8; 32];
        etiqueta[..10].copy_from_slice(b"Escritorio");
        carga.extend_from_slice(&etiqueta);
        carga.extend_from_slice(&[0; 8]);
        let (c, n) = estado_crudo(&carga).unwrap_or_else(|| panic!("no se leyó"));
        assert_eq!(n.as_deref(), Some("Escritorio"));
        let e = estado_de(&c, Some((2500, 9000)));
        assert_eq!((e.encendido, e.modo, e.color.map(|x| (x.h, x.s))), (Some(true), Some(ModoLuz::Color), Some((120, 100))));
        assert_eq!(e.brillo, Some(51));
        // A blanco cálido: saturación 0 y el Kelvin más bajo; el tono queda.
        let blanco = color_nuevo(&c, &Cambio { temperatura: Some(0), ..Default::default() }, Some((2500, 9000)));
        assert_eq!((blanco.saturacion, blanco.kelvin, blanco.tono), (0, 2500, 21845));
        let rojo = color_nuevo(&c, &Cambio { color: Some(Hsv { h: 0, s: 100, v: 100 }), ..Default::default() }, None);
        assert_eq!((rojo.tono, rojo.saturacion, rojo.brillo), (0, 65535, 65535));
        // SetColor: 13 bytes (reservado, HSBK, duración); SetPower: 6 bytes.
        let m = mensaje_con(TIPO_SET_COLOR, Some([0xd0, 0x73, 0xd5, 1, 2, 3]), &carga_color(&rojo), true, 7);
        assert_eq!((m.len(), m[22], m[23]), (49, 0b10, 7));
        assert_eq!(&m[36..45], &[0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xac, 0x0d]);
        assert_eq!(carga_encendido(true), vec![0xff, 0xff, 0, 0, 0, 0]);
    }
}
