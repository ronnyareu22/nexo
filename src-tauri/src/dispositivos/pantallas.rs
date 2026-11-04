//! Pantallas: la de la MacBook (DisplayServices) y los monitores externos (DDC/CI).
//!
//! Todo el brillo pasa por un solo hilo dueño de los monitores: los comandos DDC no se pisan
//! y varios toques seguidos de las teclas de brillo se juntan en un solo cambio.
//! La resolución, la frecuencia y cuál es la pantalla principal se cambian con CoreGraphics
//! (funciones sueltas al final: no tocan DDC, así que no pasan por ese hilo).
//! Windows tiene lo mismo en `pantallas_windows.rs` (DDC con dxva2, WMI y ChangeDisplaySettingsEx).
//!
//! Los monitores externos tienen un brillo mínimo (10 % si no se eligió otro): el 0 % de Nexo es ese mínimo,
//! así ningún camino (panel, teclas, menú, escenas, widgets) baja al nivel en que el monitor parpadea.
//! Los demás ajustes del monitor (contraste, color, entrada…) también pasan por el hilo: ver `vcp.rs`.

#[path = "vcp.rs"]
pub mod vcp;

use serde::{Deserialize, Serialize};
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

/// Cómo responde una pantalla a las teclas de brillo (se guarda por pantalla en los ajustes).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TeclasPantalla {
    /// Con el mouse encima, las teclas cambian esta pantalla. Si no, van a la del portátil (o la Mac).
    pub responde: bool,
    /// Cuánto cambia cada toque, en %: 3 (fino), 6 (normal, 1/16) o 10 (grande).
    pub paso: u8,
}

