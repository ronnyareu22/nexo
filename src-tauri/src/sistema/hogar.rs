//! La casa: luces, enchufes, la tele, la puerta, cortinas y clima desde un solo lugar ("Casa" en la barra lateral).
//!
//! - Modelo común (`modelo`): clases, capacidades, estado, acciones y órdenes. Lo hablan la interfaz, las escenas,
//!   el menú del ícono y, más adelante, la voz (`hogar_ejecutar` con una `Orden`).
//! - Proveedores: la nube de Tuya (`tuya`), Home Assistant (`ha`), la red local (`lan`: WiZ, Yeelight y Govee se
//!   manejan; Tuya, Hue, LIFX, teles y otros se detectan) y Bluetooth (`ble`: tiras ELK-BLEDOM y Triones).
//! - Las claves (Access Secret de Tuya, token de Home Assistant) van al llavero (`llavero`); lo demás, sin secretos,
//!   en `ajustes.json › hogar`. La lista de Tuya con lo que acepta cada aparato se guarda en `casa.json`.
//! - Nada corre en segundo plano: se lee al abrir Casa o Mi setup (como mucho cada 15 s), tras cada orden y al
//!   buscar (≈6 s, a pedido). Avisa "casa-cambio" cuando algo cambió desde el menú del ícono o una escena.

pub mod ble;
pub mod ejemplo;
pub mod firma;
pub mod ha;
pub mod http;
pub mod lan;
pub mod llavero;
pub mod modelo;
mod nucleo;
pub mod tuya;

pub use nucleo::{aplicar_objetivo, elegir_en_bandeja, menu_bandeja};

use modelo::{Capacidades, Clase, EstadoAparato, FormaLuz, Hallazgo, Orden, Proveedor, Via};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::AppHandle;

// ---------------------------------------------------------------- Lo guardado (sin secretos)

/// La cuenta de Tuya: Access ID y centro de datos. El Access Secret va al llavero.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CuentaTuya {
    pub access_id: String,
    pub region: String,
}

/// Home Assistant: su dirección. El token va al llavero.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CuentaHa {
    pub url: String,
}

/// Un aparato agregado desde una búsqueda (WiZ, Yeelight, Govee o Bluetooth).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Local {
    pub id: String,
    pub proveedor: Proveedor,
    pub nombre: String,
    pub clase: Clase,
    #[serde(default)]
    pub forma: Option<FormaLuz>,
    #[serde(default)]
    pub marca: String,
    #[serde(default)]
    pub modelo: Option<String>,
    /// La IP o el Bluetooth.
    pub direccion: String,
    pub capacidades: Capacidades,
}

/// Lo que la persona cambió de un aparato.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferencias {
    pub alias: Option<String>,
    /// El id de su habitación; `Some("")` lo deja en "Sin cuarto" aunque Home Assistant diga otra.
    pub habitacion: Option<String>,
    pub forma: Option<FormaLuz>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Habitacion {
    #[serde(default)]
    pub id: String,
    pub nombre: String,
}

/// Varios aparatos con un nombre ("Escritorio" = la tira y la barra del monitor).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Grupo {
    #[serde(default)]
    pub id: String,
    pub nombre: String,
    #[serde(default)]
    pub aparatos: Vec<String>,
    #[serde(default)]
    pub habitacion: Option<String>,
}

/// `ajustes.json › hogar`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AjustesHogar {
    pub tuya: Option<CuentaTuya>,
    pub home_assistant: Option<CuentaHa>,
    pub locales: Vec<Local>,
    pub aparatos: BTreeMap<String, Preferencias>,
    /// `None`: las de fábrica (Sala, Cuarto, Oficina, Cocina, Escritorio).
    pub habitaciones: Option<Vec<Habitacion>>,
    pub grupos: Vec<Grupo>,
    /// Lo que no es de la casa (sensores sueltos de Home Assistant) también se ve.
    pub mostrar_todo: bool,
    /// Las luces ya se sumaron a las escenas de fábrica (se hace una sola vez).
    pub escenas_listas: bool,
}

// ---------------------------------------------------------------- Lo que ve la interfaz

/// Un aparato de la casa, ya con su nombre, habitación y estado.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Aparato {
    /// "tuya:<id>", "ha:light.sala", "wiz:<mac>", "ble:<dirección>"…
    pub id: String,
    pub proveedor: Proveedor,
    pub via: Via,
    /// El nombre que se muestra: el alias o el suyo.
    pub nombre: String,
    pub nombre_original: String,
    pub alias: Option<String>,
    pub clase: Clase,
    pub forma: Option<FormaLuz>,
    pub habitacion: Option<String>,
    pub marca: Option<String>,
    pub modelo: Option<String>,
    pub direccion: Option<String>,
    pub en_linea: bool,
    /// Nexo lo maneja (si no, solo se ve: `nota` dice por qué).
    pub controlable: bool,
    pub capacidades: Capacidades,
    pub estado: EstadoAparato,
    /// Solo con "Mostrar todo".
    pub oculto: bool,
    /// Tele: sus entradas y la de ahora.
    pub entradas: Vec<String>,
    pub entrada: Option<String>,
    pub nota: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatosTuya {
    pub access_id: String,
    pub region: String,
    pub region_nombre: String,
}

