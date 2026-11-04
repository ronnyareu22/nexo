//! Windows: lo que mide Analítica, todo con kernel32 (sin WMI ni procesos aparte).
//! - CPU: `GetSystemTimes` (la diferencia entre dos lecturas). Memoria: `GlobalMemoryStatusEx`.
//! - Batería: `GetSystemPowerStatus` (sin ciclos ni salud). Disco: `GetDiskFreeSpaceExW` de la unidad del sistema.
//! - Encendida: `GetTickCount64`. Procesos: `CreateToolhelp32Snapshot` + `GetProcessTimes` y la memoria de cada uno.
//! - La temperatura y la red no se miden aquí: la interfaz dice "No disponible".

use super::{Bateria, Cpu, Disco, Grupo, Memoria, Muestra, Procesos, Temperatura};
use std::collections::HashMap;
use std::time::Instant;

const TH32CS_SNAPPROCESS: u32 = 0x2;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const INVALIDO: isize = -1;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Filetime {
    bajo: u32,
    alto: u32,
}

impl Filetime {
    /// En unidades de 100 ns.
    fn valor(self) -> u64 {
        ((self.alto as u64) << 32) | self.bajo as u64
    }
}

#[repr(C)]
#[derive(Default)]
struct EstadoMemoria {
    largo: u32,
    _carga: u32,
    total: u64,
    disponible: u64,
    _total_paginacion: u64,
    _disponible_paginacion: u64,
    _total_virtual: u64,
    _disponible_virtual: u64,
    _extendida: u64,
}

#[repr(C)]
#[derive(Default)]
struct EstadoEnergia {
    corriente: u8,
    banderas: u8,
    porcentaje: u8,
    _ahorro: u8,
    segundos: u32,
    _segundos_llena: u32,
}

#[repr(C)]
struct EntradaProceso {
    largo: u32,
    _uso: u32,
    pid: u32,
    _monton: usize,
    _modulo: u32,
    _hilos: u32,
    _padre: u32,
    _prioridad: i32,
    _banderas: u32,
    exe: [u16; 260],
}

#[repr(C)]
#[derive(Default)]
struct MemoriaProceso {
    largo: u32,
    _fallos: u32,
    _pico: usize,
    en_uso: usize,
    _otros: [usize; 6],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetSystemTimes(inactivo: *mut Filetime, nucleo: *mut Filetime, usuario: *mut Filetime) -> i32;
    fn GlobalMemoryStatusEx(estado: *mut EstadoMemoria) -> i32;
    fn GetSystemPowerStatus(estado: *mut EstadoEnergia) -> i32;
    fn GetDiskFreeSpaceExW(carpeta: *const u16, libre_tuyo: *mut u64, total: *mut u64, libre: *mut u64) -> i32;
    fn GetTickCount64() -> u64;
    fn CreateToolhelp32Snapshot(banderas: u32, pid: u32) -> isize;
    fn Process32FirstW(foto: isize, entrada: *mut EntradaProceso) -> i32;
    fn Process32NextW(foto: isize, entrada: *mut EntradaProceso) -> i32;
    fn OpenProcess(acceso: u32, heredar: i32, pid: u32) -> isize;
    fn GetProcessTimes(
        proceso: isize,
        creado: *mut Filetime,
        salida: *mut Filetime,
        nucleo: *mut Filetime,
        usuario: *mut Filetime,
    ) -> i32;
    fn K32GetProcessMemoryInfo(proceso: isize, contadores: *mut MemoriaProceso, largo: u32) -> i32;
    fn CloseHandle(objeto: isize) -> i32;
}

/// Cierra el identificador al salir.
struct Manija(isize);

impl Drop for Manija {
    fn drop(&mut self) {
        // SAFETY: es un identificador propio y válido.
        unsafe { CloseHandle(self.0) };
    }
}

