//! Los equipos de la red local, como los ve el sistema: la tabla de vecinos (ARP), el router (la puerta de enlace)
//! y el tamaño de la red. Sin permisos de administrador ni sockets crudos.
//!
//! - `despertar`: un datagrama UDP de un byte (al puerto 9, "discard") a cada equipo de la red, como mucho 254 (la
//!   /24 de este equipo). Para mandarlo, el sistema pregunta por ARP quién tiene cada IP y así la tabla se llena
//!   con los que están. Se manda en tandas cortas (≈0,1 s en total) y no se espera respuesta.
//! - Mac: la tabla y el router salen de `sysctl` (`NET_RT_FLAGS` con `RTF_LLINFO` y `RTF_GATEWAY`), lo mismo que
//!   leen `arp -an` y `route`; el tamaño de la red, de `getifaddrs`.
//! - Windows: `GetIpNetTable2`, `GetIpForwardTable2` y `GetUnicastIpAddressTable` (IP Helper).

use std::net::{Ipv4Addr, UdpSocket};
use std::time::Duration;

/// La red de este equipo.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Red {
    pub ip: Option<Ipv4Addr>,
    pub prefijo: Option<u8>,
    pub router: Option<Ipv4Addr>,
}

/// Las IPs a despertar: las de la red de este equipo (como mucho su /24), sin la propia, la de la red ni la de
/// difusión.
pub fn hosts(ip: Ipv4Addr, prefijo: u8) -> Vec<Ipv4Addr> {
    let p = prefijo.clamp(24, 30) as u32;
    let mascara = u32::MAX << (32 - p);
    let base = u32::from(ip) & mascara;
    let cuantos = 1u32 << (32 - p);
    (1..cuantos - 1).map(|i| Ipv4Addr::from(base + i)).filter(|h| *h != ip).collect()
}

/// Despierta la tabla de vecinos. Devuelve a cuántas IPs se mandó.
pub fn despertar(red: &Red) -> usize {
    let (Some(ip), Some(prefijo)) = (red.ip, red.prefijo.or(Some(24))) else { return 0 };
    if !(ip.is_private() || ip.is_link_local()) {
        return 0;
    }
    let Ok(s) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else { return 0 };
    let lista = hosts(ip, prefijo);
    for (i, h) in lista.iter().enumerate() {
        let _ = s.send_to(&[0], (*h, 9));
        if i % 32 == 31 {
            std::thread::sleep(Duration::from_millis(8));
        }
    }
    lista.len()
}

/// Bits en uno de una máscara (255.255.255.0 → 24). Windows da el prefijo ya contado.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn prefijo_de(mascara: Ipv4Addr) -> u8 {
    u32::from(mascara).count_ones() as u8
}

/// Una MAC de 6 bytes en la forma de Nexo; `None` si es de difusión, de grupo o vacía.
pub fn mac_de(b: &[u8]) -> Option<String> {
    if b.len() != 6 || b.iter().all(|x| *x == 0) || b[0] & 1 == 1 {
        return None;
    }
    Some(b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(":"))
}

pub use sistema::{red, vecinos};

#[cfg(target_os = "macos")]
mod sistema {
    use super::{Red, mac_de, prefijo_de};
    use std::net::{IpAddr, Ipv4Addr};

    /// El largo de un sockaddr redondeado a 4 bytes (`ROUNDUP` de route.h); uno vacío ocupa 4.
    fn redondeo(n: usize) -> usize {
        if n == 0 { 4 } else { (n + 3) & !3 }
    }

    /// Una dirección de un mensaje de rutas.
    #[derive(Debug, Clone, PartialEq)]
    pub enum Dir {
        Ip(Ipv4Addr),
        Enlace(Vec<u8>),
        Otra,
    }

