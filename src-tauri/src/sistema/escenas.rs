//! Escenas: varias acciones de Nexo con un solo toque ("Modo estudio": brillo, audífonos y volumen).
//!
//! - Se guardan en `escenas.json`. La primera vez se crean cuatro con los aparatos que haya (Modo estudio,
//!   remoto, noche y reunión); si un aparato no está emparejado, esa acción no va.
//! - Solo hacen lo que Nexo ya controla en este equipo. `luces` queda a la espera: nunca se ejecuta.
//! - `activar_escena` corre las acciones en orden, fuera del hilo principal, y devuelve cómo salió cada una.
//!   Avisa el avance con "escena-progreso" y el final con "escena-activa". Una escena a la vez.
//! - La bandeja tiene un submenú "Escenas" que se rehace cuando cambian.

use crate::dispositivos;
use crate::dispositivos::pantallas::Pantallas;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::menu::{MenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_notification::NotificationExt;

const MAX_PASOS: usize = 20;
const MAX_ESCENAS: usize = 24;
const MAX_NOMBRE: usize = 40;

/// Los íconos que se pueden elegir (los mismos nombres de `Icono.svelte`).
const ICONOS: &[&str] = &[
    "audio", "remoto", "luna", "camara", "microfono", "sol", "portatil", "pantallas", "teclado", "rayo",
    "bombilla", "reloj", "inicio", "setup",
];

/// Un paso de una escena.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum Accion {
    /// `pantalla`: "principal" o "todas".
    Brillo { pantalla: String, valor: u8 },
    /// `dispositivo`: la dirección Bluetooth. `nombre` es solo para mostrarlo.
    Conectar {
        dispositivo: String,
        #[serde(default)]
        nombre: Option<String>,
    },
    Desconectar {
        dispositivo: String,
        #[serde(default)]
        nombre: Option<String>,
    },
    /// Usar para el sonido: por dirección Bluetooth o por el UID de sonido del sistema.
    Salida {
        dispositivo: String,
        #[serde(default)]
        nombre: Option<String>,
    },
    /// Volumen de la salida en uso.
    Volumen { valor: u8 },
    /// Volumen del micrófono en uso.
    VolumenMicrofono { valor: u8 },
    /// El modo trabajo de Remoto (la Mac no se duerme).
    ModoTrabajo { activo: bool },
    /// Solo abre apps: en Mac con `open -a`; en Windows, el acceso del menú Inicio o el .exe.
    AbrirApp { nombre: String },
    /// Para cuando lleguen las luces: se muestra, pero no se ejecuta.
    Luces {},
    /// Un paso de una versión más nueva de Nexo: se salta.
    #[serde(other)]
    Desconocida,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Escena {
    #[serde(default)]
    pub id: String,
    pub nombre: String,
    #[serde(default)]
    pub icono: String,
    #[serde(default)]
    pub acciones: Vec<Accion>,
}

/// La última escena que se activó.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Ultima {
    pub id: String,
    /// Segundos desde 1970.
    pub momento: u64,
    /// Todos los pasos salieron bien.
    #[serde(default)]
    pub ok: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct Archivo {
    escenas: Vec<Escena>,
    ultima: Option<Ultima>,
}

/// `escenas`, `guardar_escena` y `borrar_escena`: la lista y la última activada.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoEscenas {
    pub escenas: Vec<Escena>,
    pub ultima: Option<Ultima>,
}

/// Cómo salió un paso. `omitida`: no se ejecutó a propósito (las luces, o algo que no es de este sistema).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Resultado {
    pub accion: Accion,
    pub ok: bool,
    pub error: Option<String>,
    pub omitida: bool,
}

/// "escena-progreso": empieza el paso `indice` (desde 0) de `total`.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Avance {
    id: String,
    indice: usize,
    total: usize,
    accion: Accion,
}

/// "escena-activa": terminó, con el resultado de cada paso.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Fin {
    id: String,
    resultados: Vec<Resultado>,
}

enum Paso {
    Hecho,
    Omitido(String),
}

static ESTADO: Mutex<Option<Archivo>> = Mutex::new(None);
/// Un cambio a la vez (leer, cambiar y guardar), así dos cambios seguidos no se pisan.
static CAMBIANDO: Mutex<()> = Mutex::new(());
static CORRIENDO: AtomicBool = AtomicBool::new(false);

