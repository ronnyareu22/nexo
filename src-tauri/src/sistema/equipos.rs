//! Tus equipos y el perfil de trabajo remoto.
//!
//! Lo de Remoto (conectar a la Mac, su pantalla virtual, los atajos de Windows, F13…) sirve cuando hay del otro lado
//! un equipo tuyo que lo complete. El perfil se enciende solo cuando Nexo lo encuentra entre tus equipos de Tailscale:
//! - En Windows, una Mac tuya; en la Mac, una PC con Windows tuya. Primero la que tiene Nexo, luego la que está en
//!   línea y, si no, cualquiera.
//! - El último encontrado se guarda (ajustes.json › equipos): si la Mac duerme, el perfil sigue y se ve apagada.
//! - La persona elige Automático (de fábrica), Siempre o Nunca.
//!
//! `equipos_estado` dice además cómo está Tailscale, este equipo, tus otros equipos (con si Nexo responde en
//! `GET /nexo/hola`, por la búsqueda de XDrop) y RustDesk. Lo que se pregunta a la red se recuerda 30 s (8 s mientras
//! la guía está a la vista). Sin hilos propios: se lee cuando el panel lo pide y en la búsqueda de destinos del menú
//! del ícono (cada 2 min).
//!
//! RustDesk en la Mac: de su configuración solo se mira la línea `direct-server = 'Y'` (el acceso directo por IP).
//! Ese archivo tiene claves: no se guarda, no se anota y no se devuelve nada más.

use super::{ajustes, bandeja, drop as xdrop, remoto, remoto::tailscale};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// Cuánto se recuerda lo leído de la red: de costumbre y con la guía a la vista.
const EDAD: Duration = Duration::from_secs(30);
const EDAD_FRESCA: Duration = Duration::from_secs(8);

const AUTO: &str = "auto";
const SIEMPRE: &str = "siempre";
const NUNCA: &str = "nunca";

/// Lo que se guarda en ajustes.json, dentro de "equipos".
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AjustesEquipos {
    /// "auto" (de fábrica), "siempre" o "nunca". Cualquier otra cosa cuenta como "auto".
    pub modo: String,
    /// El último otro equipo encontrado (la Mac en Windows, la PC en la Mac).
    pub otro: Option<Guardado>,
}

/// El otro equipo, como quedó guardado.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Guardado {
    pub nombre: String,
    pub ip: String,
    /// "mac" o "windows".
    pub plataforma: String,
    /// Nexo respondió allá alguna vez.
    #[serde(default)]
    pub con_nexo: bool,
}

/// Este equipo.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Propio {
    pub nombre: String,
    /// Su dirección de Tailscale, si está conectado.
    pub ip: Option<String>,
    pub plataforma: &'static str,
}

/// Otro equipo tuyo en Tailscale.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Equipo {
    pub nombre: String,
    pub ip: String,
    /// "mac", "windows", "linux" u "otro" (teléfonos, tabletas…).
    pub plataforma: &'static str,
    /// Tailscale lo ve conectado.
    pub en_linea: bool,
    /// Nexo respondió allá.
    pub nexo: bool,
}

/// El otro equipo del perfil.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Otro {
    pub nombre: String,
    pub ip: String,
    pub plataforma: String,
    pub en_linea: bool,
    pub nexo: bool,
    /// Está ahora entre tus equipos de Tailscale (si no, es el que se guardó la última vez).
    pub encontrado: bool,
    /// Nexo respondió allá alguna vez.
    pub con_nexo: bool,
}

/// RustDesk en este equipo.
#[derive(Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RustDesk {
    pub instalado: bool,
    /// Mac: el acceso directo por IP está encendido (Ajustes › Seguridad). En Windows no hace falta: `None`.
    pub acceso_directo: Option<bool>,
}

/// Lo que devuelve `equipos_estado`.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EstadoEquipos {
    /// "activo", "detenido", "sin sesion" o "no instalado".
    pub tailscale: &'static str,
    pub propio: Propio,
    pub equipos: Vec<Equipo>,
    pub otro: Option<Otro>,
    /// "auto", "siempre" o "nunca".
    pub modo: &'static str,
    /// El perfil de trabajo remoto está encendido.
    pub activo: bool,
    pub rustdesk: RustDesk,
}

/// Lo que se le preguntó a la red (lo demás sale de los ajustes en cada lectura).
#[derive(Clone, Debug)]
struct Deteccion {
    tailscale: &'static str,
    propio: Propio,
    equipos: Vec<Equipo>,
    rustdesk: RustDesk,
}

