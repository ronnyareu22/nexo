//! Mi setup: los aparatos de cada equipo del dueño y el mapa de su escritorio (casa y oficina).
//!
//! - `GET /nexo/aparatos` (en el receptor de XDrop, con su misma seguridad: solo equipos del mismo dueño)
//!   devuelve los aparatos y las pantallas de este equipo. Ver `aparatos_json`.
//! - `setup_estado` junta los de este equipo y los de los otros equipos del dueño que respondan (2 s como mucho,
//!   y la respuesta se recuerda 10 s). Lo último que se vio de cada uno queda en `setup-visto.json`, para
//!   dibujarlo tenue cuando ese equipo está apagado.
//! - El mapa (lo que el dueño acomodó en cada lugar) va en `setup.json`: `{ casa: {items}, oficina: {items} }`.
//!   Un lugar en `null` todavía no se acomodó: la interfaz arma uno con los aparatos que haya.

use super::drop;
use crate::dispositivos::{self, Dispositivo, historial::Historial, pantallas::Pantallas};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const PLAZO_CONECTAR: Duration = Duration::from_millis(1500);
const PLAZO_LEER: Duration = Duration::from_millis(2500);
/// Cuánto se recuerda la respuesta de los otros equipos.
const RECUERDO: Duration = Duration::from_secs(10);
const MAX_RESPUESTA: u64 = 512 * 1024;
const MAX_APARATOS: usize = 64;
const MAX_ITEMS: usize = 60;
const MAX_TEXTO: usize = 200;

/// Los tipos de aparato que entiende la interfaz (`Tipo` en tipos.ts).
const TIPOS: &[&str] =
    &["audifonos", "diadema", "telefono", "teclado", "mouse", "parlante", "monitor", "luz", "portatil", "otro"];

/// Lo que se puede poner en el mapa. "equipo" es la computadora misma (la MacBook o la torre del PC); "hogar", un
/// aparato de la casa (una luz, el enchufe…), por su id de `hogar`.
const TIPOS_ITEM: &[&str] = &["aparato", "pantalla", "equipo", "deco", "hogar"];

/// Las decoraciones: fotos en static/setup/<nombre>.png.
pub const DECORACIONES: &[&str] = &[
    "planta-grande",
    "planta-pequena",
    "lampara",
    "taza",
    "cuadro",
    "repisa",
    "tira-luz",
    "soporte-audifonos",
    "alfombrilla",
    "base-monitor",
    "parlante",
];

/// Un aparato Bluetooth, en corto: lo que hace falta para dibujarlo y decir si está encendido.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Aparato {
    pub id: String,
    pub nombre: String,
    #[serde(default)]
    pub alias: Option<String>,
    pub tipo: String,
    pub conectado: bool,
    /// El nivel que lo resume: el general o, en audífonos sueltos, el más bajo de los dos.
    #[serde(default)]
    pub bateria: Option<u8>,
    #[serde(default)]
    pub fabricante: Option<String>,
    #[serde(default)]
    pub vendor_id: Option<String>,
    #[serde(default)]
    pub product_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PantallaSetup {
    #[serde(default)]
    pub id: u32,
    pub nombre: String,
    pub interna: bool,
    #[serde(default)]
    pub brillo: Option<u8>,
}

/// Lo que tiene un equipo: su nombre en Tailscale, su sistema, sus aparatos y sus pantallas.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Aparatos {
    pub equipo: String,
    /// "mac", "windows" u "otro".
    pub plataforma: String,
    #[serde(default)]
    pub dispositivos: Vec<Aparato>,
    #[serde(default)]
    pub pantallas: Vec<PantallaSetup>,
}

/// Otro equipo del dueño y lo que tiene.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EquipoRemoto {
    pub nombre: String,
    pub ip: String,
    pub plataforma: String,
    /// Nexo respondió en ese equipo.
    pub en_linea: bool,
    /// Tailscale lo ve conectado (aunque Nexo no responda).
    pub tailscale_en_linea: bool,
    /// Sus aparatos: los de ahora o, si no responde, los últimos que se vieron.
    pub aparatos: Option<Aparatos>,
    /// `aparatos` es de ahora (false: es lo último que se vio).
    pub al_dia: bool,
    /// Cuándo se vieron esos aparatos (segundos desde 1970).
    pub visto: Option<u64>,
    /// Por qué no se pudieron leer ahora, en palabras.
    pub error: Option<String>,
}

