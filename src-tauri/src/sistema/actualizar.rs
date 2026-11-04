//! Actualizaciones de Nexo para Windows, desde la Mac por XDrop (sin nube ni servicio de firmas).
//!
//! - Sello: cada compilación trae `COMPILADO` (segundos UTC) y `BUILD` (id corto), que pone build.rs.
//!   La versión sigue en 0.1.0: se compara el sello.
//! - En la Mac (la que ofrece): `<carpeta de ajustes>/actualizaciones/windows/` tiene `nexo-setup.exe` e
//!   `info.json` `{compilado, tamano, sha256}` (los deja `scripts/publicar-windows.sh`). El receptor de XDrop
//!   responde `GET /nexo/actualizacion?plataforma=windows` (el info.json, o 404) y
//!   `GET /nexo/actualizacion/archivo?plataforma=windows` (el instalador). Antes de llegar aquí ya pasó el control
//!   de XDrop: solo equipos de Tailscale del mismo dueño (y este mismo equipo).
//! - En Windows (el que pide): a los 20 s de abrir y cada 30 min pregunta a las Mac del dueño que responden
//!   `hola`. Si alguna ofrece una compilación más nueva, avisa ("actualizacion-disponible" y una notificación).
//! - `actualizar_instalar`: baja el instalador a %TEMP%, revisa tamaño y sha256, lo abre en silencio y aparte
//!   (`/S /UPDATE /R`) y Nexo se cierra. El instalador de Tauri cierra lo que quede abierto, reemplaza los archivos
//!   sin tocar los accesos directos ni el arranque, y vuelve a abrir Nexo.

use super::drop::PUERTO;
use super::remoto::tailscale;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

/// Cuándo se compiló este Nexo (segundos UTC desde 1970).
pub const COMPILADO: u64 = numero(env!("NEXO_COMPILADO"));
/// Un id corto de esta compilación (el commit de git).
pub const BUILD: &str = env!("NEXO_BUILD");

/// Banderas del instalador NSIS de Tauri: en silencio (/S), como actualización (/UPDATE: no rehace los accesos
/// directos ni quita el arranque) y abrir Nexo al terminar (/R).
#[cfg_attr(not(windows), allow(dead_code))]
pub const BANDERAS_INSTALADOR: [&str; 3] = ["/S", "/UPDATE", "/R"];

const PRIMERA_REVISION: Duration = Duration::from_secs(20);
const CADA: Duration = Duration::from_secs(30 * 60);
const MIN_INSTALADOR: u64 = 64 * 1024;
const MAX_INSTALADOR: u64 = 300 * 1024 * 1024;
const MAX_CABEZA: usize = 16 * 1024;
const CARPETA: &str = "actualizaciones";
const WINDOWS: &str = "windows";
const ARCHIVO: &str = "nexo-setup.exe";
const INFO: &str = "info.json";
const PREFIJO_TEMPORAL: &str = "nexo-actualizacion-";
const RUTA_INFO: &str = "/nexo/actualizacion?plataforma=windows";
const RUTA_ARCHIVO: &str = "/nexo/actualizacion/archivo?plataforma=windows";

/// Lee un número decimal al compilar (lo que no es cifra se salta).
const fn numero(texto: &str) -> u64 {
    let b = texto.as_bytes();
    let mut i = 0;
    let mut n: u64 = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            n = n.saturating_mul(10).saturating_add((b[i] - b'0') as u64);
        }
        i += 1;
    }
    n
}

/// Si este equipo busca actualizaciones en la Mac: solo Nexo en Windows.
fn busca() -> bool {
    cfg!(windows)
}

// ---------------------------------------------------------------- Estado

/// Lo que ofrece una Mac del dueño.
#[derive(Clone, Debug)]
struct Oferta {
    compilado: u64,
    tamano: u64,
    sha256: String,
    ip: Ipv4Addr,
    /// "demo-sige".
    equipo: String,
}

struct Interno {
    oferta: Option<Oferta>,
    /// La compilación de la que ya se avisó (una notificación por cada una).
    avisada: u64,
    /// La última revisión (segundos UTC) y, si no se pudo preguntar, por qué.
    revisado: Option<u64>,
    problema: Option<String>,
}