fn segundos() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ---------------------------------------------------------------- Validar

/// Sin caracteres de control, sin espacios de más y con un largo máximo.
fn limpiar_texto(t: &str, max: usize) -> String {
    let sin_control: String = t.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    sin_control.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(max).collect::<String>().trim().to_string()
}

/// Un nombre de app que se puede abrir: sin opciones ("-algo"), sin enlaces y sin caracteres de control.
pub fn nombre_app_valido(nombre: &str) -> Option<String> {
    let n = nombre.trim();
    let ok = !n.is_empty()
        && n.chars().count() <= 200
        && !n.starts_with('-')
        && !n.contains("://")
        && !n.chars().any(char::is_control);
    ok.then(|| n.to_string())
}

/// Una dirección Bluetooth: seis pares hexadecimales separados por ":" o "-".
fn es_direccion(t: &str) -> bool {
    let partes: Vec<&str> = t.split([':', '-']).collect();
    partes.len() == 6 && partes.iter().all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
}

fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn nuevo_id() -> String {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("e{ms}")
}

fn limpiar_aparato(dispositivo: &str, nombre: Option<String>) -> Result<(String, Option<String>), String> {
    let d = limpiar_texto(dispositivo, 200);
    if d.is_empty() {
        return Err("Elige el aparato de cada paso.".into());
    }
    Ok((d, nombre.map(|n| limpiar_texto(&n, 80)).filter(|n| !n.is_empty())))
}

fn limpiar_accion(a: Accion) -> Result<Option<Accion>, String> {
    Ok(Some(match a {
        Accion::Brillo { pantalla, valor } => Accion::Brillo {
            pantalla: if pantalla == "principal" { pantalla } else { "todas".into() },
            valor: valor.min(100),
        },
        Accion::Conectar { dispositivo, nombre } => {
            let (dispositivo, nombre) = limpiar_aparato(&dispositivo, nombre)?;
            Accion::Conectar { dispositivo, nombre }
        }
        Accion::Desconectar { dispositivo, nombre } => {
            let (dispositivo, nombre) = limpiar_aparato(&dispositivo, nombre)?;
            Accion::Desconectar { dispositivo, nombre }
        }
        Accion::Salida { dispositivo, nombre } => {
            let (dispositivo, nombre) = limpiar_aparato(&dispositivo, nombre)?;
            Accion::Salida { dispositivo, nombre }
        }
        Accion::Volumen { valor } => Accion::Volumen { valor: valor.min(100) },
        Accion::VolumenMicrofono { valor } => Accion::VolumenMicrofono { valor: valor.min(100) },
        Accion::ModoTrabajo { activo } => Accion::ModoTrabajo { activo },
        Accion::AbrirApp { nombre } => Accion::AbrirApp {
            nombre: nombre_app_valido(&nombre).ok_or("Escribe el nombre de la app que quieres abrir.")?,
        },
        Accion::Luces {} => Accion::Luces {},
        Accion::Desconocida => return Ok(None),
    }))
}

/// Revisa una escena antes de guardarla: nombre, ícono conocido, de 1 a 20 pasos y cada paso con lo suyo.
fn limpiar_escena(e: Escena) -> Result<Escena, String> {
    let nombre = limpiar_texto(&e.nombre, MAX_NOMBRE);
    if nombre.is_empty() {
        return Err("Ponle un nombre a la escena.".into());
    }
    if e.acciones.len() > MAX_PASOS {
        return Err(format!("Una escena tiene como mucho {MAX_PASOS} pasos."));
    }
    let mut acciones = Vec::with_capacity(e.acciones.len());
    for a in e.acciones {
        if let Some(a) = limpiar_accion(a)? {
            acciones.push(a);
        }
    }
    if acciones.is_empty() {
        return Err("Agrega al menos un paso.".into());
    }
    let icono = if ICONOS.contains(&e.icono.as_str()) { e.icono } else { "rayo".into() };
    let id = if id_valido(&e.id) { e.id } else { String::new() };
    Ok(Escena { id, nombre, icono, acciones })
}

// ---------------------------------------------------------------- Las de la primera vez

/// Los audífonos Sony del dueño (WH-CH520), por su nombre o por la marca.
fn es_sony(nombre: &str, vendor: Option<&str>) -> bool {
    let n = nombre.to_lowercase();
    n.contains("wh-ch520") || n.contains("sony") || vendor.is_some_and(|v| v.eq_ignore_ascii_case("0x054c"))
}

