//! Nexo Drop: el envío y la búsqueda de destinos.
//!
//! - Los lotes van en cola y salen de a uno, en un hilo aparte. Cada archivo es un `PUT` con su largo, leído
//!   en trozos de 256 KB: el lector cuenta los bytes (avance) y mira si se canceló. Cancelar también cierra la
//!   conexión, así no queda esperando una escritura.
//! - Las carpetas se recorren enteras (sin seguir accesos directos) y se mandan con su ruta relativa.
//! - Destinos: los equipos del mismo dueño en Tailscale (Mac o Windows) que responden `GET /nexo/hola`.
//!   La respuesta de cada uno se recuerda 8 s.

use super::historial::{self, Entrada};
use super::progreso::{Medidor, Progreso};
use super::{Destino, PUERTO, rutas, servidor, tailscale};
use serde_json::json;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

/// Más archivos que esto en un solo envío, se pide mandarlos por partes.
const MAX_ARCHIVOS: usize = 20_000;
const TROZO: usize = 256 * 1024;
const CANCELADO: &str = "Cancelado";

struct Pedido {
    lote: String,
    rutas: Vec<PathBuf>,
    ip: Ipv4Addr,
    equipo: String,
    /// Lo que se borra al terminar (la imagen copiada que se guardó para mandarla).
    temporales: Vec<PathBuf>,
}

/// El envío en curso: su lote, la marca de cancelado y la conexión abierta (para cortarla al cancelar).
struct Actual {
    lote: String,
    cancelado: Arc<AtomicBool>,
    conexion: Option<TcpStream>,
}

static COLA: Mutex<VecDeque<Pedido>> = Mutex::new(VecDeque::new());
static TRABAJANDO: AtomicBool = AtomicBool::new(false);
static ACTUAL: Mutex<Option<Actual>> = Mutex::new(None);
static CONTADOR: AtomicU64 = AtomicU64::new(1);
static SONDEOS: Mutex<Vec<(Ipv4Addr, Instant, Option<Hola>)>> = Mutex::new(Vec::new());

/// Un identificador de lote nuevo ("1730650000123-4").
pub fn nuevo_lote() -> String {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("{ms}-{}", CONTADOR.fetch_add(1, Ordering::Relaxed))
}

fn bloquear<T>(m: &'static Mutex<T>) -> std::sync::MutexGuard<'static, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn progreso_inicial(lote: &str, equipo: &str, estado: &'static str) -> Progreso {
    Progreso { lote: lote.into(), direccion: "enviado", equipo: equipo.into(), estado, ..Default::default() }
}

/// Pone un envío en la cola y devuelve su lote. Si no hay otro en curso, sale enseguida.
pub fn encolar(app: &AppHandle, rutas: Vec<PathBuf>, ip: Ipv4Addr, equipo: String, temporales: Vec<PathBuf>) -> String {
    let lote = nuevo_lote();
    Medidor::nuevo(progreso_inicial(&lote, &equipo, "esperando")).emitir(app);
    let mut cola = bloquear(&COLA);
    cola.push_back(Pedido { lote: lote.clone(), rutas, ip, equipo, temporales });
    if !TRABAJANDO.swap(true, Ordering::Relaxed) {
        let a = app.clone();
        if let Err(e) = std::thread::Builder::new().name("nexo-drop-envio".into()).spawn(move || trabajar(a)) {
            TRABAJANDO.store(false, Ordering::Relaxed);
            if let Some(p) = cola.pop_back() {
                drop(cola);
                Medidor::nuevo(progreso_inicial(&p.lote, &p.equipo, "error")).terminar(app, "error", Some(e.to_string()));
                borrar_temporales(&p.temporales);
            }
        }
    }
    lote
}

/// Cancela un envío: si está en la cola, sale de ella; si está en curso, se corta.
pub fn cancelar(app: &AppHandle, lote: &str) {
    let en_cola = {
        let mut cola = bloquear(&COLA);
        cola.iter().position(|p| p.lote == lote).and_then(|i| cola.remove(i))
    };
    if let Some(p) = en_cola {
        Medidor::nuevo(progreso_inicial(&p.lote, &p.equipo, "cancelado")).terminar(app, "cancelado", None);
        borrar_temporales(&p.temporales);
        return;
    }
    if let Some(a) = bloquear(&ACTUAL).as_ref().filter(|a| a.lote == lote) {
        a.cancelado.store(true, Ordering::Relaxed);
        if let Some(c) = &a.conexion {
            let _ = c.shutdown(Shutdown::Both);
        }
    }
}

fn trabajar(app: AppHandle) {
    loop {
        let siguiente = {
            let mut cola = bloquear(&COLA);
            let p = cola.pop_front();
            if p.is_none() {
                TRABAJANDO.store(false, Ordering::Relaxed);
            }
            p
        };
        let Some(pedido) = siguiente else { return };
        enviar_lote(&app, pedido);
    }
}

fn borrar_temporales(temporales: &[PathBuf]) {
    for t in temporales {
        let _ = if t.is_dir() { std::fs::remove_dir_all(t) } else { std::fs::remove_file(t) };
    }
}

// ---------------------------------------------------------------- Un lote

struct Archivo {
    ruta: PathBuf,
    /// "Fotos/viaje/a.jpg", con "/".
    relativa: String,
    tamano: u64,
}

fn nombre_de(ruta: &Path) -> String {
    ruta.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| ruta.display().to_string())
}

