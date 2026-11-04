//! Ajustes de Nexo, guardados en un JSON pequeño en la carpeta de la app.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Ajustes {
    /// Las teclas de brillo de la Mac (o Ctrl+Alt+F1 / F2 en Windows) cambian la pantalla que está bajo el mouse.
    /// En Windows viene encendido; en la Mac pide permiso de Accesibilidad, así que ahí viene apagado.
    pub teclas_brillo: bool,
    /// Windows: ya se encendieron las teclas de brillo la primera vez (ver `primer_inicio`). Después manda la persona.
    pub teclas_brillo_revisadas: bool,
    /// El widget de escritorio propio (ver `widget`). Viene encendido en Windows; en la Mac están los widgets
    /// nativos (widget-mac/), así que ahí viene apagado.
    pub widget: bool,
    /// Dónde dejó la persona el widget: en puntos en Mac y en píxeles reales en Windows.
    /// Vacío: arriba a la derecha de la pantalla principal.
    pub widget_posicion: Option<(i32, i32)>,
    /// El nombre que escribió la persona en la bienvenida. Vacío: el nombre de pila del sistema.
    pub nombre: Option<String>,
    /// Si ya terminó la bienvenida (su nombre y los permisos).
    pub bienvenida: bool,
    /// El remoto (RustDesk por Tailscale): traducir atajos, captura, comodidad y portapapeles liviano.
    pub remoto: super::remoto::AjustesRemoto,
    /// Nexo Drop: recibir archivos y copiar al portapapeles las imágenes que llegan.
    pub drop: super::drop::AjustesDrop,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes {
            teclas_brillo: cfg!(windows),
            teclas_brillo_revisadas: false,
            widget: !cfg!(target_os = "macos"),
            widget_posicion: None,
            nombre: None,
            bienvenida: false,
            remoto: super::remoto::AjustesRemoto::default(),
            drop: super::drop::AjustesDrop::default(),
        }
    }
}

/// Un cambio a la vez: así el widget (al moverse) y el panel no se pisan al guardar.
static CAMBIANDO: Mutex<()> = Mutex::new(());

fn ruta(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("ajustes.json"))
}

/// Los ajustes guardados, solo si se pudieron leer (sin poner los de fábrica en su lugar).
pub fn leer(app: &AppHandle) -> Option<Ajustes> {
    ruta(app).and_then(|r| std::fs::read(r).ok()).and_then(|b| serde_json::from_slice(&b).ok())
}

pub fn cargar(app: &AppHandle) -> Ajustes {
    leer(app).unwrap_or_default()
}

pub fn guardar(app: &AppHandle, ajustes: &Ajustes) -> Result<(), String> {
    let r = ruta(app).ok_or("No encontré la carpeta de ajustes")?;
    if let Some(carpeta) = r.parent() {
        std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(ajustes).map_err(|e| e.to_string())?;
    std::fs::write(r, json).map_err(|e| e.to_string())
}

/// Lee, cambia y guarda en un solo paso. Devuelve cómo quedaron los ajustes.
pub fn cambiar(app: &AppHandle, cambio: impl FnOnce(&mut Ajustes)) -> Result<Ajustes, String> {
    let _turno = CAMBIANDO.lock().unwrap_or_else(|e| e.into_inner());
    let mut a = cargar(app);
    cambio(&mut a);
    guardar(app, &a)?;
    Ok(a)
}