/// Las cuatro escenas de la primera vez. Sin los Sony emparejados, sus pasos no van.
fn por_defecto(sony: Option<(String, String)>, mac: bool) -> Vec<Escena> {
    let audio = |pasos: &mut Vec<Accion>| {
        if let Some((id, nombre)) = &sony {
            pasos.push(Accion::Conectar { dispositivo: id.clone(), nombre: Some(nombre.clone()) });
            pasos.push(Accion::Salida { dispositivo: id.clone(), nombre: Some(nombre.clone()) });
        }
    };
    let brillo = |valor| Accion::Brillo { pantalla: "todas".into(), valor };
    let escena = |id: &str, nombre: &str, icono: &str, acciones| Escena {
        id: id.into(),
        nombre: nombre.into(),
        icono: icono.into(),
        acciones,
    };

    let mut estudio = vec![brillo(70)];
    audio(&mut estudio);
    estudio.push(Accion::Volumen { valor: 40 });

    // En Windows el modo trabajo no hace nada (es de la Mac) y bajar el brillo del PC estorba: abre RustDesk.
    let remoto = if mac {
        vec![Accion::ModoTrabajo { activo: true }, brillo(0)]
    } else {
        vec![Accion::AbrirApp { nombre: "RustDesk".into() }]
    };

    let mut reunion = Vec::new();
    audio(&mut reunion);
    reunion.push(Accion::VolumenMicrofono { valor: 80 });
    reunion.push(Accion::Volumen { valor: 60 });

    vec![
        escena("estudio", "Modo estudio", "audio", estudio),
        escena("remoto", "Modo remoto", "remoto", remoto),
        escena("noche", "Modo noche", "luna", vec![brillo(15), Accion::Volumen { valor: 20 }]),
        escena("reunion", "Modo reunión", "camara", reunion),
    ]
}

/// Las de la primera vez con lo que hay emparejado. `false` si no se pudo leer el Bluetooth (no se guardan).
fn primeras() -> (Archivo, bool) {
    let (lista, seguro) = match dispositivos::listar_ahora() {
        Ok(e) => (e.dispositivos, true),
        Err(_) => (Vec::new(), false),
    };
    let sony = lista
        .iter()
        .find(|d| es_sony(&d.nombre, d.vendor_id.as_deref()))
        .map(|d| (d.id.clone(), d.nombre.clone()));
    (Archivo { escenas: por_defecto(sony, cfg!(target_os = "macos")), ultima: None }, seguro)
}

// ---------------------------------------------------------------- Guardar y leer

fn ruta(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("escenas.json"))
}

