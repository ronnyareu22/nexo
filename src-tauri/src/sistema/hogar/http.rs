//! HTTP y HTTPS con lo que trae el sistema (sin crates nuevos): NSURLSession en la Mac y WinHTTP en Windows.
//! Lo usan la nube de Tuya y Home Assistant. Una petición a la vez por llamada, con plazo; nada queda abierto.
//! - Mac: sesión efímera (sin caché ni cookies en disco: las respuestas pueden traer tokens).
//! - Los errores van en palabras; nunca llevan las cabeceras (ahí van el token y la firma).

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
    sistema::pedir(metodo, url, cabeceras, cuerpo, plazo)
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
    use super::{MAX_CUERPO, Respuesta};
    use block2::RcBlock;
    use objc2_foundation::{
        NSData, NSError, NSHTTPURLResponse, NSMutableURLRequest, NSString, NSURL, NSURLRequestCachePolicy, NSURLResponse,
        NSURLSession, NSURLSessionConfiguration,
    };
    use std::sync::mpsc;
    use std::time::Duration;

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
        let sesion = NSURLSession::sessionWithConfiguration(&ajustes);

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
    use super::{MAX_CUERPO, Respuesta, partes};
    use std::ffi::c_void;
    use std::time::Duration;
    use windows::Win32::Networking::WinHttp::*;
    use windows::core::{HSTRING, PCWSTR};

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
            WinHttpSendRequest(
                pedido.0,
                (!anchas.is_empty()).then_some(anchas.as_slice()),
                (!datos.is_empty()).then_some(datos.as_ptr() as *const c_void),
                largo,
                largo,
                0,
            )
            .map_err(|e| en_palabras(&e))?;
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

    pub fn pedir(_: &str, _: &str, _: &[(String, String)], _: Option<&[u8]>, _: Duration) -> Result<Respuesta, String> {
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
