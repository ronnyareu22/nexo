//! Nexo Drop: el receptor. Un HTTP/1.1 mínimo sobre `TcpListener` (sin dependencias), un pedido por conexión.
//!
//! - Escucha solo en la IPv4 de Tailscale de este equipo, puerto 47821. Nunca en 0.0.0.0 ni en la red local.
//!   Si Tailscale no está conectado, vuelve a probar cada 30 s. Se apaga con "Recibir archivos".
//! - Seguridad: antes de leer nada, mira quién llama. Solo atiende a equipos de Tailscale del mismo dueño que
//!   este (el `UserID` de `Self` y el del par con esa dirección) y a este mismo equipo. Al resto, 403.
//! - Cada conexión va en su propio hilo (16 a la vez como mucho) y se corta si pasa 30 s sin datos.
//! - Rutas: `GET /nexo/hola`, `PUT /nexo/archivo?nombre=&tamano=&lote=&indice=&total=&desde=`,
//!   `POST /nexo/texto` `{texto}` y `POST /nexo/enlace` `{url}`. Mi setup: `GET /nexo/aparatos` (ver sistema/setup.rs).
//!   Actualizaciones: `GET /nexo/actualizacion?plataforma=windows` y `/nexo/actualizacion/archivo` (sistema/actualizar.rs).
//! - Un archivo llega a un temporal oculto en Descargas › Nexo Drop y, completo, pasa a su nombre final
//!   (con sus carpetas y "(2)" si ya existe). Si se corta, el temporal se borra.

use super::historial::{self, Entrada};
use super::progreso::{Medidor, Progreso};
use super::{PUERTO, rutas, tailscale};
use serde_json::json;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

const MAX_CABEZA: usize = 16 * 1024;
const MAX_CONEXIONES: usize = 16;
const MAX_TEXTO: u64 = 4 * 1024 * 1024;
const MAX_ENLACE: u64 = 16 * 1024;
const SIN_DATOS: Duration = Duration::from_secs(30);
const REINTENTO: Duration = Duration::from_secs(30);
/// Con el receptor andando, cada cuánto se revisa que la dirección de Tailscale siga igual.
const REVISION: Duration = Duration::from_secs(300);

static ACTIVO: AtomicBool = AtomicBool::new(false);
/// Cada escucha nueva sube la generación: la anterior, al despertar, ve que ya no le toca y se cierra.
static GENERACION: AtomicU64 = AtomicU64::new(0);
static ESCUCHA: Mutex<Option<(Ipv4Addr, u64)>> = Mutex::new(None);
static DESPERTAR: Mutex<bool> = Mutex::new(false);
static SENAL: Condvar = Condvar::new();
static CONEXIONES: AtomicUsize = AtomicUsize::new(0);
static RELECTURA: Mutex<Option<Instant>> = Mutex::new(None);
static LOTES: Mutex<Vec<LoteEntrante>> = Mutex::new(Vec::new());
/// Elegir el nombre final y mover el archivo, de a uno (así dos archivos iguales no se pisan).
static COLOCANDO: Mutex<()> = Mutex::new(());

/// Un error que se contesta al otro lado: código HTTP y motivo en palabras.
struct Fallo(u16, String);

fn fallo(codigo: u16, motivo: &str) -> Fallo {
    Fallo(codigo, motivo.to_string())
}

// ---------------------------------------------------------------- Encender, apagar y vigilar

pub fn iniciar(app: AppHandle, activo: bool) {
    ACTIVO.store(activo, Ordering::Relaxed);
    if let Err(e) = std::thread::Builder::new().name("nexo-drop".into()).spawn(move || vigilar(app)) {
        eprintln!("Nexo Drop: no se pudo iniciar el receptor ({e})");
    }
}

/// Enciende o apaga el receptor. Al encender, intenta escuchar enseguida.
pub fn usar(app: &AppHandle, activo: bool) {
    ACTIVO.store(activo, Ordering::Relaxed);
    if activo {
        if let Some(ip) = ip_propia() {
            asegurar(app, ip);
        }
    } else {
        detener();
    }
    avisar_vigia();
}

