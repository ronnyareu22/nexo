//! El centro de la casa: lo que se sabe de cada cuenta, cómo se lee, cómo se cumple una orden y cómo se busca.
//!
//! - Memoria: la lista de Tuya (y lo que acepta cada aparato, también en `casa.json`), las entidades de Home
//!   Assistant, el estado de lo local y lo encontrado en la última búsqueda. Las claves se leen del llavero una
//!   vez y quedan solo en memoria.
//! - Se lee a pedido (como mucho cada 15 s por cuenta); tras una orden, el estado se ajusta enseguida con lo que se
//!   mandó (sin volver a preguntar).
//! - Varias órdenes a la vez van en paralelo (una por aparato) y se cuentan en palabras.

use super::modelo::{
    Accion, Cambio, Capacidades, Clase, Destino, EstadoAparato, Hallazgo, Manejo, Objetivo, Orden, Proveedor, Via, cambio_de,
    normalizar,
};
use super::{
    Aparato, AjustesHogar, CambioAparato, CuentaHa, CuentaTuya, DatosConexionHa, DatosConexionTuya, DatosTuya, EstadoCasa,
    Falla, Grupo, HallazgoVista, Habitacion, Local, ResultadoBusqueda, ResultadoOrden, ble, ejemplo, ha, lan, llavero, tuya,
};
use crate::sistema::ajustes;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

/// Cuánto se recuerda lo leído de cada cuenta.
const RECUERDO: Duration = Duration::from_secs(15);
/// La lista de Tuya (aparatos nuevos, nombres) se vuelve a pedir cada 10 min.
const LISTA: Duration = Duration::from_secs(600);
/// Las áreas de Home Assistant, cada 5 min.
const AREAS: Duration = Duration::from_secs(300);
const BUSQUEDA: Duration = Duration::from_secs(6);
const MAX_NOMBRE: usize = 40;
const MAX_GRUPOS: usize = 30;
const MAX_HABITACIONES: usize = 30;
pub const PRONTO: &str = "Detectado · pronto podrás manejarlo desde Nexo.";

/// Un aparato de Tuya con lo que acepta (se guarda en `casa.json`).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct DeTuya {
    aparato: tuya::DispositivoTuya,
    puntos: tuya::Puntos,
}

/// `casa.json`: la lista de Tuya de esa cuenta (sin secretos).
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Archivo {
    cuenta: String,
    fuente: tuya::Fuente,
    tuya: Vec<DeTuya>,
}

struct Memoria {
    cargada: bool,
    cuenta_tuya: String,
    tuya: Vec<DeTuya>,
    fuente: tuya::Fuente,
    tuya_estado: Option<Instant>,
    tuya_lista: Option<Instant>,
    error_tuya: Option<String>,
    ha: Vec<ha::Entidad>,
    ha_areas: BTreeMap<String, (String, Vec<String>)>,
    ha_leido: Option<Instant>,
    ha_areas_leidas: Option<Instant>,
    error_ha: Option<String>,
    /// Lo local: lo último leído o mandado, y si respondió.
    locales: BTreeMap<String, (EstadoAparato, bool)>,
    locales_leido: Option<Instant>,
    /// (Access ID, secreto) y (dirección, token), leídos del llavero una vez.
    secreto_tuya: Option<(String, String)>,
    token_ha: Option<(String, String)>,
    hallazgos: Vec<Hallazgo>,
    actualizado: Option<u64>,
}

static MEMORIA: Mutex<Memoria> = Mutex::new(Memoria {
    cargada: false,
    cuenta_tuya: String::new(),
    tuya: Vec::new(),
    fuente: tuya::Fuente::Asociados,
    tuya_estado: None,
    tuya_lista: None,
    error_tuya: None,
    ha: Vec::new(),
    ha_areas: BTreeMap::new(),
    ha_leido: None,
    ha_areas_leidas: None,
    error_ha: None,
    locales: BTreeMap::new(),
    locales_leido: None,
    secreto_tuya: None,
    token_ha: None,
    hallazgos: Vec::new(),
    actualizado: None,
});
static LEYENDO: AtomicBool = AtomicBool::new(false);
static BUSCANDO: AtomicBool = AtomicBool::new(false);
/// Un cambio de ajustes a la vez (leer, cambiar, guardar).
static CAMBIANDO: Mutex<()> = Mutex::new(());

fn memoria() -> MutexGuard<'static, Memoria> {
    MEMORIA.lock().unwrap_or_else(|e| e.into_inner())
}

fn segundos() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn viejo(t: Option<Instant>, plazo: Duration) -> bool {
    t.is_none_or(|t| t.elapsed() >= plazo)
}

