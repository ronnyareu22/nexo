//! Menú del ícono (barra de menús en la Mac, bandeja en Windows): accesos rápidos que siempre están a mano.
//!
//! - Orden: captura y XDrop, escenas, sonido, brillo, aparatos, remoto y, al final, abrir, widget y salir.
//!   En el remoto de la Mac va la casilla de la pantalla virtual para RustDesk. Lo del remoto solo aparece con el
//!   perfil de trabajo remoto encendido (`equipos`: se encontró tu Mac o tu PC, o se eligió "Siempre").
//! - Abrirlo no lee nada: se arma con lo que ya se sabe. Los aparatos los trae el vigía de lib.rs (cada 20 s) y
//!   los destinos de XDrop son los de la última búsqueda (al arrancar, cada 2 min o cuando el panel los pide).
//!   Esa búsqueda es la de `equipos`, que además guarda el otro equipo del perfil cuando aparece.
//! - Se pone al día cuando cambia lo que muestra: en cada vuelta del vigía, tras cada acción del menú, al terminar
//!   una escena, al cambiar el brillo con las teclas y cuando cambian los destinos de XDrop. Si solo cambian textos
//!   o marcas, se cambian en su lugar; si cambian los elementos, se rehace esa parte.
//! - Cambiar el menú espera al hilo principal: se hace siempre desde otro hilo. Las acciones también van aparte
//!   y avisan cómo salieron con una notificación.
//! - Sin atajos globales nuevos: en la Mac la captura ya tiene F13.

use super::{ajustes, aviso, drop as xdrop, equipos, escenas, pantalla_virtual, remoto, teclas, widget, widget_brillo};
use crate::dispositivos::audio::{self, Cambio, DispositivoAudio};
use crate::dispositivos::historial::Historial;
use crate::dispositivos::pantallas::{Pantalla, Pantallas};
use crate::dispositivos::{self, Dispositivo, Tipo};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Listener, Manager, Wry};
use tauri_plugin_notification::NotificationExt;

/// Los nombres más largos se cortan con "…", así el menú no crece de ancho.
const LARGO: usize = 32;
const MAX_DESTINOS: usize = 6;
const MAX_APARATOS: usize = 8;
/// Cada cuántas vueltas del vigía (20 s) se buscan los destinos de XDrop: unos 2 min.
const VUELTAS_DESTINOS: u32 = 6;
/// Espera para juntar varios pedidos seguidos en una sola puesta al día.
const JUNTAR: Duration = Duration::from_millis(300);

/// El menú del ícono.
struct MenuBandeja(Menu<Wry>);

