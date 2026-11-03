//! Ajustes de Nexo, guardados en un JSON pequeño en la carpeta de la app.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Ajustes {
    /// Las teclas de brillo de la Mac cambian el monitor externo que está bajo el mouse.
    pub teclas_brillo: bool,
}

fn ruta(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("ajustes.json"))
}

pub fn cargar(app: &AppHandle) -> Ajustes {
    ruta(app)
        .and_then(|r| std::fs::read(r).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn guardar(app: &AppHandle, ajustes: &Ajustes) -> Result<(), String> {
    let r = ruta(app).ok_or("No encontré la carpeta de ajustes")?;
    if let Some(carpeta) = r.parent() {
        std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(ajustes).map_err(|e| e.to_string())?;
    std::fs::write(r, json).map_err(|e| e.to_string())
}
