//! Analítica: el estado del equipo (CPU, memoria, temperatura, batería, disco, red, procesos y la sesión remota),
//! para verlo aquí o desde el otro equipo del dueño por XDrop (`GET /nexo/analitica`, con su misma seguridad).
//!
//! - Solo se mide mientras alguien mira: cada pedido del panel vale 6 s y cada pedido del otro equipo, 15 s.
//!   Un hilo toma una muestra cada 2 s y se apaga solo cuando nadie pide más. Sin nadie mirando, no corre nada.
//! - Se guardan los últimos 5 minutos de CPU, memoria y red para las gráficas.
//! - Mac (`mac`): mach (CPU y memoria), sysctl, IOKit (batería), NSProcessInfo (temperatura en palabras),
//!   statfs (disco), getifaddrs (red) y libproc (procesos). Windows (`win`): kernel32.
//!   Lo que un sistema no informa va en `None` y la interfaz dice "No disponible".

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
use mac as plataforma;
#[cfg(windows)]
mod win;
#[cfg(windows)]
use win as plataforma;
#[cfg(not(any(target_os = "macos", windows)))]
mod otro;
#[cfg(not(any(target_os = "macos", windows)))]
use otro as plataforma;

use super::remoto::tailscale;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

/// Cada cuánto se toma una muestra mientras alguien mira.
const CADA: Duration = Duration::from_secs(2);
/// La primera muestra: la CPU sale de la diferencia con la base, tomada 1 s antes.
const PRIMERA: Duration = Duration::from_secs(1);
/// Cuánto vale un pedido del panel y uno del otro equipo.
const LOCAL: Duration = Duration::from_secs(6);
const REMOTO: Duration = Duration::from_secs(15);
/// Una muestra más vieja que esto no sirve: se espera la siguiente (1,5 s como mucho).
const FRESCA: Duration = Duration::from_secs(3);
const ESPERA: Duration = Duration::from_millis(1500);
/// Lo que guardan las gráficas: 5 minutos.
const VENTANA_MS: u64 = 5 * 60 * 1000;
const MAX_PUNTOS: usize = 200;
/// Cuántos procesos se muestran en cada lista.
const TOP: usize = 6;

const PLAZO_CONECTAR: Duration = Duration::from_millis(1000);
const PLAZO_LEER: Duration = Duration::from_millis(2500);
const MAX_RESPUESTA: u64 = 256 * 1024;

