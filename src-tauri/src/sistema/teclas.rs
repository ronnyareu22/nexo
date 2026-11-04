//! Teclas de brillo de la Mac → monitor externo bajo el mouse.
//!
//! Escucha las teclas de brillo (eventos "system defined" de macOS). Si el mouse está sobre
//! un monitor externo que responde a DDC, Nexo cambia ese monitor y se queda con la tecla.
//! Si está sobre la pantalla de la MacBook, la deja pasar y macOS hace lo de siempre.
//! Necesita el permiso de Accesibilidad. En Windows no hay atajo (ver abajo).

use crate::dispositivos::pantallas::Pantallas;
use std::sync::Arc;

#[cfg(target_os = "macos")]
pub use mac::{activar, permiso_concedido, pedir_permiso};

// Windows: los teclados de escritorio no mandan teclas de brillo como la Mac, así que no hay atajo.
// El brillo se cambia desde Nexo o el widget. Apagarlo siempre funciona (no hay nada que detener).
#[cfg(windows)]
pub fn activar(_pantallas: Arc<Pantallas>, encendido: bool) -> Result<(), String> {
    if encendido { Err("En Windows usa el brillo desde Nexo o el widget.".into()) } else { Ok(()) }
}
/// Windows no pide permiso de Accesibilidad.
#[cfg(windows)]
pub fn permiso_concedido() -> bool {
    true
}
#[cfg(windows)]
pub fn pedir_permiso() {}

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