    /// Los mensajes de un volcado de rutas: (banderas, direcciones en orden de `rtm_addrs`).
    pub fn mensajes(buf: &[u8]) -> Vec<(i32, Vec<(i32, Dir)>)> {
        let cabeza = std::mem::size_of::<libc::rt_msghdr>();
        let mut v = Vec::new();
        let mut i = 0;
        while i + cabeza <= buf.len() {
            let largo = u16::from_ne_bytes([buf[i], buf[i + 1]]) as usize;
            if largo < cabeza || i + largo > buf.len() {
                break;
            }
            let m = &buf[i..i + largo];
            let banderas = i32::from_ne_bytes([m[8], m[9], m[10], m[11]]);
            let cuales = i32::from_ne_bytes([m[12], m[13], m[14], m[15]]);
            let mut pos = cabeza;
            let mut dirs = Vec::new();
            for bit in 0..8 {
                let b = 1 << bit;
                if cuales & b == 0 {
                    continue;
                }
                let Some(&tam) = m.get(pos) else { break };
                let tam = tam as usize;
                let familia = m.get(pos + 1).copied().unwrap_or(0) as i32;
                let d = if familia == libc::AF_INET && tam >= 8 && pos + 8 <= m.len() {
                    Dir::Ip(Ipv4Addr::new(m[pos + 4], m[pos + 5], m[pos + 6], m[pos + 7]))
                } else if familia == libc::AF_LINK && tam >= 8 && pos + 8 <= m.len() {
                    let nombre = m[pos + 5] as usize;
                    let largo_dir = m[pos + 6] as usize;
                    let ini = pos + 8 + nombre;
                    m.get(ini..ini + largo_dir).map(|x| Dir::Enlace(x.to_vec())).unwrap_or(Dir::Otra)
                } else {
                    Dir::Otra
                };
                dirs.push((b, d));
                pos += redondeo(tam);
            }
            v.push((banderas, dirs));
            i += largo;
        }
        v
    }

    /// Un volcado de rutas IPv4 con esas banderas.
    fn volcado(banderas: i32) -> Vec<u8> {
        let mut mib = [libc::CTL_NET, libc::PF_ROUTE, 0, libc::AF_INET, libc::NET_RT_FLAGS, banderas];
        let mut largo: libc::size_t = 0;
        // SAFETY: primero se pide el tamaño y luego se llena un búfer de ese tamaño (con margen por si creció).
        unsafe {
            if libc::sysctl(mib.as_mut_ptr(), 6, std::ptr::null_mut(), &mut largo, std::ptr::null_mut(), 0) != 0 || largo == 0 {
                return Vec::new();
            }
            let mut buf = vec![0u8; largo + 4096];
            let mut tam = buf.len();
            if libc::sysctl(mib.as_mut_ptr(), 6, buf.as_mut_ptr().cast(), &mut tam, std::ptr::null_mut(), 0) != 0 {
                return Vec::new();
            }
            buf.truncate(tam);
            buf
        }
    }

    /// (IP, MAC) de la tabla ARP, sin las incompletas.
    pub fn vecinos() -> Vec<(Ipv4Addr, String)> {
        let mut v: Vec<(Ipv4Addr, String)> = mensajes(&volcado(libc::RTF_LLINFO))
            .into_iter()
            .filter_map(|(_, dirs)| {
                let ip = dirs.iter().find_map(|(b, d)| match d {
                    Dir::Ip(ip) if *b == libc::RTA_DST => Some(*ip),
                    _ => None,
                })?;
                let mac = dirs.iter().find_map(|(b, d)| match d {
                    Dir::Enlace(m) if *b == libc::RTA_GATEWAY => mac_de(m),
                    _ => None,
                })?;
                Some((ip, mac))
            })
            .filter(|(ip, _)| ip.is_private() || ip.is_link_local())
            .collect();
        v.sort();
        v.dedup_by(|a, b| a.0 == b.0);
        v
    }

    /// La puerta de enlace de la ruta por defecto.
    fn router() -> Option<Ipv4Addr> {
        mensajes(&volcado(libc::RTF_GATEWAY)).into_iter().find_map(|(_, dirs)| {
            let destino = dirs.iter().find(|(b, _)| *b == libc::RTA_DST).map(|(_, d)| d.clone());
            let puerta = dirs.iter().find(|(b, _)| *b == libc::RTA_GATEWAY).map(|(_, d)| d.clone());
            match (destino, puerta) {
                (Some(Dir::Ip(d)), Some(Dir::Ip(g))) if d.is_unspecified() && (g.is_private() || g.is_link_local()) => Some(g),
                _ => None,
            }
        })
    }

