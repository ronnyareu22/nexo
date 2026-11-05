//! Red local (Wi-Fi): buscar y manejar sin nube.
//!
//! - `buscar_todo`: una búsqueda corta de todas las familias a la vez, cada una en su hilo. Al terminar no queda
//!   ningún socket ni hilo vivo. Lo que falle (sin red, puerto ocupado, cortafuegos de Windows) se deja afuera.
//!   - Tuya (avisos en 6666/6667/7000): solo se detecta; funciona con su nube.
//!   - WiZ (UDP 38899), Yeelight (búsqueda en 239.255.255.250:1982, órdenes por TCP 55443), Govee (búsqueda en
//!     239.255.255.250:4001, respuestas en 4002, órdenes en 4003) y LIFX (UDP 56700): se manejan aquí mismo.
//!   - mDNS: Hue y Nanoleaf (se vinculan con su botón), Elgato, WLED y Shelly (se prueban por HTTP), Home
//!     Assistant (con su dirección, para vincularlo) y lo que solo se ve (teles, HomeKit, Matter, ESPHome).
//!   - Tasmota y OpenBeken no se anuncian: se prueba `Status 2` por HTTP con lo que la tabla de vecinos dice que
//!     tiene chip de aparato inteligente (o se llama `tasmota-…`/`obk…`). Antes, `red::despertar` llena esa tabla.
//!   - SSDP (Roku, Samsung, LG, DIAL): solo se detectan, con el motivo en palabras.
//! - `leer` y `cambiar`: lo agregado, con plazos cortos (≈1,5 s), por su IP (`Acceso`).

mod aes;
pub mod elgato;
mod govee;
pub mod hue;
pub mod lifx;
mod lifx_productos;
pub mod mdns;
pub mod nanoleaf;
pub mod shelly;
mod ssdp;
pub mod tasmota;
mod tuya_udp;
pub mod web;
mod wiz;
pub mod wled;
mod yeelight;

use super::modelo::{Cambio, Clase, EstadoAparato, FormaLuz, FormaVinculo, Hallazgo, Manejo, Proveedor, Via};
use super::red;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// Plazo de una orden o una lectura.
const PLAZO_ORDEN: Duration = Duration::from_millis(1500);
/// mDNS escucha hasta aquí; lo que queda es para probar por HTTP lo encontrado.
const CORTE_MDNS: Duration = Duration::from_millis(2600);
/// Como mucho tantos aparatos se prueban por HTTP a la vez.
const MAX_SONDEOS: usize = 32;

/// Una familia: busca hasta el instante dado y devuelve lo que encontró.
type Familia = fn(Instant) -> Vec<Hallazgo>;

/// Lo que dejó una búsqueda: lo encontrado y, para "Tu red", la red, los vecinos y lo que dijo mDNS de cada IP.
#[derive(Debug, Default)]
pub struct Busqueda {
    pub hallazgos: Vec<Hallazgo>,
    pub pistas: Vec<(Ipv4Addr, Vec<String>)>,
    pub vecinos: Vec<(Ipv4Addr, String)>,
    pub red: red::Red,
}

/// Busca en la red local durante `plazo`.
pub fn buscar(plazo: Duration) -> Vec<Hallazgo> {
    buscar_todo(plazo).hallazgos
}

/// Busca todo durante `plazo` (al menos ≈3 s para que alcance a probar por HTTP).
pub fn buscar_todo(plazo: Duration) -> Busqueda {
    let inicio = Instant::now();
    let fin = inicio + plazo;
    let familias: [(&str, Familia); 6] = [
        ("tuya", tuya_udp::buscar),
        ("wiz", wiz::buscar),
        ("yeelight", yeelight::buscar),
        ("govee", govee::buscar),
        ("ssdp", ssdp::buscar),
        ("lifx", lifx::buscar),
    ];
    let hilos: Vec<_> = familias
        .into_iter()
        .filter_map(|(nombre, f)| std::thread::Builder::new().name(format!("nexo-lan-{nombre}")).spawn(move || f(fin)).ok())
        .collect();
    // En este hilo: la red, mDNS y las pruebas por HTTP.
    let r = red::red();
    red::despertar(&r);
    let instancias = mdns::buscar((inicio + CORTE_MDNS).min(fin));
    let vecinos = red::vecinos();
    let propia = ip_propia();
    let mut listas = vec![mdns::hallazgos(&instancias, propia), sondear(&instancias, &vecinos, propia, fin)];
    // Cada familia vuelve sola al llegar a `fin` (sus sockets esperan de a poco): aquí solo se juntan.
    listas.extend(hilos.into_iter().filter_map(|h| h.join().ok()));
    let mut hallazgos = juntar(listas);
    for h in hallazgos.iter_mut().filter(|h| h.mac.is_none() && h.via == Via::Wifi) {
        let ip = h.direccion.as_deref().and_then(|d| ip_de(d).ok());
        h.mac = ip.and_then(|ip| vecinos.iter().find(|(v, _)| *v == ip)).map(|(_, m)| m.clone());
    }
    let pistas = instancias.iter().filter_map(|i| Some((i.ip?, mdns::pistas(i)))).collect();
    Busqueda { hallazgos, pistas, vecinos, red: r }
}

