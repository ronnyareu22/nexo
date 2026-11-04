//! Integración con el sistema: ajustes guardados, teclas de brillo, aviso en pantalla,
//! permisos, abrir Nexo al iniciar sesión, el widget de escritorio, el remoto (RustDesk por Tailscale)
//! y Nexo Drop (archivos, textos y enlaces entre los equipos del dueño por Tailscale).
//! Mi setup: el mapa de la casa y la oficina (`setup`) y las escenas de un toque (`escenas`).

pub mod ajustes;
pub mod arranque;
pub mod aviso;
pub mod drop;
pub mod escenas;
pub mod permisos;
pub mod remoto;
#[cfg(windows)]
pub mod remoto_windows;
pub mod setup;
pub mod teclas;
pub mod widget;
pub mod widgets_mac;

use crate::dispositivos::pantallas::Pantallas;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoAjustes {
    pub teclas_brillo: bool,
    /// Si macOS ya le dio a Nexo el permiso de Accesibilidad.
    pub permiso: bool,
}

fn estado(app: &AppHandle) -> EstadoAjustes {
    EstadoAjustes { teclas_brillo: ajustes::cargar(app).teclas_brillo, permiso: teclas::permiso_concedido() }
}

#[tauri::command]
pub fn leer_ajustes(app: AppHandle) -> EstadoAjustes {
    estado(&app)
}

/// Enciende o apaga "teclas de brillo en el monitor bajo el mouse".
/// Si falta el permiso, abre el aviso de macOS y lo activa solo en cuanto se conceda (espera hasta 3 min).
#[tauri::command]
pub fn usar_teclas_brillo(app: AppHandle, pantallas: State<'_, Arc<Pantallas>>, activar: bool) -> Result<EstadoAjustes, String> {
    ajustes::cambiar(&app, |a| a.teclas_brillo = activar)?;

    let p = pantallas.inner().clone();
    if !activar || teclas::permiso_concedido() {
        teclas::activar(p, activar)?;
    } else {
        teclas::pedir_permiso();
        std::thread::spawn(move || {
            for _ in 0..90 {
                std::thread::sleep(std::time::Duration::from_secs(2));
                if teclas::permiso_concedido() {
                    let _ = teclas::activar(p, true);
                    return;
                }
            }
        });
    }
    Ok(estado(&app))
}

/// El nivel que debe mostrar el aviso al abrirse.
#[tauri::command]
pub fn nivel_aviso() -> u8 {
    aviso::nivel_actual()
}

// ---------------------------------------------------------------- Permisos

/// Estado de los permisos que usa Nexo (accesibilidad, Bluetooth, notificaciones y grabación de pantalla). Solo lee.
#[tauri::command]
pub async fn permisos(app: AppHandle) -> permisos::Permisos {
    tauri::async_runtime::spawn_blocking(move || permisos::leer(&app)).await.unwrap_or_else(|_| permisos::Permisos {
        accesibilidad: teclas::permiso_concedido(),
        bluetooth: "pendiente".into(),
        notificaciones: "pendiente".into(),
        pantalla: remoto::pantalla_permitida(),
    })
}

/// Abre la sección de Ajustes de un permiso: "accesibilidad", "bluetooth", "notificaciones" o "pantalla".
#[tauri::command]
pub fn abrir_ajustes_privacidad(seccion: String) -> Result<(), String> {
    permisos::abrir_ajustes(&seccion)
}

/// Pide un permiso ("bluetooth", "accesibilidad", "notificaciones" o "pantalla"): muestra el aviso del sistema si aún
/// no se preguntó, o abre su sección de Ajustes si se negó. Devuelve cómo están los permisos ahora.
#[tauri::command]
pub async fn pedir_permiso(app: AppHandle, clave: String) -> Result<permisos::Permisos, String> {
    tauri::async_runtime::spawn_blocking(move || permisos::pedir(&app, &clave)).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- Bienvenida

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bienvenida {
    /// Si la persona ya terminó la bienvenida.
    pub completa: bool,
    /// El nombre guardado o, si no hay, el de pila del sistema. Puede venir vacío.
    pub nombre: String,
}

/// Quita espacios de más y caracteres de control, y lo deja en 30 letras. `None` si queda vacío.
fn limpiar_nombre(texto: &str) -> Option<String> {
    let sin_control: String = texto.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let junto = sin_control.split_whitespace().collect::<Vec<_>>().join(" ");
    let corto: String = junto.chars().take(30).collect();
    let corto = corto.trim_end();
    (!corto.is_empty()).then(|| corto.to_string())
}

/// El nombre para saludar: el que escribió la persona o, si no hay, el de pila del sistema. Puede ser "".
fn nombre_de(a: &ajustes::Ajustes) -> String {
    a.nombre.as_deref().and_then(limpiar_nombre).unwrap_or_else(crate::dispositivos::nombre_de_pila)
}

/// Lo mismo, leyendo los ajustes guardados. Lo usa también el saludo de Inicio.
pub fn nombre_para_saludo(app: &AppHandle) -> String {
    nombre_de(&ajustes::cargar(app))
}

/// Si ya se hizo la bienvenida y con qué nombre saludar.
#[tauri::command]
pub async fn leer_bienvenida(app: AppHandle) -> Bienvenida {
    tauri::async_runtime::spawn_blocking(move || {
        let a = ajustes::cargar(&app);
        Bienvenida { completa: a.bienvenida, nombre: nombre_de(&a) }
    })
    .await
    .unwrap_or(Bienvenida { completa: false, nombre: String::new() })
}

/// Guarda el nombre que escribió la persona y devuelve cómo quedó.
#[tauri::command]
pub fn guardar_nombre(app: AppHandle, nombre: String) -> Result<String, String> {
    let limpio = limpiar_nombre(&nombre).ok_or("Escribe tu nombre.")?;
    ajustes::cambiar(&app, |a| a.nombre = Some(limpio.clone()))?;
    Ok(limpio)
}

/// Marca la bienvenida como terminada: la próxima vez Nexo abre directo en el panel.
#[tauri::command]
pub fn completar_bienvenida(app: AppHandle) -> Result<(), String> {
    ajustes::cambiar(&app, |a| a.bienvenida = true).map(|_| ())
}

// ---------------------------------------------------------------- Abrir al iniciar sesión

/// Si Nexo se abre solo al iniciar sesión.
#[tauri::command]
pub async fn arranque() -> bool {
    tauri::async_runtime::spawn_blocking(arranque::activo).await.unwrap_or(false)
}

/// Activa o desactiva abrir Nexo al iniciar sesión. Devuelve cómo quedó.
#[tauri::command]
pub async fn usar_arranque(activar: bool) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || arranque::usar(activar))
        .await
        .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- Widget de escritorio

/// Si el widget de escritorio está encendido.
#[tauri::command]
pub fn widget_activo(app: AppHandle) -> bool {
    widget::activo(&app)
}

/// Enciende o apaga el widget de escritorio. Devuelve cómo quedó.
/// Va en segundo plano: en Windows, crear una ventana desde un comando síncrono la traba.
#[tauri::command]
pub async fn usar_widget(app: AppHandle, activar: bool) -> Result<bool, String> {
    widget::usar(&app, activar)
}