// ---------------------------------------------------------------- Lo que se mide

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Cpu {
    /// Uso de todo el equipo, de 0 a 100.
    pub total: f32,
    /// Uso de cada núcleo, de 0 a 100 (Windows: vacío).
    pub nucleos: Vec<f32>,
    pub cantidad: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Memoria {
    /// Bytes.
    pub total: u64,
    /// Lo que está en uso (en Mac: apps + cableada + comprimida, como el Monitor de Actividad).
    pub usada: u64,
    pub apps: Option<u64>,
    pub cableada: Option<u64>,
    pub comprimida: Option<u64>,
    /// Archivos en caché: se liberan si hace falta.
    pub cache: Option<u64>,
    pub libre: u64,
    pub swap_usada: Option<u64>,
    pub swap_total: Option<u64>,
    /// "normal", "aviso" o "critica" (solo Mac).
    pub presion: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Temperatura {
    /// "normal", "tibia", "caliente" o "muy-caliente" (el estado térmico de macOS). `None`: no se sabe.
    pub estado: Option<String>,
    /// Grados de la batería, si el sistema los da.
    pub bateria: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Bateria {
    pub nivel: u8,
    pub cargando: bool,
    /// Enchufada al cargador.
    pub conectada: bool,
    pub completa: bool,
    /// Minutos para vaciarse (sin cargador) o para llenarse (cargando).
    pub minutos: Option<u32>,
    pub ciclos: Option<u32>,
    /// Capacidad de hoy frente a la de fábrica, en %.
    pub salud: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Disco {
    pub total: u64,
    pub libre: u64,
}

/// Bytes por segundo.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Tramo {
    pub bajada: f64,
    pub subida: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Red {
    /// Por las interfaces de verdad (Wi-Fi y cable), sin contar dos veces lo que pasa por Tailscale.
    pub bajada: f64,
    pub subida: f64,
    /// Lo que pasa por Tailscale (RustDesk directo y XDrop van por aquí).
    pub tailscale: Option<Tramo>,
}

/// Una app con todos sus procesos sumados.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Proceso {
    pub nombre: String,
    /// Parte de todo el equipo, de 0 a 100 (la misma escala que la CPU total).
    pub cpu: f32,
    /// Bytes en memoria.
    pub memoria: u64,
    pub procesos: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Procesos {
    pub cpu: Vec<Proceso>,
    pub memoria: Vec<Proceso>,
    /// Solo se ven los del usuario (en Mac, los del sistema piden ser administrador).
    pub solo_tuyos: bool,
}

/// Lo que mide el hilo en cada vuelta.
#[derive(Clone, Debug, Default)]
pub struct Muestra {
    pub cpu: Option<Cpu>,
    pub memoria: Option<Memoria>,
    pub temperatura: Temperatura,
    pub bateria: Option<Bateria>,
    pub disco: Option<Disco>,
    pub red: Option<Red>,
    /// Segundos desde que arrancó.
    pub encendida: Option<u64>,
    pub procesos: Procesos,
    /// RustDesk está abierto.
    pub rustdesk: bool,
}

/// Lo que se suma de cada app al recorrer los procesos.
#[derive(Default)]
pub struct Grupo {
    pub cpu: f32,
    pub memoria: u64,
    pub procesos: u32,
}

/// Las apps que más CPU y más memoria usan.
pub fn mas_gastan(grupos: HashMap<String, Grupo>, solo_tuyos: bool) -> Procesos {
    let mut lista: Vec<Proceso> = grupos
        .into_iter()
        .map(|(nombre, g)| Proceso { nombre, cpu: redondear(g.cpu), memoria: g.memoria, procesos: g.procesos })
        .collect();
    lista.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.memoria.cmp(&a.memoria)));
    let cpu = lista.iter().take(TOP).cloned().collect();
    lista.sort_by(|a, b| b.memoria.cmp(&a.memoria));
    lista.truncate(TOP);
    Procesos { cpu, memoria: lista, solo_tuyos }
}

/// Con un decimal (para que la respuesta no lleve ruido).
pub fn redondear(x: f32) -> f32 {
    if x.is_finite() { (x * 10.0).round() / 10.0 } else { 0.0 }
}

// ---------------------------------------------------------------- Lo que se entrega

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PantallaVirtual {
    pub activa: bool,
    pub ancho: u32,
    pub alto: u32,
}

/// Un envío de XDrop en curso.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Transferencia {
    /// "enviado" o "recibido".
    pub direccion: String,
    pub equipo: String,
    pub nombre: String,
    pub total: u32,
    /// De 0 a 1, si se sabe.
    pub avance: Option<f32>,
    /// Bytes por segundo.
    pub velocidad: f64,
}

/// La sesión de RustDesk y lo que la acompaña.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Sesion {
    /// RustDesk está abierto.
    pub rustdesk: bool,
    /// Hay una sesión entrando (solo Mac).
    pub activa: bool,
    pub directa: bool,
    pub rele: bool,
    /// El equipo del otro lado, si se sabe.
    pub equipo: Option<String>,
    /// Mac: la pantalla virtual para RustDesk. `None` donde no existe.
    pub pantalla_virtual: Option<PantallaVirtual>,
    pub xdrop: Vec<Transferencia>,
    /// caffeinate está activo: la Mac no se dormirá.
    pub sin_reposo: bool,
    /// Mac: el modo trabajo. `None` en Windows.
    pub modo_trabajo: Option<bool>,
}

/// Los últimos 5 minutos, en columnas (una muestra cada 2 s).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Historial {
    /// Milisegundos entre muestras.
    pub cada: u32,
    /// Cuándo se tomó cada muestra (ms desde 1970, con el reloj del equipo medido).
    pub momentos: Vec<u64>,
    pub cpu: Vec<Option<f32>>,
    /// % de la memoria en uso.
    pub memoria: Vec<Option<f32>>,
    pub bajada: Vec<Option<f64>>,
    pub subida: Vec<Option<f64>>,
}

/// Todo lo que muestra la sección Analítica de un equipo.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Analitica {
    /// El nombre del equipo en Tailscale ("demo-sige").
    pub equipo: String,
    /// "mac", "windows" u "otro".
    pub plataforma: String,
    /// Cuándo se midió (ms desde 1970).
    pub momento: u64,
    pub cpu: Option<Cpu>,
    pub memoria: Option<Memoria>,
    pub temperatura: Temperatura,
    pub bateria: Option<Bateria>,
    pub disco: Option<Disco>,
    pub red: Option<Red>,
    /// Segundos desde que arrancó.
    pub encendida: Option<u64>,
    pub procesos: Procesos,
    pub sesion: Sesion,
    pub historial: Historial,
}

/// Este equipo o uno de los otros equipos del dueño.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EquipoAnalitica {
    pub nombre: String,
    /// `None`: este equipo.
    pub ip: Option<String>,
    pub plataforma: String,
    /// Nexo respondió (este equipo, siempre).
    pub en_linea: bool,
    pub propio: bool,
}

