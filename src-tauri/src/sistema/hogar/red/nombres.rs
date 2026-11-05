//! El nombre de cada equipo por su IP (DNS inverso), con plazo corto y sin dejar nada esperando.
//!
//! - Al router (puerto 53): una pregunta PTR por IP ("7.1.168.192.in-addr.arpa"). Los routers de casa suelen
//!   saber el nombre que cada equipo dio al pedir su IP ("iPhone-de-Ana", "DESKTOP-4F2K").
//! - Por mDNS (224.0.0.251): las mismas preguntas juntas; contestan las Mac, los iPhone y lo que tenga Bonjour
//!   ("MacBook-Pro.local").
//! - Se lee lo que llegue durante el plazo con el mismo lector de `lan::mdns` y se cierra el socket.

use crate::sistema::hogar::lan::mdns;
use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// "192.168.1.7" → "7.1.168.192.in-addr.arpa".
pub fn inverso(ip: Ipv4Addr) -> String {
    let o = ip.octets();
    format!("{}.{}.{}.{}.in-addr.arpa", o[3], o[2], o[1], o[0])
}

/// La IP de un nombre inverso (por etiquetas).
pub fn ip_de_inverso(etiquetas: &[String]) -> Option<Ipv4Addr> {
    if etiquetas.len() != 6 || !etiquetas[4].eq_ignore_ascii_case("in-addr") || !etiquetas[5].eq_ignore_ascii_case("arpa") {
        return None;
    }
    let n: Vec<u8> = etiquetas[..4].iter().filter_map(|e| e.parse::<u8>().ok()).collect();
    (n.len() == 4).then(|| Ipv4Addr::new(n[3], n[2], n[1], n[0]))
}

/// Un nombre de equipo presentable: sin ".local", ".lan" ni ".home", y nada que sea solo una IP.
pub fn limpio(etiquetas: &[String]) -> Option<String> {
    let mut e: Vec<&str> = etiquetas.iter().map(String::as_str).filter(|x| !x.is_empty()).collect();
    while let Some(ultima) = e.last() {
        if ["local", "lan", "home", "localdomain", "internal", "domain", "router", "home-gateway"].contains(&ultima.to_ascii_lowercase().as_str())
            && e.len() > 1
        {
            e.pop();
        } else {
            break;
        }
    }
    let primero = e.first()?;
    let t: String = primero.chars().filter(|c| !c.is_control()).take(48).collect();
    let solo_numeros = t.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '.');
    (!t.is_empty() && !solo_numeros && !t.eq_ignore_ascii_case("localhost")).then_some(t)
}

/// Los nombres que se encontraron, por IP.
pub fn inversos(ips: &[Ipv4Addr], router: Option<Ipv4Addr>, plazo: Duration) -> BTreeMap<Ipv4Addr, String> {
    let mut v = BTreeMap::new();
    let lista: Vec<Ipv4Addr> = ips.iter().copied().take(254).collect();
    if lista.is_empty() {
        return v;
    }
    let Ok(s) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else { return v };
    let _ = s.set_multicast_ttl_v4(255);
    let nombres: Vec<String> = lista.iter().map(|ip| inverso(*ip)).collect();
    if let Some(r) = router {
        for (i, n) in nombres.iter().enumerate() {
            let _ = s.send_to(&mdns::pregunta(i as u16 + 1, &[n.as_str()], mdns::TIPO_PTR, false), (r, 53));
        }
    }
    for (i, tanda) in nombres.chunks(24).enumerate() {
        let t: Vec<&str> = tanda.iter().map(String::as_str).collect();
        let _ = s.send_to(&mdns::pregunta(0x4000 + i as u16, &t, mdns::TIPO_PTR, true), SocketAddr::from((Ipv4Addr::new(224, 0, 0, 251), 5353)));
    }
    let fin = Instant::now() + plazo;
    let mut buf = [0u8; 9000];
    while let Some(queda) = fin.checked_duration_since(Instant::now()) {
        if s.set_read_timeout(Some(queda.max(Duration::from_millis(1)))).is_err() {
            break;
        }
        let Ok((n, _)) = s.recv_from(&mut buf) else { break };
        for r in mdns::leer(&buf[..n]).unwrap_or_default() {
            if let mdns::Registro::Ptr { nombre, destino } = r
                && let Some(ip) = ip_de_inverso(&nombre)
                && lista.contains(&ip)
                && let Some(t) = limpio(&destino)
            {
                v.entry(ip).or_insert(t);
            }
        }
        if v.len() == lista.len() {
            break;
        }
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn e(t: &str) -> Vec<String> {
        t.split('.').map(str::to_string).collect()
    }

    #[test]
    fn red_nombres_inversos() {
        assert_eq!(inverso(Ipv4Addr::new(192, 168, 1, 7)), "7.1.168.192.in-addr.arpa");
        assert_eq!(ip_de_inverso(&e("7.1.168.192.in-addr.arpa")), Some(Ipv4Addr::new(192, 168, 1, 7)));
        assert_eq!(ip_de_inverso(&e("_hue._tcp.local")), None);
        assert_eq!(limpio(&e("MacBook-Pro.local")).as_deref(), Some("MacBook-Pro"));
        assert_eq!(limpio(&e("DESKTOP-4F2K.lan")).as_deref(), Some("DESKTOP-4F2K"));
        assert_eq!(limpio(&e("192-168-1-7.home")), None);
        assert_eq!(limpio(&e("localhost")), None);
    }

    #[test]
    fn red_lee_la_respuesta_del_router() {
        // Lo que contesta un router a la pregunta PTR (con el nombre comprimido hacia la pregunta).
        let p = mdns::pregunta(3, &["7.1.168.192.in-addr.arpa"], mdns::TIPO_PTR, false);
        assert_eq!(&p[..4], &[0, 3, 1, 0]);
        let mut r = p.clone();
        r[2] = 0x81;
        r[3] = 0x80;
        r[7] = 1;
        r.extend_from_slice(&[0xC0, 12, 0, 12, 0, 1, 0, 0, 0, 60]);
        let mut destino = Vec::new();
        for parte in ["iPhone-de-Ana", "lan"] {
            destino.push(parte.len() as u8);
            destino.extend_from_slice(parte.as_bytes());
        }
        destino.push(0);
        r.extend_from_slice(&(destino.len() as u16).to_be_bytes());
        r.extend(destino);
        let registros = mdns::leer(&r).unwrap_or_default();
        let Some(mdns::Registro::Ptr { nombre, destino }) = registros.first() else { panic!("sin PTR") };
        assert_eq!(ip_de_inverso(nombre), Some(Ipv4Addr::new(192, 168, 1, 7)));
        assert_eq!(limpio(destino).as_deref(), Some("iPhone-de-Ana"));
    }
}
