//! Remoto en Windows: atajos que Windows se queda antes de que RustDesk los mande a la Mac.
//!
//! Con una ventana de sesión de RustDesk al frente, un gancho de teclado de bajo nivel
//! (WH_KEYBOARD_LL, en su propio hilo) se queda con estas combinaciones y manda la que entiende la Mac:
//!
//! - Win+Tab → Ctrl+↑ (Mission Control).
//! - Ctrl+Win+← / → → Ctrl+← / → (cambiar de escritorio).
//! - Win+D → F11 (mostrar el escritorio).
//! - Win+Shift+S e Imprimir pantalla → F13 (la captura liviana de Nexo en la Mac).
//! - Alt+Tab → Win+Tab, que llega como ⌘⇥; al soltar Alt se suelta ⌘ y la Mac elige la app.
//! - Win+E → Win+Alt+Espacio (⌘⌥Espacio: ventana de búsqueda del Finder).
//!
//! También traduce las que inyectan otros programas (los gestos de Logi Options+): solo ignora las de Nexo.
//!
//! Cómo se mandan: RustDesk, en modo "map", lleva cada tecla a la Mac por su scan code (su posición),
//! no por su tecla virtual. Nexo inyecta el scan code real con una tecla virtual sin uso (0xE8): RustDesk
//! la manda como la tecla de verdad y Windows no ve ningún atajo (ni Vista de tareas, ni Inicio,
//! ni Win+↑ maximizando la ventana). Cada lote es completo (pulsa y suelta sus modificadores), así
//! que en la Mac nada queda apretado aunque los gestos lleguen muy seguidos.
//!
//! El mismo gancho atiende los atajos de brillo (los que elige la persona; de fábrica Ctrl+Alt+F1 / F2, ver
//! `teclas`): se pone si la traducción o las teclas de brillo están encendidas, y nunca hay dos. Los de brillo
//! valen con cualquier ventana al frente.

use std::cell::{Cell, RefCell};
use super::teclas::Atajo;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tauri::AppHandle;
use windows::Win32::Foundation::{CloseHandle, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    GetCurrentThreadId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    SendInput, VIRTUAL_KEY, VK_CONTROL, VK_D, VK_E, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN,
    VK_MENU, VK_RCONTROL, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_S, VK_SHIFT, VK_SNAPSHOT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, EVENT_SYSTEM_FOREGROUND, GetForegroundWindow, GetMessageW, GetWindowTextW,
    GetWindowThreadProcessId, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, LLKHF_UP, MSG, PM_NOREMOVE,
    PeekMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WINEVENT_OUTOFCONTEXT,
    WM_APP,
};
use windows::core::{PCWSTR, PWSTR};

/// Si la traducción está encendida (el ajuste "traducir"). La cambia `usar`.
pub static ACTIVO: AtomicBool = AtomicBool::new(true);
/// Si las teclas de brillo están encendidas (el ajuste "teclas de brillo"). La cambia `usar_brillo`.
static BRILLO: AtomicBool = AtomicBool::new(false);
/// Los atajos de brillo, empacados (ver `empacar`). Los cambia `poner_atajos_brillo`.
static ATAJO_BAJAR: AtomicU32 = AtomicU32::new(empacar(Atajo::BAJAR));
static ATAJO_SUBIR: AtomicU32 = AtomicU32::new(empacar(Atajo::SUBIR));

/// Marca de las teclas que inyecta Nexo ("NEXO"): el gancho las deja pasar sin mirarlas.
const MARCA: usize = 0x4E45_584F;
/// Tecla virtual sin uso en Windows: lleva el scan code a RustDesk sin que Windows vea un atajo.
const VK_NEUTRA: u16 = 0xE8;
/// Mensaje al hilo del gancho: mirar ACTIVO y BRILLO y poner o quitar el gancho.
const REVISAR: u32 = WM_APP + 0x4E;
/// Dos bajadas de una tecla tragada más separadas que esto ya no son repetición (se perdió la soltada).
const REPETICION_MS: u32 = 1200;
/// Con las teclas de brillo apretadas, un paso cada 80 ms como mucho (unos 12 por segundo).
const PAUSA_BRILLO_MS: u32 = 80;

static INICIADO: AtomicBool = AtomicBool::new(false);
static HILO: AtomicU32 = AtomicU32::new(0);
static ESCUCHANDO: AtomicBool = AtomicBool::new(false);

