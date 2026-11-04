//! Otros sistemas: no se mide nada (Nexo es para Mac y Windows).

use super::Muestra;

pub struct Lector;

impl Lector {
    pub fn nuevo() -> Self {
        Lector
    }

    pub fn medir(&mut self) -> Muestra {
        Muestra::default()
    }
}