/// La dirección donde escucha ahora.
pub fn escuchando() -> Option<Ipv4Addr> {
    ESCUCHA.lock().unwrap_or_else(|e| e.into_inner()).map(|(ip, _)| ip)
}

/// Se supo la dirección de Tailscale de este equipo (al buscar destinos): si cambió, se escucha en la nueva.
pub fn al_conocer_ip(app: &AppHandle, ip: Ipv4Addr) {
    if ACTIVO.load(Ordering::Relaxed) && escuchando() != Some(ip) {
        asegurar(app, ip);
    }
}

fn ip_propia() -> Option<Ipv4Addr> {
    tailscale::leer(Duration::from_secs(25)).propio.and_then(|p| p.ipv4())
}

/// Mientras el receptor esté encendido, cuida que haya una escucha en la dirección de Tailscale.
fn vigilar(app: AppHandle) {
    loop {
        if !ACTIVO.load(Ordering::Relaxed) {
            detener();
            esperar(None);
            continue;
        }
        let espera = match ip_propia() {
            Some(ip) if asegurar(&app, ip) => REVISION,
            _ => REINTENTO,
        };
        esperar(Some(espera));
    }
}

/// Duerme hasta que alguien avise (o hasta el límite). No gasta nada mientras espera.
fn esperar(limite: Option<Duration>) {
    let mut avisado = DESPERTAR.lock().unwrap_or_else(|e| e.into_inner());
    let fin = limite.map(|l| Instant::now() + l);
    while !*avisado {
        match fin {
            None => avisado = SENAL.wait(avisado).unwrap_or_else(|e| e.into_inner()),
            Some(fin) => {
                let resto = fin.saturating_duration_since(Instant::now());
                if resto.is_zero() {
                    break;
                }
                avisado = match SENAL.wait_timeout(avisado, resto) {
                    Ok((g, _)) => g,
                    Err(e) => e.into_inner().0,
                };
            }
        }
    }
    *avisado = false;
}

fn avisar_vigia() {
    *DESPERTAR.lock().unwrap_or_else(|e| e.into_inner()) = true;
    SENAL.notify_all();
}

/// Deja una escucha en esa dirección (si ya la hay, no hace nada). Devuelve si quedó escuchando.
fn asegurar(app: &AppHandle, ip: Ipv4Addr) -> bool {
    let mut escucha = ESCUCHA.lock().unwrap_or_else(|e| e.into_inner());
    if escucha.is_some_and(|(actual, _)| actual == ip) {
        return true;
    }
    let oyente = match TcpListener::bind(SocketAddr::from((ip, PUERTO))) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Nexo Drop: no se pudo escuchar en {ip}:{PUERTO} ({e})");
            return false;
        }
    };
    let generacion = GENERACION.fetch_add(1, Ordering::Relaxed) + 1;
    let anterior = escucha.replace((ip, generacion));
    drop(escucha);
    if let Some((vieja, _)) = anterior {
        despertar_escucha(vieja);
    }
    let a = app.clone();
    let hilo = std::thread::Builder::new().name("nexo-drop-escucha".into()).spawn(move || aceptar(a, oyente, ip, generacion));
    if let Err(e) = hilo {
        eprintln!("Nexo Drop: no se pudo escuchar ({e})");
        let mut escucha = ESCUCHA.lock().unwrap_or_else(|e| e.into_inner());
        if *escucha == Some((ip, generacion)) {
            *escucha = None;
        }
        return false;
    }
    true
}

fn detener() {
    let anterior = ESCUCHA.lock().unwrap_or_else(|e| e.into_inner()).take();
    GENERACION.fetch_add(1, Ordering::Relaxed);
    if let Some((ip, _)) = anterior {
        despertar_escucha(ip);
    }
}