/// Lo que hay que mandar: cada archivo y, de cada carpeta, todo lo de adentro.
fn expandir(rutas: &[PathBuf]) -> Result<Vec<Archivo>, String> {
    let mut lista = Vec::new();
    for r in rutas {
        let nombre = nombre_de(r);
        let meta = std::fs::metadata(r).map_err(|_| format!("No encontré «{nombre}»."))?;
        if meta.is_dir() {
            recorrer(r, &nombre, &mut lista)?;
        } else if meta.is_file() {
            lista.push(Archivo { ruta: r.clone(), relativa: nombre, tamano: meta.len() });
        }
        if lista.len() > MAX_ARCHIVOS {
            return Err("Son más de 20 000 archivos: mándalos por partes.".into());
        }
    }
    Ok(lista)
}

fn recorrer(carpeta: &Path, prefijo: &str, lista: &mut Vec<Archivo>) -> Result<(), String> {
    let leer = std::fs::read_dir(carpeta).map_err(|e| format!("No pude leer «{prefijo}» ({e})."))?;
    let mut entradas: Vec<_> = leer.filter_map(Result::ok).collect();
    entradas.sort_by_key(|e| e.file_name());
    for e in entradas {
        let nombre = e.file_name().to_string_lossy().into_owned();
        // Lo que cada sistema deja en las carpetas no hace falta del otro lado.
        if matches!(nombre.as_str(), ".DS_Store" | ".localized" | "Thumbs.db" | "desktop.ini") {
            continue;
        }
        let Ok(tipo) = e.file_type() else { continue };
        let relativa = format!("{prefijo}/{nombre}");
        if tipo.is_dir() {
            recorrer(&e.path(), &relativa, lista)?;
        } else if tipo.is_file() {
            let tamano = e.metadata().map(|m| m.len()).unwrap_or(0);
            lista.push(Archivo { ruta: e.path(), relativa, tamano });
        }
        if lista.len() > MAX_ARCHIVOS {
            return Err("Son más de 20 000 archivos: mándalos por partes.".into());
        }
    }
    Ok(())
}

fn enviar_lote(app: &AppHandle, p: Pedido) {
    let cancelado = Arc::new(AtomicBool::new(false));
    *bloquear(&ACTUAL) = Some(Actual { lote: p.lote.clone(), cancelado: cancelado.clone(), conexion: None });
    let mut medidor = Medidor::nuevo(progreso_inicial(&p.lote, &p.equipo, "transfiriendo"));
    medidor.emitir(app);

    let mut enviados = 0u32;
    let mut total = 0u32;
    let resultado = (|| -> Result<(), String> {
        let archivos = expandir(&p.rutas)?;
        if archivos.is_empty() {
            return Err("No hay archivos para enviar: las carpetas están vacías.".into());
        }
        total = archivos.len() as u32;
        medidor.p.total = total;
        medidor.p.tamano_lote = archivos.iter().map(|a| a.tamano).sum();
        let desde = super::nombre_propio();
        for (i, a) in archivos.iter().enumerate() {
            if cancelado.load(Ordering::Relaxed) {
                return Err(CANCELADO.into());
            }
            medidor.p.indice = i as u32;
            medidor.p.nombre = a.relativa.clone();
            medidor.p.enviados = 0;
            medidor.p.tamano = a.tamano;
            medidor.emitir(app);
            enviar_archivo(app, &p, a, i as u32, total, &desde, &cancelado, &mut medidor)?;
            enviados += 1;
        }
        Ok(())
    })();
    *bloquear(&ACTUAL) = None;

    let (estado, error) = match resultado {
        Ok(()) => ("listo", None),
        Err(_) if cancelado.load(Ordering::Relaxed) => ("cancelado", None),
        Err(e) => ("error", Some(e)),
    };
    // Al historial: un solo elemento con su nombre (y su ruta, para mostrarlo), o varios.
    let solo = (p.rutas.len() == 1).then(|| &p.rutas[0]);
    let tipo = match solo {
        Some(r) if r.is_dir() => "carpeta",
        Some(_) => "archivo",
        None => "varios",
    };
    let entrada = Entrada {
        direccion: "enviado".into(),
        tipo: tipo.into(),
        nombre: p.rutas.first().map(|r| nombre_de(r)).unwrap_or_default(),
        cantidad: enviados,
        total,
        tamano: medidor.p.enviados_lote,
        equipo: p.equipo.clone(),
        ruta: solo.filter(|_| p.temporales.is_empty()).map(|r| r.display().to_string()),
        estado: match estado {
            "listo" => "completo",
            otro => otro,
        }
        .into(),
        error: error.clone(),
        lote: Some(p.lote.clone()),
        ..Default::default()
    };
    historial::agregar(entrada, true);
    medidor.terminar(app, estado, error);
    borrar_temporales(&p.temporales);
}