    /// El prefijo de la interfaz que tiene esa IP.
    fn prefijo(ip: Ipv4Addr) -> Option<u8> {
        let mut lista: *mut libc::ifaddrs = std::ptr::null_mut();
        // SAFETY: getifaddrs reserva la lista y freeifaddrs la suelta; solo se leen sockaddr_in de familia AF_INET.
        unsafe {
            if libc::getifaddrs(&mut lista) != 0 {
                return None;
            }
            let mut p = lista;
            let mut hallado = None;
            while !p.is_null() {
                let i = &*p;
                if !i.ifa_addr.is_null() && !i.ifa_netmask.is_null() && (*i.ifa_addr).sa_family as i32 == libc::AF_INET {
                    let a = &*(i.ifa_addr as *const libc::sockaddr_in);
                    let m = &*(i.ifa_netmask as *const libc::sockaddr_in);
                    if Ipv4Addr::from(u32::from_be(a.sin_addr.s_addr)) == ip {
                        hallado = Some(prefijo_de(Ipv4Addr::from(u32::from_be(m.sin_addr.s_addr))));
                        break;
                    }
                }
                p = i.ifa_next;
            }
            libc::freeifaddrs(lista);
            hallado
        }
    }

    pub fn red() -> Red {
        let ip = match crate::sistema::hogar::lan::ip_propia() {
            Some(IpAddr::V4(v)) => Some(v),
            _ => None,
        };
        Red { ip, prefijo: ip.and_then(prefijo), router: router() }
    }
}