/// Arranca el hilo del gancho (una sola vez). Pone el gancho si ACTIVO o BRILLO están encendidos.
pub fn iniciar(_app: AppHandle) {
    if INICIADO.swap(true, Ordering::SeqCst) {
        return;
    }
    let creado = std::thread::Builder::new().name("nexo-remoto-teclado".into()).stack_size(256 * 1024).spawn(hilo);
    if let Err(e) = creado {
        INICIADO.store(false, Ordering::SeqCst);
        eprintln!("Nexo: no se pudo iniciar el gancho de teclado (atajos para RustDesk y teclas de brillo) ({e})");
    }
}

/// Enciende o apaga la traducción. Si las teclas de brillo también están apagadas, no queda ningún gancho puesto.
pub fn usar(activo: bool) {
    ACTIVO.store(activo, Ordering::SeqCst);
    revisar();
}

/// Un atajo en un número: la tecla en los 8 bits bajos y un bit por modificador. El gancho lo lee sin bloqueos.
const fn empacar(a: Atajo) -> u32 {
    (a.vk as u32 & 0xFF) | (a.ctrl as u32) << 8 | (a.alt as u32) << 9 | (a.shift as u32) << 10 | (a.win as u32) << 11
}

/// Cambia los atajos de brillo (ya revisados por `teclas`).
pub fn poner_atajos_brillo(bajar: Atajo, subir: Atajo) {
    ATAJO_BAJAR.store(empacar(bajar), Ordering::SeqCst);
    ATAJO_SUBIR.store(empacar(subir), Ordering::SeqCst);
}

/// Enciende o apaga los atajos de brillo en el mismo gancho.
pub fn usar_brillo(activo: bool) {
    BRILLO.store(activo, Ordering::SeqCst);
    revisar();
}

/// Pide al hilo del gancho que lo ponga o lo quite. Si el hilo aún no arranca, lo mira al arrancar.
fn revisar() {
    let hilo = HILO.load(Ordering::SeqCst);
    if hilo != 0 {
        let _ = unsafe { PostThreadMessageW(hilo, REVISAR, WPARAM(0), LPARAM(0)) };
    }
}

/// Si el gancho está puesto ahora mismo.
#[allow(dead_code)] // para mostrarlo en Remoto más adelante
pub fn escuchando() -> bool {
    ESCUCHANDO.load(Ordering::SeqCst)
}

// ---------------------------------------------------------------- Teclas

/// Una tecla física: su scan code y si es "extendida" (prefijo E0).
#[derive(Clone, Copy)]
struct Tecla {
    scan: u16,
    ext: bool,
}

const fn tecla(scan: u16, ext: bool) -> Tecla {
    Tecla { scan, ext }
}

const WIN_IZQ: Tecla = tecla(0x5B, true);
const CTRL_IZQ: Tecla = tecla(0x1D, false);
const ALT_IZQ: Tecla = tecla(0x38, false);
const TAB: Tecla = tecla(0x0F, false);
const ARRIBA: Tecla = tecla(0x48, true);
const IZQUIERDA: Tecla = tecla(0x4B, true);
const DERECHA: Tecla = tecla(0x4D, true);
const ESPACIO: Tecla = tecla(0x39, false);
const F11: Tecla = tecla(0x57, false);
// F13 por su propio scan code: RustDesk no lleva Imprimir pantalla (E0 37) a ninguna tecla de la Mac.
const F13: Tecla = tecla(0x64, false);
/// Tecla de relleno (scan 0: RustDesk no la manda). Va antes de soltar Win o Alt para que Windows no abra Inicio ni el menú.
const RELLENO: Tecla = tecla(0, false);

// Modificadores abajo, un bit por tecla.
const WIN_I: u8 = 1;
const WIN_D: u8 = 2;
const CTRL_I: u8 = 4;
const CTRL_D: u8 = 8;
const ALT_I: u8 = 16;
const ALT_D: u8 = 32;
const MAYUS_I: u8 = 64;
const MAYUS_D: u8 = 128;
const WIN: u8 = WIN_I | WIN_D;
const CTRL: u8 = CTRL_I | CTRL_D;
const ALT: u8 = ALT_I | ALT_D;
const MAYUS: u8 = MAYUS_I | MAYUS_D;

