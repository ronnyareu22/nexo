//! Nexo: arranque de la app.
//!
//! - Vive en la barra de menús (Mac) o en la bandeja (Windows) con un ícono.
//! - Clic en el ícono o doble clic en la app: abre el panel.
//! - Al cerrar el panel se destruye su ventana para liberar memoria; la app sigue en segundo plano.

mod dispositivos;
mod sistema;

use dispositivos::historial::{Evento, Historial};
use dispositivos::pantallas::Pantallas;
use std::sync::Arc;
use std::time::Duration;
use tauri_plugin_notification::NotificationExt;

use tauri::{
    AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// Abre el panel, o lo trae al frente si ya está abierto.
fn abrir_panel(app: &AppHandle) {
    if let Some(ventana) = app.get_webview_window("panel") {
        let _ = ventana.show();
        let _ = ventana.set_focus();
        return;
    }

    // Mientras el panel está abierto, Nexo aparece en el Dock como una app normal.
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

    let constructor = WebviewWindowBuilder::new(app, "panel", WebviewUrl::default())
        .title("Nexo")
        .inner_size(1080.0, 720.0)
        .min_inner_size(780.0, 540.0)
        .center();

    // En Mac, la barra de título es transparente y el contenido llega hasta arriba.
    #[cfg(target_os = "macos")]
    let constructor = constructor
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    if let Err(e) = constructor.build() {
        eprintln!("No se pudo abrir el panel: {e}");
    }
}

/// Vigía: cada 20 s mira qué está conectado, anota el historial y aplica los ajustes automáticos.
fn iniciar_vigia(app: AppHandle, historial: Arc<Historial>) {
    std::thread::Builder::new()
        .name("nexo-vigia".into())
        .spawn(move || loop {
            if let Ok(estado) = dispositivos::listar_ahora() {
                for evento in historial.registrar(&estado.dispositivos) {
                    match evento {
                        // macOS tarda un par de segundos en crear el sonido del aparato recién conectado.
                        Evento::Conectado { id } if historial.automatico(&id).usar_salida_al_conectar => {
                            std::thread::spawn(move || {
                                for _ in 0..4 {
                                    std::thread::sleep(Duration::from_millis(2500));
                                    if dispositivos::usar_como_salida(&id).is_ok() {
                                        break;
                                    }
                                }
                            });
                        }
                        Evento::BateriaBaja { nombre, nivel } => {
                            let _ = app
                                .notification()
                                .builder()
                                .title("Batería baja")
                                .body(format!("A {nombre} le queda {nivel}%. Ponlo a cargar."))
                                .show();
                        }
                        _ => {}
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(20));
        })
        .expect("no se pudo iniciar el vigía");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            dispositivos::listar_dispositivos,
            dispositivos::cambiar_conexion,
            dispositivos::poner_alias,
            dispositivos::olvidar_dispositivo,
            dispositivos::sonido,
            dispositivos::ajustar_sonido,
            dispositivos::dispositivos_audio,
            dispositivos::usar_dispositivo_audio,
            dispositivos::ajustar_dispositivo_audio,
            dispositivos::nombre_usuario,
            dispositivos::listar_pantallas,
            dispositivos::poner_brillo,
            dispositivos::brillos,
            dispositivos::modos_pantalla,
            dispositivos::cambiar_modo_pantalla,
            dispositivos::hacer_principal,
            dispositivos::historial,
            dispositivos::borrar_historial,
            dispositivos::automatico,
            dispositivos::guardar_automatico,
            dispositivos::abrir_ajustes_bluetooth,
            sistema::leer_ajustes,
            sistema::usar_teclas_brillo,
            sistema::nivel_aviso,
            sistema::permisos,
            sistema::abrir_ajustes_privacidad,
            sistema::arranque,
            sistema::usar_arranque,
        ])
        .setup(|app| {
            // Pantallas: al cambiar el brillo con las teclas se muestra el aviso en ese monitor.
            let manejador = app.handle().clone();
            let pantallas = Arc::new(Pantallas::iniciar(Box::new(move |id, nivel| {
                sistema::aviso::mostrar(&manejador, id, nivel)
            })));
            app.manage(pantallas.clone());
            if sistema::ajustes::cargar(app.handle()).teclas_brillo && sistema::teclas::permiso_concedido() {
                let _ = sistema::teclas::activar(pantallas, true);
            }

            // Historial de uso y ajustes automáticos: el vigía revisa el Bluetooth cada 20 s.
            let historial = Arc::new(Historial::cargar(app.handle()));
            app.manage(historial.clone());
            iniciar_vigia(app.handle().clone(), historial);

            let abrir = MenuItem::with_id(app, "abrir", "Abrir Nexo", true, None::<&str>)?;
            let salir = MenuItem::with_id(app, "salir", "Salir de Nexo", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&abrir, &salir])?;

            TrayIconBuilder::with_id("nexo")
                .icon(Image::from_bytes(include_bytes!("../icons/bandeja.png"))?)
                .icon_as_template(true)
                .tooltip("Nexo")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, evento| match evento.id.as_ref() {
                    "abrir" => abrir_panel(app),
                    "salir" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|icono, evento| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = evento
                    {
                        abrir_panel(icono.app_handle());
                    }
                })
                .build(app)?;

            abrir_panel(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("no se pudo iniciar Nexo");

    app.run(|app, evento| match evento {
        // Se cerró el panel: la app no se cierra, solo queda el ícono (y sale del Dock).
        RunEvent::ExitRequested { api, code: None, .. } => {
            api.prevent_exit();
            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }
        // Doble clic en la app cuando ya está corriendo.
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => abrir_panel(app),
        _ => {}
    });
}