fn guardar(app: &AppHandle, a: &Archivo) -> Result<(), String> {
    let r = ruta(app).ok_or("No encontré la carpeta de ajustes.")?;
    if let Some(carpeta) = r.parent() {
        std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(a).map_err(|e| e.to_string())?;
    let temporal = r.with_extension("json.tmp");
    std::fs::write(&temporal, json).map_err(|e| e.to_string())?;
    std::fs::rename(&temporal, &r).map_err(|e| e.to_string())
}

/// Las escenas guardadas; la primera vez, las de fábrica.
fn cargar(app: &AppHandle) -> Archivo {
    let mut guardia = ESTADO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(a) = guardia.as_ref() {
        return a.clone();
    }
    let archivo = ruta(app).filter(|r| r.exists());
    let leido = archivo
        .as_ref()
        .and_then(|r| std::fs::read(r).ok())
        .and_then(|b| serde_json::from_slice::<Archivo>(&b).ok());
    let a = match leido {
        Some(mut a) => {
            // Lo que no pasa la revisión (de una versión vieja o editado a mano) se deja afuera.
            a.escenas = a.escenas.into_iter().filter_map(|e| limpiar_escena(e).ok()).filter(|e| !e.id.is_empty()).collect();
            a
        }
        None => {
            let (a, seguro) = primeras();
            // Solo se escriben si se pudo leer el Bluetooth y no había un archivo (uno dañado no se pisa).
            if seguro && archivo.is_none() {
                let _ = guardar(app, &a);
            } else {
                return a;
            }
            a
        }
    };
    *guardia = Some(a.clone());
    a
}

/// Cambia el archivo en un solo paso (leer, cambiar, guardar). Con `bandeja`, rehace el submenú del ícono.
fn cambiar<T>(app: &AppHandle, bandeja: bool, f: impl FnOnce(&mut Archivo) -> Result<T, String>) -> Result<T, String> {
    let _turno = CAMBIANDO.lock().unwrap_or_else(|e| e.into_inner());
    let mut a = cargar(app);
    let r = f(&mut a)?;
    guardar(app, &a).map_err(|e| format!("No pude guardar las escenas ({e})."))?;
    *ESTADO.lock().unwrap_or_else(|e| e.into_inner()) = Some(a);
    if bandeja {
        actualizar_bandeja(app);
    }
    Ok(r)
}

fn estado(a: Archivo) -> EstadoEscenas {
    EstadoEscenas { escenas: a.escenas, ultima: a.ultima }
}

async fn en_hilo<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

async fn dormir(d: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

// ---------------------------------------------------------------- Ejecutar

/// El brillo de la pantalla principal o de todas las que lo dejan cambiar.
fn brillo(app: &AppHandle, cual: &str, valor: u8) -> Result<(), String> {
    let p = app.try_state::<Arc<Pantallas>>().ok_or("Las pantallas todavía no están listas.")?;
    let lista = p.listar();
    let elegidas: Vec<_> = lista.iter().filter(|x| x.controlable && (cual != "principal" || x.principal)).collect();
    if elegidas.is_empty() {
        return Err(if cual == "principal" {
            "La pantalla principal no deja cambiar el brillo.".into()
        } else {
            "Ninguna pantalla deja cambiar el brillo.".into()
        });
    }
    let fallas: Vec<String> = elegidas
        .into_iter()
        .filter_map(|x| p.poner_brillo(x.id, valor).err().map(|e| format!("{}: {e}", x.nombre)))
        .collect();
    if fallas.is_empty() { Ok(()) } else { Err(fallas.join(" · ")) }
}

/// Conecta o desconecta. Si ya está así, no hace nada.
async fn conexion(id: String, conectar: bool) -> Result<Paso, String> {
    let buscado = id.clone();
    let ahora = en_hilo(move || {
        Ok(dispositivos::listar_ahora()
            .ok()
            .map(|e| e.dispositivos.into_iter().find(|d| d.id.eq_ignore_ascii_case(&buscado)).map(|d| d.conectado)))
    })
    .await?;
    match ahora {
        Some(None) => Err("Ese aparato ya no está emparejado en este equipo.".into()),
        Some(Some(c)) if c == conectar => Ok(Paso::Hecho),
        _ => dispositivos::cambiar_conexion(id, conectar).await.map(|_| Paso::Hecho),
    }
}

/// Usar para el sonido. Recién conectado, el sistema tarda unos segundos en crear su salida: se reintenta.
async fn salida(dispositivo: String) -> Result<Paso, String> {
    let mut ultimo = String::new();
    for intento in 0..6 {
        let r = if es_direccion(&dispositivo) {
            let d = dispositivo.clone();
            en_hilo(move || dispositivos::usar_como_salida(&d)).await
        } else {
            dispositivos::usar_dispositivo_audio(dispositivo.clone(), false).await.map(|_| ())
        };
        match r {
            Ok(()) => return Ok(Paso::Hecho),
            Err(e) => ultimo = e,
        }
        if intento < 5 {
            dormir(Duration::from_millis(1500)).await;
        }
    }
    Err(ultimo)
}

/// El volumen de la salida (o del micrófono) que está en uso.
async fn volumen(valor: u8, microfono: bool) -> Result<Paso, String> {
    let lista = dispositivos::dispositivos_audio().await?;
    let uid = lista
        .iter()
        .find(|d| if microfono { d.entrada_en_uso } else { d.salida_en_uso })
        .map(|d| d.uid.clone())
        .ok_or(if microfono { "No hay un micrófono en uso." } else { "No hay una salida de sonido en uso." })?;
    // El cambio es un tipo interno de `dispositivos`: se arma con el mismo JSON que manda el panel.
    let accion = if microfono { "volumenMicrofono" } else { "volumen" };
    let cambio = serde_json::from_value(json!({ "accion": accion, "valor": valor.min(100) })).map_err(|e| e.to_string())?;
    dispositivos::ajustar_dispositivo_audio(uid, cambio).await.map(|_| Paso::Hecho)
}

async fn modo_trabajo(app: &AppHandle, activo: bool) -> Result<Paso, String> {
    if !cfg!(target_os = "macos") {
        return Ok(Paso::Omitido("El modo trabajo es de la Mac.".into()));
    }
    let cambio = super::remoto::CambioRemoto { modo_trabajo: Some(activo), ..Default::default() };
    super::remoto::remoto_ajustar(app.clone(), cambio).await.map(|_| Paso::Hecho)
}

/// Mac: `open -a` solo abre apps (por su nombre o por la ruta a una .app).
#[cfg(target_os = "macos")]
fn abrir_app(nombre: &str) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let n = nombre_app_valido(nombre).ok_or("Ese nombre de app no sirve.")?;
    let estado = Command::new("/usr/bin/open")
        .args(["-a", &n])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if estado.success() { Ok(()) } else { Err(format!("No encontré «{n}» en Aplicaciones.")) }
}

/// Windows: una ruta a un .exe o .lnk, el acceso directo del menú Inicio con ese nombre o, si no, `<nombre>.exe`
/// (Windows lo busca en App Paths). Nunca scripts.
#[cfg(windows)]
fn abrir_app(nombre: &str) -> Result<(), String> {
    use std::path::Path;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, PCWSTR, w};

    let n = nombre_app_valido(nombre).ok_or("Ese nombre de app no sirve.")?;
    let extension_ok = |r: &Path| {
        r.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| ["exe", "lnk", "appref-ms"].contains(&e.to_ascii_lowercase().as_str()))
    };
    let ruta = Path::new(&n);
    let destino: PathBuf = if ruta.is_absolute() {
        if !extension_ok(ruta) || !ruta.is_file() {
            return Err(format!("«{n}» no es una app."));
        }
        ruta.to_path_buf()
    } else if n.contains(['\\', '/']) {
        return Err("Escribe el nombre de la app o su ruta completa.".into());
    } else {
        let buscado = n.trim_end_matches(".lnk").to_lowercase();
        menu_inicio(&buscado).unwrap_or_else(|| {
            let exe = if buscado.ends_with(".exe") { buscado.clone() } else { format!("{buscado}.exe") };
            PathBuf::from(exe)
        })
    };
    let r = unsafe {
        ShellExecuteW(None, w!("open"), &HSTRING::from(destino.as_os_str()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL)
    };
    // Devuelve un número mayor que 32 si pudo.
    if r.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!("No encontré «{n}». Escribe su nombre como aparece en el menú Inicio."))
    }
}