static MEMORIA: Mutex<Option<(Instant, Deteccion)>> = Mutex::new(None);
/// Una detección a la vez: los demás esperan y usan lo recién leído.
static TURNO: Mutex<()> = Mutex::new(());

fn bloquear<T>(m: &'static Mutex<T>) -> std::sync::MutexGuard<'static, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------- Reglas (sin red: tienen pruebas)

fn modo_de(texto: &str) -> &'static str {
    match texto {
        SIEMPRE => SIEMPRE,
        NUNCA => NUNCA,
        _ => AUTO,
    }
}

/// "macOS" → "mac", "windows" → "windows", "linux" → "linux"; el resto (iOS, Android…) → "otro".
fn plataforma_de(sistema: &str) -> &'static str {
    match sistema.to_ascii_lowercase().as_str() {
        "macos" | "mac" => "mac",
        "windows" => "windows",
        "linux" => "linux",
        _ => "otro",
    }
}

/// El otro equipo que busca este: en Windows una Mac, en la Mac una PC con Windows.
fn buscada() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("windows")
    } else if cfg!(windows) {
        Some("mac")
    } else {
        None
    }
}

/// Entre los equipos de esa plataforma: primero con Nexo, luego en línea, luego el guardado y por nombre.
fn elegir<'a>(equipos: &'a [Equipo], plataforma: &str, guardado: Option<&Guardado>) -> Option<&'a Equipo> {
    let es_guardado = |e: &Equipo| guardado.is_some_and(|g| g.ip == e.ip);
    equipos
        .iter()
        .filter(|e| e.plataforma == plataforma)
        .min_by(|a, b| {
            b.nexo
                .cmp(&a.nexo)
                .then(b.en_linea.cmp(&a.en_linea))
                .then(es_guardado(b).cmp(&es_guardado(a)))
                .then(a.nombre.cmp(&b.nombre))
        })
}

/// Si el perfil está encendido según lo guardado: "nunca" no, "siempre" sí y "auto" si hay otro equipo guardado.
/// Lo usa el menú del ícono sin preguntar nada a la red (la detección guarda lo que encuentra).
pub fn perfil_activo(a: &ajustes::Ajustes) -> bool {
    match modo_de(&a.equipos.modo) {
        NUNCA => false,
        SIEMPRE => true,
        _ => a.equipos.otro.as_ref().is_some_and(|o| Some(o.plataforma.as_str()) == buscada()),
    }
}

/// En la configuración de RustDesk, solo la línea `direct-server = 'Y'`. Lo demás ni se mira.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn acceso_directo_en(texto: &str) -> bool {
    texto.lines().any(|l| {
        let Some((clave, valor)) = l.split_once('=') else { return false };
        clave.trim() == "direct-server" && valor.trim().trim_matches(|c| c == '\'' || c == '"') == "Y"
    })
}

// ---------------------------------------------------------------- Lo que se lee

/// Mac: si RustDesk deja entrar directo por IP (Ajustes › Seguridad › "Acceso directo por IP").
#[cfg(target_os = "macos")]
fn acceso_directo_rustdesk() -> bool {
    use std::io::Read;
    let Some(casa) = std::env::var_os("HOME") else { return false };
    let ruta = std::path::Path::new(&casa).join("Library/Preferences/com.carriez.RustDesk/RustDesk2.toml");
    let Ok(archivo) = std::fs::File::open(ruta) else { return false };
    // Es un archivo chico; si fuera enorme, no es el de RustDesk.
    let mut texto = String::new();
    if archivo.take(256 * 1024).read_to_string(&mut texto).is_err() {
        return false;
    }
    acceso_directo_en(&texto)
}

fn leer_rustdesk() -> RustDesk {
    #[cfg(target_os = "macos")]
    let acceso_directo = Some(acceso_directo_rustdesk());
    #[cfg(not(target_os = "macos"))]
    let acceso_directo = None;
    RustDesk { instalado: remoto::rustdesk_instalado(), acceso_directo }
}

