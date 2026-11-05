//! Remoto: usar esta Mac desde el PC del trabajo con RustDesk por Tailscale (y, en Windows, conectarse a ella).
//!
//! En la Mac:
//! - Sesión: cada 5 s mira con libproc (sin abrir procesos) si RustDesk tiene una conexión TCP establecida:
//!   directa (puerto local 21118) o por el relé (puerto remoto 21117). Avisa a las ventanas con "remoto-cambio".
//! - Atajos: traduce los de Windows (Ctrl+C → ⌘C…) solo en las teclas que inyecta RustDesk (`teclado`).
//! - Captura: F13 (Imprimir pantalla del PC) toma una zona, la deja en 1x en el portapapeles y la guarda (`captura`).
//! - Portapapeles liviano: durante la sesión achica las imágenes grandes antes de que RustDesk las mande, y guarda
//!   las últimas 12 copias para volver a copiarlas (`portapapeles`).
//! - Durante la sesión: la Mac no se duerme (caffeinate) y la pantalla de la MacBook baja al mínimo.
//!   Si se eligió, la pantalla virtual para RustDesk se crea al empezar y se quita al terminar (`pantalla_virtual`).
//!
//! En Windows: abre RustDesk conectado a la Mac y, con la sesión al frente, manda a la Mac los atajos que
//! Windows se guarda (Win+Tab, Win+D, Ctrl+Win+←/→, Win+Shift+S, Alt+Tab, Win+E): ver `remoto_windows`.
//! La traducción de Ctrl → ⌘ la hace la Mac. La Mac es la que encontró `equipos` entre tus equipos de Tailscale.

// XDrop (sistema/drop) también usa Tailscale, las imágenes, el portapapeles y la captura.
pub(super) mod tailscale;

// Manejar la Mac desde Nexo en Windows, por el receptor de XDrop: su pantalla virtual y el modo trabajo.
pub mod mando;

#[cfg(target_os = "macos")]
pub(super) mod captura;
#[cfg(target_os = "macos")]
pub(super) mod imagen;
#[cfg(target_os = "macos")]
pub(super) mod portapapeles;
#[cfg(target_os = "macos")]
mod procesos;
#[cfg(target_os = "macos")]
mod teclado;

use super::ajustes;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::AppHandle;

/// Windows sin una Mac encontrada: qué decir al pedir "Conectar a mi Mac".
#[cfg(windows)]
const SIN_MAC: &str = "Todavía no encontré tu Mac en Tailscale. En Remoto te muestro cómo conectar tus equipos.";

/// Lo que se guarda en ajustes.json, dentro de "remoto". Todo viene encendido.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AjustesRemoto {
    /// Traducir los atajos de Windows que llegan por RustDesk.
    pub traducir: bool,
    /// F13 (Imprimir pantalla del PC) hace una captura liviana.
    pub captura: bool,
    /// Durante la sesión, la Mac no se duerme.
    pub evitar_reposo: bool,
    /// Durante la sesión, la pantalla de la MacBook baja al mínimo.
    pub brillo_minimo: bool,
    /// Durante la sesión, las imágenes grandes que se copian se achican (RustDesk no se congela).
    pub portapapeles_liviano: bool,
}

impl Default for AjustesRemoto {
    fn default() -> Self {
        AjustesRemoto { traducir: true, captura: true, evitar_reposo: true, brillo_minimo: true, portapapeles_liviano: true }
    }
}

/// Lo que cambia la interfaz con `remoto_ajustar`. Lo que no viene, queda igual.
#[derive(Deserialize, Default, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase", default)]
pub struct CambioRemoto {
    pub traducir: Option<bool>,
    pub captura: Option<bool>,
    pub evitar_reposo: Option<bool>,
    pub brillo_minimo: Option<bool>,
    pub portapapeles_liviano: Option<bool>,
    /// Modo trabajo: la Mac no se duerme aunque todavía no haya sesión (lo enciende "Preparar para el trabajo").
    pub modo_trabajo: Option<bool>,
}

/// La sesión de RustDesk que está entrando a esta Mac.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sesion {
    pub activa: bool,
    /// Del otro lado: el PC si es directa; el servidor de RustDesk si va por el relé.
    pub ip_remota: Option<String>,
    /// El nombre del PC en Tailscale ("innotech1017"), si se sabe.
    pub equipo: Option<String>,
    /// Directa por Tailscale (el otro lado es una dirección 100.x).
    pub directa: bool,
    /// Por el relé de RustDesk (más lenta).
    pub rele: bool,
}

