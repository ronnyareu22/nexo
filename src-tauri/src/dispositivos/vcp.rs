//! Ajustes del monitor por DDC/CI (códigos VCP de MCCS) y el brillo mínimo.
//!
//! - Solo lógica pura: el hilo de pantallas de cada sistema le pasa cómo leer el monitor.
//! - Las capacidades se leen una vez por monitor y quedan en su `Memoria`. Nexo solo muestra lo que el monitor
//!   dice que tiene; si no da su lista, prueba los controles continuos leyéndolos.
//! - Brillo mínimo: muchos monitores parpadean (PWM) con el brillo en 0. El 0 % de Nexo es el mínimo de cada
//!   monitor y el 100 % es su máximo, así nunca se manda un valor por debajo del mínimo.

use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

pub const BRILLO: u8 = 0x10;
pub const FABRICA: u8 = 0x04;
pub const CONTRASTE: u8 = 0x12;
pub const COLOR: u8 = 0x14;
pub const ROJO: u8 = 0x16;
pub const VERDE: u8 = 0x18;
pub const AZUL: u8 = 0x1A;
pub const ENTRADA: u8 = 0x60;
pub const VOLUMEN: u8 = 0x62;
pub const NITIDEZ: u8 = 0x87;
pub const SILENCIO: u8 = 0x8D;
pub const MODO_IMAGEN: u8 = 0xDC;

/// Lo que Nexo muestra si el monitor lo tiene. El brillo va aparte (con su mínimo).
const CONOCIDOS: [u8; 11] = [FABRICA, CONTRASTE, COLOR, ROJO, VERDE, AZUL, ENTRADA, VOLUMEN, NITIDEZ, SILENCIO, MODO_IMAGEN];
/// Los que se eligen de la lista que da el monitor.
const DE_LISTA: [u8; 3] = [COLOR, ENTRADA, MODO_IMAGEN];
/// Si el monitor no da su lista, se prueban estos leyéndolos (los de lista no: sin ella no se sabe qué ofrecer).
const SE_PRUEBAN: [u8; 7] = [CONTRASTE, ROJO, VERDE, AZUL, NITIDEZ, VOLUMEN, SILENCIO];

/// Brillo mínimo de fábrica de los monitores externos (la pantalla integrada no tiene).
pub const MINIMO_EXTERNO: u8 = 10;
/// Hasta dónde se puede subir el mínimo.
pub const MINIMO_TOPE: u8 = 40;

/// Un ajuste que el monitor tiene. `valores`: las opciones, solo en los que se eligen de una lista.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Capacidad {
    pub codigo: u8,
    pub valores: Vec<u8>,
}

/// Valor actual y máximo de un código, en unidades del monitor.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Valor {
    pub actual: u16,
    pub maximo: u16,
}

/// Lo que ve el panel en "Ajustes del monitor".
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AjustesMonitor {
    /// Brillo mínimo (0–40 % del monitor): el 0 % de Nexo.
    pub minimo: u8,
    pub capacidades: Vec<Capacidad>,
    /// Código → valor actual y máximo (solo de lo que respondió).
    pub valores: BTreeMap<u8, Valor>,
    /// El monitor dio su lista de capacidades (si no, solo están los controles que respondieron).
    pub con_lista: bool,
}

/// Los códigos del monitor y si salieron de su lista de capacidades o de probarlos.
#[derive(Clone, Debug, PartialEq)]
pub struct Lista {
    pub codigos: Vec<(u8, Vec<u8>)>,
    pub de_capacidades: bool,
}

/// Lo que el hilo de pantallas recuerda de cada monitor (se pierde al desconectarlo).
#[derive(Default, Debug)]
pub struct Memoria {
    /// None: todavía no se leyó.
    pub lista: Option<Lista>,
    /// Máximo de cada código, de la última lectura.
    pub maximos: HashMap<u8, u32>,
}

// ---------------------------------------------------------------- Brillo mínimo

/// El mínimo en unidades del monitor.
pub fn piso(minimo: u8, maximo: u32) -> u32 {
    ((minimo.min(MINIMO_TOPE) as f64 / 100.0) * maximo as f64).round() as u32
}