static ESTADO: Mutex<Interno> = Mutex::new(Interno { oferta: None, avisada: 0, revisado: None, problema: None });
/// Una revisión a la vez (la automática y "Buscar actualización").
static REVISANDO: Mutex<()> = Mutex::new(());
static INSTALANDO: AtomicBool = AtomicBool::new(false);

fn bloquear() -> std::sync::MutexGuard<'static, Interno> {
    ESTADO.lock().unwrap_or_else(|e| e.into_inner())
}

/// Lo que ve la interfaz.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EstadoActualizacion {
    /// Una Mac del dueño ofrece una compilación más nueva que esta.
    pub disponible: bool,
    pub compilado_actual: u64,
    pub compilado_nuevo: Option<u64>,
    /// La Mac que la ofrece ("demo-sige").
    pub desde: Option<String>,
    /// El id corto de esta compilación.
    pub build: &'static str,
    /// Si este equipo busca actualizaciones (solo Windows).
    pub busca: bool,
    /// Cuándo se revisó por última vez (segundos UTC).
    pub revisado: Option<u64>,
    /// Por qué no se pudo revisar la última vez (sin Tailscale, la Mac no responde…).
    pub problema: Option<String>,
    /// Se está bajando o instalando.
    pub instalando: bool,
}

fn vista(e: &Interno) -> EstadoActualizacion {
    EstadoActualizacion {
        disponible: e.oferta.is_some(),
        compilado_actual: COMPILADO,
        compilado_nuevo: e.oferta.as_ref().map(|o| o.compilado),
        desde: e.oferta.as_ref().map(|o| o.equipo.clone()),
        build: BUILD,
        busca: busca(),
        revisado: e.revisado,
        problema: e.problema.clone(),
        instalando: INSTALANDO.load(Ordering::SeqCst),
    }
}

/// El avance de "Actualizar": "descargando", "verificando", "instalando" o "error".
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Avance {
    fase: &'static str,
    porcentaje: u8,
}

fn avance(app: &AppHandle, fase: &'static str, porcentaje: u8) {
    let _ = app.emit("actualizacion-progreso", Avance { fase, porcentaje });
}

fn segundos_ahora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ---------------------------------------------------------------- Windows: buscar

/// Al abrir Nexo: en Windows busca a los 20 s y después cada 30 min (un hilo que duerme entre vueltas).
pub fn iniciar(app: &AppHandle) {
    if !busca() {
        return;
    }
    let app = app.clone();
    let hilo = std::thread::Builder::new().name("nexo-actualizar".into()).spawn(move || {
        std::thread::sleep(PRIMERA_REVISION);
        limpiar_temporales();
        loop {
            revisar(&app, true);
            std::thread::sleep(CADA);
        }
    });
    if let Err(e) = hilo {
        eprintln!("Nexo: no se pudo iniciar la búsqueda de actualizaciones ({e})");
    }
}

/// Pregunta a las Mac y guarda lo que encontró. Si no respondió ninguna, se queda con lo que ya sabía.
fn revisar(app: &AppHandle, automatico: bool) -> EstadoActualizacion {
    let _turno = REVISANDO.lock().unwrap_or_else(|e| e.into_inner());
    // A mano, con lo que dice Tailscale ahora; la automática se conforma con lo leído hace un minuto.
    let edad = Duration::from_secs(if automatico { 60 } else { 5 });
    let (respondio, oferta, problema) = buscar_en_macs(edad);
    let (estado, notificar) = {
        let mut e = bloquear();
        e.revisado = Some(segundos_ahora());
        e.problema = problema;
        if respondio {
            e.oferta = oferta.filter(|o| o.compilado > COMPILADO);
        }
        let mut notificar = false;
        if let Some(nueva) = e.oferta.as_ref().map(|o| o.compilado).filter(|c| *c != e.avisada) {
            e.avisada = nueva;
            // A mano ya se ve en pantalla: la notificación es solo para la revisión automática.
            notificar = automatico;
        }
        (vista(&e), notificar)
    };
    if estado.disponible {
        let _ = app.emit("actualizacion-disponible", &estado);
    }
    if notificar {
        let _ = app.notification().builder().title("Nexo").body("Hay una versión nueva de Nexo desde tu Mac").show();
    }
    estado
}

