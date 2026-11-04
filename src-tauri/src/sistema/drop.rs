//! XDrop (antes Nexo Drop): pasar archivos, carpetas, textos, enlaces y capturas entre los equipos del dueño por Tailscale.
//!
//! - Receptor (`servidor`): un HTTP mínimo que escucha solo en la dirección de Tailscale de este equipo, puerto 47821.
//!   Solo atiende a equipos del mismo dueño en Tailscale (y a este mismo equipo, para probar).
//!   Lo recibido queda en Descargas › XDrop; el texto va al portapapeles y el enlace se abre en el navegador.
//! - Envío (`envio`): un lote a la vez, en otro hilo, con avance ("drop-progreso") y la opción de cancelar.
//! - Historial (`historial`): las últimas 40 entradas en drop.json.
//! - Lo propio de cada sistema (portapapeles, miniaturas, abrir y mostrar) va en `mac` y `win`.

mod envio;
mod historial;
mod progreso;
mod rutas;
mod servidor;

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

use super::ajustes;
use super::remoto::tailscale;
use historial::{Entrada, EntradaVista};
use progreso::Progreso;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

/// El puerto de XDrop en la red de Tailscale.
pub const PUERTO: u16 = 47821;

/// El nombre de XDrop en pantalla y en las notificaciones.
pub const NOMBRE: &str = "XDrop";

/// La carpeta de lo recibido, dentro de Descargas (igual en Mac y en Windows).
pub const CARPETA: &str = "XDrop";

/// "mac" o "windows": lo que responde `GET /nexo/hola`.
pub const PLATAFORMA: &str = if cfg!(target_os = "macos") {
    "mac"
} else if cfg!(windows) {
    "windows"
} else {
    "otro"
};

/// Lo que se guarda en ajustes.json, dentro de "drop". Todo viene encendido.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AjustesDrop {
    /// El receptor: si está apagado, Nexo no escucha en la red.
    pub recibir: bool,
    /// Si llega una sola imagen, también queda en el portapapeles.
    pub copiar_imagenes: bool,
}

impl Default for AjustesDrop {
    fn default() -> Self {
        AjustesDrop { recibir: true, copiar_imagenes: true }
    }
}

/// Lo que cambia `drop_ajustar`. Lo que no viene, queda igual.
#[derive(Deserialize, Default, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase", default)]
pub struct CambioDrop {
    pub recibir: Option<bool>,
    pub copiar_imagenes: Option<bool>,
}

/// Cómo está XDrop en este equipo.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoDrop {
    pub ajustes: AjustesDrop,
    /// La dirección donde escucha ahora; vacío si no escucha.
    pub escuchando: Option<String>,
    /// Este equipo en Tailscale ("demo-sige").
    pub nombre: String,
    pub plataforma: &'static str,
    /// Dónde queda lo recibido.
    pub carpeta: Option<String>,
    /// "activo", "detenido" o "no instalado".
    pub tailscale: &'static str,
    pub puerto: u16,
}

/// Otro equipo del dueño, al que se le puede mandar.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Destino {
    pub nombre: String,
    pub ip: String,
    /// "mac", "windows" u "otro".
    pub plataforma: &'static str,
    /// Nexo respondió en ese equipo.
    pub en_linea: bool,
    /// Tailscale lo ve conectado (aunque Nexo no responda).
    pub tailscale_en_linea: bool,
    pub version: Option<String>,
}

/// Lo que devuelve `drop_enviar`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Envio {
    pub lote: String,
}

/// Lo que devuelve `drop_enviar_portapapeles`: qué se mandó.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvioCopiado {
    /// "texto", "imagen" o "archivos".
    pub tipo: &'static str,
    /// El lote, si fueron archivos o una imagen (el avance llega por "drop-progreso").
    pub lote: Option<String>,
    pub cantidad: usize,
}

/// Lo que hay copiado en el portapapeles.
pub enum Copiado {
    /// `oculto`: lo marcó un gestor de contraseñas (no se guarda en el historial).
    Texto { texto: String, oculto: bool },
    /// Una imagen, ya en PNG.
    Imagen(Vec<u8>),
    /// Archivos o carpetas copiados en Finder o en el Explorador.
    Archivos(Vec<PathBuf>),
}

static COPIAR_IMAGENES: AtomicBool = AtomicBool::new(true);

/// Lo último que se supo de los destinos: el menú del ícono lo usa sin volver a preguntar.
static DESTINOS: Mutex<Vec<Destino>> = Mutex::new(Vec::new());

/// Cómo terminó un envío (lo usa el menú del ícono para avisar).
pub enum FinEnvio {
    Listo,
    Cancelado,
    Fallo(String),
}