/// Un atajo de Windows y lo que hace en la Mac. Es la tabla que muestra la interfaz.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Atajo {
    pub grupo: &'static str,
    pub windows: &'static [&'static str],
    pub mac: &'static [&'static str],
    pub accion: &'static str,
}

const fn atajo(
    grupo: &'static str,
    windows: &'static [&'static str],
    mac: &'static [&'static str],
    accion: &'static str,
) -> Atajo {
    Atajo { grupo, windows, mac, accion }
}

const EDICION: &str = "Edición";
const PESTANAS: &str = "Ventanas y pestañas";
const TEXTO: &str = "Moverse en el texto";
const ESCRITORIOS: &str = "Escritorios y apps";
const TERMINAL: &str = "En la Terminal";

/// Lo que traduce `teclado` (y lo que pasa igual, para que se vea completo).
const ATAJOS: &[Atajo] = &[
    atajo(EDICION, &["Ctrl", "C"], &["⌘", "C"], "Copiar"),
    atajo(EDICION, &["Ctrl", "V"], &["⌘", "V"], "Pegar"),
    atajo(EDICION, &["Ctrl", "X"], &["⌘", "X"], "Cortar"),
    atajo(EDICION, &["Ctrl", "Z"], &["⌘", "Z"], "Deshacer"),
    atajo(EDICION, &["Ctrl", "Y"], &["⌘", "⇧", "Z"], "Rehacer"),
    atajo(EDICION, &["Ctrl", "A"], &["⌘", "A"], "Seleccionar todo"),
    atajo(EDICION, &["Ctrl", "S"], &["⌘", "S"], "Guardar"),
    atajo(EDICION, &["Ctrl", "F"], &["⌘", "F"], "Buscar"),
    atajo(EDICION, &["Ctrl", "P"], &["⌘", "P"], "Imprimir"),
    atajo(EDICION, &["Ctrl", "⌫"], &["⌥", "⌫"], "Borrar la palabra anterior"),
    atajo(PESTANAS, &["Ctrl", "N"], &["⌘", "N"], "Ventana o documento nuevo"),
    atajo(PESTANAS, &["Ctrl", "T"], &["⌘", "T"], "Pestaña nueva"),
    atajo(PESTANAS, &["Ctrl", "W"], &["⌘", "W"], "Cerrar la pestaña"),
    atajo(PESTANAS, &["Ctrl", "Shift", "T"], &["⌘", "⇧", "T"], "Reabrir la pestaña cerrada"),
    atajo(PESTANAS, &["Ctrl", "R"], &["⌘", "R"], "Recargar"),
    atajo(PESTANAS, &["Ctrl", "L"], &["⌘", "L"], "Ir a la barra de direcciones"),
    atajo(TEXTO, &["Inicio"], &["⌘", "←"], "Principio de la línea"),
    atajo(TEXTO, &["Fin"], &["⌘", "→"], "Final de la línea"),
    atajo(TEXTO, &["Ctrl", "Inicio"], &["⌘", "↑"], "Principio del documento"),
    atajo(TEXTO, &["Ctrl", "Fin"], &["⌘", "↓"], "Final del documento"),
    atajo(ESCRITORIOS, &["Ctrl", "← →"], &["⌃", "← →"], "Cambiar de escritorio"),
    atajo(ESCRITORIOS, &["Ctrl", "Win", "← →"], &["⌃", "← →"], "Cambiar de escritorio, con el atajo de Windows"),
    atajo(ESCRITORIOS, &["Ctrl", "↑"], &["⌃", "↑"], "Mission Control: todos los escritorios"),
    atajo(ESCRITORIOS, &["Alt", "Tab"], &["⌘", "⇥"], "Cambiar de app"),
    atajo(TERMINAL, &["Ctrl", "C"], &["⌃", "C"], "Detener lo que corre (no se traduce)"),
    atajo(TERMINAL, &["Ctrl", "Shift", "C"], &["⌘", "C"], "Copiar"),
    atajo(TERMINAL, &["Ctrl", "Shift", "V"], &["⌘", "V"], "Pegar"),
];

