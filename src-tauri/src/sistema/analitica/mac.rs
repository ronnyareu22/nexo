//! Mac: lo que mide Analítica, sin abrir procesos.
//! - CPU: los ticks de cada núcleo con `host_processor_info` (la diferencia entre dos lecturas).
//! - Memoria: `host_statistics64` (como el Monitor de Actividad), la swap y la presión con sysctl.
//! - Temperatura: el estado térmico de `NSProcessInfo` en palabras, y los grados de la batería (AppleSmartBattery).
//! - Batería: IOKit (`IOPSCopyPowerSourcesInfo`) y, cada minuto, los ciclos y la salud de AppleSmartBattery.
//! - Disco: statfs de "/". Red: getifaddrs (bytes de cada interfaz). Arranque: sysctl `kern.boottime`.
//! - Procesos: libproc (`PROC_PIDTASKINFO`), agrupados por app. macOS solo deja leer los del usuario.

use super::{Bateria, Cpu, Disco, Grupo, Memoria, Muestra, Procesos, Red, Temperatura, Tramo};
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PROCESSOR_CPU_LOAD_INFO: c_int = 2;
const CPU_STATE_MAX: usize = 4; // usuario, sistema, inactivo, nice
const CPU_STATE_IDLE: usize = 2;
const HOST_VM_INFO64: c_int = 4;
const NUMERO_I64: isize = 4; // kCFNumberSInt64Type
const UTF8: u32 = 0x0800_0100; // kCFStringEncodingUTF8
/// Los ciclos, la salud y los grados de la batería cambian despacio: se leen una vez por minuto.
const REGISTRO_CADA: Duration = Duration::from_secs(60);

#[repr(C)]
#[derive(Default)]
struct Timebase {
    numer: u32,
    denom: u32,
}

/// `vm_statistics64` hasta `total_uncompressed_pages_in_compressor` (la versión 1, la de todos los macOS).
#[repr(C)]
#[derive(Default)]
struct Vm {
    _libres: u32,
    _activas: u32,
    _inactivas: u32,
    cableadas: u32,
    _contadores: [u64; 9],
    purgables: u32,
    _especulativas: u32,
    _otros: [u64; 4],
    comprimidas: u32,
    _retenidas: u32,
    externas: u32,
    internas: u32,
    _sin_comprimir: u64,
}

/// `struct xsw_usage` de sysctl `vm.swapusage`.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Swap {
    total: u64,
    _libre: u64,
    usada: u64,
    _pagina: u32,
    _cifrada: i32,
}

/// `struct timeval` de sysctl `kern.boottime`.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Hora {
    segundos: i64,
    _micro: i32,
}

