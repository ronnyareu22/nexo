//! Red local (Wi-Fi): buscar y manejar sin nube.
//!
//! - `buscar`: una búsqueda corta (≈6 s) de todas las familias a la vez, cada una en su hilo. Al terminar no queda
//!   ningún socket ni hilo vivo. Lo que falle (sin red, puerto ocupado, cortafuegos de Windows) se deja afuera.
//!   - Tuya (avisos en 6666/6667/7000): solo se detecta; se maneja por la nube.
//!   - WiZ (UDP 38899), Yeelight (búsqueda en 239.255.255.250:1982, órdenes por TCP 55443) y Govee (búsqueda en
//!     239.255.255.250:4001, respuestas en 4002, órdenes en 4003): se manejan aquí mismo.
//!   - Home Assistant (mDNS): se muestra con su dirección, para vincularlo.
//!   - mDNS (Hue, teles, HomeKit, Matter, Shelly, ESPHome), SSDP (Roku, Samsung, LG, DIAL) y LIFX: solo se
//!     detectan, con el motivo en palabras.
//! - `leer` y `cambiar`: solo WiZ, Yeelight y Govee, con plazos cortos (≈1,5 s).

mod aes;
mod govee;
mod lifx;
mod mdns;
mod ssdp;
mod tuya_udp;
mod wiz;
mod yeelight;

use super::modelo::{Cambio, Clase, EstadoAparato, Hallazgo, Manejo, Proveedor};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// Plazo de una orden o una lectura.
const PLAZO_ORDEN: Duration = Duration::from_millis(1500);

/// Una familia: busca hasta el instante dado y devuelve lo que encontró.
type Familia = fn(Instant) -> Vec<Hallazgo>;

/// Busca en la red local durante `plazo`. Lo que falle (sin red, puerto ocupado, cortafuegos) se deja afuera.
pub fn buscar(plazo: Duration) -> Vec<Hallazgo> {
    let fin = Instant::now() + plazo;
    let familias: [(&str, Familia); 7] = [
        ("tuya", tuya_udp::buscar),
        ("wiz", wiz::buscar),
        ("yeelight", yeelight::buscar),
        ("govee", govee::buscar),
        ("mdns", mdns::buscar),
        ("ssdp", ssdp::buscar),
        ("lifx", lifx::buscar),
    ];
    let hilos: Vec<_> = familias
        .into_iter()
        .filter_map(|(nombre, f)| std::thread::Builder::new().name(format!("nexo-lan-{nombre}")).spawn(move || f(fin)).ok())
        .collect();
    // Cada familia vuelve sola al llegar a `fin` (sus sockets esperan de a poco): aquí solo se juntan.
    juntar(hilos.into_iter().filter_map(|h| h.join().ok()).collect())
}

/// Lo que solo se ve (todavía sin control).
fn solo_se_ve(h: &Hallazgo) -> bool {
    matches!(h.proveedor, Proveedor::Ninguno | Proveedor::Hue | Proveedor::Lifx)
}

/// Junta lo de todas las familias: primero lo que se maneja y, por IP, una sola cosa de las que solo se ven.
fn juntar(listas: Vec<Vec<Hallazgo>>) -> Vec<Hallazgo> {
    let (mut lista, solo_ver): (Vec<Hallazgo>, Vec<Hallazgo>) = listas.into_iter().flatten().partition(|h| !solo_se_ve(h));
    let mut vistos: Vec<String> = Vec::new();
    lista.retain(|h| {
        let nuevo = !vistos.contains(&h.id);
        vistos.push(h.id.clone());
        nuevo
    });
    let mut resto: Vec<Hallazgo> = Vec::new();
    for h in solo_ver {
        // Si en esa IP ya hay algo que Nexo maneja (un WiZ que además anuncia HomeKit), no se repite.
        if h.direccion.is_some() && lista.iter().any(|m| m.direccion == h.direccion) {
            continue;
        }
        match resto.iter_mut().find(|r| r.id == h.id || (r.direccion.is_some() && r.direccion == h.direccion)) {
            Some(r) => {
                if rango(&h) > rango(r) {
                    *r = h;
                }
            }
            None => resto.push(h),
        }
    }
    let orden = |h: &Hallazgo| match h.manejo {
        Manejo::Listo => 0,
        Manejo::CuentaTuya | Manejo::HomeAssistant { .. } => 1,
        Manejo::Activar { .. } => 2,
        Manejo::Pronto { .. } => 3,
    };
    lista.extend(resto);
    lista.sort_by_cached_key(|h| (orden(h), h.nombre.to_lowercase()));
    lista
}