/// Cada modificador: su bit, su tecla virtual (para leer el estado de Windows) y su tecla física.
const MODIFICADORES: [(u8, VIRTUAL_KEY, Tecla); 8] = [
    (WIN_I, VK_LWIN, tecla(0x5B, true)),
    (WIN_D, VK_RWIN, tecla(0x5C, true)),
    (CTRL_I, VK_LCONTROL, tecla(0x1D, false)),
    (CTRL_D, VK_RCONTROL, tecla(0x1D, true)),
    (ALT_I, VK_LMENU, tecla(0x38, false)),
    (ALT_D, VK_RMENU, tecla(0x38, true)),
    (MAYUS_I, VK_LSHIFT, tecla(0x2A, false)),
    (MAYUS_D, VK_RSHIFT, tecla(0x36, false)),
];

/// El bit del modificador, si lo es. Otros programas a veces mandan Ctrl, Alt o Shift sin lado.
fn bit_de(vk: u16, scan: u32, ext: bool) -> Option<u8> {
    Some(match VIRTUAL_KEY(vk) {
        VK_LWIN => WIN_I,
        VK_RWIN => WIN_D,
        VK_LCONTROL => CTRL_I,
        VK_RCONTROL => CTRL_D,
        VK_CONTROL if ext => CTRL_D,
        VK_CONTROL => CTRL_I,
        VK_LMENU => ALT_I,
        VK_RMENU => ALT_D,
        VK_MENU if ext => ALT_D,
        VK_MENU => ALT_I,
        VK_LSHIFT => MAYUS_I,
        VK_RSHIFT => MAYUS_D,
        VK_SHIFT if scan == 0x36 => MAYUS_D,
        VK_SHIFT => MAYUS_I,
        _ => return None,
    })
}

/// Un evento inyectado, con la marca de Nexo.
fn evento(vk: u16, t: Tecla, arriba: bool) -> INPUT {
    let mut banderas = KEYBD_EVENT_FLAGS(0);
    if t.ext {
        banderas |= KEYEVENTF_EXTENDEDKEY;
    }
    if arriba {
        banderas |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: VIRTUAL_KEY(vk), wScan: t.scan, dwFlags: banderas, time: 0, dwExtraInfo: MARCA },
        },
    }
}

/// Una tecla solo para RustDesk: Windows la ve como la tecla virtual sin uso.
fn neutra(t: Tecla, arriba: bool) -> INPUT {
    evento(VK_NEUTRA, t, arriba)
}

/// Manda un lote. `false` si Windows no aceptó nada (entonces no se traga la tecla original).
fn enviar(lote: &[INPUT]) -> bool {
    if lote.is_empty() {
        return true;
    }
    unsafe { SendInput(lote, std::mem::size_of::<INPUT>() as i32) > 0 }
}

// ---------------------------------------------------------------- Combinaciones

#[derive(Clone, Copy, PartialEq)]
enum Accion {
    MissionControl,
    /// `true`: a la izquierda.
    Escritorio(bool),
    MostrarEscritorio,
    Captura,
    CambiarApp,
    BuscarFinder,
}

/// El atajo de bajar da −1 y el de subir +1. Los modificadores tienen que ser justo los del atajo.
fn tecla_de_brillo(vk: u16, m: u8) -> Option<i32> {
    let pulsado =
        empacar(Atajo { vk, ctrl: m & CTRL != 0, alt: m & ALT != 0, shift: m & MAYUS != 0, win: m & WIN != 0 });
    if pulsado == ATAJO_BAJAR.load(Ordering::Relaxed) {
        Some(-1)
    } else if pulsado == ATAJO_SUBIR.load(Ordering::Relaxed) {
        Some(1)
    } else {
        None
    }
}

/// Qué combinación es, mirando la tecla y los modificadores abajo.
fn combinacion(vk: u16, m: u8) -> Option<Accion> {
    let (win, ctrl, alt, mayus) = (m & WIN != 0, m & CTRL != 0, m & ALT != 0, m & MAYUS != 0);
    match VIRTUAL_KEY(vk) {
        VK_TAB if win && !ctrl && !alt => Some(Accion::MissionControl),
        VK_TAB if alt && !win && !ctrl => Some(Accion::CambiarApp),
        VK_LEFT if win && ctrl && !alt && !mayus => Some(Accion::Escritorio(true)),
        VK_RIGHT if win && ctrl && !alt && !mayus => Some(Accion::Escritorio(false)),
        VK_D if win && !ctrl && !alt && !mayus => Some(Accion::MostrarEscritorio),
        VK_E if win && !ctrl && !alt && !mayus => Some(Accion::BuscarFinder),
        VK_S if win && mayus && !ctrl && !alt => Some(Accion::Captura),
        VK_SNAPSHOT if !win && !ctrl && !alt => Some(Accion::Captura),
        _ => None,
    }
}

