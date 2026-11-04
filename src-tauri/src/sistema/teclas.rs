//! Teclas de brillo → pantalla bajo el mouse.
//!
//! Mac: escucha las teclas de brillo (eventos "system defined" de macOS). Si el mouse está sobre
//! un monitor externo que responde a DDC, Nexo cambia ese monitor y se queda con la tecla.
//! Si está sobre la pantalla de la MacBook (o el monitor tiene apagado "responde a las teclas"),
//! la deja pasar y macOS hace lo de siempre. Necesita el permiso de Accesibilidad.
//!
//! Windows: dos atajos que la persona elige (de fábrica Ctrl+Alt+F1 baja y Ctrl+Alt+F2 sube) cambian el brillo
//! de la pantalla bajo el mouse (externa por DDC/CI o la del portátil por WMI), con el mismo aviso. Los ve el
//! gancho de teclado de `remoto_windows` (uno solo para todo), que se pone primero aunque RustDesk esté al frente.
//! Ctrl+Alt+↑/↓ no: con los controladores de Intel giran la pantalla, y VS Code las usa para poner cursores.
//! Además, las teclas de brillo del portátil (F6/F7…) pasan al monitor bajo el mouse: ver `vigilar_portatil`
//! en `pantallas_windows`.
//!
//! Cada pantalla tiene su ajuste: si responde a las teclas y de cuánto es el paso (`TeclasPantalla`).

use crate::dispositivos::pantallas::Pantallas;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Un atajo de Windows: la tecla (código virtual, VK) y sus modificadores.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Atajo {
    pub vk: u16,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

const VK_F1: u16 = 0x70;
const VK_F24: u16 = 0x87;

impl Atajo {
    /// Bajar brillo de fábrica: Ctrl+Alt+F1.
    pub const BAJAR: Atajo = Atajo { vk: VK_F1, ctrl: true, alt: true, shift: false, win: false };
    /// Subir brillo de fábrica: Ctrl+Alt+F2.
    pub const SUBIR: Atajo = Atajo { vk: VK_F1 + 1, ctrl: true, alt: true, shift: false, win: false };

    fn es_tecla_f(&self) -> bool {
        (VK_F1..=VK_F24).contains(&self.vk)
    }

    /// Revisa que sirva: una tecla que no sea modificador, con Ctrl, Alt o Win (o una tecla F sola o con Mayús),
    /// y que no sea un atajo de Windows.
    pub fn validar(&self) -> Result<(), String> {
        // Ctrl, Alt, Mayús y Win (izquierdo y derecho), y teclas sin uso.
        let modificador = matches!(self.vk, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5);
        if self.vk == 0 || self.vk > 0xFE || modificador {
            return Err("Ese atajo necesita una tecla además de Ctrl, Alt, Mayús o Win.".into());
        }
        if !self.ctrl && !self.alt && !self.win && !self.es_tecla_f() {
            return Err("Usa Ctrl, Alt o Win con otra tecla, o una tecla F sola.".into());
        }
        match reservado(self) {
            Some(uso) => Err(format!("Windows usa esa combinación para {uso}. Elige otra.")),
            None => Ok(()),
        }
    }

    /// Los modificadores como se escriben: "Ctrl+Alt+".
    fn texto_modificadores(&self) -> String {
        let mut t = String::new();
        for (si, nombre) in [(self.ctrl, "Ctrl+"), (self.alt, "Alt+"), (self.shift, "Mayús+"), (self.win, "Win+")] {
            if si {
                t.push_str(nombre);
            }
        }
        t
    }

    /// Para el menú del ícono: "Ctrl+Alt+F1/F2" si comparten modificadores; si no, "Ctrl+F9 / Alt+F10".
    pub fn texto_par(bajar: Atajo, subir: Atajo) -> String {
        let (mb, ms) = (bajar.texto_modificadores(), subir.texto_modificadores());
        if mb == ms {
            format!("{mb}{}/{}", nombre_tecla(bajar.vk), nombre_tecla(subir.vk))
        } else {
            format!("{mb}{} / {ms}{}", nombre_tecla(bajar.vk), nombre_tecla(subir.vk))
        }
    }

    /// El atajo si sirve; si no, el de fábrica.
    pub fn valido_o(self, defecto: Atajo) -> Atajo {
        if self.validar().is_ok() { self } else { defecto }
    }
}

/// El nombre corto de una tecla virtual de Windows (el mismo que muestra el panel).
fn nombre_tecla(vk: u16) -> String {
    match vk {
        VK_F1..=VK_F24 => format!("F{}", vk - VK_F1 + 1),
        0x30..=0x39 | 0x41..=0x5A => char::from(vk as u8).to_string(),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        0x08 => "Retroceso".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x1B => "Esc".into(),
        0x20 => "Espacio".into(),
        0x21 => "RePág".into(),
        0x22 => "AvPág".into(),
        0x23 => "Fin".into(),
        0x24 => "Inicio".into(),
        0x25 => "←".into(),
        0x26 => "↑".into(),
        0x27 => "→".into(),
        0x28 => "↓".into(),
        0x2D => "Insert".into(),
        0x2E => "Supr".into(),
        0x6A => "Num *".into(),
        0x6B => "Num +".into(),
        0x6D => "Num -".into(),
        0x6F => "Num /".into(),
        0xBB => "+".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        otro => format!("Tecla {otro}"),
    }
}

/// Atajos que usa Windows (o casi todas las apps). Nexo no los toma.
fn reservado(a: &Atajo) -> Option<&'static str> {
    let letra = |c: u8| a.vk == c as u16;
    let solo = |ctrl: bool, alt: bool, shift: bool, win: bool| (a.ctrl, a.alt, a.shift, a.win) == (ctrl, alt, shift, win);
    if solo(false, false, false, true) {
        return Some(match a.vk {
            0x4C => "bloquear la computadora",
            0x44 => "mostrar el escritorio",
            0x09 => "la vista de tareas",
            0x25..=0x28 => "acomodar las ventanas",
            0x30..=0x39 | 0x41..=0x5A | 0xBE | 0x2C => "sus atajos",
            _ => return None,
        });
    }
    if solo(false, false, true, true) && letra(b'S') {
        return Some("recortar la pantalla");
    }
    if solo(false, true, false, false) {
        return match a.vk {
            0x73 => Some("cerrar la ventana"),
            0x09 => Some("cambiar de ventana"),
            0x1B | 0x20 => Some("el menú de la ventana"),
            _ => None,
        };
    }
    if solo(true, false, false, false) {
        return match a.vk {
            0x1B => Some("abrir Inicio"),
            0x41..=0x5A => Some("los atajos de las apps (copiar, pegar, deshacer…)"),
            _ => None,
        };
    }
    if solo(true, false, true, false) && a.vk == 0x1B {
        return Some("abrir el Administrador de tareas");
    }
    if solo(true, true, false, false) && a.vk == 0x2E {
        return Some("la pantalla de seguridad");
    }
    None
}