unsafe extern "C" {
    static mach_task_self_: u32;
    fn mach_host_self() -> u32;
    fn mach_timebase_info(info: *mut Timebase) -> c_int;
    fn host_processor_info(host: u32, tipo: c_int, cuantos: *mut u32, datos: *mut *mut c_int, largo: *mut u32) -> c_int;
    fn host_statistics64(host: u32, tipo: c_int, datos: *mut c_int, largo: *mut u32) -> c_int;
    fn vm_deallocate(tarea: u32, direccion: usize, tamano: usize) -> c_int;
    fn mach_port_deallocate(tarea: u32, puerto: u32) -> c_int;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPSCopyPowerSourcesInfo() -> *const c_void;
    fn IOPSCopyPowerSourcesList(info: *const c_void) -> *const c_void;
    fn IOPSGetPowerSourceDescription(info: *const c_void, fuente: *const c_void) -> *const c_void;
    fn IOServiceMatching(nombre: *const c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(puerto: u32, busqueda: *mut c_void) -> u32;
    fn IORegistryEntryCreateCFProperty(entrada: u32, clave: *const c_void, alloc: *const c_void, opciones: u32) -> *const c_void;
    fn IOObjectRelease(objeto: u32) -> c_int;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayGetCount(arreglo: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(arreglo: *const c_void, i: isize) -> *const c_void;
    fn CFDictionaryGetValue(diccionario: *const c_void, clave: *const c_void) -> *const c_void;
    fn CFNumberGetValue(numero: *const c_void, tipo: isize, valor: *mut c_void) -> u8;
    fn CFBooleanGetValue(booleano: *const c_void) -> u8;
    fn CFStringCreateWithCString(alloc: *const c_void, texto: *const c_char, codificacion: u32) -> *const c_void;
    fn CFStringGetCString(texto: *const c_void, buf: *mut c_char, tam: isize, codificacion: u32) -> u8;
    fn CFGetTypeID(objeto: *const c_void) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFRelease(objeto: *const c_void);
}

/// Un objeto de CoreFoundation propio (Create/Copy) que se libera al salir.
struct Cf(*const c_void);

impl Cf {
    fn nuevo(p: *const c_void) -> Option<Self> {
        (!p.is_null()).then_some(Cf(p))
    }

    fn texto(c: &CStr) -> Option<Self> {
        // SAFETY: `c` termina en cero; CF copia el texto.
        Cf::nuevo(unsafe { CFStringCreateWithCString(null(), c.as_ptr(), UTF8) })
    }
}

impl Drop for Cf {
    fn drop(&mut self) {
        // SAFETY: es un objeto CF propio y no nulo.
        unsafe { CFRelease(self.0) }
    }
}

/// Un número de CF (entero), si lo es.
fn entero_cf(v: *const c_void) -> Option<i64> {
    if v.is_null() {
        return None;
    }
    // SAFETY: `v` es un objeto CF vivo; se mira su tipo antes de leerlo.
    unsafe {
        if CFGetTypeID(v) != CFNumberGetTypeID() {
            return None;
        }
        let mut x = 0i64;
        (CFNumberGetValue(v, NUMERO_I64, &mut x as *mut i64 as *mut c_void) != 0).then_some(x)
    }
}

fn booleano_cf(v: *const c_void) -> Option<bool> {
    if v.is_null() {
        return None;
    }
    // SAFETY: `v` es un objeto CF vivo; se mira su tipo antes de leerlo.
    unsafe { (CFGetTypeID(v) == CFBooleanGetTypeID()).then(|| CFBooleanGetValue(v) != 0) }
}

fn texto_cf(v: *const c_void) -> Option<String> {
    if v.is_null() {
        return None;
    }
    let mut buf = [0 as c_char; 128];
    // SAFETY: `v` es un objeto CF vivo; el búfer mide lo que se dice.
    unsafe {
        if CFGetTypeID(v) != CFStringGetTypeID() || CFStringGetCString(v, buf.as_mut_ptr(), buf.len() as isize, UTF8) == 0 {
            return None;
        }
        Some(CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    }
}

/// El valor de una clave de un diccionario de CF (sin tomarlo: no hay que liberarlo).
fn valor(diccionario: *const c_void, clave: &CStr) -> *const c_void {
    match Cf::texto(clave) {
        // SAFETY: el diccionario está vivo mientras dure la lectura.
        Some(k) => unsafe { CFDictionaryGetValue(diccionario, k.0) },
        None => null(),
    }
}

/// Un valor de sysctl de tamaño fijo.
fn sysctl<T: Default + Copy>(nombre: &CStr) -> Option<T> {
    let mut v = T::default();
    let mut tam = std::mem::size_of::<T>();
    // SAFETY: el búfer mide `tam` bytes y sysctl dice cuántos escribió.
    let r = unsafe { libc::sysctlbyname(nombre.as_ptr(), &mut v as *mut T as *mut c_void, &mut tam, null_mut(), 0) };
    (r == 0 && tam == std::mem::size_of::<T>()).then_some(v)
}

/// Lo que se lee una vez por minuto de AppleSmartBattery.
#[derive(Clone, Copy, Default)]
struct Registro {
    ciclos: Option<u32>,
    salud: Option<u8>,
    grados: Option<f32>,
}

/// Lo que mide la Mac, con lo que hace falta recordar entre muestras.
pub struct Lector {
    host: u32,
    base: Timebase,
    pagina: u64,
    total: u64,
    nucleos: Vec<[u32; CPU_STATE_MAX]>,
    red: HashMap<String, (u32, u32)>,
    red_momento: Option<Instant>,
    /// Por cada proceso: el tiempo de CPU (ns) y su app.
    procesos: HashMap<i32, (u64, String)>,
    procesos_momento: Option<Instant>,
    registro: Option<(Instant, Registro)>,
}

impl Lector {
    pub fn nuevo() -> Self {
        let mut base = Timebase::default();
        // SAFETY: llena la estructura; sin datos, se usa 1/1.
        if unsafe { mach_timebase_info(&mut base) } != 0 || base.denom == 0 {
            base = Timebase { numer: 1, denom: 1 };
        }
        // SAFETY: sysconf solo lee.
        let pagina = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        Lector {
            // SAFETY: devuelve el puerto del equipo (se guarda uno solo para todas las lecturas).
            host: unsafe { mach_host_self() },
            base,
            pagina: if pagina > 0 { pagina as u64 } else { 16384 },
            total: sysctl::<u64>(c"hw.memsize").unwrap_or(0),
            nucleos: Vec::new(),
            red: HashMap::new(),
            red_momento: None,
            procesos: HashMap::new(),
            procesos_momento: None,
            registro: None,
        }
    }

    pub fn medir(&mut self) -> Muestra {
        let registro = self.registro();
        let (procesos, rustdesk) = self.procesos();
        Muestra {
            cpu: self.cpu(),
            memoria: self.memoria(),
            temperatura: Temperatura { estado: estado_termico().map(String::from), bateria: registro.grados },
            bateria: bateria(registro),
            disco: disco(),
            red: self.red(),
            encendida: encendida(),
            procesos,
            rustdesk,
        }
    }

    // ------------------------------------------------------------ CPU

    fn ticks(&self) -> Option<Vec<[u32; CPU_STATE_MAX]>> {
        let mut cuantos = 0u32;
        let mut datos: *mut c_int = null_mut();
        let mut largo = 0u32;
        // SAFETY: mach reserva el arreglo en esta tarea; se copia y se libera enseguida.
        unsafe {
            if host_processor_info(self.host, PROCESSOR_CPU_LOAD_INFO, &mut cuantos, &mut datos, &mut largo) != 0
                || datos.is_null()
            {
                return None;
            }
            let lista = std::slice::from_raw_parts(datos as *const u32, largo as usize);
            let ticks = lista
                .chunks_exact(CPU_STATE_MAX)
                .take(cuantos as usize)
                .map(|c| [c[0], c[1], c[2], c[3]])
                .collect();
            vm_deallocate(mach_task_self_, datos as usize, largo as usize * std::mem::size_of::<c_int>());
            Some(ticks)
        }
    }

    fn cpu(&mut self) -> Option<Cpu> {
        let ahora = self.ticks()?;
        let antes = std::mem::replace(&mut self.nucleos, ahora.clone());
        if antes.len() != ahora.len() {
            return None;
        }
        let (mut ocupado_total, mut todo_total) = (0u64, 0u64);
        let nucleos = ahora
            .iter()
            .zip(&antes)
            .map(|(a, b)| {
                let d: Vec<u64> = (0..CPU_STATE_MAX).map(|i| a[i].wrapping_sub(b[i]) as u64).collect();
                let todo: u64 = d.iter().sum();
                let ocupado = todo - d[CPU_STATE_IDLE];
                ocupado_total += ocupado;
                todo_total += todo;
                if todo == 0 { 0.0 } else { super::redondear(ocupado as f32 * 100.0 / todo as f32) }
            })
            .collect();
        let total = if todo_total == 0 { 0.0 } else { ocupado_total as f32 * 100.0 / todo_total as f32 };
        Some(Cpu { total: super::redondear(total), nucleos, cantidad: ahora.len() as u32 })
    }

    // ------------------------------------------------------------ Memoria

    fn memoria(&self) -> Option<Memoria> {
        let mut vm = Vm::default();
        let mut largo = (std::mem::size_of::<Vm>() / std::mem::size_of::<c_int>()) as u32;
        // SAFETY: la estructura mide `largo` enteros; mach no escribe más que eso.
        let r = unsafe { host_statistics64(self.host, HOST_VM_INFO64, &mut vm as *mut Vm as *mut c_int, &mut largo) };
        if r != 0 || self.total == 0 {
            return None;
        }
        let p = self.pagina;
        let apps = (vm.internas.saturating_sub(vm.purgables) as u64) * p;
        let cableada = vm.cableadas as u64 * p;
        let comprimida = vm.comprimidas as u64 * p;
        let cache = (vm.externas as u64 + vm.purgables as u64) * p;
        let usada = (apps + cableada + comprimida).min(self.total);
        let swap = sysctl::<Swap>(c"vm.swapusage");
        let presion = sysctl::<c_int>(c"kern.memorystatus_vm_pressure_level").map(|n| {
            match n {
                4 => "critica",
                2 => "aviso",
                _ => "normal",
            }
            .to_string()
        });
        Some(Memoria {
            total: self.total,
            usada,
            apps: Some(apps),
            cableada: Some(cableada),
            comprimida: Some(comprimida),
            cache: Some(cache),
            libre: self.total.saturating_sub(usada + cache),
            swap_usada: swap.map(|s| s.usada),
            swap_total: swap.map(|s| s.total),
            presion,
        })
    }

    // ------------------------------------------------------------ Red

    fn red(&mut self) -> Option<Red> {
        let (contadores, tailscale) = interfaces()?;
        let ahora = Instant::now();
        let lapso = self.red_momento.replace(ahora).map(|t| ahora.duration_since(t).as_secs_f64());
        let antes = std::mem::replace(&mut self.red, contadores);
        let lapso = lapso.filter(|l| *l > 0.2)?;
        let (mut fisica, mut tunel) = (Tramo::default(), Tramo::default());
        for (nombre, (entra, sale)) in &self.red {
            let Some((entra0, sale0)) = antes.get(nombre) else { continue };
            // Los contadores son de 32 bits: la resta con vuelta sirve aunque den la vuelta.
            let bajada = entra.wrapping_sub(*entra0) as f64 / lapso;
            let subida = sale.wrapping_sub(*sale0) as f64 / lapso;
            if tailscale.contains(nombre) {
                tunel.bajada += bajada;
                tunel.subida += subida;
            } else if nombre.starts_with("en") || nombre.starts_with("pdp_ip") {
                // Wi-Fi, cable y el teléfono: lo que de verdad sale del equipo (Tailscale ya va dentro).
                fisica.bajada += bajada;
                fisica.subida += subida;
            }
        }
        Some(Red { bajada: fisica.bajada, subida: fisica.subida, tailscale: (!tailscale.is_empty()).then_some(tunel) })
    }

    // ------------------------------------------------------------ Procesos

    /// Las apps que más gastan y si RustDesk está abierto.
    fn procesos(&mut self) -> (Procesos, bool) {
        let ahora = Instant::now();
        let lapso_ns = self.procesos_momento.replace(ahora).map(|t| ahora.duration_since(t).as_nanos() as f64);
        let nucleos = self.nucleos.len().max(1) as f64;
        let antes = std::mem::take(&mut self.procesos);
        let mut grupos: HashMap<String, Grupo> = HashMap::new();
        let mut rustdesk = false;
        for pid in pids() {
            let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
            let tam = std::mem::size_of::<libc::proc_taskinfo>() as c_int;
            // SAFETY: el búfer mide `tam` bytes. Los procesos de otros usuarios responden 0 (sin permiso).
            let n = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTASKINFO, 0, &mut info as *mut _ as *mut c_void, tam) };
            if n != tam {
                continue;
            }
            let ticks = info.pti_total_user.saturating_add(info.pti_total_system) as u128;
            let ns = (ticks * self.base.numer as u128 / self.base.denom as u128) as u64;
            let (ns0, app) = match antes.get(&pid) {
                Some((ns0, app)) => (Some(*ns0), app.clone()),
                None => match ruta(pid) {
                    Some(r) => (None, nombre_amable(&r)),
                    None => continue,
                },
            };
            rustdesk |= app == "RustDesk";
            let g = grupos.entry(app.clone()).or_default();
            g.memoria += info.pti_resident_size;
            g.procesos += 1;
            if let (Some(ns0), Some(lapso)) = (ns0, lapso_ns)
                && lapso > 0.0
            {
                g.cpu += (ns.saturating_sub(ns0) as f64 / (lapso * nucleos) * 100.0) as f32;
            }
            self.procesos.insert(pid, (ns, app));
        }
        (super::mas_gastan(grupos, true), rustdesk)
    }

    // ------------------------------------------------------------ Batería (una vez por minuto)

    fn registro(&mut self) -> Registro {
        if let Some((t, r)) = self.registro
            && t.elapsed() < REGISTRO_CADA
        {
            return r;
        }
        let r = leer_registro();
        self.registro = Some((Instant::now(), r));
        r
    }
}

impl Drop for Lector {
    fn drop(&mut self) {
        // SAFETY: suelta el puerto del equipo que pidió `nuevo`.
        unsafe { mach_port_deallocate(mach_task_self_, self.host) };
    }
}

// ---------------------------------------------------------------- Temperatura

/// El estado térmico de macOS: "normal", "tibia", "caliente" o "muy-caliente".
fn estado_termico() -> Option<&'static str> {
    use objc2::msg_send;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::{AnyClass, AnyObject};
    autoreleasepool(|_| {
        let clase = AnyClass::get(c"NSProcessInfo")?;
        // SAFETY: processInfo es el objeto compartido del proceso; thermalState devuelve un NSInteger.
        let info: *mut AnyObject = unsafe { msg_send![clase, processInfo] };
        if info.is_null() {
            return None;
        }
        let estado: isize = unsafe { msg_send![info, thermalState] };
        Some(match estado {
            0 => "normal",
            1 => "tibia",
            2 => "caliente",
            _ => "muy-caliente",
        })
    })
}