/// Pregunta a Tailscale y, a tus equipos en línea, si tienen Nexo (la búsqueda de XDrop, con 1,5 s para conectar).
fn detectar(app: &AppHandle, edad: Duration) -> Deteccion {
    let _turno = bloquear(&TURNO);
    if let Some((cuando, d)) = bloquear(&MEMORIA).as_ref()
        && cuando.elapsed() < edad
    {
        return d.clone();
    }
    let info = tailscale::leer(if edad < EDAD { Duration::from_secs(4) } else { Duration::from_secs(15) });
    // También pone al día los destinos de XDrop (y el menú del ícono si cambiaron).
    let destinos = xdrop::buscar_destinos(app);
    let mut equipos: Vec<Equipo> = info
        .pares_del_dueno()
        .into_iter()
        .filter_map(|n| {
            let ip = n.ipv4()?.to_string();
            let hola = destinos.iter().find(|d| d.ip == ip && d.en_linea);
            Some(Equipo {
                plataforma: hola.map(|d| d.plataforma).filter(|p| *p != "otro").unwrap_or_else(|| plataforma_de(&n.sistema)),
                nexo: hola.is_some(),
                en_linea: n.en_linea,
                nombre: n.nombre,
                ip,
            })
        })
        .collect();
    // Primero los que tienen Nexo, luego los que están en línea y las computadoras antes que los teléfonos.
    equipos.sort_by(|a, b| {
        b.nexo
            .cmp(&a.nexo)
            .then(b.en_linea.cmp(&a.en_linea))
            .then((a.plataforma == "otro").cmp(&(b.plataforma == "otro")))
            .then(a.nombre.cmp(&b.nombre))
    });
    let propio = Propio {
        nombre: info.propio.as_ref().map(|p| p.nombre.clone()).unwrap_or_else(xdrop::nombre_propio),
        ip: info.propio.as_ref().and_then(|p| p.ipv4()).map(|ip| ip.to_string()),
        plataforma: xdrop::PLATAFORMA,
    };
    let d = Deteccion { tailscale: info.estado, propio, equipos, rustdesk: leer_rustdesk() };
    *bloquear(&MEMORIA) = Some((Instant::now(), d.clone()));
    d
}

/// Junta lo detectado con lo guardado. Si encontró otro equipo (o cambió), lo guarda y avisa.
fn armar(app: &AppHandle, d: Deteccion) -> EstadoEquipos {
    let guardados = ajustes::cargar(app).equipos;
    let buscada = buscada();
    let elegido = buscada.and_then(|p| elegir(&d.equipos, p, guardados.otro.as_ref()));
    let otro = match elegido {
        Some(e) => {
            let antes = guardados.otro.as_ref().filter(|g| g.ip == e.ip);
            let nuevo = Guardado {
                nombre: e.nombre.clone(),
                ip: e.ip.clone(),
                plataforma: e.plataforma.into(),
                con_nexo: e.nexo || antes.is_some_and(|g| g.con_nexo),
            };
            if guardados.otro.as_ref() != Some(&nuevo) {
                let se_guardo = ajustes::cambiar(app, |a| a.equipos.otro = Some(nuevo.clone())).is_ok();
                if se_guardo {
                    avisar(app);
                }
            }
            Some(Otro {
                nombre: nuevo.nombre,
                ip: nuevo.ip,
                plataforma: nuevo.plataforma,
                en_linea: e.en_linea,
                nexo: e.nexo,
                encontrado: true,
                con_nexo: nuevo.con_nexo,
            })
        }
        // No está ahora (Tailscale apagado, la Mac fuera de la cuenta…): el que se guardó, apagado.
        None => guardados.otro.as_ref().filter(|g| Some(g.plataforma.as_str()) == buscada).map(|g| Otro {
            nombre: g.nombre.clone(),
            ip: g.ip.clone(),
            plataforma: g.plataforma.clone(),
            en_linea: false,
            nexo: false,
            encontrado: false,
            con_nexo: g.con_nexo,
        }),
    };
    let modo = modo_de(&guardados.modo);
    let activo = match modo {
        NUNCA => false,
        SIEMPRE => true,
        _ => otro.is_some(),
    };
    EstadoEquipos {
        tailscale: d.tailscale,
        propio: d.propio,
        equipos: d.equipos,
        otro,
        modo,
        activo,
        rustdesk: d.rustdesk,
    }
}

/// Cambió el perfil (otro equipo guardado o el modo): el menú del ícono y las ventanas se ponen al día.
fn avisar(app: &AppHandle) {
    bandeja::actualizar(app);
    let _ = app.emit("equipos-cambio", ());
}

fn estado(app: &AppHandle, edad: Duration) -> EstadoEquipos {
    armar(app, detectar(app, edad))
}