#[cfg(target_os = "macos")]
pub use mac::{activar, permiso_concedido, pedir_permiso};

#[cfg(windows)]
pub use win::{activar, configurar, pulsar};
/// Windows no pide permiso de Accesibilidad.
#[cfg(windows)]
pub fn permiso_concedido() -> bool {
    true
}
#[cfg(windows)]
pub fn pedir_permiso() {}

/// Solo Windows: los atajos y las teclas del portátil.
#[cfg(not(windows))]
pub fn configurar(_bajar: Atajo, _subir: Atajo, _portatil: bool) {}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn activar(_pantallas: Arc<Pantallas>, _encendido: bool) -> Result<(), String> {
    Err("Todavía no disponible en este sistema".into())
}
#[cfg(not(any(target_os = "macos", windows)))]
pub fn permiso_concedido() -> bool {
    false
}
#[cfg(not(any(target_os = "macos", windows)))]
pub fn pedir_permiso() {}

#[cfg(windows)]
mod win {
    use super::*;
    use crate::dispositivos::pantallas;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, SyncSender};

    static PANTALLAS: OnceLock<Arc<Pantallas>> = OnceLock::new();
    /// Los toques van a un hilo propio: el gancho de teclado no espera nada.
    static CANAL: OnceLock<SyncSender<i32>> = OnceLock::new();
    /// El ajuste "teclas de brillo" (atajos y teclas del portátil).
    static ENCENDIDO: AtomicBool = AtomicBool::new(false);
    /// El ajuste "usar las teclas de brillo del portátil".
    static PORTATIL: AtomicBool = AtomicBool::new(true);

    /// Los atajos de brillo y si las teclas del portátil pasan al monitor. Se puede llamar antes de `activar`.
    pub fn configurar(bajar: Atajo, subir: Atajo, portatil: bool) {
        crate::sistema::remoto_windows::poner_atajos_brillo(bajar.valido_o(Atajo::BAJAR), subir.valido_o(Atajo::SUBIR));
        PORTATIL.store(portatil, Ordering::SeqCst);
        revisar_portatil();
    }

    /// Escucha las teclas del portátil solo con los dos ajustes encendidos.
    fn revisar_portatil() {
        if let Some(p) = PANTALLAS.get() {
            let usar = ENCENDIDO.load(Ordering::SeqCst) && PORTATIL.load(Ordering::SeqCst);
            pantallas::vigilar_portatil(p.clone(), usar);
        }
    }

    /// Enciende o apaga los atajos y las teclas del portátil. La primera vez crea el hilo que atiende los toques.
    pub fn activar(pantallas: Arc<Pantallas>, encendido: bool) -> Result<(), String> {
        let _ = PANTALLAS.set(pantallas);
        ENCENDIDO.store(encendido, Ordering::SeqCst);
        revisar_portatil();
        if encendido && CANAL.get().is_none() {
            let (tx, rx) = mpsc::sync_channel(16);
            std::thread::Builder::new()
                .name("nexo-teclas".into())
                .stack_size(256 * 1024)
                .spawn(move || trabajar(rx))
                .map_err(|e| format!("No se pudo escuchar las teclas de brillo ({e})"))?;
            let _ = CANAL.set(tx);
        }
        crate::sistema::remoto_windows::usar_brillo(encendido);
        Ok(())
    }

    /// Lo llama el gancho de teclado: −1 baja, +1 sube. No espera; si la cola está llena, el toque se pierde.
    pub fn pulsar(pasos: i32) {
        if let Some(tx) = CANAL.get() {
            let _ = tx.try_send(pasos);
        }
    }

    /// Busca la pantalla bajo el mouse y le manda el paso (el de esa pantalla; 1/16 de fábrica, como en la Mac).
    /// Si ese monitor no responde a las teclas (su ajuste), va a la pantalla del portátil. El aviso lo muestra
    /// el hilo de pantallas.
    fn trabajar(rx: Receiver<i32>) {
        while let Ok(primero) = rx.recv() {
            // Los toques que ya esperan se juntan en uno.
            let pasos = primero + rx.try_iter().sum::<i32>();
            if pasos == 0 {
                continue;
            }
            let (Some(p), Some(id)) = (PANTALLAS.get(), pantallas::id_bajo_el_cursor()) else {
                continue;
            };
            let id = if p.teclas_de(id).responde {
                id
            } else {
                match pantallas::id_integrada() {
                    Some(i) => i,
                    None => continue,
                }
            };
            // Quizá se acaba de conectar o se activó DDC/CI en su menú: se mira de nuevo antes del paso.
            if !p.es_controlable(id) {
                p.refrescar();
            }
            p.paso(id, pasos);
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use objc2::encode::{Encode, Encoding, RefEncode};
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    type Evento = *mut c_void;
    type Puerto = *mut c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Punto {
        x: f64,
        y: f64,
    }

    /// Puntero a CGEvent con la codificación que espera Objective-C.
    #[repr(transparent)]
    struct CGEventPtr(*mut c_void);
    unsafe impl Encode for CGEventPtr {
        const ENCODING: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
    }
    unsafe impl RefEncode for CGEventPtr {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
    }

    type AlEvento = extern "C" fn(*mut c_void, u32, Evento, *mut c_void) -> Evento;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventTapCreate(tap: u32, lugar: u32, opciones: u32, mascara: u64, cb: AlEvento, datos: *mut c_void) -> Puerto;
        fn CGEventTapEnable(tap: Puerto, encender: bool);
        fn CGEventGetLocation(e: Evento) -> Punto;
        fn CGGetDisplaysWithPoint(p: Punto, max: u32, ids: *mut u32, cantidad: *mut u32) -> i32;
        fn CGDisplayIsBuiltin(id: u32) -> u32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFMachPortCreateRunLoopSource(alloc: *const c_void, puerto: Puerto, orden: isize) -> *mut c_void;
        fn CFRunLoopGetCurrent() -> *mut c_void;
        fn CFRunLoopAddSource(bucle: *mut c_void, fuente: *mut c_void, modo: *const c_void);
        fn CFRunLoopRun();
        fn CFDictionaryCreate(
            alloc: *const c_void,
            claves: *const *const c_void,
            valores: *const *const c_void,
            n: isize,
            cb_claves: *const c_void,
            cb_valores: *const c_void,
        ) -> *const c_void;
        fn CFRelease(objeto: *const c_void);
        static kCFRunLoopCommonModes: *const c_void;
        static kCFBooleanTrue: *const c_void;
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(opciones: *const c_void) -> bool;
        static kAXTrustedCheckOptionPrompt: *const c_void;
    }

    struct Tap(Puerto);
    unsafe impl Send for Tap {}
    unsafe impl Sync for Tap {}

    static TAP: OnceLock<Tap> = OnceLock::new();
    static PANTALLAS: OnceLock<Arc<Pantallas>> = OnceLock::new();
    static ACTIVO: AtomicBool = AtomicBool::new(false);

    const EVENTO_SISTEMA: u32 = 14; // NSEventTypeSystemDefined
    const TECLA_SUBIR: i64 = 2; // NX_KEYTYPE_BRIGHTNESS_UP
    const TECLA_BAJAR: i64 = 3; // NX_KEYTYPE_BRIGHTNESS_DOWN

    pub fn permiso_concedido() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    /// Muestra el aviso de macOS que lleva a Ajustes › Privacidad › Accesibilidad.
    pub fn pedir_permiso() {
        unsafe {
            let claves = [kAXTrustedCheckOptionPrompt];
            let valores = [kCFBooleanTrue];
            let opciones = CFDictionaryCreate(
                std::ptr::null(),
                claves.as_ptr(),
                valores.as_ptr(),
                1,
                &kCFTypeDictionaryKeyCallBacks as *const c_void,
                &kCFTypeDictionaryValueCallBacks as *const c_void,
            );
            AXIsProcessTrustedWithOptions(opciones);
            CFRelease(opciones);
        }
    }

    /// Enciende o apaga el atajo. La primera vez crea la escucha en su propio hilo.
    pub fn activar(pantallas: Arc<Pantallas>, encendido: bool) -> Result<(), String> {
        let _ = PANTALLAS.set(pantallas);
        ACTIVO.store(encendido, Ordering::SeqCst);
        if !encendido {
            if let Some(t) = TAP.get() {
                unsafe { CGEventTapEnable(t.0, false) };
            }
            return Ok(());
        }
        if !permiso_concedido() {
            return Err("Falta el permiso de Accesibilidad".into());
        }
        if let Some(t) = TAP.get() {
            unsafe { CGEventTapEnable(t.0, true) };
            return Ok(());
        }
        std::thread::Builder::new()
            .name("nexo-teclas".into())
            .spawn(|| unsafe {
                let tap = CGEventTapCreate(1, 0, 0, 1u64 << EVENTO_SISTEMA, al_evento, std::ptr::null_mut());
                if tap.is_null() {
                    eprintln!("Nexo: no se pudo escuchar las teclas de brillo");
                    return;
                }
                let _ = TAP.set(Tap(tap));
                let fuente = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
                CFRunLoopAddSource(CFRunLoopGetCurrent(), fuente, kCFRunLoopCommonModes);
                CGEventTapEnable(tap, true);
                CFRunLoopRun();
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// (tecla, presionada) de un evento de teclas especiales, si lo es.
    fn leer_tecla(evento: Evento) -> Option<(i64, bool)> {
        let clase = AnyClass::get(c"NSEvent")?;
        objc2::rc::autoreleasepool(|_| unsafe {
            let ns: *mut AnyObject = msg_send![clase, eventWithCGEvent: CGEventPtr(evento)];
            if ns.is_null() {
                return None;
            }
            let subtipo: i16 = msg_send![ns, subtype];
            if subtipo != 8 {
                return None; // no es de teclas especiales (brillo, volumen…)
            }
            let datos: isize = msg_send![ns, data1];
            let datos = datos as i64;
            let tecla = (datos & 0xFFFF_0000) >> 16;
            let presionada = ((datos & 0xFF00) >> 8) == 0x0A;
            Some((tecla, presionada))
        })
    }

    extern "C" fn al_evento(_proxy: *mut c_void, tipo: u32, evento: Evento, _datos: *mut c_void) -> Evento {
        // macOS apaga la escucha si tarda o si el usuario la interrumpe: se vuelve a encender.
        if tipo == 0xFFFF_FFFE || tipo == 0xFFFF_FFFF {
            if let (Some(t), true) = (TAP.get(), ACTIVO.load(Ordering::SeqCst)) {
                unsafe { CGEventTapEnable(t.0, true) };
            }
            return evento;
        }
        if tipo != EVENTO_SISTEMA || !ACTIVO.load(Ordering::SeqCst) {
            return evento;
        }
        let Some((tecla, presionada)) = leer_tecla(evento) else { return evento };
        if tecla != TECLA_SUBIR && tecla != TECLA_BAJAR {
            return evento;
        }

        let mut id = 0u32;
        let mut cantidad = 0u32;
        let punto = unsafe { CGEventGetLocation(evento) };
        if unsafe { CGGetDisplaysWithPoint(punto, 1, &mut id, &mut cantidad) } != 0 || cantidad == 0 {
            return evento;
        }
        if unsafe { CGDisplayIsBuiltin(id) } != 0 {
            return evento; // la MacBook: lo maneja macOS
        }
        let Some(pantallas) = PANTALLAS.get() else { return evento };
        if !pantallas.teclas_de(id).responde {
            return evento; // ese monitor no responde a las teclas: macOS cambia la MacBook
        }
        if !pantallas.es_controlable(id) {
            pantallas.refrescar(); // quizá se acaba de conectar; la próxima vez ya estará
            return evento;
        }
        if presionada {
            pantallas.paso(id, if tecla == TECLA_SUBIR { 1 } else { -1 });
        }
        std::ptr::null_mut() // Nexo se queda con la tecla
    }
}

#[cfg(test)]
mod pruebas {
    use super::Atajo;

    #[test]
    fn atajos_validos_y_reservados() {
        assert!(Atajo::BAJAR.validar().is_ok());
        assert!(Atajo::SUBIR.validar().is_ok());
        // Una tecla F sola sirve; una letra sola, no.
        assert!(Atajo { vk: 0x75, ..Default::default() }.validar().is_ok());
        assert!(Atajo { vk: 0x41, ..Default::default() }.validar().is_err());
        assert!(Atajo { vk: 0x41, shift: true, ..Default::default() }.validar().is_err());
        // Solo un modificador: no.
        assert!(Atajo { vk: 0x11, ctrl: true, ..Default::default() }.validar().is_err());
        // Los de Windows: no.
        assert!(Atajo { vk: 0x4C, win: true, ..Default::default() }.validar().is_err());
        assert!(Atajo { vk: 0x73, alt: true, ..Default::default() }.validar().is_err());
        assert!(Atajo { vk: 0x2E, ctrl: true, alt: true, ..Default::default() }.validar().is_err());
        assert!(Atajo { vk: 0x43, ctrl: true, ..Default::default() }.validar().is_err());
        // Ctrl+Alt+letra sí.
        assert!(Atajo { vk: 0x42, ctrl: true, alt: true, ..Default::default() }.validar().is_ok());
        assert_eq!(Atajo { vk: 0, ..Default::default() }.valido_o(Atajo::BAJAR), Atajo::BAJAR);
    }

    #[test]
    fn texto_de_los_atajos() {
        assert_eq!(Atajo::texto_par(Atajo::BAJAR, Atajo::SUBIR), "Ctrl+Alt+F1/F2");
        let f9 = Atajo { vk: 0x78, ctrl: true, ..Default::default() };
        let f10 = Atajo { vk: 0x79, alt: true, ..Default::default() };
        assert_eq!(Atajo::texto_par(f9, f10), "Ctrl+F9 / Alt+F10");
    }
}