/// Busca "<nombre>.lnk" en el menú Inicio (de todos y de esta cuenta), hasta tres carpetas adentro.
#[cfg(windows)]
fn menu_inicio(nombre: &str) -> Option<PathBuf> {
    fn buscar(carpeta: &std::path::Path, archivo: &str, hondura: u8) -> Option<PathBuf> {
        let entradas = std::fs::read_dir(carpeta).ok()?;
        let mut carpetas = Vec::new();
        for e in entradas.flatten() {
            let Ok(tipo) = e.file_type() else { continue };
            if tipo.is_dir() {
                carpetas.push(e.path());
            } else if e.file_name().to_string_lossy().to_lowercase() == archivo {
                return Some(e.path());
            }
        }
        if hondura == 0 {
            return None;
        }
        carpetas.iter().find_map(|c| buscar(c, archivo, hondura - 1))
    }
    let archivo = format!("{nombre}.lnk");
    ["ProgramData", "APPDATA"]
        .iter()
        .filter_map(|v| std::env::var_os(v))
        .map(|base| PathBuf::from(base).join("Microsoft").join("Windows").join("Start Menu").join("Programs"))
        .find_map(|c| buscar(&c, &archivo, 3))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn abrir_app(_nombre: &str) -> Result<(), String> {
    Err("Abrir apps no está disponible en este sistema.".into())
}

async fn ejecutar(app: &AppHandle, accion: &Accion) -> Result<Paso, String> {
    match accion.clone() {
        Accion::Brillo { pantalla, valor } => {
            let a = app.clone();
            en_hilo(move || brillo(&a, &pantalla, valor)).await.map(|_| Paso::Hecho)
        }
        Accion::Conectar { dispositivo, .. } => conexion(dispositivo, true).await,
        Accion::Desconectar { dispositivo, .. } => conexion(dispositivo, false).await,
        Accion::Salida { dispositivo, .. } => salida(dispositivo).await,
        Accion::Volumen { valor } => volumen(valor, false).await,
        Accion::VolumenMicrofono { valor } => volumen(valor, true).await,
        Accion::ModoTrabajo { activo } => modo_trabajo(app, activo).await,
        Accion::AbrirApp { nombre } => en_hilo(move || abrir_app(&nombre)).await.map(|_| Paso::Hecho),
        Accion::Luces {} => Ok(Paso::Omitido("Disponible cuando conectes tus luces.".into())),
        Accion::Desconocida => Ok(Paso::Omitido("Este paso es de una versión más nueva de Nexo.".into())),
    }
}

/// Suelta la marca de "activando" al terminar, pase lo que pase.
struct Turno;

impl Drop for Turno {
    fn drop(&mut self) {
        CORRIENDO.store(false, Ordering::SeqCst);
    }
}

/// Corre la escena paso a paso y devuelve cómo salió cada uno.
pub async fn activar(app: AppHandle, id: String) -> Result<(String, Vec<Resultado>), String> {
    let a = app.clone();
    let archivo = en_hilo(move || Ok(cargar(&a))).await?;
    let escena = archivo.escenas.into_iter().find(|e| e.id == id).ok_or("Esa escena ya no existe.")?;
    if CORRIENDO.swap(true, Ordering::SeqCst) {
        return Err("Ya se está activando una escena; espera a que termine.".into());
    }
    let _turno = Turno;
    let total = escena.acciones.len();
    let mut resultados = Vec::with_capacity(total);
    for (indice, accion) in escena.acciones.iter().enumerate() {
        let _ = app.emit("escena-progreso", Avance { id: id.clone(), indice, total, accion: accion.clone() });
        let (ok, error, omitida) = match ejecutar(&app, accion).await {
            Ok(Paso::Hecho) => (true, None, false),
            Ok(Paso::Omitido(motivo)) => (true, Some(motivo), true),
            Err(e) => (false, Some(e), false),
        };
        resultados.push(Resultado { accion: accion.clone(), ok, error, omitida });
    }
    let ultima = Ultima { id: id.clone(), momento: segundos(), ok: resultados.iter().all(|r| r.ok) };
    let a = app.clone();
    let _ = en_hilo(move || {
        cambiar(&a, false, |archivo| {
            archivo.ultima = Some(ultima);
            Ok(())
        })
    })
    .await;
    let _ = app.emit("escena-activa", Fin { id, resultados: resultados.clone() });
    Ok((escena.nombre, resultados))
}

// ---------------------------------------------------------------- Bandeja

/// El submenú "Escenas" del ícono, para rehacerlo cuando cambian.
pub struct MenuEscenas(pub Submenu<Wry>);

/// El submenú vacío; se llena enseguida en otro hilo (la primera vez puede tener que leer el Bluetooth).
pub fn submenu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let s = Submenu::with_id(app, "escenas", "Escenas", true)?;
    app.manage(MenuEscenas(s.clone()));
    let a = app.clone();
    let _ = std::thread::Builder::new().name("nexo-escenas".into()).spawn(move || actualizar_bandeja(&a));
    Ok(s)
}

