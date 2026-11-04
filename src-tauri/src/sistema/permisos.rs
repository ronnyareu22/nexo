//! Permisos del sistema que usa Nexo y el atajo para abrir su sección en Ajustes.
//!
//! - Accesibilidad: las teclas de brillo en el monitor externo (ver `teclas`).
//! - Bluetooth: macOS pide permiso para usar el Bluetooth (CoreBluetooth, desde macOS 10.15).
//! - Notificaciones: el aviso de batería baja.
//! - Grabación de pantalla: la captura liviana del remoto (Imprimir pantalla desde el PC, ver `remoto`).
//!
//! La bienvenida los pide con `pedir` y no deja seguir hasta que estén concedidos.

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
    /// Grabación de pantalla (Mac). En Windows no hace falta: siempre true.
    pub pantalla: bool,
}

/// Lee los permisos. Solo lee: no muestra ningún aviso. Puede tardar hasta 2 s (notificaciones).
pub fn leer(app: &AppHandle) -> Permisos {
    Permisos {
        accesibilidad: teclas::permiso_concedido(),
        bluetooth: bluetooth().into(),
        notificaciones: notificaciones(app).into(),
        pantalla: super::remoto::pantalla_permitida(),
    }
}

/// Pide un permiso: "bluetooth", "accesibilidad", "notificaciones" o "pantalla".
/// Si macOS aún no preguntó, muestra su aviso; si ya se negó (o el aviso no vuelve a salir), abre Ajustes.
/// No espera la respuesta: devuelve cómo están ahora y la interfaz vuelve a leer con `leer`.
pub fn pedir(app: &AppHandle, clave: &str) -> Result<Permisos, String> {
    if !matches!(clave, "bluetooth" | "accesibilidad" | "notificaciones" | "pantalla") {
        return Err(format!("No conozco el permiso «{clave}»"));
    }
    // Windows no pide ninguno de estos permisos a las apps de escritorio: no hay nada que hacer.
    #[cfg(target_os = "macos")]
    {
        match clave {
            "bluetooth" => match bluetooth() {
                "concedido" => {}
                "pendiente" if mac::pedir_bluetooth() => {}
                _ => abrir_ajustes("bluetooth")?,
            },
            "accesibilidad" => {
                if !teclas::permiso_concedido() {
                    // El aviso pone a Nexo en la lista; Ajustes queda abierto para que se vea el interruptor
                    // (en macOS 26 es fácil no ver el aviso).
                    teclas::pedir_permiso();
                    abrir_ajustes("accesibilidad")?;
                }
            }
            "pantalla" => {
                if !super::remoto::pantalla_permitida() {
                    // El aviso pone a Nexo en la lista; se activa en Ajustes y macOS pide volver a abrir Nexo.
                    super::remoto::pedir_pantalla();
                    abrir_ajustes("pantalla")?;
                }
            }
            _ => match notificaciones(app) {
                "concedido" => {}
                "pendiente" if mac::pedir_notificaciones() => {}
                _ => abrir_ajustes("notificaciones")?,
            },
        }
    }
    Ok(leer(app))
}

/// Estado del permiso de notificaciones.
fn notificaciones(app: &AppHandle) -> &'static str {
    // Mac: se le pregunta al sistema de verdad, si Nexo corre desde su .app.
    #[cfg(target_os = "macos")]
    if let Some(estado) = mac::notificaciones() {
        return estado;
    }
    // Si no (Windows, o `tauri dev` en la Mac), el complemento: en escritorio siempre responde "concedido".
    // En Windows es lo correcto: una app de escritorio no necesita permiso para sus avisos.
    match app.notification().permission_state() {
        Ok(PermissionState::Granted) => "concedido",
        Ok(PermissionState::Denied) => "denegado",
        _ => "pendiente",
    }
}

/// Estado del permiso de Bluetooth (`CBManager.authorization`). Solo lee: no muestra ningún aviso.
#[cfg(target_os = "macos")]
pub fn bluetooth() -> &'static str {
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

