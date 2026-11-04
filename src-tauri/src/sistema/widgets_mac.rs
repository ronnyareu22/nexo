//! Widgets nativos de macOS (carpeta widget-mac/, WidgetKit), los que se ponen junto al Calendario o el Clima.
//!
//! - Nexo les deja el estado en `estado-widget.json` (carpeta de la app) cada vez que algo cambia.
//! - Sus botones de brillo llegan como notificaciones del sistema:
//!   "com.ronny.nexo.brillo.<pantalla>.<bajar|subir>". La pantalla 0 es la principal.
//! - Después de escribir, se le pide al sistema que redibuje los widgets (ayudante `nexo-recargar-widgets`).
//!
//! En Windows no hace nada: allá está el widget de escritorio propio (sistema/widget.rs).

use crate::dispositivos::Dispositivo;
use tauri::AppHandle;

/// Lo llama el vigía con la lista actual (alias ya puestos). Escribe solo si algo cambió.
#[cfg(target_os = "macos")]
pub fn actualizar(app: &AppHandle, bluetooth: bool, dispositivos: &[Dispositivo]) {
    mac::actualizar(app, bluetooth, Some(dispositivos));
}

#[cfg(not(target_os = "macos"))]
pub fn actualizar(_app: &AppHandle, _bluetooth: bool, _dispositivos: &[Dispositivo]) {}

/// Empieza a escuchar los botones de brillo de los widgets.
#[cfg(target_os = "macos")]
pub fn escuchar(app: AppHandle) {
    mac::escuchar(app);
}

#[cfg(not(target_os = "macos"))]
pub fn escuchar(_app: AppHandle) {}

#[cfg(target_os = "macos")]
mod mac {
    use crate::dispositivos::pantallas::Pantallas;
    use crate::dispositivos::Dispositivo;
    use serde::Serialize;
    use std::collections::HashMap;
    use std::ffi::{c_char, c_int, CString};
    use std::io::Read;
    use std::os::fd::FromRawFd;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tauri::{AppHandle, Manager};

    #[derive(Serialize)]
    struct Aparato {
        id: String,
        nombre: String,
        tipo: crate::dispositivos::Tipo,
        conectado: bool,
        bateria: Option<u8>,
        vendor: Option<String>,
        producto: Option<String>,
    }

    #[derive(Serialize)]
    struct PantallaWidget {
        nombre: String,
        interna: bool,
        brillo: Option<u8>,
    }