/// (alguna Mac respondió, la oferta más nueva, el problema si no se pudo preguntar).
fn buscar_en_macs(edad: Duration) -> (bool, Option<Oferta>, Option<String>) {
    let info = tailscale::leer(edad);
    match info.estado {
        "no instalado" => return (false, None, Some("Tailscale no está instalado en este equipo.".into())),
        "detenido" => return (false, None, Some("Tailscale no está conectado en este equipo.".into())),
        "sin sesion" => return (false, None, Some("Entra con tu cuenta en Tailscale en este equipo.".into())),
        _ => {}
    }
    let macs: Vec<(String, Ipv4Addr)> = info
        .pares_del_dueno()
        .into_iter()
        .filter(|n| n.en_linea && matches!(n.sistema.to_ascii_lowercase().as_str(), "macos" | ""))
        .filter_map(|n| n.ipv4().map(|ip| (n.nombre.clone(), ip)))
        .collect();
    if macs.is_empty() {
        return (false, None, Some("Tu Mac no aparece conectada en Tailscale.".into()));
    }
    let mut respondio = false;
    let mut mejor: Option<Oferta> = None;
    for (equipo, ip) in macs {
        // Solo un Nexo de Mac que contesta `hola`.
        let Ok((200, hola)) = pedir_json(ip, "/nexo/hola") else { continue };
        if hola.get("plataforma").and_then(|p| p.as_str()) != Some("mac") {
            continue;
        }
        respondio = true;
        let Ok((200, v)) = pedir_json(ip, RUTA_INFO) else { continue };
        if let Some(o) = oferta_de(&v, ip, &equipo) {
            if mejor.as_ref().is_none_or(|m| o.compilado > m.compilado) {
                mejor = Some(o);
            }
        }
    }
    let problema = (!respondio).then(|| "Tu Mac no responde: revisa que Nexo esté abierto en ella.".to_string());
    (respondio, mejor, problema)
}

fn es_huella(texto: &str) -> bool {
    texto.len() == 64 && texto.bytes().all(|b| b.is_ascii_hexdigit())
}

fn oferta_de(v: &serde_json::Value, ip: Ipv4Addr, equipo: &str) -> Option<Oferta> {
    let compilado = v.get("compilado")?.as_u64().filter(|n| *n > 0)?;
    let tamano = v.get("tamano")?.as_u64().filter(|n| (MIN_INSTALADOR..=MAX_INSTALADOR).contains(n))?;
    let sha256 = v.get("sha256")?.as_str().filter(|s| es_huella(s))?.to_ascii_lowercase();
    Some(Oferta { compilado, tamano, sha256, ip, equipo: equipo.to_string() })
}

// ---------------------------------------------------------------- HTTP del lado que pide

fn conectar(ip: Ipv4Addr, limite: Duration) -> io::Result<TcpStream> {
    TcpStream::connect_timeout(&SocketAddr::from((ip, PUERTO)), limite)
}

fn pedido(ip: Ipv4Addr, ruta: &str) -> String {
    format!(
        "GET {ruta} HTTP/1.1\r\nHost: {ip}:{PUERTO}\r\nUser-Agent: Nexo/{} ({BUILD})\r\nConnection: close\r\n\r\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// Una respuesta HTTP: el código, el encabezado y lo que ya vino del cuerpo.
fn separar(datos: &[u8]) -> Option<(u16, &str, &[u8])> {
    let fin = datos.windows(4).position(|v| v == b"\r\n\r\n")?;
    let cabeza = std::str::from_utf8(&datos[..fin]).ok()?;
    let codigo = cabeza.lines().next()?.split(' ').nth(1)?.parse().ok()?;
    Some((codigo, cabeza, &datos[fin + 4..]))
}

fn largo_de(cabeza: &str) -> Option<u64> {
    cabeza.lines().skip(1).find_map(|l| {
        let (clave, valor) = l.split_once(':')?;
        if clave.trim().eq_ignore_ascii_case("content-length") { valor.trim().parse().ok() } else { None }
    })
}

/// Un GET pequeño (hasta 64 KB): el código y el JSON.
fn pedir_json(ip: Ipv4Addr, ruta: &str) -> Result<(u16, serde_json::Value), String> {
    let mut s = conectar(ip, Duration::from_secs(2)).map_err(|e| e.to_string())?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(5)));
    s.write_all(pedido(ip, ruta).as_bytes()).map_err(|e| e.to_string())?;
    let mut datos = Vec::new();
    let _ = Read::by_ref(&mut s).take(64 * 1024).read_to_end(&mut datos);
    let (codigo, _, cuerpo) = separar(&datos).ok_or("No respondió.")?;
    Ok((codigo, serde_json::from_slice(cuerpo).unwrap_or(serde_json::Value::Null)))
}