/// El nombre de una instancia en minúsculas.
fn etiqueta(i: &mdns::Instancia) -> String {
    i.etiqueta().to_ascii_lowercase()
}

/// Un Nanoleaf visto por mDNS: se vincula con su botón. Su id es su MAC (por la tabla de vecinos) o el del TXT.
fn nanoleaf_hallazgo(i: &mdns::Instancia, ip: Ipv4Addr, mac: Option<String>) -> Hallazgo {
    let id = mac.as_deref().map(|m| m.replace(':', "")).or_else(|| i.txt("id").map(|t| t.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase()));
    let puerto = i.puerto.filter(|p| *p != nanoleaf::PUERTO);
    Hallazgo {
        id: format!("nanoleaf:{}", id.unwrap_or_else(|| ip.to_string().replace('.', "-"))),
        via: Via::Wifi,
        proveedor: Proveedor::Nanoleaf,
        clase: Clase::Luz,
        forma: Some(FormaLuz::Lampara),
        marca: "Nanoleaf".into(),
        nombre: i.etiqueta().chars().filter(|c| !c.is_control()).take(40).collect::<String>().trim().to_string(),
        modelo: i.txt("md").map(str::to_string),
        direccion: Some(match puerto {
            Some(p) => format!("{ip}:{p}"),
            None => ip.to_string(),
        }),
        mac,
        senal: None,
        version: i.txt("srcvers").map(str::to_string),
        capacidades: nanoleaf::capacidades(),
        estado: None,
        manejo: Manejo::Vincular {
            forma: FormaVinculo::Boton,
            paso: "Mantén presionado el botón de encendido de 5 a 7 segundos, hasta que las luces parpadeen.".into(),
        },
    }
}

