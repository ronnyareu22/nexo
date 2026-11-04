//! Manejar la Mac desde Nexo en Windows (o desde otra Mac, para probar), por el receptor de XDrop.
//!
//! - En la Mac, `drop/servidor.rs` atiende (con su seguridad: solo equipos del mismo dueño en Tailscale):
//!   - `GET /nexo/remoto/estado`: el equipo, la pantalla virtual, la sesión, el modo trabajo y el reposo.
//!   - `POST /nexo/remoto/pantalla-virtual` `{activar, ancho?, alto?}`: la enciende, la apaga o le cambia la resolución.
//!   - `POST /nexo/remoto/ajustar` `{crearAlEmpezar?, quitarAlTerminar?, modoTrabajo?}`.
//!   Usan lo mismo que el panel de la Mac (`pantalla_virtual` y los ajustes del remoto) y devuelven cómo quedó todo.
//! - Del lado que pregunta: `remoto_mac_estado`, `remoto_mac_pantalla_virtual` y `remoto_mac_ajustar`, con plazos cortos
//!   y errores en palabras. La interfaz elige la Mac entre los destinos de XDrop.

use super::{CambioRemoto, MODO_TRABAJO, aplicar_cambio, sesion_actual};
use crate::sistema::{ajustes, drop, pantalla_virtual, setup};
use serde::{Deserialize, Serialize};
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const PLAZO_CONECTAR: Duration = Duration::from_secs(3);
const PLAZO_LEER: Duration = Duration::from_secs(3);
/// Encender o cambiar la pantalla virtual tarda más: la Mac espera hasta 3 s a que macOS la muestre.
const PLAZO_PANTALLA: Duration = Duration::from_secs(12);
const MAX_RESPUESTA: u64 = 64 * 1024;
const MAX_NOMBRE: usize = 64;

const NO_RESPONDE: &str = "Tu Mac no responde: revisa que Nexo esté abierto allá.";
const TARDO: &str = "Tu Mac tardó en responder. Revisa en un momento si el cambio se hizo.";

/// La pantalla virtual de la Mac, como se ve desde Windows.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMac {
    pub activa: bool,
    pub ancho: u32,
    pub alto: u32,
    /// Ese macOS deja crearla.
    pub disponible: bool,
    pub quitar_al_terminar: bool,
    pub crear_al_empezar: bool,
}

/// La sesión de RustDesk que está entrando a la Mac.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SesionMac {
    pub activa: bool,
    /// Directa por Tailscale.
    pub directa: bool,
    /// Por el relé de RustDesk (más lenta).
    #[serde(default)]
    pub rele: bool,
    /// El nombre del PC en Tailscale, si se sabe.
    pub equipo: Option<String>,
}

/// Lo que responde `GET /nexo/remoto/estado` (y los dos POST, ya con el cambio hecho).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EstadoMac {
    /// La Mac en Tailscale ("demo-sige").
    pub equipo: String,
    /// "mac", "windows" u "otro".
    pub plataforma: String,
    pub pantalla_virtual: VirtualMac,
    pub sesion: SesionMac,
    pub modo_trabajo: bool,
    /// "Evitar que la Mac se duerma": sin él, el modo trabajo no la mantiene despierta.
    pub evitar_reposo: bool,
}

/// Lo que cambia `POST /nexo/remoto/ajustar`. Lo que no viene, queda igual.
#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct CambioMac {
    pub crear_al_empezar: Option<bool>,
    pub quitar_al_terminar: Option<bool>,
    pub modo_trabajo: Option<bool>,
}

/// El cuerpo de `POST /nexo/remoto/pantalla-virtual`.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PedidoPantalla {
    activar: bool,
    ancho: Option<u32>,
    alto: Option<u32>,
}

// ---------------------------------------------------------------- En la Mac: lo que atiende el receptor

fn estado(app: &AppHandle) -> EstadoMac {
    let v = pantalla_virtual::estado(app);
    let s = sesion_actual();
    EstadoMac {
        equipo: drop::nombre_propio(),
        plataforma: drop::PLATAFORMA.into(),
        pantalla_virtual: VirtualMac {
            activa: v.activa,
            ancho: v.ancho,
            alto: v.alto,
            disponible: v.disponible,
            quitar_al_terminar: v.quitar_al_terminar,
            crear_al_empezar: v.crear_al_empezar,
        },
        sesion: SesionMac { activa: s.activa, directa: s.directa, rele: s.rele, equipo: s.equipo },
        modo_trabajo: MODO_TRABAJO.load(Ordering::Relaxed),
        evitar_reposo: ajustes::cargar(app).remoto.evitar_reposo,
    }
}