// ---------------------------------------------------------------- Windows: bajar e instalar

fn hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn porcentaje(parte: u64, total: u64) -> u8 {
    if total == 0 { 0 } else { (parte.saturating_mul(100) / total).min(100) as u8 }
}

/// Baja el instalador a %TEMP% y revisa su tamaño y su sha256. Devuelve dónde quedó.
fn descargar(app: &AppHandle, o: &Oferta) -> Result<PathBuf, String> {
    let temporal = std::env::temp_dir();
    let destino = temporal.join(format!("{PREFIJO_TEMPORAL}{}.exe", o.compilado));
    let parte = temporal.join(format!("{PREFIJO_TEMPORAL}{}.parte", o.compilado));
    let sin_conexion = || format!("No pude conectar con {}. Revisa que esté encendida y con Nexo abierto.", o.equipo);
    let cortado = || format!("Se cortó la descarga desde {}. Prueba de nuevo.", o.equipo);

    let mut s = conectar(o.ip, Duration::from_secs(5)).map_err(|_| sin_conexion())?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(10)));
    s.write_all(pedido(o.ip, RUTA_ARCHIVO).as_bytes()).map_err(|_| sin_conexion())?;

    // Primero el encabezado (con lo que ya llegó del archivo).
    let mut datos = Vec::with_capacity(8 * 1024);
    let mut trozo = vec![0u8; 256 * 1024];
    while !datos.windows(4).any(|v| v == b"\r\n\r\n") {
        let n = s.read(&mut trozo).map_err(|_| cortado())?;
        if n == 0 {
            return Err(cortado());
        }
        datos.extend_from_slice(&trozo[..n]);
        if datos.len() > MAX_CABEZA + trozo.len() {
            return Err(format!("{} respondió algo que no entiendo.", o.equipo));
        }
    }
    let (codigo, cabeza, resto) =
        separar(&datos).ok_or_else(|| format!("{} respondió algo que no entiendo.", o.equipo))?;
    if codigo != 200 {
        let motivo = serde_json::from_slice::<serde_json::Value>(resto)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string));
        return Err(motivo.unwrap_or(format!("{} no entregó la actualización ({codigo}).", o.equipo)));
    }
    if largo_de(cabeza) != Some(o.tamano) {
        return Err("El instalador de la Mac no coincide con lo que anunció. Vuelve a publicarlo.".into());
    }

    let mut f = File::create(&parte).map_err(|e| format!("No pude guardar la actualización ({e})."))?;
    let escribir_mal = |e: io::Error| format!("No pude guardar la actualización ({e}).");
    let resultado = (|| -> Result<String, String> {
        let mut huella = Sha256::new();
        let primero = &resto[..resto.len().min(o.tamano as usize)];
        f.write_all(primero).map_err(escribir_mal)?;
        huella.update(primero);
        let mut llegados = primero.len() as u64;
        let mut ultimo_aviso = Instant::now();
        avance(app, "descargando", porcentaje(llegados, o.tamano));
        while llegados < o.tamano {
            let quiero = (trozo.len() as u64).min(o.tamano - llegados) as usize;
            let n = match s.read(&mut trozo[..quiero]) {
                Ok(0) => return Err(cortado()),
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(cortado()),
            };
            f.write_all(&trozo[..n]).map_err(escribir_mal)?;
            huella.update(&trozo[..n]);
            llegados += n as u64;
            if ultimo_aviso.elapsed() >= Duration::from_millis(150) {
                ultimo_aviso = Instant::now();
                avance(app, "descargando", porcentaje(llegados, o.tamano));
            }
        }
        f.flush().map_err(escribir_mal)?;
        f.sync_all().map_err(escribir_mal)?;
        Ok(hexadecimal(&huella.finalize()))
    })();
    drop(f);
    let _ = s.shutdown(Shutdown::Both);

    avance(app, "verificando", 100);
    let en_disco = std::fs::metadata(&parte).map(|m| m.len()).unwrap_or(0);
    match resultado {
        Ok(h) if h == o.sha256 && en_disco == o.tamano => {}
        Ok(_) => {
            let _ = std::fs::remove_file(&parte);
            return Err("La actualización llegó dañada (su huella no coincide). Prueba de nuevo.".into());
        }
        Err(e) => {
            let _ = std::fs::remove_file(&parte);
            return Err(e);
        }
    }
    let _ = std::fs::remove_file(&destino);
    std::fs::rename(&parte, &destino).map_err(|e| {
        let _ = std::fs::remove_file(&parte);
        format!("No pude preparar el instalador ({e}).")
    })?;
    Ok(destino)
}

