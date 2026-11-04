//! Escucha de teclado del remoto (pide Accesibilidad):
//! - Traduce los atajos de Windows a los de la Mac solo en las teclas que inyecta RustDesk. Cada tecla trae el
//!   proceso que la mandó (campo 41 del evento): se traduce solo si su programa está dentro de RustDesk.app.
//!   El teclado de la Mac nunca se toca.
//! - F13 (a esa tecla lleva RustDesk la de Imprimir pantalla) lanza la captura liviana, venga del teclado que venga.
//! - Anota cuándo se pulsó copiar, para saber de dónde vino lo copiado (historial del portapapeles).
//! - Ctrl de RustDesk se guarda un instante: si la tecla que sigue es un atajo (Ctrl+C, Ctrl+V…), Ctrl se manda
//!   como ⌘. Si no, se manda Ctrl tal cual. Así Chrome, VS Code y las apps Electron, que miran qué
//!   modificador está abajo en ese momento, ven ⌘V y no ⌃⌘V.
//! La escucha es rápida y nunca espera: la captura va en su propio hilo.

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

type Evento = *mut c_void;
type Puerto = *mut c_void;
type AlEvento = extern "C" fn(*mut c_void, u32, Evento, *mut c_void) -> Evento;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(tap: u32, lugar: u32, opciones: u32, mascara: u64, cb: AlEvento, datos: *mut c_void) -> Puerto;
    fn CGEventTapEnable(tap: Puerto, encender: bool);
    fn CGEventGetIntegerValueField(e: Evento, campo: u32) -> i64;
    fn CGEventSetIntegerValueField(e: Evento, campo: u32, valor: i64);
    fn CGEventGetFlags(e: Evento) -> u64;
    fn CGEventSetFlags(e: Evento, banderas: u64);
    fn CGEventKeyboardSetUnicodeString(e: Evento, largo: usize, texto: *const u16);
    fn CGEventCreateCopy(e: Evento) -> Evento;
    fn CGEventTapPostEvent(proxy: *mut c_void, e: Evento);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFMachPortCreateRunLoopSource(alloc: *const c_void, puerto: Puerto, orden: isize) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(bucle: *mut c_void, fuente: *mut c_void, modo: *const c_void);
    fn CFRunLoopRun();
    fn CFRelease(objeto: *const c_void);
    static kCFRunLoopCommonModes: *const c_void;
}

struct Tap(Puerto);
// SAFETY: el puerto solo se usa para encender o apagar la escucha, que macOS permite desde cualquier hilo.
unsafe impl Send for Tap {}
unsafe impl Sync for Tap {}

static TAP: OnceLock<Tap> = OnceLock::new();
static CREANDO: AtomicBool = AtomicBool::new(false);
/// Último proceso visto: (pid << 1) | es_rustdesk. Cero: nada guardado.
static ULTIMO_PID: AtomicU64 = AtomicU64::new(0);
/// Alt+Tab de Windows en curso: mientras Alt siga abajo, Alt (⌥) vale como ⌘.
static ALT_TAB: AtomicBool = AtomicBool::new(false);
/// Cuándo se pulsó ⌘C o ⌘X por última vez (ms desde 1970).
static ULTIMA_COPIA: AtomicU64 = AtomicU64::new(0);
/// Ctrl de RustDesk guardado hasta ver qué tecla viene (una copia del evento; nulo si no hay).
static CTRL_GUARDADO: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
/// Ctrl se mandó como ⌘ (porque vino un atajo): al soltar Ctrl se suelta ⌘.
static CTRL_COMO_COMANDO: AtomicBool = AtomicBool::new(false);
/// Modificadores que RustDesk tiene abajo ahora (de su último aviso de modificadores).
static MODS_RUSTDESK: AtomicU64 = AtomicU64::new(0);

