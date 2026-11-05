//! HTTP y HTTPS con lo que trae el sistema (sin crates nuevos): NSURLSession en la Mac y WinHTTP en Windows.
//! Lo usan la nube de Tuya, Home Assistant y el puente Hue. Una petición a la vez por llamada, con plazo; nada
//! queda abierto.
//! - Mac: sesión efímera (sin caché ni cookies en disco: las respuestas pueden traer tokens).
//! - Certificado fijado (`pedir_fijado`, para el puente Hue, que usa un certificado propio y no uno de internet):
//!   la primera vez se acepta y se anota su huella SHA-256 (la del certificado del aparato); después solo se
//!   acepta ese mismo certificado. En la Mac lo decide un delegado de NSURLSession al abrir la conexión; en Windows,
//!   el aviso `SENDING_REQUEST` de WinHTTP, antes de mandar la petición (si no coincide, se cierra sin mandar nada).
//! - Los errores van en palabras; nunca llevan las cabeceras (ahí van el token y la firma).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Lo que respondió el servidor.
pub struct Respuesta {
    pub estado: u16,
    pub cuerpo: Vec<u8>,
}

/// Como mucho 4 MB de respuesta (la lista de estados de un Home Assistant grande cabe de sobra).
const MAX_CUERPO: usize = 4 * 1024 * 1024;

/// Hace una petición. `cabeceras` como pares; `cuerpo` solo en POST.
pub fn pedir(
    metodo: &str,
    url: &str,
    cabeceras: &[(String, String)],
    cuerpo: Option<&[u8]>,
    plazo: Duration,
) -> Result<Respuesta, String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("La dirección tiene que empezar con http:// o https://.".into());
    }
    sistema::pedir(metodo, url, cabeceras, cuerpo, plazo, None)
}

/// La huella que se espera del certificado y la que se vio (se comparte con el delegado o el aviso de WinHTTP).
pub struct Fijar {
    esperada: Option<[u8; 32]>,
    vista: Mutex<Option<[u8; 32]>>,
    rechazado: AtomicBool,
}

impl Fijar {
    fn nuevo(esperada: Option<[u8; 32]>) -> Self {
        Fijar { esperada, vista: Mutex::new(None), rechazado: AtomicBool::new(false) }
    }

    /// Anota la huella vista y dice si se acepta: la esperada o, si todavía no hay, cualquiera.
    fn decidir(&self, vista: Option<[u8; 32]>) -> bool {
        if let Ok(mut v) = self.vista.lock() {
            *v = vista;
        }
        let ok = match (self.esperada, vista) {
            (Some(e), Some(v)) => e == v,
            (None, Some(_)) => true,
            _ => false,
        };
        if !ok {
            self.rechazado.store(true, Ordering::SeqCst);
        }
        ok
    }

    fn vista(&self) -> Option<[u8; 32]> {
        self.vista.lock().ok().and_then(|v| *v)
    }
}

/// El error de un certificado que no es el de antes.
pub const OTRO_CERTIFICADO: &str =
    "El aparato no es el mismo de antes: su certificado cambió. Si lo cambiaste o lo restableciste, quítalo de Nexo y vincúlalo de nuevo.";

/// HTTPS con el certificado fijado: con `esperada`, solo ese; sin ella, el que venga (y se devuelve su huella para
/// guardarla). Solo `https://`.
pub fn pedir_fijado(
    metodo: &str,
    url: &str,
    cabeceras: &[(String, String)],
    cuerpo: Option<&[u8]>,
    plazo: Duration,
    esperada: Option<[u8; 32]>,
) -> Result<(Respuesta, [u8; 32]), String> {
    if !url.starts_with("https://") {
        return Err("Esa dirección tiene que ser https://.".into());
    }
    let fijar = std::sync::Arc::new(Fijar::nuevo(esperada));
    let r = sistema::pedir(metodo, url, cabeceras, cuerpo, plazo, Some(fijar.clone()));
    if fijar.rechazado.load(Ordering::SeqCst) {
        return Err(OTRO_CERTIFICADO.into());
    }
    let r = r?;
    let huella = fijar.vista().ok_or("No se pudo leer el certificado del aparato.")?;
    Ok((r, huella))
}