/// Todo lo que muestra la sección Remoto.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoRemoto {
    #[serde(flatten)]
    pub sesion: Sesion,
    /// "activo", "detenido", "sin sesion" o "no instalado".
    pub tailscale: &'static str,
    /// Mac: RustDesk está abierto. Windows: está instalado.
    pub rustdesk: bool,
    pub rustdesk_instalado: bool,
    pub ajustes: AjustesRemoto,
    pub atajos: &'static [Atajo],
    pub modo_trabajo: bool,
    /// Accesibilidad: para traducir atajos y escuchar F13.
    pub permiso_teclas: bool,
    /// Grabación de pantalla: para la captura.
    pub permiso_pantalla: bool,
    /// La app aparte "Captura remota" sigue abierta (Nexo ya hace lo mismo).
    pub captura_remota_abierta: bool,
    /// La dirección de Tailscale de la Mac (la que se escribe en RustDesk): en la Mac, la suya; en Windows, la de la
    /// Mac encontrada (`equipos`). `None` si todavía no se sabe.
    pub ip_mac: Option<String>,
}

/// Una captura hecha con F13 o con "Probar captura".
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct Captura {
    pub ancho: u32,
    pub alto: u32,
    pub kb: u32,
    /// Dónde quedó la copia; vacío si no se pudo guardar.
    pub archivo: Option<String>,
    /// Si quedó en el portapapeles.
    pub copiada: bool,
}

/// Una copia del historial del portapapeles, como la ve la interfaz.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct EntradaPortapapeles {
    pub id: u64,
    /// "texto" o "imagen".
    pub tipo: &'static str,
    /// Los primeros 2000 caracteres del texto.
    pub texto: Option<String>,
    /// Cuántos caracteres tiene el texto completo.
    pub largo: Option<usize>,
    /// Miniatura de la imagen (data URL, PNG pequeño).
    pub miniatura: Option<String>,
    pub ancho: Option<u32>,
    pub alto: Option<u32>,
    /// Segundos desde 1970.
    pub momento: u64,
    /// "mac" o "remoto" (lo que llegó desde el PC). Es una estimación.
    pub origen: &'static str,
}

// Los ajustes en vivo: la escucha de teclado y los vigías los leen sin abrir el archivo.
static TRADUCIR: AtomicBool = AtomicBool::new(true);
static CAPTURA: AtomicBool = AtomicBool::new(true);
static EVITAR_REPOSO: AtomicBool = AtomicBool::new(true);
static BRILLO_MINIMO: AtomicBool = AtomicBool::new(true);
static LIVIANO: AtomicBool = AtomicBool::new(true);
static MODO_TRABAJO: AtomicBool = AtomicBool::new(false);

/// La última sesión vista (la escribe el vigía de la Mac).
static SESION: Mutex<Sesion> = Mutex::new(Sesion { activa: false, ip_remota: None, equipo: None, directa: false, rele: false });

fn poner_banderas(a: &AjustesRemoto) {
    TRADUCIR.store(a.traducir, Ordering::Relaxed);
    CAPTURA.store(a.captura, Ordering::Relaxed);
    EVITAR_REPOSO.store(a.evitar_reposo, Ordering::Relaxed);
    BRILLO_MINIMO.store(a.brillo_minimo, Ordering::Relaxed);
    LIVIANO.store(a.portapapeles_liviano, Ordering::Relaxed);
    // En Windows, el gancho que manda a la Mac los atajos que Windows se guarda (Win+Tab, Win+D…).
    #[cfg(windows)]
    super::remoto_windows::usar(a.traducir);
}