    #[derive(Serialize)]
    struct EstadoWidget<'a> {
        actualizado: u64,
        bluetooth: bool,
        dispositivos: &'a [Aparato],
        pantallas: &'a [PantallaWidget],
    }

    /// Lo último que se escribió, para no reescribir si nada cambió y para que los botones
    /// sepan qué pantalla es la 0 y la 1.
    struct Memoria {
        bluetooth: bool,
        aparatos: Vec<Aparato>,
        orden: Vec<u32>,
        firma: String,
    }
    static MEMORIA: Mutex<Option<Memoria>> = Mutex::new(None);

    fn ruta(app: &AppHandle) -> Option<std::path::PathBuf> {
        app.path().app_config_dir().ok().map(|d| d.join("estado-widget.json"))
    }

    pub fn actualizar(app: &AppHandle, bluetooth: bool, dispositivos: Option<&[Dispositivo]>) {
        // Pantallas: la principal primero (es la que muestra el widget pequeño).
        // Sin la pantalla virtual de RustDesk: no es una pantalla que se vea en la casa.
        let mut pantallas: Vec<_> = app.state::<Arc<Pantallas>>().listar().into_iter().filter(|p| !p.es_virtual).collect();
        pantallas.sort_by_key(|p| !p.principal);
        let orden: Vec<u32> = pantallas.iter().map(|p| p.id).collect();
        let lista_pantallas: Vec<PantallaWidget> = pantallas
            .iter()
            .map(|p| PantallaWidget {
                nombre: p.nombre.clone(),
                interna: p.interna,
                brillo: if p.controlable { p.brillo } else { None },
            })
            .collect();

        let Ok(mut memoria) = MEMORIA.lock() else { return };
        let aparatos: Vec<Aparato> = match dispositivos {
            Some(lista) => lista
                .iter()
                .map(|d| Aparato {
                    id: d.id.clone(),
                    nombre: d.alias.clone().filter(|a| !a.trim().is_empty()).unwrap_or_else(|| d.nombre.clone()),
                    tipo: d.tipo,
                    conectado: d.conectado,
                    bateria: d.bateria.or(match (d.bateria_izquierda, d.bateria_derecha) {
                        (Some(i), Some(r)) => Some(i.min(r)),
                        (i, r) => i.or(r),
                    }),
                    vendor: d.vendor_id.clone(),
                    producto: d.product_id.clone(),
                })
                .collect(),
            // Desde un botón de brillo: los aparatos no cambiaron.
            None => memoria.as_mut().map(|m| std::mem::take(&mut m.aparatos)).unwrap_or_default(),
        };
        let bluetooth = if dispositivos.is_some() { bluetooth } else { memoria.as_ref().is_some_and(|m| m.bluetooth) };

        let cuerpo = |actualizado| {
            serde_json::to_vec(&EstadoWidget {
                actualizado,
                bluetooth,
                dispositivos: &aparatos,
                pantallas: &lista_pantallas,
            })
            .unwrap_or_default()
        };
        let firma = String::from_utf8(cuerpo(0)).unwrap_or_default();
        let cambio = memoria.as_ref().is_none_or(|m| m.firma != firma);
        if cambio {
            if let Some(ruta) = ruta(app) {
                let ahora = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                // Se escribe aparte y se renombra, así el widget nunca lee un archivo a medias.
                let temporal = ruta.with_extension("json.tmp");
                if std::fs::write(&temporal, cuerpo(ahora)).is_ok() {
                    let _ = std::fs::rename(&temporal, &ruta);
                }
            }
        }
        *memoria = Some(Memoria { bluetooth, aparatos, orden, firma });
        drop(memoria);
        if cambio {
            recargar();
        }
    }

    /// Le pide al sistema que redibuje los widgets de Nexo ya mismo.
    fn recargar() {
        let Some(ayudante) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("nexo-recargar-widgets")))
        else {
            return;
        };
        if ayudante.exists() {
            let _ = std::process::Command::new(ayudante).spawn().map(|mut hijo| {
                std::thread::spawn(move || hijo.wait());
            });
        }
    }

    /// Un toque en − o +: el brillo va al múltiplo de 10 anterior o siguiente (31 → 30 → 20…; 31 → 40).
    fn cambiar_brillo(app: &AppHandle, indice: usize, subir: bool) {
        let id = MEMORIA.lock().ok().and_then(|m| m.as_ref().and_then(|m| m.orden.get(indice).copied()));
        let Some(id) = id else { return };
        let pantallas = app.state::<Arc<Pantallas>>();
        let actual = pantallas.brillos().into_iter().find(|(p, _)| *p == id).map(|(_, v)| v);
        let Some(actual) = actual else { return };
        let nuevo = if subir { (actual / 10 + 1) * 10 } else { actual.saturating_sub(1) / 10 * 10 }.min(100);
        if let Ok(nivel) = pantallas.poner_brillo(id, nuevo) {
            // El mismo aviso que con las teclas, para ver el cambio en esa pantalla.
            crate::sistema::aviso::mostrar(app, id, nivel);
        }
        actualizar(app, false, None);
    }

    pub fn escuchar(app: AppHandle) {
        unsafe extern "C" {
            fn notify_register_file_descriptor(nombre: *const c_char, fd: *mut c_int, opciones: c_int, token: *mut c_int) -> u32;
        }
        const NOTIFY_REUSE: c_int = 1;

        let _ = std::thread::Builder::new().name("nexo-widgets".into()).spawn(move || {
            let mut fd: c_int = -1;
            let mut tokens: HashMap<i32, (usize, bool)> = HashMap::new();
            for indice in 0..2 {
                for subir in [false, true] {
                    let Ok(nombre) = CString::new(format!("com.ronny.nexo.brillo.{indice}.{}", if subir { "subir" } else { "bajar" }))
                    else {
                        continue;
                    };
                    let mut token: c_int = 0;
                    let opciones = if fd >= 0 { NOTIFY_REUSE } else { 0 };
                    // SAFETY: nombre es una cadena C válida; fd y token son punteros a enteros vivos.
                    if unsafe { notify_register_file_descriptor(nombre.as_ptr(), &mut fd, opciones, &mut token) } == 0 {
                        tokens.insert(token, (indice, subir));
                    }
                }
            }
            if fd < 0 {
                return;
            }
            // SAFETY: fd lo creó notify y es solo de este hilo.
            let mut archivo = unsafe { std::fs::File::from_raw_fd(fd) };
            let mut bytes = [0u8; 4];
            // Espera bloqueado (sin gastar nada) hasta que llegue un toque.
            while archivo.read_exact(&mut bytes).is_ok() {
                if let Some(&(indice, subir)) = tokens.get(&i32::from_be_bytes(bytes)) {
                    cambiar_brillo(&app, indice, subir);
                }
            }
        });
    }
}
