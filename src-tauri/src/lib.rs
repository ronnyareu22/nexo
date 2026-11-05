//! Nexo: arranque de la app.
//!
//! - Vive en la barra de menús (Mac) o en la bandeja (Windows) con un ícono.
//! - Clic en el ícono o doble clic en la app: abre el panel. Su menú tiene accesos rápidos (sistema/bandeja.rs).
//! - Al cerrar el panel se destruye su ventana para liberar memoria; la app sigue en segundo plano.
//! - El widget de escritorio (sistema/widget.rs) vive aparte y abre el panel en la sección de cada casilla.
//! - Los widgets de brillo (sistema/widget_brillo.rs), uno por pantalla, siguen el aviso "brillo-cambio".

mod dispositivos;
mod sistema;

use dispositivos::historial::{Evento, Historial};
use dispositivos::pantallas::{Avisos, Pantallas};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tauri_plugin_notification::NotificationExt;

use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

/// Las secciones del panel (las mismas de `src/routes/+page.svelte`).
const SECCIONES: [&str; 11] = [
    "inicio",
    "setup",
    "casa",
    "dispositivos",
    "audio",
    "pantallas",
    "remoto",
    "analitica",
    "drop",
    "automatizaciones",
    "ajustes",
];

/// Adónde abrir el panel: una sección y, en Dispositivos, la página de un aparato.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct Destino {
    seccion: String,
    dispositivo: Option<String>,
}

impl Destino {
    /// Solo secciones conocidas; el aparato solo cuenta en Dispositivos.
    fn nuevo(seccion: &str, dispositivo: Option<String>) -> Option<Self> {
        if !SECCIONES.contains(&seccion) {
            return None;
        }
        let dispositivo = dispositivo.filter(|d| seccion == "dispositivos" && !d.is_empty() && d.len() <= 200);
        Some(Destino { seccion: seccion.into(), dispositivo })
    }

    /// La dirección con que se abre el panel: `/?seccion=…`, o `/?pagina=…` para la página de un aparato.
    fn direccion(&self) -> String {
        match &self.dispositivo {
            Some(d) => format!("/?pagina={}", codificar(d)),
            None => format!("/?seccion={}", self.seccion),
        }
    }
}

/// Deja pasar letras, números y `:-_.`; lo demás va en %XX (los id de Windows pueden traer `#` o `&`).
fn codificar(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b':' | b'-' | b'_' | b'.') {
            salida.push(b as char);
        } else {
            salida.push_str(&format!("%{b:02X}"));
        }
    }
    salida
}

/// Abre el panel, o lo trae al frente si ya está abierto. Con un destino, va a esa sección.
fn abrir_panel(app: &AppHandle, destino: Option<Destino>) {
    if let Some(ventana) = app.get_webview_window("panel") {
        let _ = ventana.unminimize();
        let _ = ventana.show();
        let _ = ventana.set_focus();
        // El panel ya cargado cambia de sección solo (escucha "nexo-ir" en +page.svelte).
        if let Some(d) = destino {
            let _ = app.emit_to("panel", "nexo-ir", d);
        }
        return;
    }

    // Mientras el panel está abierto, Nexo aparece en el Dock como una app normal.
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

    let pagina = match &destino {
        Some(d) => WebviewUrl::App(d.direccion().into()),
        None => WebviewUrl::default(),
    };
    let constructor = WebviewWindowBuilder::new(app, "panel", pagina)
        .title("Nexo")
        .inner_size(1080.0, 720.0)
        .min_inner_size(780.0, 540.0)
        .center();

    // En Mac, la barra de título es transparente y el contenido llega hasta arriba.
    // En Windows queda la barra de título normal del sistema.
    #[cfg(target_os = "macos")]
    let constructor = constructor
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    if let Err(e) = constructor.build() {
        eprintln!("No se pudo abrir el panel: {e}");
    }
}