/// `setup_estado`: este equipo y los otros.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoSetup {
    pub local: Aparatos,
    pub remotos: Vec<EquipoRemoto>,
}

// ---------------------------------------------------------------- Mapa

/// Los dos lugares. `None`: todavía no se acomodó (la interfaz arma uno con lo que haya).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Mapa {
    pub casa: Option<Lugar>,
    pub oficina: Option<Lugar>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Lugar {
    pub items: Vec<Item>,
}

/// Algo puesto en el mapa. Las posiciones van en % de la escena, así se ve igual con cualquier tamaño de ventana.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    /// "aparato", "pantalla", "equipo", "deco" u "hogar".
    pub tipo: String,
    /// La dirección del aparato, el nombre de la pantalla, el nombre del equipo o la clase de decoración.
    #[serde(rename = "ref")]
    pub referencia: String,
    /// Centro, de 0 a 100 a lo ancho.
    pub x: f64,
    /// Solo lo de la pared: dónde apoya, de 0 a 100 a lo alto. Lo del escritorio va por su capa.
    #[serde(default)]
    pub y: Option<f64>,
    /// 0 fondo, 1 medio, 2 frente.
    #[serde(default)]
    pub capa: u8,
    #[serde(default = "escala_normal")]
    pub escala: f64,
    /// El equipo al que pertenece (su nombre en Tailscale). Vacío en las decoraciones.
    #[serde(default)]
    pub equipo: Option<String>,
}

fn escala_normal() -> f64 {
    1.0
}

// ---------------------------------------------------------------- Este equipo

fn segundos() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Sin caracteres de control y con un largo máximo.
fn texto_limpio(t: &str, max: usize) -> String {
    t.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

fn tipo_de(t: dispositivos::Tipo) -> String {
    serde_json::to_value(t).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "otro".into())
}

/// El nivel que resume la batería: el general o el más bajo de los dos lados.
fn nivel(general: Option<u8>, izquierda: Option<u8>, derecha: Option<u8>) -> Option<u8> {
    general.or(match (izquierda, derecha) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    })
}

fn aparato(d: &Dispositivo) -> Aparato {
    Aparato {
        id: d.id.clone(),
        nombre: d.nombre.clone(),
        alias: d.alias.clone(),
        tipo: tipo_de(d.tipo),
        conectado: d.conectado,
        bateria: nivel(d.bateria, d.bateria_izquierda, d.bateria_derecha),
        fabricante: d.fabricante.clone(),
        vendor_id: d.vendor_id.clone(),
        product_id: d.product_id.clone(),
    }
}

/// Los aparatos y las pantallas de este equipo. Tarda un poco (system_profiler, DDC): fuera del hilo principal.
pub fn leer_aparatos(app: &AppHandle) -> Aparatos {
    let mut lista = dispositivos::listar_ahora().map(|e| e.dispositivos).unwrap_or_default();
    if let Some(h) = app.try_state::<Arc<Historial>>() {
        h.poner_alias_en(&mut lista);
    }
    let pantallas = app.try_state::<Arc<Pantallas>>().map(|p| p.listar()).unwrap_or_default();
    Aparatos {
        equipo: drop::nombre_propio(),
        plataforma: drop::PLATAFORMA.into(),
        dispositivos: lista.iter().map(aparato).collect(),
        // La pantalla virtual de RustDesk no va en el mapa: no está en el escritorio.
        pantallas: pantallas
            .into_iter()
            .filter(|p| !p.es_virtual)
            .map(|p| PantallaSetup { id: p.id, nombre: p.nombre, interna: p.interna, brillo: p.brillo })
            .collect(),
    }
}

/// Lo que responde `GET /nexo/aparatos`. Lo llama el receptor de XDrop con la seguridad ya revisada.
pub fn aparatos_json(app: &AppHandle) -> serde_json::Value {
    serde_json::to_value(leer_aparatos(app)).unwrap_or(serde_json::Value::Null)
}

// ---------------------------------------------------------------- Los otros equipos