// Tipos de evento
const TECLA_ABAJO: u32 = 10;
const TECLA_ARRIBA: u32 = 11;
const MODIFICADORES: u32 = 12;
const APAGADA_POR_TIEMPO: u32 = 0xFFFF_FFFE;
const APAGADA_POR_USUARIO: u32 = 0xFFFF_FFFF;

// Campos del evento
const CAMPO_REPETIDA: u32 = 8; // kCGKeyboardEventAutorepeat
const CAMPO_CODIGO: u32 = 9; // kCGKeyboardEventKeycode
const CAMPO_PID: u32 = 41; // kCGEventSourceUnixProcessID

// Modificadores (y el bit de cada tecla física, izquierda o derecha, para que todo cuadre)
const SHIFT: u64 = 0x0002_0000 | 0x0002 | 0x0004;
const CONTROL: u64 = 0x0004_0000 | 0x0001 | 0x2000;
const OPCION: u64 = 0x0008_0000 | 0x0020 | 0x0040;
const COMANDO: u64 = 0x0010_0000 | 0x0008 | 0x0010;
const BIT_SHIFT: u64 = 0x0002_0000;
const BIT_CONTROL: u64 = 0x0004_0000;
const BIT_OPCION: u64 = 0x0008_0000;
const BIT_COMANDO: u64 = 0x0010_0000;
const SHIFT_IZQ: u64 = 0x0002_0000 | 0x0002;
const OPCION_IZQ: u64 = 0x0008_0000 | 0x0020;
const COMANDO_IZQ: u64 = 0x0010_0000 | 0x0008;
/// Las flechas reales traen "teclado numérico" y "fn".
const FLECHA: u64 = 0x0020_0000 | 0x0080_0000;

// Teclas (posición en el teclado; RustDesk en modo "map" manda la misma posición)
const A: u16 = 0;
const S: u16 = 1;
const F: u16 = 3;
const Z: u16 = 6;
const X: u16 = 7;
const C: u16 = 8;
const V: u16 = 9;
const W: u16 = 13;
const R: u16 = 15;
const Y: u16 = 16;
const T: u16 = 17;
const P: u16 = 35;
const L: u16 = 37;
const N: u16 = 45;
const TAB: u16 = 48;
const BORRAR: u16 = 51;
const COMANDO_TECLA: u16 = 55;
const CONTROL_TECLA: u16 = 59;
const CONTROL_DER_TECLA: u16 = 62;
const COMANDO_DER_TECLA: u16 = 54;
const OPCION_TECLA: u16 = 58;
const OPCION_DER_TECLA: u16 = 61;
const F13: u16 = 105;
const INICIO: u16 = 115;
const PAG_ARRIBA: u16 = 116;
const SUPR: u16 = 117;
const PAG_ABAJO: u16 = 121;
const FIN: u16 = 119;
const IZQUIERDA: u16 = 123;
const DERECHA: u16 = 124;
const ABAJO: u16 = 125;
const ARRIBA: u16 = 126;

/// Ctrl + estas letras pasan a ⌘ + la misma letra.
const LETRAS: [u16; 13] = [C, V, X, Z, A, S, F, P, N, T, W, R, L];

/// Apps de terminal: ahí ⌃C interrumpe, así que solo se traduce Ctrl+Shift (como en Windows Terminal).
const TERMINALES: &[&str] = &[
    "com.apple.Terminal",
    "com.googlecode.iterm2",
    "dev.warp.Warp-Stable",
    "dev.warp.Warp",
    "com.mitchellh.ghostty",
    "org.alacritty",
    "io.alacritty",
    "net.kovidgoyal.kitty",
    "com.github.wez.wezterm",
    "co.zeit.hyper",
    "com.raphaelamorim.rio",
    "org.tabby",
];

fn quiere() -> bool {
    super::TRADUCIR.load(Ordering::Relaxed) || super::CAPTURA.load(Ordering::Relaxed) || super::LIVIANO.load(Ordering::Relaxed)
}