// ---------------------------------------------------------------- El hilo que mide

struct Punto {
    momento: u64,
    cpu: Option<f32>,
    memoria: Option<f32>,
    bajada: Option<f64>,
    subida: Option<f64>,
}

struct Estado {
    /// Hasta cuándo alguien mira.
    hasta: Option<Instant>,
    /// El hilo está corriendo.
    vivo: bool,
    /// La última muestra: cuándo (reloj del hilo y ms desde 1970) y qué.
    ultima: Option<(Instant, u64, Muestra)>,
    puntos: VecDeque<Punto>,
}

static ESTADO: Mutex<Estado> = Mutex::new(Estado { hasta: None, vivo: false, ultima: None, puntos: VecDeque::new() });
/// Avisa que llegó una muestra nueva (para quien espera la primera).
static NUEVA: Condvar = Condvar::new();

fn bloquear() -> MutexGuard<'static, Estado> {
    ESTADO.lock().unwrap_or_else(|e| e.into_inner())
}

fn ahora_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Alguien mira durante `por`: si el hilo no corre, arranca.
fn mirar(por: Duration) {
    let mut e = bloquear();
    let fin = Instant::now() + por;
    if e.hasta.is_none_or(|h| h < fin) {
        e.hasta = Some(fin);
    }
    if e.vivo {
        return;
    }
    e.vivo = true;
    if let Err(error) = std::thread::Builder::new().name("nexo-analitica".into()).spawn(muestrear) {
        e.vivo = false;
        eprintln!("Nexo: no se pudo medir el equipo ({error})");
    }
}

/// Mide cada 2 s mientras alguien mira; después se apaga (la próxima vez arranca de nuevo).
fn muestrear() {
    let mut lector = plataforma::Lector::nuevo();
    // La base: la CPU y la red salen de la diferencia con esta lectura.
    let _ = lector.medir();
    std::thread::sleep(PRIMERA);
    loop {
        let m = lector.medir();
        let momento = ahora_ms();
        {
            let mut e = bloquear();
            e.puntos.push_back(Punto {
                momento,
                cpu: m.cpu.as_ref().map(|c| c.total),
                memoria: m.memoria.as_ref().filter(|x| x.total > 0).map(|x| redondear(x.usada as f32 * 100.0 / x.total as f32)),
                bajada: m.red.as_ref().map(|r| r.bajada.round()),
                subida: m.red.as_ref().map(|r| r.subida.round()),
            });
            while e.puntos.front().is_some_and(|p| momento.saturating_sub(p.momento) > VENTANA_MS)
                || e.puntos.len() > MAX_PUNTOS
            {
                e.puntos.pop_front();
            }
            e.ultima = Some((Instant::now(), momento, m));
            NUEVA.notify_all();
            // Nadie más mira: se apaga aquí mismo (con el candado tomado, así nadie queda sin hilo).
            if e.hasta.is_none_or(|h| Instant::now() >= h) {
                e.vivo = false;
                e.hasta = None;
                return;
            }
        }
        std::thread::sleep(CADA);
    }
}

fn historial(puntos: &VecDeque<Punto>) -> Historial {
    Historial {
        cada: CADA.as_millis() as u32,
        momentos: puntos.iter().map(|p| p.momento).collect(),
        cpu: puntos.iter().map(|p| p.cpu).collect(),
        memoria: puntos.iter().map(|p| p.memoria).collect(),
        bajada: puntos.iter().map(|p| p.bajada).collect(),
        subida: puntos.iter().map(|p| p.subida).collect(),
    }
}

/// La foto del equipo ahora (y los últimos 5 minutos). Marca que alguien mira durante `por`.
fn foto(app: &AppHandle, por: Duration) -> Analitica {
    mirar(por);
    let limite = Instant::now() + ESPERA;
    let mut e = bloquear();
    // Si el hilo recién arranca, se espera su primera muestra.
    while e.ultima.as_ref().is_none_or(|(t, _, _)| t.elapsed() > FRESCA) {
        let resto = limite.saturating_duration_since(Instant::now());
        if resto.is_zero() {
            break;
        }
        e = match NUEVA.wait_timeout(e, resto) {
            Ok((g, _)) => g,
            Err(x) => x.into_inner().0,
        };
    }
    let historial = historial(&e.puntos);
    let (momento, muestra) = e.ultima.as_ref().map(|(_, m, x)| (*m, x.clone())).unwrap_or_else(|| (ahora_ms(), Muestra::default()));
    std::mem::drop(e);

    Analitica {
        equipo: super::drop::nombre_propio(),
        plataforma: super::drop::PLATAFORMA.into(),
        momento,
        cpu: muestra.cpu,
        memoria: muestra.memoria,
        temperatura: muestra.temperatura,
        bateria: muestra.bateria,
        disco: muestra.disco,
        red: muestra.red,
        encendida: muestra.encendida,
        procesos: muestra.procesos,
        sesion: sesion(app, muestra.rustdesk),
        historial,
    }
}

