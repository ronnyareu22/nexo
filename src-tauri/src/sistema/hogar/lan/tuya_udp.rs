//! Los avisos que los aparatos Tuya mandan solos a la red local (cada 5 s, más o menos).
//!
//! - Puerto 6666: versión 3.1, JSON a la vista dentro de un marco 55AA.
//! - Puerto 6667: versión 3.3 y 3.4, el JSON cifrado con AES-128-ECB y la clave UDP conocida.
//! - Puerto 7000: versión 3.5, marco 6699 con AES-GCM y la misma clave. También llegan avisos de la app
//!   (`"from":"app"`): esos se ignoran.
//!
//! Igual que el buscador de tinytuya (`decrypt_udp`). Solo sirve para saber que el aparato está, su IP y su
//! versión: se maneja por la nube.

use super::super::modelo::{Capacidades, Clase, Hallazgo, Manejo, Proveedor, Via};
use super::aes;
use std::net::IpAddr;
use std::time::Instant;

/// Los puertos donde escuchar.
pub const PUERTOS: [u16; 3] = [6666, 6667, 7000];

/// md5("yGAdlopoPVldABfn"): la clave de los avisos (la misma para todos los aparatos; viene de tuya-convert).
pub const CLAVE_UDP: [u8; 16] =
    [0x6c, 0x1e, 0xc8, 0xe2, 0xbb, 0x9b, 0xb5, 0x9a, 0xb5, 0x0b, 0x0d, 0xaf, 0x64, 0x9b, 0x41, 0x0a];

const PREFIJO_55AA: u32 = 0x0000_55AA;
const PREFIJO_6699: u32 = 0x0000_6699;

