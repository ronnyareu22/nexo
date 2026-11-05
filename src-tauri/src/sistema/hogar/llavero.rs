//! Las claves de la casa (el Access Secret de Tuya, el token de Home Assistant y las de los aparatos de la red: la
//! clave del puente Hue, el token de una Nanoleaf, la contraseña de un Shelly o un Tasmota) en el llavero del sistema:
//! el Llavero de la Mac y el Administrador de credenciales de Windows. Nunca en `ajustes.json`, en un registro
//! ni en la interfaz: se escriben al conectar, se leen al usarlas y se borran al desconectar.
//! - Mac: con la firma local (ad hoc), después de cada versión nueva macOS puede preguntar una vez si Nexo puede
//!   leerla ("Permitir siempre").

/// Dónde va cada clave. El nombre se ve en Acceso a Llaveros o en el Administrador de credenciales.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clave {
    Tuya,
    HomeAssistant,
    /// Un aparato de la red; la cuenta es su id en Nexo ("hue:001788fffe23bfc2", "shelly:a8032ab1c2d3").
    Aparato,
}

impl Clave {
    fn servicio(self) -> &'static str {
        match self {
            Clave::Tuya => "Nexo · Tuya / Smart Life",
            Clave::HomeAssistant => "Nexo · Home Assistant",
            Clave::Aparato => "Nexo · Aparatos de la casa",
        }
    }
}

/// Guarda la clave (reemplaza la que hubiera para esa cuenta).
pub fn guardar(clave: Clave, cuenta: &str, secreto: &str) -> Result<(), String> {
    sistema::guardar(clave.servicio(), cuenta, secreto)
}

/// La clave guardada para esa cuenta; `None` si no hay.
pub fn leer(clave: Clave, cuenta: &str) -> Result<Option<String>, String> {
    sistema::leer(clave.servicio(), cuenta)
}

/// Borra la clave de esa cuenta (si no había, no pasa nada).
pub fn borrar(clave: Clave, cuenta: &str) -> Result<(), String> {
    sistema::borrar(clave.servicio(), cuenta)
}

#[cfg(target_os = "macos")]
mod sistema {
    use core_foundation::base::{CFType, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::{CFData, CFDataRef};
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::{CFString, CFStringRef};

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        static kSecClass: CFStringRef;
        static kSecClassGenericPassword: CFStringRef;
        static kSecAttrService: CFStringRef;
        static kSecAttrAccount: CFStringRef;
        static kSecAttrLabel: CFStringRef;
        static kSecValueData: CFStringRef;
        static kSecReturnData: CFStringRef;
        static kSecMatchLimit: CFStringRef;
        static kSecMatchLimitOne: CFStringRef;
        fn SecItemAdd(atributos: CFDictionaryRef, resultado: *mut CFTypeRef) -> i32;
        fn SecItemCopyMatching(consulta: CFDictionaryRef, resultado: *mut CFTypeRef) -> i32;
        fn SecItemUpdate(consulta: CFDictionaryRef, cambios: CFDictionaryRef) -> i32;
        fn SecItemDelete(consulta: CFDictionaryRef) -> i32;
    }

    const NO_ESTA: i32 = -25300;
    const YA_ESTA: i32 = -25299;
    const CANCELADO: i32 = -128;

    // SAFETY: son constantes de Security.framework, válidas durante toda la vida del proceso.
    fn k(c: CFStringRef) -> CFString {
        unsafe { CFString::wrap_under_get_rule(c) }
    }

    fn en_palabras(codigo: i32) -> String {
        match codigo {
            CANCELADO => "No se dio permiso para usar el llavero.".into(),
            -25308 => "El llavero está bloqueado. Desbloquéalo e intenta de nuevo.".into(),
            otro => format!("El llavero respondió con un error ({otro})."),
        }
    }

    /// La consulta base: contraseña genérica de este servicio y esta cuenta.
    fn consulta(servicio: &str, cuenta: &str, extra: &[(CFString, CFType)]) -> CFDictionary<CFString, CFType> {
        // SAFETY: lectura de las constantes del framework.
        let mut pares: Vec<(CFString, CFType)> = unsafe {
            vec![
                (k(kSecClass), k(kSecClassGenericPassword).as_CFType()),
                (k(kSecAttrService), CFString::new(servicio).as_CFType()),
                (k(kSecAttrAccount), CFString::new(cuenta).as_CFType()),
            ]
        };
        pares.extend(extra.iter().cloned());
        CFDictionary::from_CFType_pairs(&pares)
    }