/// La escucha está dormida en `accept`: una conexión propia la despierta para que vea que debe cerrarse.
fn despertar_escucha(ip: Ipv4Addr) {
    let _ = std::thread::Builder::new().name("nexo-drop-cierre".into()).spawn(move || {
        let _ = TcpStream::connect_timeout(&SocketAddr::from((ip, PUERTO)), Duration::from_secs(1));
    });
}

fn aceptar(app: AppHandle, oyente: TcpListener, ip: Ipv4Addr, generacion: u64) {
    let mut errores = 0u32;
    for conexion in oyente.incoming() {
        if GENERACION.load(Ordering::Relaxed) != generacion || !ACTIVO.load(Ordering::Relaxed) {
            break;
        }
        match conexion {
            Ok(s) => {
                errores = 0;
                atender_aparte(&app, s);
            }
            Err(_) => {
                // Muchos errores seguidos (la red se cayó): se suelta y el vigía vuelve a probar.
                errores += 1;
                if errores >= 50 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
    let mut escucha = ESCUCHA.lock().unwrap_or_else(|e| e.into_inner());
    if *escucha == Some((ip, generacion)) {
        *escucha = None;
        drop(escucha);
        avisar_vigia();
    }
}

/// Resta una conexión al terminar, pase lo que pase.
struct Turno;

impl Drop for Turno {
    fn drop(&mut self) {
        CONEXIONES.fetch_sub(1, Ordering::Relaxed);
    }
}

fn atender_aparte(app: &AppHandle, mut s: TcpStream) {
    if CONEXIONES.fetch_add(1, Ordering::Relaxed) >= MAX_CONEXIONES {
        CONEXIONES.fetch_sub(1, Ordering::Relaxed);
        let _ = responder_error(&mut s, 503, "Hay demasiadas conexiones; prueba en un momento.");
        return;
    }
    let turno = Turno;
    let a = app.clone();
    // Si el hilo no se crea, el cierre se descarta (y con él la conexión y el turno).
    let _ = std::thread::Builder::new().name("nexo-drop-pedido".into()).spawn(move || {
        let _turno = turno;
        atender(&a, s);
    });
}

// ---------------------------------------------------------------- Seguridad

/// Solo este equipo o un equipo de Tailscale del mismo dueño.
fn permitido(ip: Ipv4Addr) -> bool {
    let texto = ip.to_string();
    if let Some(si) = tailscale::leer(Duration::from_secs(60)).es_del_dueno(&texto) {
        return si;
    }
    // Una dirección que no se conoce: se le pregunta de nuevo a Tailscale, como mucho una vez cada 5 s.
    {
        let mut ultima = RELECTURA.lock().unwrap_or_else(|e| e.into_inner());
        if ultima.is_some_and(|t| t.elapsed() < Duration::from_secs(5)) {
            return false;
        }
        *ultima = Some(Instant::now());
    }
    tailscale::leer(Duration::ZERO).es_del_dueno(&texto).unwrap_or(false)
}

// ---------------------------------------------------------------- HTTP

struct Pedido {
    metodo: String,
    ruta: String,
    consulta: HashMap<String, String>,
    largo: Option<u64>,
    por_trozos: bool,
    pide_continuar: bool,
}

impl Pedido {
    fn leer(cabeza: &[u8]) -> Option<Pedido> {
        let texto = std::str::from_utf8(cabeza).ok()?;
        let mut lineas = texto.split("\r\n");
        let mut primera = lineas.next()?.split(' ');
        let metodo = primera.next()?.to_string();
        let destino = primera.next()?;
        if !primera.next()?.starts_with("HTTP/1.") {
            return None;
        }
        let (ruta, consulta) = destino.split_once('?').unwrap_or((destino, ""));
        let mut pedido = Pedido {
            metodo,
            ruta: ruta.to_string(),
            consulta: rutas::consulta(consulta),
            largo: None,
            por_trozos: false,
            pide_continuar: false,
        };
        for linea in lineas.filter(|l| !l.is_empty()) {
            let (clave, valor) = linea.split_once(':')?;
            let valor = valor.trim();
            match clave.trim().to_ascii_lowercase().as_str() {
                "content-length" => pedido.largo = Some(valor.parse().ok()?),
                "transfer-encoding" => pedido.por_trozos = !valor.eq_ignore_ascii_case("identity"),
                "expect" => pedido.pide_continuar = valor.eq_ignore_ascii_case("100-continue"),
                _ => {}
            }
        }
        Some(pedido)
    }
}

/// Lee el encabezado (hasta la línea en blanco). Devuelve el encabezado y lo que ya llegó del cuerpo.
fn leer_cabeza(s: &mut TcpStream) -> Result<(Vec<u8>, Vec<u8>), Fallo> {
    let mut datos = Vec::with_capacity(2048);
    let mut trozo = [0u8; 4096];
    loop {
        let n = s.read(&mut trozo).map_err(|_| fallo(408, "Se agotó la espera."))?;
        if n == 0 {
            return Err(fallo(400, "La conexión se cerró antes de tiempo."));
        }
        let desde = datos.len().saturating_sub(3);
        datos.extend_from_slice(&trozo[..n]);
        if let Some(i) = datos[desde..].windows(4).position(|v| v == b"\r\n\r\n") {
            let fin = desde + i;
            let resto = datos.split_off(fin + 4);
            datos.truncate(fin);
            return Ok((datos, resto));
        }
        if datos.len() > MAX_CABEZA {
            return Err(fallo(431, "El encabezado es demasiado grande."));
        }
    }
}

fn razon(codigo: u16) -> &'static str {
    match codigo {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        507 => "Insufficient Storage",
        _ => "Internal Server Error",
    }
}

fn responder(s: &mut TcpStream, codigo: u16, cuerpo: &serde_json::Value) -> io::Result<()> {
    let texto = cuerpo.to_string();
    let cabeza = format!(
        "HTTP/1.1 {codigo} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        razon(codigo),
        texto.len()
    );
    s.write_all(cabeza.as_bytes())?;
    s.write_all(texto.as_bytes())?;
    s.flush()?;
    let _ = s.shutdown(Shutdown::Write);
    Ok(())
}

fn responder_error(s: &mut TcpStream, codigo: u16, motivo: &str) -> io::Result<()> {
    responder(s, codigo, &json!({ "ok": false, "error": motivo }))
}

/// curl y otros piden permiso antes de mandar un cuerpo grande ("Expect: 100-continue").
fn continuar(s: &mut TcpStream, p: &Pedido) -> Result<(), Fallo> {
    if p.pide_continuar {
        s.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").map_err(|_| fallo(400, "Se cortó la conexión."))?;
    }
    Ok(())
}

/// Lee un cuerpo pequeño (texto o enlace) de largo conocido.
fn leer_cuerpo(s: &mut TcpStream, p: &Pedido, mut ya: Vec<u8>, max: u64) -> Result<Vec<u8>, Fallo> {
    if p.por_trozos {
        return Err(fallo(411, "Falta Content-Length."));
    }
    let largo = p.largo.ok_or_else(|| fallo(411, "Falta Content-Length."))?;
    if largo > max {
        return Err(fallo(413, "Es demasiado grande."));
    }
    continuar(s, p)?;
    let largo = largo as usize;
    ya.truncate(largo);
    let mut falta = vec![0u8; largo - ya.len()];
    s.read_exact(&mut falta).map_err(|_| fallo(408, "Se cortó la conexión."))?;
    ya.extend_from_slice(&falta);
    Ok(ya)
}

fn atender(app: &AppHandle, mut s: TcpStream) {
    let _ = s.set_read_timeout(Some(SIN_DATOS));
    let _ = s.set_write_timeout(Some(SIN_DATOS));
    let ip = match s.peer_addr().map(|a| a.ip()) {
        Ok(IpAddr::V4(v4)) => Some(v4),
        Ok(IpAddr::V6(v6)) => v6.to_ipv4_mapped(),
        Err(_) => None,
    };
    let Some(ip) = ip.filter(|ip| permitido(*ip)) else {
        let _ = responder_error(&mut s, 403, "Solo tus equipos en Tailscale pueden usar este Nexo.");
        return;
    };
    let resultado = leer_cabeza(&mut s).and_then(|(cabeza, resto)| {
        let p = Pedido::leer(&cabeza).ok_or_else(|| fallo(400, "No entendí el pedido."))?;
        match (p.metodo.as_str(), p.ruta.as_str()) {
            ("GET", "/nexo/hola") => hola(&mut s),
            ("PUT", "/nexo/archivo") => archivo(app, &mut s, &p, resto, ip),
            ("POST", "/nexo/texto") => texto(app, &mut s, &p, resto, ip),
            ("POST", "/nexo/enlace") => enlace(app, &mut s, &p, resto, ip),
            ("GET", "/nexo/aparatos") => {
                let _ = responder(&mut s, 200, &crate::sistema::setup::aparatos_json(app));
                Ok(())
            }
            // Actualizaciones para Windows (ver sistema/actualizar.rs).
            ("GET", "/nexo/actualizacion") => {
                crate::sistema::actualizar::servir_info(app, &mut s, &p.consulta).map_err(|(c, m)| Fallo(c, m))
            }
            ("GET", "/nexo/actualizacion/archivo") => {
                crate::sistema::actualizar::servir_archivo(app, &mut s, &p.consulta).map_err(|(c, m)| Fallo(c, m))
            }
            (
                _,
                "/nexo/hola" | "/nexo/archivo" | "/nexo/texto" | "/nexo/enlace" | "/nexo/aparatos" | "/nexo/actualizacion"
                | "/nexo/actualizacion/archivo",
            ) => {
                Err(fallo(405, "Método no permitido."))
            }
            _ => Err(fallo(404, "No existe.")),
        }
    });
    if let Err(Fallo(codigo, motivo)) = resultado {
        let _ = responder_error(&mut s, codigo, &motivo);
    }
}

/// Quién manda: su nombre en Tailscale o, si no se sabe, el que dijo.
fn equipo_de(ip: Ipv4Addr, dijo: Option<&String>) -> String {
    super::nombre_de(ip)
        .or_else(|| dijo.map(|d| d.chars().filter(|c| !c.is_control()).take(64).collect::<String>()).filter(|d| !d.trim().is_empty()))
        .unwrap_or_else(|| ip.to_string())
}

// ---------------------------------------------------------------- Rutas

fn hola(s: &mut TcpStream) -> Result<(), Fallo> {
    let cuerpo = json!({
        "nombre": super::plataforma::nombre_equipo(),
        "plataforma": super::PLATAFORMA,
        "version": env!("CARGO_PKG_VERSION"),
    });
    let _ = responder(s, 200, &cuerpo);
    Ok(())
}

fn texto(app: &AppHandle, s: &mut TcpStream, p: &Pedido, resto: Vec<u8>, ip: Ipv4Addr) -> Result<(), Fallo> {
    let cuerpo = leer_cuerpo(s, p, resto, MAX_TEXTO)?;
    let v: serde_json::Value = serde_json::from_slice(&cuerpo).map_err(|_| fallo(400, "Se esperaba {\"texto\": \"…\"}."))?;
    let texto = v.get("texto").and_then(|t| t.as_str()).filter(|t| !t.is_empty()).ok_or_else(|| fallo(400, "Falta el texto."))?;
    let oculto = v.get("oculto").and_then(|o| o.as_bool()).unwrap_or(false);
    let equipo = equipo_de(ip, None);
    let copiado = super::texto_recibido(app, &equipo, texto, oculto);
    let _ = responder(s, 200, &json!({ "ok": true, "copiado": copiado }));
    Ok(())
}

fn enlace(app: &AppHandle, s: &mut TcpStream, p: &Pedido, resto: Vec<u8>, ip: Ipv4Addr) -> Result<(), Fallo> {
    let cuerpo = leer_cuerpo(s, p, resto, MAX_ENLACE)?;
    let v: serde_json::Value = serde_json::from_slice(&cuerpo).map_err(|_| fallo(400, "Se esperaba {\"url\": \"…\"}."))?;
    let url = v
        .get("url")
        .and_then(|u| u.as_str())
        .and_then(super::enlace_valido)
        .ok_or_else(|| fallo(400, "Solo se abren enlaces http o https."))?;
    let equipo = equipo_de(ip, None);
    super::enlace_recibido(app, &equipo, &url).map_err(|e| Fallo(500, e))?;
    let _ = responder(s, 200, &json!({ "ok": true }));
    Ok(())
}

fn unico() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

fn archivo(app: &AppHandle, s: &mut TcpStream, p: &Pedido, resto: Vec<u8>, ip: Ipv4Addr) -> Result<(), Fallo> {
    let nombre = p.consulta.get("nombre").ok_or_else(|| fallo(400, "Falta el nombre."))?;
    let partes = rutas::partes_seguras(nombre).ok_or_else(|| fallo(400, "Ese nombre no sirve."))?;
    let tamano: u64 = p.consulta.get("tamano").and_then(|t| t.parse().ok()).ok_or_else(|| fallo(400, "Falta el tamaño."))?;
    if p.por_trozos {
        return Err(fallo(411, "Falta Content-Length."));
    }
    if p.largo != Some(tamano) {
        return Err(fallo(400, "El tamaño no coincide con lo que llega."));
    }
    let total: u32 = p.consulta.get("total").and_then(|t| t.parse().ok()).unwrap_or(1).clamp(1, 1_000_000);
    let indice: u32 = p.consulta.get("indice").and_then(|t| t.parse().ok()).unwrap_or(0);
    if indice >= total {
        return Err(fallo(400, "El índice no corresponde al total."));
    }
    let lote = p.consulta.get("lote").and_then(|l| rutas::lote_seguro(l)).unwrap_or_else(|| format!("suelto-{}", unico()));
    let equipo = equipo_de(ip, p.consulta.get("desde"));

    let carpeta = super::carpeta(app).ok_or_else(|| fallo(500, "No encontré la carpeta Descargas."))?;
    std::fs::create_dir_all(&carpeta).map_err(|e| Fallo(500, format!("No pude crear la carpeta XDrop ({e}).")))?;
    let temporal = carpeta.join(format!(".nexo-{lote}-{indice}-{}.parte", unico()));
    let mut f = File::create(&temporal).map_err(|e| Fallo(500, format!("No pude guardar el archivo ({e}).")))?;

    let mut medidor = Medidor::nuevo(Progreso {
        lote: lote.clone(),
        direccion: "recibido",
        equipo: equipo.clone(),
        indice,
        total,
        nombre: partes.join("/"),
        tamano,
        estado: "transfiriendo",
        ..Default::default()
    });
    medidor.emitir(app);

    let copia = continuar(s, p).and_then(|_| copiar_cuerpo(app, s, resto, &mut f, tamano, &mut medidor));
    drop(f);
    if let Err(e) = copia {
        let _ = std::fs::remove_file(&temporal);
        medidor.terminar(app, "error", Some(e.1.clone()));
        return Err(e);
    }
    let destino = colocar(&carpeta, &lote, ip, &partes, &temporal).map_err(|e| {
        let _ = std::fs::remove_file(&temporal);
        medidor.terminar(app, "error", Some(e.clone()));
        Fallo(500, e)
    })?;
    let relativa = destino.strip_prefix(&carpeta).unwrap_or(&destino).to_string_lossy().replace('\\', "/");
    let _ = responder(s, 200, &json!({ "ok": true, "nombre": relativa }));
    medidor.terminar(app, if indice + 1 >= total { "listo" } else { "transfiriendo" }, None);
    anotar(app, &lote, ip, &equipo, total, tamano, &carpeta, &destino);
    Ok(())
}

/// Pasa el cuerpo al archivo: lo que ya llegó con el encabezado y luego el resto, hasta `tamano` bytes.
fn copiar_cuerpo(
    app: &AppHandle,
    s: &mut TcpStream,
    ya: Vec<u8>,
    f: &mut File,
    tamano: u64,
    medidor: &mut Medidor,
) -> Result<(), Fallo> {
    let escribir_mal = |e: io::Error| Fallo(507, format!("No pude escribir el archivo ({e})."));
    let primero = &ya[..ya.len().min(tamano as usize)];
    f.write_all(primero).map_err(escribir_mal)?;
    medidor.sumar(app, primero.len() as u64);
    let mut faltan = tamano - primero.len() as u64;
    let mut trozo = vec![0u8; 256 * 1024];
    while faltan > 0 {
        let quiero = (trozo.len() as u64).min(faltan) as usize;
        let n = match s.read(&mut trozo[..quiero]) {
            Ok(0) => return Err(fallo(400, "Se cortó el envío antes de terminar.")),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(fallo(408, "Se cortó el envío (30 s sin datos).")),
        };
        f.write_all(&trozo[..n]).map_err(escribir_mal)?;
        faltan -= n as u64;
        medidor.sumar(app, n as u64);
    }
    f.flush().map_err(escribir_mal)
}

// ---------------------------------------------------------------- Lotes que llegan

/// Un envío de varios archivos que va llegando: sus carpetas, cuántos llegaron y su entrada del historial.
struct LoteEntrante {
    lote: String,
    ip: Ipv4Addr,
    /// Cada carpeta de arriba: (como venía, como quedó) — "Fotos" puede quedar "Fotos (2)".
    carpetas: Vec<(String, String)>,
    /// Lo de arriba que llegó: (nombre final, es carpeta).
    arriba: Vec<(String, bool)>,
    primero: Option<PathBuf>,
    recibidos: u32,
    bytes: u64,
    historial: Option<u64>,
    ultimo_aviso: Instant,
    actividad: Instant,
}

fn bloquear_lotes() -> std::sync::MutexGuard<'static, Vec<LoteEntrante>> {
    LOTES.lock().unwrap_or_else(|e| e.into_inner())
}

/// El lote (lo crea si es nuevo). Olvida los que llevan una hora quietos.
fn lote_de<'a>(lotes: &'a mut Vec<LoteEntrante>, lote: &str, ip: Ipv4Addr) -> &'a mut LoteEntrante {
    lotes.retain(|l| l.actividad.elapsed() < Duration::from_secs(3600));
    let i = match lotes.iter().position(|l| l.lote == lote && l.ip == ip) {
        Some(i) => i,
        None => {
            lotes.push(LoteEntrante {
                lote: lote.to_string(),
                ip,
                carpetas: Vec::new(),
                arriba: Vec::new(),
                primero: None,
                recibidos: 0,
                bytes: 0,
                historial: None,
                ultimo_aviso: Instant::now(),
                actividad: Instant::now(),
            });
            lotes.len() - 1
        }
    };
    let l = &mut lotes[i];
    l.actividad = Instant::now();
    l
}