/// RustDesk, la pantalla virtual, XDrop, caffeinate y el modo trabajo. Todo ya está en memoria: no se mide nada.
fn sesion(app: &AppHandle, rustdesk: bool) -> Sesion {
    let (s, modo, sin_reposo) = super::remoto::resumen();
    let pantalla_virtual = super::pantalla_virtual::en_menu().map(|activa| {
        if activa {
            let e = super::pantalla_virtual::estado(app);
            PantallaVirtual { activa: e.activa, ancho: e.ancho, alto: e.alto }
        } else {
            PantallaVirtual::default()
        }
    });
    let xdrop = super::drop::drop_envios()
        .into_iter()
        .filter(|p| p.estado == "transfiriendo" || p.estado == "esperando")
        .take(4)
        .map(|p| {
            let avance = if p.tamano_lote > 0 {
                Some(p.enviados_lote as f32 / p.tamano_lote as f32)
            } else if p.tamano > 0 {
                Some(p.enviados as f32 / p.tamano as f32)
            } else {
                None
            };
            Transferencia {
                direccion: p.direccion.into(),
                equipo: p.equipo,
                nombre: p.nombre,
                total: p.total,
                avance: avance.map(|a| a.clamp(0.0, 1.0)),
                velocidad: p.velocidad,
            }
        })
        .collect();
    Sesion {
        rustdesk: rustdesk || s.activa,
        activa: s.activa,
        directa: s.directa,
        rele: s.rele,
        equipo: s.equipo,
        pantalla_virtual,
        xdrop,
        sin_reposo,
        modo_trabajo: cfg!(target_os = "macos").then_some(modo),
    }
}

/// Lo que responde `GET /nexo/analitica`. Lo llama el receptor de XDrop con la seguridad ya revisada.
/// El otro equipo mira: se sigue midiendo 15 s más.
pub fn json_remoto(app: &AppHandle) -> serde_json::Value {
    serde_json::to_value(foto(app, REMOTO)).unwrap_or(serde_json::Value::Null)
}

// ---------------------------------------------------------------- El otro equipo

/// Separa una respuesta HTTP en su código y su cuerpo.
fn separar_respuesta(datos: &[u8]) -> Option<(u16, &[u8])> {
    let fin = datos.windows(4).position(|v| v == b"\r\n\r\n")?;
    let cabeza = std::str::from_utf8(&datos[..fin]).ok()?;
    let codigo = cabeza.lines().next()?.split(' ').nth(1)?.parse().ok()?;
    Some((codigo, &datos[fin + 4..]))
}

