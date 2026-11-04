//! Nexo Drop en otros sistemas: Nexo es para Mac y Windows; aquí solo se reciben archivos.

use super::Copiado;
use std::path::Path;

pub fn poner_texto(_texto: &str) -> bool {
    false
}

pub fn poner_imagen(_ruta: &Path) -> bool {
    false
}

pub fn leer_copiado() -> Option<Copiado> {
    None
}

pub fn miniatura(_ruta: &Path) -> Option<String> {
    None
}

pub fn abrir(_ruta: &Path) -> Result<(), String> {
    Err("Todavía no disponible en este sistema".into())
}

pub fn mostrar(_ruta: &Path) -> Result<(), String> {
    Err("Todavía no disponible en este sistema".into())
}

pub fn abrir_enlace(_url: &str) -> Result<(), String> {
    Err("Todavía no disponible en este sistema".into())
}

pub fn nombre_equipo() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "Equipo".into())
}