/// Mueve el temporal a su lugar final, con sus carpetas. La carpeta de arriba de cada lote es nueva:
/// si ya existía una igual, este lote usa "Fotos (2)".
fn colocar(carpeta: &Path, lote: &str, ip: Ipv4Addr, partes: &[String], temporal: &Path) -> Result<PathBuf, String> {
    let _turno = COLOCANDO.lock().unwrap_or_else(|e| e.into_inner());
    let (nombre, carpetas) = partes.split_last().ok_or("Falta el nombre.")?;
    let mut dir = carpeta.to_path_buf();
    if let Some((primera, resto)) = carpetas.split_first() {
        let real = {
            let mut lotes = bloquear_lotes();
            let l = lote_de(&mut lotes, lote, ip);
            match l.carpetas.iter().find(|(o, _)| o == primera) {
                Some((_, r)) => r.clone(),
                None => {
                    let nueva = rutas::ruta_libre(carpeta, primera, true);
                    std::fs::create_dir_all(&nueva).map_err(|e| format!("No pude crear la carpeta ({e})."))?;
                    let real = nueva.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| primera.clone());
                    l.carpetas.push((primera.clone(), real.clone()));
                    real
                }
            }
        };
        dir = dir.join(real);
        for c in resto {
            dir = dir.join(c);
        }
        std::fs::create_dir_all(&dir).map_err(|e| format!("No pude crear la carpeta ({e})."))?;
    }
    let destino = rutas::ruta_libre(&dir, nombre, false);
    std::fs::rename(temporal, &destino).map_err(|e| format!("No pude guardar el archivo ({e})."))?;
    Ok(destino)
}