fn a_json(e: &EstadoMac) -> serde_json::Value {
    serde_json::to_value(e).unwrap_or(serde_json::Value::Null)
}

/// `GET /nexo/remoto/estado`.
pub fn estado_json(app: &AppHandle) -> serde_json::Value {
    a_json(&estado(app))
}

/// `POST /nexo/remoto/pantalla-virtual` y `POST /nexo/remoto/ajustar`: hace el cambio y devuelve cómo quedó todo.
/// Si falla, el código HTTP y el motivo en palabras. Corre en el hilo del pedido (nunca en el principal).
pub fn atender(app: &AppHandle, ruta: &str, cuerpo: &[u8]) -> Result<serde_json::Value, (u16, String)> {
    match ruta {
        "/nexo/remoto/pantalla-virtual" => {
            let p: PedidoPantalla = serde_json::from_slice(cuerpo)
                .map_err(|_| (400, "Se esperaba {\"activar\": true, \"ancho\": 1920, \"alto\": 1080}.".to_string()))?;
            // Solo las resoluciones de la lista, y el ancho con el alto.
            match (p.ancho, p.alto) {
                (Some(a), Some(h)) => {
                    pantalla_virtual::resolucion(a, h).map_err(|e| (400, e))?;
                }
                (None, None) => {}
                _ => return Err((400, "Falta el ancho o el alto de la pantalla virtual.".into())),
            }
            if p.activar && !pantalla_virtual::disponible() {
                return Err((400, "Este macOS no deja crear pantallas virtuales.".into()));
            }
            pantalla_virtual::usar(app, p.activar, p.ancho, p.alto).map_err(|e| (500, e))?;
        }
        "/nexo/remoto/ajustar" => {
            let c: CambioMac = serde_json::from_slice(cuerpo).map_err(|_| (400, "Se esperaba {\"modoTrabajo\": true}.".to_string()))?;
            if c.crear_al_empezar.is_some() || c.quitar_al_terminar.is_some() {
                let cambio = pantalla_virtual::CambioPantallaVirtual {
                    quitar_al_terminar: c.quitar_al_terminar,
                    crear_al_empezar: c.crear_al_empezar,
                };
                pantalla_virtual::ajustar(app, cambio).map_err(|e| (500, e))?;
            }
            if let Some(v) = c.modo_trabajo {
                aplicar_cambio(app, CambioRemoto { modo_trabajo: Some(v), ..Default::default() }).map_err(|e| (500, e))?;
                // El panel de la Mac se pone al día enseguida (lo escucha como un cambio de la sesión).
                let _ = app.emit("remoto-cambio", sesion_actual());
            }
        }
        _ => return Err((404, "No existe.".into())),
    }
    Ok(a_json(&estado(app)))
}

// ---------------------------------------------------------------- Del lado que pregunta (Windows)

/// Un nombre de la respuesta: sin caracteres de control y corto.
fn limpio(texto: &str) -> String {
    texto.chars().filter(|c| !c.is_control()).take(MAX_NOMBRE).collect::<String>().trim().to_string()
}

fn limpiar(mut e: EstadoMac) -> EstadoMac {
    e.equipo = limpio(&e.equipo);
    e.plataforma = match e.plataforma.to_ascii_lowercase().as_str() {
        "mac" | "macos" => "mac",
        "windows" => "windows",
        _ => "otro",
    }
    .into();
    e.sesion.equipo = e.sesion.equipo.as_deref().map(limpio).filter(|n| !n.is_empty());
    e
}