/// Lo que se manda por una combinación. Primero la Mac suelta los modificadores que no van
/// (⌘ de Win, ⌥ de Alt…); después se pulsan los de la Mac, la tecla, y se sueltan.
fn lote_de(accion: Accion, e: &Estado) -> Vec<INPUT> {
    // (modificadores de la Mac, tecla, modificadores abajo que la Mac conserva)
    let (pulsar, tecla, conservar): (&[Tecla], Tecla, u8) = match accion {
        Accion::MissionControl => (&[CTRL_IZQ], ARRIBA, 0),
        Accion::Escritorio(izquierda) => (&[CTRL_IZQ], if izquierda { IZQUIERDA } else { DERECHA }, CTRL),
        Accion::MostrarEscritorio => (&[], F11, 0),
        Accion::Captura => (&[], F13, 0),
        Accion::BuscarFinder => (&[WIN_IZQ, ALT_IZQ], ESPACIO, WIN),
        // Alt+Tab ya abierto: otro Tab avanza (con Shift, retrocede).
        Accion::CambiarApp if e.selector => return vec![neutra(TAB, false), neutra(TAB, true)],
        // ⌘ queda apretado en la Mac hasta que se suelte Alt. Shift se conserva (⌘⇧⇥ retrocede).
        Accion::CambiarApp => (&[WIN_IZQ], TAB, MAYUS),
    };
    let mut lote = Vec::with_capacity(12);
    for (bit, _, t) in MODIFICADORES {
        if e.mods & bit != 0 && conservar & bit == 0 {
            lote.push(neutra(t, true));
        }
    }
    for &t in pulsar {
        lote.push(neutra(t, false));
    }
    lote.push(neutra(tecla, false));
    lote.push(neutra(tecla, true));
    if accion != Accion::CambiarApp {
        for &t in pulsar.iter().rev() {
            lote.push(neutra(t, true));
        }
    }
    lote
}

// ---------------------------------------------------------------- Estado del gancho (vive en su hilo)

struct Estado {
    /// Modificadores abajo según los eventos que no son de Nexo (teclado físico o Logi Options+).
    mods: u8,
    /// Teclas cuya bajada se tragó, con la hora de su último evento: su soltada y sus repeticiones también se tragan.
    tragadas: [Option<u32>; 256],
    /// Se tradujo algo con Win (o Alt) abajo: su soltada se reinyecta detrás de un relleno, así Windows
    /// no abre Inicio ni el menú de la ventana aunque los eventos lleguen todos juntos.
    win_usada: bool,
    alt_usada: bool,
    /// Alt+Tab en curso: ⌘ sigue apretado en la Mac.
    selector: bool,
    /// Imprimir pantalla bajó (a veces solo llega su soltada).
    imppnt_abajo: bool,
    /// Último proceso revisado y si es RustDesk.
    pid: u32,
    pid_rustdesk: bool,
    /// Hora del último paso de brillo (para no pasar de unos 12 por segundo).
    ultimo_brillo: Option<u32>,
}

impl Estado {
    const fn nuevo() -> Self {
        Estado {
            mods: 0,
            tragadas: [None; 256],
            win_usada: false,
            alt_usada: false,
            selector: false,
            imppnt_abajo: false,
            pid: 0,
            pid_rustdesk: false,
            ultimo_brillo: None,
        }
    }
}

thread_local! {
    static ESTADO: RefCell<Estado> = const { RefCell::new(Estado::nuevo()) };
    static GANCHO: Cell<Option<HHOOK>> = const { Cell::new(None) };
    static VIGIA: Cell<Option<HWINEVENTHOOK>> = const { Cell::new(None) };
}