fn sesion_actual() -> Sesion {
    SESION.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Para Analítica: la sesión, si está el modo trabajo y si la Mac no se dormirá (caffeinate activo).
pub fn resumen() -> (Sesion, bool, bool) {
    let s = sesion_actual();
    let modo = MODO_TRABAJO.load(Ordering::Relaxed);
    let sin_reposo = cfg!(target_os = "macos") && EVITAR_REPOSO.load(Ordering::Relaxed) && (s.activa || modo);
    (s, modo, sin_reposo)
}

/// Arranque: carga los ajustes y, en la Mac, enciende el vigía de la sesión, la escucha de teclado y el portapapeles.
pub fn iniciar(app: &AppHandle) {
    poner_banderas(&ajustes::cargar(app).remoto);
    #[cfg(target_os = "macos")]
    mac::iniciar(app);
    #[cfg(windows)]
    super::remoto_windows::iniciar(app.clone());
}

/// Corre un programa y devuelve lo que escribió. Si pasa del límite, lo cierra por su PID: nunca deja procesos.
fn ejecutar(programa: &Path, args: &[&str], limite: Duration) -> Option<Vec<u8>> {
    let mut comando = Command::new(programa);
    comando.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    // Sin TERM (Nexo abierto al iniciar sesión), el programa de Tailscale.app cree que lo abren como app y no
    // contesta ("The Tailscale GUI failed to start"): XDrop no escuchaba tras reiniciar la Mac.
    #[cfg(target_os = "macos")]
    if std::env::var_os("TERM").is_none() {
        comando.env("TERM", "dumb");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        comando.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: sin consola que asome
    }
    let mut hijo = comando.spawn().ok()?;
    let cerrar = |hijo: &mut std::process::Child| {
        let _ = hijo.kill();
        let _ = hijo.wait();
    };
    let Some(mut salida) = hijo.stdout.take() else {
        cerrar(&mut hijo);
        return None;
    };
    // Se lee en otro hilo para que el programa no se trabe si escribe mucho.
    let lector = std::thread::Builder::new().name("nexo-lector".into()).spawn(move || {
        let mut texto = Vec::new();
        let _ = salida.read_to_end(&mut texto);
        texto
    });
    let Ok(lector) = lector else {
        cerrar(&mut hijo);
        return None;
    };
    let fin = Instant::now() + limite;
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < fin => std::thread::sleep(Duration::from_millis(40)),
            _ => {
                cerrar(&mut hijo);
                break;
            }
        }
    }
    lector.join().ok()
}

fn estado(app: &AppHandle) -> EstadoRemoto {
    let (rustdesk, rustdesk_instalado, captura_remota_abierta) = programas();
    let info = tailscale::leer(Duration::from_secs(20));
    let ip_mac = if cfg!(target_os = "macos") {
        info.propio.as_ref().and_then(|p| p.ipv4()).map(|ip| ip.to_string())
    } else {
        ajustes::cargar(app).equipos.otro.filter(|o| o.plataforma == "mac").map(|o| o.ip)
    };
    EstadoRemoto {
        sesion: sesion_actual(),
        tailscale: info.estado,
        rustdesk,
        rustdesk_instalado,
        ajustes: ajustes::cargar(app).remoto,
        atajos: ATAJOS,
        modo_trabajo: MODO_TRABAJO.load(Ordering::Relaxed),
        permiso_teclas: super::teclas::permiso_concedido(),
        permiso_pantalla: pantalla_permitida(),
        captura_remota_abierta,
        ip_mac,
    }
}

/// (RustDesk abierto, RustDesk instalado, "Captura remota" abierta).
#[cfg(target_os = "macos")]
fn programas() -> (bool, bool, bool) {
    let l = procesos::leer();
    (l.rustdesk, l.rustdesk || rustdesk_en_aplicaciones(), l.captura_remota)
}

#[cfg(target_os = "macos")]
fn rustdesk_en_aplicaciones() -> bool {
    Path::new("/Applications/RustDesk.app").exists()
        || std::env::var_os("HOME").is_some_and(|h| Path::new(&h).join("Applications/RustDesk.app").exists())
}

/// Si RustDesk está instalado en este equipo (lo usa la guía de `equipos`).
#[cfg(target_os = "macos")]
pub fn rustdesk_instalado() -> bool {
    rustdesk_en_aplicaciones() || procesos::leer().rustdesk
}