/// Lo que llega de otro equipo se limpia: textos cortos, tipos conocidos y listas acotadas.
fn limpiar_aparatos(a: Aparatos) -> Aparatos {
    let opcional = |t: Option<String>| t.map(|t| texto_limpio(&t, MAX_TEXTO)).filter(|t| !t.is_empty());
    Aparatos {
        equipo: texto_limpio(&a.equipo, 64),
        plataforma: match a.plataforma.as_str() {
            "mac" | "windows" => a.plataforma,
            _ => "otro".into(),
        },
        dispositivos: a
            .dispositivos
            .into_iter()
            .filter(|d| !d.id.trim().is_empty())
            .take(MAX_APARATOS)
            .map(|d| Aparato {
                id: texto_limpio(&d.id, MAX_TEXTO),
                nombre: texto_limpio(&d.nombre, MAX_TEXTO),
                alias: opcional(d.alias),
                tipo: if TIPOS.contains(&d.tipo.as_str()) { d.tipo } else { "otro".into() },
                conectado: d.conectado,
                bateria: d.bateria.map(|b| b.min(100)),
                fabricante: opcional(d.fabricante),
                vendor_id: opcional(d.vendor_id),
                product_id: opcional(d.product_id),
            })
            .collect(),
        pantallas: a
            .pantallas
            .into_iter()
            .take(MAX_APARATOS)
            .map(|p| PantallaSetup {
                id: p.id,
                nombre: texto_limpio(&p.nombre, MAX_TEXTO),
                interna: p.interna,
                brillo: p.brillo.map(|b| b.min(100)),
            })
            .collect(),
    }
}

/// Separa una respuesta HTTP en su código y su cuerpo. También la usa el mando de la Mac (remoto/mando.rs).
pub(super) fn separar_respuesta(datos: &[u8]) -> Option<(u16, &[u8])> {
    let fin = datos.windows(4).position(|v| v == b"\r\n\r\n")?;
    let cabeza = std::str::from_utf8(&datos[..fin]).ok()?;
    let codigo = cabeza.lines().next()?.split(' ').nth(1)?.parse().ok()?;
    Some((codigo, &datos[fin + 4..]))
}

/// Le pide sus aparatos a otro equipo del dueño (`GET /nexo/aparatos`).
fn pedir_aparatos(ip: Ipv4Addr) -> Result<Aparatos, String> {
    let mut s = TcpStream::connect_timeout(&SocketAddr::from((ip, drop::PUERTO)), PLAZO_CONECTAR)
        .map_err(|_| "No respondió.".to_string())?;
    let _ = s.set_read_timeout(Some(PLAZO_LEER));
    let _ = s.set_write_timeout(Some(PLAZO_LEER));
    let pedido = format!(
        "GET /nexo/aparatos HTTP/1.1\r\nHost: {ip}:{}\r\nUser-Agent: Nexo/{}\r\nConnection: close\r\n\r\n",
        drop::PUERTO,
        env!("CARGO_PKG_VERSION")
    );
    s.write_all(pedido.as_bytes()).map_err(|_| "Se cortó la conexión.".to_string())?;
    let mut datos = Vec::new();
    // Si se agota la espera a mitad, se usa lo que llegó (y si no alcanza, cae en "no respondió a tiempo").
    let _ = Read::by_ref(&mut s).take(MAX_RESPUESTA).read_to_end(&mut datos);
    match separar_respuesta(&datos) {
        Some((200, cuerpo)) => serde_json::from_slice::<Aparatos>(cuerpo)
            .map(limpiar_aparatos)
            .map_err(|_| "Su respuesta no se entiende.".into()),
        Some((404, _)) => Err("Tiene una versión anterior de Nexo: actualízala para ver aquí sus aparatos.".into()),
        Some((403, _)) => Err("No aceptó la consulta: revisa que sea tuyo en Tailscale.".into()),
        Some((codigo, _)) => Err(format!("No pudo mostrar sus aparatos ({codigo}).")),
        None => Err("No respondió a tiempo.".into()),
    }
}

/// Lo último que se vio de un equipo.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Visto {
    aparatos: Aparatos,
    momento: u64,
}

