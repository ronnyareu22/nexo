//! LIFX por la red local (UDP 56700): solo se detecta, con su nombre. Manejarla llega más adelante.
//!
//! - `GetService` (tipo 2) a toda la red con `tagged`; cada luz contesta `StateService` (tipo 3) y en el
//!   encabezado viene su MAC. Luego `GetLabel` (23) a cada una para su nombre (`StateLabel`, 25).
//! - Encabezado de 36 bytes, todo en little endian: tamaño, protocolo 1024 con `addressable` y `tagged`, origen,
//!   destino (8 bytes; los 6 primeros son la MAC), reservado, banderas, secuencia, reservado, tipo, reservado.

use super::super::modelo::{Capacidades, Clase, FormaLuz, Hallazgo, Manejo, Proveedor, Via};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

pub const PUERTO: u16 = 56700;
const TIPO_GET_SERVICE: u16 = 2;
const TIPO_STATE_SERVICE: u16 = 3;
const TIPO_GET_LABEL: u16 = 23;
const TIPO_STATE_LABEL: u16 = 25;
/// Quién pregunta (cualquier número distinto de 0 hace que la luz conteste solo a este equipo).
const ORIGEN: u32 = 0x4E45_584F;

/// Un mensaje sin carga: a todas (`destino` en ceros, `tagged`) o a una MAC.
pub fn mensaje(tipo: u16, destino: Option<[u8; 6]>) -> Vec<u8> {
    let mut m = Vec::with_capacity(36);
    m.extend_from_slice(&36u16.to_le_bytes());
    let tagged: u16 = if destino.is_none() { 1 << 13 } else { 0 };
    m.extend_from_slice(&(1024u16 | (1 << 12) | tagged).to_le_bytes());
    m.extend_from_slice(&ORIGEN.to_le_bytes());
    let mut objetivo = [0u8; 8];
    if let Some(mac) = destino {
        objetivo[..6].copy_from_slice(&mac);
    }
    m.extend_from_slice(&objetivo);
    m.extend_from_slice(&[0; 6]);
    // Pide respuesta (res_required).
    m.push(1);
    m.push(0);
    m.extend_from_slice(&[0; 8]);
    m.extend_from_slice(&tipo.to_le_bytes());
    m.extend_from_slice(&[0; 2]);
    m
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

/// El nombre de `StateLabel`: 32 bytes en UTF-8, rellenos con ceros.
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

pub fn hallazgo(mac: &[u8; 6], ip: Ipv4Addr, nombre: Option<String>) -> Hallazgo {
    Hallazgo {
        id: format!("lifx:{}", mac_texto(mac)),
        via: Via::Wifi,
        proveedor: Proveedor::Lifx,
        clase: Clase::Luz,
        forma: Some(FormaLuz::Foco),
        marca: "LIFX".into(),
        nombre: nombre.unwrap_or_else(|| "Luz LIFX".into()),
        modelo: None,
        direccion: Some(ip.to_string()),
        senal: None,
        version: None,
        capacidades: Capacidades::ninguna(),
        estado: None,
        manejo: Manejo::Pronto { motivo: "Es una luz LIFX. Pronto podrás manejarla desde Nexo.".into() },
    }
}

pub fn buscar(fin: Instant) -> Vec<Hallazgo> {
    let Ok(s) = super::abrir_udp(0) else { return Vec::new() };
    let difusion = SocketAddr::from((Ipv4Addr::BROADCAST, PUERTO));
    let mut luces: Vec<([u8; 6], Ipv4Addr, Option<String>)> = Vec::new();
    let mut envios = [Instant::now(), Instant::now() + Duration::from_millis(1500)].into_iter().peekable();
    while Instant::now() < fin {
        if envios.peek().is_some_and(|t| Instant::now() >= *t) {
            envios.next();
            let _ = s.send_to(&mensaje(TIPO_GET_SERVICE, None), difusion);
        }
        let hasta = envios.peek().copied().unwrap_or(fin).min(fin);
        super::recibir(&s, hasta, 200, |datos, origen| {
            let IpAddr::V4(ip) = origen.ip() else { return };
            if !super::es_local(origen.ip()) {
                return;
            }
            match leer(datos) {
                Some((TIPO_STATE_SERVICE, mac, _)) if !luces.iter().any(|(m, _, _)| *m == mac) && luces.len() < 32 => {
                    luces.push((mac, ip, None));
                    let _ = s.send_to(&mensaje(TIPO_GET_LABEL, Some(mac)), (ip, PUERTO));
                }
                Some((TIPO_STATE_LABEL, mac, carga)) => {
                    if let Some(l) = luces.iter_mut().find(|(m, _, _)| *m == mac) {
                        l.2 = nombre(carga);
                    }
                }
                _ => {}
            }
        });
    }
    luces.into_iter().map(|(mac, ip, n)| hallazgo(&mac, ip, n)).collect()
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
        let a_una = mensaje(TIPO_GET_LABEL, Some([0xd0, 0x73, 0xd5, 1, 2, 3]));
        assert_eq!(&a_una[2..4], &[0x00, 0x14]);
        assert_eq!(&a_una[8..14], &[0xd0, 0x73, 0xd5, 1, 2, 3]);
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
        let h = hallazgo(&mac, Ipv4Addr::new(192, 168, 1, 8), None);
        assert_eq!((h.id.as_str(), h.nombre.as_str()), ("lifx:d073d5aabbcc", "Luz LIFX"));
    }
}