/// Le hace un pedido a la Mac y devuelve su estado. `plazo` es cuánto se espera la respuesta; `tarde`, qué decir si
/// no llega a tiempo.
fn pedir(
    ip: Ipv4Addr,
    metodo: &str,
    ruta: &str,
    cuerpo: Option<&serde_json::Value>,
    plazo: Duration,
    tarde: &str,
) -> Result<EstadoMac, String> {
    let mut s = TcpStream::connect_timeout(&SocketAddr::from((ip, drop::PUERTO)), PLAZO_CONECTAR)
        .map_err(|_| NO_RESPONDE.to_string())?;
    let _ = s.set_read_timeout(Some(plazo));
    let _ = s.set_write_timeout(Some(PLAZO_LEER));
    let texto = cuerpo.map(|c| c.to_string()).unwrap_or_default();
    let pedido = format!(
        "{metodo} {ruta} HTTP/1.1\r\nHost: {ip}:{}\r\nUser-Agent: Nexo/{}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{texto}",
        drop::PUERTO,
        env!("CARGO_PKG_VERSION"),
        texto.len()
    );
    s.write_all(pedido.as_bytes()).map_err(|_| NO_RESPONDE.to_string())?;
    let mut datos = Vec::new();
    let lectura = Read::by_ref(&mut s).take(MAX_RESPUESTA).read_to_end(&mut datos);
    let Some((codigo, cuerpo)) = setup::separar_respuesta(&datos) else {
        return Err(match lectura {
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => tarde.into(),
            _ => NO_RESPONDE.into(),
        });
    };
    match codigo {
        200 => serde_json::from_slice::<EstadoMac>(cuerpo)
            .map(limpiar)
            .map_err(|_| "La respuesta de tu Mac no se entiende: actualiza Nexo en los dos equipos.".into()),
        404 => Err("Tu Mac tiene una versión anterior de Nexo: actualízala para manejarla desde aquí.".into()),
        403 => Err("Tu Mac no aceptó el pedido: revisa que los dos equipos sean tuyos en Tailscale.".into()),
        _ => Err(serde_json::from_slice::<serde_json::Value>(cuerpo)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(limpio_largo))
            .unwrap_or_else(|| format!("Tu Mac no pudo hacerlo ({codigo})."))),
    }
}

/// El motivo que mandó la Mac, sin caracteres de control y de largo razonable.
fn limpio_largo(texto: &str) -> String {
    texto.chars().filter(|c| !c.is_control()).take(300).collect()
}

/// La dirección de la Mac, si es un equipo del dueño en Tailscale (o este mismo, para probar).
fn direccion(ip: &str) -> Result<Ipv4Addr, String> {
    drop::destino_valido(ip).map(|(ip, _)| ip)
}

async fn en_segundo_plano(f: impl FnOnce() -> Result<EstadoMac, String> + Send + 'static) -> Result<EstadoMac, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- Comandos

/// Cómo está la Mac: su pantalla virtual, la sesión de RustDesk, el modo trabajo y el reposo.
#[tauri::command]
pub async fn remoto_mac_estado(ip: String) -> Result<EstadoMac, String> {
    en_segundo_plano(move || pedir(direccion(&ip)?, "GET", "/nexo/remoto/estado", None, PLAZO_LEER, NO_RESPONDE)).await
}

/// Enciende (`activar`), apaga o cambia de resolución la pantalla virtual de la Mac. Devuelve cómo quedó.
#[tauri::command]
pub async fn remoto_mac_pantalla_virtual(
    ip: String,
    activar: bool,
    ancho: Option<u32>,
    alto: Option<u32>,
) -> Result<EstadoMac, String> {
    en_segundo_plano(move || {
        if let (Some(a), Some(h)) = (ancho, alto) {
            pantalla_virtual::resolucion(a, h)?;
        }
        let cuerpo = serde_json::json!({ "activar": activar, "ancho": ancho, "alto": alto });
        pedir(direccion(&ip)?, "POST", "/nexo/remoto/pantalla-virtual", Some(&cuerpo), PLAZO_PANTALLA, TARDO)
    })
    .await
}

/// Cambia en la Mac "Crear al empezar", "Quitar al terminar" o el modo trabajo. Devuelve cómo quedó.
#[tauri::command]
pub async fn remoto_mac_ajustar(ip: String, cambio: CambioMac) -> Result<EstadoMac, String> {
    en_segundo_plano(move || {
        let cuerpo = serde_json::to_value(cambio).map_err(|e| e.to_string())?;
        pedir(direccion(&ip)?, "POST", "/nexo/remoto/ajustar", Some(&cuerpo), PLAZO_LEER, TARDO)
    })
    .await
}