static REMOTOS: Mutex<Option<(Instant, Vec<EquipoRemoto>)>> = Mutex::new(None);
static VISTOS: Mutex<Option<HashMap<String, Visto>>> = Mutex::new(None);

fn ruta(app: &AppHandle, archivo: &str) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join(archivo))
}

/// Escribe en un temporal y lo pone en su lugar: si algo falla a mitad, el archivo anterior queda entero.
fn escribir(ruta: Option<PathBuf>, datos: &[u8]) -> Result<(), String> {
    let r = ruta.ok_or("No encontré la carpeta de ajustes.")?;
    if let Some(carpeta) = r.parent() {
        std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
    }
    let temporal = r.with_extension("json.tmp");
    std::fs::write(&temporal, datos).map_err(|e| e.to_string())?;
    std::fs::rename(&temporal, &r).map_err(|e| e.to_string())
}

/// Trabaja con lo último visto (lo lee del disco la primera vez).
fn con_vistos<T>(app: &AppHandle, f: impl FnOnce(&mut HashMap<String, Visto>) -> T) -> T {
    let mut guardia = VISTOS.lock().unwrap_or_else(|e| e.into_inner());
    let mapa = guardia.get_or_insert_with(|| {
        ruta(app, "setup-visto.json")
            .and_then(|r| std::fs::read(r).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    });
    f(mapa)
}

/// Anota lo que tiene un equipo. Se escribe al disco si cambió o si lo guardado tiene más de 5 min.
fn anotar_visto(app: &AppHandle, nombre: &str, aparatos: &Aparatos) {
    let ahora = segundos();
    let json = con_vistos(app, |m| {
        let cambio =
            m.get(nombre).is_none_or(|v| v.aparatos != *aparatos || ahora.saturating_sub(v.momento) > 300);
        m.insert(nombre.to_string(), Visto { aparatos: aparatos.clone(), momento: ahora });
        if cambio { serde_json::to_vec(m).ok() } else { None }
    });
    if let Some(json) = json {
        let _ = escribir(ruta(app, "setup-visto.json"), &json);
    }
}

fn visto(app: &AppHandle, nombre: &str) -> Option<Visto> {
    con_vistos(app, |m| m.get(nombre).cloned())
}

/// Pregunta a cada equipo que responde (todos a la vez) y arma la lista. Lo que no responde va con lo último visto.
fn leer_remotos(app: &AppHandle, destinos: Vec<drop::Destino>) -> Vec<EquipoRemoto> {
    let respuestas: Vec<Option<Result<Aparatos, String>>> = std::thread::scope(|alcance| {
        let hilos: Vec<_> = destinos
            .iter()
            .map(|d| {
                let ip = d.ip.parse::<Ipv4Addr>().ok().filter(|_| d.en_linea);
                std::thread::Builder::new()
                    .name("nexo-setup".into())
                    .spawn_scoped(alcance, move || ip.map(pedir_aparatos))
            })
            .collect();
        hilos.into_iter().map(|h| h.ok().and_then(|h| h.join().ok()).flatten()).collect()
    });
    destinos
        .into_iter()
        .zip(respuestas)
        .map(|(d, respuesta)| {
            let (aparatos, al_dia, visto_en, error) = match respuesta {
                Some(Ok(a)) => {
                    anotar_visto(app, &d.nombre, &a);
                    (Some(a), true, Some(segundos()), None)
                }
                otra => {
                    let error = match otra {
                        Some(Err(e)) => Some(e),
                        _ => None,
                    };
                    let v = visto(app, &d.nombre);
                    (v.as_ref().map(|v| v.aparatos.clone()), false, v.map(|v| v.momento), error)
                }
            };
            EquipoRemoto {
                nombre: d.nombre,
                ip: d.ip,
                plataforma: d.plataforma.to_string(),
                en_linea: d.en_linea,
                tailscale_en_linea: d.tailscale_en_linea,
                aparatos,
                al_dia,
                visto: visto_en,
                error,
            }
        })
        .collect()
}

/// Los otros equipos del dueño (lo de hace menos de 10 s se repite sin preguntar de nuevo).
async fn remotos(app: AppHandle) -> Vec<EquipoRemoto> {
    if let Some((cuando, lista)) = REMOTOS.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
        && cuando.elapsed() < RECUERDO
    {
        return lista.clone();
    }
    let destinos = drop::drop_destinos(app.clone()).await;
    let a = app.clone();
    let lista = tauri::async_runtime::spawn_blocking(move || leer_remotos(&a, destinos)).await.unwrap_or_default();
    *REMOTOS.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), lista.clone()));
    lista
}