/// Lo que mide Windows, con lo que hace falta recordar entre muestras.
pub struct Lector {
    /// (inactivo, núcleo, usuario) de la lectura anterior.
    cpu: Option<(u64, u64, u64)>,
    /// Por cada proceso: el tiempo de CPU (100 ns) y su programa.
    procesos: HashMap<u32, (u64, String)>,
    procesos_momento: Option<Instant>,
    nucleos: u32,
}

impl Lector {
    pub fn nuevo() -> Self {
        let nucleos = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1);
        Lector { cpu: None, procesos: HashMap::new(), procesos_momento: None, nucleos }
    }

    pub fn medir(&mut self) -> Muestra {
        let (procesos, rustdesk) = self.procesos();
        Muestra {
            cpu: self.cpu(),
            memoria: memoria(),
            temperatura: Temperatura::default(),
            bateria: bateria(),
            disco: disco(),
            red: None,
            // SAFETY: solo lee el reloj.
            encendida: Some(unsafe { GetTickCount64() } / 1000),
            procesos,
            rustdesk,
        }
    }

    fn cpu(&mut self) -> Option<Cpu> {
        let (mut inactivo, mut nucleo, mut usuario) = (Filetime::default(), Filetime::default(), Filetime::default());
        // SAFETY: las tres estructuras son de este hilo.
        if unsafe { GetSystemTimes(&mut inactivo, &mut nucleo, &mut usuario) } == 0 {
            return None;
        }
        let ahora = (inactivo.valor(), nucleo.valor(), usuario.valor());
        let (i0, n0, u0) = self.cpu.replace(ahora)?;
        // El tiempo de núcleo ya incluye el inactivo.
        let todo = ahora.1.saturating_sub(n0) + ahora.2.saturating_sub(u0);
        let libre = ahora.0.saturating_sub(i0);
        let total = if todo == 0 { 0.0 } else { todo.saturating_sub(libre) as f32 * 100.0 / todo as f32 };
        Some(Cpu { total: super::redondear(total.clamp(0.0, 100.0)), nucleos: Vec::new(), cantidad: self.nucleos })
    }

    /// Los programas que más gastan y si RustDesk está abierto.
    fn procesos(&mut self) -> (Procesos, bool) {
        let ahora = Instant::now();
        // En unidades de 100 ns, como los tiempos de proceso.
        let lapso = self.procesos_momento.replace(ahora).map(|t| ahora.duration_since(t).as_nanos() as f64 / 100.0);
        let antes = std::mem::take(&mut self.procesos);
        let mut grupos: HashMap<String, Grupo> = HashMap::new();
        let mut rustdesk = false;
        // SAFETY: una foto de los procesos de este momento; se cierra al salir.
        let foto = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if foto == INVALIDO || foto == 0 {
            return (super::mas_gastan(grupos, false), false);
        }
        let foto = Manija(foto);
        let mut e = EntradaProceso {
            largo: std::mem::size_of::<EntradaProceso>() as u32,
            _uso: 0,
            pid: 0,
            _monton: 0,
            _modulo: 0,
            _hilos: 0,
            _padre: 0,
            _prioridad: 0,
            _banderas: 0,
            exe: [0; 260],
        };
        // SAFETY: la entrada lleva su tamaño, como pide Toolhelp.
        let mut hay = unsafe { Process32FirstW(foto.0, &mut e) } != 0;
        while hay {
            if e.pid != 0 {
                let fin = e.exe.iter().position(|c| *c == 0).unwrap_or(e.exe.len());
                let exe = String::from_utf16_lossy(&e.exe[..fin]);
                rustdesk |= exe.eq_ignore_ascii_case("rustdesk.exe");
                if let Some((tiempo, memoria)) = medir_proceso(e.pid) {
                    let nombre = antes.get(&e.pid).map(|(_, n)| n.clone()).unwrap_or_else(|| nombre_amable(&exe));
                    let g = grupos.entry(nombre.clone()).or_default();
                    g.memoria += memoria;
                    g.procesos += 1;
                    if let (Some((t0, _)), Some(lapso)) = (antes.get(&e.pid), lapso)
                        && lapso > 0.0
                    {
                        g.cpu += (tiempo.saturating_sub(*t0) as f64 / (lapso * self.nucleos.max(1) as f64) * 100.0) as f32;
                    }
                    self.procesos.insert(e.pid, (tiempo, nombre));
                }
            }
            // SAFETY: la misma foto y la misma entrada.
            hay = unsafe { Process32NextW(foto.0, &mut e) } != 0;
        }
        (super::mas_gastan(grupos, false), rustdesk)
    }
}