impl Default for TeclasPantalla {
    fn default() -> Self {
        TeclasPantalla { responde: true, paso: vcp::PASO_NORMAL }
    }
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

/// Respuesta de una orden al hilo de pantallas.
type Respuesta<T> = mpsc::Sender<Result<T, String>>;

enum Orden {
    Listar(mpsc::Sender<Vec<Pantalla>>),
    PonerBrillo(u32, u8, Respuesta<u8>),
    /// Toques de tecla: +1 sube, −1 baja, con el tamaño de paso de esa pantalla (%).
    #[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))] // lo usa el atajo de teclas (Mac y Windows)
    Paso(u32, i32, u8),
    Refrescar,
    /// Lee los ajustes del monitor (capacidades una vez; los valores, cada vez).
    Ajustes(u32, Respuesta<vcp::AjustesMonitor>),
    /// Cambia un ajuste del monitor (código VCP, valor).
    PonerVcp(u32, u8, u16, Respuesta<vcp::Valor>),
    /// Cambia el brillo mínimo (0–40 %) y vuelve a poner el brillo con él. Devuelve el nivel.
    Minimo(u32, u8, Respuesta<u8>),
}

/// Lo que se avisa desde el hilo de pantallas: (pantalla, nivel 0–100).
pub type Aviso = Box<dyn Fn(u32, u8) + Send>;

/// Avisos del hilo de pantallas.
pub struct Avisos {
    /// Tras cambiar el brillo con las teclas (el aviso en pantalla).
    pub tecla: Aviso,
    /// Tras cualquier cambio de nivel (panel, teclas, menú, escenas, widgets): los widgets de brillo lo siguen.
    pub cambio: Aviso,
}

/// Las órdenes que esperan, juntas: de lo que se repite seguido (arrastrar una barra) vale lo último
/// de cada pantalla o código, y los toques de teclas se suman.
struct Lote {
    minimos: HashMap<u32, (u8, Vec<Respuesta<u8>>)>,
    brillos: HashMap<u32, (u8, Vec<Respuesta<u8>>)>,
    vcp: HashMap<(u32, u8), (u16, Vec<Respuesta<vcp::Valor>>)>,
    /// Toques sumados de cada pantalla y su tamaño de paso.
    #[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
    pasos: HashMap<u32, (i32, u8)>,
    otras: Vec<Orden>,
}

impl Lote {
    fn juntar(primera: Orden, rx: &mpsc::Receiver<Orden>) -> Self {
        let mut l = Lote {
            minimos: HashMap::new(),
            brillos: HashMap::new(),
            vcp: HashMap::new(),
            pasos: HashMap::new(),
            otras: Vec::new(),
        };
        for orden in std::iter::once(primera).chain(rx.try_iter()) {
            match orden {
                Orden::Paso(id, n, tamano) => {
                    let e = l.pasos.entry(id).or_insert((0, tamano));
                    e.0 += n;
                    e.1 = tamano;
                }
                Orden::PonerBrillo(id, valor, t) => ultimo(&mut l.brillos, id, valor, t),
                Orden::Minimo(id, valor, t) => ultimo(&mut l.minimos, id, valor, t),
                Orden::PonerVcp(id, codigo, valor, t) => ultimo(&mut l.vcp, (id, codigo), valor, t),
                otra => l.otras.push(otra),
            }
        }
        l
    }
}

fn ultimo<K: std::hash::Hash + Eq, V, T>(mapa: &mut HashMap<K, (V, Vec<Respuesta<T>>)>, clave: K, valor: V, t: Respuesta<T>) {
    match mapa.get_mut(&clave) {
        Some(e) => {
            e.0 = valor;
            e.1.push(t);
        }
        None => {
            mapa.insert(clave, (valor, vec![t]));
        }
    }
}

fn responder<T: Clone>(respuestas: Vec<Respuesta<T>>, r: &Result<T, String>) {
    for t in respuestas {
        let _ = t.send(r.clone());
    }
}

pub struct Pantallas {
    tx: Mutex<mpsc::Sender<Orden>>,
    /// Monitores externos que responden a DDC (lo consulta el atajo de teclas sin esperar).
    /// En Windows también la pantalla del portátil, si WMI deja cambiarla.
    #[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
    controlables: Arc<Mutex<HashSet<u32>>>,
    /// Último brillo conocido de cada monitor externo (para leerlo al instante).
    /// En Windows también el de la pantalla del portátil (WMI tampoco se lee al instante).
    brillos_externos: Arc<Mutex<HashMap<u32, u8>>>,
    /// Cómo responde cada pantalla a las teclas (sin dato: responde, paso normal).
    teclas: Mutex<HashMap<u32, TeclasPantalla>>,
}

impl Pantallas {
    /// `minimos`: el brillo mínimo guardado de cada monitor externo (los que no están usan el 10 %).
    /// `teclas`: cómo responde cada pantalla a las teclas de brillo.
    pub fn iniciar(avisos: Avisos, minimos: HashMap<u32, u8>, teclas: HashMap<u32, TeclasPantalla>) -> Self {
        let (tx, rx) = mpsc::channel();
        let controlables = Arc::new(Mutex::new(HashSet::new()));
        let brillos_externos = Arc::new(Mutex::new(HashMap::new()));
        let (c, b) = (controlables.clone(), brillos_externos.clone());
        // Si el hilo no se crea, Nexo abre igual: sin él, las pantallas salen vacías y el brillo dice que falló.
        let hilo = std::thread::Builder::new()
            .name("nexo-pantallas".into())
            .spawn(move || trabajador(rx, c, b, avisos, minimos));
        if let Err(e) = hilo {
            eprintln!("Nexo: no se pudo iniciar el hilo de pantallas ({e})");
        }
        let p = Self { tx: Mutex::new(tx), controlables, brillos_externos, teclas: Mutex::new(teclas) };
        p.enviar(Orden::Refrescar);
        p
    }