/// Lo más útil de un mismo aparato visto por dos lados: una tele antes que "otro", y con modelo antes que sin él.
fn rango(h: &Hallazgo) -> u8 {
    let clase = match h.clase {
        Clase::Tv => 3,
        Clase::Otro => 0,
        _ => 2,
    };
    clase * 2 + u8::from(h.modelo.is_some())
}

/// Cómo está un aparato local. `direccion` es su IP; `modelo`, lo que dijo al buscarlo.
pub fn leer(proveedor: Proveedor, direccion: &str, modelo: Option<&str>) -> Result<EstadoAparato, String> {
    match proveedor {
        Proveedor::Wiz => wiz::leer(&ip_de(direccion)?, modelo),
        Proveedor::Yeelight => yeelight::leer(direccion, modelo),
        Proveedor::Govee => govee::leer(&ip_de(direccion)?),
        _ => Err("Nexo todavía no lee este aparato por la red local.".into()),
    }
}

/// Manda un cambio a un aparato local.
pub fn cambiar(proveedor: Proveedor, direccion: &str, modelo: Option<&str>, cambio: &Cambio) -> Result<(), String> {
    if cambio.vacio() {
        return Ok(());
    }
    match proveedor {
        Proveedor::Wiz => wiz::cambiar(&ip_de(direccion)?, modelo, cambio),
        Proveedor::Yeelight => yeelight::cambiar(direccion, modelo, cambio),
        Proveedor::Govee => govee::cambiar(&ip_de(direccion)?, cambio),
        _ => Err("Nexo todavía no maneja este aparato por la red local.".into()),
    }
}

// ---------------------------------------------------------------- Lo común

/// Solo IPv4 de la red local (privadas o de enlace local): nunca se manda nada a internet.
fn ip_de(direccion: &str) -> Result<Ipv4Addr, String> {
    let texto = direccion.rsplit_once(':').map_or(direccion, |(ip, _)| ip);
    match texto.trim().parse::<Ipv4Addr>() {
        Ok(ip) if ip.is_private() || ip.is_link_local() => Ok(ip),
        _ => Err(format!("«{direccion}» no es una dirección de tu red local.")),
    }
}

/// El error de siempre cuando algo local no contesta.
fn no_responde(ip: &str) -> String {
    format!("La luz no responde en {ip}. Revisa que esté encendida y en la misma red.")
}

/// Un socket UDP en ese puerto (0: cualquiera), con permiso de difusión. En la Mac se comparte el puerto con otras
/// apps (SO_REUSEADDR y SO_REUSEPORT), así una que ya escucha los avisos de Tuya o de Govee no lo impide.
fn abrir_udp(puerto: u16) -> std::io::Result<UdpSocket> {
    let s = abrir_compartido(puerto)?;
    s.set_broadcast(true)?;
    Ok(s)
}

#[cfg(target_os = "macos")]
fn abrir_compartido(puerto: u16) -> std::io::Result<UdpSocket> {
    use std::os::fd::FromRawFd;
    if puerto == 0 {
        return UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0));
    }
    // SAFETY: llamadas BSD con argumentos válidos; el descriptor se cierra si algo falla y, si no, pasa al
    // `UdpSocket`, que lo cierra al soltarse.
    unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, libc::IPPROTO_UDP);
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let si: libc::c_int = 1;
        let largo = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
        for opcion in [libc::SO_REUSEADDR, libc::SO_REUSEPORT] {
            libc::setsockopt(fd, libc::SOL_SOCKET, opcion, (&si as *const libc::c_int).cast(), largo);
        }
        let mut dir: libc::sockaddr_in = std::mem::zeroed();
        dir.sin_len = std::mem::size_of::<libc::sockaddr_in>() as u8;
        dir.sin_family = libc::AF_INET as libc::sa_family_t;
        dir.sin_port = puerto.to_be();
        let r = libc::bind(
            fd,
            (&dir as *const libc::sockaddr_in).cast(),
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        );
        if r < 0 {
            let e = std::io::Error::last_os_error();
            libc::close(fd);
            return Err(e);
        }
        Ok(UdpSocket::from_raw_fd(fd))
    }
}

/// En Windows (y lo demás) el puerto no se comparte: si otra app lo tiene, esa familia se salta.
#[cfg(not(target_os = "macos"))]
fn abrir_compartido(puerto: u16) -> std::io::Result<UdpSocket> {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, puerto))
}