/// Lee el archivo contando los bytes (el avance) y se detiene si se canceló.
struct Contador<'a, R: Read> {
    dentro: R,
    cancelado: &'a AtomicBool,
    medidor: &'a mut Medidor,
    app: &'a AppHandle,
}

impl<R: Read> Read for Contador<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancelado.load(Ordering::Relaxed) {
            return Err(io::Error::other(CANCELADO));
        }
        let n = self.dentro.read(buf)?;
        self.medidor.sumar(self.app, n as u64);
        Ok(n)
    }
}

#[allow(clippy::too_many_arguments)]
fn enviar_archivo(
    app: &AppHandle,
    p: &Pedido,
    a: &Archivo,
    indice: u32,
    total: u32,
    desde: &str,
    cancelado: &AtomicBool,
    medidor: &mut Medidor,
) -> Result<(), String> {
    let f = File::open(&a.ruta).map_err(|e| format!("No pude abrir «{}» ({e}).", a.relativa))?;
    // El tamaño de ahora (pudo cambiar desde que se recorrió la carpeta).
    let tamano = f.metadata().map(|m| m.len()).unwrap_or(a.tamano);
    medidor.p.tamano_lote = medidor.p.tamano_lote.saturating_sub(a.tamano) + tamano;
    medidor.p.tamano = tamano;

    let mut s = conectar(p.ip, Duration::from_secs(5))
        .map_err(|_| format!("No pude conectar con {}. Revisa que esté encendida y con Nexo abierto.", p.equipo))?;
    let _ = s.set_write_timeout(Some(Duration::from_secs(30)));
    let _ = s.set_read_timeout(Some(Duration::from_secs(60)));
    if let Some(actual) = bloquear(&ACTUAL).as_mut() {
        actual.conexion = s.try_clone().ok();
    }

    let ruta = format!(
        "/nexo/archivo?nombre={}&tamano={tamano}&lote={}&indice={indice}&total={total}&desde={}",
        rutas::codificar(&a.relativa),
        p.lote,
        rutas::codificar(desde)
    );
    let cabeza = format!(
        "PUT {ruta} HTTP/1.1\r\nHost: {}:{PUERTO}\r\nUser-Agent: Nexo/{}\r\nContent-Type: application/octet-stream\r\n\
         Content-Length: {tamano}\r\nConnection: close\r\n\r\n",
        p.ip,
        env!("CARGO_PKG_VERSION")
    );
    let envio = (|| -> io::Result<()> {
        s.write_all(cabeza.as_bytes())?;
        let mut lector = Contador { dentro: f.take(tamano), cancelado, medidor: &mut *medidor, app };
        let mut trozo = vec![0u8; TROZO];
        let mut mandados = 0u64;
        loop {
            let n = lector.read(&mut trozo)?;
            if n == 0 {
                break;
            }
            s.write_all(&trozo[..n])?;
            mandados += n as u64;
        }
        if mandados < tamano {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "el archivo se achicó mientras se enviaba"));
        }
        s.flush()
    })();
    if cancelado.load(Ordering::Relaxed) {
        return Err(CANCELADO.into());
    }
    // La respuesta se lee también si falló la escritura: el otro lado pudo rechazarlo antes (y dice por qué).
    let respuesta = leer_respuesta(&mut s);
    if let Some(actual) = bloquear(&ACTUAL).as_mut() {
        actual.conexion = None;
    }
    match (envio, respuesta) {
        (Ok(()), Ok((200, _))) => Ok(()),
        (_, Ok((codigo, cuerpo))) if codigo != 200 => Err(motivo(&cuerpo).unwrap_or(format!("{} no lo aceptó ({codigo}).", p.equipo))),
        (Err(e), _) => Err(format!("Se cortó el envío a {} ({e}).", p.equipo)),
        (Ok(()), Err(e)) => Err(e),
        (Ok(()), Ok(_)) => Err(format!("{} no confirmó el archivo.", p.equipo)),
    }
}

