//! "Abrir Nexo al iniciar sesión".
//!
//! Mac: `SMAppService.mainAppService` (ServiceManagement, macOS 13 o posterior). El sistema guarda
//! el estado y la persona lo ve en Ajustes › General › Ítems de inicio; Nexo no guarda nada aparte.
//! Windows: pendiente.

#[cfg(target_os = "macos")]
pub use mac::{activo, usar};

#[cfg(not(target_os = "macos"))]
pub fn activo() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn usar(_activar: bool) -> Result<bool, String> {
    Err("Todavía no disponible en Windows".into())
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::msg_send;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;

    #[link(name = "ServiceManagement", kind = "framework")]
    unsafe extern "C" {}

    // SMAppServiceStatus
    const ACTIVO: isize = 1; // enabled
    const FALTA_APROBAR: isize = 2; // requiresApproval

    /// La clase SMAppService, si este macOS la tiene (13 o posterior).
    fn clase() -> Option<&'static AnyClass> {
        AnyClass::get(c"SMAppService")
    }

    /// El servicio de la propia app. Hay que usarlo dentro de un autoreleasepool.
    fn servicio() -> Option<*mut AnyObject> {
        let c = clase()?;
        // SAFETY: mainAppService es una propiedad de clase que devuelve un SMAppService.
        let s: *mut AnyObject = unsafe { msg_send![c, mainAppService] };
        (!s.is_null()).then_some(s)
    }

    fn estado(s: *mut AnyObject) -> isize {
        // SAFETY: `s` es un SMAppService vivo (sale de `servicio`, dentro del mismo autoreleasepool).
        unsafe { msg_send![s, status] }
    }

    /// Si Nexo se abre al iniciar sesión. Solo lee.
    pub fn activo() -> bool {
        autoreleasepool(|_| servicio().is_some_and(|s| estado(s) == ACTIVO))
    }

    /// Activa o desactiva el arranque y devuelve cómo quedó.
    pub fn usar(activar: bool) -> Result<bool, String> {
        autoreleasepool(|_| {
            let s = servicio().ok_or("Esta opción necesita macOS 13 o posterior")?;
            let mut error: *mut AnyObject = std::ptr::null_mut();
            let puntero_error = &mut error as *mut *mut AnyObject;
            // SAFETY: los dos mensajes devuelven BOOL y llenan un NSError** si fallan.
            let listo: bool = unsafe {
                if activar {
                    msg_send![s, registerAndReturnError: puntero_error]
                } else {
                    msg_send![s, unregisterAndReturnError: puntero_error]
                }
            };
            let ahora = estado(s);

            // macOS puede pedir que la persona lo apruebe: se abre esa sección de Ajustes.
            if activar && ahora == FALTA_APROBAR {
                if let Some(c) = clase() {
                    // SAFETY: método de clase sin argumentos (macOS 13+), que existe si la clase existe.
                    let _: () = unsafe { msg_send![c, openSystemSettingsLoginItems] };
                }
                return Err("Falta tu aprobación: en Ajustes › General › Ítems de inicio, activa Nexo.".into());
            }
            if !listo && (ahora == ACTIVO) != activar {
                let detalle = if error.is_null() {
                    String::new()
                } else {
                    // SAFETY: `error` es un NSError que dejó el sistema; localizedDescription es un NSString.
                    let texto: *mut NSString = unsafe { msg_send![error, localizedDescription] };
                    if texto.is_null() { String::new() } else { format!(" ({})", unsafe { &*texto }) }
                };
                return Err(format!("macOS no dejó cambiar el arranque de Nexo{detalle}."));
            }
            Ok(ahora == ACTIVO)
        })
    }
}
