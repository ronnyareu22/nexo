//! Widget de escritorio (Mac y Windows): una tarjeta pequeña sin bordes con los audífonos,
//! las pantallas, el Bluetooth y Nexo Bot (la página es `src/routes/widget`).
//!
//! - Queda debajo de las demás ventanas, en todos los escritorios y fuera de la barra de tareas.
//! - Se arrastra a cualquier lado. La posición se guarda en ajustes.json cuando se suelta
//!   y se recupera al abrir; si quedó fuera de las pantallas (se desconectó un monitor),
//!   vuelve arriba a la derecha de la pantalla principal.
//! - Al apagarlo se destruye su ventana, así no ocupa memoria.

use super::ajustes;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::menu::CheckMenuItem;
use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent, Wry};

pub const ETIQUETA: &str = "widget";
/// Tamaño de la ventana en puntos. La tarjeta deja 6 px transparentes alrededor para su sombra.
const ANCHO: f64 = 568.0;
const ALTO: f64 = 150.0;
/// Distancia al borde de la pantalla (más los 6 px de la sombra: unos 24 px a la vista).
const MARGEN: f64 = 18.0;
/// Tiempo quieto antes de guardar la posición.
const ESPERA: Duration = Duration::from_millis(700);

/// La casilla "Widget en el escritorio" del menú del ícono, para ponerla al día desde cualquier lado.
pub struct CasillaBandeja(pub CheckMenuItem<Wry>);

static CREADO: Mutex<Option<Instant>> = Mutex::new(None);
static ULTIMO_MOVIMIENTO: Mutex<Option<Instant>> = Mutex::new(None);
static ESPERANDO: AtomicBool = AtomicBool::new(false);

/// Si el widget está encendido en los ajustes.
pub fn activo(app: &AppHandle) -> bool {
    ajustes::cargar(app).widget
}

/// Al arrancar: lo abre si está encendido. Si falla, el menú lo muestra apagado (y se reintenta al volver a abrir Nexo).
pub fn iniciar(app: &AppHandle) {
    if !activo(app) {
        return;
    }
    if let Err(e) = crear(app) {
        eprintln!("No se pudo abrir el widget: {e}");
        if let Some(casilla) = app.try_state::<CasillaBandeja>() {
            let _ = casilla.0.set_checked(false);
        }
    }
}

/// Enciende o apaga el widget, lo guarda y pone al día el menú del ícono. Devuelve cómo quedó.
pub fn usar(app: &AppHandle, activar: bool) -> Result<bool, String> {
    let resultado = if activar {
        crear(app).map_err(|e| format!("No se pudo abrir el widget: {e}"))
    } else {
        quitar(app).map_err(|e| format!("No se pudo quitar el widget: {e}"))
    };
    let quedo = activar && resultado.is_ok();
    ajustes::cambiar(app, |a| a.widget = quedo)?;
    if let Some(casilla) = app.try_state::<CasillaBandeja>() {
        let _ = casilla.0.set_checked(quedo);
    }
    resultado.map(|_| quedo)
}

/// Desde el menú del ícono: lo contrario de lo que hay ahora.
pub fn alternar(app: &AppHandle) {
    let abierto = app.get_webview_window(ETIQUETA).is_some();
    if let Err(e) = usar(app, !abierto) {
        eprintln!("{e}");
    }
}

/// Le pide al widget que vuelva a leer los datos (el vigía lo llama cuando algo cambió).
pub fn avisar_cambio(app: &AppHandle) {
    if app.get_webview_window(ETIQUETA).is_some() {
        let _ = app.emit_to(ETIQUETA, "widget-actualizar", ());
    }
}