/// Abre el instalador aparte de Nexo (así sigue aunque Nexo se cierre). Sin ventana ni preguntas.
#[cfg(windows)]
fn lanzar(instalador: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let abrir = |banderas: u32| {
        Command::new(instalador)
            .args(BANDERAS_INSTALADOR)
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(banderas)
            .spawn()
    };
    // Si Nexo corre dentro de un "job" que no deja salir, se abre sin esa bandera.
    abrir(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| abrir(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP))
        .map(|_| ())
        .map_err(|e| format!("No pude abrir el instalador ({e})."))
}

#[cfg(not(windows))]
fn lanzar(_instalador: &Path) -> Result<(), String> {
    Err("Las actualizaciones desde la Mac son para Nexo en Windows.".into())
}

fn instalar(app: &AppHandle) -> Result<(), String> {
    let conocida = bloquear().oferta.clone();
    let oferta = match conocida {
        Some(o) => o,
        None => {
            revisar(app, false);
            bloquear().oferta.clone().ok_or("Ya tienes la versión más nueva de Nexo.")?
        }
    };
    avance(app, "descargando", 0);
    let instalador = descargar(app, &oferta)?;
    avance(app, "instalando", 100);
    lanzar(&instalador)?;
    // Nexo se cierra enseguida (antes de que el instalador revise si está abierto); el instalador reemplaza los
    // archivos y lo vuelve a abrir. Si este hilo no se crea, el instalador igual cierra Nexo antes de copiar.
    let a = app.clone();
    let _ = std::thread::Builder::new().name("nexo-actualizar-salir".into()).spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        a.exit(0);
    });
    Ok(())
}