/// Anota el archivo recibido en su lote y en el historial. Al completarse el lote, avisa.
#[allow(clippy::too_many_arguments)]
fn anotar(app: &AppHandle, lote: &str, ip: Ipv4Addr, equipo: &str, total: u32, tamano: u64, carpeta: &Path, destino: &Path) {
    let (entrada, completo, avisar) = {
        let mut lotes = bloquear_lotes();
        let l = lote_de(&mut lotes, lote, ip);
        l.recibidos += 1;
        l.bytes += tamano;
        l.primero.get_or_insert_with(|| destino.to_path_buf());
        // Lo de arriba: el archivo suelto o la carpeta del lote donde quedó.
        let relativa = destino.strip_prefix(carpeta).unwrap_or(destino);
        let mut componentes = relativa.components();
        let arriba = componentes.next().map(|c| c.as_os_str().to_string_lossy().into_owned()).unwrap_or_default();
        let es_carpeta = componentes.next().is_some();
        if !l.arriba.iter().any(|(n, _)| *n == arriba) {
            l.arriba.push((arriba, es_carpeta));
        }

        let (tipo, nombre, ruta) = match l.arriba.as_slice() {
            [(n, false)] if l.recibidos == 1 => ("archivo", n.clone(), destino.to_path_buf()),
            [(n, true)] => ("carpeta", n.clone(), carpeta.join(n)),
            [(n, _), ..] => ("varios", n.clone(), l.primero.clone().unwrap_or_else(|| destino.to_path_buf())),
            [] => ("varios", String::new(), destino.to_path_buf()),
        };
        let completo = l.recibidos >= total;
        let estado = if completo { "completo" } else { "en curso" };
        let entrada = Entrada {
            direccion: "recibido".into(),
            tipo: tipo.into(),
            nombre,
            cantidad: l.recibidos,
            total,
            tamano: l.bytes,
            equipo: equipo.into(),
            ruta: Some(ruta.display().to_string()),
            estado: estado.into(),
            lote: Some(lote.into()),
            ..Default::default()
        };
        let id = match l.historial {
            Some(id) => {
                let nueva = entrada.clone();
                historial::cambiar(id, completo, move |e| {
                    let (id, momento) = (e.id, e.momento);
                    *e = Entrada { id, momento, ..nueva };
                });
                id
            }
            None => {
                let id = historial::agregar(entrada, true);
                l.historial = Some(id);
                id
            }
        };
        let avisar = completo || l.recibidos == 1 || l.ultimo_aviso.elapsed() >= Duration::from_secs(1);
        if avisar {
            l.ultimo_aviso = Instant::now();
        }
        if completo {
            lotes.retain(|x| !(x.lote == lote && x.ip == ip));
        }
        (historial::buscar(id), completo, avisar)
    };
    let Some(entrada) = entrada else { return };
    if avisar {
        super::avisar_recibido(app, &entrada);
    }
    if completo {
        super::lote_recibido(app, &entrada);
    }
}