    fn enviar(&self, orden: Orden) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(orden);
        }
    }

    /// Manda una orden y espera su respuesta.
    fn pedir<T>(&self, orden: impl FnOnce(Respuesta<T>) -> Orden) -> Result<T, String> {
        let (t, r) = mpsc::channel();
        self.enviar(orden(t));
        r.recv().map_err(|_| "Las pantallas no respondieron.".to_string())?
    }

    pub fn listar(&self) -> Vec<Pantalla> {
        let (t, r) = mpsc::channel();
        self.enviar(Orden::Listar(t));
        r.recv().unwrap_or_default()
    }

    /// El brillo en la escala de Nexo (0–100). En los monitores externos el 0 es su brillo mínimo.
    pub fn poner_brillo(&self, id: u32, valor: u8) -> Result<u8, String> {
        self.pedir(|t| Orden::PonerBrillo(id, valor.min(100), t))
    }

    /// Lo usa el atajo de teclas: no espera respuesta. Cada toque mueve el paso elegido para esa pantalla.
    #[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
    pub fn paso(&self, id: u32, pasos: i32) {
        let tamano = self.teclas_de(id).paso;
        self.enviar(Orden::Paso(id, pasos, tamano));
    }

    /// Cómo responde una pantalla a las teclas de brillo.
    pub fn teclas_de(&self, id: u32) -> TeclasPantalla {
        let t = self.teclas.lock().ok().and_then(|m| m.get(&id).copied()).unwrap_or_default();
        TeclasPantalla { paso: vcp::paso_valido(t.paso), ..t }
    }

    /// Cambia cómo responde una pantalla a las teclas (quien llama lo guarda en los ajustes).
    pub fn poner_teclas(&self, id: u32, t: TeclasPantalla) {
        if let Ok(mut m) = self.teclas.lock() {
            m.insert(id, TeclasPantalla { paso: vcp::paso_valido(t.paso), ..t });
        }
    }

    #[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
    pub fn refrescar(&self) {
        self.enviar(Orden::Refrescar);
    }

    /// Ajustes del monitor: el brillo mínimo, lo que el monitor tiene y sus valores (los lee por DDC/CI).
    pub fn ajustes_monitor(&self, id: u32) -> Result<vcp::AjustesMonitor, String> {
        self.pedir(|t| Orden::Ajustes(id, t))
    }

    /// Cambia un ajuste del monitor por DDC/CI. Devuelve cómo quedó.
    pub fn poner_vcp(&self, id: u32, codigo: u8, valor: u16) -> Result<vcp::Valor, String> {
        self.pedir(|t| Orden::PonerVcp(id, codigo, valor, t))
    }

    /// Cambia el brillo mínimo (0–40 %) de un monitor externo. Devuelve el nivel en la escala de Nexo.
    pub fn poner_minimo(&self, id: u32, minimo: u8) -> Result<u8, String> {
        self.pedir(|t| Orden::Minimo(id, minimo.min(vcp::MINIMO_TOPE), t))
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

    #[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
    pub fn es_controlable(&self, id: u32) -> bool {
        self.controlables.lock().map(|c| c.contains(&id)).unwrap_or(false)
    }
}

// ---------------------------------------------------------------- Hilo de pantallas

/// Lo que cada sistema sabe hacer con sus pantallas. Solo lo usa el hilo de pantallas.
trait Sistema {
    /// Mira qué pantallas hay (DDC se abre solo en las nuevas).
    fn refrescar(&mut self);
    /// Mira qué pantallas hay y las da en orden.
    fn listar(&mut self) -> Vec<Pantalla>;
    /// Brillo en la escala de Nexo. Devuelve el nivel final.
    fn poner(&mut self, id: u32, valor: u8) -> Result<u8, String>;
    /// Sube o baja `pasos` toques de `tamano` % (6 es 1/16). Devuelve el nivel final.
    fn paso(&mut self, id: u32, pasos: i32, tamano: u8) -> Option<u8>;
    /// Nuevo brillo mínimo: el nivel de Nexo se mantiene y el monitor se pone con el mínimo nuevo.
    fn poner_minimo(&mut self, id: u32, minimo: u8) -> Result<u8, String>;
    fn ajustes(&mut self, id: u32) -> Result<vcp::AjustesMonitor, String>;
    fn poner_vcp(&mut self, id: u32, codigo: u8, valor: u16) -> Result<vcp::Valor, String>;
    /// Las pantallas que se pueden cambiar y su nivel (la caché que se lee al instante).
    fn niveles(&self) -> HashMap<u32, u8>;
}