// ---------------------------------------------------------------- Batería

/// Ciclos, salud (capacidad de hoy frente a la de fábrica) y grados, de AppleSmartBattery.
fn leer_registro() -> Registro {
    // SAFETY: IOServiceMatching crea el diccionario y IOServiceGetMatchingService se lo queda.
    let servicio = unsafe {
        let busqueda = IOServiceMatching(c"AppleSmartBattery".as_ptr());
        if busqueda.is_null() {
            return Registro::default();
        }
        IOServiceGetMatchingService(0, busqueda)
    };
    if servicio == 0 {
        return Registro::default();
    }
    let propiedad = |nombre: &CStr| -> Option<i64> {
        let clave = Cf::texto(nombre)?;
        // SAFETY: el servicio está vivo; Create devuelve un objeto propio (se libera al salir).
        let v = Cf::nuevo(unsafe { IORegistryEntryCreateCFProperty(servicio, clave.0, null(), 0) })?;
        entero_cf(v.0)
    };
    let ciclos = propiedad(c"CycleCount").and_then(|c| u32::try_from(c).ok());
    let diseno = propiedad(c"DesignCapacity").filter(|d| *d > 0);
    let hoy = propiedad(c"NominalChargeCapacity").or_else(|| propiedad(c"AppleRawMaxCapacity")).filter(|c| *c > 0);
    // La "Capacidad máxima" de Ajustes del Sistema; la cuenta propia solo si macOS no la da.
    let salud = capacidad_segun_macos()
        .or_else(|| diseno.zip(hoy).map(|(d, h)| ((h * 100 + d / 2) / d).clamp(1, 100) as u8));
    // En centésimas de grado.
    let grados = propiedad(c"Temperature").map(|t| t as f32 / 100.0).filter(|g| (-20.0..90.0).contains(g));
    // SAFETY: el servicio es propio.
    unsafe { IOObjectRelease(servicio) };
    Registro { ciclos, salud, grados: grados.map(|g| (g * 10.0).round() / 10.0) }
}