/// Suelta ⌘ en la Mac si Alt+Tab quedó abierto.
fn soltar_selector(e: &mut Estado) {
    if e.selector {
        let _ = enviar(&[neutra(WIN_IZQ, true)]);
        e.selector = false;
    }
}

/// Lee de Windows qué modificadores están abajo (por si se perdió algún evento).
fn resincronizar(e: &mut Estado) {
    let mut mods = 0;
    for (bit, vk, _) in MODIFICADORES {
        if unsafe { GetAsyncKeyState(i32::from(vk.0)) } < 0 {
            mods |= bit;
        }
    }
    e.mods = mods;
    if mods & WIN == 0 {
        e.win_usada = false;
    }
    if mods & ALT == 0 {
        e.alt_usada = false;
        soltar_selector(e);
    }
}

/// Decide qué hacer con un evento de teclado. `true`: Nexo se queda con él.
fn procesar(e: &mut Estado, k: &KBDLLHOOKSTRUCT) -> bool {
    let vk = (k.vkCode & 0xFF) as u16;
    let arriba = k.flags.contains(LLKHF_UP);
    let ext = k.flags.contains(LLKHF_EXTENDED);
    let traducir = ACTIVO.load(Ordering::Relaxed);

    if let Some(bit) = bit_de(vk, k.scanCode, ext) {
        if !arriba {
            e.mods |= bit;
            return false;
        }
        e.mods &= !bit;
        let mut lote = Vec::new();
        // Soltar Alt suelta ⌘ en la Mac: el selector de apps elige.
        if e.selector && e.mods & ALT == 0 {
            lote.push(neutra(WIN_IZQ, true));
            e.selector = false;
        }
        let reinyectar = (bit & WIN != 0 && e.win_usada) || (bit & ALT != 0 && e.alt_usada);
        if e.mods & WIN == 0 {
            e.win_usada = false;
        }
        if e.mods & ALT == 0 {
            e.alt_usada = false;
        }
        if reinyectar {
            lote.push(neutra(RELLENO, false));
            lote.push(neutra(RELLENO, true));
            lote.push(evento(vk, tecla(k.scanCode as u16, ext), true));
        }
        let enviado = enviar(&lote);
        return reinyectar && enviado;
    }

    let i = usize::from(vk);
    if arriba {
        let solo_soltada = vk == VK_SNAPSHOT.0 && !std::mem::take(&mut e.imppnt_abajo);
        if e.tragadas[i].take().is_some() {
            return true;
        }
        // Imprimir pantalla que llega sin su bajada.
        if traducir && solo_soltada && e.mods & (WIN | CTRL | ALT) == 0 && sesion_al_frente(e) {
            return enviar(&lote_de(Accion::Captura, e));
        }
        return false;
    }

    // Teclas de brillo: antes que la traducción y con cualquier ventana al frente (también RustDesk).
    // El trabajo va a otro hilo (`teclas::pulsar`): aquí no se espera nada.
    if BRILLO.load(Ordering::Relaxed) {
        if let Some(pasos) = tecla_de_brillo(vk, e.mods) {
            e.tragadas[i] = Some(k.time);
            let listo = match e.ultimo_brillo {
                Some(antes) => k.time.wrapping_sub(antes) >= PAUSA_BRILLO_MS,
                None => true,
            };
            if listo {
                e.ultimo_brillo = Some(k.time);
                super::teclas::pulsar(pasos);
            }
            // Al soltar Alt o Win va detrás de un relleno: la ventana del frente no abre su menú, ni se abre Inicio.
            if e.mods & ALT != 0 {
                e.alt_usada = true;
            }
            if e.mods & WIN != 0 {
                e.win_usada = true;
            }
            return true;
        }
    }

    if vk == VK_SNAPSHOT.0 {
        e.imppnt_abajo = true;
    }
    if let Some(antes) = e.tragadas[i] {
        if k.time.wrapping_sub(antes) < REPETICION_MS {
            // Repetición de una tecla ya traducida: solo Tab con Alt+Tab abierto sigue avanzando.
            e.tragadas[i] = Some(k.time);
            if vk == VK_TAB.0 && e.selector {
                let _ = enviar(&[neutra(TAB, false), neutra(TAB, true)]);
            }
            return true;
        }
        e.tragadas[i] = None;
    }
    if !traducir {
        return false;
    }
    let Some(accion) = combinacion(vk, e.mods) else { return false };
    if !sesion_al_frente(e) || !enviar(&lote_de(accion, e)) {
        return false;
    }
    e.tragadas[i] = Some(k.time);
    if accion == Accion::CambiarApp {
        e.selector = true;
    }
    if e.mods & WIN != 0 {
        e.win_usada = true;
    }
    if e.mods & ALT != 0 {
        e.alt_usada = true;
    }
    true
}