#[cfg(windows)]
pub fn rustdesk_instalado() -> bool {
    win::rustdesk().is_some()
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn rustdesk_instalado() -> bool {
    false
}

#[cfg(windows)]
fn programas() -> (bool, bool, bool) {
    let hay = win::rustdesk().is_some();
    (hay, hay, false)
}

#[cfg(not(any(target_os = "macos", windows)))]
fn programas() -> (bool, bool, bool) {
    (false, false, false)
}

// ---------------------------------------------------------------- Permiso de Grabación de pantalla

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

/// Si macOS deja a Nexo grabar la pantalla (lo pide la captura). Solo lee.
/// Ojo: macOS suele confirmarlo recién al volver a abrir Nexo.
#[cfg(target_os = "macos")]
pub fn pantalla_permitida() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

/// Muestra el aviso de macOS (la primera vez) y pone a Nexo en la lista de Grabación de pantalla.
#[cfg(target_os = "macos")]
pub fn pedir_pantalla() {
    unsafe {
        CGRequestScreenCaptureAccess();
    }
}

/// Windows no pide permiso para capturar la pantalla.
#[cfg(not(target_os = "macos"))]
pub fn pantalla_permitida() -> bool {
    true
}

// ---------------------------------------------------------------- Comandos

/// Cómo está el remoto: la sesión, Tailscale, RustDesk, los ajustes y la tabla de atajos.
#[tauri::command]
pub async fn remoto_estado(app: AppHandle) -> EstadoRemoto {
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || estado(&a)).await.unwrap_or_else(|_| estado(&app))
}

/// Guarda uno o varios ajustes del remoto (y el modo trabajo) y los aplica. Lo usan el panel y el mando (`mando`).
fn aplicar_cambio(app: &AppHandle, cambio: CambioRemoto) -> Result<(), String> {
    let nuevos = ajustes::cambiar(app, |a| {
        let r = &mut a.remoto;
        if let Some(v) = cambio.traducir {
            r.traducir = v;
        }
        if let Some(v) = cambio.captura {
            r.captura = v;
        }
        if let Some(v) = cambio.evitar_reposo {
            r.evitar_reposo = v;
        }
        if let Some(v) = cambio.brillo_minimo {
            r.brillo_minimo = v;
        }
        if let Some(v) = cambio.portapapeles_liviano {
            r.portapapeles_liviano = v;
        }
    })?
    .remoto;
    poner_banderas(&nuevos);
    if let Some(v) = cambio.modo_trabajo {
        MODO_TRABAJO.store(v, Ordering::Relaxed);
    }
    #[cfg(target_os = "macos")]
    mac::aplicar(app, cambio.brillo_minimo.is_some());
    Ok(())
}

/// Cambia uno o varios ajustes del remoto (y el modo trabajo). Devuelve cómo quedó todo.
#[tauri::command]
pub async fn remoto_ajustar(app: AppHandle, cambio: CambioRemoto) -> Result<EstadoRemoto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        aplicar_cambio(&app, cambio)?;
        Ok(estado(&app))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// "Preparar para el trabajo": conecta Tailscale, abre RustDesk y enciende el modo trabajo (la Mac no se duerme).
#[tauri::command]
pub async fn remoto_preparar(app: AppHandle) -> Result<EstadoRemoto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        preparar()?;
        Ok(estado(&app))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(target_os = "macos")]
fn preparar() -> Result<(), String> {
    mac::preparar()
}

#[cfg(not(target_os = "macos"))]
fn preparar() -> Result<(), String> {
    Err("En Windows usa «Conectar a mi Mac».".into())
}

/// Captura liviana a mano ("Probar captura"). `None` si se canceló con Esc.
#[tauri::command]
pub async fn remoto_capturar(app: AppHandle) -> Result<Option<Captura>, String> {
    #[cfg(target_os = "macos")]
    {
        tauri::async_runtime::spawn_blocking(move || captura::capturar(&app)).await.map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("Las capturas para el remoto se hacen en la Mac.".into())
    }
}

/// Abre RustDesk.
#[tauri::command]
pub async fn remoto_abrir_rustdesk() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(target_os = "macos")]
        {
            mac::abrir_app("RustDesk", false)
        }
        #[cfg(windows)]
        {
            win::abrir_rustdesk(&[])
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            Err::<(), String>("Todavía no disponible en este sistema".into())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Windows: abre RustDesk ya conectándose a tu Mac por Tailscale (la que encontró `equipos`).
#[tauri::command]
pub async fn remoto_conectar_mac(app: AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(move || {
            let (_, ip) = super::equipos::mac(&app).ok_or(SIN_MAC)?;
            win::abrir_rustdesk(&["--connect", ip.as_str()])
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("Esto se usa desde la PC con Windows.".into())
    }
}

/// Mac: abre la carpeta Imágenes › Capturas remotas.
#[tauri::command]
pub async fn remoto_abrir_capturas() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        tauri::async_runtime::spawn_blocking(mac::abrir_capturas).await.map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Las capturas para el remoto se guardan en la Mac.".into())
    }
}