/// Windows no pide permiso a las apps de escritorio para usar el Bluetooth: es "concedido", igual que en Mac.
#[cfg(not(target_os = "macos"))]
pub fn bluetooth() -> &'static str {
    "concedido"
}

/// Mac: notificaciones (UNUserNotificationCenter) y el aviso de Bluetooth (CBCentralManager).
#[cfg(target_os = "macos")]
mod mac {
    use block2::RcBlock;
    use objc2::msg_send;
    use objc2::rc::{Allocated, Retained, autoreleasepool};
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2_foundation::NSString;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    #[link(name = "UserNotifications", kind = "framework")]
    unsafe extern "C" {}

    // UNAuthorizationOptions
    const GLOBO: usize = 1 << 0; // badge
    const SONIDO: usize = 1 << 1; // sound
    const ALERTA: usize = 1 << 2; // alert

    /// Si Nexo corre desde su paquete .app. Con `tauri dev` no, y ahí `currentNotificationCenter`
    /// cierra la app de golpe: por eso ni se intenta. No cambia mientras la app corre.
    fn dentro_de_app() -> bool {
        static DENTRO: OnceLock<bool> = OnceLock::new();
        *DENTRO.get_or_init(|| {
            let Some(clase) = AnyClass::get(c"NSBundle") else { return false };
            autoreleasepool(|_| {
                // SAFETY: mainBundle es un NSBundle; bundlePath y bundleIdentifier son NSString (o nil).
                unsafe {
                    let paquete: *mut AnyObject = msg_send![clase, mainBundle];
                    if paquete.is_null() {
                        return false;
                    }
                    let id: *mut AnyObject = msg_send![paquete, bundleIdentifier];
                    let ruta: *mut NSString = msg_send![paquete, bundlePath];
                    !id.is_null() && !ruta.is_null() && (*ruta).to_string().trim_end_matches('/').ends_with(".app")
                }
            })
        })
    }

    /// El centro de notificaciones de Nexo, solo dentro del .app. Hay que usarlo dentro de un autoreleasepool.
    fn centro() -> Option<*mut AnyObject> {
        if !dentro_de_app() {
            return None;
        }
        let clase = AnyClass::get(c"UNUserNotificationCenter")?;
        // SAFETY: propiedad de clase (macOS 10.14+) que devuelve el centro de la app; ya se vio que hay .app.
        let c: *mut AnyObject = unsafe { msg_send![clase, currentNotificationCenter] };
        (!c.is_null()).then_some(c)
    }