unsafe extern "system" fn al_teclado(codigo: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let escuchar = ACTIVO.load(Ordering::Relaxed) || BRILLO.load(Ordering::Relaxed);
    if codigo == HC_ACTION as i32 && lparam.0 != 0 && escuchar {
        let k = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        // Lo que inyecta Nexo pasa sin mirarse. Lo de otros programas (Logi Options+) sí se traduce.
        if k.dwExtraInfo != MARCA {
            let tragar = ESTADO
                .try_with(|celda| celda.try_borrow_mut().map(|mut e| procesar(&mut e, k)).unwrap_or(false))
                .unwrap_or(false);
            if tragar {
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, codigo, wparam, lparam) }
}

// ---------------------------------------------------------------- La ventana de RustDesk

/// Si la ventana es una sesión de RustDesk. El proceso se revisa solo cuando cambia.
fn es_sesion(e: &mut Estado, hwnd: HWND) -> bool {
    if hwnd.0.is_null() {
        return false;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32)) };
    // Nexo no: leer el título de una ventana propia esperaría al hilo principal.
    if pid == 0 || pid == std::process::id() {
        return false;
    }
    if pid != e.pid {
        e.pid = pid;
        e.pid_rustdesk = es_rustdesk(pid);
    }
    e.pid_rustdesk && es_titulo_de_sesion(&titulo(hwnd))
}

fn sesion_al_frente(e: &mut Estado) -> bool {
    es_sesion(e, unsafe { GetForegroundWindow() })
}

/// El ejecutable empieza con "rustdesk" (rustdesk.exe, o la portátil rustdesk-1.4.x-x86_64.exe).
fn es_rustdesk(pid: u32) -> bool {
    let Ok(proceso) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return false;
    };
    let mut ruta = [0u16; 520];
    let mut largo = ruta.len() as u32;
    let leido = unsafe { QueryFullProcessImageNameW(proceso, PROCESS_NAME_WIN32, PWSTR(ruta.as_mut_ptr()), &mut largo) };
    let _ = unsafe { CloseHandle(proceso) };
    if leido.is_err() {
        return false;
    }
    let ruta = String::from_utf16_lossy(&ruta[..(largo as usize).min(ruta.len())]);
    let nombre = ruta.rsplit(['\\', '/']).next().unwrap_or_default().to_ascii_lowercase();
    nombre.starts_with("rustdesk") && nombre.ends_with(".exe")
}

fn titulo(hwnd: HWND) -> String {
    let mut texto = [0u16; 256];
    let n = unsafe { GetWindowTextW(hwnd, &mut texto) };
    String::from_utf16_lossy(&texto[..(n.max(0) as usize).min(texto.len())])
}

/// RustDesk 1.2 a 1.5 titula la sesión "<id o alias> - Remote Desktop - RustDesk" (sin traducir).
/// No son sesión la principal ("RustDesk") ni las de archivos, puertos o cámara. Ante la duda, sí
/// (la versión vieja titulaba la sesión solo con el id).
fn es_titulo_de_sesion(texto: &str) -> bool {
    let t = texto.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("RustDesk") {
        return false;
    }
    if t.contains("Remote Desktop") {
        return true;
    }
    !["File Transfer", "Port Forward", "View Camera"].iter().any(|otra| t.contains(otra))
}

/// Cambió la ventana del frente: nada queda a medias y, si es una sesión, el gancho se pone primero.
unsafe extern "system" fn al_cambiar_ventana(
    _gancho: HWINEVENTHOOK,
    evento: u32,
    hwnd: HWND,
    _objeto: i32,
    _hijo: i32,
    _hilo: u32,
    _hora: u32,
) {
    if evento != EVENT_SYSTEM_FOREGROUND {
        return;
    }
    let sesion = ESTADO
        .try_with(|celda| {
            let Ok(mut e) = celda.try_borrow_mut() else { return false };
            soltar_selector(&mut e);
            // Las soltadas pendientes pasan: a Windows no le molesta soltar algo que no vio bajar.
            e.tragadas = [None; 256];
            e.pid = 0;
            resincronizar(&mut e);
            es_sesion(&mut e, hwnd)
        })
        .unwrap_or(false);
    if sesion {
        ponerse_primero();
    }
}