#[cfg(windows)]
mod sistema {
    use super::{Red, mac_de};
    use std::net::{IpAddr, Ipv4Addr};
    use windows::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIpForwardTable2, GetIpNetTable2, GetUnicastIpAddressTable, MIB_IPFORWARD_TABLE2, MIB_IPNET_TABLE2,
        MIB_UNICASTIPADDRESS_TABLE,
    };
    use windows::Win32::Networking::WinSock::{AF_INET, NlnsIncomplete, NlnsUnreachable, SOCKADDR_INET};

    /// La IPv4 de un SOCKADDR_INET (si es IPv4).
    fn ipv4(d: &SOCKADDR_INET) -> Option<Ipv4Addr> {
        // SAFETY: se mira la familia antes de leer la parte IPv4 de la unión.
        unsafe {
            if d.si_family != AF_INET {
                return None;
            }
            Some(Ipv4Addr::from(u32::from_be(d.Ipv4.sin_addr.S_un.S_addr)))
        }
    }

    pub fn vecinos() -> Vec<(Ipv4Addr, String)> {
        let mut tabla: *mut MIB_IPNET_TABLE2 = std::ptr::null_mut();
        let mut v = Vec::new();
        // SAFETY: GetIpNetTable2 reserva la tabla y FreeMibTable la suelta; se leen `NumEntries` filas.
        unsafe {
            if GetIpNetTable2(AF_INET, &mut tabla).is_err() || tabla.is_null() {
                return v;
            }
            let n = (*tabla).NumEntries as usize;
            let filas = std::slice::from_raw_parts((*tabla).Table.as_ptr(), n);
            for f in filas {
                if f.State == NlnsUnreachable || f.State == NlnsIncomplete || f.PhysicalAddressLength != 6 {
                    continue;
                }
                if let (Some(ip), Some(mac)) = (ipv4(&f.Address), mac_de(&f.PhysicalAddress[..6]))
                    && (ip.is_private() || ip.is_link_local())
                {
                    v.push((ip, mac));
                }
            }
            FreeMibTable(tabla as *const _);
        }
        v.sort();
        v.dedup_by(|a, b| a.0 == b.0);
        v
    }

    fn router() -> Option<Ipv4Addr> {
        let mut tabla: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
        // SAFETY: igual que arriba.
        unsafe {
            if GetIpForwardTable2(AF_INET, &mut tabla).is_err() || tabla.is_null() {
                return None;
            }
            let n = (*tabla).NumEntries as usize;
            let filas = std::slice::from_raw_parts((*tabla).Table.as_ptr(), n);
            let mejor = filas
                .iter()
                .filter(|f| f.DestinationPrefix.PrefixLength == 0)
                .filter_map(|f| ipv4(&f.NextHop).filter(|g| !g.is_unspecified()).map(|g| (f.Metric, g)))
                .min_by_key(|(m, _)| *m)
                .map(|(_, g)| g);
            FreeMibTable(tabla as *const _);
            mejor
        }
    }

    fn prefijo(ip: Ipv4Addr) -> Option<u8> {
        let mut tabla: *mut MIB_UNICASTIPADDRESS_TABLE = std::ptr::null_mut();
        // SAFETY: igual que arriba.
        unsafe {
            if GetUnicastIpAddressTable(AF_INET, &mut tabla).is_err() || tabla.is_null() {
                return None;
            }
            let n = (*tabla).NumEntries as usize;
            let filas = std::slice::from_raw_parts((*tabla).Table.as_ptr(), n);
            let p = filas.iter().find(|f| ipv4(&f.Address) == Some(ip)).map(|f| f.OnLinkPrefixLength);
            FreeMibTable(tabla as *const _);
            p
        }
    }

    pub fn red() -> Red {
        let ip = match crate::sistema::hogar::lan::ip_propia() {
            Some(IpAddr::V4(v)) => Some(v),
            _ => None,
        };
        Red { ip, prefijo: ip.and_then(prefijo), router: router() }
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod sistema {
    use super::Red;
    use std::net::Ipv4Addr;

    pub fn vecinos() -> Vec<(Ipv4Addr, String)> {
        Vec::new()
    }

    pub fn red() -> Red {
        Red::default()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn red_ips_a_despertar() {
        let h = hosts(Ipv4Addr::new(192, 168, 1, 20), 24);
        assert_eq!(h.len(), 253);
        assert_eq!((h[0], h[h.len() - 1]), (Ipv4Addr::new(192, 168, 1, 1), Ipv4Addr::new(192, 168, 1, 254)));
        assert!(!h.contains(&Ipv4Addr::new(192, 168, 1, 20)));
        // Una red grande (/16) se limita a la /24 de este equipo; una chica (/28), a la suya.
        assert_eq!(hosts(Ipv4Addr::new(10, 0, 7, 9), 16).len(), 253);
        assert_eq!(hosts(Ipv4Addr::new(10, 0, 7, 9), 28), (1..15).map(|i| Ipv4Addr::new(10, 0, 7, i)).filter(|x| *x != Ipv4Addr::new(10, 0, 7, 9)).collect::<Vec<_>>());
        assert_eq!(prefijo_de(Ipv4Addr::new(255, 255, 255, 0)), 24);
        assert_eq!(mac_de(&[0xa4, 0x2b, 0xb0, 1, 2, 3]).as_deref(), Some("a4:2b:b0:01:02:03"));
        assert_eq!(mac_de(&[0xff; 6]), None);
        assert_eq!(mac_de(&[0x01, 0, 0x5e, 0, 0, 0xfb]), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn red_lee_un_volcado_de_rutas() {
        // Un mensaje RTM_GET de la tabla ARP: rt_msghdr, sockaddr_in (destino) y sockaddr_dl (la MAC en en0).
        let cabeza = std::mem::size_of::<libc::rt_msghdr>();
        assert_eq!(cabeza, 92);
        let mut m = vec![0u8; cabeza];
        m[8..12].copy_from_slice(&libc::RTF_LLINFO.to_ne_bytes());
        m[12..16].copy_from_slice(&(libc::RTA_DST | libc::RTA_GATEWAY).to_ne_bytes());
        let mut sin = vec![16u8, libc::AF_INET as u8, 0, 0, 192, 168, 1, 7];
        sin.resize(16, 0);
        m.extend(sin);
        // sockaddr_dl: largo 20, AF_LINK, índice 2, tipo 6, nombre de 3 ("en0"), dirección de 6.
        let mut dl = vec![20u8, libc::AF_LINK as u8, 2, 0, 6, 3, 6, 0];
        dl.extend_from_slice(b"en0");
        dl.extend_from_slice(&[0xd8, 0x1f, 0x12, 0xaa, 0xbb, 0xcc]);
        dl.resize(20, 0);
        m.extend(dl);
        let largo = m.len() as u16;
        m[0..2].copy_from_slice(&largo.to_ne_bytes());
        let lista = sistema::mensajes(&[m.clone(), m].concat());
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].1[0], (libc::RTA_DST, sistema::Dir::Ip(Ipv4Addr::new(192, 168, 1, 7))));
        assert_eq!(lista[0].1[1], (libc::RTA_GATEWAY, sistema::Dir::Enlace(vec![0xd8, 0x1f, 0x12, 0xaa, 0xbb, 0xcc])));
        assert!(sistema::mensajes(&[1, 2, 3]).is_empty());
    }
}