/// Lee lo que llegue a `s` hasta `fin`, esperando de a `tanda_ms` como mucho. Vuelve tras una tanda sin nada (o al
/// llegar a `fin`), así quien llama puede reenviar la búsqueda o atender otros sockets.
fn recibir(s: &UdpSocket, fin: Instant, tanda_ms: u64, mut f: impl FnMut(&[u8], SocketAddr)) {
    let mut buf = vec![0u8; 9000];
    loop {
        let queda = fin.saturating_duration_since(Instant::now());
        if queda.is_zero() {
            return;
        }
        let espera = queda.min(Duration::from_millis(tanda_ms)).max(Duration::from_millis(1));
        if s.set_read_timeout(Some(espera)).is_err() {
            return;
        }
        match s.recv_from(&mut buf) {
            Ok((n, origen)) => f(&buf[..n], origen),
            Err(_) => return,
        }
    }
}

/// La IP de este equipo en la red local (la de la interfaz que sale hacia la red), para no listarse a sí mismo.
fn ip_propia() -> Option<IpAddr> {
    let s = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // Conectar un socket UDP no manda nada: solo elige la interfaz.
    s.connect((Ipv4Addr::new(192, 168, 0, 1), 9)).ok()?;
    s.local_addr().ok().map(|a| a.ip()).filter(|ip| !ip.is_unspecified())
}

/// Solo respuestas de la red local.
fn es_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
        IpAddr::V6(_) => false,
    }
}

#[cfg(test)]
mod pruebas {
    use super::super::modelo::{Capacidades, Via};
    use super::*;

    fn h(id: &str, proveedor: Proveedor, clase: Clase, ip: &str, manejo: Manejo) -> Hallazgo {
        Hallazgo {
            id: id.into(),
            via: Via::Wifi,
            proveedor,
            clase,
            forma: None,
            marca: String::new(),
            nombre: id.into(),
            modelo: None,
            direccion: Some(ip.into()),
            senal: None,
            version: None,
            capacidades: Capacidades::ninguna(),
            estado: None,
            manejo,
        }
    }

    #[test]
    fn lan_junta_sin_repetir() {
        let pronto = || Manejo::Pronto { motivo: "x".into() };
        let listas = vec![
            vec![h("wiz:a", Proveedor::Wiz, Clase::Luz, "192.168.1.5", Manejo::Listo)],
            vec![
                // El mismo WiZ anunciando HomeKit: no se repite.
                h("mdns:hap", Proveedor::Ninguno, Clase::Luz, "192.168.1.5", pronto()),
                // Una tele por mDNS y por SSDP: queda una sola, la tele.
                h("mdns:cast", Proveedor::Ninguno, Clase::Otro, "192.168.1.9", pronto()),
            ],
            vec![h("ssdp:tv", Proveedor::Ninguno, Clase::Tv, "192.168.1.9", pronto())],
            vec![h("tuya:x", Proveedor::Tuya, Clase::Otro, "192.168.1.7", Manejo::CuentaTuya)],
            vec![h("wiz:a", Proveedor::Wiz, Clase::Luz, "192.168.1.5", Manejo::Listo)],
        ];
        let r = juntar(listas);
        let ids: Vec<&str> = r.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["wiz:a", "tuya:x", "ssdp:tv"]);
    }

    #[test]
    fn lan_solo_ips_de_la_red_local() {
        assert_eq!(ip_de("192.168.1.40"), Ok(Ipv4Addr::new(192, 168, 1, 40)));
        assert_eq!(ip_de("10.0.0.2:55443"), Ok(Ipv4Addr::new(10, 0, 0, 2)));
        assert!(ip_de("8.8.8.8").is_err());
        assert!(ip_de("localhost").is_err());
        assert!(es_local(IpAddr::V4(Ipv4Addr::new(172, 16, 4, 1))));
        assert!(!es_local(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }

    #[test]
    fn lan_abre_puertos_compartidos() {
        // Dos sockets en el mismo puerto: en la Mac se comparte; en lo demás, el segundo falla sin romper nada.
        let a = abrir_udp(46_999);
        let b = abrir_udp(46_999);
        assert!(a.is_ok());
        if cfg!(target_os = "macos") {
            assert!(b.is_ok());
        }
    }

    /// Una búsqueda de verdad en la red de este equipo (no corre sola):
    /// `cargo test --lib lan_busca_en_vivo -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn lan_busca_en_vivo() {
        let inicio = Instant::now();
        let lista = buscar(Duration::from_secs(6));
        let tardo = inicio.elapsed();
        for x in &lista {
            println!("{:?} · {} · {} · {:?} · {:?} · {:?}", x.proveedor, x.nombre, x.marca, x.direccion, x.clase, x.manejo);
        }
        println!("{} hallazgos en {tardo:?}", lista.len());
        assert!(tardo < Duration::from_millis(6600));
    }
}
