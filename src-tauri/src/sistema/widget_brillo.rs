//! Widget de brillo de una sola pantalla (Mac y Windows; la página es `src/routes/brillo`).
//!
//! - Una tarjeta chica sin bordes por pantalla, con la etiqueta `brillo-<id>`. La página sabe su pantalla por
//!   la dirección (`brillo?id=<id>`) y, si no viene, por la etiqueta de su ventana.
//! - Como el widget de escritorio: debajo de las demás ventanas, en todos los escritorios y fuera de la barra de
//!   tareas. Se arrastra y recuerda dónde quedó, cada una la suya (ajustes.json, `widgetsBrillo`).
//! - La primera vez va abajo a la derecha de la zona útil de su pantalla, a 24 px del borde.
//! - Se enciende y se apaga desde la tarjeta de la pantalla (Pantallas) y desde el submenú Brillo del ícono.
//! - Sigue el brillo con el aviso "brillo-cambio" del hilo de pantallas y una lectura de la caché cada 2 s.
//! - Al apagarlo se destruye su ventana, así no ocupa memoria.

use super::ajustes::{self, WidgetBrillo};
use super::widget;
use crate::dispositivos::pantallas;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Monitor, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub const PREFIJO: &str = "brillo-";
/// Tamaño de la ventana en puntos. La tarjeta deja 6 px transparentes alrededor para su sombra.
/// Si el nombre de la pantalla ocupa más líneas, la página agranda el alto.
const ANCHO: f64 = 260.0;
const ALTO: f64 = 96.0;
/// Distancia al borde de la pantalla (más los 6 px de la sombra: 24 px a la vista).
const MARGEN: f64 = 18.0;
/// Tiempo quieto antes de guardar la posición.
const ESPERA: Duration = Duration::from_millis(700);

/// Cuándo se creó cada widget (sus primeros "se movió" son el sistema ubicándolo) y cuándo se movió cada uno.
static CREADOS: Mutex<Vec<(u32, Instant)>> = Mutex::new(Vec::new());
static MOVIDOS: Mutex<Vec<(u32, Instant)>> = Mutex::new(Vec::new());
static ESPERANDO: AtomicBool = AtomicBool::new(false);

pub fn etiqueta(id: u32) -> String {
    format!("{PREFIJO}{id}")
}

/// Las pantallas con su widget de brillo encendido.
pub fn activos(app: &AppHandle) -> Vec<u32> {
    ajustes::cargar(app).widgets_brillo.iter().map(|w| w.pantalla_id).collect()
}

/// Al arrancar: abre los encendidos cuya pantalla está conectada.
pub fn iniciar(app: &AppHandle) {
    for id in activos(app) {
        if pantallas::limites(id).is_none() {
            continue;
        }
        if let Err(e) = crear(app, id) {
            eprintln!("No se pudo abrir el widget de brillo: {e}");
        }
    }
}

/// Enciende o apaga el widget de una pantalla, lo guarda y avisa al panel y al menú del ícono. Devuelve cómo quedó.
pub fn usar(app: &AppHandle, id: u32, activar: bool) -> Result<bool, String> {
    let resultado = if activar {
        crear(app, id).map_err(|e| format!("No se pudo abrir el widget de brillo: {e}"))
    } else {
        quitar(app, id).map_err(|e| format!("No se pudo quitar el widget de brillo: {e}"))
    };
    let quedo = activar && resultado.is_ok();
    ajustes::cambiar(app, |a| {
        let esta = a.widgets_brillo.iter().any(|w| w.pantalla_id == id);
        if quedo && !esta {
            a.widgets_brillo.push(WidgetBrillo { pantalla_id: id, posicion: None });
        } else if !quedo {
            a.widgets_brillo.retain(|w| w.pantalla_id != id);
        }
    })?;
    // El panel pone al día su interruptor y el menú del ícono su casilla.
    let _ = app.emit("widgets-brillo", activos(app));
    super::bandeja::actualizar_marcas(app);
    resultado.map(|_| quedo)
}

/// Desde el menú del ícono: lo contrario de lo que hay ahora.
pub fn alternar(app: &AppHandle, id: u32) {
    let abierto = app.get_webview_window(&etiqueta(id)).is_some();
    if let Err(e) = usar(app, id, !abierto) {
        eprintln!("{e}");
    }
}

