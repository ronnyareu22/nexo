//! Bluetooth: lista los aparatos emparejados, su batería, y los conecta, desconecta u olvida.
//!
//! Mac: lee `system_profiler SPBluetoothDataType -json` (tarda unos 60 ms) y usa IOBluetooth para conectar.
//! Olvidar usa `remove`, un mensaje privado de IOBluetooth: solo se llama si existe y se confirma el resultado.
//! Windows: pendiente (Windows.Devices.Bluetooth); por ahora devuelve la lista vacía.

use super::{Dispositivo, Estado, Tipo};

#[cfg(target_os = "macos")]
pub fn listar() -> Result<Estado, String> {
    use serde_json::Value;

    let salida = std::process::Command::new("/usr/sbin/system_profiler")
        .args(["SPBluetoothDataType", "-json"])
        .output()
        .map_err(|e| format!("No se pudo leer el Bluetooth: {e}"))?;
    let json: Value = serde_json::from_slice(&salida.stdout).map_err(|e| e.to_string())?;
    let datos = &json["SPBluetoothDataType"][0];

    let encendido = datos["controller_properties"]["controller_state"] == "attrib_on";
    let mut lista = Vec::new();
    for (grupo, conectado) in [("device_connected", true), ("device_not_connected", false)] {
        for item in datos[grupo].as_array().into_iter().flatten() {
            for (nombre, propiedades) in item.as_object().into_iter().flatten() {
                lista.push(crear(nombre, propiedades, conectado));
            }
        }
    }
    // Primero los conectados; dentro de cada grupo, por nombre.
    lista.sort_by(|a, b| b.conectado.cmp(&a.conectado).then(a.nombre.cmp(&b.nombre)));
    Ok(Estado { bluetooth_encendido: encendido, dispositivos: lista })
}

#[cfg(target_os = "macos")]
fn crear(nombre: &str, p: &serde_json::Value, conectado: bool) -> Dispositivo {
    let texto = |clave: &str| p[clave].as_str().map(str::to_owned);
    let porcentaje = |clave: &str| {
        p[clave].as_str().and_then(|v| v.trim().trim_end_matches('%').trim().parse::<u8>().ok())
    };

    let minusculas = nombre.to_lowercase();
    let tipo = match p["device_minorType"].as_str().unwrap_or("") {
        "Headphones" => Tipo::Audifonos,
        "Headset" => Tipo::Diadema,
        "Keyboard" => Tipo::Teclado,
        "Mouse" | "Trackpad" => Tipo::Mouse,
        "Speaker" | "Loudspeaker" | "Portable Audio" | "HiFi Audio Device" => Tipo::Parlante,
        "Smartphone" | "Cellular" | "Phone" => Tipo::Telefono,
        _ if ["iphone", "galaxy", "pixel", "redmi", "xiaomi", "motorola"]
            .iter()
            .any(|m| minusculas.contains(m)) =>
        {
            Tipo::Telefono
        }
        _ => Tipo::Otro,
    };

    let fabricante = match p["device_vendorID"].as_str().unwrap_or("") {
        _ if minusculas.contains("beats") => Some("Beats"),
        "0x004C" => Some("Apple"),
        "0x054C" => Some("Sony"),
        "0x046D" => Some("Logitech"),
        "0x045E" => Some("Microsoft"),
        "0x0075" | "0x04E8" => Some("Samsung"),
        "0x05AC" => Some("Apple"),
        "0x0A12" => Some("JBL"),
        _ if minusculas.contains("iphone") => Some("Apple"),
        _ => None,
    }
    .map(str::to_owned);

    Dispositivo {
        id: texto("device_address").unwrap_or_else(|| nombre.to_owned()),
        nombre: nombre.to_owned(),
        tipo,
        conectado,
        bateria: porcentaje("device_batteryLevelMain").or_else(|| porcentaje("device_batteryLevel")),
        bateria_izquierda: porcentaje("device_batteryLevelLeft"),
        bateria_derecha: porcentaje("device_batteryLevelRight"),
        bateria_estuche: porcentaje("device_batteryLevelCase"),
        fabricante,
        firmware: texto("device_firmwareVersion"),
        vendor_id: texto("device_vendorID"),
        product_id: texto("device_productID"),
        alias: None, // lo completa el historial
    }
}

