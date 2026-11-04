//! Una sola copia de Nexo en Windows. Si ya hay una abierta (por ejemplo, abres el acceso directo con Nexo en
//! la bandeja), la nueva le pide mostrar el panel y se cierra. Así no quedan dos íconos, dos widgets, dos ganchos
//! de teclado ni dos receptores de XDrop. En la Mac de esto se encarga el sistema.
//!
//! Un mutex con nombre marca la copia abierta (Windows lo suelta solo si Nexo se cierra o se cae) y un evento
//! con nombre le avisa que muestre el panel. Ambos son de la sesión actual ("Local\").

/// Si esta es la única copia. Si ya había otra, le avisa y devuelve `false` (entonces hay que salir).
pub fn primera() -> bool {
    #[cfg(windows)]
    {
        win::primera()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Cuando otra copia pide mostrarse, llama a `al_pedir` (desde un hilo propio que duerme mientras espera).
pub fn escuchar(al_pedir: impl Fn() + Send + 'static) {
    #[cfg(windows)]
    win::escuchar(al_pedir);
    #[cfg(not(windows))]
    let _ = al_pedir;
}

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{
        CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent, WaitForSingleObject,
    };
    use windows::core::{PCWSTR, w};

    fn nombre_mutex() -> PCWSTR {
        w!("Local\\com.ronny.nexo.instancia")
    }

    fn nombre_evento() -> PCWSTR {
        w!("Local\\com.ronny.nexo.mostrar")
    }

    pub fn primera() -> bool {
        // Si no se puede crear, se abre igual: mejor dos copias que ninguna.
        let Ok(mutex) = (unsafe { CreateMutexW(None, false, nombre_mutex()) }) else { return true };
        if unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
            // El mutex queda abierto mientras viva Nexo.
            return true;
        }
        if let Ok(evento) = unsafe { OpenEventW(EVENT_MODIFY_STATE, false, nombre_evento()) } {
            let _ = unsafe { SetEvent(evento) };
            let _ = unsafe { CloseHandle(evento) };
        }
        let _ = unsafe { CloseHandle(mutex) };
        false
    }

    pub fn escuchar(al_pedir: impl Fn() + Send + 'static) {
        let Ok(evento) = (unsafe { CreateEventW(None, false, false, nombre_evento()) }) else {
            eprintln!("Nexo: sin aviso entre copias (no se pudo crear el evento)");
            return;
        };
        // HANDLE no cruza hilos; su valor sí.
        let crudo = evento.0 as usize;
        let hilo = std::thread::Builder::new().name("nexo-instancia".into()).spawn(move || {
            let evento = HANDLE(crudo as *mut core::ffi::c_void);
            while unsafe { WaitForSingleObject(evento, INFINITE) } == WAIT_OBJECT_0 {
                al_pedir();
            }
        });
        if let Err(e) = hilo {
            eprintln!("Nexo: sin aviso entre copias ({e})");
        }
    }
}