/// Arranque: lee el historial y los ajustes y, si está encendido, levanta el receptor.
pub fn iniciar(app: &AppHandle) {
    historial::cargar(app);
    let a = ajustes::cargar(app).drop;
    COPIAR_IMAGENES.store(a.copiar_imagenes, Ordering::Relaxed);
    servidor::iniciar(app.clone(), a.recibir);
}

/// Descargas › XDrop.
pub fn carpeta(app: &AppHandle) -> Option<PathBuf> {
    let descargas = app.path().download_dir().ok().or_else(|| app.path().home_dir().ok().map(|h| h.join("Downloads")))?;
    Some(descargas.join(CARPETA))
}

/// El nombre de este equipo en Tailscale ("demo-sige") o, si no se sabe, el del sistema.
pub fn nombre_propio() -> String {
    tailscale::leer(Duration::from_secs(60)).propio.map(|p| p.nombre).unwrap_or_else(plataforma::nombre_equipo)
}

/// El nombre en Tailscale del equipo con esa dirección.
fn nombre_de(ip: Ipv4Addr) -> Option<String> {
    tailscale::leer(Duration::from_secs(60)).nombre_de(&ip.to_string())
}

fn notificar(app: &AppHandle, cuerpo: String) {
    let _ = app.notification().builder().title(NOMBRE).body(cuerpo).show();
}

fn avisar_recibido(app: &AppHandle, e: &Entrada) {
    let _ = app.emit("drop-recibido", e);
}

fn archivos_en_palabras(n: u32) -> String {
    if n == 1 { "1 archivo".into() } else { format!("{n} archivos") }
}

/// Terminó de llegar un lote de archivos: la notificación y, si fue una sola imagen, el portapapeles.
fn lote_recibido(app: &AppHandle, e: &Entrada) {
    let copiada = e.tipo == "archivo"
        && e.cantidad == 1
        && COPIAR_IMAGENES.load(Ordering::Relaxed)
        && rutas::es_imagen(&e.nombre)
        && e.ruta.as_deref().is_some_and(|r| plataforma::poner_imagen(Path::new(r)));
    let mensaje = match e.tipo.as_str() {
        "archivo" if copiada => format!("Recibiste {} de {} · ya está en tu portapapeles", e.nombre, e.equipo),
        "archivo" => format!("Recibiste {} de {}", e.nombre, e.equipo),
        "carpeta" => format!("Recibiste la carpeta {} de {} ({})", e.nombre, e.equipo, archivos_en_palabras(e.cantidad)),
        _ => format!("Recibiste {} de {}", archivos_en_palabras(e.cantidad), e.equipo),
    };
    notificar(app, mensaje);
}

/// Llegó un texto: al portapapeles, al historial y una notificación.
fn texto_recibido(app: &AppHandle, equipo: &str, texto: &str, oculto: bool) -> bool {
    let copiado = plataforma::poner_texto(texto);
    let e = Entrada {
        direccion: "recibido".into(),
        tipo: "texto".into(),
        nombre: "Texto".into(),
        cantidad: 1,
        total: 1,
        tamano: texto.len() as u64,
        equipo: equipo.into(),
        texto: (!oculto).then(|| texto.chars().take(historial::MAX_TEXTO).collect()),
        estado: "completo".into(),
        ..Default::default()
    };
    let id = historial::agregar(e, true);
    if let Some(e) = historial::buscar(id) {
        avisar_recibido(app, &e);
    }
    notificar(
        app,
        if copiado {
            format!("Texto recibido de {equipo} · ya está en tu portapapeles")
        } else {
            format!("Texto recibido de {equipo}, pero no pude ponerlo en el portapapeles")
        },
    );
    copiado
}

/// Llegó un enlace: se abre en el navegador, va al historial y se avisa.
fn enlace_recibido(app: &AppHandle, equipo: &str, url: &str) -> Result<(), String> {
    plataforma::abrir_enlace(url)?;
    let e = Entrada {
        direccion: "recibido".into(),
        tipo: "enlace".into(),
        nombre: "Enlace".into(),
        cantidad: 1,
        total: 1,
        tamano: url.len() as u64,
        equipo: equipo.into(),
        texto: Some(url.into()),
        estado: "completo".into(),
        ..Default::default()
    };
    let id = historial::agregar(e, true);
    if let Some(e) = historial::buscar(id) {
        avisar_recibido(app, &e);
    }
    notificar(app, format!("Se abrió un enlace de {equipo}"));
    Ok(())
}