/// Rehace el submenú con las escenas de ahora.
fn actualizar_bandeja(app: &AppHandle) {
    let Some(menu) = app.try_state::<MenuEscenas>() else { return };
    let escenas = cargar(app).escenas;
    while let Ok(Some(_)) = menu.0.remove_at(0) {}
    if escenas.is_empty() {
        if let Ok(vacio) = MenuItem::with_id(app, "escena-vacia", "Todavía no hay escenas", false, None::<&str>) {
            let _ = menu.0.append(&vacio);
        }
        return;
    }
    for e in escenas {
        if let Ok(item) = MenuItem::with_id(app, format!("escena:{}", e.id), &e.nombre, true, None::<&str>) {
            let _ = menu.0.append(&item);
        }
    }
}

/// Un paso en palabras, para la notificación ("Conectar WH-CH520").
fn describir(a: &Accion) -> String {
    let aparato = |n: &Option<String>| n.clone().unwrap_or_else(|| "el aparato".into());
    match a {
        Accion::Brillo { valor, .. } => format!("Brillo al {valor} %"),
        Accion::Conectar { nombre, .. } => format!("Conectar {}", aparato(nombre)),
        Accion::Desconectar { nombre, .. } => format!("Desconectar {}", aparato(nombre)),
        Accion::Salida { nombre, .. } => format!("Usar {} para el sonido", aparato(nombre)),
        Accion::Volumen { valor } => format!("Volumen al {valor} %"),
        Accion::VolumenMicrofono { valor } => format!("Micrófono al {valor} %"),
        Accion::ModoTrabajo { activo: true } => "Encender el modo trabajo".into(),
        Accion::ModoTrabajo { activo: false } => "Apagar el modo trabajo".into(),
        Accion::AbrirApp { nombre } => format!("Abrir {nombre}"),
        Accion::Luces {} => "Luces".into(),
        Accion::Desconocida => "Un paso nuevo".into(),
    }
}