/// Lo que se recuerda de la red, sin volver a preguntar (o una detección nueva si no hay nada).
fn estado_sin_preguntar(app: &AppHandle) -> EstadoEquipos {
    let guardado = bloquear(&MEMORIA).as_ref().map(|(_, d)| d.clone());
    match guardado {
        Some(d) => armar(app, d),
        None => estado(app, EDAD),
    }
}

/// Que la próxima lectura vuelva a preguntar.
fn olvidar_lo_leido() {
    *bloquear(&MEMORIA) = None;
}

// ---------------------------------------------------------------- Para los demás módulos

/// Cada 2 min, desde el menú del ícono (en su hilo): busca los destinos de XDrop y, si aparece el otro equipo,
/// lo guarda.
pub fn revisar(app: &AppHandle) {
    let _ = estado(app, EDAD);
}

/// Windows: tu Mac (nombre y dirección de Tailscale). Primero la guardada; si no hay, la busca.
#[cfg(windows)]
pub fn mac(app: &AppHandle) -> Option<(String, String)> {
    let de_ajustes = |app: &AppHandle| {
        ajustes::cargar(app).equipos.otro.filter(|o| o.plataforma == "mac").map(|o| (o.nombre, o.ip))
    };
    de_ajustes(app).or_else(|| {
        revisar(app);
        de_ajustes(app)
    })
}

/// Abre un enlace de la guía en el navegador. Solo estos tres: la interfaz no manda direcciones sueltas.
fn enlace(cual: &str) -> Option<&'static str> {
    match cual {
        "tailscale" => Some("https://tailscale.com/download"),
        "nexo" => Some("https://github.com/ronnyareu22/nexo/releases/latest"),
        "rustdesk" => Some("https://rustdesk.com/"),
        _ => None,
    }
}

async fn en_segundo_plano<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- Comandos

/// Tailscale, este equipo, tus otros equipos (con Nexo o no), el otro equipo del perfil, el modo y RustDesk.
/// Con `fresco` (la guía a la vista), lo de la red se recuerda solo 8 s; si no, 30 s.
#[tauri::command]
pub async fn equipos_estado(app: AppHandle, fresco: Option<bool>) -> Result<EstadoEquipos, String> {
    let edad = if fresco.unwrap_or(false) { EDAD_FRESCA } else { EDAD };
    en_segundo_plano(move || Ok(estado(&app, edad))).await
}

/// Cambia el modo del trabajo remoto: "auto", "siempre" o "nunca".
#[tauri::command]
pub async fn equipos_modo(app: AppHandle, modo: String) -> Result<EstadoEquipos, String> {
    en_segundo_plano(move || {
        if !matches!(modo.as_str(), AUTO | SIEMPRE | NUNCA) {
            return Err("Elige Automático, Siempre o Nunca.".into());
        }
        ajustes::cambiar(&app, |a| a.equipos.modo = modo)?;
        avisar(&app);
        Ok(estado_sin_preguntar(&app))
    })
    .await
}

/// Olvida el otro equipo guardado. Si sigue entre tus equipos de Tailscale, se vuelve a encontrar.
#[tauri::command]
pub async fn equipos_olvidar(app: AppHandle) -> Result<EstadoEquipos, String> {
    en_segundo_plano(move || {
        ajustes::cambiar(&app, |a| a.equipos.otro = None)?;
        avisar(&app);
        Ok(estado_sin_preguntar(&app))
    })
    .await
}

/// "Abrir Tailscale": abre su app (ahí se entra con la cuenta) y, si ya hay sesión pero está desconectado, lo
/// conecta (`tailscale up`, hasta 8 s). Devuelve cómo quedó todo.
#[tauri::command]
pub async fn equipos_abrir_tailscale(app: AppHandle) -> Result<EstadoEquipos, String> {
    en_segundo_plano(move || {
        tailscale::abrir()?;
        if tailscale::leer(Duration::ZERO).estado == "detenido" {
            tailscale::conectar();
        }
        olvidar_lo_leido();
        Ok(estado(&app, EDAD_FRESCA))
    })
    .await
}