/// Las últimas copias (texto e imágenes), de la más nueva a la más vieja. En Windows, vacío.
#[tauri::command]
pub fn portapapeles_historial() -> Vec<EntradaPortapapeles> {
    #[cfg(target_os = "macos")]
    {
        portapapeles::historial()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

/// Vuelve a poner una copia del historial en el portapapeles.
#[tauri::command]
pub async fn portapapeles_copiar(app: AppHandle, id: u64) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        tauri::async_runtime::spawn_blocking(move || portapapeles::copiar(&app, id)).await.map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, id);
        Err("El historial del portapapeles está en la Mac.".into())
    }
}

/// Borra el historial del portapapeles (y sus imágenes guardadas).
#[tauri::command]
pub async fn portapapeles_borrar(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        tauri::async_runtime::spawn_blocking(move || portapapeles::borrar(&app)).await.map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(())
    }
}

// ---------------------------------------------------------------- Mac: vigía de la sesión y comodidad

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use crate::dispositivos::pantallas::Pantallas;
    use objc2::msg_send;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;
    use std::net::IpAddr;
    use std::process::Child;
    use std::sync::{Arc, OnceLock};
    use tauri::{Emitter, Manager};

    pub static APP: OnceLock<AppHandle> = OnceLock::new();
    /// Si hay una sesión entrando (la lee el portapapeles cada vez que mira).
    pub static SESION_ACTIVA: AtomicBool = AtomicBool::new(false);
    /// El caffeinate que mantiene despierta la Mac. Se cierra por su PID.
    static CAFEINA: Mutex<Option<Child>> = Mutex::new(None);
    /// El brillo que tenía la pantalla de la MacBook antes de bajarlo, para devolverlo al terminar.
    static BRILLO_PREVIO: Mutex<Vec<(u32, u8)>> = Mutex::new(Vec::new());

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGDisplayIsBuiltin(id: u32) -> u32;
    }

    pub fn iniciar(app: &AppHandle) {
        let _ = APP.set(app.clone());
        let a = app.clone();
        if let Err(e) = std::thread::Builder::new().name("nexo-remoto".into()).spawn(move || vigilar(a)) {
            eprintln!("Nexo: no se pudo vigilar la sesión remota ({e})");
        }
        portapapeles::iniciar(app.clone());
    }

    /// Cada 5 s: ¿hay sesión de RustDesk? Al empezar y al terminar aplica la comodidad y avisa a las ventanas.
    fn vigilar(app: AppHandle) {
        let mut ausencias = 0u8;
        loop {
            teclado::asegurar();
            let antes = sesion_actual();
            match procesos::leer().conexion {
                Some(c) => {
                    ausencias = 0;
                    let ip = c.ip.to_string();
                    if !antes.activa || antes.ip_remota.as_deref() != Some(ip.as_str()) || antes.rele != c.rele {
                        let directa = !c.rele && es_tailscale(&c.ip);
                        // El nombre del PC sale de Tailscale; por el relé solo se ve el servidor de RustDesk.
                        let equipo = if directa { tailscale::leer(Duration::from_secs(5)).nombre_de(&ip) } else { None };
                        cambiar(&app, Sesion { activa: true, ip_remota: Some(ip), equipo, directa, rele: c.rele });
                    }
                }
                None if antes.activa => {
                    // Una lectura sin conexión puede ser un corte de un instante: se espera a la segunda (10 s).
                    ausencias += 1;
                    if ausencias >= 2 {
                        ausencias = 0;
                        cambiar(&app, Sesion::default());
                    }
                }
                None => {}
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    }

    /// 100.64.0.0/10 y fd7a:115c:a1e0::/48 son las direcciones de Tailscale.
    fn es_tailscale(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => {
                let o = v4.octets();
                o[0] == 100 && (o[1] & 0xC0) == 64
            }
            IpAddr::V6(v6) => {
                let s = v6.segments();
                s[0] == 0xfd7a && s[1] == 0x115c && s[2] == 0xa1e0
            }
        }
    }

    fn cambiar(app: &AppHandle, nueva: Sesion) {
        let antes = {
            let mut s = SESION.lock().unwrap_or_else(|e| e.into_inner());
            std::mem::replace(&mut *s, nueva.clone())
        };
        SESION_ACTIVA.store(nueva.activa, Ordering::Relaxed);
        if nueva.activa != antes.activa {
            actualizar_cafeina();
            if nueva.activa {
                if BRILLO_MINIMO.load(Ordering::Relaxed) {
                    bajar_brillo(app);
                }
            } else {
                restaurar_brillo(app);
            }
            crate::sistema::pantalla_virtual::al_cambiar_sesion(app, nueva.activa);
        }
        let _ = app.emit("remoto-cambio", &nueva);
    }

    /// Tras cambiar un ajuste: la escucha de teclado, caffeinate y (si cambió ese ajuste) el brillo.
    pub fn aplicar(app: &AppHandle, cambio_brillo: bool) {
        teclado::asegurar();
        actualizar_cafeina();
        if cambio_brillo && SESION_ACTIVA.load(Ordering::Relaxed) {
            if BRILLO_MINIMO.load(Ordering::Relaxed) {
                bajar_brillo(app);
            } else {
                restaurar_brillo(app);
            }
        }
    }

    /// caffeinate -s (no dormir con el cargador puesto) mientras haya sesión o esté el modo trabajo.
    /// Con -w se cierra solo si Nexo se cierra; si no, Nexo lo cierra por su PID.
    fn actualizar_cafeina() {
        let quiere = EVITAR_REPOSO.load(Ordering::Relaxed)
            && (SESION_ACTIVA.load(Ordering::Relaxed) || MODO_TRABAJO.load(Ordering::Relaxed));
        let mut c = CAFEINA.lock().unwrap_or_else(|e| e.into_inner());
        // Si se cerró por su cuenta, se recoge y se olvida.
        let vivo = c.as_mut().is_some_and(|h| matches!(h.try_wait(), Ok(None) | Err(_)));
        if !vivo {
            if let Some(mut h) = c.take() {
                let _ = h.wait();
            }
        }
        if quiere && c.is_none() {
            let pid = std::process::id().to_string();
            *c = Command::new("/usr/bin/caffeinate")
                .args(["-s", "-w", pid.as_str()])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok();
        } else if !quiere {
            if let Some(mut h) = c.take() {
                let _ = h.kill();
                let _ = h.wait();
            }
        }
    }

    fn bajar_brillo(app: &AppHandle) {
        let Some(p) = app.try_state::<Arc<Pantallas>>() else { return };
        let internas: Vec<(u32, u8)> =
            p.brillos().into_iter().filter(|(id, _)| unsafe { CGDisplayIsBuiltin(*id) } != 0).collect();
        {
            let mut previo = BRILLO_PREVIO.lock().unwrap_or_else(|e| e.into_inner());
            // Si ya se había bajado, se guarda el de antes (no el 0).
            if previo.is_empty() {
                *previo = internas.clone();
            }
        }
        for (id, _) in internas {
            let _ = p.poner_brillo(id, 0);
        }
    }

    fn restaurar_brillo(app: &AppHandle) {
        let previo = std::mem::take(&mut *BRILLO_PREVIO.lock().unwrap_or_else(|e| e.into_inner()));
        let Some(p) = app.try_state::<Arc<Pantallas>>() else { return };
        for (id, nivel) in previo {
            if nivel > 0 {
                let _ = p.poner_brillo(id, nivel);
            }
        }
    }

    /// Abre una app con `open -a` (con `fondo`, sin traerla al frente) y espera a que `open` termine.
    pub fn abrir_app(nombre: &str, fondo: bool) -> Result<(), String> {
        let mut c = Command::new("/usr/bin/open");
        if fondo {
            c.arg("-g");
        }
        let estado = c
            .args(["-a", nombre])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        if estado.success() { Ok(()) } else { Err(format!("No encontré {nombre} en Aplicaciones.")) }
    }

    /// Tailscale conectado (lo abre y, si no se conecta solo, prueba `tailscale up`), RustDesk abierto y modo trabajo.
    pub fn preparar() -> Result<(), String> {
        if Path::new("/Applications/Tailscale.app").exists() || tailscale::programa().is_some() {
            let _ = abrir_app("Tailscale", true);
            for intento in 0..8 {
                if tailscale::leer(Duration::ZERO).estado == "activo" {
                    break;
                }
                if intento == 3 {
                    tailscale::conectar();
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        let rustdesk = abrir_app("RustDesk", true);
        MODO_TRABAJO.store(true, Ordering::Relaxed);
        actualizar_cafeina();
        rustdesk
    }

    pub fn abrir_capturas() -> Result<(), String> {
        let carpeta = captura::carpeta().ok_or("No encontré tu carpeta de Imágenes.")?;
        std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
        Command::new("/usr/bin/open")
            .arg(&carpeta)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// El identificador de la app que está al frente ("com.apple.Terminal"). Es rápido: lo usa la escucha de teclas.
    pub fn app_al_frente() -> Option<String> {
        let clase = AnyClass::get(c"NSWorkspace")?;
        autoreleasepool(|_| {
            // SAFETY: sharedWorkspace, frontmostApplication y bundleIdentifier devuelven objetos o nil.
            unsafe {
                let espacio: *mut AnyObject = msg_send![clase, sharedWorkspace];
                if espacio.is_null() {
                    return None;
                }
                let app: *mut AnyObject = msg_send![espacio, frontmostApplication];
                if app.is_null() {
                    return None;
                }
                let id: *mut NSString = msg_send![app, bundleIdentifier];
                (!id.is_null()).then(|| (*id).to_string())
            }
        })
    }
}

// ---------------------------------------------------------------- Windows: RustDesk

#[cfg(windows)]
mod win {
    use std::path::PathBuf;

    /// Dónde está rustdesk.exe: la carpeta de siempre o la que anotó su instalador en el registro.
    pub fn rustdesk() -> Option<PathBuf> {
        let mut candidatos = Vec::new();
        if let Some(programas) = std::env::var_os("ProgramFiles") {
            candidatos.push(PathBuf::from(programas).join("RustDesk").join("rustdesk.exe"));
        }
        candidatos.push(PathBuf::from(r"C:\Program Files\RustDesk\rustdesk.exe"));
        if let Some(r) = del_registro() {
            candidatos.push(r);
        }
        candidatos.into_iter().find(|p| p.is_file())
    }

    fn del_registro() -> Option<PathBuf> {
        use std::ffi::c_void;
        use windows::Win32::Foundation::ERROR_SUCCESS;
        use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
        use windows::core::{PCWSTR, w};

        let leer = |valor: PCWSTR| -> Option<String> {
            let clave = w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\RustDesk");
            let mut tam = 0u32;
            let r = unsafe {
                RegGetValueW(HKEY_LOCAL_MACHINE, clave, valor, RRF_RT_REG_SZ, None, None, Some(&mut tam as *mut u32))
            };
            if r != ERROR_SUCCESS || !(2..=64 * 1024).contains(&tam) {
                return None;
            }
            let mut datos = vec![0u16; (tam as usize).div_ceil(2)];
            let r = unsafe {
                RegGetValueW(
                    HKEY_LOCAL_MACHINE,
                    clave,
                    valor,
                    RRF_RT_REG_SZ,
                    None,
                    Some(datos.as_mut_ptr() as *mut c_void),
                    Some(&mut tam as *mut u32),
                )
            };
            if r != ERROR_SUCCESS {
                return None;
            }
            let fin = datos.iter().position(|c| *c == 0).unwrap_or(datos.len());
            Some(String::from_utf16_lossy(&datos[..fin]))
        };

        if let Some(carpeta) = leer(w!("InstallLocation")) {
            let ruta = PathBuf::from(carpeta.trim().trim_matches('"')).join("rustdesk.exe");
            if ruta.is_file() {
                return Some(ruta);
            }
        }
        // DisplayIcon trae la ruta del .exe, a veces con ",0" al final.
        let icono = leer(w!("DisplayIcon"))?;
        let icono = icono.trim().trim_matches('"');
        let ruta = match icono.rsplit_once(',') {
            Some((r, n)) if n.trim().parse::<i32>().is_ok() => r,
            _ => icono,
        };
        Some(PathBuf::from(ruta.trim_matches('"'))).filter(|p| p.is_file())
    }

    /// Abre RustDesk con esos argumentos (por ejemplo `--connect 100.64.0.2`).
    pub fn abrir_rustdesk(args: &[&str]) -> Result<(), String> {
        let ruta = rustdesk().ok_or("No encontré RustDesk en este PC. Instálalo desde rustdesk.com.")?;
        std::process::Command::new(ruta)
            .args(args)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("No pude abrir RustDesk ({e})."))
    }
}