fn u32_be(d: &[u8], i: usize) -> Option<u32> {
    d.get(i..i + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn es_json(b: &[u8]) -> bool {
    b.first() == Some(&b'{') && b.last() == Some(&b'}')
}

/// El JSON de un aviso, ya descifrado. `None` si no es un aviso Tuya o no se pudo leer.
pub fn descifrar(datos: &[u8]) -> Option<String> {
    let texto = match u32_be(datos, 0)? {
        PREFIJO_55AA => {
            // Cabecera de 16 (prefijo, número, orden, largo), 4 de código de retorno, carga, 4 de CRC y 4 de cierre.
            let largo = u32_be(datos, 12)? as usize;
            let fin = 16usize.checked_add(largo)?.checked_sub(8)?;
            let carga = datos.get(20..fin)?;
            if es_json(carga) {
                carga.to_vec()
            } else {
                aes::descifrar_ecb(&CLAVE_UDP, carga)?
            }
        }
        PREFIJO_6699 => {
            // Cabecera de 18 (prefijo, 2 sin uso, número, orden, largo); luego IV de 12, cifrado, etiqueta de 16 y
            // cierre de 4. La cabecera sin el prefijo es el dato asociado (no hace falta: no se revisa la etiqueta).
            let largo = u32_be(datos, 14)? as usize;
            let fin = 18usize.checked_add(largo)?.checked_sub(16)?;
            let mut iv = [0u8; 12];
            iv.copy_from_slice(datos.get(18..30)?);
            let mut claro = aes::ctr_gcm(&CLAVE_UDP, &iv, datos.get(30..fin)?);
            // A veces llega con un código de retorno de 4 bytes antes del JSON.
            if claro.first() != Some(&b'{') && claro.get(4) == Some(&b'{') {
                claro.drain(..4);
            }
            while claro.last() == Some(&0) {
                claro.pop();
            }
            claro
        }
        // Sin marco: el JSON cifrado a secas.
        _ => aes::descifrar_ecb(&CLAVE_UDP, datos)?,
    };
    String::from_utf8(texto).ok().filter(|t| t.starts_with('{'))
}

/// Lo que se saca de un aviso: el id, la IP, la versión y el producto.
#[derive(Debug, Clone, PartialEq)]
pub struct Aviso {
    pub id: String,
    pub ip: String,
    pub version: Option<String>,
    pub producto: Option<String>,
}

/// Lee el JSON de un aviso. La IP es la de quien lo mandó (como tinytuya), salvo que no se sepa.
pub fn leer(json: &str, origen: Option<IpAddr>) -> Option<Aviso> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    if v.get("from").and_then(|f| f.as_str()) == Some("app") {
        return None;
    }
    let id = v.get("gwId").and_then(|g| g.as_str()).filter(|g| !g.is_empty() && g.len() <= 64)?;
    if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let ip = origen
        .map(|o| o.to_string())
        .or_else(|| v.get("ip").and_then(|i| i.as_str()).map(str::to_string))?;
    let texto = |k: &str| v.get(k).and_then(|x| x.as_str()).filter(|x| !x.is_empty()).map(str::to_string);
    Some(Aviso { id: id.to_string(), ip, version: texto("version"), producto: texto("productKey") })
}

pub fn hallazgo(a: &Aviso) -> Hallazgo {
    Hallazgo {
        id: format!("tuya:{}", a.id),
        via: Via::Wifi,
        proveedor: Proveedor::Tuya,
        // El aviso no dice qué es: lo sabe la nube.
        clase: Clase::Otro,
        forma: None,
        marca: "Tuya / Smart Life".into(),
        nombre: "Aparato Tuya".into(),
        modelo: a.producto.clone(),
        direccion: Some(a.ip.clone()),
        senal: None,
        version: a.version.clone(),
        capacidades: Capacidades::ninguna(),
        estado: None,
        manejo: Manejo::CuentaTuya,
    }
}

/// Escucha los tres puertos hasta `fin`. Lo que no se pueda abrir se salta.
pub fn buscar(fin: Instant) -> Vec<Hallazgo> {
    let sockets: Vec<_> = PUERTOS.iter().filter_map(|&p| super::abrir_udp(p).ok()).collect();
    if sockets.is_empty() {
        return Vec::new();
    }
    let mut vistos: Vec<Aviso> = Vec::new();
    while Instant::now() < fin {
        for s in &sockets {
            super::recibir(s, fin, 60, |datos, origen| {
                if !super::es_local(origen.ip()) || vistos.len() >= 64 {
                    return;
                }
                if let Some(a) = descifrar(datos).and_then(|j| leer(&j, Some(origen.ip())))
                    && !vistos.iter().any(|x| x.id == a.id)
                {
                    vistos.push(a);
                }
            });
        }
    }
    vistos.iter().map(hallazgo).collect()
}

#[cfg(test)]
pub(super) fn marco_55aa(carga: &[u8]) -> Vec<u8> {
    // Cabecera, código de retorno, carga, CRC (no se revisa) y cierre.
    let largo = (4 + carga.len() + 8) as u32;
    let mut m = Vec::new();
    m.extend_from_slice(&PREFIJO_55AA.to_be_bytes());
    m.extend_from_slice(&0u32.to_be_bytes());
    m.extend_from_slice(&0x13u32.to_be_bytes());
    m.extend_from_slice(&largo.to_be_bytes());
    m.extend_from_slice(&0u32.to_be_bytes());
    m.extend_from_slice(carga);
    m.extend_from_slice(&0u32.to_be_bytes());
    m.extend_from_slice(&0x0000_AA55u32.to_be_bytes());
    m
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const JSON: &str = r#"{"ip":"192.168.1.40","gwId":"eb0123456789abcdefgh","active":2,"ability":0,"mode":0,"encrypt":true,"productKey":"keyjup78v54myhan","version":"3.3"}"#;

    #[test]
    fn lan_tuya_clave_udp_es_el_md5_conocido() {
        // md5("yGAdlopoPVldABfn") = 6c1ec8e2bb9bb59ab50b0daf649b410a (calculado con hashlib).
        assert_eq!(CLAVE_UDP.to_vec(), aes::hex("6c1ec8e2bb9bb59ab50b0daf649b410a"));
    }

    #[test]
    fn lan_tuya_lee_el_aviso_a_la_vista_6666() {
        let m = marco_55aa(JSON.as_bytes());
        assert_eq!(descifrar(&m).as_deref(), Some(JSON));
        let a = leer(JSON, None);
        assert_eq!(
            a,
            Some(Aviso {
                id: "eb0123456789abcdefgh".into(),
                ip: "192.168.1.40".into(),
                version: Some("3.3".into()),
                producto: Some("keyjup78v54myhan".into()),
            })
        );
    }

    #[test]
    fn lan_tuya_descifra_el_aviso_3_3_de_6667() {
        let m = marco_55aa(&aes::cifrar_ecb(&CLAVE_UDP, JSON.as_bytes()));
        assert_eq!(descifrar(&m).as_deref(), Some(JSON));
        // La IP de quien lo mandó manda sobre la del JSON.
        let a = leer(JSON, "192.168.1.77".parse().ok());
        assert_eq!(a.map(|a| a.ip), Some("192.168.1.77".into()));
        let h = hallazgo(&leer(JSON, None).unwrap_or(Aviso { id: String::new(), ip: String::new(), version: None, producto: None }));
        assert_eq!(h.id, "tuya:eb0123456789abcdefgh");
        assert_eq!(h.manejo, Manejo::CuentaTuya);
        assert_eq!(h.direccion.as_deref(), Some("192.168.1.40"));
    }

    #[test]
    fn lan_tuya_descifra_el_aviso_3_5_de_7000() {
        // Un marco 6699 armado como el de un aparato: código de retorno, JSON y ceros al final.
        let json = JSON.replace("3.3", "3.5");
        let mut claro = vec![0, 0, 0, 0];
        claro.extend_from_slice(json.as_bytes());
        claro.extend_from_slice(&[0, 0]);
        let iv: [u8; 12] = [7; 12];
        let cifrado = aes::ctr_gcm(&CLAVE_UDP, &iv, &claro);
        let largo = (12 + cifrado.len() + 16) as u32;
        let mut m = Vec::new();
        m.extend_from_slice(&PREFIJO_6699.to_be_bytes());
        m.extend_from_slice(&[0, 0]);
        m.extend_from_slice(&0u32.to_be_bytes());
        m.extend_from_slice(&0x25u32.to_be_bytes());
        m.extend_from_slice(&largo.to_be_bytes());
        m.extend_from_slice(&iv);
        m.extend_from_slice(&cifrado);
        m.extend_from_slice(&[0xAB; 16]);
        m.extend_from_slice(&0x0000_9966u32.to_be_bytes());
        assert_eq!(descifrar(&m), Some(json));
    }

    #[test]
    fn lan_tuya_ignora_lo_que_no_sirve() {
        assert_eq!(descifrar(b"hola"), None);
        assert_eq!(descifrar(&[0, 0, 0x55, 0xAA, 0, 0, 0, 1]), None);
        // Un largo mentiroso no se sale del paquete.
        let mut m = marco_55aa(JSON.as_bytes());
        m[12..16].copy_from_slice(&9999u32.to_be_bytes());
        assert_eq!(descifrar(&m), None);
        assert_eq!(leer(r#"{"from":"app","gwId":"x"}"#, None), None);
        assert_eq!(leer(r#"{"ip":"1.2.3.4"}"#, None), None);
        assert_eq!(leer(r#"{"gwId":"../malo","ip":"1.2.3.4"}"#, None), None);
    }
}