fn quitar(app: &AppHandle, id: u32) -> Result<(), String> {
    match app.get_webview_window(&etiqueta(id)) {
        Some(v) => v.destroy().map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

fn crear(app: &AppHandle, id: u32) -> Result<(), String> {
    let etiqueta = etiqueta(id);
    if app.get_webview_window(&etiqueta).is_some() {
        return Ok(());
    }
    let lugar = lugar(app, id);

    // Como el widget: "brillo" sirve en desarrollo (ruta de Vite) y en la app final (Tauri entrega brillo.html).
    let pagina = WebviewUrl::App(format!("brillo?id={id}").into());
    let constructor = WebviewWindowBuilder::new(app, &etiqueta, pagina)
        .title("Brillo")
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
        // Un clic en el widget sin foco también llega a la barra o al botón.
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

    marcar(&CREADOS, id);
    let ventana = constructor.build().map_err(|e| e.to_string())?;

    #[cfg(not(target_os = "macos"))]
    {
        if let Some((x, y)) = lugar {
            let p = tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32);
            // Si su pantalla tiene otra escala, Windows la ajusta al llegar: la segunda vez queda en su lugar.
            let _ = ventana.set_position(p);
            let _ = ventana.set_position(p);
        }
        let _ = ventana.show();
    }

    let app2 = app.clone();
    ventana.on_window_event(move |evento| {
        if let WindowEvent::Moved(_) = evento {
            al_mover(&app2, id);
        }
    });
    Ok(())
}

// ---------------------------------------------------------------- Posición

/// El monitor de Tauri de esa pantalla: el que tiene su centro.
fn monitor_de(app: &AppHandle, id: u32) -> Option<Monitor> {
    let (x, y, ancho, alto) = pantallas::limites(id)?;
    let (cx, cy) = (x + ancho / 2.0, y + alto / 2.0);
    app.available_monitors().ok()?.into_iter().find(|m| {
        // Mac: Tauri da píxeles con la escala de esa pantalla y las de Nexo van en puntos. Windows: píxeles los dos.
        let e = if cfg!(target_os = "macos") { m.scale_factor() } else { 1.0 };
        let (mx, my) = (m.position().x as f64 / e, m.position().y as f64 / e);
        let (mw, mh) = (m.size().width as f64 / e, m.size().height as f64 / e);
        cx >= mx && cx < mx + mw && cy >= my && cy < my + mh
    })
}

/// Abajo a la derecha de la zona útil de su pantalla (o de la principal, si no se encuentra).
fn posicion_inicial(app: &AppHandle, id: u32) -> Option<(f64, f64)> {
    let m = monitor_de(app, id).or_else(|| app.primary_monitor().ok().flatten())?;
    let (x, y, ancho, alto) = widget::area_util(&m);
    let p = widget::punto(&m);
    Some((x + ancho - (ANCHO + MARGEN) * p, y + alto - (ALTO + MARGEN) * p))
}

/// La posición guardada si todavía se ve; si no, la de la primera vez.
fn lugar(app: &AppHandle, id: u32) -> Option<(f64, f64)> {
    ajustes::cargar(app)
        .widgets_brillo
        .into_iter()
        .find(|w| w.pantalla_id == id)
        .and_then(|w| w.posicion)
        .map(|(x, y)| (x as f64, y as f64))
        .filter(|&p| widget::se_ve(app, p, (ANCHO, ALTO)))
        .or_else(|| posicion_inicial(app, id))
}

fn marcar(lista: &Mutex<Vec<(u32, Instant)>>, id: u32) {
    if let Ok(mut l) = lista.lock() {
        l.retain(|(x, _)| *x != id);
        l.push((id, Instant::now()));
    }
}

/// Cada movimiento renueva la espera de ese widget; un solo hilo guarda los que quedaron quietos.
fn al_mover(app: &AppHandle, id: u32) {
    let recien = CREADOS
        .lock()
        .ok()
        .and_then(|l| l.iter().find(|(x, _)| *x == id).map(|(_, t)| *t))
        .is_some_and(|t| t.elapsed() < Duration::from_secs(1));
    if recien {
        return;
    }
    marcar(&MOVIDOS, id);
    if ESPERANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let hilo = std::thread::Builder::new().name("nexo-widget-brillo".into()).spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let quietos: Vec<u32> = match MOVIDOS.lock() {
                Ok(mut l) => {
                    let q = l.iter().filter(|(_, t)| t.elapsed() >= ESPERA).map(|(x, _)| *x).collect();
                    l.retain(|(_, t)| t.elapsed() < ESPERA);
                    q
                }
                Err(_) => Vec::new(),
            };
            for id in quietos {
                guardar_posicion(&app, id);
            }
            let vacio = || MOVIDOS.lock().map(|l| l.is_empty()).unwrap_or(true);
            if vacio() {
                ESPERANDO.store(false, Ordering::SeqCst);
                // Si justo llegó otro movimiento, este hilo sigue (el otro no se creó).
                if vacio() || ESPERANDO.swap(true, Ordering::SeqCst) {
                    break;
                }
            }
        }
    });
    if hilo.is_err() {
        ESPERANDO.store(false, Ordering::SeqCst);
    }
}

fn guardar_posicion(app: &AppHandle, id: u32) {
    let Some(v) = app.get_webview_window(&etiqueta(id)) else { return };
    let (Ok(pos), Ok(escala)) = (v.outer_position(), v.scale_factor()) else { return };
    let nueva = widget::a_nativa(pos, escala);
    let guardada = ajustes::cargar(app).widgets_brillo.into_iter().find(|w| w.pantalla_id == id);
    if guardada.as_ref().is_none_or(|w| w.posicion == Some(nueva)) {
        return;
    }
    let r = ajustes::cambiar(app, |a| {
        if let Some(w) = a.widgets_brillo.iter_mut().find(|w| w.pantalla_id == id) {
            w.posicion = Some(nueva);
        }
    });
    if let Err(e) = r {
        eprintln!("No se pudo guardar dónde quedó el widget de brillo: {e}");
    }
}