/// Le pide su analítica a otro equipo del dueño (`GET /nexo/analitica`): 1 s para conectar y 2,5 s para responder.
fn pedir(ip: Ipv4Addr) -> Result<Analitica, String> {
    let mut s = TcpStream::connect_timeout(&SocketAddr::from((ip, super::drop::PUERTO)), PLAZO_CONECTAR)
        .map_err(|_| "No respondió.".to_string())?;
    let _ = s.set_write_timeout(Some(PLAZO_LEER));
    let pedido = format!(
        "GET /nexo/analitica HTTP/1.1\r\nHost: {ip}:{}\r\nUser-Agent: Nexo/{}\r\nConnection: close\r\n\r\n",
        super::drop::PUERTO,
        env!("CARGO_PKG_VERSION")
    );
    s.write_all(pedido.as_bytes()).map_err(|_| "Se cortó la conexión.".to_string())?;
    // Un solo plazo para toda la respuesta, no por cada lectura.
    let fin = Instant::now() + PLAZO_LEER;
    let mut datos = Vec::new();
    let mut trozo = [0u8; 16 * 1024];
    loop {
        let resto = fin.saturating_duration_since(Instant::now());
        if resto.is_zero() || datos.len() as u64 > MAX_RESPUESTA {
            break;
        }
        let _ = s.set_read_timeout(Some(resto));
        match s.read(&mut trozo) {
            Ok(0) => break,
            Ok(n) => datos.extend_from_slice(&trozo[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    match separar_respuesta(&datos) {
        Some((200, cuerpo)) => {
            serde_json::from_slice::<Analitica>(cuerpo).map(limpiar).map_err(|_| "Su respuesta no se entiende.".into())
        }
        Some((404, _)) => Err("Tiene una versión anterior de Nexo: actualízala para ver aquí su analítica.".into()),
        Some((403, _)) => Err("No aceptó la consulta: revisa que sea tuyo en Tailscale.".into()),
        Some((codigo, _)) => Err(format!("No pudo mostrar su analítica ({codigo}).")),
        None => Err("No respondió a tiempo.".into()),
    }
}

fn corto(t: &str, max: usize) -> String {
    t.chars().filter(|c| !c.is_control()).take(max).collect()
}

/// Lo que llega del otro equipo, con límites: textos cortos y listas acotadas.
fn limpiar(mut a: Analitica) -> Analitica {
    a.equipo = corto(&a.equipo, 64);
    if !matches!(a.plataforma.as_str(), "mac" | "windows") {
        a.plataforma = "otro".into();
    }
    if let Some(c) = a.cpu.as_mut() {
        c.nucleos.truncate(256);
    }
    let procesos = |lista: &mut Vec<Proceso>| {
        lista.truncate(TOP);
        for p in lista.iter_mut() {
            p.nombre = corto(&p.nombre, 80);
        }
    };
    procesos(&mut a.procesos.cpu);
    procesos(&mut a.procesos.memoria);
    a.sesion.equipo = a.sesion.equipo.as_deref().map(|e| corto(e, 64));
    a.sesion.xdrop.truncate(4);
    for t in a.sesion.xdrop.iter_mut() {
        t.equipo = corto(&t.equipo, 64);
        t.nombre = corto(&t.nombre, 200);
        t.direccion = if t.direccion == "recibido" { "recibido".into() } else { "enviado".into() };
    }
    if let Some(p) = a.memoria.as_mut().and_then(|m| m.presion.as_mut())
        && !matches!(p.as_str(), "normal" | "aviso" | "critica")
    {
        *p = "normal".into();
    }
    let h = &mut a.historial;
    h.momentos.truncate(MAX_PUNTOS);
    h.cpu.truncate(MAX_PUNTOS);
    h.memoria.truncate(MAX_PUNTOS);
    h.bajada.truncate(MAX_PUNTOS);
    h.subida.truncate(MAX_PUNTOS);
    a
}

/// La dirección elegida, si es de un equipo del mismo dueño en Tailscale.
fn direccion_valida(ip: &str) -> Result<Ipv4Addr, String> {
    let ip: Ipv4Addr = ip.trim().parse().map_err(|_| "Esa no es una dirección de Tailscale.".to_string())?;
    let texto = ip.to_string();
    let mut info = tailscale::leer(Duration::from_secs(60));
    if info.es_del_dueno(&texto) != Some(true) {
        info = tailscale::leer(Duration::ZERO);
    }
    if info.es_del_dueno(&texto) != Some(true) {
        return Err("Ese equipo no es tuyo en Tailscale.".into());
    }
    Ok(ip)
}

// ---------------------------------------------------------------- Comandos

/// La analítica de este equipo. Mientras el panel la pida, se sigue midiendo cada 2 s.
#[tauri::command]
pub async fn analitica_local(app: AppHandle) -> Analitica {
    tauri::async_runtime::spawn_blocking(move || foto(&app, LOCAL)).await.unwrap_or_default()
}

/// La analítica de otro equipo del dueño, por XDrop. Falla con el motivo en palabras si no responde.
#[tauri::command]
pub async fn analitica_remota(ip: String) -> Result<Analitica, String> {
    tauri::async_runtime::spawn_blocking(move || pedir(direccion_valida(&ip)?)).await.map_err(|e| e.to_string())?
}

/// Este equipo y los otros equipos del dueño (Mac o Windows), con si Nexo responde en cada uno.
/// Usa la búsqueda de XDrop (cada equipo se recuerda 8 s).
#[tauri::command]
pub async fn analitica_equipos(app: AppHandle) -> Vec<EquipoAnalitica> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut lista = vec![EquipoAnalitica {
            nombre: super::drop::nombre_propio(),
            ip: None,
            plataforma: super::drop::PLATAFORMA.into(),
            en_linea: true,
            propio: true,
        }];
        lista.extend(super::drop::buscar_destinos(&app).into_iter().filter(|d| d.plataforma != "otro").map(|d| {
            EquipoAnalitica {
                nombre: d.nombre,
                ip: Some(d.ip),
                plataforma: d.plataforma.into(),
                en_linea: d.en_linea,
                propio: false,
            }
        }));
        lista
    })
    .await
    .unwrap_or_default()
}
