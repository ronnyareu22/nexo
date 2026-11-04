//! Aviso de brillo en pantalla (vidrio, abajo al centro del monitor que cambió).
//!
//! Es una ventanita transparente que se crea al primer uso, se oculta sola a los 1,4 s
//! y se cierra del todo tras 60 s sin uso para no ocupar memoria.

use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const ETIQUETA: &str = "aviso";
const ANCHO: f64 = 320.0;
const ALTO: f64 = 84.0;

static ULTIMO: Mutex<Option<Instant>> = Mutex::new(None);
static NIVEL: Mutex<u8> = Mutex::new(0);

/// El nivel más reciente (la ventana lo pide al abrirse por primera vez).
pub fn nivel_actual() -> u8 {
    NIVEL.lock().map(|n| *n).unwrap_or(0)
}

pub fn mostrar(app: &AppHandle, pantalla: u32, nivel: u8) {
    if let Ok(mut n) = NIVEL.lock() {
        *n = nivel;
    }
    let marca = Instant::now();
    if let Ok(mut u) = ULTIMO.lock() {
        *u = Some(marca);
    }

    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        let ventana = match app2.get_webview_window(ETIQUETA) {
            Some(v) => v,
            None => match crear(&app2) {
                Some(v) => v,
                None => return,
            },
        };
        #[cfg(not(windows))]
        if let Some((x, y)) = posicion(pantalla) {
            let _ = ventana.set_position(tauri::LogicalPosition::new(x, y));
        }
        #[cfg(windows)]
        if let Some(p) = posicion_windows(pantalla) {
            // Si el monitor tiene otra escala, Windows le cambia el tamaño a la ventana al llegar:
            // la segunda vez queda justo en su lugar.
            let _ = ventana.set_position(p);
            let _ = ventana.set_position(p);
        }
        let _ = app2.emit_to(ETIQUETA, "aviso-brillo", nivel);
        let _ = ventana.show();
    });

    // Ocultar a los 1,4 s si no hubo otro toque; cerrar a los 60 s.
    let app3 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1400));
        if ULTIMO.lock().ok().and_then(|u| *u) != Some(marca) {
            return;
        }
        let a = app3.clone();
        let _ = app3.run_on_main_thread(move || {
            if let Some(v) = a.get_webview_window(ETIQUETA) {
                let _ = v.hide();
            }
        });
        std::thread::sleep(Duration::from_secs(60));
        if ULTIMO.lock().ok().and_then(|u| *u) != Some(marca) {
            return;
        }
        let a = app3.clone();
        let _ = app3.run_on_main_thread(move || {
            if let Some(v) = a.get_webview_window(ETIQUETA) {
                let _ = v.destroy();
            }
        });
    });
}

fn crear(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    // En desarrollo la sirve Vite como ruta; en la app final es un archivo aparte.
    // Siempre "aviso": Tauri sirve aviso.html, y así la dirección (/aviso) coincide con la ruta de SvelteKit.
    let pagina = "aviso";
    let ventana = WebviewWindowBuilder::new(app, ETIQUETA, WebviewUrl::App(pagina.into()))
        .title("Brillo")
        .inner_size(ANCHO, ALTO)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .visible(false)
        .visible_on_all_workspaces(true)
        .build()
        .ok()?;
    let _ = ventana.set_ignore_cursor_events(true);
    Some(ventana)
}

/// Abajo al centro de la pantalla indicada (coordenadas lógicas, origen arriba a la izquierda).
#[cfg(target_os = "macos")]
fn posicion(pantalla: u32) -> Option<(f64, f64)> {
    #[repr(C)]
    struct Rect {
        x: f64,
        y: f64,
        ancho: f64,
        alto: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGDisplayBounds(id: u32) -> Rect;
    }
    let r = unsafe { CGDisplayBounds(pantalla) };
    (r.ancho > 0.0).then(|| (r.x + (r.ancho - ANCHO) / 2.0, r.y + r.alto - ALTO - 90.0))
}

/// Windows: abajo al centro de la zona útil del monitor (sin la barra de tareas), a unos 90 px del borde.
/// Va en píxeles físicos con la escala de ese monitor: en lógicos, Tauri usaría la escala del monitor
/// donde está la ventana ahora, que puede ser otro.
#[cfg(windows)]
fn posicion_windows(pantalla: u32) -> Option<tauri::PhysicalPosition<i32>> {
    let a = crate::dispositivos::pantallas::area_de_trabajo(pantalla)?;
    let (ancho, alto, margen) = (ANCHO * a.escala, ALTO * a.escala, 90.0 * a.escala);
    let x = a.izquierda as f64 + ((a.derecha - a.izquierda) as f64 - ancho) / 2.0;
    let y = (a.abajo as f64 - alto - margen).max(a.arriba as f64);
    Some(tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn posicion(_pantalla: u32) -> Option<(f64, f64)> {
    None
}