// ---------------------------------------------------------------- Mapa guardado

fn numero(v: f64, min: f64, max: f64, si_no: f64) -> f64 {
    if v.is_finite() { v.clamp(min, max) } else { si_no }
}

/// Deja el lugar en orden: tipos conocidos, números dentro de la escena, textos cortos y sin repetidos.
fn limpiar_lugar(l: Lugar) -> Lugar {
    let mut vistos: Vec<String> = Vec::new();
    let mut items = Vec::new();
    for i in l.items {
        let id = texto_limpio(&i.id, 64);
        let referencia = texto_limpio(&i.referencia, MAX_TEXTO);
        let valido = TIPOS_ITEM.contains(&i.tipo.as_str())
            && !id.is_empty()
            && !referencia.is_empty()
            && (i.tipo != "deco" || DECORACIONES.contains(&referencia.as_str()))
            && !vistos.contains(&id);
        if !valido {
            continue;
        }
        vistos.push(id.clone());
        items.push(Item {
            id,
            referencia,
            x: numero(i.x, 0.0, 100.0, 50.0),
            y: i.y.filter(|y| y.is_finite()).map(|y| y.clamp(0.0, 100.0)),
            capa: i.capa.min(2),
            escala: numero(i.escala, 0.5, 2.0, 1.0),
            equipo: i.equipo.map(|e| texto_limpio(&e, 64)).filter(|e| !e.is_empty()),
            tipo: i.tipo,
        });
        if items.len() >= MAX_ITEMS {
            break;
        }
    }
    Lugar { items }
}

fn limpiar_mapa(m: Mapa) -> Mapa {
    Mapa { casa: m.casa.map(limpiar_lugar), oficina: m.oficina.map(limpiar_lugar) }
}

static GUARDANDO: Mutex<()> = Mutex::new(());

fn leer_mapa(app: &AppHandle) -> Mapa {
    ruta(app, "setup.json")
        .and_then(|r| std::fs::read(r).ok())
        .and_then(|b| serde_json::from_slice::<Mapa>(&b).ok())
        .map(limpiar_mapa)
        .unwrap_or_default()
}

// ---------------------------------------------------------------- Comandos

/// Este equipo (nombre, sistema, aparatos y pantallas) y los otros equipos del dueño con lo suyo.
#[tauri::command]
pub async fn setup_estado(app: AppHandle) -> EstadoSetup {
    let a = app.clone();
    let local = tauri::async_runtime::spawn_blocking(move || leer_aparatos(&a)).await.unwrap_or_else(|_| Aparatos {
        equipo: String::new(),
        plataforma: drop::PLATAFORMA.into(),
        dispositivos: Vec::new(),
        pantallas: Vec::new(),
    });
    let remotos = remotos(app).await;
    EstadoSetup { local, remotos }
}

/// El mapa guardado de la casa y la oficina (un lugar en `null` todavía no se acomodó).
#[tauri::command]
pub async fn setup_mapa(app: AppHandle) -> Mapa {
    tauri::async_runtime::spawn_blocking(move || leer_mapa(&app)).await.unwrap_or_default()
}