/// Un elemento del menú tal como debe verse.
#[derive(Clone, Debug, PartialEq)]
enum Pieza {
    Item { id: String, texto: String, activo: bool, atajo: Option<&'static str> },
    Casilla { id: String, texto: String, marcada: bool },
    Sub { id: String, texto: String, hijos: Vec<Pieza> },
    Raya,
    /// El submenú de escenas: lo llena escenas.rs.
    Escenas,
    /// La casilla del widget: la marca widget.rs.
    Widget,
}

/// Lo último que se puso en el menú. Mientras alguien lo cambia, los demás esperan.
static PUESTO: Mutex<Option<Vec<Pieza>>> = Mutex::new(None);
/// Bluetooth encendido y los aparatos (con sus alias), como los dejó el vigía.
static APARATOS: Mutex<Option<(bool, Vec<Dispositivo>)>> = Mutex::new(None);
/// Aparatos que se están conectando (true) o desconectando desde el menú.
static OCUPADOS: Mutex<Vec<(String, bool)>> = Mutex::new(Vec::new());
static PENDIENTE: AtomicBool = AtomicBool::new(false);
static FORZAR: AtomicBool = AtomicBool::new(false);
static BUSCANDO: AtomicBool = AtomicBool::new(false);
static VUELTAS: AtomicU32 = AtomicU32::new(0);

fn bloquear<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Baja la marca al terminar, pase lo que pase.
struct Suelta(&'static AtomicBool);

impl Drop for Suelta {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------- Arranque y puesta al día

/// Arma el ícono y su menú (en el arranque, en el hilo principal). Lo que tarda en leerse llega enseguida desde otro hilo.
pub fn crear(app: &AppHandle) -> tauri::Result<()> {
    let casilla = CheckMenuItem::with_id(app, "widget", "Widget en el escritorio", true, widget::activo(app), None::<&str>)?;
    app.manage(widget::CasillaBandeja(casilla));
    // Mi setup: las escenas a un clic (el submenú se rehace solo cuando cambian).
    escenas::submenu(app)?;

    let menu = Menu::new(app)?;
    let inicial = armar(&Datos::vacios());
    for p in &inicial {
        menu.append(&crear_pieza(app, p)?)?;
    }
    // Sin esperar: este es el hilo principal. Si no se pudo anotar, la primera puesta al día lo rehace entero.
    if let Ok(mut puesto) = PUESTO.try_lock() {
        *puesto = Some(inicial);
    }
    app.manage(MenuBandeja(menu.clone()));

    TrayIconBuilder::with_id("nexo")
        .icon(Image::from_bytes(include_bytes!("../../icons/bandeja.png"))?)
        .icon_as_template(true)
        .tooltip("Nexo")
        .menu(&menu)
        // Mac: el clic normal muestra el menú de accesos rápidos, como los demás íconos de la barra de menús.
        // Windows: el clic normal abre Nexo y el clic derecho muestra el menú, como en la bandeja de Windows.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, evento| elegir(app, evento.id.as_ref()))
        .on_tray_icon_event(|icono, evento| {
            if cfg!(target_os = "macos") {
                return;
            }
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = evento {
                crate::abrir_panel(icono.app_handle(), None);
            }
        })
        .build(app)?;

    escuchar_escenas(app);
    actualizar(app);
    Ok(())
}

/// Pide poner el menú al día. Vuelve enseguida: lo hace otro hilo y varios pedidos seguidos se juntan en uno.
pub fn actualizar(app: &AppHandle) {
    if PENDIENTE.swap(true, Ordering::SeqCst) {
        return;
    }
    let a = app.clone();
    let hilo = std::thread::Builder::new().name("nexo-bandeja".into()).spawn(move || {
        std::thread::sleep(JUNTAR);
        PENDIENTE.store(false, Ordering::SeqCst);
        refrescar(&a);
    });
    if hilo.is_err() {
        PENDIENTE.store(false, Ordering::SeqCst);
    }
}

/// Igual, pero vuelve a poner todas las marcas: al hacer clic en una casilla, el sistema ya la cambió solo.
pub(super) fn actualizar_marcas(app: &AppHandle) {
    FORZAR.store(true, Ordering::SeqCst);
    actualizar(app);
}

/// Cada vuelta del vigía (en su hilo): los aparatos que ya leyó, si pudo. Pone el menú al día si cambió algo
/// y, cada 2 min, busca los destinos de XDrop en otro hilo.
pub fn al_vigilar(app: &AppHandle, aparatos: Option<(bool, Vec<Dispositivo>)>) {
    if let Some(a) = aparatos {
        *bloquear(&APARATOS) = Some(a);
    }
    if VUELTAS.fetch_add(1, Ordering::Relaxed).is_multiple_of(VUELTAS_DESTINOS) {
        buscar_destinos(app);
    }
    refrescar(app);
}

/// Busca los destinos de XDrop (y el otro equipo del perfil), una búsqueda a la vez. Si cambian, se pide poner el
/// menú al día.
fn buscar_destinos(app: &AppHandle) {
    if BUSCANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    let a = app.clone();
    let hilo = std::thread::Builder::new().name("nexo-bandeja-xdrop".into()).spawn(move || {
        let _suelta = Suelta(&BUSCANDO);
        equipos::revisar(&a);
    });
    if hilo.is_err() {
        BUSCANDO.store(false, Ordering::SeqCst);
    }
}

/// Al terminar una escena (desde el menú o el panel) pudo cambiar el sonido, el brillo o los aparatos.
fn escuchar_escenas(app: &AppHandle) {
    let a = app.clone();
    app.listen("escena-activa", move |evento| {
        let bluetooth = serde_json::from_str::<serde_json::Value>(evento.payload())
            .ok()
            .and_then(|v| v.get("resultados")?.as_array().cloned())
            .is_some_and(|r| r.iter().any(|x| matches!(x["accion"]["tipo"].as_str(), Some("conectar" | "desconectar"))));
        if bluetooth {
            let b = a.clone();
            tauri::async_runtime::spawn(async move { releer_aparatos(&b).await });
        } else {
            actualizar(&a);
        }
    });
}

/// Pone el menú al día con lo de ahora. Nunca en el hilo principal: cambiar el menú espera a ese hilo.
fn refrescar(app: &AppHandle) {
    let Some(menu) = app.try_state::<MenuBandeja>().map(|m| m.0.clone()) else { return };
    let mut puesto = bloquear(&PUESTO);
    let forzar = FORZAR.swap(false, Ordering::SeqCst);
    let nuevas = armar(&Datos::leer(app));
    if !forzar && puesto.as_deref() == Some(nuevas.as_slice()) {
        return;
    }
    match poner(app, Caja::Menu(&menu), puesto.as_deref(), &nuevas, forzar) {
        Ok(()) => *puesto = Some(nuevas),
        Err(e) => {
            eprintln!("No se pudo poner al día el menú del ícono: {e}");
            // La próxima vez se rehace entero.
            *puesto = None;
        }
    }
}

// ---------------------------------------------------------------- Qué se muestra

/// Lo que hace falta para armar el menú.
struct Datos {
    mac: bool,
    windows: bool,
    /// F13 hace la captura (está encendido en Remoto y hay permiso de Accesibilidad).
    f13: bool,
    /// Windows: los atajos de brillo, si están encendidos ("Ctrl+Alt+F1/F2"; se dice junto a "Brillo").
    atajo_brillo: Option<String>,
    /// Destinos de XDrop con Nexo en línea: (dirección, nombre).
    destinos: Vec<(String, String)>,
    sonido: Vec<DispositivoAudio>,
    /// La pantalla del brillo.
    pantalla: Option<Pantalla>,
    /// Las pantallas que dejan cambiar el brillo, para sus widgets: (id, nombre, widget encendido).
    widgets: Vec<(u32, String, bool)>,
    aparatos: Option<(bool, Vec<Dispositivo>)>,
    ocupados: Vec<(String, bool)>,
    /// Mac: la pantalla virtual para RustDesk existe (`None` si este macOS no deja crearla).
    pantalla_virtual: Option<bool>,
    /// El perfil de trabajo remoto está encendido: "Preparar para el trabajo", la pantalla virtual o "Conectar a mi Mac".
    remoto: bool,
}

impl Datos {
    /// Para el primer menú, que se arma sin esperar nada.
    fn vacios() -> Self {
        Datos {
            mac: cfg!(target_os = "macos"),
            windows: cfg!(windows),
            f13: false,
            atajo_brillo: None,
            destinos: Vec::new(),
            sonido: Vec::new(),
            pantalla: None,
            widgets: Vec::new(),
            aparatos: None,
            ocupados: Vec::new(),
            pantalla_virtual: None,
            remoto: false,
        }
    }

    /// Todo lo que se lee es rápido (sin abrir programas): el sonido, el brillo y lo guardado.
    fn leer(app: &AppHandle) -> Self {
        let lista = app.try_state::<Arc<Pantallas>>().map(|p| p.listar()).unwrap_or_default();
        let guardados = ajustes::cargar(app);
        let encendidos: Vec<u32> = guardados.widgets_brillo.iter().map(|w| w.pantalla_id).collect();
        let widgets = lista
            .iter()
            .filter(|p| p.controlable)
            .map(|p| (p.id, p.nombre.clone(), encendidos.contains(&p.id)))
            .collect();
        let pantalla = elegir_pantalla(lista);
        Datos {
            f13: cfg!(target_os = "macos") && guardados.remoto.captura && teclas::permiso_concedido(),
            atajo_brillo: (cfg!(windows) && guardados.teclas_brillo).then(|| {
                let bajar = guardados.atajo_bajar.valido_o(teclas::Atajo::BAJAR);
                teclas::Atajo::texto_par(bajar, guardados.atajo_subir.valido_o(teclas::Atajo::SUBIR))
            }),
            destinos: xdrop::destinos_en_linea(),
            sonido: audio::listar(),
            pantalla,
            widgets,
            aparatos: bloquear(&APARATOS).clone(),
            ocupados: bloquear(&OCUPADOS).clone(),
            pantalla_virtual: pantalla_virtual::en_menu(),
            remoto: equipos::perfil_activo(&guardados),
            ..Datos::vacios()
        }
    }
}

/// La pantalla del brillo: la principal si deja cambiarlo; si no, la primera que deje.
fn elegir_pantalla(lista: Vec<Pantalla>) -> Option<Pantalla> {
    lista.into_iter().filter(|p| p.controlable).min_by_key(|p| !p.principal)
}

fn item(id: impl Into<String>, texto: impl Into<String>) -> Pieza {
    Pieza::Item { id: id.into(), texto: texto.into(), activo: true, atajo: None }
}

fn apagado(id: impl Into<String>, texto: impl Into<String>) -> Pieza {
    Pieza::Item { id: id.into(), texto: texto.into(), activo: false, atajo: None }
}

/// Un nombre para el menú: corto y con el "&" a salvo (el menú lo toma como tecla de acceso).
fn corto(texto: &str) -> String {
    let limpio = texto.trim();
    let mut s: String = limpio.chars().take(LARGO).collect();
    if limpio.chars().count() > LARGO {
        s = s.chars().take(LARGO - 1).collect::<String>().trim_end().to_string();
        s.push('…');
    }
    s.replace('&', "&&")
}

/// El menú entero, en orden.
fn armar(d: &Datos) -> Vec<Pieza> {
    let mut m = Vec::new();

    // Captura y XDrop. La captura de zona es de la Mac.
    if d.mac {
        m.push(Pieza::Item {
            id: "captura".into(),
            texto: "Tomar captura de una zona".into(),
            activo: true,
            atajo: d.f13.then_some("F13"),
        });
        m.extend(enviar_a("xdrop-captura", "Enviar captura a", &d.destinos));
    }
    m.extend(enviar_a("xdrop-copiado", "Enviar lo que copiaste a", &d.destinos));
    if d.mac {
        m.push(item("carpeta-capturas", "Abrir carpeta de capturas"));
    }
    m.push(item("carpeta-xdrop", "Abrir carpeta XDrop"));

    m.push(Pieza::Raya);
    m.push(Pieza::Escenas);

    let sonido = sonido(&d.sonido);
    if !sonido.is_empty() {
        m.push(Pieza::Raya);
        m.extend(sonido);
    }

    if let Some(p) = &d.pantalla {
        m.push(Pieza::Raya);
        let atajo = d.atajo_brillo.as_deref().filter(|_| d.windows);
        m.push(brillo(p, atajo, &d.widgets));
    }

    let aparatos = aparatos(d);
    if !aparatos.is_empty() {
        m.push(Pieza::Raya);
        m.extend(aparatos);
    }

    if d.mac && d.remoto {
        m.push(Pieza::Raya);
        m.push(item("preparar", "Preparar para el trabajo"));
        if let Some(marcada) = d.pantalla_virtual {
            m.push(Pieza::Casilla { id: "pantalla-virtual".into(), texto: "Pantalla virtual para RustDesk".into(), marcada });
        }
    } else if d.windows && d.remoto {
        m.push(Pieza::Raya);
        m.push(item("conectar-mac", "Conectar a mi Mac"));
    }

    m.push(Pieza::Raya);
    m.push(item("abrir", "Abrir Nexo"));
    m.push(Pieza::Widget);
    m.push(item("salir", "Salir de Nexo"));
    m
}

/// "Enviar … a innotech1017" con un destino; con varios, un submenú con sus nombres. Sin destinos, nada.
fn enviar_a(id: &str, texto: &str, destinos: &[(String, String)]) -> Option<Pieza> {
    match destinos {
        [] => None,
        [(ip, nombre)] => Some(item(format!("{id}:{ip}"), format!("{texto} {}", corto(nombre)))),
        varios => Some(Pieza::Sub {
            id: id.into(),
            texto: texto.into(),
            hijos: varios.iter().take(MAX_DESTINOS).map(|(ip, nombre)| item(format!("{id}:{ip}"), corto(nombre))).collect(),
        }),
    }
}

fn en_uso(d: &DispositivoAudio, microfono: bool) -> bool {
    if microfono { d.entrada_en_uso } else { d.salida_en_uso }
}

/// "Salida: …" y "Micrófono: …" (submenús con casillas) y "Silenciar" o "Quitar silencio".
fn sonido(lista: &[DispositivoAudio]) -> Vec<Pieza> {
    let mut v = Vec::new();
    for microfono in [false, true] {
        let mut opciones: Vec<&DispositivoAudio> =
            lista.iter().filter(|d| if microfono { d.entrada } else { d.salida }).collect();
        if opciones.is_empty() {
            continue;
        }
        // Orden fijo (no "el que está en uso primero"): así el submenú no se reacomoda al cambiar.
        opciones.sort_by_cached_key(|d| (d.transporte != "integrado", d.nombre.to_lowercase()));
        let (id, rotulo) = if microfono { ("microfono", "Micrófono") } else { ("salida", "Salida") };
        let texto = match opciones.iter().find(|d| en_uso(d, microfono)) {
            Some(d) => format!("{rotulo}: {}", corto(&d.nombre)),
            None => rotulo.to_string(),
        };
        let hijos = opciones
            .iter()
            .map(|d| Pieza::Casilla { id: format!("{id}:{}", d.uid), texto: corto(&d.nombre), marcada: en_uso(d, microfono) })
            .collect();
        v.push(Pieza::Sub { id: id.into(), texto, hijos });
    }
    // Solo si la salida en uso deja silenciarla.
    if let Some(silenciada) = lista.iter().find(|d| d.salida_en_uso).and_then(|d| d.silenciado) {
        v.push(item("silencio", if silenciada { "Quitar silencio" } else { "Silenciar" }));
    }
    v
}

/// "Brillo: 40 %" con los niveles fijos y los pasos de 10. En Windows, con el atajo si está encendido.
/// Al final, una casilla "Widget de brillo: …" por pantalla que deja cambiarlo.
fn brillo(p: &Pantalla, atajo: Option<&str>, widgets: &[(u32, String, bool)]) -> Pieza {
    let nivel = p.brillo;
    let mut hijos: Vec<Pieza> = [(0, "Mínimo"), (25, "25 %"), (50, "50 %"), (75, "75 %"), (100, "Máximo")]
        .into_iter()
        .map(|(valor, texto)| item(format!("brillo:{valor}"), texto))
        .collect();
    hijos.push(Pieza::Raya);
    hijos.push(Pieza::Item { id: "brillo-bajar".into(), texto: "Bajar 10 %".into(), activo: nivel != Some(0), atajo: None });
    hijos.push(Pieza::Item { id: "brillo-subir".into(), texto: "Subir 10 %".into(), activo: nivel != Some(100), atajo: None });
    if !widgets.is_empty() {
        hijos.push(Pieza::Raya);
        hijos.extend(widgets.iter().map(|(id, nombre, marcada)| Pieza::Casilla {
            id: format!("widget-brillo:{id}"),
            texto: format!("Widget de brillo: {}", corto(nombre)),
            marcada: *marcada,
        }));
    }
    let mut texto = nivel.map_or_else(|| "Brillo".to_string(), |n| format!("Brillo: {n} %"));
    if let Some(a) = atajo {
        texto.push_str(" · ");
        texto.push_str(a);
    }
    Pieza::Sub { id: "brillo".into(), texto, hijos }
}

/// El nombre con que se muestra un aparato: su alias en Nexo o el suyo.
fn nombre_de(x: &Dispositivo) -> &str {
    x.alias.as_deref().map(str::trim).filter(|a| !a.is_empty()).unwrap_or(&x.nombre)
}

/// Un "Conectar …" o "Desconectar …" por aparato de audio emparejado. El id lleva lo que se pidió, no lo que
/// se lee al hacer clic: si el aparato cambió solo entre medio, no se hace lo contrario.
fn aparatos(d: &Datos) -> Vec<Pieza> {
    let Some((encendido, lista)) = &d.aparatos else { return Vec::new() };
    let mut de_sonido: Vec<&Dispositivo> =
        lista.iter().filter(|x| matches!(x.tipo, Tipo::Audifonos | Tipo::Diadema | Tipo::Parlante)).collect();
    if de_sonido.is_empty() {
        return Vec::new();
    }
    if !encendido {
        return vec![apagado("bluetooth-apagado", "El Bluetooth está apagado")];
    }
    de_sonido.sort_by_cached_key(|x| nombre_de(x).to_lowercase());
    de_sonido
        .into_iter()
        .take(MAX_APARATOS)
        .map(|x| {
            let nombre = corto(nombre_de(x));
            match d.ocupados.iter().find(|(id, _)| *id == x.id) {
                Some((_, true)) => apagado(format!("ocupado:{}", x.id), format!("Conectando {nombre}…")),
                Some((_, false)) => apagado(format!("ocupado:{}", x.id), format!("Desconectando {nombre}…")),
                None if x.conectado => item(format!("bt:0:{}", x.id), format!("Desconectar {nombre}")),
                None => item(format!("bt:1:{}", x.id), format!("Conectar {nombre}")),
            }
        })
        .collect()
}

// ---------------------------------------------------------------- Del dibujo al menú nativo

/// El menú o un submenú: los dos se llenan igual.
enum Caja<'a> {
    Menu(&'a Menu<Wry>),
    Sub(&'a Submenu<Wry>),
}

impl Caja<'_> {
    fn items(&self) -> tauri::Result<Vec<MenuItemKind<Wry>>> {
        match self {
            Caja::Menu(m) => m.items(),
            Caja::Sub(s) => s.items(),
        }
    }

    fn quitar(&self, i: &MenuItemKind<Wry>) -> tauri::Result<()> {
        match self {
            Caja::Menu(m) => m.remove(i),
            Caja::Sub(s) => s.remove(i),
        }
    }

    fn agregar(&self, i: &MenuItemKind<Wry>) -> tauri::Result<()> {
        match self {
            Caja::Menu(m) => m.append(i),
            Caja::Sub(s) => s.append(i),
        }
    }
}

/// Lo que no se puede cambiar en su lugar: el tipo y el id.
fn llave(p: &Pieza) -> (u8, &str) {
    match p {
        Pieza::Item { id, .. } => (0, id),
        Pieza::Casilla { id, .. } => (1, id),
        Pieza::Sub { id, .. } => (2, id),
        Pieza::Raya => (3, ""),
        Pieza::Escenas => (4, ""),
        Pieza::Widget => (5, ""),
    }
}

fn misma_forma(a: &[Pieza], b: &[Pieza]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| llave(x) == llave(y))
}

fn crear_pieza(app: &AppHandle, p: &Pieza) -> tauri::Result<MenuItemKind<Wry>> {
    Ok(match p {
        Pieza::Item { id, texto, activo, atajo } => {
            MenuItemKind::MenuItem(MenuItem::with_id(app, id.as_str(), texto, *activo, *atajo)?)
        }
        Pieza::Casilla { id, texto, marcada } => {
            MenuItemKind::Check(CheckMenuItem::with_id(app, id.as_str(), texto, true, *marcada, None::<&str>)?)
        }
        Pieza::Sub { id, texto, hijos } => {
            let s = Submenu::with_id(app, id.as_str(), texto, true)?;
            for h in hijos {
                s.append(&crear_pieza(app, h)?)?;
            }
            MenuItemKind::Submenu(s)
        }
        Pieza::Raya => MenuItemKind::Predefined(PredefinedMenuItem::separator(app)?),
        Pieza::Escenas => match app.try_state::<escenas::MenuEscenas>() {
            Some(m) => MenuItemKind::Submenu(m.0.clone()),
            None => MenuItemKind::Submenu(Submenu::with_id(app, "escenas", "Escenas", false)?),
        },
        Pieza::Widget => match app.try_state::<widget::CasillaBandeja>() {
            Some(c) => MenuItemKind::Check(c.0.clone()),
            None => MenuItemKind::Check(CheckMenuItem::with_id(app, "widget", "Widget en el escritorio", false, false, None::<&str>)?),
        },
    })
}

/// Deja la caja como `nuevas`. Si los elementos son los mismos (tipo e id), solo cambia lo que cambió;
/// si no, la vacía y la vuelve a llenar. Con `forzar`, vuelve a poner todas las marcas.
fn poner(app: &AppHandle, caja: Caja, antes: Option<&[Pieza]>, nuevas: &[Pieza], forzar: bool) -> tauri::Result<()> {
    let vivos = caja.items()?;
    if let Some(antes) = antes.filter(|a| misma_forma(a, nuevas) && vivos.len() == nuevas.len()) {
        for ((a, n), vivo) in antes.iter().zip(nuevas).zip(&vivos) {
            if forzar || a != n {
                cambiar(app, a, n, vivo, forzar)?;
            }
        }
        return Ok(());
    }
    for vivo in &vivos {
        caja.quitar(vivo)?;
    }
    for n in nuevas {
        caja.agregar(&crear_pieza(app, n)?)?;
    }
    Ok(())
}

/// Cambia en su lugar un elemento de la misma forma.
fn cambiar(app: &AppHandle, antes: &Pieza, nueva: &Pieza, vivo: &MenuItemKind<Wry>, forzar: bool) -> tauri::Result<()> {
    match (antes, nueva, vivo) {
        (
            Pieza::Item { texto: t0, activo: a0, atajo: k0, .. },
            Pieza::Item { texto, activo, atajo, .. },
            MenuItemKind::MenuItem(i),
        ) => {
            if t0 != texto {
                i.set_text(texto)?;
            }
            if a0 != activo {
                i.set_enabled(*activo)?;
            }
            if k0 != atajo {
                i.set_accelerator(*atajo)?;
            }
        }
        (Pieza::Casilla { texto: t0, marcada: m0, .. }, Pieza::Casilla { texto, marcada, .. }, MenuItemKind::Check(c)) => {
            if t0 != texto {
                c.set_text(texto)?;
            }
            if forzar || m0 != marcada {
                c.set_checked(*marcada)?;
            }
        }
        (Pieza::Sub { texto: t0, hijos: h0, .. }, Pieza::Sub { texto, hijos, .. }, MenuItemKind::Submenu(s)) => {
            if t0 != texto {
                s.set_text(texto)?;
            }
            poner(app, Caja::Sub(s), Some(h0), hijos, forzar)?;
        }
        _ => {}
    }
    Ok(())
}

// ---------------------------------------------------------------- Acciones

/// Se eligió algo del menú (llega en el hilo principal): lo que tarda va aparte.
fn elegir(app: &AppHandle, id: &str) {
    match id {
        "abrir" => crate::abrir_panel(app, None),
        "widget" => widget::alternar(app),
        _ if id.starts_with("widget-brillo:") => {
            if let Some(pantalla) = id.strip_prefix("widget-brillo:").and_then(|n| n.parse::<u32>().ok()) {
                widget_brillo::alternar(app, pantalla);
            }
        }
        "salir" => app.exit(0),
        _ if id.starts_with("escena:") => escenas::desde_bandeja(app, id),
        _ => {
            tauri::async_runtime::spawn(accion(app.clone(), id.to_string()));
        }
    }
}

async fn accion(app: AppHandle, id: String) {
    let (clave, resto) = id.split_once(':').unwrap_or((id.as_str(), ""));
    match clave {
        "captura" => captura(&app).await,
        "xdrop-captura" => enviar_captura(&app, resto).await,
        "xdrop-copiado" => enviar_copiado(&app, resto).await,
        "carpeta-capturas" => {
            if let Err(e) = remoto::remoto_abrir_capturas().await {
                avisar(&app, "No se pudo abrir la carpeta", &e);
            }
        }
        "carpeta-xdrop" => {
            if let Err(e) = xdrop::drop_abrir_carpeta(app.clone()).await {
                avisar(&app, "No se pudo abrir la carpeta", &e);
            }
        }
        "salida" => usar_sonido(&app, resto, false).await,
        "microfono" => usar_sonido(&app, resto, true).await,
        "silencio" => silenciar(&app).await,
        "brillo" => {
            if let Ok(valor) = resto.parse::<u8>() {
                poner_brillo(&app, Brillo::Fijo(valor)).await;
            }
        }
        "brillo-bajar" => poner_brillo(&app, Brillo::Bajar).await,
        "brillo-subir" => poner_brillo(&app, Brillo::Subir).await,
        "bt" => aparato(&app, resto).await,
        "preparar" => preparar(&app).await,
        "pantalla-virtual" => usar_pantalla_virtual(&app).await,
        "conectar-mac" => {
            if let Err(e) = remoto::remoto_conectar_mac(app.clone()).await {
                avisar(&app, "No se pudo conectar a tu Mac", &e);
            }
        }
        _ => {}
    }
}

fn avisar(app: &AppHandle, titulo: &str, cuerpo: &str) {
    let _ = app.notification().builder().title(titulo).body(cuerpo).show();
}

async fn en_hilo<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tauri::async_runtime::spawn_blocking(f).await.ok()
}

/// La captura liviana del remoto: eliges una zona, queda en 1x en el portapapeles y en Capturas remotas.
#[cfg(target_os = "macos")]
async fn captura(app: &AppHandle) {
    let a = app.clone();
    let Some(r) = en_hilo(move || remoto::captura::capturar(&a)).await else { return };
    match r {
        Ok(None) => {} // se canceló con Esc
        Ok(Some(c)) => {
            let medida = format!("{} × {}", c.ancho, c.alto);
            let (titulo, cuerpo) = match (c.copiada, c.archivo.is_some()) {
                (true, true) => (format!("Captura copiada · {medida}"), "También quedó en Imágenes › Capturas remotas."),
                (true, false) => (format!("Captura copiada · {medida}"), "No pude guardar la copia en Imágenes."),
                (false, true) => (format!("Captura guardada · {medida}"), "No pude copiarla. Está en Imágenes › Capturas remotas."),
                (false, false) => ("No se pudo guardar la captura".into(), "Inténtalo otra vez."),
            };
            avisar(app, &titulo, cuerpo);
        }
        Err(e) => avisar(app, "No se pudo capturar", &e),
    }
}

/// En Windows no hay captura de zona (el menú no la muestra).
#[cfg(not(target_os = "macos"))]
async fn captura(_app: &AppHandle) {}

fn nombre_destino(ip: &str) -> String {
    xdrop::destinos_en_linea().into_iter().find(|(d, _)| d == ip).map(|(_, n)| n).unwrap_or_else(|| ip.to_string())
}

fn no_se_pudo_enviar(app: &AppHandle, e: &str) {
    avisar(app, "No se pudo enviar", e);
    // Quizá ese equipo ya no está: se vuelve a mirar.
    buscar_destinos(app);
}

async fn enviar_captura(app: &AppHandle, ip: &str) {
    let equipo = nombre_destino(ip);
    match xdrop::drop_enviar_captura(app.clone(), ip.to_string()).await {
        Ok(None) => {} // se canceló con Esc
        Ok(Some(envio)) => esperar_envio(app, envio.lote, format!("Captura enviada a {equipo}")),
        Err(e) => no_se_pudo_enviar(app, &e),
    }
}

async fn enviar_copiado(app: &AppHandle, ip: &str) {
    let equipo = nombre_destino(ip);
    match xdrop::drop_enviar_portapapeles(app.clone(), ip.to_string()).await {
        Ok(envio) => {
            let listo = match (envio.tipo, envio.cantidad) {
                ("texto", _) => format!("Texto enviado a {equipo}"),
                ("imagen", _) => format!("Imagen enviada a {equipo}"),
                (_, 1) => format!("Enviado a {equipo}"),
                (_, n) => format!("{n} archivos enviados a {equipo}"),
            };
            match envio.lote {
                Some(lote) => esperar_envio(app, lote, listo),
                None => avisar(app, xdrop::NOMBRE, &listo),
            }
        }
        Err(e) => no_se_pudo_enviar(app, &e),
    }
}

/// Espera en otro hilo a que termine un envío de XDrop y avisa cómo salió (si se canceló, no avisa).
fn esperar_envio(app: &AppHandle, lote: String, listo: String) {
    let a = app.clone();
    let _ = std::thread::Builder::new().name("nexo-bandeja-envio".into()).spawn(move || {
        // Se mira dos veces por segundo, hasta 2 horas (un envío muy grande).
        for _ in 0..(2 * 60 * 60 * 2) {
            std::thread::sleep(Duration::from_millis(500));
            match xdrop::como_termino(&lote) {
                None => continue,
                Some(xdrop::FinEnvio::Listo) => avisar(&a, xdrop::NOMBRE, &listo),
                Some(xdrop::FinEnvio::Cancelado) => {}
                Some(xdrop::FinEnvio::Fallo(e)) => no_se_pudo_enviar(&a, &e),
            }
            return;
        }
    });
}

async fn usar_sonido(app: &AppHandle, uid: &str, microfono: bool) {
    let uid = uid.to_string();
    let cambio = if microfono { Cambio::UsarMicrofono } else { Cambio::UsarSalida };
    if let Some(Err(e)) = en_hilo(move || audio::ajustar_por_uid(&uid, cambio)).await {
        avisar(app, "No se pudo cambiar el sonido", &e);
    }
    actualizar_marcas(app);
}

async fn silenciar(app: &AppHandle) {
    let r = en_hilo(|| {
        let lista = audio::listar();
        let actual = lista.iter().find(|d| d.salida_en_uso).ok_or("No hay una salida de sonido en uso.")?;
        let silenciada = actual.silenciado.ok_or("Esta salida no deja silenciarla.")?;
        audio::ajustar_por_uid(&actual.uid, Cambio::Silenciar(!silenciada))
    })
    .await;
    if let Some(Err(e)) = r {
        avisar(app, "No se pudo cambiar el silencio", &e);
    }
    actualizar(app);
}

enum Brillo {
    Fijo(u8),
    Bajar,
    Subir,
}

/// El brillo de la pantalla elegida. Los pasos van al múltiplo de 10 anterior o siguiente (como el widget).
async fn poner_brillo(app: &AppHandle, como: Brillo) {
    let a = app.clone();
    let r = en_hilo(move || {
        let p = a.try_state::<Arc<Pantallas>>().map(|p| p.inner().clone()).ok_or("Las pantallas todavía no están listas.")?;
        let pantalla = elegir_pantalla(p.listar()).ok_or("Ninguna pantalla deja cambiar el brillo.")?;
        let actual = pantalla.brillo.unwrap_or(50).min(100);
        let valor = match como {
            Brillo::Fijo(v) => v.min(100),
            Brillo::Bajar => actual.saturating_sub(1) / 10 * 10,
            Brillo::Subir => ((actual / 10 + 1) * 10).min(100),
        };
        let nivel = p.poner_brillo(pantalla.id, valor)?;
        // El mismo aviso que con las teclas, para ver el cambio en esa pantalla.
        aviso::mostrar(&a, pantalla.id, nivel);
        Ok::<(), String>(())
    })
    .await;
    if let Some(Err(e)) = r {
        avisar(app, "No se pudo cambiar el brillo", &e);
    }
    actualizar(app);
}

/// "bt:1:<id>" conecta y "bt:0:<id>" desconecta. Mientras tanto, el menú dice "Conectando …".
async fn aparato(app: &AppHandle, resto: &str) {
    let Some((pedido, id)) = resto.split_once(':') else { return };
    let (conectar, id) = (pedido == "1", id.to_string());
    let nombre = bloquear(&APARATOS)
        .as_ref()
        .and_then(|(_, l)| l.iter().find(|x| x.id == id).map(|x| nombre_de(x).to_string()));
    {
        let mut ocupados = bloquear(&OCUPADOS);
        if ocupados.iter().any(|(x, _)| *x == id) {
            return;
        }
        ocupados.push((id.clone(), conectar));
    }
    actualizar(app);
    let r = dispositivos::cambiar_conexion(id.clone(), conectar).await;
    bloquear(&OCUPADOS).retain(|(x, _)| *x != id);
    match r {
        Ok(()) => {
            let titulo = nombre.as_deref().unwrap_or("Bluetooth");
            avisar(app, titulo, if conectar { "Conectado." } else { "Desconectado." });
        }
        Err(e) => {
            let verbo = if conectar { "conectar" } else { "desconectar" };
            let nombre = nombre.as_deref().unwrap_or("el aparato");
            avisar(app, &format!("No se pudo {verbo} {nombre}"), &e);
        }
    }
    releer_aparatos(app).await;
}

/// Vuelve a leer el Bluetooth (tras conectar o desconectar) sin esperar la próxima vuelta del vigía.
async fn releer_aparatos(app: &AppHandle) {
    let a = app.clone();
    let leido = en_hilo(move || {
        let mut e = dispositivos::listar_ahora().ok()?;
        if let Some(h) = a.try_state::<Arc<Historial>>() {
            h.poner_alias_en(&mut e.dispositivos);
        }
        Some((e.bluetooth_encendido, e.dispositivos))
    })
    .await
    .flatten();
    if let Some(l) = leido {
        *bloquear(&APARATOS) = Some(l);
    }
    actualizar(app);
}

/// "Preparar para el trabajo": Tailscale, RustDesk y el modo trabajo (la Mac no se duerme).
async fn preparar(app: &AppHandle) {
    match remoto::remoto_preparar(app.clone()).await {
        Ok(e) if e.tailscale == "activo" => {
            avisar(app, "Listo para el trabajo", "Tailscale conectado, RustDesk abierto y la Mac no se dormirá.")
        }
        Ok(_) => avisar(app, "Casi listo", "RustDesk está abierto y la Mac no se dormirá, pero Tailscale no se conectó."),
        Err(e) => avisar(app, "No se pudo preparar", &e),
    }
}

/// "Pantalla virtual para RustDesk": la crea o la quita. La marca vuelve a lo real (el clic ya la cambió).
async fn usar_pantalla_virtual(app: &AppHandle) {
    let a = app.clone();
    if let Some(Err(e)) = en_hilo(move || pantalla_virtual::alternar(&a)).await {
        avisar(app, "No se pudo cambiar la pantalla virtual", &e);
    }
    actualizar_marcas(app);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn sonido_de(uid: &str, nombre: &str, salida: bool, en_uso: bool, silenciado: Option<bool>) -> DispositivoAudio {
        DispositivoAudio {
            uid: uid.into(),
            nombre: nombre.into(),
            salida,
            entrada: !salida,
            salida_en_uso: salida && en_uso,
            entrada_en_uso: !salida && en_uso,
            volumen: None,
            silenciado: if salida { silenciado } else { None },
            volumen_entrada: None,
            silenciado_entrada: None,
            transporte: if uid.starts_with("BuiltIn") { "integrado" } else { "bluetooth" }.into(),
        }
    }

    fn aparato_de(id: &str, nombre: &str, tipo: Tipo, conectado: bool, alias: Option<&str>) -> Dispositivo {
        Dispositivo {
            id: id.into(),
            nombre: nombre.into(),
            tipo,
            conectado,
            bateria: None,
            bateria_izquierda: None,
            bateria_derecha: None,
            bateria_estuche: None,
            fabricante: None,
            firmware: None,
            vendor_id: None,
            product_id: None,
            alias: alias.map(str::to_string),
        }
    }

    fn ids(m: &[Pieza]) -> Vec<String> {
        m.iter()
            .map(|p| match p {
                Pieza::Item { id, .. } | Pieza::Casilla { id, .. } | Pieza::Sub { id, .. } => id.clone(),
                Pieza::Raya => "-".into(),
                Pieza::Escenas => "escenas".into(),
                Pieza::Widget => "widget".into(),
            })
            .collect()
    }

    fn texto_de<'a>(m: &'a [Pieza], buscado: &str) -> Option<&'a str> {
        m.iter().find_map(|p| match p {
            Pieza::Item { id, texto, .. } | Pieza::Casilla { id, texto, .. } | Pieza::Sub { id, texto, .. } if id == buscado => {
                Some(texto.as_str())
            }
            _ => None,
        })
    }

    fn mac_completa() -> Datos {
        Datos {
            mac: true,
            windows: false,
            f13: true,
            atajo_brillo: None,
            destinos: vec![("100.64.0.2".into(), "innotech1017".into())],
            sonido: vec![
                sonido_de("BuiltInSpeakerDevice", "Bocinas de MacBook Pro", true, false, Some(false)),
                sonido_de("E8-9E:output", "Sony WH-CH520", true, true, Some(false)),
                sonido_de("BuiltInMicrophoneDevice", "Micrófono de MacBook Pro", false, true, None),
            ],
            pantalla: Some(Pantalla {
                id: 1,
                nombre: "Pantalla de la MacBook".into(),
                interna: true,
                brillo: Some(40),
                controlable: true,
                principal: true,
                es_virtual: false,
            }),
            widgets: vec![(1, "Pantalla de la MacBook".into(), false), (2, "MNT-027".into(), true)],
            aparatos: Some((
                true,
                vec![
                    aparato_de("E8-9E", "WH-CH520", Tipo::Audifonos, true, Some("Sony WH-CH520")),
                    aparato_de("AA-BB", "Magic Keyboard", Tipo::Teclado, true, None),
                ],
            )),
            ocupados: Vec::new(),
            pantalla_virtual: Some(false),
            remoto: true,
        }
    }

    #[test]
    fn bandeja_mac_en_orden() {
        let m = armar(&mac_completa());
        assert_eq!(
            ids(&m),
            [
                "captura",
                "xdrop-captura:100.64.0.2",
                "xdrop-copiado:100.64.0.2",
                "carpeta-capturas",
                "carpeta-xdrop",
                "-",
                "escenas",
                "-",
                "salida",
                "microfono",
                "silencio",
                "-",
                "brillo",
                "-",
                "bt:0:E8-9E",
                "-",
                "preparar",
                "pantalla-virtual",
                "-",
                "abrir",
                "widget",
                "salir",
            ]
        );
        assert!(matches!(&m[0], Pieza::Item { atajo: Some("F13"), .. }));
        assert_eq!(texto_de(&m, "pantalla-virtual"), Some("Pantalla virtual para RustDesk"));
        // Encendida: misma forma, solo cambia la marca.
        let encendida = armar(&Datos { pantalla_virtual: Some(true), ..mac_completa() });
        assert!(misma_forma(&m, &encendida));
        assert!(matches!(encendida.iter().find(|p| llave(p) == (1, "pantalla-virtual")), Some(Pieza::Casilla { marcada: true, .. })));
        assert_eq!(texto_de(&m, "xdrop-captura:100.64.0.2"), Some("Enviar captura a innotech1017"));
        assert_eq!(texto_de(&m, "salida"), Some("Salida: Sony WH-CH520"));
        assert_eq!(texto_de(&m, "silencio"), Some("Silenciar"));
        assert_eq!(texto_de(&m, "brillo"), Some("Brillo: 40 %"));
        // El alias manda; el teclado no es de audio.
        assert_eq!(texto_de(&m, "bt:0:E8-9E"), Some("Desconectar Sony WH-CH520"));
    }

    #[test]
    fn bandeja_windows_sin_lo_de_la_mac() {
        let m = armar(&Datos { mac: false, windows: true, ..mac_completa() });
        let lista = ids(&m);
        assert!(!lista.iter().any(|i| i == "captura" || i == "carpeta-capturas" || i == "preparar" || i == "pantalla-virtual"));
        assert!(!lista.iter().any(|i| i.starts_with("xdrop-captura")));
        assert_eq!(lista[0], "xdrop-copiado:100.64.0.2");
        assert!(lista.iter().any(|i| i == "conectar-mac"));
        assert_eq!(texto_de(&m, "brillo"), Some("Brillo: 40 %"));
        // Con las teclas de brillo encendidas, el atajo va junto al nivel.
        let atajo_brillo = Some("Ctrl+Alt+F1/F2".to_string());
        let m = armar(&Datos { mac: false, windows: true, atajo_brillo, ..mac_completa() });
        assert_eq!(texto_de(&m, "brillo"), Some("Brillo: 40 % · Ctrl+Alt+F1/F2"));
    }

    #[test]
    fn bandeja_sin_destinos_ni_datos() {
        let m = armar(&Datos { mac: true, windows: false, remoto: true, ..Datos::vacios() });
        assert_eq!(
            ids(&m),
            ["captura", "carpeta-capturas", "carpeta-xdrop", "-", "escenas", "-", "preparar", "-", "abrir", "widget", "salir"]
        );
        assert!(matches!(&m[0], Pieza::Item { atajo: None, .. }));
        // El primer menú, antes de leer los ajustes, sale sin lo del remoto.
        assert!(!ids(&armar(&Datos { mac: true, windows: false, ..Datos::vacios() })).iter().any(|i| i == "preparar"));
    }

    #[test]
    fn bandeja_sin_perfil_remoto() {
        // Mac sin tu PC: ni "Preparar para el trabajo" ni la pantalla virtual, y sin raya de más.
        let m = armar(&Datos { remoto: false, ..mac_completa() });
        let lista = ids(&m);
        assert!(!lista.iter().any(|i| i == "preparar" || i == "pantalla-virtual"));
        assert!(!lista.windows(2).any(|par| par[0] == "-" && par[1] == "-"));
        assert_eq!(&lista[lista.len() - 5..], ["bt:0:E8-9E", "-", "abrir", "widget", "salir"]);
        // Windows sin tu Mac: sin "Conectar a mi Mac".
        let m = armar(&Datos { mac: false, windows: true, remoto: false, ..mac_completa() });
        assert!(!ids(&m).iter().any(|i| i == "conectar-mac"));
        // Encenderlo cambia la forma del menú: esa parte se rehace.
        assert!(!misma_forma(&m, &armar(&Datos { mac: false, windows: true, ..mac_completa() })));
    }

    #[test]
    fn bandeja_varios_destinos_en_submenu() {
        let destinos = vec![("100.64.0.2".into(), "innotech1017".into()), ("100.64.0.3".into(), "demo-sige".into())];
        let m = armar(&Datos { destinos, ..mac_completa() });
        let Some(Pieza::Sub { hijos, texto, .. }) = m.iter().find(|p| llave(p) == (2, "xdrop-copiado")) else {
            panic!("falta el submenú de destinos");
        };
        assert_eq!(texto, "Enviar lo que copiaste a");
        assert_eq!(ids(hijos), ["xdrop-copiado:100.64.0.2", "xdrop-copiado:100.64.0.3"]);
    }

    #[test]
    fn bandeja_cambios_en_su_lugar() {
        let antes = armar(&mac_completa());
        // Otro nivel de brillo y la salida silenciada: mismos elementos, otros textos.
        let mut datos = mac_completa();
        if let Some(p) = datos.pantalla.as_mut() {
            p.brillo = Some(70);
        }
        datos.sonido[1].silenciado = Some(true);
        let despues = armar(&datos);
        assert!(misma_forma(&antes, &despues));
        assert_eq!(texto_de(&despues, "silencio"), Some("Quitar silencio"));
        // Conectar o desconectar cambia el id: ese elemento se rehace.
        if let Some((_, l)) = datos.aparatos.as_mut() {
            l[0].conectado = false;
        }
        assert!(!misma_forma(&antes, &armar(&datos)));
    }

    #[test]
    fn bandeja_widgets_de_brillo_en_el_submenu() {
        let m = armar(&mac_completa());
        let Some(Pieza::Sub { hijos, .. }) = m.iter().find(|p| llave(p) == (2, "brillo")) else {
            panic!("falta el submenú de brillo");
        };
        assert_eq!(texto_de(hijos, "widget-brillo:2"), Some("Widget de brillo: MNT-027"));
        assert!(matches!(hijos.last(), Some(Pieza::Casilla { marcada: true, .. })));
        // Sin pantallas que dejen cambiarlo, no hay casillas ni raya de más.
        let m = armar(&Datos { widgets: Vec::new(), ..mac_completa() });
        let Some(Pieza::Sub { hijos, .. }) = m.iter().find(|p| llave(p) == (2, "brillo")) else {
            panic!("falta el submenú de brillo");
        };
        assert!(!hijos.iter().any(|h| matches!(h, Pieza::Casilla { .. })));
    }

    #[test]
    fn bandeja_aparato_ocupado_y_bluetooth_apagado() {
        let mut datos = mac_completa();
        datos.ocupados = vec![("E8-9E".into(), false)];
        let m = armar(&datos);
        assert_eq!(texto_de(&m, "ocupado:E8-9E"), Some("Desconectando Sony WH-CH520…"));
        datos.aparatos = Some((false, datos.aparatos.take().map(|(_, l)| l).unwrap_or_default()));
        let m = armar(&datos);
        assert!(matches!(m.iter().find(|p| llave(p) == (0, "bluetooth-apagado")), Some(Pieza::Item { activo: false, .. })));
    }

    #[test]
    fn bandeja_nombres_cortos_y_con_ampersand() {
        assert_eq!(corto("  Bose QC  "), "Bose QC");
        assert_eq!(corto("Tom & Jerry"), "Tom && Jerry");
        let largo = corto("Un nombre de aparato que es demasiado largo para el menú");
        assert!(largo.ends_with('…'));
        assert!(largo.chars().count() <= LARGO);
    }
}