/// Se eligió una escena en el menú del ícono: se activa y se avisa con una notificación cómo salió.
pub fn desde_bandeja(app: &AppHandle, id_menu: &str) {
    let Some(id) = id_menu.strip_prefix("escena:").map(str::to_string) else { return };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let (titulo, cuerpo) = match activar(app.clone(), id).await {
            Ok((nombre, resultados)) => {
                let fallas: Vec<String> = resultados
                    .iter()
                    .filter(|r| !r.ok)
                    .map(|r| format!("{}: {}", describir(&r.accion), r.error.clone().unwrap_or_default()))
                    .collect();
                if fallas.is_empty() {
                    (nombre, "Listo.".to_string())
                } else {
                    (nombre, format!("Listo, salvo esto. {}", fallas.join(" · ")))
                }
            }
            Err(e) => ("Escenas".to_string(), e),
        };
        let _ = app.notification().builder().title(titulo).body(cuerpo).show();
    });
}

// ---------------------------------------------------------------- Comandos

/// Las escenas (la primera vez, las de fábrica) y la última que se activó.
#[tauri::command]
pub async fn escenas(app: AppHandle) -> EstadoEscenas {
    tauri::async_runtime::spawn_blocking(move || estado(cargar(&app))).await.unwrap_or(EstadoEscenas {
        escenas: Vec::new(),
        ultima: None,
    })
}

/// Guarda una escena nueva (sin id) o cambia la que tiene ese id. Devuelve la lista.
#[tauri::command]
pub async fn guardar_escena(app: AppHandle, escena: Escena) -> Result<EstadoEscenas, String> {
    en_hilo(move || {
        let limpia = limpiar_escena(escena)?;
        cambiar(&app, true, |a| {
            match a.escenas.iter_mut().find(|e| !limpia.id.is_empty() && e.id == limpia.id) {
                Some(e) => *e = limpia,
                None => {
                    if a.escenas.len() >= MAX_ESCENAS {
                        return Err(format!("Puedes tener hasta {MAX_ESCENAS} escenas."));
                    }
                    let id = if limpia.id.is_empty() { nuevo_id() } else { limpia.id.clone() };
                    a.escenas.push(Escena { id, ..limpia });
                }
            }
            Ok(())
        })?;
        Ok(estado(cargar(&app)))
    })
    .await
}

/// Borra una escena. Devuelve la lista.
#[tauri::command]
pub async fn borrar_escena(app: AppHandle, id: String) -> Result<EstadoEscenas, String> {
    en_hilo(move || {
        cambiar(&app, true, |a| {
            a.escenas.retain(|e| e.id != id);
            if a.ultima.as_ref().is_some_and(|u| u.id == id) {
                a.ultima = None;
            }
            Ok(())
        })?;
        Ok(estado(cargar(&app)))
    })
    .await
}

