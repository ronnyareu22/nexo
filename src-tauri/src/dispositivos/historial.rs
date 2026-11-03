//! Historial de uso: cuándo se conecta y desconecta cada aparato y cómo baja su batería,
//! más sus ajustes automáticos y el nombre que la persona le pone dentro de Nexo (alias).
//! Lo alimenta un vigía que revisa el Bluetooth cada 20 s (tarda ~60 ms).
//! Se guarda en historial.json, en la carpeta de la app.

use super::Dispositivo;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const SEMANA: i64 = 7 * 24 * 3600;
const DIA: i64 = 24 * 3600;
/// Una muestra de batería cada 5 min como mucho (o antes si cambió el nivel).
const CADA_MUESTRA: i64 = 300;
const UMBRAL_BATERIA_BAJA: u8 = 20;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Registro {
    /// Sesiones de uso [inicio, fin] en segundos Unix; fin vacío si sigue conectado. Última semana.
    pub sesiones: Vec<(i64, Option<i64>)>,
    /// Muestras de batería [momento, nivel] de las últimas 24 h.
    pub bateria: Vec<(i64, u8)>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Automatico {
    /// Al conectarse, el sonido de la Mac sale por este aparato.
    pub usar_salida_al_conectar: bool,
    /// Notificación de macOS cuando la batería baja del 20 %.
    pub avisar_bateria_baja: bool,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Datos {
    registros: HashMap<String, Registro>,
    automatico: HashMap<String, Automatico>,
    /// Nombre que la persona le puso a cada aparato dentro de Nexo. El nombre Bluetooth
    /// del aparato no se puede cambiar desde la Mac, así que el alias vive solo aquí.
    alias: HashMap<String, String>,
    /// Última vez que el vigía revisó; sirve para cerrar sesiones que quedaron abiertas al salir de Nexo.
    ultima_revision: i64,
}

pub enum Evento {
    Conectado { id: String },
    BateriaBaja { nombre: String, nivel: u8 },
}

pub struct Historial {
    datos: Mutex<Datos>,
    ruta: Option<PathBuf>,
    /// Estado anterior de cada aparato (solo en memoria), para detectar cuándo se conecta.
    antes: Mutex<HashMap<String, bool>>,
    /// Aparatos a los que ya se avisó de batería baja en esta descarga.
    avisados: Mutex<HashSet<String>>,
}

pub fn ahora() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Historial {
    pub fn cargar(app: &AppHandle) -> Self {
        let ruta = app.path().app_config_dir().ok().map(|d| d.join("historial.json"));
        let mut datos: Datos = ruta
            .as_ref()
            .and_then(|r| std::fs::read(r).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        // Sesiones que quedaron abiertas cuando Nexo se cerró: terminan en la última revisión.
        let fin = datos.ultima_revision;
        for r in datos.registros.values_mut() {
            if let Some(ultima) = r.sesiones.last_mut() {
                if ultima.1.is_none() {
                    ultima.1 = Some(fin.max(ultima.0));
                }
            }
        }
        Self { datos: Mutex::new(datos), ruta, antes: Mutex::new(HashMap::new()), avisados: Mutex::new(HashSet::new()) }
    }

    pub fn registro(&self, id: &str) -> Registro {
        self.datos.lock().ok().and_then(|d| d.registros.get(id).cloned()).unwrap_or_default()
    }

    pub fn automatico(&self, id: &str) -> Automatico {
        self.datos.lock().ok().and_then(|d| d.automatico.get(id).cloned()).unwrap_or_default()
    }

    pub fn guardar_automatico(&self, id: &str, a: Automatico) -> Automatico {
        if let Ok(mut d) = self.datos.lock() {
            d.automatico.insert(id.to_owned(), a.clone());
        }
        self.guardar();
        a
    }

    /// Pone el alias de un aparato. Vacío (o sin alias) lo quita y vuelve el nombre del aparato.
    pub fn poner_alias(&self, id: &str, alias: Option<String>) {
        let alias = alias.map(|a| a.trim().to_owned()).filter(|a| !a.is_empty());
        if let Ok(mut d) = self.datos.lock() {
            match alias {
                Some(a) => d.alias.insert(id.to_owned(), a),
                None => d.alias.remove(id),
            };
        }
        self.guardar();
    }

    /// Completa el alias de cada aparato de la lista.
    pub fn poner_alias_en(&self, lista: &mut [Dispositivo]) {
        if let Ok(d) = self.datos.lock() {
            for x in lista {
                x.alias = d.alias.get(&x.id).cloned();
            }
        }
    }

    /// Borra las sesiones de uso y las muestras de batería. Los ajustes automáticos y los alias se quedan.
    pub fn borrar(&self) {
        if let Ok(mut d) = self.datos.lock() {
            d.registros.clear();
        }
        self.guardar();
    }

    /// Lo llama el vigía con la lista actual. Devuelve lo que pasó (para los ajustes automáticos).
    pub fn registrar(&self, lista: &[Dispositivo]) -> Vec<Evento> {
        let t = ahora();
        let mut eventos = Vec::new();
        let mut cambio = false;
        let (Ok(mut datos), Ok(mut antes), Ok(mut avisados)) = (self.datos.lock(), self.antes.lock(), self.avisados.lock())
        else {
            return eventos;
        };
        datos.ultima_revision = t;

        for d in lista {
            let automatico = datos.automatico.get(&d.id).cloned().unwrap_or_default();
            let alias = datos.alias.get(&d.id).cloned();
            let r = datos.registros.entry(d.id.clone()).or_default();
            let abierta = r.sesiones.last().is_some_and(|s| s.1.is_none());

            if d.conectado && !abierta {
                r.sesiones.push((t, None));
                cambio = true;
                // Solo cuenta como "se acaba de conectar" si antes lo vimos desconectado.
                if antes.get(&d.id) == Some(&false) {
                    eventos.push(Evento::Conectado { id: d.id.clone() });
                }
            } else if !d.conectado && abierta {
                if let Some(s) = r.sesiones.last_mut() {
                    s.1 = Some(t);
                }
                cambio = true;
            }

            if let (true, Some(nivel)) = (d.conectado, d.bateria) {
                let ultima = r.bateria.last().copied();
                if ultima.is_none_or(|(m, n)| t - m >= CADA_MUESTRA || n != nivel) {
                    r.bateria.push((t, nivel));
                    cambio = true;
                }
                if nivel <= UMBRAL_BATERIA_BAJA && automatico.avisar_bateria_baja && avisados.insert(d.id.clone()) {
                    // El aviso usa el nombre que la persona reconoce: su alias, si tiene.
                    let nombre = alias.unwrap_or_else(|| d.nombre.clone());
                    eventos.push(Evento::BateriaBaja { nombre, nivel });
                } else if nivel > UMBRAL_BATERIA_BAJA + 5 {
                    avisados.remove(&d.id);
                }
            }

            // Solo se guarda lo reciente.
            r.sesiones.retain(|s| s.1.is_none_or(|fin| t - fin < SEMANA));
            r.bateria.retain(|(m, _)| t - m < DIA);
            antes.insert(d.id.clone(), d.conectado);
        }

        drop((datos, antes, avisados));
        if cambio {
            self.guardar();
        }
        eventos
    }

    fn guardar(&self) {
        let (Some(ruta), Ok(datos)) = (&self.ruta, self.datos.lock()) else { return };
        if let Some(carpeta) = ruta.parent() {
            let _ = std::fs::create_dir_all(carpeta);
        }
        if let Ok(json) = serde_json::to_vec(&*datos) {
            let _ = std::fs::write(ruta, json);
        }
    }
}
