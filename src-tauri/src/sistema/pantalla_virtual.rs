//! Pantalla virtual (solo Mac): una segunda pantalla para verla en otra ventana de RustDesk.
//!
//! - Se crea con las clases privadas de CoreGraphics (CGVirtualDisplay y compañía), como DeskPad y BetterDisplay.
//!   Probado en macOS 26.6: se crea, cambia de resolución con `applySettings:` y se va al soltarla.
//! - Existe mientras Nexo guarda el objeto: soltarlo la quita. Si Nexo se cierra, macOS la quita sola.
//! - Se crea, se cambia y se suelta en el hilo principal, con la cola principal en el descriptor (como DeskPad).
//! - Resoluciones fijas, a 60 Hz y sin HiDPI: menos píxeles que mandar por RustDesk.
//! - Se guarda en ajustes.json (`pantallaVirtual`): al abrir Nexo vuelve si estaba activa.
//! - Con la sesión remota (remoto.rs) puede crearse al empezar y quitarse al terminar.
//! - Pantallas la muestra como "Virtual": sin brillo y fuera de DDC/CI (ver `es_nuestra`).
//!
//! En Windows no existe: los comandos dicen "Solo en la Mac".

use super::ajustes;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// El nombre con que macOS la muestra (Ajustes › Pantallas) y con que sale en Nexo.
#[cfg(target_os = "macos")]
pub const NOMBRE: &str = "Nexo · Pantalla virtual";

/// Las resoluciones que se pueden elegir (la primera es la de fábrica). Caben en el monitor de la oficina.
pub const RESOLUCIONES: [(u32, u32); 4] = [(1920, 1080), (1600, 900), (2560, 1440), (1440, 900)];

const SOLO_MAC: &str = "Solo en la Mac.";

/// Lo que se guarda en ajustes.json, dentro de "pantallaVirtual".
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AjustesPantallaVirtual {
    /// Debe existir: al abrir Nexo se vuelve a crear.
    pub activa: bool,
    pub ancho: u32,
    pub alto: u32,
    /// Quitarla cuando termina la sesión de RustDesk.
    pub quitar_al_terminar: bool,
    /// Crearla cuando empieza una sesión de RustDesk.
    pub crear_al_empezar: bool,
}

impl Default for AjustesPantallaVirtual {
    fn default() -> Self {
        let (ancho, alto) = RESOLUCIONES[0];
        AjustesPantallaVirtual { activa: false, ancho, alto, quitar_al_terminar: false, crear_al_empezar: false }
    }
}

/// Lo que ve la interfaz.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EstadoPantallaVirtual {
    /// Existe ahora mismo.
    pub activa: bool,
    /// La resolución en uso o, si no existe, la que tendrá.
    pub ancho: u32,
    pub alto: u32,
    /// Este macOS deja crearla (en Windows, nunca).
    pub disponible: bool,
    /// Su número de pantalla en macOS, mientras existe.
    pub id: Option<u32>,
    pub quitar_al_terminar: bool,
    pub crear_al_empezar: bool,
}

/// Lo que cambia la interfaz con `pantalla_virtual_ajustar`. Lo que no viene, queda igual.
#[derive(Deserialize, Default, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase", default)]
pub struct CambioPantallaVirtual {
    pub quitar_al_terminar: Option<bool>,
    pub crear_al_empezar: Option<bool>,
}

/// Solo las resoluciones de la lista.
fn resolucion(ancho: u32, alto: u32) -> Result<(u32, u32), String> {
    RESOLUCIONES
        .into_iter()
        .find(|r| *r == (ancho, alto))
        .ok_or_else(|| format!("La pantalla virtual no tiene {ancho} × {alto}. Elige una de la lista."))
}

/// La resolución guardada; si no es de la lista, la de fábrica.
fn guardada(a: &AjustesPantallaVirtual) -> (u32, u32) {
    resolucion(a.ancho, a.alto).unwrap_or(RESOLUCIONES[0])
}

