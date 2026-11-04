//! Procesos y conexiones, leídos con libproc: sin abrir `lsof` ni otro programa (unos pocos ms cada 5 s).
//!
//! RustDesk recibe la sesión en su proceso `RustDesk --server`: directa por el puerto 21118 (cuando el dueño
//! escribe la IP de Tailscale de la Mac) o por el relé, que es una conexión saliente al puerto 21117.
//! Solo se ven los procesos del usuario (el servicio de RustDesk corre como root y no recibe la sesión).

use std::ffi::{c_int, c_void};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const PUERTO_DIRECTO: u16 = 21118;
const PUERTO_RELE: u16 = 21117;

// `struct socket_fdinfo` de <sys/proc_info.h>: tamaño y posiciones medidos con el encabezado del sistema.
// Si un macOS futuro cambiara el tamaño, la lectura no coincide y la conexión simplemente no se cuenta.
const TAM_SOCKET: usize = 792;
const PROC_PIDFDSOCKETINFO: c_int = 3;
const SOI_KIND: usize = 256; // psi.soi_kind
const SOCKINFO_TCP: i32 = 2;
const PUERTO_REMOTO: usize = 264; // insi_fport (en orden de red)
const PUERTO_LOCAL: usize = 268; // insi_lport
const VFLAG: usize = 288; // insi_vflag
const DIRECCION: usize = 296; // insi_faddr (16 bytes)
const DIRECCION_V4: usize = 308; // insi_faddr.ina_46.i46a_addr4
const ESTADO_TCP: usize = 344; // tcpsi_state
const ESTABLECIDA: i32 = 4; // TSI_S_ESTABLISHED
const INI_IPV4: u8 = 1;

pub struct Conexion {
    /// El otro lado: el PC (directa) o el servidor de RustDesk (relé).
    pub ip: IpAddr,
    pub rele: bool,
}

#[derive(Default)]
pub struct Lectura {
    pub conexion: Option<Conexion>,
    /// Hay algún proceso de RustDesk.app abierto.
    pub rustdesk: bool,
    /// La app aparte "Captura remota" está abierta.
    pub captura_remota: bool,
}

/// Una vuelta por los procesos: RustDesk, su sesión (si hay) y "Captura remota".
pub fn leer() -> Lectura {
    let mut l = Lectura::default();
    for pid in pids() {
        let Some(ruta) = ruta(pid) else { continue };
        if ruta.contains("/Captura remota.app/") {
            l.captura_remota = true;
            continue;
        }
        if !ruta.contains("/RustDesk.app/") {
            continue;
        }
        l.rustdesk = true;
        for (local, remoto, ip) in conexiones(pid) {
            if local == PUERTO_DIRECTO {
                // La directa gana siempre.
                l.conexion = Some(Conexion { ip, rele: false });
            } else if remoto == PUERTO_RELE && l.conexion.is_none() && es_servidor(pid) {
                l.conexion = Some(Conexion { ip, rele: true });
            }
        }
    }
    l
}

/// Si ese proceso es de RustDesk.app (lo pregunta la escucha de teclas por cada proceso nuevo que inyecta teclas).
pub fn es_rustdesk(pid: i32) -> bool {
    ruta(pid).is_some_and(|r| r.contains("/RustDesk.app/"))
}

fn pids() -> Vec<i32> {
    // SAFETY: con un búfer nulo, libproc solo dice cuántos hay.
    let n = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if n <= 0 {
        return Vec::new();
    }
    let capacidad = n as usize + 64;
    let mut lista = vec![0i32; capacidad];
    let bytes = (capacidad * std::mem::size_of::<i32>()) as c_int;
    // SAFETY: el búfer tiene `bytes` bytes.
    let m = unsafe { libc::proc_listallpids(lista.as_mut_ptr() as *mut c_void, bytes) };
    if m <= 0 {
        return Vec::new();
    }
    lista.truncate((m as usize).min(capacidad));
    lista.retain(|p| *p > 0);
    lista
}

/// La ruta del programa de un proceso.
fn ruta(pid: i32) -> Option<String> {
    let mut buf = [0u8; 4096]; // PROC_PIDPATHINFO_MAXSIZE
    // SAFETY: el búfer mide lo que se dice.
    let largo = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr() as *mut c_void, buf.len() as u32) };
    if largo <= 0 {
        return None;
    }
    Some(String::from_utf8_lossy(buf.get(..largo as usize)?).into_owned())
}

