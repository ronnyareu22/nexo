//! "Abrir Nexo al iniciar sesión".
//!
//! Mac: `SMAppService.mainAppService` (ServiceManagement, macOS 13 o posterior). El sistema guarda
//! el estado y la persona lo ve en Ajustes › General › Ítems de inicio; Nexo no guarda nada aparte.
//! Windows: el valor "Nexo" en HKCU\Software\Microsoft\Windows\CurrentVersion\Run, con la ruta de este
//! .exe entre comillas. No pide permiso de administrador y la persona lo ve en Configuración › Aplicaciones › Inicio.

#[cfg(target_os = "macos")]
pub use mac::{activo, usar};

#[cfg(windows)]
pub use win::{activo, usar};

#[cfg(not(any(target_os = "macos", windows)))]
pub fn activo() -> bool {
    false
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn usar(_activar: bool) -> Result<bool, String> {
    Err("Todavía no disponible en este sistema".into())
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_ROUTINE_FLAGS, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW,
        RegSetKeyValueW,
    };
    use windows::core::{PCWSTR, w};

    fn clave_run() -> PCWSTR {
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
    }

    /// Donde Windows anota si la persona apagó el arranque desde el Administrador de tareas › Inicio.
    fn clave_aprobados() -> PCWSTR {
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run")
    }

    fn nombre() -> PCWSTR {
        w!("Nexo")
    }

    /// La ruta de este .exe entre comillas, en UTF-16 y terminada en cero (como se guarda en el registro).
    fn comando() -> Result<Vec<u16>, String> {
        let ruta = std::env::current_exe().map_err(|e| format!("No encontré dónde está Nexo ({e})."))?;
        let mut texto: Vec<u16> = Vec::new();
        texto.push('"' as u16);
        texto.extend(ruta.as_os_str().encode_wide());
        texto.push('"' as u16);
        texto.push(0);
        Ok(texto)
    }

    fn leer(subclave: PCWSTR, tipo: REG_ROUTINE_FLAGS) -> Option<Vec<u8>> {
        let mut tam = 0u32;
        let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, subclave, nombre(), tipo, None, None, Some(&mut tam as *mut u32)) };
        if r != ERROR_SUCCESS || tam == 0 || tam > 64 * 1024 {
            return None;
        }
        let mut datos = vec![0u8; tam as usize];
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                subclave,
                nombre(),
                tipo,
                None,
                Some(datos.as_mut_ptr() as *mut c_void),
                Some(&mut tam as *mut u32),
            )
        };
        if r != ERROR_SUCCESS {
            return None;
        }
        datos.truncate(tam as usize);
        Some(datos)
    }

    fn a_texto(bytes: &[u8]) -> String {
        let unidades: Vec<u16> = bytes.chunks_exact(2).map(|p| u16::from_le_bytes([p[0], p[1]])).collect();
        let fin = unidades.iter().position(|c| *c == 0).unwrap_or(unidades.len());
        String::from_utf16_lossy(&unidades[..fin])
    }

    /// Si Nexo se abre al iniciar sesión: el valor existe, apunta a este .exe y no está apagado en el Administrador de tareas.
    pub fn activo() -> bool {
        let Some(guardado) = leer(clave_run(), RRF_RT_REG_SZ) else { return false };
        let Ok(propio) = comando() else { return false };
        let propio = String::from_utf16_lossy(&propio[..propio.len().saturating_sub(1)]);
        let limpiar = |t: &str| t.trim().trim_matches('"').to_lowercase();
        if limpiar(&a_texto(&guardado)) != limpiar(&propio) {
            return false;
        }
        // Primer byte impar (03 00 00 00…) = apagado por la persona.
        !leer(clave_aprobados(), RRF_RT_REG_BINARY).is_some_and(|d| d.first().is_some_and(|b| b & 1 == 1))
    }

    /// Activa o desactiva el arranque y devuelve cómo quedó.
    pub fn usar(activar: bool) -> Result<bool, String> {
        if activar {
            let datos = comando()?;
            let r = unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    clave_run(),
                    nombre(),
                    REG_SZ.0,
                    Some(datos.as_ptr() as *const c_void),
                    (datos.len() * 2) as u32,
                )
            };
            if r != ERROR_SUCCESS {
                return Err(format!("Windows no dejó activar el arranque de Nexo (código {}).", r.0));
            }
            // Si se había apagado desde el Administrador de tareas, se quita esa marca para que vuelva a abrirse.
            let _ = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, clave_aprobados(), nombre()) };
        } else {
            let r = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, clave_run(), nombre()) };
            if r != ERROR_SUCCESS && r != ERROR_FILE_NOT_FOUND {
                return Err(format!("Windows no dejó quitar el arranque de Nexo (código {}).", r.0));
            }
        }
        Ok(activo())
    }
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