/// Baja la marca al terminar, pase lo que pase.
struct Suelta(&'static AtomicBool);

impl Drop for Suelta {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------- Ajustes y archivo

fn hogar(app: &AppHandle) -> AjustesHogar {
    ajustes::cargar(app).hogar
}

/// Cambia `ajustes.json › hogar` en un solo paso.
fn cambiar_hogar<T>(app: &AppHandle, f: impl FnOnce(&mut AjustesHogar) -> Result<T, String>) -> Result<T, String> {
    let _turno = CAMBIANDO.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = None;
    let mut fallo = None;
    ajustes::cambiar(app, |a| match f(&mut a.hogar) {
        Ok(v) => r = Some(v),
        Err(e) => fallo = Some(e),
    })?;
    match (r, fallo) {
        (Some(v), _) => Ok(v),
        (_, Some(e)) => Err(e),
        _ => Err("No se pudo guardar.".into()),
    }
}

fn ruta(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("casa.json"))
}

fn guardar_archivo(app: &AppHandle) {
    let Some(r) = ruta(app) else { return };
    let archivo = {
        let m = memoria();
        Archivo { cuenta: m.cuenta_tuya.clone(), fuente: m.fuente, tuya: m.tuya.clone() }
    };
    if let (Some(c), Ok(json)) = (r.parent(), serde_json::to_vec_pretty(&archivo)) {
        let _ = std::fs::create_dir_all(c);
        let temporal = r.with_extension("json.tmp");
        if std::fs::write(&temporal, json).is_ok() {
            let _ = std::fs::rename(&temporal, &r);
        }
    }
}

/// La primera vez: lo guardado en `casa.json` (o la casa de ejemplo).
fn cargar(app: &AppHandle) {
    let mut m = memoria();
    if m.cargada {
        return;
    }
    m.cargada = true;
    if ejemplo::activo() {
        m.cuenta_tuya = "ejemplo".into();
        m.tuya = ejemplo::tuya()
            .into_iter()
            .map(|(d, f)| DeTuya { puntos: tuya::puntos(&f, &d.crudo), aparato: d })
            .collect();
        m.ha = ejemplo::home_assistant();
        m.tuya_estado = Some(Instant::now());
        m.tuya_lista = Some(Instant::now());
        m.ha_leido = Some(Instant::now());
        m.actualizado = Some(segundos());
        return;
    }
    let leido = ruta(app).and_then(|r| std::fs::read(r).ok()).and_then(|b| serde_json::from_slice::<Archivo>(&b).ok());
    if let Some(a) = leido {
        m.cuenta_tuya = a.cuenta;
        m.fuente = a.fuente;
        m.tuya = a.tuya;
    }
}

// ---------------------------------------------------------------- Claves

fn credenciales_tuya(a: &AjustesHogar) -> Result<Option<tuya::Credenciales>, String> {
    let Some(c) = &a.tuya else { return Ok(None) };
    let guardado = memoria().secreto_tuya.clone().filter(|(id, _)| *id == c.access_id);
    let secreto = match guardado {
        Some((_, s)) => s,
        None => {
            let s = llavero::leer(llavero::Clave::Tuya, &c.access_id)?
                .ok_or("No encuentro la clave de Tuya en el llavero. Conecta tu cuenta de nuevo en Ajustes › Casa.")?;
            memoria().secreto_tuya = Some((c.access_id.clone(), s.clone()));
            s
        }
    };
    Ok(Some(tuya::Credenciales { id: c.access_id.clone(), secreto, region: c.region.clone() }))
}

fn conexion_ha(a: &AjustesHogar) -> Result<Option<ha::Conexion>, String> {
    let Some(c) = &a.home_assistant else { return Ok(None) };
    let guardado = memoria().token_ha.clone().filter(|(url, _)| *url == c.url);
    let token = match guardado {
        Some((_, t)) => t,
        None => {
            let t = llavero::leer(llavero::Clave::HomeAssistant, &c.url)?
                .ok_or("No encuentro el token de Home Assistant en el llavero. Conéctalo de nuevo en Ajustes › Casa.")?;
            memoria().token_ha = Some((c.url.clone(), t.clone()));
            t
        }
    };
    Ok(Some(ha::Conexion { url: c.url.clone(), token }))
}

// ---------------------------------------------------------------- Leer cada cuenta

/// Lo que acepta cada aparato nuevo (una llamada por aparato, en paralelo de a 4). Lo conocido se reutiliza.
fn con_puntos(c: &tuya::Credenciales, lista: Vec<tuya::DispositivoTuya>, conocidos: &[DeTuya]) -> Vec<DeTuya> {
    let mut salida: Vec<DeTuya> = Vec::with_capacity(lista.len());
    let mut nuevos = Vec::new();
    for d in lista {
        match conocidos.iter().find(|k| k.aparato.id == d.id && k.puntos != tuya::Puntos::default()) {
            Some(k) => salida.push(DeTuya { puntos: k.puntos.clone(), aparato: d }),
            None => nuevos.push(d),
        }
    }
    for tanda in nuevos.chunks(4) {
        let hechos: Vec<DeTuya> = std::thread::scope(|s| {
            let hilos: Vec<_> = tanda
                .iter()
                .map(|d| {
                    s.spawn(move || {
                        let funciones = tuya::funciones(c, &d.id).unwrap_or_default();
                        DeTuya { puntos: tuya::puntos(&funciones, &d.crudo), aparato: d.clone() }
                    })
                })
                .collect();
            hilos.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        salida.extend(hechos);
    }
    salida
}

fn leer_tuya(app: &AppHandle, a: &AjustesHogar) {
    if ejemplo::activo() {
        return;
    }
    let c = match credenciales_tuya(a) {
        Ok(Some(c)) => c,
        Ok(None) => {
            let mut m = memoria();
            m.tuya.clear();
            m.error_tuya = None;
            return;
        }
        Err(e) => {
            memoria().error_tuya = Some(e);
            return;
        }
    };
    let (conocidos, fuente, lista_vieja, otra_cuenta) = {
        let m = memoria();
        (m.tuya.clone(), m.fuente, viejo(m.tuya_lista, LISTA), m.cuenta_tuya != c.id)
    };
    // Con la API de la cuenta vinculada, una llamada trae la lista y el estado. Si no, el estado aparte.
    let r = if fuente == tuya::Fuente::Asociados || lista_vieja || otra_cuenta || conocidos.is_empty() {
        tuya::listar(&c).map(|(l, f)| (Some(l), f))
    } else {
        let ids: Vec<String> = conocidos.iter().map(|d| d.aparato.id.clone()).collect();
        tuya::estados(&c, &ids).map(|estados| {
            let mut m = memoria();
            for (id, crudo) in estados {
                if let Some(d) = m.tuya.iter_mut().find(|d| d.aparato.id == id) {
                    d.aparato.crudo = crudo;
                }
            }
            (None, fuente)
        })
    };
    match r {
        Ok((Some(lista), f)) => {
            let nuevos = lista.iter().any(|d| !conocidos.iter().any(|k| k.aparato.id == d.id));
            let completa = con_puntos(&c, lista, &conocidos);
            {
                let mut m = memoria();
                m.tuya = completa;
                m.fuente = f;
                m.cuenta_tuya = c.id.clone();
                m.tuya_lista = Some(Instant::now());
                m.tuya_estado = Some(Instant::now());
                m.error_tuya = None;
            }
            if nuevos || otra_cuenta {
                guardar_archivo(app);
            }
        }
        Ok((None, _)) => {
            let mut m = memoria();
            m.tuya_estado = Some(Instant::now());
            m.error_tuya = None;
        }
        Err(e) => {
            let mut m = memoria();
            m.tuya_estado = Some(Instant::now());
            m.error_tuya = Some(e.texto);
        }
    }
}

fn leer_ha(a: &AjustesHogar) {
    if ejemplo::activo() {
        return;
    }
    let c = match conexion_ha(a) {
        Ok(Some(c)) => c,
        Ok(None) => {
            let mut m = memoria();
            m.ha.clear();
            m.error_ha = None;
            return;
        }
        Err(e) => {
            memoria().error_ha = Some(e);
            return;
        }
    };
    let areas_viejas = viejo(memoria().ha_areas_leidas, AREAS);
    let r = ha::estados(&c);
    let areas = if areas_viejas && r.is_ok() { Some(ha::areas(&c)) } else { None };
    let mut m = memoria();
    m.ha_leido = Some(Instant::now());
    match r {
        Ok(estados) => {
            if let Some(a) = areas {
                m.ha_areas = a;
                m.ha_areas_leidas = Some(Instant::now());
            }
            m.ha = ha::entidades(&estados, &m.ha_areas);
            m.error_ha = None;
        }
        Err(e) => m.error_ha = Some(e),
    }
}

/// Lo local que sabe decir cómo está (WiZ, Yeelight, Govee), en paralelo. Bluetooth no: se recuerda lo mandado.
fn leer_locales(a: &AjustesHogar) {
    if ejemplo::activo() {
        return;
    }
    let lista: Vec<&Local> = a.locales.iter().filter(|l| !l.proveedor.por_bluetooth() && l.capacidades.informa_estado).collect();
    let leidos: Vec<(String, Option<EstadoAparato>)> = std::thread::scope(|s| {
        let hilos: Vec<_> = lista
            .iter()
            .map(|l| s.spawn(move || (l.id.clone(), lan::leer(l.proveedor, &l.direccion, l.modelo.as_deref()).ok())))
            .collect();
        hilos.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let mut m = memoria();
    for (id, e) in leidos {
        match e {
            Some(e) => {
                m.locales.insert(id, (e, true));
            }
            None => {
                let antes = m.locales.get(&id).map(|x| x.0).unwrap_or_default();
                m.locales.insert(id, (antes, false));
            }
        }
    }
    m.locales_leido = Some(Instant::now());
}

/// Lee lo que haga falta (en paralelo) y arma la casa.
pub fn estado(app: &AppHandle, fresco: bool) -> EstadoCasa {
    cargar(app);
    let a = hogar(app);
    let nunca = {
        let m = memoria();
        m.actualizado.is_none()
    };
    if (fresco || nunca) && !LEYENDO.swap(true, Ordering::SeqCst) {
        let _suelta = Suelta(&LEYENDO);
        let (t, h, l) = {
            let m = memoria();
            (viejo(m.tuya_estado, RECUERDO), viejo(m.ha_leido, RECUERDO), viejo(m.locales_leido, RECUERDO))
        };
        std::thread::scope(|s| {
            if t && a.tuya.is_some() {
                s.spawn(|| leer_tuya(app, &a));
            }
            if h && a.home_assistant.is_some() {
                s.spawn(|| leer_ha(&a));
            }
            if l && !a.locales.is_empty() {
                s.spawn(|| leer_locales(&a));
            }
        });
        memoria().actualizado = Some(segundos());
    }
    armar(&a)
}

// ---------------------------------------------------------------- Armar la casa

const HABITACIONES: [(&str, &str); 5] =
    [("sala", "Sala"), ("cuarto", "Cuarto"), ("oficina", "Oficina"), ("cocina", "Cocina"), ("escritorio", "Escritorio")];

fn habitaciones_base(a: &AjustesHogar) -> Vec<Habitacion> {
    a.habitaciones.clone().unwrap_or_else(|| {
        HABITACIONES.iter().map(|(id, n)| Habitacion { id: (*id).into(), nombre: (*n).into() }).collect()
    })
}

fn id_de_area(area: &str) -> String {
    format!("area-{}", normalizar(area).replace(' ', "-"))
}

/// Lo que va después de "tuya:", "wiz:"… en forma comparable (sin ":" ni "-").
fn clave_directa(id: &str) -> String {
    id.split_once(':').map(|(_, r)| r).unwrap_or(id).to_lowercase().replace([':', '-'], "")
}

fn orden_clase(c: Clase) -> u8 {
    match c {
        Clase::Luz => 0,
        Clase::Enchufe => 1,
        Clase::Tv => 2,
        Clase::Clima => 3,
        Clase::Cortina => 4,
        Clase::Cerradura => 5,
        Clase::SensorPuerta => 6,
        Clase::Otro => 7,
    }
}

fn de_tuya(t: &DeTuya) -> Aparato {
    let p = &t.puntos;
    let clase = tuya::clase_de(&t.aparato.categoria, p);
    let controlable = Proveedor::Tuya.clases().contains(&clase) && p.encendido.is_some();
    let mut capacidades = if controlable { p.capacidades() } else { Capacidades::ninguna() };
    match clase {
        Clase::Cerradura => capacidades.requiere_confirmacion = true,
        Clase::SensorPuerta => {
            capacidades.solo_lectura = true;
            capacidades.informa_estado = true;
        }
        _ => {}
    }
    let producto = t.aparato.producto.clone().unwrap_or_default();
    Aparato {
        id: format!("tuya:{}", t.aparato.id),
        proveedor: Proveedor::Tuya,
        via: Via::Nube,
        nombre: t.aparato.nombre.clone(),
        nombre_original: t.aparato.nombre.clone(),
        alias: None,
        clase,
        forma: (clase == Clase::Luz).then(|| tuya::forma_de(&t.aparato.categoria, &t.aparato.nombre, &producto)),
        habitacion: None,
        marca: Some("Tuya / Smart Life".into()),
        modelo: t.aparato.producto.clone(),
        direccion: None,
        en_linea: t.aparato.en_linea,
        controlable,
        capacidades,
        estado: tuya::estado(p, &t.aparato.crudo),
        oculto: false,
        entradas: Vec::new(),
        entrada: None,
        nota: (!controlable && clase != Clase::SensorPuerta).then(|| PRONTO.into()),
    }
}

fn tiene_acciones(c: &Capacidades) -> bool {
    c.encender || c.abrir || c.volumen || c.posicion
}

fn de_ha(e: &ha::Entidad) -> Aparato {
    let controlable = tiene_acciones(&e.capacidades) && !e.oculto;
    Aparato {
        id: format!("ha:{}", e.id),
        proveedor: Proveedor::HomeAssistant,
        via: Via::Wifi,
        nombre: e.nombre.clone(),
        nombre_original: e.nombre.clone(),
        alias: None,
        clase: e.clase,
        forma: e.forma,
        habitacion: None,
        marca: Some("Home Assistant".into()),
        modelo: None,
        direccion: None,
        en_linea: e.en_linea,
        controlable,
        capacidades: e.capacidades,
        estado: e.estado,
        oculto: e.oculto,
        entradas: e.entradas.clone(),
        entrada: e.entrada.clone(),
        nota: (!controlable && !e.capacidades.solo_lectura && !e.oculto).then(|| PRONTO.into()),
    }
}

fn de_local(l: &Local, leido: Option<&(EstadoAparato, bool)>) -> Aparato {
    let (estado, responde) = leido.copied().unwrap_or((EstadoAparato::default(), true));
    Aparato {
        id: l.id.clone(),
        proveedor: l.proveedor,
        via: if l.proveedor.por_bluetooth() { Via::Bluetooth } else { Via::Wifi },
        nombre: l.nombre.clone(),
        nombre_original: l.nombre.clone(),
        alias: None,
        clase: l.clase,
        forma: l.forma,
        habitacion: None,
        marca: Some(l.marca.clone()).filter(|m| !m.is_empty()),
        modelo: l.modelo.clone(),
        direccion: Some(l.direccion.clone()),
        en_linea: responde,
        controlable: true,
        capacidades: l.capacidades,
        estado,
        oculto: false,
        entradas: Vec::new(),
        entrada: None,
        nota: None,
    }
}

/// Junta todas las cuentas en una sola casa: sin repetidos, con nombres, habitaciones y en orden.
fn armar(a: &AjustesHogar) -> EstadoCasa {
    let m = memoria();
    let mut aparatos: Vec<Aparato> = m.tuya.iter().map(de_tuya).collect();
    aparatos.extend(a.locales.iter().map(|l| de_local(l, m.locales.get(&l.id))));
    // Lo de Home Assistant que Nexo ya maneja directo (por la cuenta de Tuya o la red local) no se repite.
    let directos: BTreeSet<String> = aparatos.iter().map(|x| clave_directa(&x.id)).collect();
    let areas: BTreeMap<String, String> =
        m.ha.iter().filter_map(|e| e.area.clone().map(|ar| (format!("ha:{}", e.id), ar))).collect();
    aparatos.extend(m.ha.iter().filter(|e| !e.enlaces.iter().any(|x| directos.contains(x))).map(de_ha));

    // Habitaciones: las de la persona y las áreas de Home Assistant que no estén.
    let mut habitaciones = habitaciones_base(a);
    for area in ha::nombres_de_areas(&m.ha) {
        if !habitaciones.iter().any(|h| normalizar(&h.nombre) == normalizar(&area)) {
            habitaciones.push(Habitacion { id: id_de_area(&area), nombre: area });
        }
    }
    let ejemplo = ejemplo::activo();
    for x in &mut aparatos {
        let pref = a.aparatos.get(&x.id).cloned().unwrap_or_default();
        if let Some(alias) = pref.alias.filter(|s| !s.trim().is_empty()) {
            x.nombre = alias.clone();
            x.alias = Some(alias);
        }
        if x.clase == Clase::Luz && pref.forma.is_some() {
            x.forma = pref.forma;
        }
        x.habitacion = match pref.habitacion {
            Some(h) if h.is_empty() => None,
            Some(h) if habitaciones.iter().any(|x| x.id == h) => Some(h),
            _ => areas
                .get(&x.id)
                .and_then(|ar| habitaciones.iter().find(|h| normalizar(&h.nombre) == normalizar(ar)))
                .map(|h| h.id.clone())
                .or_else(|| ejemplo.then(|| ejemplo::habitacion_de(&x.id).map(str::to_string)).flatten()),
        };
    }
    let ocultos = aparatos.iter().filter(|x| x.oculto).count();
    if !a.mostrar_todo {
        aparatos.retain(|x| !x.oculto);
    }
    aparatos.sort_by_cached_key(|x| (x.oculto, orden_clase(x.clase), normalizar(&x.nombre)));
    let existentes: BTreeSet<&str> = aparatos.iter().map(|x| x.id.as_str()).collect();
    let grupos = a
        .grupos
        .iter()
        .map(|g| Grupo { aparatos: g.aparatos.iter().filter(|i| existentes.contains(i.as_str())).cloned().collect(), ..g.clone() })
        .collect();
    let tuya = if ejemplo {
        Some(DatosTuya { access_id: "ejemplo".into(), region: "us".into(), region_nombre: "América (oeste)".into() })
    } else {
        a.tuya.as_ref().map(|c| DatosTuya {
            access_id: c.access_id.clone(),
            region: c.region.clone(),
            region_nombre: tuya::REGIONES.iter().find(|r| r.0 == c.region).map(|r| r.1.to_string()).unwrap_or_default(),
        })
    };
    let home_assistant = if ejemplo { Some(CuentaHa { url: "http://homeassistant.local:8123".into() }) } else { a.home_assistant.clone() };
    EstadoCasa {
        aparatos,
        habitaciones,
        grupos,
        tuya,
        home_assistant,
        error_tuya: m.error_tuya.clone(),
        error_ha: m.error_ha.clone(),
        actualizado: m.actualizado,
        mostrar_todo: a.mostrar_todo,
        ocultos,
        ejemplo,
        regiones: tuya::REGIONES.iter().map(|(k, n, _)| (k.to_string(), n.to_string())).collect(),
    }
}

// ---------------------------------------------------------------- A quién va una orden

const SOBRAN: &[&str] = &["la", "el", "las", "los", "de", "del", "en", "mi", "mis", "a", "al", "y", "oye", "nexo", "por", "favor"];

/// Las palabras que nombran una clase ("la tele", "las luces"). No se usan para buscar por nombre.
fn clase_de_palabra(p: &str) -> Option<Clase> {
    match p {
        "luz" | "luces" => Some(Clase::Luz),
        "enchufe" | "enchufes" => Some(Clase::Enchufe),
        "tele" | "teles" | "tv" | "television" | "televisor" | "televisores" => Some(Clase::Tv),
        "puerta" | "puertas" => Some(Clase::SensorPuerta),
        "cerradura" | "cerraduras" => Some(Clase::Cerradura),
        "cortina" | "cortinas" | "persiana" | "persianas" => Some(Clase::Cortina),
        "clima" | "aire" => Some(Clase::Clima),
        _ => None,
    }
}

fn palabras(texto: &str) -> Vec<String> {
    normalizar(texto)
        .split(' ')
        .filter(|p| !p.is_empty() && !SOBRAN.contains(p) && clase_de_palabra(p).is_none())
        .map(str::to_string)
        .collect()
}

fn es_de(x: &Aparato, c: Option<Clase>) -> bool {
    match c {
        None => true,
        // "La puerta" vale para el sensor y la cerradura.
        Some(Clase::SensorPuerta) => matches!(x.clase, Clase::SensorPuerta | Clase::Cerradura),
        Some(c) => x.clase == c,
    }
}

fn lista_de_nombres(n: &[String]) -> String {
    match n {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} y {b}"),
        [a, b, c] => format!("{a}, {b} y {c}"),
        varios => format!("{} aparatos", varios.len()),
    }
}

/// "La luz del escritorio", "la tira", "las luces de la sala", "la tele": a qué aparatos se refiere.
/// Primero un grupo, luego una habitación y luego un aparato con ese nombre; si no, el único que lo contiene.
pub(super) fn resolver_nombre(
    texto: &str,
    clase: Option<Clase>,
    aparatos: &[Aparato],
    grupos: &[Grupo],
    habitaciones: &[Habitacion],
) -> Result<Vec<String>, String> {
    let dicha = normalizar(texto).split(' ').find_map(clase_de_palabra);
    let clase = clase.or(dicha);
    let buscado = palabras(texto);
    let validos: Vec<&Aparato> = aparatos.iter().filter(|x| !x.oculto && es_de(x, clase)).collect();
    let ids = |v: Vec<&Aparato>| v.into_iter().map(|x| x.id.clone()).collect::<Vec<_>>();
    if buscado.is_empty() || matches!(buscado.as_slice(), [p] if p == "todo" || p == "todas" || p == "todos") {
        if validos.is_empty() {
            return Err(match clase {
                Some(c) => format!("No tienes ninguna {} en Nexo.", c.nombre()),
                None => "No hay aparatos en tu casa todavía.".into(),
            });
        }
        return Ok(ids(validos));
    }
    let mismo = |nombre: &str| palabras(nombre) == buscado;
    let contiene = |nombre: &str| {
        let p = palabras(nombre);
        buscado.iter().all(|b| p.contains(b))
    };
    let del_grupo = |g: &Grupo| validos.iter().filter(|x| g.aparatos.contains(&x.id)).copied().collect::<Vec<_>>();
    let de_habitacion = |h: &Habitacion| validos.iter().filter(|x| x.habitacion.as_deref() == Some(&h.id)).copied().collect::<Vec<_>>();
    for g in grupos.iter().filter(|g| mismo(&g.nombre)) {
        let v = del_grupo(g);
        if !v.is_empty() {
            return Ok(ids(v));
        }
    }
    for h in habitaciones.iter().filter(|h| mismo(&h.nombre)) {
        let v = de_habitacion(h);
        if !v.is_empty() {
            return Ok(ids(v));
        }
    }
    let exactos: Vec<&Aparato> = validos.iter().filter(|x| mismo(&x.nombre) || mismo(&x.nombre_original)).copied().collect();
    if !exactos.is_empty() {
        return Ok(ids(exactos));
    }
    let parecidos: Vec<&Aparato> = validos.iter().filter(|x| contiene(&x.nombre) || contiene(&x.nombre_original)).copied().collect();
    match parecidos.len() {
        1 => return Ok(ids(parecidos)),
        n if n > 1 => {
            let nombres: Vec<String> = parecidos.iter().map(|x| x.nombre.clone()).collect();
            return Err(format!("Hay {n} que se llaman parecido: {}. ¿Cuál?", lista_de_nombres(&nombres)));
        }
        _ => {}
    }
    for g in grupos.iter().filter(|g| contiene(&g.nombre)) {
        let v = del_grupo(g);
        if !v.is_empty() {
            return Ok(ids(v));
        }
    }
    for h in habitaciones.iter().filter(|h| contiene(&h.nombre)) {
        let v = de_habitacion(h);
        if !v.is_empty() {
            return Ok(ids(v));
        }
    }
    Err(format!("No encontré «{}» en tu casa.", texto.trim()))
}

fn resolver(destino: &Destino, e: &EstadoCasa) -> Result<Vec<String>, String> {
    let manejables = |x: &&Aparato| x.controlable && !x.oculto;
    match destino {
        Destino::Todos { clase } => {
            let v: Vec<String> = e.aparatos.iter().filter(manejables).filter(|x| es_de(x, *clase)).map(|x| x.id.clone()).collect();
            if v.is_empty() {
                Err(match clase {
                    Some(c) => format!("No tienes ninguna {} que Nexo maneje.", c.nombre()),
                    None => "Todavía no hay aparatos que Nexo maneje.".into(),
                })
            } else {
                Ok(v)
            }
        }
        Destino::Aparato { id } => e
            .aparatos
            .iter()
            .find(|x| x.id == *id)
            .map(|x| vec![x.id.clone()])
            .ok_or_else(|| "Ese aparato ya no está en tu casa.".into()),
        Destino::Grupo { id } => {
            let g = e.grupos.iter().find(|g| g.id == *id).ok_or("Ese grupo ya no existe.")?;
            if g.aparatos.is_empty() {
                return Err(format!("El grupo «{}» no tiene aparatos.", g.nombre));
            }
            Ok(g.aparatos.clone())
        }
        Destino::Habitacion { id, clase } => {
            let v: Vec<String> = e
                .aparatos
                .iter()
                .filter(manejables)
                .filter(|x| x.habitacion.as_deref() == Some(id) && es_de(x, *clase))
                .map(|x| x.id.clone())
                .collect();
            if v.is_empty() { Err("En esa habitación no hay aparatos que Nexo maneje.".into()) } else { Ok(v) }
        }
        Destino::Nombre { texto, clase } => resolver_nombre(texto, *clase, &e.aparatos, &e.grupos, &e.habitaciones),
    }
}

// ---------------------------------------------------------------- Cumplir una orden

/// Si un aparato entiende la acción (con "todas", los que no, se saltan sin error).
fn entiende(x: &Aparato, a: &Accion) -> bool {
    let c = &x.capacidades;
    if c.solo_lectura {
        return a.es_consulta();
    }
    match a {
        Accion::Encender | Accion::Apagar | Accion::Alternar => c.encender,
        Accion::Brillo { .. } | Accion::Temperatura { .. } | Accion::Color { .. } => {
            c.brillo || c.temperatura || c.color
        }
        Accion::Subir { .. } | Accion::Bajar { .. } => c.brillo || c.volumen,
        Accion::Abrir | Accion::Cerrar => c.abrir,
        Accion::Posicion { .. } => c.posicion,
        Accion::Volumen { .. } => c.volumen,
        Accion::Silenciar { .. } => c.silencio,
        Accion::Entrada { .. } => c.entradas,
        Accion::Consultar => true,
    }
}

/// El id de Tuya o de Home Assistant que va después del prefijo.
fn sin_prefijo<'a>(id: &'a str, prefijo: &str) -> &'a str {
    id.strip_prefix(prefijo).unwrap_or(id)
}

/// Manda un cambio (luces y enchufes) por el proveedor de cada aparato y ajusta lo recordado.
fn hacer_cambio(a: &AjustesHogar, x: &Aparato, cambio: &Cambio) -> Result<(), String> {
    if cambio.vacio() {
        return Ok(());
    }
    let ejemplo = ejemplo::activo();
    match x.proveedor {
        Proveedor::Tuya => {
            let id = sin_prefijo(&x.id, "tuya:").to_string();
            let puntos = memoria().tuya.iter().find(|d| d.aparato.id == id).map(|d| d.puntos.clone()).unwrap_or_default();
            let comandos = tuya::comandos(&puntos, &x.estado, cambio);
            if comandos.is_empty() {
                return Err(format!("{} no acepta ese cambio.", x.nombre));
            }
            if !ejemplo {
                let c = credenciales_tuya(a)?.ok_or("Conecta tu cuenta de Tuya en Ajustes › Casa.")?;
                tuya::enviar(&c, &id, &comandos).map_err(|e| tuya::con_nombre(&e, &x.nombre))?;
            }
            if let Some(d) = memoria().tuya.iter_mut().find(|d| d.aparato.id == id) {
                tuya::aplicar(&mut d.aparato.crudo, &comandos);
            }
            Ok(())
        }
        Proveedor::HomeAssistant => {
            let id = sin_prefijo(&x.id, "ha:").to_string();
            let entidad = memoria().ha.iter().find(|e| e.id == id).cloned().ok_or("Home Assistant ya no tiene ese aparato.")?;
            let (dominio, servicio, datos) = ha::servicio_de_cambio(&entidad, cambio)?;
            if !ejemplo {
                let c = conexion_ha(a)?.ok_or("Conecta Home Assistant en Ajustes › Casa.")?;
                ha::servicio(&c, &dominio, &servicio, &datos)?;
            }
            if let Some(e) = memoria().ha.iter_mut().find(|e| e.id == id) {
                cambio.aplicar(&mut e.estado);
            }
            Ok(())
        }
        Proveedor::Wiz | Proveedor::Yeelight | Proveedor::Govee | Proveedor::BleElk | Proveedor::BleTriones => {
            if !ejemplo {
                let direccion = x.direccion.clone().unwrap_or_default();
                if x.proveedor.por_bluetooth() {
                    ble::cambiar(x.proveedor, &direccion, cambio)?;
                } else {
                    lan::cambiar(x.proveedor, &direccion, x.modelo.as_deref(), cambio)?;
                }
            }
            let mut m = memoria();
            let e = m.locales.entry(x.id.clone()).or_insert((x.estado, true));
            cambio.aplicar(&mut e.0);
            e.1 = true;
            Ok(())
        }
        Proveedor::Hue | Proveedor::Lifx | Proveedor::Ninguno | Proveedor::Ejemplo => Err(format!("{} {}", x.nombre, PRONTO)),
    }
}

/// Lo que se ve enseguida en Home Assistant tras una acción que no es de luz (tele, cerradura, cortina).
fn ha_optimista(e: &mut ha::Entidad, accion: &Accion) {
    let s = &mut e.estado;
    match accion {
        Accion::Encender => s.encendido = Some(true),
        Accion::Apagar => s.encendido = Some(false),
        Accion::Alternar => s.encendido = Some(!s.encendido.unwrap_or(false)),
        Accion::Abrir => s.abierto = Some(true),
        Accion::Cerrar => s.abierto = Some(false),
        Accion::Posicion { valor } => {
            s.posicion = Some((*valor).min(100));
            s.abierto = Some(*valor > 0);
        }
        Accion::Volumen { valor } => s.volumen = Some((*valor).min(100)),
        Accion::Silenciar { activo } => s.silenciado = Some(*activo),
        Accion::Entrada { nombre } => e.entrada = Some(nombre.clone()),
        _ => {}
    }
}

/// Cumple una acción en un aparato.
fn hacer_accion(a: &AjustesHogar, x: &Aparato, accion: &Accion, confirmado: bool) -> Result<(), String> {
    if !x.controlable {
        return Err(x.nota.clone().map(|n| format!("{}: {n}", x.nombre)).unwrap_or_else(|| format!("{} solo informa.", x.nombre)));
    }
    if !x.en_linea && x.proveedor != Proveedor::HomeAssistant {
        return Err(format!("{} está sin conexión.", x.nombre));
    }
    // Home Assistant tiene su servicio para cada cosa (tele, cerradura, cortina); las luces van por el cambio.
    if x.proveedor == Proveedor::HomeAssistant && x.clase != Clase::Luz {
        let id = sin_prefijo(&x.id, "ha:").to_string();
        let entidad = memoria().ha.iter().find(|e| e.id == id).cloned().ok_or("Home Assistant ya no tiene ese aparato.")?;
        let (dominio, servicio, datos) = ha::servicio_de(&entidad, accion, confirmado)?;
        if !ejemplo::activo() {
            let c = conexion_ha(a)?.ok_or("Conecta Home Assistant en Ajustes › Casa.")?;
            ha::servicio(&c, &dominio, &servicio, &datos)?;
        }
        if let Some(e) = memoria().ha.iter_mut().find(|e| e.id == id) {
            ha_optimista(e, accion);
        }
        return Ok(());
    }
    let cambio = cambio_de(accion, &x.estado, &x.capacidades, &x.nombre)?;
    hacer_cambio(a, x, &cambio)
}

/// Cómo está un aparato, en palabras ("La puerta de entrada está cerrada.").
fn como_esta(x: &Aparato) -> String {
    let fem = matches!(x.clase, Clase::Luz | Clase::Tv | Clase::Cerradura | Clase::Cortina | Clase::SensorPuerta);
    let g = |m: &str, f: &str| if fem { f.to_string() } else { m.to_string() };
    let e = &x.estado;
    let texto = if let Some(ab) = e.abierto {
        if ab { g("abierto", "abierta") } else { g("cerrado", "cerrada") }
    } else if let Some(on) = e.encendido {
        match (on, e.brillo, e.grados) {
            (true, Some(b), _) if x.clase == Clase::Luz => format!("{} al {b} %", g("encendido", "encendida")),
            (true, _, Some(t)) => format!("{} y marca {t:.0} °C", g("encendido", "encendida")),
            (true, _, _) => g("encendido", "encendida"),
            (false, _, _) => g("apagado", "apagada"),
        }
    } else if let Some(t) = e.grados {
        format!("en {t:.0} °C")
    } else {
        "sin datos de ahora".into()
    };
    if x.en_linea { format!("{} está {texto}.", x.nombre) } else { format!("{} está sin conexión.", x.nombre) }
}

fn mensaje_hecho(accion: &Accion, nombres: &[String]) -> String {
    let l = lista_de_nombres(nombres);
    match accion {
        Accion::Encender => format!("Encendí {l}."),
        Accion::Apagar => format!("Apagué {l}."),
        Accion::Alternar => format!("Listo: {l}."),
        Accion::Brillo { valor } => format!("Puse {l} al {} %.", (*valor).clamp(1, 100)),
        Accion::Subir { .. } => format!("Subí {l}."),
        Accion::Bajar { .. } => format!("Bajé {l}."),
        Accion::Temperatura { .. } => format!("Cambié el blanco de {l}."),
        Accion::Color { .. } => format!("Cambié el color de {l}."),
        Accion::Abrir => format!("Abrí {l}."),
        Accion::Cerrar => format!("Cerré {l}."),
        Accion::Posicion { valor } => format!("Puse {l} al {} %.", (*valor).min(100)),
        Accion::Volumen { valor } => format!("Puse el volumen de {l} en {}.", (*valor).min(100)),
        Accion::Silenciar { activo: true } => format!("Silencié {l}."),
        Accion::Silenciar { activo: false } => format!("Le quité el silencio a {l}."),
        Accion::Entrada { nombre } => format!("Cambié {l} a {nombre}."),
        Accion::Consultar => String::new(),
    }
}

/// Cumple una orden (de la interfaz, una escena, el menú del ícono o la voz).
pub fn ejecutar(app: &AppHandle, orden: &Orden, avisar: bool) -> ResultadoOrden {
    cargar(app);
    let a = hogar(app);
    let casa = armar(&a);
    let fallo = |mensaje: String, casa: EstadoCasa| ResultadoOrden {
        ok: false,
        mensaje,
        hechos: Vec::new(),
        fallas: Vec::new(),
        requiere_confirmacion: false,
        estado: casa,
    };
    let ids = match resolver(&orden.destino, &casa) {
        Ok(v) => v,
        Err(e) => return fallo(e, casa),
    };
    let elegidos: Vec<Aparato> = casa.aparatos.iter().filter(|x| ids.contains(&x.id)).cloned().collect();
    if orden.accion.es_consulta() {
        let texto = elegidos.iter().map(como_esta).collect::<Vec<_>>().join(" ");
        return ResultadoOrden { ok: true, mensaje: texto, hechos: ids, fallas: Vec::new(), requiere_confirmacion: false, estado: casa };
    }
    // Con varios, solo los que entienden la acción; con uno solo, se dice por qué no.
    let elegidos: Vec<Aparato> = if elegidos.len() > 1 {
        elegidos.into_iter().filter(|x| entiende(x, &orden.accion) && x.controlable).collect()
    } else {
        elegidos
    };
    if elegidos.is_empty() {
        return fallo("Ninguno de esos aparatos hace eso.".into(), casa);
    }
    let delicados: Vec<String> = elegidos
        .iter()
        .filter(|x| x.capacidades.requiere_confirmacion && matches!(orden.accion, Accion::Abrir))
        .map(|x| x.nombre.clone())
        .collect();
    if !delicados.is_empty() && !orden.confirmado {
        return ResultadoOrden {
            ok: false,
            mensaje: format!("¿Abrir {}? Confírmalo en Nexo.", lista_de_nombres(&delicados)),
            hechos: Vec::new(),
            fallas: Vec::new(),
            requiere_confirmacion: true,
            estado: casa,
        };
    }
    let resultados: Vec<(Aparato, Result<(), String>)> = std::thread::scope(|s| {
        let hilos: Vec<_> = elegidos
            .iter()
            .map(|x| {
                let a = &a;
                s.spawn(move || (x.clone(), hacer_accion(a, x, &orden.accion, orden.confirmado)))
            })
            .collect();
        hilos.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let hechos: Vec<&Aparato> = resultados.iter().filter(|(_, r)| r.is_ok()).map(|(x, _)| x).collect();
    let fallas: Vec<Falla> = resultados
        .iter()
        .filter_map(|(x, r)| r.as_ref().err().map(|e| Falla { id: x.id.clone(), nombre: x.nombre.clone(), error: e.clone() }))
        .collect();
    let nombres: Vec<String> = hechos.iter().map(|x| x.nombre.clone()).collect();
    let mut mensaje = if nombres.is_empty() { String::new() } else { mensaje_hecho(&orden.accion, &nombres) };
    for f in &fallas {
        if !mensaje.is_empty() {
            mensaje.push(' ');
        }
        mensaje.push_str(&f.error);
    }
    if !hechos.is_empty() && avisar {
        let _ = app.emit("casa-cambio", ());
    }
    ResultadoOrden {
        ok: fallas.is_empty(),
        mensaje,
        hechos: hechos.iter().map(|x| x.id.clone()).collect(),
        fallas,
        requiere_confirmacion: false,
        estado: armar(&a),
    }
}

/// Un paso "luces" de una escena: lleva a cada aparato al estado pedido (lo que no sabe hacer se deja afuera).
pub async fn aplicar_objetivo(app: AppHandle, destino: Destino, objetivo: Objetivo) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        cargar(&app);
        let a = hogar(&app);
        if a.tuya.is_none() && a.home_assistant.is_none() && a.locales.is_empty() && !ejemplo::activo() {
            return Err("Conecta tus luces en Ajustes › Casa para que este paso corra.".into());
        }
        let casa = armar(&a);
        let ids = resolver(&destino, &casa)?;
        let elegidos: Vec<&Aparato> = casa.aparatos.iter().filter(|x| ids.contains(&x.id) && x.controlable).collect();
        let errores: Vec<String> = std::thread::scope(|s| {
            let hilos: Vec<_> = elegidos
                .iter()
                .map(|x| {
                    let a = &a;
                    s.spawn(move || {
                        if !x.en_linea && x.proveedor != Proveedor::HomeAssistant {
                            return Some(format!("{} está sin conexión.", x.nombre));
                        }
                        hacer_cambio(a, x, &objetivo.cambio(&x.capacidades)).err()
                    })
                })
                .collect();
            hilos.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
        });
        let _ = app.emit("casa-cambio", ());
        if errores.is_empty() { Ok(()) } else { Err(errores.join(" · ")) }
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- Buscar

/// Busca en la red y por Bluetooth a la vez (≈6 s); de paso, pone al día la lista de Tuya.
pub fn buscar(app: &AppHandle) -> Result<ResultadoBusqueda, String> {
    if BUSCANDO.swap(true, Ordering::SeqCst) {
        return Err("Ya estoy buscando; espera unos segundos.".into());
    }
    let _suelta = Suelta(&BUSCANDO);
    cargar(app);
    let inicio = Instant::now();
    let a = hogar(app);
    let mut avisos = Vec::new();
    let mut hallazgos = if ejemplo::activo() {
        std::thread::sleep(Duration::from_millis(1500));
        ejemplo::hallazgos()
    } else {
        let (red, bt) = std::thread::scope(|s| {
            let red = s.spawn(|| lan::buscar(BUSQUEDA));
            let bt = s.spawn(|| ble::buscar(BUSQUEDA));
            if a.tuya.is_some() {
                memoria().tuya_lista = None;
                s.spawn(|| leer_tuya(app, &a));
            }
            (red.join().unwrap_or_default(), bt.join().unwrap_or_else(|_| Ok(Vec::new())))
        });
        let mut v = red;
        match bt {
            Ok(l) => v.extend(l),
            Err(e) => avisos.push(e),
        }
        v
    };

    // Lo de Tuya visto en la red: si está en la cuenta, ya está en Nexo.
    let (nube, conectado_tuya): (BTreeMap<String, String>, bool) = {
        let m = memoria();
        (m.tuya.iter().map(|d| (format!("tuya:{}", d.aparato.id), d.aparato.nombre.clone())).collect(), a.tuya.is_some() || ejemplo::activo())
    };
    let locales: BTreeSet<&str> = a.locales.iter().map(|l| l.id.as_str()).collect();
    let mut vistas = Vec::with_capacity(hallazgos.len());
    for h in &mut hallazgos {
        let mut agregado = locales.contains(h.id.as_str());
        if h.proveedor == Proveedor::Tuya {
            if let Some(nombre) = nube.get(&h.id) {
                h.nombre = nombre.clone();
                h.manejo = Manejo::Listo;
                agregado = true;
            } else if conectado_tuya {
                h.manejo = Manejo::Pronto { motivo: "Está en tu red, pero no en la cuenta de Smart Life vinculada.".into() };
            }
        }
        if let Manejo::HomeAssistant { url } = &h.manejo {
            agregado = a.home_assistant.as_ref().is_some_and(|c| c.url == *url) || ejemplo::activo();
        }
        vistas.push(HallazgoVista { hallazgo: h.clone(), agregado });
    }
    // Si una luz local cambió de IP, se anota la nueva.
    let movidas: Vec<(String, String)> = hallazgos
        .iter()
        .filter_map(|h| {
            let l = a.locales.iter().find(|l| l.id == h.id)?;
            let d = h.direccion.clone()?;
            (d != l.direccion).then(|| (l.id.clone(), d))
        })
        .collect();
    if !movidas.is_empty() {
        let _ = cambiar_hogar(app, |hg| {
            for (id, d) in &movidas {
                if let Some(l) = hg.locales.iter_mut().find(|l| l.id == *id) {
                    l.direccion = d.clone();
                }
            }
            Ok(())
        });
    }
    {
        let mut m = memoria();
        for h in hallazgos.iter().filter(|h| h.proveedor.es_local()) {
            if let Some(e) = h.estado {
                m.locales.insert(h.id.clone(), (e, true));
            }
        }
        m.hallazgos = hallazgos;
    }
    Ok(ResultadoBusqueda { hallazgos: vistas, avisos, segundos: inicio.elapsed().as_secs_f32(), estado: armar(&hogar(app)) })
}

// ---------------------------------------------------------------- Cambios de la persona

fn limpiar_nombre(t: &str) -> String {
    t.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NOMBRE).collect::<String>().trim().to_string()
}

fn nuevo_id(prefijo: &str) -> String {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("{prefijo}{ms}")
}

/// Suma a Nexo algo encontrado (solo lo que se maneja directo: WiZ, Yeelight, Govee y Bluetooth).
pub fn agregar(app: &AppHandle, id: &str) -> Result<EstadoCasa, String> {
    let h = memoria().hallazgos.iter().find(|h| h.id == id).cloned().ok_or("Busca de nuevo: ya no lo veo en la lista.")?;
    if !h.proveedor.es_local() || h.manejo != Manejo::Listo {
        return Err(match h.manejo {
            Manejo::CuentaTuya => "Se maneja con tu cuenta de Tuya: conéctala en Ajustes › Casa.".into(),
            Manejo::HomeAssistant { .. } => "Conecta tu Home Assistant en Ajustes › Casa.".into(),
            Manejo::Activar { paso } => paso,
            Manejo::Pronto { motivo } => motivo,
            Manejo::Listo => PRONTO.into(),
        });
    }
    let direccion = h.direccion.clone().ok_or("No sé cómo llegar a ese aparato; busca de nuevo.")?;
    cambiar_hogar(app, |hg| {
        if hg.locales.iter().any(|l| l.id == h.id) {
            return Ok(());
        }
        hg.locales.push(Local {
            id: h.id.clone(),
            proveedor: h.proveedor,
            nombre: limpiar_nombre(&h.nombre),
            clase: h.clase,
            forma: h.forma,
            marca: h.marca.clone(),
            modelo: h.modelo.clone(),
            direccion,
            capacidades: h.capacidades,
        });
        Ok(())
    })?;
    if let Some(e) = h.estado {
        memoria().locales.insert(h.id.clone(), (e, true));
    }
    sumar_a_escenas(app);
    crate::sistema::bandeja::actualizar(app);
    Ok(armar(&hogar(app)))
}

pub fn quitar(app: &AppHandle, id: &str) -> Result<EstadoCasa, String> {
    cambiar_hogar(app, |hg| {
        let antes = hg.locales.len();
        hg.locales.retain(|l| l.id != id);
        if antes == hg.locales.len() {
            return Err("Solo se quita lo que agregaste desde una búsqueda.".into());
        }
        hg.aparatos.remove(id);
        for g in &mut hg.grupos {
            g.aparatos.retain(|x| x != id);
        }
        Ok(())
    })?;
    memoria().locales.remove(id);
    Ok(armar(&hogar(app)))
}

pub fn editar(app: &AppHandle, id: &str, cambio: CambioAparato) -> Result<EstadoCasa, String> {
    let casa = armar(&hogar(app));
    if !casa.aparatos.iter().any(|x| x.id == id) {
        return Err("Ese aparato ya no está en tu casa.".into());
    }
    let habitaciones: Vec<String> = casa.habitaciones.iter().map(|h| h.id.clone()).collect();
    cambiar_hogar(app, |hg| {
        let p = hg.aparatos.entry(id.to_string()).or_default();
        if let Some(alias) = cambio.alias {
            let limpio = limpiar_nombre(&alias);
            p.alias = (!limpio.is_empty()).then_some(limpio);
        }
        if let Some(h) = cambio.habitacion {
            if !h.is_empty() && !habitaciones.contains(&h) {
                return Err("Esa habitación ya no existe.".into());
            }
            p.habitacion = Some(h);
        }
        if cambio.forma.is_some() {
            p.forma = cambio.forma;
        }
        Ok(())
    })?;
    crate::sistema::bandeja::actualizar(app);
    Ok(armar(&hogar(app)))
}

pub fn guardar_grupo(app: &AppHandle, grupo: Grupo) -> Result<EstadoCasa, String> {
    let nombre = limpiar_nombre(&grupo.nombre);
    if nombre.is_empty() {
        return Err("Ponle un nombre al grupo.".into());
    }
    let mut aparatos: Vec<String> = Vec::new();
    for x in grupo.aparatos.into_iter().take(40) {
        if !aparatos.contains(&x) {
            aparatos.push(x);
        }
    }
    if aparatos.is_empty() {
        return Err("Elige al menos un aparato para el grupo.".into());
    }
    cambiar_hogar(app, |hg| {
        let limpio = Grupo { id: grupo.id.clone(), nombre, aparatos, habitacion: grupo.habitacion.clone().filter(|h| !h.is_empty()) };
        match hg.grupos.iter_mut().find(|g| !limpio.id.is_empty() && g.id == limpio.id) {
            Some(g) => *g = limpio,
            None => {
                if hg.grupos.len() >= MAX_GRUPOS {
                    return Err(format!("Puedes tener hasta {MAX_GRUPOS} grupos."));
                }
                hg.grupos.push(Grupo { id: nuevo_id("g"), ..limpio });
            }
        }
        Ok(())
    })?;
    crate::sistema::bandeja::actualizar(app);
    Ok(armar(&hogar(app)))
}

pub fn borrar_grupo(app: &AppHandle, id: &str) -> Result<EstadoCasa, String> {
    cambiar_hogar(app, |hg| {
        hg.grupos.retain(|g| g.id != id);
        Ok(())
    })?;
    crate::sistema::bandeja::actualizar(app);
    Ok(armar(&hogar(app)))
}

pub fn guardar_habitacion(app: &AppHandle, h: Habitacion) -> Result<EstadoCasa, String> {
    let nombre = limpiar_nombre(&h.nombre);
    if nombre.is_empty() {
        return Err("Ponle un nombre a la habitación.".into());
    }
    cambiar_hogar(app, |hg| {
        let mut lista = habitaciones_base(hg);
        if lista.iter().any(|x| x.id != h.id && normalizar(&x.nombre) == normalizar(&nombre)) {
            return Err(format!("Ya tienes una habitación «{nombre}»."));
        }
        match lista.iter_mut().find(|x| !h.id.is_empty() && x.id == h.id) {
            Some(x) => x.nombre = nombre,
            None => {
                if lista.len() >= MAX_HABITACIONES {
                    return Err(format!("Puedes tener hasta {MAX_HABITACIONES} habitaciones."));
                }
                // Una de Home Assistant que se nombra pasa a ser de la persona, con el mismo id.
                let id = if h.id.starts_with("area-") { h.id.clone() } else { nuevo_id("h") };
                lista.push(Habitacion { id, nombre });
            }
        }
        hg.habitaciones = Some(lista);
        Ok(())
    })?;
    Ok(armar(&hogar(app)))
}

pub fn borrar_habitacion(app: &AppHandle, id: &str) -> Result<EstadoCasa, String> {
    cambiar_hogar(app, |hg| {
        let mut lista = habitaciones_base(hg);
        lista.retain(|h| h.id != id);
        hg.habitaciones = Some(lista);
        for p in hg.aparatos.values_mut() {
            if p.habitacion.as_deref() == Some(id) {
                p.habitacion = Some(String::new());
            }
        }
        for g in &mut hg.grupos {
            if g.habitacion.as_deref() == Some(id) {
                g.habitacion = None;
            }
        }
        Ok(())
    })?;
    Ok(armar(&hogar(app)))
}

pub fn mostrar_todo(app: &AppHandle, activo: bool) -> Result<EstadoCasa, String> {
    cambiar_hogar(app, |hg| {
        hg.mostrar_todo = activo;
        Ok(())
    })?;
    Ok(armar(&hogar(app)))
}

// ---------------------------------------------------------------- Cuentas

fn texto_clave(t: &str, que: &str, min: usize, max: usize) -> Result<String, String> {
    let t = t.trim();
    if t.len() < min || t.len() > max || t.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!("Revisa el {que}: cópialo completo, sin espacios."));
    }
    Ok(t.to_string())
}

pub fn conectar_tuya(app: &AppHandle, datos: DatosConexionTuya) -> Result<EstadoCasa, String> {
    let id = texto_clave(&datos.access_id, "Access ID", 8, 64)?;
    let secreto = texto_clave(&datos.access_secret, "Access Secret", 8, 128)?;
    let (region, lista, fuente) = if datos.region == "auto" {
        tuya::detectar(&id, &secreto).map_err(|e| e.texto)?
    } else {
        if !tuya::region_valida(&datos.region) {
            return Err("Elige el centro de datos de tu cuenta.".into());
        }
        let c = tuya::Credenciales { id: id.clone(), secreto: secreto.clone(), region: datos.region.clone() };
        let (l, f) = tuya::probar(&c).map_err(|e| e.texto)?;
        (datos.region.clone(), l, f)
    };
    llavero::guardar(llavero::Clave::Tuya, &id, &secreto)?;
    let anterior = hogar(app).tuya;
    cambiar_hogar(app, |hg| {
        hg.tuya = Some(CuentaTuya { access_id: id.clone(), region: region.clone() });
        Ok(())
    })?;
    if let Some(a) = anterior.filter(|a| a.access_id != id) {
        let _ = llavero::borrar(llavero::Clave::Tuya, &a.access_id);
    }
    let c = tuya::Credenciales { id: id.clone(), secreto: secreto.clone(), region };
    let vacia = lista.is_empty();
    let conocidos = memoria().tuya.clone();
    let completa = con_puntos(&c, lista, &conocidos);
    {
        let mut m = memoria();
        m.secreto_tuya = Some((id.clone(), secreto));
        m.cuenta_tuya = id;
        m.tuya = completa;
        m.fuente = fuente;
        m.tuya_lista = Some(Instant::now());
        m.tuya_estado = Some(Instant::now());
        m.actualizado = Some(segundos());
        m.error_tuya = vacia.then(|| {
            "Conecté con Tuya, pero no veo aparatos. Revisa que vinculaste tu cuenta de Smart Life en el proyecto (Devices › Link Tuya App Account).".into()
        });
    }
    guardar_archivo(app);
    sumar_a_escenas(app);
    crate::sistema::bandeja::actualizar(app);
    Ok(armar(&hogar(app)))
}

pub fn desconectar_tuya(app: &AppHandle) -> Result<EstadoCasa, String> {
    if let Some(c) = hogar(app).tuya {
        llavero::borrar(llavero::Clave::Tuya, &c.access_id)?;
    }
    cambiar_hogar(app, |hg| {
        hg.tuya = None;
        Ok(())
    })?;
    tuya::olvidar_sesion();
    {
        let mut m = memoria();
        m.secreto_tuya = None;
        m.tuya.clear();
        m.cuenta_tuya.clear();
        m.error_tuya = None;
        m.tuya_lista = None;
        m.tuya_estado = None;
    }
    if let Some(r) = ruta(app) {
        let _ = std::fs::remove_file(r);
    }
    crate::sistema::bandeja::actualizar(app);
    Ok(armar(&hogar(app)))
}

pub fn conectar_ha(app: &AppHandle, datos: DatosConexionHa) -> Result<EstadoCasa, String> {
    let url = ha::url_limpia(&datos.url)
        .ok_or("Escribe la dirección de tu Home Assistant, por ejemplo http://homeassistant.local:8123.")?;
    let token = texto_clave(&datos.token, "token", 20, 2000)?;
    let c = ha::Conexion { url: url.clone(), token: token.clone() };
    ha::probar(&c)?;
    let estados = ha::estados(&c)?;
    let areas = ha::areas(&c);
    llavero::guardar(llavero::Clave::HomeAssistant, &url, &token)?;
    let anterior = hogar(app).home_assistant;
    cambiar_hogar(app, |hg| {
        hg.home_assistant = Some(CuentaHa { url: url.clone() });
        Ok(())
    })?;
    if let Some(a) = anterior.filter(|a| a.url != url) {
        let _ = llavero::borrar(llavero::Clave::HomeAssistant, &a.url);
    }
    {
        let mut m = memoria();
        m.token_ha = Some((url, token));
        m.ha = ha::entidades(&estados, &areas);
        m.ha_areas = areas;
        m.ha_leido = Some(Instant::now());
        m.ha_areas_leidas = Some(Instant::now());
        m.actualizado = Some(segundos());
        m.error_ha = None;
    }
    sumar_a_escenas(app);
    crate::sistema::bandeja::actualizar(app);
    Ok(armar(&hogar(app)))
}

pub fn desconectar_ha(app: &AppHandle) -> Result<EstadoCasa, String> {
    if let Some(c) = hogar(app).home_assistant {
        llavero::borrar(llavero::Clave::HomeAssistant, &c.url)?;
    }
    cambiar_hogar(app, |hg| {
        hg.home_assistant = None;
        Ok(())
    })?;
    {
        let mut m = memoria();
        m.token_ha = None;
        m.ha.clear();
        m.ha_areas.clear();
        m.error_ha = None;
        m.ha_leido = None;
        m.ha_areas_leidas = None;
    }
    crate::sistema::bandeja::actualizar(app);
    Ok(armar(&hogar(app)))
}

/// La primera vez que hay luces, se suman a las escenas de fábrica (una sola vez).
pub fn sumar_a_escenas(app: &AppHandle) {
    if hogar(app).escenas_listas {
        return;
    }
    if crate::sistema::escenas::sumar_luces(app).is_ok() {
        let _ = cambiar_hogar(app, |hg| {
            hg.escenas_listas = true;
            Ok(())
        });
    }
}

// ---------------------------------------------------------------- Menú del ícono

/// El submenú "Luces": `None` si no hay luces conectadas; si no, los grupos (id, nombre).
pub fn menu_bandeja(app: &AppHandle) -> Option<Vec<(String, String)>> {
    let a = hogar(app);
    if a.tuya.is_none() && a.home_assistant.is_none() && a.locales.is_empty() && !ejemplo::activo() {
        return None;
    }
    Some(a.grupos.iter().map(|g| (g.id.clone(), g.nombre.clone())).collect())
}

/// "encender:todas", "apagar:todas", "encender:g:<id>" o "apagar:g:<id>". Avisa solo si algo falló.
pub fn elegir_en_bandeja(app: &AppHandle, resto: &str) {
    let (que, destino) = resto.split_once(':').unwrap_or((resto, ""));
    let accion = match que {
        "encender" => Accion::Encender,
        "apagar" => Accion::Apagar,
        _ => return,
    };
    let destino = match destino.strip_prefix("g:") {
        Some(id) => Destino::Grupo { id: id.to_string() },
        None => Destino::Todos { clase: Some(Clase::Luz) },
    };
    let app = app.clone();
    let _ = std::thread::Builder::new().name("nexo-casa".into()).spawn(move || {
        let r = ejecutar(&app, &Orden { destino, accion, confirmado: false }, true);
        if !r.ok {
            use tauri_plugin_notification::NotificationExt;
            let _ = app.notification().builder().title("Luces").body(r.mensaje).show();
        }
    });
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ap(id: &str, nombre: &str, clase: Clase, habitacion: Option<&str>) -> Aparato {
        Aparato {
            id: id.into(),
            proveedor: Proveedor::Ejemplo,
            via: Via::Nube,
            nombre: nombre.into(),
            nombre_original: nombre.into(),
            alias: None,
            clase,
            forma: None,
            habitacion: habitacion.map(str::to_string),
            marca: None,
            modelo: None,
            direccion: None,
            en_linea: true,
            controlable: true,
            capacidades: Capacidades::luz(true, false, false),
            estado: EstadoAparato::default(),
            oculto: false,
            entradas: Vec::new(),
            entrada: None,
            nota: None,
        }
    }

    fn casa() -> (Vec<Aparato>, Vec<Grupo>, Vec<Habitacion>) {
        let aparatos = vec![
            ap("t", "Tira del escritorio", Clase::Luz, Some("escritorio")),
            ap("b", "Barra del monitor", Clase::Luz, Some("escritorio")),
            ap("f1", "Foco de la sala", Clase::Luz, Some("sala")),
            ap("f2", "Foco del cuarto", Clase::Luz, Some("cuarto")),
            ap("tv", "Tele de la sala", Clase::Tv, Some("sala")),
            ap("p", "Puerta de entrada", Clase::SensorPuerta, None),
        ];
        let grupos = vec![Grupo { id: "g1".into(), nombre: "Escritorio".into(), aparatos: vec!["t".into(), "b".into()], habitacion: None }];
        let habitaciones = vec![
            Habitacion { id: "sala".into(), nombre: "Sala".into() },
            Habitacion { id: "cuarto".into(), nombre: "Cuarto".into() },
        ];
        (aparatos, grupos, habitaciones)
    }

    fn r(texto: &str) -> Result<Vec<String>, String> {
        let (a, g, h) = casa();
        resolver_nombre(texto, None, &a, &g, &h)
    }

    #[test]
    fn hogar_nombres_para_la_voz() {
        // Un grupo por su nombre.
        assert_eq!(r("la luz del escritorio"), Ok(vec!["t".into(), "b".into()]));
        // Una habitación, solo las luces.
        assert_eq!(r("las luces de la sala"), Ok(vec!["f1".into()]));
        // Un aparato que lo contiene.
        assert_eq!(r("la tira"), Ok(vec!["t".into()]));
        assert_eq!(r("el foco del cuarto"), Ok(vec!["f2".into()]));
        assert_eq!(r("la tele"), Ok(vec!["tv".into()]));
        assert_eq!(r("la puerta"), Ok(vec!["p".into()]));
        // Todas las luces.
        assert_eq!(r("las luces").map(|v| v.len()), Ok(4));
        // Ambiguo o desconocido, en palabras.
        assert_eq!(r("el foco"), Err("Hay 2 que se llaman parecido: Foco de la sala y Foco del cuarto. ¿Cuál?".into()));
        assert_eq!(r("la licuadora"), Err("No encontré «la licuadora» en tu casa.".into()));
    }

    #[test]
    fn hogar_mensajes_en_palabras() {
        let n = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(mensaje_hecho(&Accion::Encender, &n(&["Tira del escritorio"])), "Encendí Tira del escritorio.");
        assert_eq!(mensaje_hecho(&Accion::Apagar, &n(&["A", "B", "C", "D"])), "Apagué 4 aparatos.");
        assert_eq!(mensaje_hecho(&Accion::Brillo { valor: 0 }, &n(&["A", "B"])), "Puse A y B al 1 %.");
        let mut p = ap("p", "La puerta de entrada", Clase::SensorPuerta, None);
        p.estado.abierto = Some(false);
        assert_eq!(como_esta(&p), "La puerta de entrada está cerrada.");
        let mut e = ap("e", "Enchufe", Clase::Enchufe, None);
        e.estado.encendido = Some(true);
        assert_eq!(como_esta(&e), "Enchufe está encendido.");
    }

    #[test]
    fn hogar_quien_entiende_que() {
        let mut sensor = ap("p", "Puerta", Clase::SensorPuerta, None);
        sensor.capacidades = Capacidades { solo_lectura: true, ..Capacidades::ninguna() };
        assert!(!entiende(&sensor, &Accion::Apagar));
        assert!(entiende(&sensor, &Accion::Consultar));
        let mut enchufe = ap("e", "Enchufe", Clase::Enchufe, None);
        enchufe.capacidades = Capacidades::enchufe();
        assert!(entiende(&enchufe, &Accion::Apagar));
        assert!(!entiende(&enchufe, &Accion::Brillo { valor: 20 }));
        assert_eq!(clave_directa("wiz:A8:BB:50:01:02:03"), "a8bb50010203");
        assert_eq!(clave_directa("tuya:bf12AB"), "bf12ab");
    }

    #[test]
    fn hogar_ejemplo_completo() {
        // La casa de ejemplo se arma con los mismos caminos que la real.
        let tuya: Vec<Aparato> = ejemplo::tuya()
            .into_iter()
            .map(|(d, f)| de_tuya(&DeTuya { puntos: tuya::puntos(&f, &d.crudo), aparato: d }))
            .collect();
        assert_eq!(tuya.len(), 5);
        let tira = &tuya[0];
        assert_eq!((tira.clase, tira.forma, tira.capacidades.color), (Clase::Luz, Some(modelo_forma::Tira), true));
        assert_eq!(tuya[1].forma, Some(modelo_forma::Barra));
        assert_eq!((tuya[4].clase, tuya[4].capacidades.brillo), (Clase::Enchufe, false));
        let ha: Vec<Aparato> = ejemplo::home_assistant().iter().map(de_ha).collect();
        assert!(ha.iter().any(|x| x.clase == Clase::Tv && x.controlable));
        assert!(ha.iter().any(|x| x.clase == Clase::Cerradura && x.capacidades.requiere_confirmacion));
        assert!(ha.iter().any(|x| x.clase == Clase::SensorPuerta && !x.controlable && x.nota.is_none()));
        assert!(ha.iter().any(|x| x.oculto));
        assert_eq!(ejemplo::hallazgos().len(), 7);
    }

    use super::super::modelo::FormaLuz as modelo_forma;
}