/// Lo que se prueba por HTTP (Shelly, Elgato, WLED, Tasmota y OpenBeken) y lo que se arma sin probar (Hue,
/// Nanoleaf), en paralelo y con plazo.
fn sondear(instancias: &[mdns::Instancia], vecinos: &[(Ipv4Addr, String)], propia: Option<IpAddr>, fin: Instant) -> Vec<Hallazgo> {
    enum Prueba {
        Shelly(Ipv4Addr),
        Elgato(Ipv4Addr, u16, Option<String>),
        Wled(Ipv4Addr),
        Tasmota(Ipv4Addr),
    }
    let mac_de = |ip: Ipv4Addr| vecinos.iter().find(|(v, _)| *v == ip).map(|(_, m)| m.clone());
    let mut listos: Vec<Hallazgo> = Vec::new();
    let mut pruebas: Vec<Prueba> = Vec::new();
    let mut vistas: Vec<Ipv4Addr> = Vec::new();
    for i in instancias {
        let Some(ip) = i.ip.filter(|ip| Some(IpAddr::V4(*ip)) != propia) else { continue };
        let e = etiqueta(i);
        let prueba = match i.corto() {
            "hue" => {
                listos.push(hue::hallazgo(ip, i.txt("bridgeid"), i.txt("modelid")));
                None
            }
            "nanoleafapi" => {
                listos.push(nanoleaf_hallazgo(i, ip, mac_de(ip)));
                None
            }
            "shelly" => Some(Prueba::Shelly(ip)),
            "elg" => Some(Prueba::Elgato(ip, i.puerto.unwrap_or(elgato::PUERTO), i.txt("id").map(str::to_string))),
            "wled" => Some(Prueba::Wled(ip)),
            "http" if e.starts_with("shelly") => Some(Prueba::Shelly(ip)),
            "http" if e.starts_with("wled") => Some(Prueba::Wled(ip)),
            "http" if e.starts_with("tasmota") || e.starts_with("obk") || e.starts_with("openbk") => Some(Prueba::Tasmota(ip)),
            _ => None,
        };
        if let Some(p) = prueba
            && !vistas.contains(&ip)
        {
            vistas.push(ip);
            pruebas.push(p);
        }
    }
    // Lo que tiene chip de aparato inteligente y no se anunció: quizá Tasmota u OpenBeken.
    for (ip, mac) in vecinos {
        if vistas.contains(ip) || Some(IpAddr::V4(*ip)) == propia || red::privada(mac) {
            continue;
        }
        if red::marca(mac).is_some_and(|(_, pista)| pista == "iot") {
            vistas.push(*ip);
            pruebas.push(Prueba::Tasmota(*ip));
        }
    }
    pruebas.truncate(MAX_SONDEOS);
    let queda = fin.saturating_duration_since(Instant::now()).max(Duration::from_millis(600));
    let plazo = queda.min(Duration::from_millis(1500));
    let probados: Vec<Hallazgo> = std::thread::scope(|s| {
        let hilos: Vec<_> = pruebas
            .iter()
            .map(|p| {
                s.spawn(move || -> Option<Hallazgo> {
                    match p {
                        Prueba::Shelly(ip) => shelly::probar(*ip).map(|i| shelly::hallazgo(*ip, &i)),
                        Prueba::Elgato(ip, puerto, id) => elgato::probar(*ip, *puerto, id.as_deref()),
                        Prueba::Wled(ip) => wled::probar(*ip),
                        Prueba::Tasmota(ip) => match tasmota::sondear(*ip, plazo) {
                            tasmota::Sondeo::Hallado(f, version) => {
                                let (mac, nombre, estado) = tasmota::detalle(*ip, None).ok()?;
                                let mac = mac.or_else(|| mac_de(*ip));
                                Some(tasmota::hallazgo(*ip, f, Some(version), mac, nombre, Some(&estado), false))
                            }
                            tasmota::Sondeo::Clave(f) => Some(tasmota::hallazgo(*ip, f, None, mac_de(*ip), None, None, true)),
                            tasmota::Sondeo::Nada => None,
                        },
                    }
                })
            })
            .collect();
        hilos.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
    });
    listos.extend(probados);
    listos
}

/// Lo que solo se ve (todavía sin control).
fn solo_se_ve(h: &Hallazgo) -> bool {
    matches!(h.proveedor, Proveedor::Ninguno) || matches!(h.manejo, Manejo::Pronto { .. })
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
        Manejo::Vincular { .. } => 1,
        Manejo::CuentaTuya | Manejo::HomeAssistant { .. } => 2,
        Manejo::Activar { .. } => 3,
        Manejo::Pronto { .. } => 4,
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

/// Cómo llegar a un aparato agregado (lo guardado en Nexo y su clave, si tiene).
#[derive(Debug, Clone, Copy, Default)]
pub struct Acceso<'a> {
    pub id: &'a str,
    pub direccion: &'a str,
    pub modelo: Option<&'a str>,
    /// La salida o la luz: "3" en un puente Hue, "switch:1" en un Shelly, "2" en un Tasmota.
    pub canal: Option<&'a str>,
    pub clave: Option<&'a str>,
    pub kelvin: Option<(u32, u32)>,
    pub puerto: Option<u16>,
    /// Es una luz (Tasmota: lee `Dimmer` y lo demás) y tiene color (WLED: el blanco en los tres canales).
    pub luz: bool,
    pub color: bool,
    /// Hue: la huella del certificado del puente.
    pub huella: Option<[u8; 32]>,
}

fn sin_clave() -> String {
    "Falta la clave de este aparato: quítalo y vincúlalo de nuevo.".into()
}

