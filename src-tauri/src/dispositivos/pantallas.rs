//! Pantallas: la de la MacBook (DisplayServices) y los monitores externos (DDC/CI).
//!
//! Todo el brillo pasa por un solo hilo dueño de los monitores: los comandos DDC no se pisan
//! y varios toques seguidos de las teclas de brillo se juntan en un solo cambio.
//! La resolución, la frecuencia y cuál es la pantalla principal se cambian con CoreGraphics
//! (funciones sueltas al final: no tocan DDC, así que no pasan por ese hilo).
//! Windows tiene lo mismo en `pantallas_windows.rs` (DDC con dxva2, WMI y ChangeDisplaySettingsEx).

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, mpsc};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Pantalla {
    pub id: u32,
    pub nombre: String,
    pub interna: bool,
    /// Brillo 0–100, si se puede leer.
    pub brillo: Option<u8>,
    /// Si Nexo puede cambiarle el brillo.
    pub controlable: bool,
    /// La pantalla principal: la que tiene la barra de menús.
    pub principal: bool,
}

/// Un modo de pantalla (resolución y frecuencia) que se puede elegir.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct ModoPantalla {
    /// Identificador del modo; es lo que recibe `cambiar_modo`. En macOS, su ioDisplayModeID;
    /// en Windows, su número en la lista del controlador (iModeNum de EnumDisplaySettingsExW).
    pub id: i32,
    /// Tamaño del escritorio en puntos (lo que se ve como "resolución").
    pub ancho: u32,
    pub alto: u32,
    /// Píxeles reales que se dibujan; en Retina son el doble de los puntos.
    pub pixeles_ancho: u32,
    pub pixeles_alto: u32,
    /// Frecuencia en Hz, con dos decimales (59,94). Las pantallas que dicen 0 se toman como 60.
    pub hz: f64,
    /// Se ve nítido (HiDPI): cada punto usa más de un píxel.
    pub retina: bool,
    /// Es el modo que la pantalla usa ahora.
    pub actual: bool,
}

enum Orden {
    Listar(mpsc::Sender<Vec<Pantalla>>),
    PonerBrillo(u32, u8, mpsc::Sender<Result<u8, String>>),
    /// Un toque de tecla: +1 sube, −1 baja (en pasos de 1/16).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // hoy solo lo usa el atajo de teclas de la Mac
    Paso(u32, i32),
    Refrescar,
}

/// Lo que se avisa tras cambiar el brillo con las teclas: (pantalla, nivel 0–100).
pub type Aviso = Box<dyn Fn(u32, u8) + Send>;

pub struct Pantallas {
    tx: Mutex<mpsc::Sender<Orden>>,
    /// Monitores externos que responden a DDC (lo consulta el atajo de teclas sin esperar).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    controlables: Arc<Mutex<HashSet<u32>>>,
    /// Último brillo conocido de cada monitor externo (para leerlo al instante).
    /// En Windows también el de la pantalla del portátil (WMI tampoco se lee al instante).
    brillos_externos: Arc<Mutex<HashMap<u32, u8>>>,
}

impl Pantallas {
    pub fn iniciar(aviso: Aviso) -> Self {
        let (tx, rx) = mpsc::channel();
        let controlables = Arc::new(Mutex::new(HashSet::new()));
        let brillos_externos = Arc::new(Mutex::new(HashMap::new()));
        let (c, b) = (controlables.clone(), brillos_externos.clone());
        std::thread::Builder::new()
            .name("nexo-pantallas".into())
            .spawn(move || trabajador(rx, c, b, aviso))
            .expect("no se pudo iniciar el hilo de pantallas");
        let p = Self { tx: Mutex::new(tx), controlables, brillos_externos };
        p.enviar(Orden::Refrescar);
        p
    }