fn quitar(app: &AppHandle) -> Result<(), String> {
    match app.get_webview_window(ETIQUETA) {
        Some(v) => v.destroy().map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

fn crear(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(ETIQUETA).is_some() {
        return Ok(());
    }
    let lugar = lugar(app);

    // "widget" sirve en desarrollo (ruta de Vite) y en la app final: Tauri entrega widget.html,
    // y la dirección queda como /widget, que es la ruta que reconoce SvelteKit.
    let constructor = WebviewWindowBuilder::new(app, ETIQUETA, WebviewUrl::App("widget".into()))
        .title("Widget de Nexo")
        .inner_size(ANCHO, ALTO)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .always_on_bottom(true)
        .visible_on_all_workspaces(true)
        .focused(false)
        // Un clic en el widget sin foco también llega al botón.
        .accept_first_mouse(true);

    // Mac: se crea ya en su lugar y a la vista (mostrarlo después le daría el foco).
    #[cfg(target_os = "macos")]
    let constructor = match lugar {
        Some((x, y)) => constructor.position(x, y),
        None => constructor,
    };
    // Windows: se crea oculto, se mueve en píxeles reales y recién ahí se muestra (sin quitar el foco).
    #[cfg(not(target_os = "macos"))]
    let constructor = constructor.visible(false);

    if let Ok(mut c) = CREADO.lock() {
        *c = Some(Instant::now());
    }
    let ventana = constructor.build().map_err(|e| e.to_string())?;

    #[cfg(not(target_os = "macos"))]
    {
        if let Some((x, y)) = lugar {
            let _ = ventana.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
        }
        let _ = ventana.show();
    }

    let app2 = app.clone();
    ventana.on_window_event(move |evento| {
        if let WindowEvent::Moved(_) = evento {
            al_mover(&app2);
        }
    });
    Ok(())
}

// ---------------------------------------------------------------- Posición

/// Mac ubica las ventanas en puntos y Windows en píxeles reales. Todo aquí va en esa unidad,
/// así la posición sirve aunque las pantallas tengan escalas distintas.
#[cfg(target_os = "macos")]
pub(super) fn a_nativa(p: PhysicalPosition<i32>, escala: f64) -> (i32, i32) {
    ((p.x as f64 / escala).round() as i32, (p.y as f64 / escala).round() as i32)
}
#[cfg(not(target_os = "macos"))]
pub(super) fn a_nativa(p: PhysicalPosition<i32>, _escala: f64) -> (i32, i32) {
    (p.x, p.y)
}

/// Cuánto mide un punto en la unidad nativa de esa pantalla (1 en Mac; la escala en Windows).
pub(super) fn punto(m: &Monitor) -> f64 {
    if cfg!(target_os = "macos") { 1.0 } else { m.scale_factor() }
}

/// Lo útil de una pantalla (sin barra de menús, Dock ni barra de tareas): x, y, ancho, alto.
pub(super) fn area_util(m: &Monitor) -> (f64, f64, f64, f64) {
    let a = m.work_area();
    // Tauri la da en píxeles con la escala de esa pantalla; en Mac se pasa a puntos.
    let e = if cfg!(target_os = "macos") { m.scale_factor() } else { 1.0 };
    (
        a.position.x as f64 / e,
        a.position.y as f64 / e,
        a.size.width as f64 / e,
        a.size.height as f64 / e,
    )
}

/// Arriba a la derecha de la pantalla principal.
fn posicion_inicial(app: &AppHandle) -> Option<(f64, f64)> {
    let m = app
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| app.available_monitors().ok()?.into_iter().next())?;
    let (x, y, ancho, _) = area_util(&m);
    let p = punto(&m);
    Some((x + ancho - (ANCHO + MARGEN) * p, y + MARGEN * p))
}

/// Si en esa posición se ve una buena parte de una ventana de ese tamaño (en puntos) en alguna pantalla.
pub(super) fn se_ve(app: &AppHandle, (x, y): (f64, f64), (ancho, alto): (f64, f64)) -> bool {
    let Ok(pantallas) = app.available_monitors() else { return false };
    let (ancho_min, alto_min) = ((ancho * 0.6).min(160.0), (alto * 0.6).min(80.0));
    pantallas.iter().any(|m| {
        let (ax, ay, aw, ah) = area_util(m);
        let p = punto(m);
        let visto_ancho = (x + ancho * p).min(ax + aw) - x.max(ax);
        let visto_alto = (y + alto * p).min(ay + ah) - y.max(ay);
        visto_ancho >= ancho_min * p && visto_alto >= alto_min * p
    })
}

/// La posición guardada si todavía se ve; si no, la de siempre.
fn lugar(app: &AppHandle) -> Option<(f64, f64)> {
    ajustes::cargar(app)
        .widget_posicion
        .map(|(x, y)| (x as f64, y as f64))
        .filter(|&p| se_ve(app, p, (ANCHO, ALTO)))
        .or_else(|| posicion_inicial(app))
}

/// Cada movimiento renueva la espera; un solo hilo guarda cuando el widget queda quieto.
fn al_mover(app: &AppHandle) {
    // Al crearlo, Windows avisa que se movió a su lugar: eso no es la persona arrastrándolo.
    if CREADO.lock().ok().and_then(|c| *c).is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
        return;
    }
    if let Ok(mut u) = ULTIMO_MOVIMIENTO.lock() {
        *u = Some(Instant::now());
    }
    if ESPERANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let quieto = ULTIMO_MOVIMIENTO.lock().ok().and_then(|u| *u).is_none_or(|t| t.elapsed() >= ESPERA);
            if quieto {
                break;
            }
        }
        ESPERANDO.store(false, Ordering::SeqCst);
        guardar_posicion(&app);
    });
}

fn guardar_posicion(app: &AppHandle) {
    let Some(v) = app.get_webview_window(ETIQUETA) else { return };
    let (Ok(pos), Ok(escala)) = (v.outer_position(), v.scale_factor()) else { return };
    let nueva = a_nativa(pos, escala);
    if ajustes::cargar(app).widget_posicion == Some(nueva) {
        return;
    }
    if let Err(e) = ajustes::cambiar(app, |a| a.widget_posicion = Some(nueva)) {
        eprintln!("No se pudo guardar dónde quedó el widget: {e}");
    }
}