    pub fn guardar(servicio: &str, cuenta: &str, secreto: &str) -> Result<(), String> {
        let datos = CFData::from_buffer(secreto.as_bytes());
        // SAFETY: diccionarios y datos de CoreFoundation que viven hasta el final de cada llamada.
        unsafe {
            let base = consulta(servicio, cuenta, &[]);
            let cambios = CFDictionary::from_CFType_pairs(&[(k(kSecValueData), datos.as_CFType())]);
            let r = SecItemUpdate(base.as_concrete_TypeRef(), cambios.as_concrete_TypeRef());
            if r == 0 {
                return Ok(());
            }
            if r != NO_ESTA {
                return Err(en_palabras(r));
            }
            let nueva = consulta(
                servicio,
                cuenta,
                &[
                    (k(kSecValueData), datos.as_CFType()),
                    (k(kSecAttrLabel), CFString::new(servicio).as_CFType()),
                ],
            );
            match SecItemAdd(nueva.as_concrete_TypeRef(), std::ptr::null_mut()) {
                0 => Ok(()),
                YA_ESTA => Err("Ya había una clave guardada que no se pudo cambiar.".into()),
                otro => Err(en_palabras(otro)),
            }
        }
    }

    pub fn leer(servicio: &str, cuenta: &str) -> Result<Option<String>, String> {
        // SAFETY: igual que arriba; lo que devuelve SecItemCopyMatching es nuestro (regla de copia) y es un CFData.
        unsafe {
            let q = consulta(
                servicio,
                cuenta,
                &[
                    (k(kSecReturnData), CFBoolean::true_value().as_CFType()),
                    (k(kSecMatchLimit), k(kSecMatchLimitOne).as_CFType()),
                ],
            );
            let mut resultado: CFTypeRef = std::ptr::null();
            match SecItemCopyMatching(q.as_concrete_TypeRef(), &mut resultado) {
                0 if !resultado.is_null() => {
                    let datos = CFData::wrap_under_create_rule(resultado as CFDataRef);
                    String::from_utf8(datos.bytes().to_vec()).map(Some).map_err(|_| "La clave guardada está dañada.".into())
                }
                0 | NO_ESTA => Ok(None),
                otro => Err(en_palabras(otro)),
            }
        }
    }

    pub fn borrar(servicio: &str, cuenta: &str) -> Result<(), String> {
        // SAFETY: igual que arriba.
        match unsafe { SecItemDelete(consulta(servicio, cuenta, &[]).as_concrete_TypeRef()) } {
            0 | NO_ESTA => Ok(()),
            otro => Err(en_palabras(otro)),
        }
    }
}

#[cfg(windows)]
mod sistema {
    use windows::Win32::Foundation::ERROR_NOT_FOUND;
    use windows::Win32::Security::Credentials::{
        CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW, CredWriteW,
    };
    use windows::core::{HSTRING, PWSTR};

    /// En el Administrador de credenciales: "Nexo · Tuya / Smart Life (<cuenta>)".
    fn destino(servicio: &str, cuenta: &str) -> HSTRING {
        HSTRING::from(format!("{servicio} ({cuenta})"))
    }

    fn no_esta(e: &windows::core::Error) -> bool {
        e.code() == ERROR_NOT_FOUND.to_hresult()
    }

    pub fn guardar(servicio: &str, cuenta: &str, secreto: &str) -> Result<(), String> {
        let mut nombre: Vec<u16> = format!("{servicio} ({cuenta})").encode_utf16().chain(std::iter::once(0)).collect();
        let mut usuario: Vec<u16> = cuenta.encode_utf16().chain(std::iter::once(0)).collect();
        let mut datos = secreto.as_bytes().to_vec();
        let c = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(nombre.as_mut_ptr()),
            CredentialBlobSize: datos.len() as u32,
            CredentialBlob: datos.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(usuario.as_mut_ptr()),
            ..Default::default()
        };
        // SAFETY: todos los punteros viven hasta después de la llamada.
        unsafe { CredWriteW(&c, 0) }.map_err(|e| format!("El Administrador de credenciales no guardó la clave ({e})."))
    }

    pub fn leer(servicio: &str, cuenta: &str) -> Result<Option<String>, String> {
        let mut c: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: CredReadW llena `c`; se libera con CredFree.
        match unsafe { CredReadW(&destino(servicio, cuenta), CRED_TYPE_GENERIC, None, &mut c) } {
            Ok(()) if !c.is_null() => {
                // SAFETY: `c` es válido hasta CredFree; se copian los bytes antes.
                let bytes = unsafe {
                    let cred = &*c;
                    let v = std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize).to_vec();
                    CredFree(c as *const _);
                    v
                };
                String::from_utf8(bytes).map(Some).map_err(|_| "La clave guardada está dañada.".into())
            }
            Ok(()) => Ok(None),
            Err(e) if no_esta(&e) => Ok(None),
            Err(e) => Err(format!("El Administrador de credenciales no dio la clave ({e}).")),
        }
    }

    pub fn borrar(servicio: &str, cuenta: &str) -> Result<(), String> {
        // SAFETY: llamada simple con una cadena que vive hasta el final.
        match unsafe { CredDeleteW(&destino(servicio, cuenta), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(e) if no_esta(&e) => Ok(()),
            Err(e) => Err(format!("El Administrador de credenciales no borró la clave ({e}).")),
        }
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod sistema {
    pub fn guardar(_: &str, _: &str, _: &str) -> Result<(), String> {
        Err("Este sistema no tiene llavero.".into())
    }
    pub fn leer(_: &str, _: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    pub fn borrar(_: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
}