#[cfg(target_os = "macos")]
pub fn cambiar_conexion(direccion: &str, conectar: bool) -> Result<(), String> {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;

    #[link(name = "IOBluetooth", kind = "framework")]
    unsafe extern "C" {}

    let clase = AnyClass::get(c"IOBluetoothDevice").ok_or("El Bluetooth de la Mac no está disponible")?;
    let direccion = NSString::from_str(&direccion.replace(':', "-").to_lowercase());

    // SAFETY: IOBluetoothDevice responde a estos mensajes; se revisa que el dispositivo exista.
    let codigo: i32 = unsafe {
        let dispositivo: *mut AnyObject = msg_send![clase, deviceWithAddressString: &*direccion];
        if dispositivo.is_null() {
            return Err("No encontré ese dispositivo entre los emparejados".into());
        }
        if conectar { msg_send![dispositivo, openConnection] } else { msg_send![dispositivo, closeConnection] }
    };

    match codigo {
        0 => Ok(()),
        _ if conectar => Err("No se pudo conectar. Revisa que esté encendido y cerca.".into()),
        _ => Err(format!("No se pudo desconectar (código {codigo:#x}).")),
    }
}

#[cfg(target_os = "macos")]
const NO_SE_PUDO_OLVIDAR: &str = "No se pudo olvidar desde Nexo. Hazlo en Ajustes › Bluetooth.";

/// Olvida (desempareja) un aparato. IOBluetooth no tiene una forma pública de hacerlo:
/// se usa el mensaje privado `remove` solo si el aparato lo entiende, y después se revisa
/// que de verdad haya salido de la lista de emparejados. Si algo no cuadra, se pide hacerlo en Ajustes.
#[cfg(target_os = "macos")]
pub fn olvidar(direccion: &str) -> Result<(), String> {
    use objc2::rc::autoreleasepool;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::{msg_send, sel};
    use objc2_foundation::NSString;
    use std::time::Duration;

    #[link(name = "IOBluetooth", kind = "framework")]
    unsafe extern "C" {}

    let clase = AnyClass::get(c"IOBluetoothDevice").ok_or("El Bluetooth de la Mac no está disponible")?;
    // IOBluetooth escribe las direcciones como "e8-9e-13-61-ce-b7"; system_profiler, como "E8:9E:13:61:CE:B7".
    let normalizar = |d: &str| d.replace(':', "-").to_lowercase();
    let buscada = normalizar(direccion);

    // SAFETY: pairedDevices devuelve un NSArray de IOBluetoothDevice (o nil); cada uno responde a addressString.
    let emparejado = || {
        autoreleasepool(|_| unsafe {
            let lista: *mut AnyObject = msg_send![clase, pairedDevices];
            if lista.is_null() {
                return false;
            }
            let cantidad: usize = msg_send![lista, count];
            (0..cantidad).any(|i| {
                let d: *mut AnyObject = msg_send![lista, objectAtIndex: i];
                let texto: *mut NSString = if d.is_null() { std::ptr::null_mut() } else { msg_send![d, addressString] };
                !texto.is_null() && normalizar(&(*texto).to_string()) == buscada
            })
        })
    };

    // Si no está entre los emparejados de IOBluetooth, no hay cómo confirmar nada.
    if !emparejado() {
        return Err(NO_SE_PUDO_OLVIDAR.into());
    }

    // SAFETY: se revisa que el aparato exista y que entienda `remove` antes de enviarlo.
    let enviado = autoreleasepool(|_| unsafe {
        let texto = NSString::from_str(&buscada);
        let dispositivo: *mut AnyObject = msg_send![clase, deviceWithAddressString: &*texto];
        if dispositivo.is_null() {
            return false;
        }
        let entiende: bool = msg_send![dispositivo, respondsToSelector: sel!(remove)];
        if entiende {
            let _: () = msg_send![dispositivo, remove];
        }
        entiende
    });
    if !enviado {
        return Err(NO_SE_PUDO_OLVIDAR.into());
    }

    // macOS tarda un momento en sacarlo de la lista: se espera hasta 3 s.
    for _ in 0..12 {
        std::thread::sleep(Duration::from_millis(250));
        if !emparejado() {
            return Ok(());
        }
    }
    Err(NO_SE_PUDO_OLVIDAR.into())
}

#[cfg(not(target_os = "macos"))]
pub fn olvidar(_direccion: &str) -> Result<(), String> {
    Err("Todavía no disponible en Windows".into())
}

#[cfg(not(target_os = "macos"))]
pub fn listar() -> Result<Estado, String> {
    Ok(Estado { bluetooth_encendido: true, dispositivos: Vec::new() })
}

#[cfg(not(target_os = "macos"))]
pub fn cambiar_conexion(_direccion: &str, _conectar: bool) -> Result<(), String> {
    Err("Todavía no disponible en Windows".into())
}