/// El brillo mínimo guardado de un monitor externo, o el de fábrica (10 %).
#[cfg(any(target_os = "macos", windows))]
fn minimo_en(minimos: &HashMap<u32, u8>, id: u32) -> u8 {
    minimos.get(&id).copied().unwrap_or(vcp::MINIMO_EXTERNO).min(vcp::MINIMO_TOPE)
}

#[cfg(any(target_os = "macos", windows))]
const NO_RESPONDE: &str = "Este monitor no responde. Activa DDC/CI en su menú.";

/// Atiende las órdenes en lotes. Primero los mínimos y el brillo, luego los ajustes del monitor,
/// después listar, refrescar o leer ajustes (así leen lo recién cambiado) y al final las teclas.
fn atender(
    rx: mpsc::Receiver<Orden>,
    mut s: impl Sistema,
    controlables: Arc<Mutex<HashSet<u32>>>,
    brillos: Arc<Mutex<HashMap<u32, u8>>>,
    avisos: Avisos,
) {
    let publicar = |niveles: HashMap<u32, u8>| {
        if let Ok(mut c) = controlables.lock() {
            *c = niveles.keys().copied().collect();
        }
        if let Ok(mut b) = brillos.lock() {
            *b = niveles;
        }
    };

    while let Ok(primera) = rx.recv() {
        let lote = Lote::juntar(primera, &rx);

        for (id, (minimo, respuestas)) in lote.minimos {
            let r = s.poner_minimo(id, minimo);
            publicar(s.niveles());
            if let Ok(nivel) = r {
                (avisos.cambio)(id, nivel);
            }
            responder(respuestas, &r);
        }
        for (id, (valor, respuestas)) in lote.brillos {
            let r = s.poner(id, valor);
            publicar(s.niveles());
            if let Ok(nivel) = r {
                (avisos.cambio)(id, nivel);
            }
            responder(respuestas, &r);
        }
        for ((id, codigo), (valor, respuestas)) in lote.vcp {
            let r = s.poner_vcp(id, codigo, valor);
            // Restaurar los valores de fábrica también cambia el brillo.
            if codigo == vcp::FABRICA && r.is_ok() {
                let niveles = s.niveles();
                if let Some(nivel) = niveles.get(&id) {
                    (avisos.cambio)(id, *nivel);
                }
                publicar(niveles);
            }
            responder(respuestas, &r);
        }

        // Varias listas o refrescos en cola: basta con mirar las pantallas una vez.
        let pide_lista = lote.otras.iter().any(|o| matches!(o, Orden::Listar(_)));
        let mut vistas: Option<Vec<Pantalla>> = None;
        for orden in lote.otras {
            match orden {
                Orden::Refrescar | Orden::Listar(_) => {
                    if vistas.is_none() {
                        vistas = Some(if pide_lista { s.listar() } else { s.refrescar(); Vec::new() });
                        publicar(s.niveles());
                    }
                    if let Orden::Listar(t) = orden {
                        let _ = t.send(vistas.clone().unwrap_or_default());
                    }
                }
                Orden::Ajustes(id, t) => {
                    let _ = t.send(s.ajustes(id));
                }
                Orden::PonerBrillo(..) | Orden::Paso(..) | Orden::PonerVcp(..) | Orden::Minimo(..) => {}
            }
        }

        for (id, (n, tamano)) in lote.pasos {
            if n == 0 {
                continue;
            }
            if let Some(nivel) = s.paso(id, n, tamano) {
                publicar(s.niveles());
                (avisos.tecla)(id, nivel);
                (avisos.cambio)(id, nivel);
            }
        }
    }
}

// ---------------------------------------------------------------- Resolución y pantalla principal

#[cfg(target_os = "macos")]
pub use mac::{cambiar_modo, hacer_principal, limites, modos};

