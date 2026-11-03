//! Integración con el sistema: ajustes guardados, teclas de brillo, aviso en pantalla,
//! permisos y abrir Nexo al iniciar sesión.

pub mod ajustes;
pub mod arranque;
pub mod aviso;
pub mod permisos;
pub mod teclas;

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
    let mut a = ajustes::cargar(&app);
    a.teclas_brillo = activar;
    ajustes::guardar(&app, &a)?;

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

/// Estado de los permisos que usa Nexo (accesibilidad, Bluetooth y notificaciones). Solo lee.
#[tauri::command]
pub async fn permisos(app: AppHandle) -> permisos::Permisos {
    tauri::async_runtime::spawn_blocking(move || permisos::leer(&app)).await.unwrap_or_else(|_| permisos::Permisos {
        accesibilidad: teclas::permiso_concedido(),
        bluetooth: "pendiente".into(),
        notificaciones: "pendiente".into(),
    })
}

/// Abre la sección de Ajustes de un permiso: "accesibilidad", "bluetooth" o "notificaciones".
#[tauri::command]
pub fn abrir_ajustes_privacidad(seccion: String) -> Result<(), String> {
    permisos::abrir_ajustes(&seccion)
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