/// Borra instaladores viejos de %TEMP% (el de la última actualización ya terminó).
fn limpiar_temporales() {
    let Ok(lista) = std::fs::read_dir(std::env::temp_dir()) else { return };
    for e in lista.flatten() {
        let nombre = e.file_name();
        let nombre = nombre.to_string_lossy();
        if nombre.starts_with(PREFIJO_TEMPORAL) && (nombre.ends_with(".exe") || nombre.ends_with(".parte")) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

// ---------------------------------------------------------------- Comandos

/// Cómo está: esta compilación y, si hay, la nueva que ofrece la Mac.
#[tauri::command]
pub fn actualizacion_estado() -> EstadoActualizacion {
    vista(&bloquear())
}

/// "Buscar actualización": pregunta ahora a las Mac del dueño.
#[tauri::command]
pub async fn actualizacion_buscar(app: AppHandle) -> Result<EstadoActualizacion, String> {
    if !busca() {
        return Ok(actualizacion_estado());
    }
    tauri::async_runtime::spawn_blocking(move || revisar(&app, false)).await.map_err(|e| e.to_string())
}

/// Baja la versión nueva, la revisa, abre el instalador en silencio y cierra Nexo (el instalador lo vuelve a abrir).
#[tauri::command]
pub async fn actualizar_instalar(app: AppHandle) -> Result<(), String> {
    if !busca() {
        return Err("Las actualizaciones desde la Mac son para Nexo en Windows.".into());
    }
    if INSTALANDO.swap(true, Ordering::SeqCst) {
        return Err("Nexo ya se está actualizando.".into());
    }
    let a = app.clone();
    let resultado =
        tauri::async_runtime::spawn_blocking(move || instalar(&a)).await.map_err(|e| e.to_string()).and_then(|r| r);
    if resultado.is_err() {
        INSTALANDO.store(false, Ordering::SeqCst);
        avance(&app, "error", 0);
    }
    resultado
}

// ---------------------------------------------------------------- Mac: lo que se ofrece

/// Lo que deja `scripts/publicar-windows.sh` en info.json.
#[derive(Deserialize)]
struct Publicada {
    compilado: u64,
    tamano: u64,
    sha256: String,
}

/// El info.json y el instalador publicados, si están completos. Solo "windows": ninguna ruta viene de afuera.
fn publicada(app: &AppHandle, consulta: &HashMap<String, String>) -> Result<(Publicada, PathBuf), (u16, String)> {
    if consulta.get("plataforma").map(String::as_str) != Some(WINDOWS) {
        return Err((404, "Solo hay actualizaciones para Windows.".into()));
    }
    let nada = || (404, "No hay una versión publicada en este equipo.".to_string());
    let carpeta = app.path().app_config_dir().map_err(|_| nada())?.join(CARPETA).join(WINDOWS);
    let texto = std::fs::read(carpeta.join(INFO)).map_err(|_| nada())?;
    let p: Publicada =
        serde_json::from_slice(&texto).map_err(|_| (500, "El info.json publicado no se entiende.".to_string()))?;
    let archivo = carpeta.join(ARCHIVO);
    let largo = std::fs::metadata(&archivo).ok().filter(|m| m.is_file()).map(|m| m.len()).ok_or_else(nada)?;
    if p.compilado == 0 || p.tamano != largo || !es_huella(&p.sha256) {
        return Err((500, "La versión publicada está a medias: vuelve a publicarla.".into()));
    }
    Ok((p, archivo))
}

fn cabeza_ok(tipo: &str, largo: u64) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {tipo}\r\nContent-Length: {largo}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
    )
}

/// `GET /nexo/actualizacion?plataforma=windows`: el info.json publicado.
pub fn servir_info(
    app: &AppHandle,
    s: &mut TcpStream,
    consulta: &HashMap<String, String>,
) -> Result<(), (u16, String)> {
    let (p, _) = publicada(app, consulta)?;
    let cuerpo = json!({
        "plataforma": WINDOWS,
        "compilado": p.compilado,
        "tamano": p.tamano,
        "sha256": p.sha256.to_ascii_lowercase(),
    })
    .to_string();
    let cabeza = cabeza_ok("application/json; charset=utf-8", cuerpo.len() as u64);
    let _ = s.write_all(cabeza.as_bytes()).and_then(|_| s.write_all(cuerpo.as_bytes())).and_then(|_| s.flush());
    let _ = s.shutdown(Shutdown::Write);
    Ok(())
}

/// `GET /nexo/actualizacion/archivo?plataforma=windows`: el instalador, de corrido.
pub fn servir_archivo(
    app: &AppHandle,
    s: &mut TcpStream,
    consulta: &HashMap<String, String>,
) -> Result<(), (u16, String)> {
    let (p, archivo) = publicada(app, consulta)?;
    let mut f = File::open(&archivo).map_err(|_| (404, "No hay una versión publicada en este equipo.".to_string()))?;
    let cabeza = cabeza_ok("application/octet-stream", p.tamano);
    // Con el encabezado ya enviado, un corte no se contesta: del otro lado no coincide el tamaño y se descarta.
    let envio = s
        .write_all(cabeza.as_bytes())
        .and_then(|_| io::copy(&mut Read::by_ref(&mut f).take(p.tamano), s))
        .and_then(|_| s.flush());
    if let Err(e) = envio {
        eprintln!("Nexo: se cortó el envío de la actualización ({e})");
    }
    let _ = s.shutdown(Shutdown::Write);
    Ok(())
}