// Windows: todo en su propio archivo, con la misma interfaz.
#[cfg(windows)]
#[path = "pantallas_windows.rs"]
mod win;

#[cfg(windows)]
pub use win::{
    area_de_trabajo, cambiar_modo, hacer_principal, hay_integrada, id_bajo_el_cursor, id_integrada, limites, modos,
    vigilar_portatil,
};

#[cfg(windows)]
fn trabajador(
    rx: mpsc::Receiver<Orden>,
    controlables: Arc<Mutex<HashSet<u32>>>,
    brillos: Arc<Mutex<HashMap<u32, u8>>>,
    avisos: Avisos,
    minimos: HashMap<u32, u8>,
) {
    win::trabajador(rx, controlables, brillos, avisos, minimos);
}

#[cfg(target_os = "macos")]
fn trabajador(
    rx: mpsc::Receiver<Orden>,
    controlables: Arc<Mutex<HashSet<u32>>>,
    brillos: Arc<Mutex<HashMap<u32, u8>>>,
    avisos: Avisos,
    minimos: HashMap<u32, u8>,
) {
    atender(rx, mac::Estado::nuevo(minimos), controlables, brillos, avisos);
}

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
pub fn limites(_id: u32) -> Option<(f64, f64, f64, f64)> {
    None
}

/// Otros sistemas: sin pantallas.
#[cfg(not(any(target_os = "macos", windows)))]
struct Nada;