/// `hogar_estado`.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EstadoCasa {
    pub aparatos: Vec<Aparato>,
    pub habitaciones: Vec<Habitacion>,
    pub grupos: Vec<Grupo>,
    pub tuya: Option<DatosTuya>,
    pub home_assistant: Option<CuentaHa>,
    /// Qué falló al leer cada cuenta, en palabras.
    pub error_tuya: Option<String>,
    pub error_ha: Option<String>,
    /// Segundos desde 1970 de la última lectura.
    pub actualizado: Option<u64>,
    pub mostrar_todo: bool,
    /// Cuántos hay escondidos (para "Mostrar todo").
    pub ocultos: usize,
    /// Datos de ejemplo (`NEXO_CASA=ejemplo`).
    pub ejemplo: bool,
    /// Los centros de datos de Tuya: (clave, nombre).
    pub regiones: Vec<(String, String)>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Falla {
    pub id: String,
    pub nombre: String,
    pub error: String,
}

/// `hogar_ejecutar`: qué se hizo, en palabras (para la notificación o la voz), qué falló y cómo quedó la casa.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoOrden {
    pub ok: bool,
    pub mensaje: String,
    pub hechos: Vec<String>,
    pub fallas: Vec<Falla>,
    /// Hay que confirmarlo (abrir una cerradura): la interfaz pregunta y la manda de nuevo con `confirmado`.
    pub requiere_confirmacion: bool,
    pub estado: EstadoCasa,
}

/// Algo encontrado, y si ya está en Nexo.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HallazgoVista {
    #[serde(flatten)]
    pub hallazgo: Hallazgo,
    pub agregado: bool,
}

/// `hogar_buscar`.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoBusqueda {
    pub hallazgos: Vec<HallazgoVista>,
    /// Lo que no se pudo revisar ("El Bluetooth está apagado.").
    pub avisos: Vec<String>,
    pub segundos: f32,
    pub estado: EstadoCasa,
}

/// Lo que se cambia de un aparato desde la interfaz.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CambioAparato {
    /// `Some("")` vuelve al nombre original.
    pub alias: Option<String>,
    /// `Some("")` lo deja en "Sin cuarto".
    pub habitacion: Option<String>,
    pub forma: Option<FormaLuz>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DatosConexionTuya {
    pub access_id: String,
    pub access_secret: String,
    /// Un centro de datos o "auto" (Detectar).
    pub region: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DatosConexionHa {
    pub url: String,
    pub token: String,
}

// ---------------------------------------------------------------- Comandos

async fn en_hilo<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

/// Cómo está la casa. Con `fresco`, vuelve a preguntar a cada cuenta si pasaron más de 15 s.
#[tauri::command]
pub async fn hogar_estado(app: AppHandle, fresco: bool) -> Result<EstadoCasa, String> {
    en_hilo(move || Ok(nucleo::estado(&app, fresco))).await
}

/// La orden de la interfaz, las escenas, el menú del ícono y la voz.
#[tauri::command]
pub async fn hogar_ejecutar(app: AppHandle, orden: Orden) -> Result<ResultadoOrden, String> {
    en_hilo(move || Ok(nucleo::ejecutar(&app, &orden, true))).await
}

/// Busca en la red y por Bluetooth (≈6 s).
#[tauri::command]
pub async fn hogar_buscar(app: AppHandle) -> Result<ResultadoBusqueda, String> {
    en_hilo(move || nucleo::buscar(&app)).await
}

/// Suma a Nexo algo encontrado en la última búsqueda (WiZ, Yeelight, Govee o Bluetooth).
#[tauri::command]
pub async fn hogar_agregar(app: AppHandle, id: String) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::agregar(&app, &id)).await
}

/// Quita de Nexo un aparato agregado desde una búsqueda (en la cuenta de Tuya o Home Assistant no se puede).
#[tauri::command]
pub async fn hogar_quitar(app: AppHandle, id: String) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::quitar(&app, &id)).await
}

/// Nombre, habitación o forma de un aparato.
#[tauri::command]
pub async fn hogar_editar(app: AppHandle, id: String, cambio: CambioAparato) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::editar(&app, &id, cambio)).await
}

/// Guarda un grupo nuevo (sin id) o cambia uno.
#[tauri::command]
pub async fn hogar_guardar_grupo(app: AppHandle, grupo: Grupo) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::guardar_grupo(&app, grupo)).await
}

#[tauri::command]
pub async fn hogar_borrar_grupo(app: AppHandle, id: String) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::borrar_grupo(&app, &id)).await
}

/// Guarda una habitación nueva (sin id) o le cambia el nombre.
#[tauri::command]
pub async fn hogar_guardar_habitacion(app: AppHandle, habitacion: Habitacion) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::guardar_habitacion(&app, habitacion)).await
}

/// Borra una habitación: lo que tenía queda en "Sin cuarto".
#[tauri::command]
pub async fn hogar_borrar_habitacion(app: AppHandle, id: String) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::borrar_habitacion(&app, &id)).await
}

#[tauri::command]
pub async fn hogar_mostrar_todo(app: AppHandle, activo: bool) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::mostrar_todo(&app, activo)).await
}

/// Conecta la cuenta de Tuya: comprueba el token y la lista; guarda el secreto en el llavero.
#[tauri::command]
pub async fn hogar_tuya_conectar(app: AppHandle, datos: DatosConexionTuya) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::conectar_tuya(&app, datos)).await
}

#[tauri::command]
pub async fn hogar_tuya_desconectar(app: AppHandle) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::desconectar_tuya(&app)).await
}

/// Conecta Home Assistant: comprueba `/api/` y `/api/states`; guarda el token en el llavero.
#[tauri::command]
pub async fn hogar_ha_conectar(app: AppHandle, datos: DatosConexionHa) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::conectar_ha(&app, datos)).await
}

#[tauri::command]
pub async fn hogar_ha_desconectar(app: AppHandle) -> Result<EstadoCasa, String> {
    en_hilo(move || nucleo::desconectar_ha(&app)).await
}
