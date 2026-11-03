//! Permisos del sistema que usa Nexo y el atajo para abrir su sección en Ajustes.
//!
//! - Accesibilidad: las teclas de brillo en el monitor externo (ver `teclas`).
//! - Bluetooth: macOS pide permiso para usar el Bluetooth (CoreBluetooth, desde macOS 10.15).
//! - Notificaciones: el aviso de batería baja.

use super::teclas;
use serde::Serialize;
use tauri::AppHandle;
use tauri::plugin::PermissionState;
use tauri_plugin_notification::NotificationExt;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Permisos {
    pub accesibilidad: bool,
    /// "concedido", "denegado", "pendiente" (aún no se ha preguntado) o "restringido" (lo bloquea un perfil).
    pub bluetooth: String,
    /// "concedido", "denegado" o "pendiente".
    pub notificaciones: String,
}

pub fn leer(app: &AppHandle) -> Permisos {
    // En escritorio, el complemento de notificaciones responde "concedido" mientras no haya
    // una forma de preguntarle al sistema; si algún día lo hace, aquí se verá igual.
    let notificaciones = match app.notification().permission_state() {
        Ok(PermissionState::Granted) => "concedido",
        Ok(PermissionState::Denied) => "denegado",
        _ => "pendiente",
    };
    Permisos {
        accesibilidad: teclas::permiso_concedido(),
        bluetooth: bluetooth().into(),
        notificaciones: notificaciones.into(),
    }
}

/// Estado del permiso de Bluetooth (`CBManager.authorization`). Solo lee: no muestra ningún aviso.
#[cfg(target_os = "macos")]
fn bluetooth() -> &'static str {
    use objc2::runtime::AnyClass;
    use objc2::{msg_send, sel};

    #[link(name = "CoreBluetooth", kind = "framework")]
    unsafe extern "C" {}

    let Some(clase) = AnyClass::get(c"CBManager") else { return "pendiente" };
    // SAFETY: se pregunta si la clase entiende `authorization` antes de enviarlo (no existe antes de macOS 10.15).
    unsafe {
        let entiende: bool = msg_send![clase, respondsToSelector: sel!(authorization)];
        if !entiende {
            return "concedido"; // antes de macOS 10.15 el Bluetooth no pedía permiso
        }
        let estado: isize = msg_send![clase, authorization];
        match estado {
            3 => "concedido",   // allowedAlways
            2 => "denegado",    // denied
            1 => "restringido", // restricted
            _ => "pendiente",   // notDetermined
        }
    }
}

/// Windows no pide permiso a las apps de escritorio para usar el Bluetooth.
#[cfg(not(target_os = "macos"))]
fn bluetooth() -> &'static str {
    "concedido"
}

/// Abre la sección de Ajustes donde se concede cada permiso: "accesibilidad", "bluetooth" o "notificaciones".
pub fn abrir_ajustes(seccion: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let destino = match seccion {
            "accesibilidad" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            "bluetooth" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Bluetooth",
            "notificaciones" => "x-apple.systempreferences:com.apple.Notifications-Settings.extension",
            _ => return Err(format!("No conozco la sección «{seccion}»")),
        };
        std::process::Command::new("/usr/bin/open").arg(destino).spawn().map(|_| ()).map_err(|e| e.to_string())
    }
    #[cfg(windows)]
    {
        let destino = match seccion {
            "bluetooth" => "ms-settings:privacy-radios",
            "notificaciones" => "ms-settings:notifications",
            "accesibilidad" => return Err("En Windows no hace falta este permiso".into()),
            _ => return Err(format!("No conozco la sección «{seccion}»")),
        };
        std::process::Command::new("cmd").args(["/C", "start", destino]).spawn().map(|_| ()).map_err(|e| e.to_string())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = seccion;
        Err("Todavía no disponible en este sistema".into())
    }
}