/// Abre el panel (o lo trae al frente) en una sección; con `dispositivo`, en la página de ese aparato.
/// Si la sección no es una conocida (por ejemplo, vacía), lo abre donde quedó. Lo usa el widget.
#[tauri::command]
async fn abrir_seccion(app: AppHandle, seccion: String, dispositivo: Option<String>) {
    let destino = Destino::nuevo(&seccion, dispositivo);
    let a = app.clone();
    let _ = app.run_on_main_thread(move || abrir_panel(&a, destino));
}

/// Lo que llega a las ventanas cuando cambia el brillo de una pantalla.
#[derive(Serialize, Clone)]
struct CambioBrillo {
    id: u32,
    nivel: u8,
}

/// Huella de lo que muestra el widget: si cambia, se le avisa para que se ponga al día.
fn huella(estado: &dispositivos::Estado) -> String {
    let mut h = format!("{}", estado.bluetooth_encendido);
    for d in &estado.dispositivos {
        h.push_str(&format!(
            "|{}:{}:{:?}:{:?}:{:?}",
            d.id, d.conectado, d.bateria, d.bateria_izquierda, d.bateria_derecha
        ));
    }
    h
}

/// Vigía: cada 20 s mira qué está conectado, anota el historial y aplica los ajustes automáticos.
fn iniciar_vigia(app: AppHandle, historial: Arc<Historial>) {
    let vigia = std::thread::Builder::new()
        .name("nexo-vigia".into())
        .spawn(move || {
            let mut huella_anterior = String::new();
            loop {
                let mut aparatos = None;
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
                    // Widgets nativos de la Mac: el estado con los nombres que la persona les puso.
                    let mut lista = estado.dispositivos.clone();
                    historial.poner_alias_en(&mut lista);
                    sistema::widgets_mac::actualizar(&app, estado.bluetooth_encendido, &lista);
                    // El widget se pone al día cuando algo cambió, sin leer más seguido por su cuenta.
                    let nueva = huella(&estado);
                    if nueva != huella_anterior {
                        if !huella_anterior.is_empty() {
                            sistema::widget::avisar_cambio(&app);
                        }
                        huella_anterior = nueva;
                    }
                    aparatos = Some((estado.bluetooth_encendido, lista));
                }
                // El menú del ícono: los aparatos ya leídos (sin volver a leer el Bluetooth). Solo se toca si cambió.
                sistema::bandeja::al_vigilar(&app, aparatos);
                std::thread::sleep(Duration::from_secs(20));
            }
        });
    // Sin vigía, Nexo abre igual (sin historial automático ni aviso de batería baja).
    if let Err(e) = vigia {
        eprintln!("Nexo: no se pudo iniciar el vigía ({e})");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Un pánico queda anotado en nexo-errores.log antes de cerrarse (sistema/fallos.rs).
    sistema::fallos::instalar();
    // Windows: si Nexo ya está abierto, esa copia muestra el panel y esta se cierra.
    if !sistema::instancia::primera() {
        return;
    }
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
            dispositivos::probar_sonido,
            dispositivos::volumen_avisos,
            dispositivos::poner_volumen_avisos,
            dispositivos::nombre_usuario,
            dispositivos::listar_pantallas,
            dispositivos::poner_brillo,
            dispositivos::brillos,
            dispositivos::ajustes_monitor,
            dispositivos::poner_vcp,
            dispositivos::poner_minimo_brillo,
            dispositivos::modos_pantalla,
            dispositivos::cambiar_modo_pantalla,
            dispositivos::hacer_principal,
            dispositivos::historial,
            dispositivos::borrar_historial,
            dispositivos::automatico,
            dispositivos::guardar_automatico,
            dispositivos::abrir_ajustes_bluetooth,
            dispositivos::abrir_ajustes_sonido,
            sistema::leer_ajustes,
            sistema::usar_teclas_brillo,
            sistema::poner_atajo_brillo,
            sistema::usar_teclas_portatil,
            sistema::teclas_pantalla,
            sistema::poner_teclas_pantalla,
            sistema::nivel_aviso,
            sistema::permisos,
            sistema::abrir_ajustes_privacidad,
            sistema::pedir_permiso,
            sistema::leer_bienvenida,
            sistema::guardar_nombre,
            sistema::completar_bienvenida,
            sistema::arranque,
            sistema::usar_arranque,
            sistema::widget_activo,
            sistema::usar_widget,
            sistema::widgets_brillo,
            sistema::usar_widget_brillo,
            sistema::remoto::remoto_estado,
            sistema::remoto::remoto_ajustar,
            sistema::remoto::remoto_preparar,
            sistema::remoto::remoto_capturar,
            sistema::remoto::remoto_abrir_rustdesk,
            sistema::remoto::remoto_conectar_mac,
            sistema::remoto::remoto_abrir_capturas,
            sistema::remoto::portapapeles_historial,
            sistema::remoto::portapapeles_copiar,
            sistema::remoto::portapapeles_borrar,
            sistema::pantalla_virtual::pantalla_virtual_estado,
            sistema::pantalla_virtual::pantalla_virtual,
            sistema::pantalla_virtual::pantalla_virtual_ajustar,
            sistema::remoto::mando::remoto_mac_estado,
            sistema::remoto::mando::remoto_mac_pantalla_virtual,
            sistema::remoto::mando::remoto_mac_ajustar,
            sistema::equipos::equipos_estado,
            sistema::equipos::equipos_modo,
            sistema::equipos::equipos_olvidar,
            sistema::equipos::equipos_abrir_tailscale,
            sistema::equipos::equipos_abrir_enlace,
            sistema::analitica::analitica_local,
            sistema::analitica::analitica_remota,
            sistema::analitica::analitica_equipos,
            sistema::drop::drop_destinos,
            sistema::drop::drop_enviar,
            sistema::drop::drop_cancelar,
            sistema::drop::drop_envios,
            sistema::drop::drop_texto,
            sistema::drop::drop_enlace,
            sistema::drop::drop_enviar_portapapeles,
            sistema::drop::drop_enviar_captura,
            sistema::drop::drop_historial,
            sistema::drop::drop_abrir,
            sistema::drop::drop_mostrar,
            sistema::drop::drop_copiar,
            sistema::drop::drop_borrar_historial,
            sistema::drop::drop_abrir_carpeta,
            sistema::drop::drop_ajustes,
            sistema::drop::drop_ajustar,
            sistema::setup::setup_estado,
            sistema::setup::setup_mapa,
            sistema::setup::setup_guardar_mapa,
            sistema::escenas::escenas,
            sistema::escenas::guardar_escena,
            sistema::escenas::borrar_escena,
            sistema::escenas::activar_escena,
            sistema::hogar::hogar_estado,
            sistema::hogar::hogar_ejecutar,
            sistema::hogar::hogar_buscar,
            sistema::hogar::hogar_agregar,
            sistema::hogar::hogar_vincular,
            sistema::hogar::hogar_red,
            sistema::hogar::hogar_quitar,
            sistema::hogar::hogar_editar,
            sistema::hogar::hogar_guardar_grupo,
            sistema::hogar::hogar_borrar_grupo,
            sistema::hogar::hogar_guardar_habitacion,
            sistema::hogar::hogar_borrar_habitacion,
            sistema::hogar::hogar_mostrar_todo,
            sistema::hogar::hogar_tuya_conectar,
            sistema::hogar::hogar_tuya_desconectar,
            sistema::hogar::hogar_ha_conectar,
            sistema::hogar::hogar_ha_desconectar,
            sistema::actualizar::actualizacion_estado,
            sistema::actualizar::actualizacion_buscar,
            sistema::actualizar::actualizar_instalar,
            abrir_seccion,
        ])
        .setup(|app| {
            // Windows, recién instalado: abrir al iniciar sesión queda activo (lo demás ya viene encendido).
            sistema::primer_inicio::aplicar(app.handle());
            // Windows: otra copia de Nexo que se abre pide mostrar el panel de esta.
            let a = app.handle().clone();
            sistema::instancia::escuchar(move || {
                let b = a.clone();
                let _ = a.run_on_main_thread(move || abrir_panel(&b, None));
            });
            // Pantallas: al cambiar el brillo con las teclas se muestra el aviso en ese monitor.
            // Cualquier cambio de brillo llega a los widgets de brillo ("brillo-cambio").
            // Cada monitor externo arranca con su brillo mínimo guardado (sin dato, 10 %).
            let (manejador, otro) = (app.handle().clone(), app.handle().clone());
            let avisos = Avisos {
                tecla: Box::new(move |id, nivel| {
                    sistema::aviso::mostrar(&manejador, id, nivel);
                    // Y el "Brillo: 40 %" del menú del ícono (los toques seguidos se juntan).
                    sistema::bandeja::actualizar(&manejador);
                }),
                cambio: Box::new(move |id, nivel| {
                    let _ = otro.emit("brillo-cambio", CambioBrillo { id, nivel });
                }),
            };
            let guardados = sistema::ajustes::cargar(app.handle());
            let minimos = guardados.minimos_brillo.into_iter().collect();
            let teclas = guardados.teclas_pantallas.into_iter().collect();
            let pantallas = Arc::new(Pantallas::iniciar(avisos, minimos, teclas));
            app.manage(pantallas.clone());
            // Remoto (Mac): vigía de la sesión de RustDesk, atajos de Windows, captura con F13 y portapapeles liviano.
            sistema::remoto::iniciar(app.handle());
            // Mac: la pantalla virtual para RustDesk vuelve si quedó activa.
            sistema::pantalla_virtual::iniciar(app.handle());
            // Nexo Drop: el receptor escucha solo en la dirección de Tailscale (puerto 47821).
            sistema::drop::iniciar(app.handle());
            // Windows: busca versiones nuevas en la Mac a los 20 s y cada 30 min.
            sistema::actualizar::iniciar(app.handle());
            // Windows: los atajos de brillo elegidos y si las teclas del portátil pasan al monitor bajo el mouse.
            sistema::teclas::configurar(guardados.atajo_bajar, guardados.atajo_subir, guardados.teclas_portatil);
            if guardados.teclas_brillo && sistema::teclas::permiso_concedido() {
                let _ = sistema::teclas::activar(pantallas, true);
            }

            // Historial de uso y ajustes automáticos: el vigía revisa el Bluetooth cada 20 s.
            let historial = Arc::new(Historial::cargar(app.handle()));
            app.manage(historial.clone());
            iniciar_vigia(app.handle().clone(), historial);
            // Botones de brillo de los widgets nativos de la Mac.
            sistema::widgets_mac::escuchar(app.handle().clone());

            // El ícono y su menú de accesos rápidos (captura, escenas, sonido, brillo, aparatos, remoto).
            // Si no se puede crear, Nexo sigue: el panel y el widget abren igual.
            if let Err(e) = sistema::bandeja::crear(app.handle()) {
                eprintln!("Nexo: no se pudo crear el ícono ({e})");
            }

            abrir_panel(app.handle(), None);
            sistema::widget::iniciar(app.handle());
            sistema::widget_brillo::iniciar(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        // Sin abortar: queda anotado y, en Windows, se dice en un cuadro (por ejemplo, si falta WebView2).
        .unwrap_or_else(|e| sistema::fallos::fatal(&e.to_string()));

    app.run(|app, evento| {
        #[cfg(not(target_os = "macos"))]
        let _ = app;
        match evento {
            // Se cerró la última ventana: la app no se cierra, solo queda el ícono.
            RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
            // Se cerró el panel: Nexo sale del Dock (el widget y el ícono siguen).
            #[cfg(target_os = "macos")]
            RunEvent::WindowEvent { label, event: tauri::WindowEvent::Destroyed, .. } if label == "panel" => {
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            // Doble clic en la app cuando ya está corriendo.
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => abrir_panel(app, None),
            _ => {}
        }
    });
}