// ---------------------------------------------------------------- HTTP del lado que manda

fn conectar(ip: Ipv4Addr, limite: Duration) -> io::Result<TcpStream> {
    TcpStream::connect_timeout(&SocketAddr::from((ip, PUERTO)), limite)
}

/// Lee la respuesta completa (el otro lado cierra al terminar). Devuelve el código y el JSON.
fn leer_respuesta(s: &mut TcpStream) -> Result<(u16, serde_json::Value), String> {
    let mut datos = Vec::new();
    let lectura = Read::by_ref(s).take(64 * 1024).read_to_end(&mut datos);
    let mut texto = String::from_utf8_lossy(&datos).into_owned();
    // "100 Continue" no es la respuesta: se salta.
    while texto.starts_with("HTTP/1.1 100") || texto.starts_with("HTTP/1.0 100") {
        match texto.split_once("\r\n\r\n") {
            Some((_, resto)) => texto = resto.to_string(),
            None => break,
        }
    }
    let Some((cabeza, cuerpo)) = texto.split_once("\r\n\r\n") else {
        return Err(match lectura {
            Err(e) => format!("No respondió ({e})."),
            Ok(_) => "No respondió.".into(),
        });
    };
    let codigo = cabeza
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .ok_or("La respuesta no se entiende.")?;
    Ok((codigo, serde_json::from_str(cuerpo).unwrap_or(serde_json::Value::Null)))
}

fn motivo(cuerpo: &serde_json::Value) -> Option<String> {
    cuerpo.get("error").and_then(|e| e.as_str()).map(str::to_string)
}

/// Un POST con JSON (texto o enlace).
fn enviar_json(ip: Ipv4Addr, equipo: &str, ruta: &str, cuerpo: &serde_json::Value) -> Result<serde_json::Value, String> {
    let texto = cuerpo.to_string();
    let mut s = conectar(ip, Duration::from_secs(5))
        .map_err(|_| format!("No pude conectar con {equipo}. Revisa que esté encendida y con Nexo abierto."))?;
    let _ = s.set_write_timeout(Some(Duration::from_secs(20)));
    let _ = s.set_read_timeout(Some(Duration::from_secs(20)));
    let pedido = format!(
        "POST {ruta} HTTP/1.1\r\nHost: {ip}:{PUERTO}\r\nUser-Agent: Nexo/{}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{texto}",
        env!("CARGO_PKG_VERSION"),
        texto.len()
    );
    s.write_all(pedido.as_bytes()).map_err(|e| format!("Se cortó el envío a {equipo} ({e})."))?;
    match leer_respuesta(&mut s)? {
        (200, v) => Ok(v),
        (codigo, v) => Err(motivo(&v).unwrap_or(format!("{equipo} no lo aceptó ({codigo})."))),
    }
}

/// Manda un texto al portapapeles del otro equipo y lo anota en el historial.
pub fn enviar_texto(app: &AppHandle, ip: Ipv4Addr, equipo: &str, texto: &str, oculto: bool) -> Result<(), String> {
    let r = enviar_json(ip, equipo, "/nexo/texto", &json!({ "texto": texto, "oculto": oculto }));
    let e = Entrada {
        direccion: "enviado".into(),
        tipo: "texto".into(),
        nombre: "Texto".into(),
        cantidad: 1,
        total: 1,
        tamano: texto.len() as u64,
        equipo: equipo.into(),
        texto: (!oculto).then(|| texto.chars().take(historial::MAX_TEXTO).collect()),
        estado: if r.is_ok() { "completo" } else { "error" }.into(),
        error: r.as_ref().err().cloned(),
        ..Default::default()
    };
    historial::agregar(e, true);
    let _ = app.emit("drop-enviado", ());
    r.map(|_| ())
}