/// De la escala de Nexo (0–100) a unidades del monitor: el 0 es el mínimo y el 100 el máximo.
pub fn a_crudo(porcentaje: u8, minimo: u8, maximo: u32) -> u32 {
    let p = piso(minimo, maximo).min(maximo);
    let parte = porcentaje.min(100) as f64 / 100.0;
    (p as f64 + parte * (maximo - p) as f64).round() as u32
}

/// De unidades del monitor a la escala de Nexo. Por debajo del mínimo (cambiado en el menú del monitor) es 0.
pub fn a_porcentaje(crudo: u32, minimo: u8, maximo: u32) -> u8 {
    let p = piso(minimo, maximo);
    if maximo <= p {
        return 0;
    }
    ((crudo.saturating_sub(p) as f64 / (maximo - p) as f64) * 100.0).round().clamp(0.0, 100.0) as u8
}

/// Un toque de tecla sube o baja 1/16 de la escala de Nexo (como las teclas de la Mac).
pub fn paso(porcentaje: u8, pasos: i32) -> u8 {
    (porcentaje as f64 + pasos as f64 * 100.0 / 16.0).round().clamp(0.0, 100.0) as u8
}

// ---------------------------------------------------------------- Capacidades

/// Los códigos de `vcp(...)` en la cadena de capacidades, con sus valores entre paréntesis.
/// "…vcp(02 10 12 14(05 08 0B) 60(0F 11))…" → [(0x02, []), (0x10, []), (0x12, []), (0x14, [5, 8, 11]), (0x60, [15, 17])].
/// Algunos monitores no ponen espacios ("vcp(021012)"): se cortan de a dos cifras.
pub fn codigos_vcp(texto: &str) -> Option<Vec<(u8, Vec<u8>)>> {
    let bajo = texto.to_ascii_lowercase();
    let b = bajo.as_bytes();
    // "vcp(" suelto: no el final de otra palabra como "vcpname(".
    let mut desde = 0;
    let inicio = loop {
        let pos = desde + bajo[desde..].find("vcp(")?;
        let antes = pos.checked_sub(1).map(|i| b[i]);
        if antes.is_none_or(|c| !c.is_ascii_alphanumeric() && c != b'_') {
            break pos + 4;
        }
        desde = pos + 4;
    };

    let mut lista: Vec<(u8, Vec<u8>)> = Vec::new();
    let mut nivel = 0usize;
    let mut ficha = String::new();
    let agregar = |ficha: &mut String, nivel: usize, lista: &mut Vec<(u8, Vec<u8>)>| {
        if ficha.len() % 2 == 0 {
            for par in ficha.as_bytes().chunks(2) {
                let Some(n) = std::str::from_utf8(par).ok().and_then(|s| u8::from_str_radix(s, 16).ok()) else { continue };
                if nivel == 0 {
                    lista.push((n, Vec::new()));
                } else if let Some((_, valores)) = lista.last_mut() {
                    if !valores.contains(&n) {
                        valores.push(n);
                    }
                }
            }
        }
        ficha.clear();
    };
    for &c in &b[inicio..] {
        if c.is_ascii_hexdigit() {
            ficha.push(c as char);
            continue;
        }
        agregar(&mut ficha, nivel, &mut lista);
        match c {
            b'(' => nivel += 1,
            b')' if nivel == 0 => break,
            b')' => nivel -= 1,
            _ => {}
        }
    }
    agregar(&mut ficha, nivel, &mut lista);
    Some(lista)
}

/// Lo que Nexo sabe mostrar de esa lista. Los de lista sin opciones no se pueden ofrecer.
fn conocidos(codigos: &[(u8, Vec<u8>)]) -> Vec<(u8, Vec<u8>)> {
    let mut salida: Vec<(u8, Vec<u8>)> = Vec::new();
    for (codigo, valores) in codigos {
        if !CONOCIDOS.contains(codigo) || salida.iter().any(|(c, _)| c == codigo) {
            continue;
        }
        if DE_LISTA.contains(codigo) {
            if valores.is_empty() {
                continue;
            }
            salida.push((*codigo, valores.clone()));
        } else {
            salida.push((*codigo, Vec::new()));
        }
    }
    salida
}