/// Las partes de una dirección: si es segura, el servidor, el puerto y la ruta con su consulta.
#[cfg_attr(not(windows), allow(dead_code))]
fn partes(url: &str) -> Option<(bool, String, u16, String)> {
    let (segura, resto) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else {
        (false, url.strip_prefix("http://")?)
    };
    let (anfitrion, ruta) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, "/"),
    };
    // IPv6 entre corchetes ([fd7a::1]:8123) o nombre/IPv4 con puerto opcional.
    let (servidor, puerto) = if let Some(r) = anfitrion.strip_prefix('[') {
        let fin = r.find(']')?;
        let puerto = r[fin + 1..].strip_prefix(':').map(|p| p.parse::<u16>().ok()).unwrap_or(Some(if segura { 443 } else { 80 }))?;
        (r[..fin].to_string(), puerto)
    } else {
        match anfitrion.rsplit_once(':') {
            Some((s, p)) => (s.to_string(), p.parse::<u16>().ok()?),
            None => (anfitrion.to_string(), if segura { 443 } else { 80 }),
        }
    };
    if servidor.is_empty() {
        return None;
    }
    Some((segura, servidor, puerto, ruta.to_string()))
}

#[cfg(target_os = "macos")]
mod sistema {
    use super::{Fijar, MAX_CUERPO, Respuesta};
    use block2::{DynBlock, RcBlock};
    use core_foundation::base::TCFType;
    use core_foundation::data::{CFData, CFDataRef};
    use objc2::encode::{Encoding, RefEncode};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{AnyThread, ClassType, DefinedClass, class, define_class, msg_send};
    use objc2_foundation::{
        NSData, NSError, NSHTTPURLResponse, NSMutableURLRequest, NSObjectProtocol, NSString, NSURL, NSURLAuthenticationMethodServerTrust,
        NSURLRequestCachePolicy, NSURLResponse, NSURLSession, NSURLSessionConfiguration,
    };
    use sha2::{Digest, Sha256};
    use std::ffi::c_void;
    use std::ptr::null_mut;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    /// La confianza de una conexión TLS (SecTrustRef).
    #[repr(C)]
    pub struct SecTrust {
        _p: [u8; 0],
    }
    // SAFETY: SecTrustRef es un puntero a `struct __SecTrust`.
    unsafe impl RefEncode for SecTrust {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__SecTrust", &[]));
    }

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecTrustGetCertificateCount(confianza: *mut SecTrust) -> isize;
        fn SecTrustGetCertificateAtIndex(confianza: *mut SecTrust, indice: isize) -> *const c_void;
        fn SecCertificateCopyData(certificado: *const c_void) -> CFDataRef;
    }

    /// La huella SHA-256 del certificado del servidor (el primero de la cadena).
    unsafe fn huella(confianza: *mut SecTrust) -> Option<[u8; 32]> {
        if confianza.is_null() {
            return None;
        }
        // SAFETY: `confianza` es la de la conexión mientras dura el reto; el CFData es nuestro (regla de copia).
        unsafe {
            if SecTrustGetCertificateCount(confianza) < 1 {
                return None;
            }
            let cert = SecTrustGetCertificateAtIndex(confianza, 0);
            if cert.is_null() {
                return None;
            }
            let datos = SecCertificateCopyData(cert);
            if datos.is_null() {
                return None;
            }
            let datos = CFData::wrap_under_create_rule(datos);
            Some(Sha256::digest(datos.bytes()).into())
        }
    }

    struct Ivars {
        fijar: Arc<Fijar>,
    }

    define_class!(
        // SAFETY: NSObject no pide nada especial a sus subclases y `Confianza` no implementa Drop.
        #[unsafe(super(NSObject))]
        #[name = "NexoHogarConfianza"]
        #[ivars = Ivars]
        struct Confianza;

        impl Confianza {
            /// El reto de la conexión: se acepta el certificado solo si su huella es la esperada.
            #[unsafe(method(URLSession:didReceiveChallenge:completionHandler:))]
            fn al_retar(&self, _sesion: *mut AnyObject, reto: *mut AnyObject, listo: &DynBlock<dyn Fn(isize, *mut AnyObject)>) {
                // 0: usar la credencial; 1: lo de siempre; 2: cancelar.
                if reto.is_null() {
                    listo.call((1, null_mut()));
                    return;
                }
                // SAFETY: `reto` es el NSURLAuthenticationChallenge que entrega NSURLSession durante esta llamada.
                unsafe {
                    let espacio: *mut AnyObject = msg_send![reto, protectionSpace];
                    let metodo: *mut NSString = if espacio.is_null() { null_mut() } else { msg_send![espacio, authenticationMethod] };
                    if metodo.is_null() || !(*metodo).isEqualToString(NSURLAuthenticationMethodServerTrust) {
                        listo.call((1, null_mut()));
                        return;
                    }
                    let confianza: *mut SecTrust = msg_send![espacio, serverTrust];
                    if !self.ivars().fijar.decidir(huella(confianza)) {
                        listo.call((2, null_mut()));
                        return;
                    }
                    let credencial: *mut AnyObject = msg_send![class!(NSURLCredential), credentialForTrust: confianza];
                    listo.call((0, credencial));
                }
            }
        }

        unsafe impl NSObjectProtocol for Confianza {}
    );

    impl Confianza {
        fn nuevo(fijar: Arc<Fijar>) -> Retained<Self> {
            let este = Self::alloc().set_ivars(Ivars { fijar });
            // SAFETY: `init` de NSObject sobre un objeto recién reservado.
            unsafe { msg_send![super(este), init] }
        }
    }

    /// Un error de NSURLSession en palabras (los códigos de NSURLErrorDomain).
    fn en_palabras(codigo: isize) -> String {
        match codigo {
            -1001 => "No respondió a tiempo.".into(),
            -1005..=-1003 => "No encontré el servidor. Revisa la dirección y tu conexión.".into(),
            -1009 => "Este equipo no tiene internet.".into(),
            -1022 => "El sistema no deja conectarse a esa dirección sin https.".into(),
            -1206..=-1200 => {
                "No se pudo abrir una conexión segura (el certificado no es válido).".into()
            }
            otro => format!("No se pudo conectar (error {otro})."),
        }
    }

    pub fn pedir(
        metodo: &str,
        url: &str,
        cabeceras: &[(String, String)],
        cuerpo: Option<&[u8]>,
        plazo: Duration,
        fijar: Option<Arc<Fijar>>,
    ) -> Result<Respuesta, String> {
        let direccion = NSURL::URLWithString(&NSString::from_str(url)).ok_or("Esa dirección no es válida.")?;
        let pedido = NSMutableURLRequest::requestWithURL_cachePolicy_timeoutInterval(
            &direccion,
            NSURLRequestCachePolicy::ReloadIgnoringLocalCacheData,
            plazo.as_secs_f64(),
        );
        pedido.setHTTPMethod(&NSString::from_str(metodo));
        for (k, v) in cabeceras {
            pedido.setValue_forHTTPHeaderField(Some(&NSString::from_str(v)), &NSString::from_str(k));
        }
        if let Some(c) = cuerpo {
            pedido.setHTTPBody(Some(&NSData::with_bytes(c)));
        }
        // Efímera: sin caché ni cookies en disco.
        let ajustes = NSURLSessionConfiguration::ephemeralSessionConfiguration();
        ajustes.setTimeoutIntervalForRequest(plazo.as_secs_f64());
        ajustes.setTimeoutIntervalForResource(plazo.as_secs_f64() + 2.0);
        let sesion: Retained<NSURLSession> = match fijar {
            // SAFETY: el delegado es un NSObject que responde al reto; la sesión lo retiene hasta `invalidateAndCancel`.
            Some(f) => unsafe {
                let delegado = Confianza::nuevo(f);
                msg_send![
                    NSURLSession::class(),
                    sessionWithConfiguration: &*ajustes,
                    delegate: &*delegado,
                    delegateQueue: null_mut::<AnyObject>()
                ]
            },
            None => NSURLSession::sessionWithConfiguration(&ajustes),
        };

        let (tx, rx) = mpsc::channel::<Result<Respuesta, String>>();
        let bloque = RcBlock::new(move |datos: *mut NSData, respuesta: *mut NSURLResponse, error: *mut NSError| {
            let r = if !error.is_null() {
                // SAFETY: NSURLSession pasa un NSError válido mientras dura la llamada.
                Err(en_palabras(unsafe { &*error }.code()))
            } else {
                // SAFETY: igual que arriba; la respuesta de una URL http(s) es un NSHTTPURLResponse.
                let estado = (!respuesta.is_null())
                    .then(|| unsafe { &*respuesta }.downcast_ref::<NSHTTPURLResponse>().map(|h| h.statusCode()))
                    .flatten()
                    .unwrap_or(0);
                // SAFETY: los datos viven mientras dura la llamada; se copian.
                let bytes = if datos.is_null() { Vec::new() } else { unsafe { &*datos }.to_vec() };
                if bytes.len() > MAX_CUERPO {
                    Err("La respuesta es demasiado grande.".into())
                } else {
                    Ok(Respuesta { estado: estado.clamp(0, u16::MAX as isize) as u16, cuerpo: bytes })
                }
            };
            let _ = tx.send(r);
        });
        // SAFETY: el bloque solo usa un `Sender` (se puede mandar entre hilos); NSURLSession lo copia y lo llama una vez.
        let tarea = unsafe { sesion.dataTaskWithRequest_completionHandler(&pedido, &bloque) };
        tarea.resume();
        let r = rx.recv_timeout(plazo + Duration::from_secs(3));
        // Sin tareas ni sesión colgando: se cancela lo pendiente y se libera.
        sesion.invalidateAndCancel();
        r.unwrap_or_else(|_| Err("No respondió a tiempo.".into()))
    }
}