    fn enviar(&self, orden: Orden) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(orden);
        }
    }

    pub fn listar(&self) -> Vec<Pantalla> {
        let (t, r) = mpsc::channel();
        self.enviar(Orden::Listar(t));
        r.recv().unwrap_or_default()
    }

    pub fn poner_brillo(&self, id: u32, valor: u8) -> Result<u8, String> {
        let (t, r) = mpsc::channel();
        self.enviar(Orden::PonerBrillo(id, valor.min(100), t));
        r.recv().map_err(|e| e.to_string())?
    }

    /// Lo usa el atajo de teclas: no espera respuesta.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn paso(&self, id: u32, pasos: i32) {
        self.enviar(Orden::Paso(id, pasos));
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn refrescar(&self) {
        self.enviar(Orden::Refrescar);
    }

    /// Brillo actual de cada pantalla, al instante (la MacBook se lee directo; los externos, del último valor).
    pub fn brillos(&self) -> Vec<(u32, u8)> {
        let mut lista = Vec::new();
        #[cfg(target_os = "macos")]
        lista.extend(mac::brillos_internos());
        if let Ok(b) = self.brillos_externos.lock() {
            lista.extend(b.iter().map(|(id, v)| (*id, *v)));
        }
        lista
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn es_controlable(&self, id: u32) -> bool {
        self.controlables.lock().map(|c| c.contains(&id)).unwrap_or(false)
    }
}

// ---------------------------------------------------------------- Resolución y pantalla principal

#[cfg(target_os = "macos")]
pub use mac::{cambiar_modo, hacer_principal, modos};

// Windows: todo en su propio archivo, con la misma interfaz.
#[cfg(windows)]
#[path = "pantallas_windows.rs"]
mod win;

#[cfg(windows)]
pub use win::{area_de_trabajo, cambiar_modo, hacer_principal, modos};

#[cfg(windows)]
use win::trabajador;