/// Las conexiones TCP establecidas de un proceso: (puerto local, puerto remoto, dirección remota).
fn conexiones(pid: i32) -> Vec<(u16, u16, IpAddr)> {
    let mut salida = Vec::new();
    // SAFETY: con búfer nulo devuelve cuántos bytes hacen falta.
    let tam = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
    if tam <= 0 {
        return salida; // no es nuestro (root) o no tiene archivos abiertos
    }
    let paso = std::mem::size_of::<libc::proc_fdinfo>();
    let capacidad = tam as usize / paso + 16;
    let mut fds = vec![libc::proc_fdinfo { proc_fd: 0, proc_fdtype: 0 }; capacidad];
    // SAFETY: el búfer tiene `capacidad` entradas.
    let usados = unsafe {
        libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, fds.as_mut_ptr() as *mut c_void, (capacidad * paso) as c_int)
    };
    if usados <= 0 {
        return salida;
    }
    fds.truncate((usados as usize / paso).min(capacidad));

    let mut info = [0u8; TAM_SOCKET];
    for fd in fds.iter().filter(|f| f.proc_fdtype == libc::PROX_FDTYPE_SOCKET as u32) {
        // SAFETY: el búfer mide justo lo que pide PROC_PIDFDSOCKETINFO.
        let r = unsafe {
            libc::proc_pidfdinfo(
                pid,
                fd.proc_fd,
                PROC_PIDFDSOCKETINFO,
                info.as_mut_ptr() as *mut c_void,
                TAM_SOCKET as c_int,
            )
        };
        if r as usize != TAM_SOCKET || entero(&info, SOI_KIND) != SOCKINFO_TCP || entero(&info, ESTADO_TCP) != ESTABLECIDA
        {
            continue;
        }
        let puerto = |i: usize| u16::from_be(entero(&info, i) as u16);
        let Some(ip) = direccion(&info) else { continue };
        salida.push((puerto(PUERTO_LOCAL), puerto(PUERTO_REMOTO), ip));
    }
    salida
}

fn entero(b: &[u8], i: usize) -> i32 {
    b.get(i..i + 4).and_then(|s| s.try_into().ok()).map(i32::from_ne_bytes).unwrap_or(0)
}

/// La dirección del otro lado. Las IPv4 que llegan por un socket IPv6 (::ffff:a.b.c.d) se devuelven como IPv4.
fn direccion(b: &[u8]) -> Option<IpAddr> {
    let vflag = *b.get(VFLAG)?;
    if vflag & INI_IPV4 != 0 {
        let o: [u8; 4] = b.get(DIRECCION_V4..DIRECCION_V4 + 4)?.try_into().ok()?;
        return Some(IpAddr::V4(Ipv4Addr::from(o)));
    }
    let o: [u8; 16] = b.get(DIRECCION..DIRECCION + 16)?.try_into().ok()?;
    let v6 = Ipv6Addr::from(o);
    Some(v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(IpAddr::V6(v6)))
}

/// Si el proceso es `RustDesk --server` (el que recibe sesiones). Si no se pueden leer sus argumentos, se asume que sí.
fn es_servidor(pid: i32) -> bool {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut tam: usize = 0;
    // SAFETY: primero se pregunta el tamaño (búfer nulo) y después se lee en un búfer de ese tamaño.
    let r = unsafe { libc::sysctl(mib.as_mut_ptr(), 3, std::ptr::null_mut(), &mut tam, std::ptr::null_mut(), 0) };
    if r != 0 || tam == 0 || tam > 1 << 20 {
        return true;
    }
    let mut buf = vec![0u8; tam];
    let r = unsafe {
        libc::sysctl(mib.as_mut_ptr(), 3, buf.as_mut_ptr() as *mut c_void, &mut tam, std::ptr::null_mut(), 0)
    };
    if r != 0 {
        return true;
    }
    buf.truncate(tam);
    // Formato: argc (4 bytes), la ruta del programa, ceros de relleno y después los argumentos separados por ceros.
    buf.get(4..).is_some_and(|resto| resto.split(|b| *b == 0).any(|arg| arg == b"--server"))
}