#[cfg(windows)]
mod sistema {
    use super::{Fijar, MAX_CUERPO, Respuesta, partes};
    use sha2::{Digest, Sha256};
    use std::ffi::c_void;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use windows::Win32::Networking::WinHttp::*;
    use windows::Win32::Security::Cryptography::{CERT_CONTEXT, CertFreeCertificateContext};
    use windows::core::{HSTRING, PCWSTR};

    /// WINHTTP_CALLBACK_FLAG_SEND_REQUEST (SENDING_REQUEST | REQUEST_SENT).
    const AVISO_ENVIO: u32 = 0x30;

    /// Lo que comparte la petición con su aviso.
    struct Contexto {
        fijar: Arc<Fijar>,
        /// El aviso cerró la petición (para no cerrarla dos veces).
        cerrada: AtomicBool,
    }

    /// Antes de mandar la petición (ya con la conexión TLS abierta): se mira el certificado del servidor y, si no
    /// es el esperado, se cierra la petición sin mandar nada.
    unsafe extern "system" fn al_enviar(h: *mut c_void, contexto: usize, estado: u32, _info: *mut c_void, _largo: u32) {
        if estado != WINHTTP_CALLBACK_STATUS_SENDING_REQUEST || contexto == 0 {
            return;
        }
        // SAFETY: `contexto` apunta al `Contexto` de `pedir`, que vive hasta que vuelve WinHttpSendRequest.
        let c = unsafe { &*(contexto as *const Contexto) };
        let mut cert: *mut CERT_CONTEXT = std::ptr::null_mut();
        let mut tam = std::mem::size_of::<*mut CERT_CONTEXT>() as u32;
        // SAFETY: WinHTTP llena `cert` con un CERT_CONTEXT propio que se libera aquí mismo.
        let huella = unsafe {
            let ok = WinHttpQueryOption(h, WINHTTP_OPTION_SERVER_CERT_CONTEXT, Some(&mut cert as *mut _ as *mut c_void), &mut tam).is_ok();
            if ok && !cert.is_null() && !(*cert).pbCertEncoded.is_null() {
                let der = std::slice::from_raw_parts((*cert).pbCertEncoded, (*cert).cbCertEncoded as usize);
                let h: [u8; 32] = Sha256::digest(der).into();
                let _ = CertFreeCertificateContext(Some(cert));
                Some(h)
            } else {
                None
            }
        };
        if !c.fijar.decidir(huella) {
            c.cerrada.store(true, Ordering::SeqCst);
            // SAFETY: cerrar la petición la corta antes de mandar nada; `pedir` ya no la vuelve a cerrar.
            let _ = unsafe { WinHttpCloseHandle(h) };
        }
    }