/// Cómo está un aparato local.
pub fn leer(proveedor: Proveedor, a: &Acceso) -> Result<EstadoAparato, String> {
    let ip = || ip_de(a.direccion);
    match proveedor {
        Proveedor::Wiz => wiz::leer(&ip()?, a.modelo),
        Proveedor::Yeelight => yeelight::leer(a.direccion, a.modelo),
        Proveedor::Govee => govee::leer(&ip()?),
        Proveedor::Lifx => {
            let mac = lifx::mac_de_id(a.id).ok_or("No sé la MAC de esta luz LIFX.")?;
            lifx::leer_luz(ip()?, mac, a.kelvin.or_else(|| lifx::rango_de_modelo(a.modelo)))
        }
        Proveedor::Shelly => shelly::leer(ip()?, a.canal.unwrap_or("switch:0"), a.clave, a.kelvin),
        Proveedor::Nanoleaf => {
            nanoleaf::leer(ip()?, a.puerto.unwrap_or(nanoleaf::PUERTO), a.clave.ok_or_else(sin_clave)?, a.kelvin.unwrap_or(nanoleaf::KELVIN))
        }
        Proveedor::Elgato => elgato::leer(ip()?),
        Proveedor::Wled => wled::leer(ip()?),
        Proveedor::Tasmota => tasmota::leer(ip()?, a.canal.unwrap_or(""), a.luz, a.clave),
        Proveedor::Hue => {
            let p = hue::Puente::nuevo(ip()?, a.modelo, a.huella);
            let (luces, _) = hue::leer_todo(&p, a.clave.ok_or_else(sin_clave)?)?;
            let l = luces.into_iter().find(|l| Some(l.numero.as_str()) == a.canal).ok_or("El puente Hue ya no tiene esa luz.")?;
            if l.alcanzable { Ok(l.estado) } else { Err("La luz no responde.".into()) }
        }
        _ => Err("Nexo todavía no lee este aparato por la red local.".into()),
    }
}

/// Manda un cambio a un aparato local.
pub fn cambiar(proveedor: Proveedor, a: &Acceso, cambio: &Cambio) -> Result<(), String> {
    if cambio.vacio() {
        return Ok(());
    }
    let ip = || ip_de(a.direccion);
    match proveedor {
        Proveedor::Wiz => wiz::cambiar(&ip()?, a.modelo, cambio),
        Proveedor::Yeelight => yeelight::cambiar(a.direccion, a.modelo, cambio),
        Proveedor::Govee => govee::cambiar(&ip()?, cambio),
        Proveedor::Lifx => {
            let mac = lifx::mac_de_id(a.id).ok_or("No sé la MAC de esta luz LIFX.")?;
            lifx::cambiar(ip()?, mac, a.kelvin.or_else(|| lifx::rango_de_modelo(a.modelo)), cambio)
        }
        Proveedor::Shelly => shelly::cambiar(ip()?, a.canal.unwrap_or("switch:0"), a.modelo.unwrap_or(""), a.clave, a.kelvin, cambio),
        Proveedor::Nanoleaf => nanoleaf::cambiar(
            ip()?,
            a.puerto.unwrap_or(nanoleaf::PUERTO),
            a.clave.ok_or_else(sin_clave)?,
            a.kelvin.unwrap_or(nanoleaf::KELVIN),
            cambio,
        ),
        Proveedor::Elgato => elgato::cambiar(ip()?, cambio),
        Proveedor::Wled => wled::cambiar(ip()?, a.color, cambio),
        Proveedor::Tasmota => tasmota::cambiar(ip()?, a.canal.unwrap_or(""), a.clave, cambio),
        Proveedor::Hue => {
            let p = hue::Puente::nuevo(ip()?, a.modelo, a.huella);
            hue::cambiar(&p, a.clave.ok_or_else(sin_clave)?, a.canal.unwrap_or("1"), a.kelvin, cambio)
        }
        _ => Err("Nexo todavía no maneja este aparato por la red local.".into()),
    }
}

// ---------------------------------------------------------------- Lo común

/// Solo IPv4 de la red local (privadas o de enlace local): nunca se manda nada a internet.
pub fn ip_de(direccion: &str) -> Result<Ipv4Addr, String> {
    let texto = direccion.rsplit_once(':').map_or(direccion, |(ip, _)| ip);
    match texto.trim().parse::<Ipv4Addr>() {
        Ok(ip) if ip.is_private() || ip.is_link_local() => Ok(ip),
        _ => Err(format!("«{direccion}» no es una dirección de tu red local.")),
    }
}

/// El error de siempre cuando algo local no contesta.
fn no_responde(ip: &str) -> String {
    format!("No responde en {ip}. Revisa que esté encendido y en la misma red.")
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
pub fn ip_propia() -> Option<IpAddr> {
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
            mac: None,
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