/// Valor de un código. En los de lista y el silencio cuenta solo el byte bajo (hay monitores que ensucian el alto).
/// Un control continuo con máximo 0 no sirve: se toma como que no respondió.
fn valor_de(codigo: u8, (actual, maximo): (u32, u32)) -> Option<Valor> {
    let de_lista = DE_LISTA.contains(&codigo) || codigo == SILENCIO;
    if !de_lista && maximo == 0 {
        return None;
    }
    let actual = if de_lista { actual & 0xFF } else { actual.min(maximo) };
    Some(Valor { actual: actual.min(u16::MAX as u32) as u16, maximo: maximo.min(u16::MAX as u32) as u16 })
}

/// Lee los ajustes del monitor. `capacidades` da la cadena de capacidades (solo se pide la primera vez) y
/// `leer` el valor actual y el máximo de un código (con su reintento). Recuerda la lista y los máximos.
pub fn leer_ajustes(
    memoria: &mut Memoria,
    minimo: u8,
    capacidades: impl FnOnce() -> Option<String>,
    mut leer: impl FnMut(u8) -> Option<(u32, u32)>,
) -> AjustesMonitor {
    let mut valores: BTreeMap<u8, Valor> = BTreeMap::new();
    let lista = match memoria.lista.take() {
        Some(l) => l,
        None => match capacidades().and_then(|t| codigos_vcp(&t)).filter(|c| !c.is_empty()) {
            Some(codigos) => Lista { codigos, de_capacidades: true },
            None => {
                let mut codigos = Vec::new();
                for codigo in SE_PRUEBAN {
                    if let Some(v) = leer(codigo).and_then(|v| valor_de(codigo, v)) {
                        valores.insert(codigo, v);
                        codigos.push((codigo, Vec::new()));
                    }
                }
                Lista { codigos, de_capacidades: false }
            }
        },
    };

    let mut capacidades = Vec::new();
    for (codigo, opciones) in conocidos(&lista.codigos) {
        // Restaurar es solo una orden: no tiene valor que leer.
        if codigo == FABRICA {
            capacidades.push(Capacidad { codigo, valores: Vec::new() });
            continue;
        }
        let valor = match valores.get(&codigo) {
            Some(v) => Some(*v),
            None => leer(codigo).and_then(|v| valor_de(codigo, v)),
        };
        // Si no respondió, no se muestra: cada control enseña su valor actual.
        let Some(v) = valor else { continue };
        valores.insert(codigo, v);
        capacidades.push(Capacidad { codigo, valores: opciones });
    }
    valores.retain(|c, _| capacidades.iter().any(|k| k.codigo == *c));
    memoria.maximos = valores.iter().map(|(c, v)| (*c, v.maximo as u32)).collect();
    let con_lista = lista.de_capacidades;
    // Si nada respondió y no había lista, quizá fue un tropiezo: se vuelve a probar la próxima vez.
    if lista.de_capacidades || !lista.codigos.is_empty() {
        memoria.lista = Some(lista);
    }
    AjustesMonitor { minimo, capacidades, valores, con_lista }
}