    /// Cierra el manejador al salir, pase lo que pase.
    struct Manejador(*mut c_void);

    impl Drop for Manejador {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: es un manejador de WinHTTP abierto por nosotros y se cierra una sola vez.
                let _ = unsafe { WinHttpCloseHandle(self.0) };
            }
        }
    }

    fn en_palabras(e: &windows::core::Error) -> String {
        match (e.code().0 as u32) & 0xFFFF {
            12002 => "No respondió a tiempo.".into(),
            12007 | 12029 | 12030 => "No encontré el servidor. Revisa la dirección y tu conexión.".into(),
            12175 | 12044 | 12045 | 12169 => "No se pudo abrir una conexión segura (el certificado no es válido).".into(),
            otro => format!("No se pudo conectar (error {otro})."),
        }
    }

    pub fn pedir(
        metodo: &str,
        url: &str,
        cabeceras: &[(String, String)],
        cuerpo: Option<&[u8]>,
        plazo: Duration,
        fijar: Option<Arc<Fijar>>,
    ) -> Result<Respuesta, String> {
        let (segura, servidor, puerto, ruta) = partes(url).ok_or("Esa dirección no es válida.")?;
        let ms = plazo.as_millis().min(120_000) as i32;
        // SAFETY: llamadas a WinHTTP con cadenas que viven hasta el final; cada manejador se cierra con `Manejador`.
        unsafe {
            let sesion = Manejador(WinHttpOpen(
                &HSTRING::from("Nexo"),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ));
            if sesion.0.is_null() {
                return Err("No se pudo preparar la conexión.".into());
            }
            let _ = WinHttpSetTimeouts(sesion.0, ms, ms, ms, ms);
            let conexion = Manejador(WinHttpConnect(sesion.0, &HSTRING::from(servidor.as_str()), puerto, 0));
            if conexion.0.is_null() {
                return Err("No encontré el servidor. Revisa la dirección y tu conexión.".into());
            }
            let banderas = if segura { WINHTTP_FLAG_SECURE } else { WINHTTP_OPEN_REQUEST_FLAGS(0) };
            let pedido = Manejador(WinHttpOpenRequest(
                conexion.0,
                &HSTRING::from(metodo),
                &HSTRING::from(ruta.as_str()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                banderas,
            ));
            if pedido.0.is_null() {
                return Err("No se pudo preparar la petición.".into());
            }
            let texto: String = cabeceras.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
            let anchas: Vec<u16> = texto.encode_utf16().collect();
            let datos = cuerpo.unwrap_or(&[]);
            let largo = datos.len() as u32;
            // Con certificado fijado: se aceptan los propios (sin autoridad ni nombre) y decide `al_enviar`.
            let contexto = fijar.map(|fijar| Box::new(Contexto { fijar, cerrada: AtomicBool::new(false) }));
            if contexto.is_some() {
                let banderas = SECURITY_FLAG_IGNORE_UNKNOWN_CA
                    | SECURITY_FLAG_IGNORE_CERT_CN_INVALID
                    | SECURITY_FLAG_IGNORE_CERT_DATE_INVALID
                    | SECURITY_FLAG_IGNORE_CERT_WRONG_USAGE;
                WinHttpSetOption(Some(pedido.0), WINHTTP_OPTION_SECURITY_FLAGS, Some(&banderas.to_ne_bytes()))
                    .map_err(|_| "No se pudo preparar la conexión segura.".to_string())?;
                let _ = WinHttpSetStatusCallback(pedido.0, Some(al_enviar), AVISO_ENVIO, 0);
            }
            let ptr_contexto = contexto.as_ref().map(|c| &**c as *const Contexto as usize).unwrap_or(0);
            let enviado = WinHttpSendRequest(
                pedido.0,
                (!anchas.is_empty()).then_some(anchas.as_slice()),
                (!datos.is_empty()).then_some(datos.as_ptr() as *const c_void),
                largo,
                largo,
                ptr_contexto,
            );
            if contexto.as_ref().is_some_and(|c| c.cerrada.load(Ordering::SeqCst)) {
                // La cerró el aviso: no se cierra otra vez.
                std::mem::forget(pedido);
                return Err(super::OTRO_CERTIFICADO.into());
            }
            enviado.map_err(|e| en_palabras(&e))?;
            WinHttpReceiveResponse(pedido.0, std::ptr::null_mut()).map_err(|e| en_palabras(&e))?;

            let mut estado: u32 = 0;
            let mut tam = std::mem::size_of::<u32>() as u32;
            let _ = WinHttpQueryHeaders(
                pedido.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some(&mut estado as *mut u32 as *mut c_void),
                &mut tam,
                std::ptr::null_mut(),
            );
            let mut bytes = Vec::new();
            loop {
                let mut hay: u32 = 0;
                WinHttpQueryDataAvailable(pedido.0, &mut hay).map_err(|e| en_palabras(&e))?;
                if hay == 0 {
                    break;
                }
                if bytes.len() + hay as usize > MAX_CUERPO {
                    return Err("La respuesta es demasiado grande.".into());
                }
                let antes = bytes.len();
                bytes.resize(antes + hay as usize, 0);
                let mut leidos: u32 = 0;
                WinHttpReadData(pedido.0, bytes[antes..].as_mut_ptr() as *mut c_void, hay, &mut leidos)
                    .map_err(|e| en_palabras(&e))?;
                bytes.truncate(antes + leidos as usize);
                if leidos == 0 {
                    break;
                }
            }
            Ok(Respuesta { estado: estado.min(u16::MAX as u32) as u16, cuerpo: bytes })
        }
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod sistema {
    use super::Respuesta;
    use std::time::Duration;

    pub fn pedir(
        _: &str,
        _: &str,
        _: &[(String, String)],
        _: Option<&[u8]>,
        _: Duration,
        _: Option<std::sync::Arc<super::Fijar>>,
    ) -> Result<Respuesta, String> {
        Err("Este sistema no está disponible.".into())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn hogar_http_partes_de_la_direccion() {
        assert_eq!(
            partes("https://openapi.tuyaus.com/v1.0/token?grant_type=1"),
            Some((true, "openapi.tuyaus.com".into(), 443, "/v1.0/token?grant_type=1".into()))
        );
        assert_eq!(partes("http://homeassistant.local:8123"), Some((false, "homeassistant.local".into(), 8123, "/".into())));
        assert_eq!(partes("http://100.64.1.2:8123/api/"), Some((false, "100.64.1.2".into(), 8123, "/api/".into())));
        assert_eq!(partes("http://[fd7a:115c::1]:8123/api/"), Some((false, "fd7a:115c::1".into(), 8123, "/api/".into())));
        assert_eq!(partes("ftp://x"), None);
        assert_eq!(partes("http://:80/"), None);
    }

    #[test]
    fn hogar_http_fijar_certificado() {
        // Sin huella guardada se acepta la primera; con huella, solo esa.
        let primera = Fijar::nuevo(None);
        assert!(primera.decidir(Some([7; 32])));
        assert_eq!(primera.vista(), Some([7; 32]));
        let guardada = Fijar::nuevo(Some([7; 32]));
        assert!(!guardada.decidir(Some([8; 32])));
        assert!(guardada.rechazado.load(Ordering::SeqCst));
        assert!(!Fijar::nuevo(None).decidir(None));
    }

    /// Una sola petición real a Tuya con datos inventados: comprueba HTTPS y que llega su error en JSON.
    /// `cargo test --lib hogar_http_en_vivo -- --ignored`
    #[test]
    #[ignore]
    fn hogar_http_en_vivo() {
        let r = pedir("GET", "https://openapi.tuyaus.com/v1.0/token?grant_type=1", &[], None, Duration::from_secs(8));
        match r {
            Ok(r) => {
                let texto = String::from_utf8_lossy(&r.cuerpo).to_string();
                println!("estado {} · {texto}", r.estado);
                assert!(texto.contains("\"success\":false"));
            }
            Err(e) => panic!("{e}"),
        }
    }
}
