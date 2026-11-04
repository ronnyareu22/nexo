//! Firma de las peticiones a la nube de Tuya (HMAC-SHA256), sin red: se prueba con los ejemplos de su documentación.
//!
//! <https://developer.tuya.com/en/docs/iot/new-singnature?id=Kbw0q34cs2e5g>
//! - Pedir o renovar el token: `sign = HMAC-SHA256(client_id + t + nonce + stringToSign, secret)` en mayúsculas.
//! - Lo demás: `sign = HMAC-SHA256(client_id + access_token + t + nonce + stringToSign, secret)`.
//! - `stringToSign = método \n SHA256(cuerpo) \n cabeceras firmadas ("clave:valor\n" cada una) \n ruta`, con los
//!   parámetros de la ruta en orden alfabético y sin codificar. Sin cabeceras firmadas queda una línea vacía.

use sha2::{Digest, Sha256};

/// SHA-256 de un cuerpo vacío (lo que va en un GET).
pub const SHA_VACIO: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

pub fn hex(bytes: &[u8]) -> String {
    const DIGITOS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(DIGITOS[(b >> 4) as usize] as char);
        s.push(DIGITOS[(b & 0x0f) as usize] as char);
    }
    s
}

/// HMAC-SHA256 (RFC 2104) con el sha2 que ya venía con Tauri.
pub fn hmac_sha256(clave: &[u8], mensaje: &[u8]) -> [u8; 32] {
    const BLOQUE: usize = 64;
    let mut k = [0u8; BLOQUE];
    if clave.len() > BLOQUE {
        k[..32].copy_from_slice(&Sha256::digest(clave));
    } else {
        k[..clave.len()].copy_from_slice(clave);
    }
    let mut dentro = [0x36u8; BLOQUE];
    let mut fuera = [0x5cu8; BLOQUE];
    for i in 0..BLOQUE {
        dentro[i] ^= k[i];
        fuera[i] ^= k[i];
    }
    let interno = Sha256::new().chain_update(dentro).chain_update(mensaje).finalize();
    Sha256::new().chain_update(fuera).chain_update(interno).finalize().into()
}

/// La ruta tal como se firma: los parámetros en orden alfabético, sin codificar; sin parámetros, sin "?".
pub fn ruta_firmada(ruta: &str, parametros: &[(&str, String)]) -> String {
    if parametros.is_empty() {
        return ruta.to_string();
    }
    let mut p: Vec<&(&str, String)> = parametros.iter().collect();
    p.sort_by(|a, b| a.0.cmp(b.0));
    let texto: Vec<String> = p.iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("{ruta}?{}", texto.join("&"))
}

/// El `stringToSign` de Tuya.
pub fn texto_a_firmar(metodo: &str, cuerpo: &[u8], cabeceras: &[(&str, &str)], ruta: &str) -> String {
    let sha = if cuerpo.is_empty() { SHA_VACIO.to_string() } else { hex(&Sha256::digest(cuerpo)) };
    let mut firmadas = String::new();
    for (k, v) in cabeceras {
        firmadas.push_str(k);
        firmadas.push(':');
        firmadas.push_str(v);
        firmadas.push('\n');
    }
    format!("{metodo}\n{sha}\n{firmadas}\n{ruta}")
}

/// La firma: `token` va vacío al pedir o renovar el token.
pub fn firmar(id: &str, token: &str, t: &str, nonce: &str, texto: &str, secreto: &str) -> String {
    let s = format!("{id}{token}{t}{nonce}{texto}");
    hex(&hmac_sha256(secreto.as_bytes(), s.as_bytes())).to_uppercase()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    // Los datos de ejemplo de la documentación de Tuya (no son de nadie).
    const ID: &str = "1KAD46OrT9HafiKdsXeg";
    const SECRETO: &str = "4OHBOnWOqaEC1mWXOpVL3yV50s0qGSRC";
    const T: &str = "1588925778000";
    const NONCE: &str = "5138cc3a9033d69856923fd07b491173";
    const CABECERAS: [(&str, &str); 2] = [("area_id", "29a33e8796834b1efa6"), ("call_id", "8afdb70ab2ed11eb85290242ac130003")];

    #[test]
    fn hogar_firma_hmac_rfc4231() {
        let r = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(hex(&r), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
        // Clave más larga que el bloque (caso 6 del RFC 4231).
        let r = hmac_sha256(&[0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First");
        assert_eq!(hex(&r), "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54");
    }

    #[test]
    fn hogar_firma_ejemplo_del_token() {
        let texto = texto_a_firmar("GET", b"", &CABECERAS, "/v1.0/token?grant_type=1");
        assert!(texto.ends_with("call_id:8afdb70ab2ed11eb85290242ac130003\n\n/v1.0/token?grant_type=1"));
        assert_eq!(firmar(ID, "", T, NONCE, &texto, SECRETO), "9E48A3E93B302EEECC803C7241985D0A34EB944F40FB573C7B5C2A82158AF13E");
    }

    #[test]
    fn hogar_firma_ejemplo_de_negocio() {
        let ruta = ruta_firmada("/v2.0/apps/schema/users", &[("page_size", "50".into()), ("page_no", "1".into())]);
        assert_eq!(ruta, "/v2.0/apps/schema/users?page_no=1&page_size=50");
        let texto = texto_a_firmar("GET", b"", &CABECERAS, &ruta);
        let firma = firmar(ID, "3f4eda2bdec17232f67c0b188af3eec1", T, NONCE, &texto, SECRETO);
        assert_eq!(firma, "AE4481C692AA80B25F3A7E12C3A5FD9BBF6251539DD78E565A1A72A508A88784");
    }

    #[test]
    fn hogar_firma_post_sin_cabeceras() {
        // Lo que manda Nexo: un POST con cuerpo y sin nonce ni cabeceras firmadas (comprobado aparte en Python).
        let cuerpo = br#"{"commands":[{"code":"switch_led","value":true}]}"#;
        let texto = texto_a_firmar("POST", cuerpo, &[], "/v1.0/iot-03/devices/vdevo123/commands");
        assert_eq!(
            texto,
            "POST\n8479c9c60cd5d531054c49333c7b361a9ce41b9b313ab8eb6bc9df4141f658ef\n\n/v1.0/iot-03/devices/vdevo123/commands"
        );
        let firma = firmar(ID, "3f4eda2bdec17232f67c0b188af3eec1", T, "", &texto, SECRETO);
        assert_eq!(firma, "EDABABFA19E3D81FA1E264A2C6FFCE85DDDB2B886D5C13896BE27598A52A4D22");
        assert_eq!(texto_a_firmar("GET", b"", &[], "/v1.0/token?grant_type=1"), format!("GET\n{SHA_VACIO}\n\n/v1.0/token?grant_type=1"));
    }
}