// ---------------------------------------------------------------- Hilo y ganchos

fn modulo() -> Option<HINSTANCE> {
    unsafe { GetModuleHandleW(PCWSTR::null()) }.ok().map(HINSTANCE::from)
}

/// RustDesk pone su propio gancho al abrir. Volver a poner el de Nexo lo deja primero en la cadena,
/// así ve las combinaciones antes que RustDesk. Mientras corre esto el hilo no atiende teclas: no se pierde ninguna.
fn ponerse_primero() {
    let _ = GANCHO.try_with(|g| {
        let Some(viejo) = g.get() else { return };
        if let Ok(nuevo) = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(al_teclado), modulo(), 0) } {
            g.set(Some(nuevo));
            let _ = unsafe { UnhookWindowsHookEx(viejo) };
        }
    });
}

fn instalar() {
    if GANCHO.try_with(|g| g.get().is_some()).unwrap_or(true) {
        return;
    }
    match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(al_teclado), modulo(), 0) } {
        Ok(gancho) => {
            let _ = GANCHO.try_with(|g| g.set(Some(gancho)));
        }
        Err(e) => {
            eprintln!("Nexo: no se pudo escuchar el teclado (atajos para RustDesk y teclas de brillo) ({e})");
            return;
        }
    }
    let vigia = unsafe {
        SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(al_cambiar_ventana), 0, 0, WINEVENT_OUTOFCONTEXT)
    };
    if vigia.is_invalid() {
        eprintln!("Nexo: sin aviso de cambio de ventana; Alt+Tab se cierra al soltar Alt");
    } else {
        let _ = VIGIA.try_with(|v| v.set(Some(vigia)));
    }
    let _ = ESTADO.try_with(|celda| {
        if let Ok(mut e) = celda.try_borrow_mut() {
            *e = Estado::nuevo();
            resincronizar(&mut e);
        }
    });
    ESCUCHANDO.store(true, Ordering::SeqCst);
}

fn quitar() {
    let _ = ESTADO.try_with(|celda| {
        if let Ok(mut e) = celda.try_borrow_mut() {
            soltar_selector(&mut e);
            *e = Estado::nuevo();
        }
    });
    if let Some(gancho) = GANCHO.try_with(Cell::take).ok().flatten() {
        let _ = unsafe { UnhookWindowsHookEx(gancho) };
    }
    if let Some(vigia) = VIGIA.try_with(Cell::take).ok().flatten() {
        let _ = unsafe { UnhookWinEvent(vigia) };
    }
    ESCUCHANDO.store(false, Ordering::SeqCst);
}

/// Pone el gancho si la traducción o las teclas de brillo lo necesitan; si no, lo quita.
fn aplicar() {
    let traducir = ACTIVO.load(Ordering::SeqCst);
    if !traducir && !BRILLO.load(Ordering::SeqCst) {
        quitar();
        return;
    }
    instalar();
    if !traducir {
        // El gancho sigue solo por el brillo: nada de la traducción queda a medias (⌘ apretado en la Mac).
        let _ = ESTADO.try_with(|celda| {
            if let Ok(mut e) = celda.try_borrow_mut() {
                soltar_selector(&mut e);
                e.win_usada = false;
                e.alt_usada = false;
            }
        });
    }
}

/// El hilo: atiende el gancho (Windows lo llama desde aquí) y los cambios de ACTIVO y BRILLO.
fn hilo() {
    let mut msg = MSG::default();
    unsafe {
        // Crea la cola de mensajes antes de anunciar el hilo; si no, PostThreadMessageW fallaría.
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        HILO.store(GetCurrentThreadId(), Ordering::SeqCst);
    }
    aplicar();
    loop {
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if r.0 == 0 || r.0 == -1 {
            break;
        }
        if msg.message == REVISAR {
            aplicar();
            continue;
        }
        unsafe { DispatchMessageW(&msg) };
    }
    quitar();
    HILO.store(0, Ordering::SeqCst);
    INICIADO.store(false, Ordering::SeqCst);
}