/// Activa una escena: corre sus pasos en orden y devuelve cómo salió cada uno.
#[tauri::command]
pub async fn activar_escena(app: AppHandle, id: String) -> Result<Vec<Resultado>, String> {
    activar(app, id).await.map(|(_, r)| r)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn escenas_de_fabrica_con_sony() {
        let lista = por_defecto(Some(("E8:9E:13:61:CE:B7".into(), "WH-CH520".into())), true);
        assert_eq!(lista.len(), 4);
        let estudio = &lista[0];
        assert_eq!(estudio.nombre, "Modo estudio");
        assert_eq!(estudio.acciones.len(), 4);
        assert!(matches!(estudio.acciones[1], Accion::Conectar { .. }));
        assert!(matches!(estudio.acciones[2], Accion::Salida { .. }));
        assert_eq!(lista[1].acciones[0], Accion::ModoTrabajo { activo: true });
        assert_eq!(lista[3].acciones.len(), 4);
    }

    #[test]
    fn escenas_de_fabrica_sin_sony_ni_mac() {
        let lista = por_defecto(None, false);
        assert_eq!(lista[0].acciones.len(), 2);
        assert_eq!(lista[1].acciones, vec![Accion::AbrirApp { nombre: "RustDesk".into() }]);
        assert_eq!(lista[3].acciones.len(), 2);
        // Todas pasan su propia revisión.
        assert!(lista.into_iter().all(|e| limpiar_escena(e).is_ok()));
    }

    #[test]
    fn escenas_reconoce_los_sony() {
        assert!(es_sony("WH-CH520", None));
        assert!(es_sony("Audífonos", Some("0x054C")));
        assert!(!es_sony("Ronny Beats Solo³", Some("0x004C")));
    }

    #[test]
    fn escenas_revisa_lo_que_llega() {
        let e = Escena {
            id: "../malo".into(),
            nombre: "  Modo\u{7}   cine  ".into(),
            icono: "cohete".into(),
            acciones: vec![
                Accion::Brillo { pantalla: "otra".into(), valor: 250 },
                Accion::Desconocida,
                Accion::Volumen { valor: 101 },
            ],
        };
        let limpia = limpiar_escena(e);
        assert!(limpia.is_ok());
        let limpia = limpia.unwrap_or_else(|_| por_defecto(None, true).remove(0));
        assert_eq!(limpia.nombre, "Modo cine");
        assert_eq!(limpia.icono, "rayo");
        assert_eq!(limpia.id, "");
        assert_eq!(limpia.acciones, vec![Accion::Brillo { pantalla: "todas".into(), valor: 100 }, Accion::Volumen { valor: 100 }]);

        let sin_nombre = Escena { id: String::new(), nombre: " ".into(), icono: String::new(), acciones: vec![] };
        assert!(limpiar_escena(sin_nombre).is_err());
        let sin_pasos = Escena { id: String::new(), nombre: "Algo".into(), icono: String::new(), acciones: vec![] };
        assert!(limpiar_escena(sin_pasos).is_err());
        let sin_aparato = Escena {
            id: String::new(),
            nombre: "Algo".into(),
            icono: String::new(),
            acciones: vec![Accion::Conectar { dispositivo: " ".into(), nombre: None }],
        };
        assert!(limpiar_escena(sin_aparato).is_err());
    }

    #[test]
    fn escenas_lee_el_json_del_panel() {
        let json = r#"{"id":"","nombre":"Cine","icono":"luna","acciones":[
            {"tipo":"brillo","pantalla":"principal","valor":20},
            {"tipo":"volumenMicrofono","valor":50},
            {"tipo":"modoTrabajo","activo":false},
            {"tipo":"abrirApp","nombre":"Spotify"},
            {"tipo":"luces"},
            {"tipo":"algoNuevo","x":1}
        ]}"#;
        let e: Result<Escena, _> = serde_json::from_str(json);
        assert!(e.is_ok());
        let e = e.unwrap_or_else(|_| por_defecto(None, true).remove(0));
        assert_eq!(e.acciones.len(), 6);
        assert_eq!(e.acciones[4], Accion::Luces {});
        assert_eq!(e.acciones[5], Accion::Desconocida);
        let texto = serde_json::to_string(&e.acciones[1]).unwrap_or_default();
        assert_eq!(texto, r#"{"tipo":"volumenMicrofono","valor":50}"#);
    }

    #[test]
    fn escenas_nombres_de_app() {
        assert_eq!(nombre_app_valido(" Spotify "), Some("Spotify".into()));
        assert_eq!(nombre_app_valido("-a"), None);
        assert_eq!(nombre_app_valido("https://x.com"), None);
        assert_eq!(nombre_app_valido(""), None);
        assert_eq!(nombre_app_valido("a\nb"), None);
    }

    #[test]
    fn escenas_reconoce_direcciones() {
        assert!(es_direccion("E8:9E:13:61:CE:B7"));
        assert!(es_direccion("e8-9e-13-61-ce-b7"));
        assert!(!es_direccion("BuiltInSpeakerDevice"));
        assert!(!es_direccion("E8-9E-13-61-CE-B7:output"));
    }
}