/// Enciende la escucha si algún ajuste la necesita y hay permiso; si no, la apaga. Se llama cada 5 s
/// (así también arranca cuando la persona da el permiso de Accesibilidad más tarde).
pub fn asegurar() {
    let quiere = quiere();
    if let Some(t) = TAP.get() {
        // SAFETY: puerto válido creado por CGEventTapCreate.
        unsafe { CGEventTapEnable(t.0, quiere) };
        return;
    }
    if !quiere || !crate::sistema::teclas::permiso_concedido() || CREANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    let r = std::thread::Builder::new().name("nexo-remoto-teclas".into()).spawn(|| unsafe {
        let mascara = (1u64 << TECLA_ABAJO) | (1u64 << TECLA_ARRIBA) | (1u64 << MODIFICADORES);
        // Sesión, al principio de la fila, escucha que puede cambiar eventos.
        let tap = CGEventTapCreate(1, 0, 0, mascara, al_evento, std::ptr::null_mut());
        if tap.is_null() {
            eprintln!("Nexo: no se pudo escuchar el teclado para el remoto");
            CREANDO.store(false, Ordering::SeqCst);
            return;
        }
        let fuente = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        if fuente.is_null() {
            CREANDO.store(false, Ordering::SeqCst);
            return;
        }
        let _ = TAP.set(Tap(tap));
        CFRunLoopAddSource(CFRunLoopGetCurrent(), fuente, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        CFRunLoopRun();
    });
    if r.is_err() {
        CREANDO.store(false, Ordering::SeqCst);
    }
}

fn ms_ahora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Si se pulsó copiar o cortar en los últimos `ms` milisegundos.
pub fn copia_reciente(ms: u64) -> bool {
    ms_ahora().saturating_sub(ULTIMA_COPIA.load(Ordering::Relaxed)) <= ms
}

extern "C" fn al_evento(proxy: *mut c_void, tipo: u32, evento: Evento, _datos: *mut c_void) -> Evento {
    // macOS apaga la escucha si tarda o si el usuario la interrumpe: se vuelve a encender.
    if tipo == APAGADA_POR_TIEMPO || tipo == APAGADA_POR_USUARIO {
        if let (Some(t), true) = (TAP.get(), quiere()) {
            unsafe { CGEventTapEnable(t.0, true) };
        }
        return evento;
    }
    if evento.is_null() || !matches!(tipo, TECLA_ABAJO | TECLA_ARRIBA | MODIFICADORES) {
        return evento;
    }
    // SAFETY: evento de teclado válido mientras dura la llamada.
    let codigo = unsafe { CGEventGetIntegerValueField(evento, CAMPO_CODIGO) } as u16;
    let originales = unsafe { CGEventGetFlags(evento) };
    let mut banderas = originales;

    // Imprimir pantalla del PC (F13): captura liviana. Nexo se queda con la tecla.
    if tipo != MODIFICADORES && codigo == F13 && super::CAPTURA.load(Ordering::Relaxed) {
        if tipo == TECLA_ABAJO && unsafe { CGEventGetIntegerValueField(evento, CAMPO_REPETIDA) } == 0 {
            super::captura::lanzar();
        }
        return std::ptr::null_mut();
    }

    let traduce = super::TRADUCIR.load(Ordering::Relaxed) && de_rustdesk(evento);
    if traduce {
        if tipo == MODIFICADORES {
            MODS_RUSTDESK.store(banderas & (SHIFT | CONTROL | OPCION | COMANDO), Ordering::Relaxed);
        } else if matches!(codigo, IZQUIERDA | DERECHA | ARRIBA | ABAJO | INICIO | FIN | PAG_ARRIBA | PAG_ABAJO | SUPR) {
            // RustDesk manda estas teclas sin los modificadores que tiene abajo (llega "→" en vez de "⌃→" o "⇧→"):
            // se los pone, así funcionan ⌃← ⌃→ (escritorios), ⌃↑ (Mission Control) y ⇧+flechas (seleccionar).
            banderas |= MODS_RUSTDESK.load(Ordering::Relaxed);
        } else {
            // Las demás teclas sí traen sus modificadores reales: sirven para corregir el recuerdo si quedó viejo.
            MODS_RUSTDESK.store(banderas & (SHIFT | CONTROL | OPCION | COMANDO), Ordering::Relaxed);
        }
        if let Some(resultado) = control_diferido(proxy, tipo, codigo, banderas, evento) {
            return resultado;
        }
    }

    let (mut nuevo_codigo, mut nuevas) = (codigo, banderas);
    if traduce {
        if let Some((c, b)) = traducir_evento(tipo, codigo, banderas) {
            nuevo_codigo = c;
            nuevas = b;
        }
        // ⌃← ⌃→ (cambiar de escritorio) y ⌃↑ ⌃↓ (Mission Control): las flechas de RustDesk llegan sin la marca
        // "fn / teclado numérico" de un teclado real y macOS no las reconoce. La marca va SOLO al presionar:
        // la Mac recuerda las marcas de la última tecla, y si quedara al soltar, fn (🌐) se quedaría "pegada".
        if matches!(nuevo_codigo, IZQUIERDA | DERECHA | ARRIBA | ABAJO) {
            if tipo == TECLA_ABAJO && nuevas & BIT_CONTROL != 0 && nuevas & (BIT_COMANDO | BIT_OPCION) == 0 {
                nuevas |= FLECHA;
            } else if tipo == TECLA_ARRIBA {
                nuevas &= !FLECHA;
            }
        }
    }
    if nuevo_codigo != codigo {
        unsafe { CGEventSetIntegerValueField(evento, CAMPO_CODIGO, nuevo_codigo as i64) };
    }
    if nuevas != originales {
        unsafe { CGEventSetFlags(evento, nuevas) };
    }
    // Las apps buscan los atajos por el carácter de la tecla: se pone el de la tecla nueva (⌘C es "c", no ⌃C).
    if tipo != MODIFICADORES && (nuevo_codigo != codigo || nuevas != banderas) {
        if let Some(caracter) = caracter(nuevo_codigo, nuevas) {
            unsafe { CGEventKeyboardSetUnicodeString(evento, 1, &caracter) };
        }
    }
    if tipo == TECLA_ABAJO && nuevas & BIT_COMANDO != 0 && matches!(nuevo_codigo, C | X) {
        ULTIMA_COPIA.store(ms_ahora(), Ordering::Relaxed);
    }
    evento
}

/// Suelta la copia guardada de Ctrl. Devuelve si había una.
fn soltar_ctrl_guardado() -> bool {
    let viejo = CTRL_GUARDADO.swap(std::ptr::null_mut(), Ordering::SeqCst);
    if viejo.is_null() {
        return false;
    }
    // SAFETY: la copia la creó CGEventCreateCopy y nadie más la tiene.
    unsafe { CFRelease(viejo) };
    true
}

/// Ctrl de RustDesk, diferido. `Some(evento)` si ya se resolvió aquí (nulo = se descarta); `None` sigue el camino normal.
fn control_diferido(proxy: *mut c_void, tipo: u32, codigo: u16, b: u64, evento: Evento) -> Option<Evento> {
    if tipo == MODIFICADORES && matches!(codigo, CONTROL_TECLA | CONTROL_DER_TECLA) {
        if b & BIT_CONTROL != 0 {
            // Ctrl abajo: con ⌘ o ⌥ ya abajo, o en una terminal (⌃C interrumpe), pasa tal cual.
            if b & (BIT_COMANDO | BIT_OPCION) != 0 || es_terminal() {
                return None;
            }
            // SAFETY: evento válido; la copia es nuestra hasta soltarla.
            let copia = unsafe { CGEventCreateCopy(evento) };
            if copia.is_null() {
                return None;
            }
            soltar_ctrl_guardado();
            CTRL_GUARDADO.store(copia, Ordering::SeqCst);
            return Some(std::ptr::null_mut());
        }
        // Ctrl arriba.
        if CTRL_COMO_COMANDO.swap(false, Ordering::SeqCst) {
            // Se había mandado como ⌘: se suelta ⌘.
            unsafe {
                CGEventSetIntegerValueField(evento, CAMPO_CODIGO, COMANDO_TECLA as i64);
                CGEventSetFlags(evento, b & !CONTROL & !COMANDO);
            }
            return Some(evento);
        }
        // Ctrl solo, sin otra tecla en medio: en la Mac no hace nada, se descarta.
        if soltar_ctrl_guardado() {
            return Some(std::ptr::null_mut());
        }
        return None;
    }
    if tipo != TECLA_ABAJO {
        return None;
    }
    let guardado = CTRL_GUARDADO.swap(std::ptr::null_mut(), Ordering::SeqCst);
    if guardado.is_null() {
        return None;
    }
    // Llegó la tecla que sigue a Ctrl: si con Ctrl es un atajo de ⌘, Ctrl va como ⌘; si no, va Ctrl.
    let como_comando = traducir(codigo, b, es_terminal).is_some_and(|(_, n)| n & BIT_COMANDO != 0 && n & BIT_CONTROL == 0);
    // SAFETY: `guardado` es nuestra copia; se manda delante de esta tecla y se suelta.
    unsafe {
        if como_comando {
            CGEventSetIntegerValueField(guardado, CAMPO_CODIGO, COMANDO_TECLA as i64);
            let f = CGEventGetFlags(guardado);
            CGEventSetFlags(guardado, (f & !CONTROL) | COMANDO_IZQ);
            CTRL_COMO_COMANDO.store(true, Ordering::SeqCst);
        }
        CGEventTapPostEvent(proxy, guardado);
        CFRelease(guardado);
    }
    None
}

/// Si la tecla la mandó RustDesk. Se recuerda el último proceso para no preguntar en cada tecla.
fn de_rustdesk(evento: Evento) -> bool {
    let pid = unsafe { CGEventGetIntegerValueField(evento, CAMPO_PID) };
    if pid <= 0 || pid > i32::MAX as i64 {
        return false; // teclado físico
    }
    let guardado = ULTIMO_PID.load(Ordering::Relaxed);
    if guardado != 0 && guardado >> 1 == pid as u64 {
        return guardado & 1 == 1;
    }
    let es = super::procesos::es_rustdesk(pid as i32);
    ULTIMO_PID.store(((pid as u64) << 1) | es as u64, Ordering::Relaxed);
    es
}

/// El carácter de las teclas que puede dejar la traducción (letras, flechas y Retroceso).
fn caracter(codigo: u16, b: u64) -> Option<u16> {
    let letra = match codigo {
        A => b'a',
        S => b's',
        F => b'f',
        Z => b'z',
        X => b'x',
        C => b'c',
        V => b'v',
        W => b'w',
        R => b'r',
        Y => b'y',
        T => b't',
        P => b'p',
        L => b'l',
        N => b'n',
        IZQUIERDA => return Some(0xF702), // NSLeftArrowFunctionKey
        DERECHA => return Some(0xF703),
        ABAJO => return Some(0xF701),
        ARRIBA => return Some(0xF700),
        BORRAR => return Some(0x7F),
        _ => return None,
    };
    let letra = if b & BIT_SHIFT != 0 { letra.to_ascii_uppercase() } else { letra };
    Some(letra as u16)
}

fn es_terminal() -> bool {
    super::mac::app_al_frente().is_some_and(|id| TERMINALES.contains(&id.as_str()))
}

/// Alt de Windows (⌥) pasa a ⌘ conservando lo demás.
fn opcion_a_comando(b: u64) -> u64 {
    if b & BIT_OPCION == 0 { b } else { (b & !OPCION) | COMANDO_IZQ }
}

fn traducir_evento(tipo: u32, codigo: u16, b: u64) -> Option<(u16, u64)> {
    let opcion = b & BIT_OPCION != 0;
    if tipo == MODIFICADORES {
        // Mientras dura Alt+Tab, soltar o pulsar Alt es soltar o pulsar ⌘ (así se cierra el selector de apps).
        if !ALT_TAB.load(Ordering::Relaxed) {
            return None;
        }
        if !opcion {
            ALT_TAB.store(false, Ordering::Relaxed);
        }
        let tecla = match codigo {
            OPCION_TECLA => COMANDO_TECLA,
            OPCION_DER_TECLA => COMANDO_DER_TECLA,
            otra => otra,
        };
        return Some((tecla, opcion_a_comando(b)));
    }
    // Alt+Tab de Windows → ⌘⇥ (cambiar de app).
    if codigo == TAB && opcion && b & (BIT_CONTROL | BIT_COMANDO) == 0 {
        if tipo == TECLA_ABAJO {
            ALT_TAB.store(true, Ordering::Relaxed);
        }
        return Some((codigo, opcion_a_comando(b)));
    }
    if ALT_TAB.load(Ordering::Relaxed) {
        if opcion {
            return Some((codigo, opcion_a_comando(b)));
        }
        // Alt ya no está abajo: terminó (por si no llegó el aviso de soltarlo).
        ALT_TAB.store(false, Ordering::Relaxed);
    }
    traducir(codigo, b, es_terminal)
}

/// La tabla de atajos (la misma que muestra la interfaz, `ATAJOS` en remoto.rs).
/// `terminal` se pregunta solo cuando hace falta (es una consulta a macOS).
fn traducir(codigo: u16, b: u64, terminal: impl Fn() -> bool) -> Option<(u16, u64)> {
    let ctrl = b & BIT_CONTROL != 0;
    let cmd = b & BIT_COMANDO != 0;
    let opcion = b & BIT_OPCION != 0;
    let shift = b & BIT_SHIFT != 0;

    // Ctrl+Win+← → (escritorios en Windows) → ⌃← → (escritorios en la Mac).
    if ctrl && cmd && !opcion && matches!(codigo, IZQUIERDA | DERECHA) {
        return Some((codigo, b & !COMANDO));
    }
    if cmd || opcion {
        return None;
    }
    match codigo {
        // Inicio y Fin: principio y final de la línea; con Ctrl, del documento. Shift sigue seleccionando.
        INICIO | FIN => {
            if terminal() {
                return None;
            }
            let flecha = b | FLECHA | COMANDO_IZQ;
            Some(match (codigo == INICIO, ctrl) {
                (true, false) => (IZQUIERDA, flecha),
                (false, false) => (DERECHA, flecha),
                (true, true) => (ARRIBA, flecha & !CONTROL),
                (false, true) => (ABAJO, flecha & !CONTROL),
            })
        }
        // Ctrl+Retroceso: borrar la palabra anterior (⌥⌫).
        BORRAR if ctrl && !shift => (!terminal()).then_some((BORRAR, (b & !CONTROL) | OPCION_IZQ)),
        _ if !ctrl => None,
        // Ctrl+← → ↑ quedan igual: en la Mac cambian de escritorio y abren Mission Control.
        IZQUIERDA | DERECHA | ARRIBA | ABAJO => None,
        _ if terminal() => {
            // En la terminal, ⌃C interrumpe: solo Ctrl+Shift copia, pega y maneja pestañas, como en Windows Terminal.
            (shift && matches!(codigo, C | V | T | W | F)).then_some((codigo, (b & !CONTROL & !SHIFT) | COMANDO_IZQ))
        }
        Y if !shift => Some((Z, (b & !CONTROL) | COMANDO_IZQ | SHIFT_IZQ)),
        _ if LETRAS.contains(&codigo) => Some((codigo, (b & !CONTROL) | COMANDO_IZQ)),
        _ => None,
    }
}