/// La "Capacidad máxima" tal como la muestra macOS (Ajustes › Batería › Salud). macOS la calcula por dentro y no
/// la deja en AppleSmartBattery: la cuenta con NominalChargeCapacity da otro número (95 frente a 98).
fn capacidad_segun_macos() -> Option<u8> {
    let salida = std::process::Command::new("/usr/sbin/system_profiler")
        .args(["SPPowerDataType", "-json", "-detailLevel", "mini"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let json: serde_json::Value = serde_json::from_slice(&salida.stdout).ok()?;
    json.get("SPPowerDataType")?.as_array()?.iter().find_map(|fuente| {
        let texto = fuente.get("sppower_battery_health_info")?.get("sppower_battery_health_maximum_capacity")?.as_str()?;
        texto.trim().trim_end_matches('%').trim().parse::<u8>().ok().filter(|v| (1..=100).contains(v))
    })
}

/// El nivel, si carga y cuánto le queda, como lo dice la barra de menús.
fn bateria(registro: Registro) -> Option<Bateria> {
    let info = Cf::nuevo(unsafe { IOPSCopyPowerSourcesInfo() })?;
    let lista = Cf::nuevo(unsafe { IOPSCopyPowerSourcesList(info.0) })?;
    // SAFETY: la lista y la información siguen vivas durante el recorrido; las descripciones no se liberan (Get).
    let cuantas = unsafe { CFArrayGetCount(lista.0) };
    for i in 0..cuantas {
        let d = unsafe { IOPSGetPowerSourceDescription(info.0, CFArrayGetValueAtIndex(lista.0, i)) };
        if d.is_null() || texto_cf(valor(d, c"Type")).as_deref() != Some("InternalBattery") {
            continue;
        }
        if booleano_cf(valor(d, c"Is Present")) == Some(false) {
            continue;
        }
        let actual = entero_cf(valor(d, c"Current Capacity"))?;
        let maxima = entero_cf(valor(d, c"Max Capacity")).filter(|m| *m > 0).unwrap_or(100);
        let nivel = (actual * 100 / maxima).clamp(0, 100) as u8;
        let conectada = texto_cf(valor(d, c"Power Source State")).as_deref() == Some("AC Power");
        let cargando = booleano_cf(valor(d, c"Is Charging")) == Some(true);
        let completa = booleano_cf(valor(d, c"Is Charged")) == Some(true) || (conectada && nivel >= 100 && !cargando);
        let minutos = if cargando {
            entero_cf(valor(d, c"Time to Full Charge"))
        } else if !conectada {
            entero_cf(valor(d, c"Time to Empty"))
        } else {
            None
        };
        return Some(Bateria {
            nivel,
            cargando,
            conectada,
            completa,
            minutos: minutos.filter(|m| *m > 0 && *m < 100 * 60).map(|m| m as u32),
            ciclos: registro.ciclos,
            salud: registro.salud,
        });
    }
    None
}

// ---------------------------------------------------------------- Disco, arranque y red

fn disco() -> Option<Disco> {
    let mut s: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: la ruta termina en cero y la estructura es la que pide statfs.
    if unsafe { libc::statfs(c"/".as_ptr(), &mut s) } != 0 {
        return None;
    }
    let bloque = s.f_bsize as u64;
    Some(Disco { total: s.f_blocks.saturating_mul(bloque), libre: s.f_bavail.saturating_mul(bloque) })
}

/// Segundos desde que arrancó la Mac.
fn encendida() -> Option<u64> {
    let arranque = sysctl::<Hora>(c"kern.boottime")?.segundos;
    let ahora = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    (arranque > 0 && ahora >= arranque).then(|| (ahora - arranque) as u64)
}

/// Los bytes que entraron y salieron por cada interfaz, y cuáles son de Tailscale (utun con una dirección 100.64/10).
fn interfaces() -> Option<(HashMap<String, (u32, u32)>, HashSet<String>)> {
    let mut lista: *mut libc::ifaddrs = null_mut();
    // SAFETY: getifaddrs arma la lista y freeifaddrs la suelta al final.
    if unsafe { libc::getifaddrs(&mut lista) } != 0 {
        return None;
    }
    let mut contadores = HashMap::new();
    let mut tailscale = HashSet::new();
    let mut p = lista;
    while !p.is_null() {
        // SAFETY: cada nodo es válido hasta freeifaddrs.
        let ifa = unsafe { &*p };
        p = ifa.ifa_next;
        if ifa.ifa_name.is_null() || ifa.ifa_addr.is_null() {
            continue;
        }
        let nombre = unsafe { CStr::from_ptr(ifa.ifa_name) }.to_string_lossy().into_owned();
        match unsafe { (*ifa.ifa_addr).sa_family } as c_int {
            libc::AF_LINK if !ifa.ifa_data.is_null() => {
                // SAFETY: en AF_LINK, ifa_data apunta a un if_data (se lee sin suponer alineación).
                let d = unsafe { std::ptr::read_unaligned(ifa.ifa_data as *const libc::if_data) };
                contadores.insert(nombre, (d.ifi_ibytes, d.ifi_obytes));
            }
            libc::AF_INET if nombre.starts_with("utun") => {
                // SAFETY: en AF_INET la dirección es un sockaddr_in.
                let sin = unsafe { std::ptr::read_unaligned(ifa.ifa_addr as *const libc::sockaddr_in) };
                let o = u32::from_be(sin.sin_addr.s_addr).to_be_bytes();
                if o[0] == 100 && (o[1] & 0xC0) == 64 {
                    tailscale.insert(nombre);
                }
            }
            _ => {}
        }
    }
    unsafe { libc::freeifaddrs(lista) };
    Some((contadores, tailscale))
}

// ---------------------------------------------------------------- Procesos

fn pids() -> Vec<i32> {
    // SAFETY: con un búfer nulo, libproc solo dice cuántos hay.
    let n = unsafe { libc::proc_listallpids(null_mut(), 0) };
    if n <= 0 {
        return Vec::new();
    }
    let capacidad = n as usize + 64;
    let mut lista = vec![0i32; capacidad];
    let bytes = (capacidad * std::mem::size_of::<i32>()) as c_int;
    // SAFETY: el búfer tiene `bytes` bytes.
    let m = unsafe { libc::proc_listallpids(lista.as_mut_ptr() as *mut c_void, bytes) };
    if m <= 0 {
        return Vec::new();
    }
    lista.truncate((m as usize).min(capacidad));
    lista.retain(|p| *p > 0);
    lista
}

/// La ruta del programa de un proceso.
fn ruta(pid: i32) -> Option<String> {
    let mut buf = [0u8; 4096]; // PROC_PIDPATHINFO_MAXSIZE
    // SAFETY: el búfer mide lo que se dice.
    let largo = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr() as *mut c_void, buf.len() as u32) };
    if largo <= 0 {
        return None;
    }
    Some(String::from_utf8_lossy(buf.get(..largo as usize)?).into_owned())
}

/// El nombre que reconoce una persona: la app (la primera ".app" de la ruta) o, si no hay, el programa.
/// "/Applications/Google Chrome.app/…/Google Chrome Helper (Renderer)" → "Google Chrome".
fn nombre_amable(ruta: &str) -> String {
    if let Some(i) = ruta.find(".app/").or_else(|| ruta.ends_with(".app").then(|| ruta.len() - 4)) {
        let antes = &ruta[..i];
        let app = antes.rsplit('/').next().unwrap_or(antes);
        if !app.is_empty() {
            return app.to_string();
        }
    }
    ruta.rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or(ruta).to_string()
}