#[cfg(not(any(target_os = "macos", windows)))]
pub fn modos(_id: u32) -> Vec<ModoPantalla> {
    Vec::new()
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn cambiar_modo(_id: u32, _modo: i32) -> Result<(), String> {
    Err("Todavía no disponible en este sistema".into())
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn hacer_principal(_id: u32) -> Result<(), String> {
    Err("Todavía no disponible en este sistema".into())
}

#[cfg(not(any(target_os = "macos", windows)))]
fn trabajador(
    rx: mpsc::Receiver<Orden>,
    _controlables: Arc<Mutex<HashSet<u32>>>,
    _brillos: Arc<Mutex<HashMap<u32, u8>>>,
    _aviso: Aviso,
) {
    for orden in rx {
        match orden {
            Orden::Listar(t) => {
                let _ = t.send(Vec::new());
            }
            Orden::PonerBrillo(_, _, t) => {
                let _ = t.send(Err("Todavía no disponible en este sistema".into()));
            }
            Orden::Paso(..) | Orden::Refrescar => {}
        }
    }
}

#[cfg(target_os = "macos")]
fn trabajador(
    rx: mpsc::Receiver<Orden>,
    controlables: Arc<Mutex<HashSet<u32>>>,
    brillos: Arc<Mutex<HashMap<u32, u8>>>,
    aviso: Aviso,
) {
    use mac::*;

    let mut externos: HashMap<u32, Externo> = HashMap::new();
    let publicar = |externos: &HashMap<u32, Externo>| {
        if let Ok(mut c) = controlables.lock() {
            *c = externos.keys().copied().collect();
        }
        if let Ok(mut b) = brillos.lock() {
            *b = externos.iter().map(|(id, e)| (*id, e.porcentaje())).collect();
        }
    };

    while let Ok(primera) = rx.recv() {
        // Junta los toques de teclas que ya estén en cola (pantalla → pasos).
        // Y de los "poner brillo" seguidos (al arrastrar), solo vale el último de cada pantalla.
        let mut pasos: HashMap<u32, i32> = HashMap::new();
        let mut poner_ultimo: HashMap<u32, (u8, Vec<mpsc::Sender<Result<u8, String>>>)> = HashMap::new();
        let mut otras = Vec::new();
        for orden in std::iter::once(primera).chain(rx.try_iter()) {
            match orden {
                Orden::Paso(id, n) => *pasos.entry(id).or_default() += n,
                Orden::PonerBrillo(id, valor, t) => {
                    let e = poner_ultimo.entry(id).or_insert((valor, Vec::new()));
                    e.0 = valor;
                    e.1.push(t);
                }
                otra => otras.push(otra),
            }
        }
        for (id, (valor, respuestas)) in poner_ultimo {
            let r = poner(&mut externos, id, valor);
            publicar(&externos);
            for t in respuestas {
                let _ = t.send(r.clone());
            }
        }

        for orden in otras {
            match orden {
                Orden::Refrescar => {
                    refrescar(&mut externos);
                    publicar(&externos);
                }
                Orden::Listar(t) => {
                    refrescar(&mut externos);
                    publicar(&externos);
                    let _ = t.send(listar(&mut externos));
                }
                Orden::PonerBrillo(..) | Orden::Paso(..) => {}
            }
        }

        for (id, n) in pasos {
            if n == 0 {
                continue;
            }
            if let Some(nivel) = paso(&mut externos, id, n) {
                publicar(&externos);
                aviso(id, nivel);
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{ModoPantalla, Pantalla};
    use ddc::Ddc;
    use std::collections::HashMap;
    use std::ffi::{CString, c_void};
    use std::sync::OnceLock;

    const BRILLO_VCP: u8 = 0x10;

    pub struct Externo {
        monitor: ddc_macos::Monitor,
        nombre: String,
        maximo: u16,
        actual: u16,
    }

    impl Externo {
        pub fn porcentaje(&self) -> u8 {
            porcentaje(self.actual, self.maximo)
        }
    }

    /// Brillo de las pantallas integradas (la de la MacBook), leído al instante.
    pub fn brillos_internos() -> Vec<(u32, u8)> {
        ids_activos()
            .into_iter()
            .filter(|id| es_interna(*id))
            .filter_map(|id| brillo_interno(id).map(|b| (id, b)))
            .collect()
    }

    type Modo = *const c_void; // CGDisplayModeRef
    type Configuracion = *mut c_void; // CGDisplayConfigRef

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect {
        x: f64,
        y: f64,
        ancho: f64,
        alto: f64,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGGetActiveDisplayList(max: u32, ids: *mut u32, cantidad: *mut u32) -> i32;
        fn CGDisplayIsBuiltin(id: u32) -> u32;
        fn CGDisplayIsMain(id: u32) -> u32;
        fn CGDisplayMirrorsDisplay(id: u32) -> u32;
        fn CGDisplayBounds(id: u32) -> Rect;

        fn CGDisplayCopyAllDisplayModes(id: u32, opciones: *const c_void) -> *const c_void;
        fn CGDisplayCopyDisplayMode(id: u32) -> Modo;
        fn CGDisplayModeRelease(modo: Modo);
        fn CGDisplayModeGetIODisplayModeID(modo: Modo) -> i32;
        fn CGDisplayModeGetWidth(modo: Modo) -> usize;
        fn CGDisplayModeGetHeight(modo: Modo) -> usize;
        fn CGDisplayModeGetPixelWidth(modo: Modo) -> usize;
        fn CGDisplayModeGetPixelHeight(modo: Modo) -> usize;
        fn CGDisplayModeGetRefreshRate(modo: Modo) -> f64;
        fn CGDisplayModeIsUsableForDesktopGUI(modo: Modo) -> u8;

        fn CGBeginDisplayConfiguration(config: *mut Configuracion) -> i32;
        fn CGConfigureDisplayWithDisplayMode(config: Configuracion, id: u32, modo: Modo, opciones: *const c_void) -> i32;
        fn CGConfigureDisplayOrigin(config: Configuracion, id: u32, x: i32, y: i32) -> i32;
        fn CGCompleteDisplayConfiguration(config: Configuracion, opcion: u32) -> i32;
        fn CGCancelDisplayConfiguration(config: Configuracion) -> i32;

        static kCGDisplayShowDuplicateLowResolutionModes: *const c_void;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFArrayGetCount(arreglo: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(arreglo: *const c_void, i: isize) -> *const c_void;
        fn CFDictionaryCreate(
            alloc: *const c_void,
            claves: *const *const c_void,
            valores: *const *const c_void,
            n: isize,
            cb_claves: *const c_void,
            cb_valores: *const c_void,
        ) -> *const c_void;
        fn CFRelease(objeto: *const c_void);
        static kCFBooleanTrue: *const c_void;
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
    }

    /// kCGConfigurePermanently: el cambio queda guardado (sobrevive a reiniciar y a cerrar sesión).
    const PERMANENTE: u32 = 2;

    // DisplayServices (privado) cambia el brillo de la pantalla de la MacBook.
    type LeerBrillo = unsafe extern "C" fn(u32, *mut f32) -> i32;
    type PonerBrillo = unsafe extern "C" fn(u32, f32) -> i32;
    struct Funciones {
        leer: Option<LeerBrillo>,
        poner: Option<PonerBrillo>,
    }
    unsafe extern "C" {
        fn dlopen(ruta: *const i8, modo: i32) -> *mut c_void;
        fn dlsym(lib: *mut c_void, nombre: *const i8) -> *mut c_void;
    }
    fn display_services() -> &'static Funciones {
        static F: OnceLock<Funciones> = OnceLock::new();
        F.get_or_init(|| unsafe {
            let ruta = CString::new("/System/Library/PrivateFrameworks/DisplayServices.framework/DisplayServices").unwrap();
            let lib = dlopen(ruta.as_ptr(), 1);
            let buscar = |n: &str| {
                let n = CString::new(n).unwrap();
                if lib.is_null() { std::ptr::null_mut() } else { dlsym(lib, n.as_ptr()) }
            };
            let (l, p) = (buscar("DisplayServicesGetBrightness"), buscar("DisplayServicesSetBrightness"));
            Funciones {
                leer: (!l.is_null()).then(|| std::mem::transmute::<*mut c_void, LeerBrillo>(l)),
                poner: (!p.is_null()).then(|| std::mem::transmute::<*mut c_void, PonerBrillo>(p)),
            }
        })
    }

    fn brillo_interno(id: u32) -> Option<u8> {
        let f = display_services().leer?;
        let mut v = 0f32;
        (unsafe { f(id, &mut v) } == 0).then(|| (v.clamp(0.0, 1.0) * 100.0).round() as u8)
    }

    fn ids_activos() -> Vec<u32> {
        let mut ids = [0u32; 16];
        let mut n = 0u32;
        if unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut n) } != 0 {
            return Vec::new();
        }
        ids[..n as usize].to_vec()
    }

    fn es_interna(id: u32) -> bool {
        unsafe { CGDisplayIsBuiltin(id) != 0 }
    }

    fn es_principal(id: u32) -> bool {
        unsafe { CGDisplayIsMain(id) != 0 }
    }

    /// Busca monitores externos nuevos (y olvida los desconectados). Solo lee DDC de los nuevos.
    pub fn refrescar(externos: &mut HashMap<u32, Externo>) {
        let activos: Vec<u32> = ids_activos().into_iter().filter(|id| !es_interna(*id)).collect();
        externos.retain(|id, _| activos.contains(id));
        if activos.iter().all(|id| externos.contains_key(id)) {
            return;
        }
        let Ok(monitores) = ddc_macos::Monitor::enumerate() else { return };
        for mut monitor in monitores {
            let id = monitor.handle().id;
            if !activos.contains(&id) || externos.contains_key(&id) {
                continue;
            }
            // Si no responde a DDC (o tiene DDC/CI apagado en su menú), no es controlable.
            if let Ok(valor) = monitor.get_vcp_feature(BRILLO_VCP) {
                let maximo = valor.maximum().max(1);
                let nombre = monitor.product_name().unwrap_or_else(|| "Monitor externo".into());
                externos.insert(id, Externo { nombre, maximo, actual: valor.value().min(maximo), monitor });
            }
        }
    }

    pub fn listar(externos: &mut HashMap<u32, Externo>) -> Vec<Pantalla> {
        let mut lista: Vec<Pantalla> = ids_activos()
            .into_iter()
            .map(|id| {
                if es_interna(id) {
                    let brillo = brillo_interno(id);
                    Pantalla {
                        id,
                        nombre: "Pantalla de la MacBook".into(),
                        interna: true,
                        brillo,
                        controlable: brillo.is_some() && display_services().poner.is_some(),
                        principal: es_principal(id),
                    }
                } else if let Some(e) = externos.get(&id) {
                    Pantalla {
                        id,
                        nombre: e.nombre.clone(),
                        interna: false,
                        brillo: Some(porcentaje(e.actual, e.maximo)),
                        controlable: true,
                        principal: es_principal(id),
                    }
                } else {
                    Pantalla {
                        id,
                        nombre: "Monitor externo".into(),
                        interna: false,
                        brillo: None,
                        controlable: false,
                        principal: es_principal(id),
                    }
                }
            })
            .collect();
        lista.sort_by_key(|p| !p.interna);
        lista
    }

    fn porcentaje(valor: u16, maximo: u16) -> u8 {
        ((valor as f32 / maximo as f32) * 100.0).round().clamp(0.0, 100.0) as u8
    }

    pub fn poner(externos: &mut HashMap<u32, Externo>, id: u32, valor: u8) -> Result<u8, String> {
        if es_interna(id) {
            let f = display_services().poner.ok_or("No se puede cambiar el brillo de esta pantalla")?;
            return if unsafe { f(id, valor as f32 / 100.0) } == 0 {
                Ok(valor)
            } else {
                Err("macOS no aceptó el cambio de brillo".into())
            };
        }
        let e = externos.get_mut(&id).ok_or("Este monitor no responde. Activa DDC/CI en su menú.")?;
        let nuevo = ((valor as f32 / 100.0) * e.maximo as f32).round() as u16;
        e.monitor
            .set_vcp_feature(BRILLO_VCP, nuevo)
            .map_err(|err| format!("El monitor no aceptó el cambio: {err}"))?;
        e.actual = nuevo;
        Ok(porcentaje(nuevo, e.maximo))
    }

    /// Sube o baja en pasos de 1/16 (como las teclas de la Mac). Devuelve el nivel final.
    pub fn paso(externos: &mut HashMap<u32, Externo>, id: u32, pasos: i32) -> Option<u8> {
        let e = externos.get_mut(&id)?;
        let tramo = (e.maximo as f32 / 16.0).max(1.0);
        let nuevo = (e.actual as f32 + pasos as f32 * tramo).round().clamp(0.0, e.maximo as f32) as u16;
        e.monitor.set_vcp_feature(BRILLO_VCP, nuevo).ok()?;
        e.actual = nuevo;
        Some(porcentaje(nuevo, e.maximo))
    }

    // ------------------------------------------------------------ Resolución y frecuencia

    /// Todos los modos de una pantalla (CFArray de CGDisplayMode); se libera solo al soltarlo.
    /// Se piden con "duplicados de baja resolución": en macOS reciente es la única forma de
    /// recibir los modos Retina (sin la opción solo llegan los de un píxel por punto).
    struct ListaModos(*const c_void);

    impl ListaModos {
        fn de(id: u32) -> Self {
            unsafe {
                let claves = [kCGDisplayShowDuplicateLowResolutionModes];
                let valores = [kCFBooleanTrue];
                let opciones = CFDictionaryCreate(
                    std::ptr::null(),
                    claves.as_ptr(),
                    valores.as_ptr(),
                    1,
                    &kCFTypeDictionaryKeyCallBacks as *const c_void,
                    &kCFTypeDictionaryValueCallBacks as *const c_void,
                );
                let lista = CGDisplayCopyAllDisplayModes(id, opciones);
                if !opciones.is_null() {
                    CFRelease(opciones);
                }
                Self(lista)
            }
        }

        /// Los modos que sirven para el escritorio (los demás son internos del sistema).
        fn usables(&self) -> impl Iterator<Item = Modo> + '_ {
            let n = if self.0.is_null() { 0 } else { unsafe { CFArrayGetCount(self.0) } };
            (0..n)
                .map(|i| unsafe { CFArrayGetValueAtIndex(self.0, i) })
                .filter(|m| !m.is_null() && unsafe { CGDisplayModeIsUsableForDesktopGUI(*m) } != 0)
        }
    }

    impl Drop for ListaModos {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0) };
            }
        }
    }

    fn leer_modo(m: Modo) -> ModoPantalla {
        let (ancho, alto, pixeles_ancho, pixeles_alto, hz) = unsafe {
            (
                CGDisplayModeGetWidth(m) as u32,
                CGDisplayModeGetHeight(m) as u32,
                CGDisplayModeGetPixelWidth(m) as u32,
                CGDisplayModeGetPixelHeight(m) as u32,
                CGDisplayModeGetRefreshRate(m),
            )
        };
        // Algunas pantallas (sobre todo las integradas antiguas y ciertos monitores) dicen 0 Hz: son 60.
        let hz = if hz > 0.0 { (hz * 100.0).round() / 100.0 } else { 60.0 };
        ModoPantalla {
            id: unsafe { CGDisplayModeGetIODisplayModeID(m) },
            ancho,
            alto,
            pixeles_ancho,
            pixeles_alto,
            hz,
            retina: pixeles_ancho > ancho,
            actual: false,
        }
    }

    /// Lo que distingue a un modo para saber si es el actual.
    fn firma(m: &ModoPantalla) -> (i32, u32, u32, u32) {
        (m.id, m.ancho, m.alto, m.pixeles_ancho)
    }

    /// Modos que se pueden elegir, sin repetidos: uno por (ancho, alto, Retina, Hz redondeados).
    /// Ordenados de mayor a menor ancho y, dentro de cada uno, de mayor a menor frecuencia.
    pub fn modos(id: u32) -> Vec<ModoPantalla> {
        let actual = unsafe {
            let m = CGDisplayCopyDisplayMode(id);
            if m.is_null() {
                None
            } else {
                let firma_actual = firma(&leer_modo(m));
                CGDisplayModeRelease(m);
                Some(firma_actual)
            }
        };

        let clave = |m: &ModoPantalla| (m.ancho, m.alto, m.retina, m.hz.round() as u32);
        let mut lista: Vec<ModoPantalla> = Vec::new();
        for m in ListaModos::de(id).usables() {
            let mut modo = leer_modo(m);
            modo.actual = Some(firma(&modo)) == actual;
            match lista.iter_mut().find(|x| clave(x) == clave(&modo)) {
                // Entre repetidos (60 y 59,94 Hz, por ejemplo) se queda el primero, salvo que el otro sea el actual.
                Some(x) if modo.actual => *x = modo,
                Some(_) => {}
                None => lista.push(modo),
            }
        }
        lista.sort_by(|a, b| {
            b.ancho
                .cmp(&a.ancho)
                .then(b.hz.total_cmp(&a.hz))
                .then(b.alto.cmp(&a.alto))
                .then(b.retina.cmp(&a.retina))
        });
        lista
    }

    /// Abre una configuración de pantallas, aplica `cambios` y la guarda para siempre (o la cancela si algo falla).
    fn configurar(cambios: impl FnOnce(Configuracion) -> Result<(), String>) -> Result<(), String> {
        let mut config: Configuracion = std::ptr::null_mut();
        if unsafe { CGBeginDisplayConfiguration(&mut config) } != 0 || config.is_null() {
            return Err("macOS no dejó cambiar las pantallas en este momento".into());
        }
        if let Err(e) = cambios(config) {
            unsafe { CGCancelDisplayConfiguration(config) };
            return Err(e);
        }
        match unsafe { CGCompleteDisplayConfiguration(config, PERMANENTE) } {
            0 => Ok(()),
            codigo => Err(format!("macOS no aceptó el cambio de pantalla (código {codigo}).")),
        }
    }

    /// Cambia la resolución y la frecuencia de una pantalla al modo `modo` (su ioDisplayModeID).
    pub fn cambiar_modo(id: u32, modo: i32) -> Result<(), String> {
        let lista = ListaModos::de(id);
        let elegido = lista
            .usables()
            .find(|m| unsafe { CGDisplayModeGetIODisplayModeID(*m) } == modo)
            .ok_or("Ese modo ya no está disponible en esta pantalla")?;
        if modos(id).iter().any(|m| m.actual && m.id == modo) {
            return Ok(()); // ya está en ese modo
        }
        // `lista` sigue viva hasta el final: `elegido` vive dentro de ella.
        configurar(|config| match unsafe { CGConfigureDisplayWithDisplayMode(config, id, elegido, std::ptr::null()) } {
            0 => Ok(()),
            codigo => Err(format!("La pantalla no aceptó ese modo (código {codigo}).")),
        })
    }

    /// Hace principal a una pantalla (la de la barra de menús). En macOS la principal es la que está
    /// en el origen (0, 0): se mueve esa al origen y las demás lo mismo, así la distribución no cambia.
    pub fn hacer_principal(id: u32) -> Result<(), String> {
        let activas = ids_activos();
        if !activas.contains(&id) {
            return Err("No encontré esa pantalla. ¿Se desconectó?".into());
        }
        if es_principal(id) {
            return Ok(());
        }
        if unsafe { CGDisplayMirrorsDisplay(id) } != 0 {
            return Err("Esta pantalla está duplicando otra; no puede ser la principal.".into());
        }
        let origen = unsafe { CGDisplayBounds(id) };
        configurar(|config| {
            for d in activas {
                // Las que duplican a otra siguen a su original; no tienen posición propia.
                if d != id && unsafe { CGDisplayMirrorsDisplay(d) } != 0 {
                    continue;
                }
                let b = unsafe { CGDisplayBounds(d) };
                let (x, y) = ((b.x - origen.x).round() as i32, (b.y - origen.y).round() as i32);
                if unsafe { CGConfigureDisplayOrigin(config, d, x, y) } != 0 {
                    return Err("macOS no dejó mover las pantallas".into());
                }
            }
            Ok(())
        })
    }
}