/// Abre en el navegador la descarga de "tailscale", "nexo" o "rustdesk".
#[tauri::command]
pub async fn equipos_abrir_enlace(cual: String) -> Result<(), String> {
    en_segundo_plano(move || {
        let url = enlace(&cual).ok_or("No conozco ese enlace.")?;
        xdrop::abrir_en_navegador(url)
    })
    .await
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn equipo(nombre: &str, ip: &str, plataforma: &'static str, en_linea: bool, nexo: bool) -> Equipo {
        Equipo { nombre: nombre.into(), ip: ip.into(), plataforma, en_linea, nexo }
    }

    fn guardado(ip: &str) -> Guardado {
        Guardado { nombre: "mac-vieja".into(), ip: ip.into(), plataforma: "mac".into(), con_nexo: true }
    }

    #[test]
    fn equipos_elige_con_nexo_luego_en_linea_luego_cualquiera() {
        let lista = vec![
            equipo("b-apagada", "100.64.0.4", "mac", false, false),
            equipo("a-en-linea", "100.64.0.3", "mac", true, false),
            equipo("pc", "100.64.0.2", "windows", true, true),
        ];
        assert_eq!(elegir(&lista, "mac", None).map(|e| e.nombre.as_str()), Some("a-en-linea"));
        let mut con_nexo = lista.clone();
        con_nexo.push(equipo("z-con-nexo", "100.64.0.5", "mac", true, true));
        assert_eq!(elegir(&con_nexo, "mac", None).map(|e| e.nombre.as_str()), Some("z-con-nexo"));
        // Solo apagadas: cualquiera, y la guardada antes que las demás.
        let apagadas = vec![equipo("a", "100.64.0.6", "mac", false, false), equipo("b", "100.64.0.7", "mac", false, false)];
        assert_eq!(elegir(&apagadas, "mac", None).map(|e| e.nombre.as_str()), Some("a"));
        assert_eq!(elegir(&apagadas, "mac", Some(&guardado("100.64.0.7"))).map(|e| e.nombre.as_str()), Some("b"));
        // Sin Mac, nada (la PC no cuenta).
        assert!(elegir(&lista[2..], "mac", None).is_none());
        assert_eq!(elegir(&lista, "windows", None).map(|e| e.nombre.as_str()), Some("pc"));
    }

    #[test]
    fn equipos_modo_y_perfil_guardado() {
        assert_eq!(modo_de("siempre"), "siempre");
        assert_eq!(modo_de("nunca"), "nunca");
        assert_eq!(modo_de(""), "auto");
        assert_eq!(modo_de("cualquier cosa"), "auto");
        let mut a = ajustes::Ajustes::default();
        assert!(!perfil_activo(&a));
        a.equipos.modo = "siempre".into();
        assert!(perfil_activo(&a));
        a.equipos.modo = "nunca".into();
        a.equipos.otro = Some(Guardado { plataforma: buscada().unwrap_or("mac").into(), ..guardado("100.64.0.3") });
        assert!(!perfil_activo(&a));
        a.equipos.modo = "auto".into();
        assert_eq!(perfil_activo(&a), buscada().is_some());
    }

    #[test]
    fn equipos_plataformas_en_palabras() {
        assert_eq!(plataforma_de("macOS"), "mac");
        assert_eq!(plataforma_de("windows"), "windows");
        assert_eq!(plataforma_de("linux"), "linux");
        assert_eq!(plataforma_de("iOS"), "otro");
        assert_eq!(plataforma_de(""), "otro");
    }

    #[test]
    fn equipos_acceso_directo_de_rustdesk() {
        assert!(acceso_directo_en("[options]\ndirect-server = 'Y'\n"));
        assert!(acceso_directo_en("direct-server=\"Y\""));
        assert!(!acceso_directo_en("[options]\ndirect-server = 'N'\n"));
        assert!(!acceso_directo_en("[options]\nallow-remote-config-modification = 'Y'\n"));
        assert!(!acceso_directo_en(""));
    }

    #[test]
    fn equipos_ajustes_viejos_se_leen() {
        // Un ajustes.json de antes (sin "equipos") y uno con un modo desconocido: los dos se leen.
        let viejo: ajustes::Ajustes = serde_json::from_str("{\"widget\": true}").expect("se lee");
        assert_eq!(viejo.equipos, AjustesEquipos::default());
        let raro: ajustes::Ajustes =
            serde_json::from_str("{\"equipos\": {\"modo\": \"otro\", \"otro\": null}}").expect("se lee");
        assert_eq!(modo_de(&raro.equipos.modo), "auto");
    }

    #[test]
    fn equipos_solo_enlaces_conocidos() {
        assert_eq!(enlace("tailscale"), Some("https://tailscale.com/download"));
        assert!(enlace("https://ejemplo.com").is_none());
    }
}
