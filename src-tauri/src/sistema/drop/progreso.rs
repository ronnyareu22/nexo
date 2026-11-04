//! Nexo Drop: el avance de lo que se envía o se recibe. Avisa a las ventanas con "drop-progreso" unas 10 veces
//! por segundo como mucho, y recuerda lo que está en curso para la interfaz que se abre a mitad de camino.

use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const CADA: Duration = Duration::from_millis(100);

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Progreso {
    pub lote: String,
    /// "enviado" (sale de aquí) o "recibido" (llega aquí).
    pub direccion: &'static str,
    /// El otro equipo.
    pub equipo: String,
    /// El archivo en curso (desde 0) y cuántos son.
    pub indice: u32,
    pub total: u32,
    /// Su ruta relativa ("Fotos/a.jpg").
    pub nombre: String,
    /// Bytes de este archivo.
    pub enviados: u64,
    pub tamano: u64,
    /// Bytes de todo el envío (al recibir, el total no se sabe: 0).
    pub enviados_lote: u64,
    pub tamano_lote: u64,
    /// Bytes por segundo, suavizado.
    pub velocidad: f64,
    /// "esperando", "transfiriendo", "listo", "cancelado" o "error".
    pub estado: &'static str,
    pub error: Option<String>,
}

/// Lo que está en curso, con la hora del último aviso.
static EN_CURSO: Mutex<Vec<(Instant, Progreso)>> = Mutex::new(Vec::new());

/// Lo que está en curso ahora (envíos en cola incluidos). Lo que llega y lleva 15 s quieto ya no cuenta:
/// el otro lado dejó de mandar.
pub fn en_curso() -> Vec<Progreso> {
    let mut lista = EN_CURSO.lock().unwrap_or_else(|e| e.into_inner());
    lista.retain(|(t, p)| p.direccion == "enviado" || t.elapsed() < Duration::from_secs(15));
    lista.iter().map(|(_, p)| p.clone()).collect()
}

fn publicar(app: &AppHandle, p: &Progreso) {
    {
        let mut lista = EN_CURSO.lock().unwrap_or_else(|e| e.into_inner());
        let terminado = matches!(p.estado, "listo" | "cancelado" | "error");
        match lista.iter().position(|(_, x)| x.lote == p.lote && x.direccion == p.direccion) {
            Some(i) if terminado => {
                lista.remove(i);
            }
            Some(i) => lista[i] = (Instant::now(), p.clone()),
            None if !terminado => lista.push((Instant::now(), p.clone())),
            None => {}
        }
    }
    let _ = app.emit("drop-progreso", p);
}

/// Cuenta los bytes y avisa el avance sin pasar de 10 avisos por segundo.
pub struct Medidor {
    pub p: Progreso,
    /// El último aviso (para no pasar de 10 por segundo).
    aviso: Instant,
    /// Desde cuándo y desde cuántos bytes se mide la velocidad.
    ventana: Instant,
    bytes_ventana: u64,
}

impl Medidor {
    pub fn nuevo(p: Progreso) -> Self {
        let ahora = Instant::now();
        Medidor { p, aviso: ahora, ventana: ahora, bytes_ventana: 0 }
    }

    /// Suma bytes y avisa si ya pasó un décimo de segundo.
    pub fn sumar(&mut self, app: &AppHandle, n: u64) {
        self.p.enviados += n;
        self.p.enviados_lote += n;
        if self.aviso.elapsed() >= CADA {
            self.emitir(app);
        }
    }

    /// Avisa ya (por ejemplo, al empezar cada archivo).
    pub fn emitir(&mut self, app: &AppHandle) {
        let lapso = self.ventana.elapsed();
        if lapso >= CADA {
            let nuevos = self.p.enviados_lote.saturating_sub(self.bytes_ventana);
            let instante = nuevos as f64 / lapso.as_secs_f64();
            self.p.velocidad = if self.p.velocidad <= 0.0 { instante } else { self.p.velocidad * 0.7 + instante * 0.3 };
            self.ventana = Instant::now();
            self.bytes_ventana = self.p.enviados_lote;
        }
        self.aviso = Instant::now();
        publicar(app, &self.p);
    }

    /// El final: "listo", "cancelado" o "error" (con su motivo).
    pub fn terminar(&mut self, app: &AppHandle, estado: &'static str, error: Option<String>) {
        self.p.estado = estado;
        self.p.error = error;
        publicar(app, &self.p);
    }
}
