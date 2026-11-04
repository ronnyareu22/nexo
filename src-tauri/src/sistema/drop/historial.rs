//! Nexo Drop: historial de lo enviado y lo recibido (las últimas 40 entradas), en `drop.json` dentro de la
//! carpeta de la app. Un envío de varios archivos es una sola entrada.
//! Las miniaturas de las imágenes se arman al pedir el historial y se recuerdan mientras Nexo corre.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const MAX_ENTRADAS: usize = 40;
/// Los textos se guardan hasta este largo (en caracteres).
pub const MAX_TEXTO: usize = 20_000;
/// Las imágenes más pesadas no tienen miniatura (leerlas tardaría).
const MAX_PESO_MINIATURA: u64 = 60 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Entrada {
    pub id: u64,
    /// "enviado" o "recibido".
    pub direccion: String,
    /// "archivo", "carpeta", "varios", "texto" o "enlace".
    pub tipo: String,
    /// El archivo o la carpeta; en "varios", el primero.
    pub nombre: String,
    /// Cuántos archivos llegaron o salieron.
    pub cantidad: u32,
    /// Cuántos archivos tenía el envío.
    pub total: u32,
    /// Bytes.
    pub tamano: u64,
    /// El otro equipo ("innotech1017").
    pub equipo: String,
    /// Segundos desde 1970.
    pub momento: u64,
    /// Dónde quedó (lo recibido) o de dónde salió (lo enviado, si fue un solo elemento).
    pub ruta: Option<String>,
    /// El texto o el enlace. Vacío si era un texto oculto (una contraseña).
    pub texto: Option<String>,
    /// "completo", "en curso", "cancelado" o "error".
    pub estado: String,
    pub error: Option<String>,
    pub lote: Option<String>,
}

/// Lo que ve la interfaz: la entrada, su miniatura y si su archivo sigue ahí.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EntradaVista {
    #[serde(flatten)]
    pub entrada: Entrada,
    pub miniatura: Option<String>,
    pub existe: bool,
}

static ENTRADAS: Mutex<Vec<Entrada>> = Mutex::new(Vec::new());
static SIGUIENTE: AtomicU64 = AtomicU64::new(1);
static RUTA: Mutex<Option<PathBuf>> = Mutex::new(None);
static MINIATURAS: Mutex<Option<HashMap<u64, Option<String>>>> = Mutex::new(None);

pub fn ahora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn bloquear() -> std::sync::MutexGuard<'static, Vec<Entrada>> {
    ENTRADAS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Lee drop.json al arrancar.
pub fn cargar(app: &AppHandle) {
    let ruta = app.path().app_config_dir().ok().map(|d| d.join("drop.json"));
    let lista: Vec<Entrada> = ruta
        .as_ref()
        .and_then(|r| std::fs::read(r).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mayor = lista.iter().map(|e| e.id).max().unwrap_or(0);
    SIGUIENTE.store(mayor + 1, Ordering::Relaxed);
    // Lo que quedó "en curso" al cerrar Nexo ya no sigue.
    let lista = lista
        .into_iter()
        .map(|mut e| {
            if e.estado == "en curso" {
                e.estado = if e.cantidad > 0 && e.cantidad >= e.total { "completo".into() } else { "cancelado".into() };
            }
            e
        })
        .collect();
    *bloquear() = lista;
    *RUTA.lock().unwrap_or_else(|e| e.into_inner()) = ruta;
}

/// Escribe drop.json (primero a un archivo aparte y luego lo cambia de nombre: nunca queda a medias).
pub fn guardar() {
    let Some(ruta) = RUTA.lock().unwrap_or_else(|e| e.into_inner()).clone() else { return };
    let Ok(json) = serde_json::to_vec(&*bloquear()) else { return };
    if let Some(carpeta) = ruta.parent() {
        let _ = std::fs::create_dir_all(carpeta);
    }
    let temporal = ruta.with_extension("json.tmp");
    if std::fs::write(&temporal, json).is_ok() {
        let _ = std::fs::rename(&temporal, &ruta);
    }
}

/// Agrega una entrada al principio y devuelve su id. Deja las 40 más nuevas.
pub fn agregar(mut e: Entrada, guardar_ya: bool) -> u64 {
    let id = SIGUIENTE.fetch_add(1, Ordering::Relaxed);
    e.id = id;
    if e.momento == 0 {
        e.momento = ahora();
    }
    {
        let mut lista = bloquear();
        lista.insert(0, e);
        lista.truncate(MAX_ENTRADAS);
    }
    if guardar_ya {
        guardar();
    }
    id
}

/// Cambia una entrada (si sigue en el historial).
pub fn cambiar(id: u64, guardar_ya: bool, cambio: impl FnOnce(&mut Entrada)) {
    {
        let mut lista = bloquear();
        let Some(e) = lista.iter_mut().find(|e| e.id == id) else { return };
        cambio(e);
    }
    if let Some(m) = MINIATURAS.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        m.remove(&id);
    }
    if guardar_ya {
        guardar();
    }
}

pub fn buscar(id: u64) -> Option<Entrada> {
    bloquear().iter().find(|e| e.id == id).cloned()
}

/// Borra todo el historial (los archivos recibidos no se tocan).
pub fn borrar() {
    bloquear().clear();
    *MINIATURAS.lock().unwrap_or_else(|e| e.into_inner()) = None;
    guardar();
}

/// El historial para la interfaz, de lo más nuevo a lo más viejo, con miniaturas de las imágenes.
pub fn vista(miniatura: impl Fn(&Path) -> Option<String>) -> Vec<EntradaVista> {
    let lista = bloquear().clone();
    let mut vistas = Vec::with_capacity(lista.len());
    for e in lista {
        let ruta = e.ruta.as_deref().map(Path::new);
        let existe = ruta.is_some_and(|r| r.exists());
        let con_imagen = existe && e.tipo == "archivo" && super::rutas::es_imagen(&e.nombre);
        let mini = if con_imagen { miniatura_de(e.id, ruta, &miniatura) } else { None };
        vistas.push(EntradaVista { entrada: e, miniatura: mini, existe });
    }
    vistas
}

fn miniatura_de(id: u64, ruta: Option<&Path>, hacer: &impl Fn(&Path) -> Option<String>) -> Option<String> {
    if let Some(m) = MINIATURAS.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if let Some(guardada) = m.get(&id) {
            return guardada.clone();
        }
    }
    let ruta = ruta?;
    let liviana = std::fs::metadata(ruta).is_ok_and(|m| m.is_file() && m.len() <= MAX_PESO_MINIATURA);
    let nueva = if liviana { hacer(ruta) } else { None };
    MINIATURAS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(id, nueva.clone());
    nueva
}