/// Abre un enlace en el navegador del otro equipo y lo anota en el historial.
pub fn enviar_enlace(app: &AppHandle, ip: Ipv4Addr, equipo: &str, url: &str) -> Result<(), String> {
    let r = enviar_json(ip, equipo, "/nexo/enlace", &json!({ "url": url }));
    let e = Entrada {
        direccion: "enviado".into(),
        tipo: "enlace".into(),
        nombre: "Enlace".into(),
        cantidad: 1,
        total: 1,
        tamano: url.len() as u64,
        equipo: equipo.into(),
        texto: Some(url.into()),
        estado: if r.is_ok() { "completo" } else { "error" }.into(),
        error: r.as_ref().err().cloned(),
        ..Default::default()
    };
    historial::agregar(e, true);
    let _ = app.emit("drop-enviado", ());
    r.map(|_| ())
}

// ---------------------------------------------------------------- Destinos

/// Lo que responde otro Nexo en `GET /nexo/hola`.
#[derive(Clone, Debug)]
struct Hola {
    plataforma: &'static str,
    version: Option<String>,
}

fn plataforma_de(texto: &str) -> &'static str {
    match texto.to_ascii_lowercase().as_str() {
        "mac" | "macos" => "mac",
        "windows" => "windows",
        _ => "otro",
    }
}

/// Le pregunta a ese equipo si tiene Nexo (la respuesta se recuerda 8 s).
fn sondear(ip: Ipv4Addr) -> Option<Hola> {
    if let Some((_, _, h)) = bloquear(&SONDEOS).iter().find(|(i, t, _)| *i == ip && t.elapsed() < Duration::from_secs(8)) {
        return h.clone();
    }
    let respuesta = (|| -> Option<Hola> {
        let mut s = conectar(ip, Duration::from_millis(1500)).ok()?;
        let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
        let _ = s.set_write_timeout(Some(Duration::from_secs(3)));
        s.write_all(format!("GET /nexo/hola HTTP/1.1\r\nHost: {ip}:{PUERTO}\r\nConnection: close\r\n\r\n").as_bytes()).ok()?;
        let (codigo, v) = leer_respuesta(&mut s).ok()?;
        if codigo != 200 {
            return None;
        }
        Some(Hola {
            plataforma: plataforma_de(v.get("plataforma").and_then(|p| p.as_str()).unwrap_or_default()),
            version: v.get("version").and_then(|p| p.as_str()).map(str::to_string),
        })
    })();
    let mut sondeos = bloquear(&SONDEOS);
    sondeos.retain(|(i, t, _)| *i != ip && t.elapsed() < Duration::from_secs(60));
    sondeos.push((ip, Instant::now(), respuesta.clone()));
    respuesta
}

/// Los otros equipos del dueño (Mac o Windows) y si Nexo responde en cada uno. Primero los que responden.
pub fn destinos(app: &AppHandle) -> Vec<Destino> {
    let info = tailscale::leer(Duration::from_secs(15));
    if let Some(ip) = info.propio.as_ref().and_then(|p| p.ipv4()) {
        servidor::al_conocer_ip(app, ip);
    }
    let pares: Vec<(tailscale::Nodo, Ipv4Addr)> = info
        .pares_del_dueno()
        .into_iter()
        .filter(|n| matches!(n.sistema.to_ascii_lowercase().as_str(), "macos" | "windows" | ""))
        .filter_map(|n| n.ipv4().map(|ip| (n, ip)))
        .collect();
    // Se pregunta a todos a la vez; a los que Tailscale ve apagados, no.
    let respuestas: Vec<Option<Hola>> = std::thread::scope(|alcance| {
        let hilos: Vec<_> = pares
            .iter()
            .map(|(n, ip)| {
                let (en_linea, ip) = (n.en_linea, *ip);
                std::thread::Builder::new().name("nexo-drop-sondeo".into()).spawn_scoped(alcance, move || {
                    if en_linea { sondear(ip) } else { None }
                })
            })
            .collect();
        hilos.into_iter().map(|h| h.ok().and_then(|h| h.join().ok()).flatten()).collect()
    });
    let mut lista: Vec<Destino> = pares
        .into_iter()
        .zip(respuestas)
        .map(|((n, ip), hola)| Destino {
            plataforma: hola.as_ref().map(|h| h.plataforma).unwrap_or_else(|| plataforma_de(&n.sistema)),
            en_linea: hola.is_some(),
            tailscale_en_linea: n.en_linea,
            version: hola.and_then(|h| h.version),
            nombre: n.nombre,
            ip: ip.to_string(),
        })
        .collect();
    lista.sort_by(|a, b| {
        b.en_linea.cmp(&a.en_linea).then(b.tailscale_en_linea.cmp(&a.tailscale_en_linea)).then(a.nombre.cmp(&b.nombre))
    });
    lista
}