/// Guarda el mapa (ya limpio) y devuelve cómo quedó.
#[tauri::command]
pub async fn setup_guardar_mapa(app: AppHandle, mapa: Mapa) -> Result<Mapa, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _turno = GUARDANDO.lock().unwrap_or_else(|e| e.into_inner());
        let limpio = limpiar_mapa(mapa);
        let json = serde_json::to_vec_pretty(&limpio).map_err(|e| e.to_string())?;
        escribir(ruta(&app, "setup.json"), &json).map_err(|e| format!("No pude guardar tu setup ({e})."))?;
        Ok(limpio)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn item(id: &str, tipo: &str, referencia: &str) -> Item {
        Item {
            id: id.into(),
            tipo: tipo.into(),
            referencia: referencia.into(),
            x: 50.0,
            y: None,
            capa: 1,
            escala: 1.0,
            equipo: None,
        }
    }

    #[test]
    fn setup_limpia_el_mapa() {
        let mut fuera = item("a", "aparato", "E8:9E:13:61:CE:B7");
        fuera.x = 140.0;
        fuera.capa = 7;
        fuera.escala = f64::NAN;
        fuera.y = Some(-3.0);
        let lugar = Lugar {
            items: vec![
                fuera,
                item("a", "deco", "lampara"),        // id repetido
                item("b", "deco", "cohete"),         // decoración que no existe
                item("c", "nave", "x"),              // tipo desconocido
                item("", "deco", "taza"),            // sin id
                item("d", "deco", "taza"),
            ],
        };
        let limpio = limpiar_mapa(Mapa { casa: Some(lugar), oficina: None });
        let items = limpio.casa.map(|l| l.items).unwrap_or_default();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].x, 100.0);
        assert_eq!(items[0].capa, 2);
        assert_eq!(items[0].escala, 1.0);
        assert_eq!(items[0].y, Some(0.0));
        assert_eq!(items[1].referencia, "taza");
        assert!(limpio.oficina.is_none());
    }

    #[test]
    fn setup_limita_los_items() {
        let items = (0..100).map(|i| item(&format!("i{i}"), "deco", "taza")).collect();
        assert_eq!(limpiar_lugar(Lugar { items }).items.len(), MAX_ITEMS);
    }

    #[test]
    fn setup_lee_el_mapa_con_ref() {
        let json = r#"{"casa":{"items":[{"id":"x","tipo":"deco","ref":"cuadro","x":30,"y":34,"capa":0,"escala":1.2}]},"oficina":null}"#;
        let m: Mapa = serde_json::from_str(json).unwrap_or_default();
        let casa = m.casa.unwrap_or_default();
        assert_eq!(casa.items.len(), 1);
        assert_eq!(casa.items[0].referencia, "cuadro");
        assert_eq!(casa.items[0].y, Some(34.0));
        let texto = serde_json::to_string(&casa.items[0]).unwrap_or_default();
        assert!(texto.contains("\"ref\":\"cuadro\""));
    }

    #[test]
    fn setup_separa_la_respuesta() {
        let r = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(separar_respuesta(r), Some((200, &b"{}"[..])));
        assert_eq!(separar_respuesta(b"HTTP/1.1 404 Not Found\r\n\r\n").map(|r| r.0), Some(404));
        assert_eq!(separar_respuesta(b"HTTP/1.1 200 OK\r\n"), None);
        assert_eq!(separar_respuesta(b""), None);
    }

    #[test]
    fn setup_limpia_lo_que_llega() {
        let a = Aparatos {
            equipo: "pc\u{7}".into(),
            plataforma: "linux".into(),
            dispositivos: vec![
                Aparato {
                    id: "AA:BB".into(),
                    nombre: "Ratón".into(),
                    alias: Some("   ".into()),
                    tipo: "cohete".into(),
                    conectado: true,
                    bateria: Some(250),
                    fabricante: None,
                    vendor_id: None,
                    product_id: None,
                },
                Aparato {
                    id: " ".into(),
                    nombre: "Sin id".into(),
                    alias: None,
                    tipo: "mouse".into(),
                    conectado: false,
                    bateria: None,
                    fabricante: None,
                    vendor_id: None,
                    product_id: None,
                },
            ],
            pantallas: vec![],
        };
        let l = limpiar_aparatos(a);
        assert_eq!(l.equipo, "pc");
        assert_eq!(l.plataforma, "otro");
        assert_eq!(l.dispositivos.len(), 1);
        assert_eq!(l.dispositivos[0].tipo, "otro");
        assert_eq!(l.dispositivos[0].bateria, Some(100));
        assert_eq!(l.dispositivos[0].alias, None);
    }

    #[test]
    fn setup_resume_la_bateria() {
        assert_eq!(nivel(Some(80), Some(10), Some(20)), Some(80));
        assert_eq!(nivel(None, Some(40), Some(30)), Some(30));
        assert_eq!(nivel(None, None, Some(30)), Some(30));
        assert_eq!(nivel(None, None, None), None);
    }
}