/// Revisa un cambio pedido por el panel y lo deja en unidades del monitor.
pub fn validar(memoria: &Memoria, codigo: u8, valor: u16) -> Result<u32, String> {
    if codigo == BRILLO || !CONOCIDOS.contains(&codigo) {
        return Err("Nexo no cambia ese ajuste del monitor.".into());
    }
    let lista = memoria.lista.as_ref().ok_or("El monitor no respondió. Vuelve a abrir sus ajustes.")?;
    let (_, opciones) = conocidos(&lista.codigos)
        .into_iter()
        .find(|(c, _)| *c == codigo)
        .ok_or("Este monitor no tiene ese ajuste.")?;
    if codigo == FABRICA {
        return Ok(1);
    }
    if DE_LISTA.contains(&codigo) {
        return match u8::try_from(valor) {
            Ok(v) if opciones.contains(&v) => Ok(v as u32),
            _ => Err("El monitor no ofrece esa opción.".into()),
        };
    }
    // Silencio: 1 = silenciado, 2 = con sonido.
    if codigo == SILENCIO {
        return if valor == 1 || valor == 2 { Ok(valor as u32) } else { Err("El monitor no ofrece esa opción.".into()) };
    }
    let maximo = memoria.maximos.get(&codigo).copied().unwrap_or(100);
    Ok((valor as u32).min(maximo))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn vcp_lee_la_lista_del_steren() {
        let t = "(prot(monitor)type(LCD)model(MNT-027)cmds(01 02 03 07 0C E3 F3)vcp(02 04 05 08 10 12 14(05 08 0B) 16 18 1A \
                 52 60(0F 11 12) 62 87 8D(01 02) AC AE B2 B6 C6 C8 CA CC(01 02 03 04 05 06 07 08 09 0A 0C 0D 11 12 14 1A 1E 1F) \
                 D6(01 04) DC(00 03 05) DF)mswhql(1)asset_eep(40)mccs_ver(2.2))";
        let l = codigos_vcp(t).unwrap_or_default();
        assert!(l.iter().any(|(c, v)| *c == 0x14 && v == &vec![5, 8, 11]));
        assert!(l.iter().any(|(c, v)| *c == 0x60 && v == &vec![0x0F, 0x11, 0x12]));
        assert!(l.iter().any(|(c, _)| *c == 0xDF));
        let k = conocidos(&l);
        let codigos: Vec<u8> = k.iter().map(|(c, _)| *c).collect();
        assert_eq!(codigos, [0x04, 0x12, 0x14, 0x16, 0x18, 0x1A, 0x60, 0x62, 0x87, 0x8D, 0xDC]);
    }

    #[test]
    fn vcp_sin_espacios_y_con_vcpname() {
        let t = "(vcpname(10(Brillo))vcp(0210121460(0f11))";
        let l = codigos_vcp(t).unwrap_or_default();
        assert_eq!(l, vec![(0x02, vec![]), (0x10, vec![]), (0x12, vec![]), (0x14, vec![]), (0x60, vec![0x0F, 0x11])]);
        // 0x14 sin opciones no se ofrece.
        assert_eq!(conocidos(&l).iter().map(|(c, _)| *c).collect::<Vec<_>>(), [0x12, 0x60]);
        assert!(codigos_vcp("(prot(monitor)type(lcd))").is_none());
    }

    #[test]
    fn minimo_ida_y_vuelta() {
        // 0 % de Nexo es el mínimo, 100 % el máximo, y nunca se baja del mínimo.
        assert_eq!(a_crudo(0, 10, 100), 10);
        assert_eq!(a_crudo(100, 10, 100), 100);
        assert_eq!(a_crudo(0, 0, 100), 0);
        for p in 0..=100u8 {
            let c = a_crudo(p, 10, 100);
            assert!(c >= 10);
            assert!(a_porcentaje(c, 10, 100).abs_diff(p) <= 1);
            assert_eq!(a_porcentaje(a_crudo(p, 0, 100), 0, 100), p);
        }
        // Por debajo del mínimo (desde el menú del monitor) se ve como 0 %.
        assert_eq!(a_porcentaje(3, 10, 100), 0);
        // Tope del 40 %.
        assert_eq!(piso(90, 100), 40);
        assert_eq!(paso(0, 1), 6);
        assert_eq!(paso(100, -1), 94);
        assert_eq!(paso(3, -1), 0);
    }

    #[test]
    fn ajustes_con_y_sin_lista() {
        let mut m = Memoria::default();
        let a = leer_ajustes(
            &mut m,
            10,
            || Some("vcp(04 10 12 14(04 05 0B) 60(11 12) 8D)".into()),
            |c| match c {
                0x12 => Some((50, 100)),
                0x14 => Some((0x0105, 0x0B)), // byte alto sucio
                0x60 => None,                  // no respondió: no se muestra
                0x8D => Some((2, 2)),
                _ => None,
            },
        );
        assert!(a.con_lista);
        assert_eq!(a.capacidades.iter().map(|c| c.codigo).collect::<Vec<_>>(), [0x04, 0x12, 0x14, 0x8D]);
        assert_eq!(a.valores.get(&0x14).map(|v| v.actual), Some(5));
        assert_eq!(validar(&m, 0x14, 4), Ok(4));
        assert!(validar(&m, 0x14, 8).is_err());
        assert!(validar(&m, 0x10, 50).is_err());
        assert_eq!(validar(&m, 0x12, 500), Ok(100));
        assert_eq!(validar(&m, 0x04, 0), Ok(1));

        let mut m = Memoria::default();
        let a = leer_ajustes(&mut m, 0, || None, |c| (c == 0x12).then_some((30, 100)));
        assert!(!a.con_lista);
        assert_eq!(a.capacidades, vec![Capacidad { codigo: 0x12, valores: vec![] }]);
        assert!(m.lista.is_some());
    }
}