    /// Estado real del permiso. `None` si no se puede preguntar (fuera del .app) o no responde en 2 s.
    pub fn notificaciones() -> Option<&'static str> {
        let (tx, rx) = mpsc::channel::<isize>();
        let preguntado = autoreleasepool(|_| {
            let Some(centro) = centro() else { return false };
            let bloque = RcBlock::new(move |ajustes: *mut AnyObject| {
                if !ajustes.is_null() {
                    // SAFETY: `ajustes` es un UNNotificationSettings; authorizationStatus es un NSInteger.
                    let estado: isize = unsafe { msg_send![ajustes, authorizationStatus] };
                    let _ = tx.send(estado);
                }
            });
            // SAFETY: el método recibe un bloque que macOS copia y llama una vez, en otro hilo.
            let _: () = unsafe { msg_send![centro, getNotificationSettingsWithCompletionHandler: &*bloque] };
            true
        });
        if !preguntado {
            return None;
        }
        let estado = rx.recv_timeout(Duration::from_secs(2)).ok()?;
        // UNAuthorizationStatus: 0 sin preguntar, 1 negado, 2 permitido, 3 provisional, 4 temporal.
        Some(match estado {
            0 => "pendiente",
            1 => "denegado",
            _ => "concedido",
        })
    }

    static NOTIFICACIONES_PEDIDAS: AtomicBool = AtomicBool::new(false);

    /// Muestra el aviso de macOS para permitir notificaciones. `false` si ya se mostró en esta sesión
    /// o si macOS no deja pedirlo: entonces conviene abrir Ajustes.
    pub fn pedir_notificaciones() -> bool {
        if NOTIFICACIONES_PEDIDAS.swap(true, Ordering::SeqCst) {
            return false;
        }
        let (tx, rx) = mpsc::channel::<bool>();
        let pedido = autoreleasepool(|_| {
            let Some(centro) = centro() else { return false };
            // Avisa si la respuesta trae un error (por ejemplo, si macOS no deja pedirlo).
            let bloque = RcBlock::new(move |_concedido: Bool, error: *mut AnyObject| {
                let _ = tx.send(error.is_null());
            });
            // SAFETY: opciones es un NSUInteger; el bloque lo copia macOS y lo llama al responder la persona.
            let _: () = unsafe {
                msg_send![centro, requestAuthorizationWithOptions: ALERTA | SONIDO | GLOBO, completionHandler: &*bloque]
            };
            true
        });
        if !pedido {
            return false;
        }
        // Si macOS responde al instante con un error, no hubo aviso. Si no responde, el aviso está en pantalla.
        !matches!(rx.recv_timeout(Duration::from_millis(800)), Ok(false))
    }

    /// El CBCentralManager que muestra el aviso de Bluetooth. Vive toda la sesión para que el aviso no se cierre.
    struct Gestor(#[allow(dead_code)] Retained<AnyObject>);
    // SAFETY: solo se guarda para mantenerlo vivo; nunca se le envían mensajes desde otro hilo.
    unsafe impl Send for Gestor {}
    unsafe impl Sync for Gestor {}

    static GESTOR_BLUETOOTH: OnceLock<Option<Gestor>> = OnceLock::new();

    /// Muestra el aviso de Bluetooth de macOS (aparece al crear un CBCentralManager).
    /// `false` si ya se había creado: el aviso no vuelve a salir y conviene abrir Ajustes.
    pub fn pedir_bluetooth() -> bool {
        let mut recien = false;
        let gestor = GESTOR_BLUETOOTH.get_or_init(|| {
            recien = true;
            let clase = AnyClass::get(c"CBCentralManager")?;
            let nulo = std::ptr::null_mut::<AnyObject>();
            autoreleasepool(|_| {
                // SAFETY: alloc + initWithDelegate:queue: (sin delegado; la cola nil es la principal).
                unsafe {
                    let nuevo: Allocated<AnyObject> = msg_send![clase, alloc];
                    let listo: Option<Retained<AnyObject>> = msg_send![nuevo, initWithDelegate: nulo, queue: nulo];
                    listo.map(Gestor)
                }
            })
        });
        recien && gestor.is_some()
    }
}

/// Abre la sección de Ajustes donde se concede cada permiso: "accesibilidad", "bluetooth", "notificaciones" o "pantalla".
pub fn abrir_ajustes(seccion: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let destino = match seccion {
            "accesibilidad" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            "bluetooth" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Bluetooth",
            "notificaciones" => "x-apple.systempreferences:com.apple.Notifications-Settings.extension",
            "pantalla" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            _ => return Err(format!("No conozco la sección «{seccion}»")),
        };
        std::process::Command::new("/usr/bin/open").arg(destino).spawn().map(|_| ()).map_err(|e| e.to_string())
    }
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::{HSTRING, PCWSTR, w};

        let destino = match seccion {
            "bluetooth" => "ms-settings:privacy-radios",
            "notificaciones" => "ms-settings:notifications",
            "accesibilidad" | "pantalla" => return Err("En Windows no hace falta este permiso".into()),
            _ => return Err(format!("No conozco la sección «{seccion}»")),
        };
        // ShellExecuteW abre Configuración sin la consola que asoma con "cmd /C start".
        let destino = HSTRING::from(destino);
        let r = unsafe { ShellExecuteW(None, w!("open"), &destino, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
        // Devuelve un número mayor que 32 si pudo.
        if r.0 as isize > 32 { Ok(()) } else { Err("Windows no pudo abrir Configuración".into()) }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = seccion;
        Err("Todavía no disponible en este sistema".into())
    }
}