/// (tiempo de CPU en 100 ns, bytes en memoria) de un proceso. Los protegidos no se dejan abrir: `None`.
fn medir_proceso(pid: u32) -> Option<(u64, u64)> {
    // SAFETY: OpenProcess devuelve 0 si no se puede; si no, se cierra al salir.
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if h == 0 {
        return None;
    }
    let h = Manija(h);
    let (mut creado, mut salida, mut nucleo, mut usuario) =
        (Filetime::default(), Filetime::default(), Filetime::default(), Filetime::default());
    // SAFETY: estructuras propias y un identificador abierto.
    if unsafe { GetProcessTimes(h.0, &mut creado, &mut salida, &mut nucleo, &mut usuario) } == 0 {
        return None;
    }
    let mut m = MemoriaProceso { largo: std::mem::size_of::<MemoriaProceso>() as u32, ..Default::default() };
    // SAFETY: la estructura lleva su tamaño.
    let memoria = if unsafe { K32GetProcessMemoryInfo(h.0, &mut m, m.largo) } != 0 { m.en_uso as u64 } else { 0 };
    Some((nucleo.valor() + usuario.valor(), memoria))
}

/// "chrome.exe" → "chrome".
fn nombre_amable(exe: &str) -> String {
    let largo = exe.len();
    if largo > 4 && exe.is_char_boundary(largo - 4) && exe[largo - 4..].eq_ignore_ascii_case(".exe") {
        exe[..largo - 4].to_string()
    } else {
        exe.to_string()
    }
}

fn memoria() -> Option<Memoria> {
    let mut m = EstadoMemoria { largo: std::mem::size_of::<EstadoMemoria>() as u32, ..Default::default() };
    // SAFETY: la estructura lleva su tamaño.
    if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 || m.total == 0 {
        return None;
    }
    Some(Memoria {
        total: m.total,
        usada: m.total.saturating_sub(m.disponible),
        libre: m.disponible,
        ..Default::default()
    })
}

fn bateria() -> Option<Bateria> {
    let mut e = EstadoEnergia::default();
    // SAFETY: estructura propia.
    if unsafe { GetSystemPowerStatus(&mut e) } == 0 {
        return None;
    }
    // 128: no hay batería; 255: no se sabe.
    if e.banderas & 128 != 0 || e.banderas == 255 || e.porcentaje > 100 {
        return None;
    }
    let conectada = e.corriente == 1;
    let cargando = e.banderas & 8 != 0;
    Some(Bateria {
        nivel: e.porcentaje,
        cargando,
        conectada,
        completa: conectada && !cargando && e.porcentaje >= 100,
        minutos: (!conectada && e.segundos != u32::MAX && e.segundos > 0).then(|| e.segundos / 60),
        ciclos: None,
        salud: None,
    })
}

/// La unidad donde está Windows (casi siempre C:).
fn disco() -> Option<Disco> {
    let unidad = std::env::var("SystemDrive").ok().filter(|u| u.len() == 2).unwrap_or_else(|| "C:".into());
    let ruta: Vec<u16> = format!("{unidad}\\").encode_utf16().chain(std::iter::once(0)).collect();
    let (mut libre_tuyo, mut total, mut _libre) = (0u64, 0u64, 0u64);
    // SAFETY: la ruta termina en cero y los tres números son propios.
    if unsafe { GetDiskFreeSpaceExW(ruta.as_ptr(), &mut libre_tuyo, &mut total, &mut _libre) } == 0 || total == 0 {
        return None;
    }
    Some(Disco { total, libre: libre_tuyo })
}