/// Un enlace que se puede abrir: http o https, sin espacios ni caracteres de control, hasta 4096.
pub fn enlace_valido(url: &str) -> Option<String> {
    let url = url.trim();
    let minus = url.to_ascii_lowercase();
    let esquema = minus.starts_with("http://") || minus.starts_with("https://");
    let limpio = !url.chars().any(|c| c.is_whitespace() || c.is_control());
    let con_sitio = url.split_once("://").is_some_and(|(_, resto)| !resto.is_empty() && !resto.starts_with('/'));
    (esquema && limpio && con_sitio && url.len() <= 4096).then(|| url.to_string())
}

/// El destino elegido, si es un equipo del mismo dueño (o este mismo, para probar): su dirección y su nombre.
fn destino_valido(destino: &str) -> Result<(Ipv4Addr, String), String> {
    let ip: Ipv4Addr = destino.trim().parse().map_err(|_| "Ese destino no es una dirección de Tailscale.".to_string())?;
    let texto = ip.to_string();
    let mut info = tailscale::leer(Duration::from_secs(30));
    if info.es_del_dueno(&texto) != Some(true) {
        info = tailscale::leer(Duration::ZERO);
    }
    if info.es_del_dueno(&texto) != Some(true) {
        return Err("Ese equipo no es tuyo en Tailscale: Nexo solo manda a tus equipos.".into());
    }
    let nombre = info
        .nombre_de(&texto)
        .or_else(|| info.propio.as_ref().filter(|p| p.ips.contains(&texto)).map(|p| p.nombre.clone()))
        .unwrap_or(texto);
    Ok((ip, nombre))
}

fn estado(app: &AppHandle) -> EstadoDrop {
    let info = tailscale::leer(Duration::from_secs(20));
    EstadoDrop {
        ajustes: ajustes::cargar(app).drop,
        escuchando: servidor::escuchando().map(|ip| ip.to_string()),
        nombre: info.propio.as_ref().map(|p| p.nombre.clone()).unwrap_or_else(plataforma::nombre_equipo),
        plataforma: PLATAFORMA,
        carpeta: carpeta(app).map(|c| c.display().to_string()),
        tailscale: info.estado,
        puerto: PUERTO,
    }
}

/// Busca los destinos y los recuerda. Si cambió a cuáles se les puede mandar, pone al día el menú del ícono.
pub fn buscar_destinos(app: &AppHandle) -> Vec<Destino> {
    let lista = envio::destinos(app);
    let cambio = {
        let mut guardados = DESTINOS.lock().unwrap_or_else(|e| e.into_inner());
        let cambio = en_linea(&guardados) != en_linea(&lista);
        guardados.clone_from(&lista);
        cambio
    };
    if cambio {
        super::bandeja::actualizar(app);
    }
    lista
}

/// (dirección, nombre) de los destinos donde Nexo respondió.
fn en_linea(lista: &[Destino]) -> Vec<(String, String)> {
    lista.iter().filter(|d| d.en_linea).map(|d| (d.ip.clone(), d.nombre.clone())).collect()
}