#[cfg(not(any(target_os = "macos", windows)))]
impl Sistema for Nada {
    fn refrescar(&mut self) {}
    fn listar(&mut self) -> Vec<Pantalla> {
        Vec::new()
    }
    fn poner(&mut self, _id: u32, _valor: u8) -> Result<u8, String> {
        Err("Todavía no disponible en este sistema".into())
    }
    fn paso(&mut self, _id: u32, _pasos: i32, _tamano: u8) -> Option<u8> {
        None
    }
    fn poner_minimo(&mut self, _id: u32, _minimo: u8) -> Result<u8, String> {
        Err("Todavía no disponible en este sistema".into())
    }
    fn ajustes(&mut self, _id: u32) -> Result<vcp::AjustesMonitor, String> {
        Err("Todavía no disponible en este sistema".into())
    }
    fn poner_vcp(&mut self, _id: u32, _codigo: u8, _valor: u16) -> Result<vcp::Valor, String> {
        Err("Todavía no disponible en este sistema".into())
    }
    fn niveles(&self) -> HashMap<u32, u8> {
        HashMap::new()
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn trabajador(
    rx: mpsc::Receiver<Orden>,
    controlables: Arc<Mutex<HashSet<u32>>>,
    brillos: Arc<Mutex<HashMap<u32, u8>>>,
    avisos: Avisos,
    _minimos: HashMap<u32, u8>,
) {
    atender(rx, Nada, controlables, brillos, avisos);
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{ModoPantalla, NO_RESPONDE, Pantalla, Sistema, minimo_en, vcp};
    use ddc::Ddc;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::ffi::{CString, c_void};
    use std::sync::OnceLock;
    use std::time::Duration;

    const BRILLO_VCP: u8 = vcp::BRILLO;

    pub struct Externo {
        monitor: ddc_macos::Monitor,
        nombre: String,
        maximo: u16,
        actual: u16,
        /// Brillo mínimo (0–40 %): el 0 % de Nexo.
        minimo: u8,
    }

    impl Externo {
        fn porcentaje(&self) -> u8 {
            vcp::a_porcentaje(self.actual as u32, self.minimo, self.maximo as u32)
        }

        /// Escribe el brillo en unidades del monitor, nunca por debajo de su mínimo. Devuelve el nivel de Nexo.
        fn escribir(&mut self, crudo: u32) -> Result<u8, String> {
            let piso = vcp::piso(self.minimo, self.maximo as u32);
            let crudo = crudo.clamp(piso, self.maximo as u32) as u16;
            if !escribir_vcp(&mut self.monitor, BRILLO_VCP, crudo) {
                return Err("El monitor no aceptó el cambio. Revisa que DDC/CI siga activo en su menú.".into());
            }
            self.actual = crudo;
            Ok(self.porcentaje())
        }
    }

    /// Lee un código VCP (valor actual y máximo), con un reintento si el monitor no contesta a tiempo.
    fn leer_vcp(m: &mut ddc_macos::Monitor, codigo: u8) -> Option<(u32, u32)> {
        for intento in 0..2 {
            if let Ok(v) = m.get_vcp_feature(codigo) {
                return Some((v.value() as u32, v.maximum() as u32));
            }
            if intento == 0 {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        None
    }

    fn escribir_vcp(m: &mut ddc_macos::Monitor, codigo: u8, valor: u16) -> bool {
        for intento in 0..2 {
            if m.set_vcp_feature(codigo, valor).is_ok() {
                return true;
            }
            if intento == 0 {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        false
    }

    fn capacidades(m: &mut ddc_macos::Monitor) -> Option<String> {
        for intento in 0..2 {
            if let Ok(t) = m.capabilities_string() {
                return Some(String::from_utf8_lossy(&t).into_owned());
            }
            if intento == 0 {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        None
    }

    /// Lo que es del hilo de pantallas: los monitores externos abiertos, lo leído de cada uno y sus mínimos.
    pub struct Estado {
        externos: HashMap<u32, Externo>,
        memorias: HashMap<u32, vcp::Memoria>,
        minimos: HashMap<u32, u8>,
    }

    impl Estado {
        pub fn nuevo(minimos: HashMap<u32, u8>) -> Self {
            Estado { externos: HashMap::new(), memorias: HashMap::new(), minimos }
        }

        /// Busca monitores externos nuevos (y olvida los desconectados). Solo lee DDC de los nuevos.
        fn buscar(&mut self) {
            let activos: Vec<u32> = ids_activos().into_iter().filter(|id| !es_interna(*id)).collect();
            self.externos.retain(|id, _| activos.contains(id));
            self.memorias.retain(|id, _| activos.contains(id));
            if activos.iter().all(|id| self.externos.contains_key(id)) {
                return;
            }
            let Ok(monitores) = ddc_macos::Monitor::enumerate() else { return };
            for mut monitor in monitores {
                let id = monitor.handle().id;
                if !activos.contains(&id) || self.externos.contains_key(&id) {
                    continue;
                }
                // Si no responde a DDC (o tiene DDC/CI apagado en su menú), no es controlable.
                let Ok(valor) = monitor.get_vcp_feature(BRILLO_VCP) else { continue };
                let maximo = valor.maximum().max(1);
                let minimo = minimo_en(&self.minimos, id);
                let mut actual = valor.value().min(maximo);
                // Si quedó por debajo de su mínimo (ahí parpadea), sube al mínimo.
                let piso = vcp::piso(minimo, maximo as u32) as u16;
                if actual < piso && escribir_vcp(&mut monitor, BRILLO_VCP, piso) {
                    actual = piso;
                }
                let nombre = monitor.product_name().unwrap_or_else(|| "Monitor externo".into());
                self.externos.insert(id, Externo { nombre, maximo, actual, minimo, monitor });
            }
        }

        fn externo(&mut self, id: u32) -> Result<&mut Externo, String> {
            if es_interna(id) {
                return Err("La pantalla integrada no tiene estos ajustes.".into());
            }
            self.externos.get_mut(&id).ok_or_else(|| NO_RESPONDE.to_string())
        }
    }

    impl Sistema for Estado {
        fn refrescar(&mut self) {
            self.buscar();
        }

        fn listar(&mut self) -> Vec<Pantalla> {
            self.buscar();
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
                    } else if let Some(e) = self.externos.get(&id) {
                        Pantalla {
                            id,
                            nombre: e.nombre.clone(),
                            interna: false,
                            brillo: Some(e.porcentaje()),
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

        fn poner(&mut self, id: u32, valor: u8) -> Result<u8, String> {
            if es_interna(id) {
                let f = display_services().poner.ok_or("No se puede cambiar el brillo de esta pantalla")?;
                return if unsafe { f(id, valor as f32 / 100.0) } == 0 {
                    Ok(valor)
                } else {
                    Err("macOS no aceptó el cambio de brillo".into())
                };
            }
            let e = self.externos.get_mut(&id).ok_or(NO_RESPONDE)?;
            e.escribir(vcp::a_crudo(valor, e.minimo, e.maximo as u32))
        }

        fn paso(&mut self, id: u32, pasos: i32, tamano: u8) -> Option<u8> {
            let e = self.externos.get_mut(&id)?;
            let nuevo = vcp::paso(e.porcentaje(), pasos, tamano);
            e.escribir(vcp::a_crudo(nuevo, e.minimo, e.maximo as u32)).ok()
        }

        fn poner_minimo(&mut self, id: u32, minimo: u8) -> Result<u8, String> {
            let minimo = minimo.min(vcp::MINIMO_TOPE);
            self.minimos.insert(id, minimo);
            let e = self.externo(id)?;
            let nivel = e.porcentaje();
            e.minimo = minimo;
            e.escribir(vcp::a_crudo(nivel, minimo, e.maximo as u32))
        }

        fn ajustes(&mut self, id: u32) -> Result<vcp::AjustesMonitor, String> {
            let minimo = minimo_en(&self.minimos, id);
            let e = self.externos.get_mut(&id).ok_or(NO_RESPONDE)?;
            let memoria = self.memorias.entry(id).or_default();
            // Las dos lecturas usan el mismo monitor, una después de la otra.
            let monitor = RefCell::new(&mut e.monitor);
            Ok(vcp::leer_ajustes(
                memoria,
                minimo,
                || capacidades(&mut **monitor.borrow_mut()),
                |codigo| leer_vcp(&mut **monitor.borrow_mut(), codigo),
            ))
        }

        fn poner_vcp(&mut self, id: u32, codigo: u8, valor: u16) -> Result<vcp::Valor, String> {
            self.externo(id)?;
            // La primera vez se lee qué tiene el monitor (para no mandar algo que no ofrece).
            if self.memorias.get(&id).is_none_or(|m| m.lista.is_none()) {
                self.ajustes(id)?;
            }
            let memoria = self.memorias.get(&id).ok_or(NO_RESPONDE)?;
            let crudo = vcp::validar(memoria, codigo, valor)?;
            let maximo = memoria.maximos.get(&codigo).copied().unwrap_or(0);
            let e = self.externo(id)?;
            if !escribir_vcp(&mut e.monitor, codigo, crudo as u16) {
                return Err("El monitor no aceptó el cambio. Revisa que DDC/CI siga activo en su menú.".into());
            }
            if codigo == vcp::FABRICA {
                // El monitor tarda un momento en volver a sus valores; después se relee el brillo.
                std::thread::sleep(Duration::from_millis(500));
                if let Some((actual, max)) = leer_vcp(&mut e.monitor, BRILLO_VCP) {
                    e.maximo = (max.max(1)).min(u16::MAX as u32) as u16;
                    e.actual = actual.min(e.maximo as u32) as u16;
                }
            }
            Ok(vcp::Valor { actual: crudo as u16, maximo: maximo.min(u16::MAX as u32) as u16 })
        }

        fn niveles(&self) -> HashMap<u32, u8> {
            self.externos.iter().map(|(id, e)| (*id, e.porcentaje())).collect()
        }
    }

    /// Dónde está una pantalla en el escritorio, en puntos (origen arriba a la izquierda de la principal).
    pub fn limites(id: u32) -> Option<(f64, f64, f64, f64)> {
        if !ids_activos().contains(&id) {
            return None;
        }
        let r = unsafe { CGDisplayBounds(id) };
        (r.ancho > 0.0 && r.alto > 0.0).then_some((r.x, r.y, r.ancho, r.alto))
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