/// Si este sistema deja crear la pantalla virtual.
pub fn disponible() -> bool {
    #[cfg(target_os = "macos")]
    {
        mac::disponible()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// (id, ancho, alto) de la pantalla virtual, si existe.
fn viva() -> Option<(u32, u32, u32)> {
    #[cfg(target_os = "macos")]
    {
        mac::actual()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

pub fn estado(app: &AppHandle) -> EstadoPantallaVirtual {
    let a = ajustes::cargar(app).pantalla_virtual;
    let (ancho, alto) = guardada(&a);
    let v = viva();
    EstadoPantallaVirtual {
        activa: v.is_some(),
        ancho: v.map_or(ancho, |v| v.1),
        alto: v.map_or(alto, |v| v.2),
        disponible: disponible(),
        id: v.map(|v| v.0),
        quitar_al_terminar: a.quitar_al_terminar,
        crear_al_empezar: a.crear_al_empezar,
    }
}

/// Para el menú del ícono: `Some(activa)` en la Mac, si se puede crear; si no, `None` (no se muestra).
pub fn en_menu() -> Option<bool> {
    disponible().then(|| viva().is_some())
}

/// Crea, cambia de resolución o quita la pantalla virtual, y lo guarda. Sin resolución, usa la guardada.
/// Espera a que macOS la muestre (o la quite) antes de volver: nunca llamar desde el hilo principal.
pub fn usar(app: &AppHandle, activar: bool, ancho: Option<u32>, alto: Option<u32>) -> Result<EstadoPantallaVirtual, String> {
    let guardados = ajustes::cargar(app).pantalla_virtual;
    let (ancho, alto) = match (ancho, alto) {
        (Some(a), Some(h)) => resolucion(a, h)?,
        (None, None) => guardada(&guardados),
        _ => return Err("Falta el ancho o el alto de la pantalla virtual.".into()),
    };
    cambiar_pantalla(app, activar, ancho, alto)?;
    ajustes::cambiar(app, |a| {
        let p = &mut a.pantalla_virtual;
        p.activa = activar;
        p.ancho = ancho;
        p.alto = alto;
    })?;
    let e = estado(app);
    avisar(app, &e);
    Ok(e)
}

#[cfg(target_os = "macos")]
fn cambiar_pantalla(app: &AppHandle, activar: bool, ancho: u32, alto: u32) -> Result<(), String> {
    if activar { mac::poner(app, ancho, alto) } else { mac::quitar(app) }
}

#[cfg(not(target_os = "macos"))]
fn cambiar_pantalla(_app: &AppHandle, _activar: bool, _ancho: u32, _alto: u32) -> Result<(), String> {
    Err(SOLO_MAC.into())
}

/// Desde el menú del ícono: la crea si no existe y la quita si existe.
pub fn alternar(app: &AppHandle) -> Result<EstadoPantallaVirtual, String> {
    usar(app, viva().is_none(), None, None)
}

/// Avisa a las ventanas ("pantalla-virtual-cambio") y pone al día la casilla del menú del ícono.
fn avisar(app: &AppHandle, e: &EstadoPantallaVirtual) {
    let _ = app.emit("pantalla-virtual-cambio", e);
    super::bandeja::actualizar_marcas(app);
}

/// Arranque (en el hilo principal): si quedó activa, vuelve a crearla. No espera a que macOS la muestre.
pub fn iniciar(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let a = ajustes::cargar(app).pantalla_virtual;
        if !a.activa || !mac::disponible() {
            return;
        }
        let (ancho, alto) = guardada(&a);
        match mac::en_principal(app, move || mac::poner_aqui(ancho, alto)) {
            Ok(Ok(_)) => {}
            Ok(Err(e)) | Err(e) => eprintln!("Nexo: no se pudo crear la pantalla virtual ({e})"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

/// La sesión de RustDesk empezó (`sesion` = true) o terminó: la crea o la quita si así se eligió.
/// Va en su propio hilo: no frena al vigía de la sesión.
#[cfg(target_os = "macos")]
pub fn al_cambiar_sesion(app: &AppHandle, sesion: bool) {
    let a = ajustes::cargar(app).pantalla_virtual;
    let quiere = if sesion { a.crear_al_empezar } else { a.quitar_al_terminar };
    if !quiere || !mac::disponible() || viva().is_some() == sesion {
        return;
    }
    let app = app.clone();
    let hilo = std::thread::Builder::new().name("nexo-pantalla-virtual".into()).spawn(move || {
        if let Err(e) = usar(&app, sesion, None, None) {
            eprintln!("Nexo: pantalla virtual con la sesión remota ({e})");
        }
    });
    if let Err(e) = hilo {
        eprintln!("Nexo: no se pudo cambiar la pantalla virtual ({e})");
    }
}

/// Si una pantalla es la virtual de Nexo (por su fabricante y modelo). Lo usa el hilo de pantallas.
#[cfg(target_os = "macos")]
pub fn es_nuestra(id: u32) -> bool {
    mac::es_nuestra(id)
}

// ---------------------------------------------------------------- Comandos

/// `{ activa, ancho, alto, disponible, id, quitarAlTerminar, crearAlEmpezar }`.
#[tauri::command]
pub async fn pantalla_virtual_estado(app: AppHandle) -> EstadoPantallaVirtual {
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || estado(&a)).await.unwrap_or_else(|_| estado(&app))
}

/// Crea (`activar`), cambia de resolución o quita la pantalla virtual. Devuelve cómo quedó.
#[tauri::command]
pub async fn pantalla_virtual(
    app: AppHandle,
    activar: bool,
    ancho: Option<u32>,
    alto: Option<u32>,
) -> Result<EstadoPantallaVirtual, String> {
    tauri::async_runtime::spawn_blocking(move || usar(&app, activar, ancho, alto)).await.map_err(|e| e.to_string())?
}

/// Cambia "Crear al empezar una sesión remota" o "Quitar al terminar la sesión remota". Devuelve cómo quedó.
#[tauri::command]
pub async fn pantalla_virtual_ajustar(app: AppHandle, cambio: CambioPantallaVirtual) -> Result<EstadoPantallaVirtual, String> {
    if !cfg!(target_os = "macos") {
        return Err(SOLO_MAC.into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        ajustes::cambiar(&app, |a| {
            let p = &mut a.pantalla_virtual;
            if let Some(v) = cambio.quitar_al_terminar {
                p.quitar_al_terminar = v;
            }
            if let Some(v) = cambio.crear_al_empezar {
                p.crear_al_empezar = v;
            }
        })?;
        let e = estado(&app);
        avisar(&app, &e);
        Ok(e)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- Mac: CGVirtualDisplay

#[cfg(target_os = "macos")]
mod mac {
    use objc2::rc::{Allocated, Retained, autoreleasepool};
    use objc2::runtime::{AnyClass, AnyObject, Bool, Sel};
    use objc2::{MainThreadMarker, msg_send, sel};
    use objc2_foundation::{NSArray, NSSize, NSString};
    use std::ffi::CStr;
    use std::sync::{Mutex, OnceLock, mpsc};
    use std::time::{Duration, Instant};
    use tauri::AppHandle;

    /// Fabricante "NXO" (tres letras de EDID), modelo y serie: así se reconoce entre las pantallas.
    /// Fijos, para que macOS recuerde dónde la dejaste en Ajustes › Pantallas › Organizar.
    const FABRICANTE: u32 = 0x3B0F;
    const MODELO: u32 = 0x4E56;
    const SERIE: u32 = 1;
    /// Un monitor de 24" (16:9), en milímetros.
    const MILIMETROS: (f64, f64) = (527.0, 296.0);
    /// Lo más grande que puede llegar a ser (la resolución mayor de la lista).
    const MAXIMO: (u32, u32) = (2560, 1440);
    const HZ: f64 = 60.0;
    /// Recién creada, macOS tarda un momento en ponerla en la lista de pantallas.
    const GRACIA: Duration = Duration::from_secs(5);

    const NO_DISPONIBLE: &str = "Este macOS no deja crear pantallas virtuales.";

    /// La pantalla virtual que existe: soltar `objeto` la quita.
    struct Viva {
        objeto: Retained<AnyObject>,
        id: u32,
        ancho: u32,
        alto: u32,
        desde: Instant,
    }

    // SAFETY: el objeto se crea, se cambia y se suelta solo en el hilo principal (`en_principal`).
    // Desde otros hilos solo se leen los números.
    unsafe impl Send for Viva {}

    static VIVA: Mutex<Option<Viva>> = Mutex::new(None);

    /// La cola principal de GCD (`dispatch_get_main_queue()` en C).
    #[repr(C)]
    struct Cola {
        _privado: [u8; 0],
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGGetOnlineDisplayList(max: u32, ids: *mut u32, cantidad: *mut u32) -> i32;
        fn CGDisplayVendorNumber(id: u32) -> u32;
        fn CGDisplayModelNumber(id: u32) -> u32;
        fn CGDisplayPixelsWide(id: u32) -> usize;
        fn CGDisplayPixelsHigh(id: u32) -> usize;
    }

    unsafe extern "C" {
        static _dispatch_main_q: Cola;
    }

    fn bloquear() -> std::sync::MutexGuard<'static, Option<Viva>> {
        VIVA.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Las clases y los mensajes que se usan. Si falta alguno (otro macOS), no se intenta nada.
    pub fn disponible() -> bool {
        static SI: OnceLock<bool> = OnceLock::new();
        *SI.get_or_init(|| {
            let tiene = |clase: &CStr, mensajes: &[Sel]| {
                AnyClass::get(clase).is_some_and(|c| mensajes.iter().all(|m| c.responds_to(*m)))
            };
            tiene(
                c"CGVirtualDisplayDescriptor",
                &[
                    sel!(setQueue:),
                    sel!(setName:),
                    sel!(setMaxPixelsWide:),
                    sel!(setMaxPixelsHigh:),
                    sel!(setSizeInMillimeters:),
                    sel!(setProductID:),
                    sel!(setVendorID:),
                    sel!(setSerialNum:),
                ],
            ) && tiene(c"CGVirtualDisplay", &[sel!(initWithDescriptor:), sel!(applySettings:), sel!(displayID)])
                && tiene(c"CGVirtualDisplaySettings", &[sel!(setHiDPI:), sel!(setModes:)])
                && tiene(c"CGVirtualDisplayMode", &[sel!(initWithWidth:height:refreshRate:)])
        })
    }

    /// Corre `f` en el hilo principal y espera su resultado. Si ya está en él, la corre ahí mismo.
    pub fn en_principal<T: Send + 'static>(app: &AppHandle, f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
        if MainThreadMarker::new().is_some() {
            return Ok(f());
        }
        let (tx, rx) = mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(f());
        })
        .map_err(|e| format!("No pude pedirle la pantalla a macOS ({e})."))?;
        rx.recv_timeout(Duration::from_secs(10)).map_err(|_| "macOS no respondió a tiempo.".to_string())
    }

    fn en_linea() -> Vec<u32> {
        let mut ids = [0u32; 16];
        let mut n = 0u32;
        // SAFETY: el arreglo tiene lugar para `max` pantallas y `n` nunca pasa de ahí.
        if unsafe { CGGetOnlineDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut n) } != 0 {
            return Vec::new();
        }
        ids[..(n as usize).min(ids.len())].to_vec()
    }

    pub fn es_nuestra(id: u32) -> bool {
        // SAFETY: funciones de solo lectura; con un id que no existe devuelven 0.
        unsafe { CGDisplayVendorNumber(id) == FABRICANTE && CGDisplayModelNumber(id) == MODELO }
    }

    fn mide(id: u32, ancho: u32, alto: u32) -> bool {
        // SAFETY: solo lectura.
        let (w, h) = unsafe { (CGDisplayPixelsWide(id), CGDisplayPixelsHigh(id)) };
        (w, h) == (ancho as usize, alto as usize)
    }

    /// (id, ancho, alto) si existe: está en la lista de macOS o se acaba de crear.
    pub fn actual() -> Option<(u32, u32, u32)> {
        let v = bloquear();
        let v = v.as_ref()?;
        (v.desde.elapsed() < GRACIA || en_linea().contains(&v.id)).then_some((v.id, v.ancho, v.alto))
    }

    /// Espera (hasta 3 s) a que macOS la muestre con esa resolución o a que la quite.
    fn esperar(listo: impl Fn() -> bool) {
        for _ in 0..30 {
            if listo() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// La crea o le cambia la resolución, y espera a que macOS la muestre.
    pub fn poner(app: &AppHandle, ancho: u32, alto: u32) -> Result<(), String> {
        let id = en_principal(app, move || poner_aqui(ancho, alto))??;
        esperar(|| en_linea().contains(&id) && mide(id, ancho, alto));
        Ok(())
    }

    /// La quita y espera a que macOS la saque de la lista.
    pub fn quitar(app: &AppHandle) -> Result<(), String> {
        let id = en_principal(app, || bloquear().take().map(|v| v.id))?;
        if let Some(id) = id {
            esperar(|| !en_linea().contains(&id));
        }
        Ok(())
    }

    /// En el hilo principal: si existe, le cambia el modo; si no aceptó el cambio (o macOS la quitó), la crea de nuevo.
    pub fn poner_aqui(ancho: u32, alto: u32) -> Result<u32, String> {
        if !disponible() {
            return Err(NO_DISPONIBLE.into());
        }
        let mut viva = bloquear();
        if let Some(v) = viva.as_mut() {
            let sigue = v.desde.elapsed() < GRACIA || en_linea().contains(&v.id);
            if sigue && (v.ancho, v.alto) == (ancho, alto) {
                return Ok(v.id);
            }
            if sigue && aplicar(&v.objeto, ancho, alto) {
                v.ancho = ancho;
                v.alto = alto;
                return Ok(v.id);
            }
        }
        // Se suelta la de antes (si había) y se crea otra.
        *viva = None;
        let (objeto, id) = crear()?;
        if !aplicar(&objeto, ancho, alto) {
            return Err(format!("macOS no aceptó {ancho} × {alto} en la pantalla virtual."));
        }
        *viva = Some(Viva { objeto, id, ancho, alto, desde: Instant::now() });
        Ok(id)
    }

    /// Crea la pantalla (todavía sin modos): el descriptor con nombre, tamaño, fabricante y la cola principal.
    fn crear() -> Result<(Retained<AnyObject>, u32), String> {
        let descriptor = AnyClass::get(c"CGVirtualDisplayDescriptor").ok_or(NO_DISPONIBLE)?;
        let clase = AnyClass::get(c"CGVirtualDisplay").ok_or(NO_DISPONIBLE)?;
        let cola = (&raw const _dispatch_main_q).cast::<AnyObject>().cast_mut();
        let nombre = NSString::from_str(super::NOMBRE);
        autoreleasepool(|_| {
            // SAFETY: mensajes revisados en `disponible()`; tipos según el runtime de macOS 26
            // (I = u32, {CGSize=dd}, @ = objeto). La cola principal vive siempre.
            unsafe {
                let d: Allocated<AnyObject> = msg_send![descriptor, alloc];
                let d: Option<Retained<AnyObject>> = msg_send![d, init];
                let d = d.ok_or("macOS no armó la pantalla virtual.")?;
                let _: () = msg_send![&*d, setQueue: cola];
                let _: () = msg_send![&*d, setName: &*nombre];
                let _: () = msg_send![&*d, setMaxPixelsWide: MAXIMO.0];
                let _: () = msg_send![&*d, setMaxPixelsHigh: MAXIMO.1];
                let _: () = msg_send![&*d, setSizeInMillimeters: NSSize::new(MILIMETROS.0, MILIMETROS.1)];
                let _: () = msg_send![&*d, setVendorID: FABRICANTE];
                let _: () = msg_send![&*d, setProductID: MODELO];
                let _: () = msg_send![&*d, setSerialNum: SERIE];

                let p: Allocated<AnyObject> = msg_send![clase, alloc];
                let p: Option<Retained<AnyObject>> = msg_send![p, initWithDescriptor: &*d];
                let p = p.ok_or("macOS no creó la pantalla virtual.")?;
                let id: u32 = msg_send![&*p, displayID];
                if id == 0 {
                    return Err("macOS no creó la pantalla virtual.".into());
                }
                Ok((p, id))
            }
        })
    }

    /// Le pone un solo modo: ancho × alto a 60 Hz, sin HiDPI. `false` si macOS no lo aceptó.
    fn aplicar(pantalla: &AnyObject, ancho: u32, alto: u32) -> bool {
        let (Some(clase_ajustes), Some(clase_modo)) =
            (AnyClass::get(c"CGVirtualDisplaySettings"), AnyClass::get(c"CGVirtualDisplayMode"))
        else {
            return false;
        };
        autoreleasepool(|_| {
            // SAFETY: mensajes revisados en `disponible()`; initWithWidth:height:refreshRate: es (I, I, d).
            unsafe {
                let a: Allocated<AnyObject> = msg_send![clase_ajustes, alloc];
                let Some(ajustes): Option<Retained<AnyObject>> = msg_send![a, init] else { return false };
                let _: () = msg_send![&*ajustes, setHiDPI: 0u32];
                let m: Allocated<AnyObject> = msg_send![clase_modo, alloc];
                let Some(modo): Option<Retained<AnyObject>> = msg_send![m, initWithWidth: ancho, height: alto, refreshRate: HZ]
                else {
                    return false;
                };
                let modos = NSArray::from_retained_slice(&[modo]);
                let _: () = msg_send![&*ajustes, setModes: &*modos];
                let ok: Bool = msg_send![pantalla, applySettings: &*ajustes];
                ok.as_bool()
            }
        })
    }
}