/// Los destinos donde Nexo respondió en la última búsqueda. No le pregunta a nadie.
pub fn destinos_en_linea() -> Vec<(String, String)> {
    en_linea(&DESTINOS.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Cómo terminó un envío; `None` mientras sigue en la cola o en curso.
pub fn como_termino(lote: &str) -> Option<FinEnvio> {
    if progreso::en_curso().iter().any(|p| p.lote == lote && p.direccion == "enviado") {
        return None;
    }
    // Al terminar, la entrada del historial se anota antes de soltar el avance.
    Some(match historial::buscar_lote(lote) {
        Some(e) if e.estado == "completo" => FinEnvio::Listo,
        Some(e) if e.estado == "cancelado" => FinEnvio::Cancelado,
        Some(e) => FinEnvio::Fallo(e.error.unwrap_or_else(|| "No se pudo enviar.".into())),
        // Sin entrada: se canceló en la cola o no llegó a salir.
        None => FinEnvio::Cancelado,
    })
}

async fn en_segundo_plano<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- Comandos

/// Los otros equipos del dueño en Tailscale (Mac o Windows), con si Nexo responde en cada uno.
#[tauri::command]
pub async fn drop_destinos(app: AppHandle) -> Vec<Destino> {
    tauri::async_runtime::spawn_blocking(move || buscar_destinos(&app)).await.unwrap_or_default()
}

/// Manda archivos y carpetas a otro equipo. Devuelve el lote; el avance llega por "drop-progreso".
#[tauri::command]
pub async fn drop_enviar(app: AppHandle, rutas: Vec<String>, destino: String) -> Result<Envio, String> {
    en_segundo_plano(move || {
        let (ip, equipo) = destino_valido(&destino)?;
        let rutas: Vec<PathBuf> = rutas.into_iter().filter(|r| !r.trim().is_empty()).map(PathBuf::from).collect();
        if rutas.is_empty() {
            return Err("No hay nada para enviar.".into());
        }
        if let Some(falta) = rutas.iter().find(|r| !r.exists()) {
            let nombre = falta.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return Err(format!("No encontré «{nombre}»."));
        }
        let lote = envio::encolar(&app, rutas, ip, equipo, Vec::new());
        Ok(Envio { lote })
    })
    .await
}

/// Cancela un envío (en curso o en cola).
#[tauri::command]
pub fn drop_cancelar(app: AppHandle, lote: String) {
    envio::cancelar(&app, &lote);
}

/// Lo que se está enviando o recibiendo ahora.
#[tauri::command]
pub fn drop_envios() -> Vec<Progreso> {
    progreso::en_curso()
}

/// Manda un texto: llega al portapapeles del otro equipo.
#[tauri::command]
pub async fn drop_texto(app: AppHandle, destino: String, texto: String) -> Result<(), String> {
    en_segundo_plano(move || {
        if texto.is_empty() {
            return Err("No hay texto para enviar.".into());
        }
        let (ip, equipo) = destino_valido(&destino)?;
        envio::enviar_texto(&app, ip, &equipo, &texto, false)
    })
    .await
}

/// Abre un enlace (http o https) en el navegador del otro equipo.
#[tauri::command]
pub async fn drop_enlace(app: AppHandle, destino: String, url: String) -> Result<(), String> {
    en_segundo_plano(move || {
        let url = enlace_valido(&url).ok_or("Escribe un enlace que empiece con http:// o https://.")?;
        let (ip, equipo) = destino_valido(&destino)?;
        envio::enviar_enlace(&app, ip, &equipo, &url)
    })
    .await
}

/// Manda lo que hay copiado: texto, una imagen (como PNG) o los archivos copiados.
#[tauri::command]
pub async fn drop_enviar_portapapeles(app: AppHandle, destino: String) -> Result<EnvioCopiado, String> {
    en_segundo_plano(move || {
        let (ip, equipo) = destino_valido(&destino)?;
        match plataforma::leer_copiado().ok_or("No hay nada copiado para enviar.")? {
            Copiado::Texto { texto, oculto } => {
                envio::enviar_texto(&app, ip, &equipo, &texto, oculto)?;
                Ok(EnvioCopiado { tipo: "texto", lote: None, cantidad: 1 })
            }
            Copiado::Imagen(png) => {
                let carpeta = std::env::temp_dir().join(format!("nexo-drop-{}", envio::nuevo_lote()));
                let archivo = carpeta.join("Imagen copiada.png");
                std::fs::create_dir_all(&carpeta)
                    .and_then(|_| std::fs::write(&archivo, &png))
                    .map_err(|e| format!("No pude preparar la imagen ({e})."))?;
                let lote = envio::encolar(&app, vec![archivo], ip, equipo, vec![carpeta]);
                Ok(EnvioCopiado { tipo: "imagen", lote: Some(lote), cantidad: 1 })
            }
            Copiado::Archivos(rutas) => {
                let cantidad = rutas.len();
                let lote = envio::encolar(&app, rutas, ip, equipo, Vec::new());
                Ok(EnvioCopiado { tipo: "archivos", lote: Some(lote), cantidad })
            }
        }
    })
    .await
}

/// Mac: eliges una zona de la pantalla (la misma captura liviana del remoto, en 1x) y se manda como PNG.
/// `None` si se canceló con Esc. El avance llega por "drop-progreso".
#[tauri::command]
pub async fn drop_enviar_captura(app: AppHandle, destino: String) -> Result<Option<Envio>, String> {
    en_segundo_plano(move || enviar_captura(&app, &destino)).await
}

#[cfg(target_os = "macos")]
fn enviar_captura(app: &AppHandle, destino: &str) -> Result<Option<Envio>, String> {
    let (ip, equipo) = destino_valido(destino)?;
    let Some((png, _, _)) = super::remoto::captura::tomar()? else { return Ok(None) };
    let carpeta = std::env::temp_dir().join(format!("nexo-drop-{}", envio::nuevo_lote()));
    let archivo = carpeta.join(format!("{}.png", super::remoto::captura::nombre()));
    std::fs::create_dir_all(&carpeta)
        .and_then(|_| std::fs::write(&archivo, &png))
        .map_err(|e| format!("No pude preparar la captura ({e})."))?;
    // La carpeta temporal se borra al terminar el envío.
    let lote = envio::encolar(app, vec![archivo], ip, equipo, vec![carpeta]);
    Ok(Some(Envio { lote }))
}

#[cfg(not(target_os = "macos"))]
fn enviar_captura(_app: &AppHandle, _destino: &str) -> Result<Option<Envio>, String> {
    Err("Enviar una captura funciona desde la Mac. Aquí, cópiala con Win+Shift+S y usa «Enviar lo que copiaste».".into())
}

/// El historial (lo enviado y lo recibido), con miniaturas de las imágenes.
#[tauri::command]
pub async fn drop_historial() -> Vec<EntradaVista> {
    tauri::async_runtime::spawn_blocking(|| historial::vista(plataforma::miniatura)).await.unwrap_or_default()
}

/// Abre lo de esa entrada: el archivo o la carpeta, o el enlace en este equipo.
#[tauri::command]
pub async fn drop_abrir(id: u64) -> Result<(), String> {
    en_segundo_plano(move || {
        let e = historial::buscar(id).ok_or("Eso ya no está en el historial.")?;
        if e.tipo == "enlace" {
            let url = e.texto.as_deref().and_then(enlace_valido).ok_or("Ese enlace no se puede abrir.")?;
            return plataforma::abrir_enlace(&url);
        }
        let ruta = e.ruta.map(PathBuf::from).filter(|r| r.exists()).ok_or("Ya no está donde quedó: lo movieron o lo borraron.")?;
        // Varios archivos sueltos: se abre la carpeta donde quedaron.
        let ruta = match ruta.parent() {
            Some(carpeta) if e.tipo == "varios" => carpeta.to_path_buf(),
            _ => ruta,
        };
        plataforma::abrir(&ruta)
    })
    .await
}

/// Muestra el archivo o la carpeta de esa entrada en Finder o en el Explorador.
#[tauri::command]
pub async fn drop_mostrar(id: u64) -> Result<(), String> {
    en_segundo_plano(move || {
        let e = historial::buscar(id).ok_or("Eso ya no está en el historial.")?;
        let ruta = e.ruta.map(PathBuf::from).filter(|r| r.exists()).ok_or("Ya no está donde quedó: lo movieron o lo borraron.")?;
        plataforma::mostrar(&ruta)
    })
    .await
}

/// Copia el texto o el enlace de esa entrada al portapapeles de este equipo.
#[tauri::command]
pub async fn drop_copiar(id: u64) -> Result<(), String> {
    en_segundo_plano(move || {
        let e = historial::buscar(id).ok_or("Eso ya no está en el historial.")?;
        let texto = e.texto.filter(|t| !t.is_empty()).ok_or("Ese texto no se guardó (parecía una contraseña).")?;
        if plataforma::poner_texto(&texto) { Ok(()) } else { Err("No pude ponerlo en el portapapeles.".into()) }
    })
    .await
}

/// Borra el historial de XDrop. Los archivos recibidos no se tocan.
#[tauri::command]
pub async fn drop_borrar_historial() {
    let _ = tauri::async_runtime::spawn_blocking(historial::borrar).await;
}

/// Abre Descargas › XDrop (la crea si no existe).
#[tauri::command]
pub async fn drop_abrir_carpeta(app: AppHandle) -> Result<(), String> {
    en_segundo_plano(move || {
        let c = carpeta(&app).ok_or("No encontré tu carpeta de Descargas.")?;
        std::fs::create_dir_all(&c).map_err(|e| e.to_string())?;
        plataforma::abrir(&c)
    })
    .await
}

/// Cómo está XDrop en este equipo: ajustes, si escucha y dónde.
#[tauri::command]
pub async fn drop_ajustes(app: AppHandle) -> Result<EstadoDrop, String> {
    en_segundo_plano(move || Ok(estado(&app))).await
}

/// Cambia "Recibir archivos" o "Copiar imágenes recibidas". Devuelve cómo quedó todo.
#[tauri::command]
pub async fn drop_ajustar(app: AppHandle, cambio: CambioDrop) -> Result<EstadoDrop, String> {
    en_segundo_plano(move || {
        let nuevos = ajustes::cambiar(&app, |a| {
            if let Some(v) = cambio.recibir {
                a.drop.recibir = v;
            }
            if let Some(v) = cambio.copiar_imagenes {
                a.drop.copiar_imagenes = v;
            }
        })?
        .drop;
        COPIAR_IMAGENES.store(nuevos.copiar_imagenes, Ordering::Relaxed);
        if cambio.recibir.is_some() {
            servidor::usar(&app, nuevos.recibir);
        }
        Ok(estado(&app))
    })
    .await
}
