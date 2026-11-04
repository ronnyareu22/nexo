//! AES-128 a mano (FIPS-197), solo para leer los avisos que los aparatos Tuya mandan a la red local.
//! Sin crates nuevos: ECB para los de la versión 3.3 y CTR (la parte de cifrado de GCM) para los de la 3.5.

const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76, 0xca, 0x82, 0xc9,
    0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0, 0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f,
    0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15, 0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07,
    0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75, 0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3,
    0x29, 0xe3, 0x2f, 0x84, 0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58,
    0xcf, 0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8, 0x51, 0xa3,
    0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2, 0xcd, 0x0c, 0x13, 0xec, 0x5f,
    0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73, 0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88,
    0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb, 0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac,
    0x62, 0x91, 0x95, 0xe4, 0x79, 0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a,
    0xae, 0x08, 0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a, 0x70,
    0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e, 0xe1, 0xf8, 0x98, 0x11,
    0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf, 0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42,
    0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const RCON: [u8; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

/// La S-box inversa, calculada de la directa.
fn sbox_inversa() -> [u8; 256] {
    let mut inv = [0u8; 256];
    for (i, &v) in SBOX.iter().enumerate() {
        inv[v as usize] = i as u8;
    }
    inv
}

/// Multiplicar por x en GF(2^8).
fn xtime(b: u8) -> u8 {
    (b << 1) ^ if b & 0x80 != 0 { 0x1b } else { 0 }
}

fn mul(mut a: u8, mut b: u8) -> u8 {
    let mut r = 0;
    while b != 0 {
        if b & 1 != 0 {
            r ^= a;
        }
        a = xtime(a);
        b >>= 1;
    }
    r
}

/// Una clave AES-128 ya expandida (11 claves de ronda).
pub struct Aes128 {
    rondas: [[u8; 16]; 11],
    inversa: [u8; 256],
}

impl Aes128 {
    pub fn nueva(clave: &[u8; 16]) -> Self {
        let mut w = [[0u8; 4]; 44];
        for (i, palabra) in w.iter_mut().take(4).enumerate() {
            palabra.copy_from_slice(&clave[i * 4..i * 4 + 4]);
        }
        for i in 4..44 {
            let mut t = w[i - 1];
            if i % 4 == 0 {
                t = [SBOX[t[1] as usize], SBOX[t[2] as usize], SBOX[t[3] as usize], SBOX[t[0] as usize]];
                t[0] ^= RCON[i / 4 - 1];
            }
            for k in 0..4 {
                w[i][k] = w[i - 4][k] ^ t[k];
            }
        }
        let mut rondas = [[0u8; 16]; 11];
        for (r, ronda) in rondas.iter_mut().enumerate() {
            for c in 0..4 {
                ronda[c * 4..c * 4 + 4].copy_from_slice(&w[r * 4 + c]);
            }
        }
        Aes128 { rondas, inversa: sbox_inversa() }
    }

    fn sumar(estado: &mut [u8; 16], clave: &[u8; 16]) {
        for (e, k) in estado.iter_mut().zip(clave) {
            *e ^= k;
        }
    }

    /// Las filas se desplazan (el estado va por columnas: índice = columna * 4 + fila).
    fn desplazar(e: &mut [u8; 16], inversa: bool) {
        let copia = *e;
        for fila in 1..4 {
            for col in 0..4 {
                let origen = if inversa { (col + 4 - fila) % 4 } else { (col + fila) % 4 };
                e[col * 4 + fila] = copia[origen * 4 + fila];
            }
        }
    }

    fn mezclar(e: &mut [u8; 16], inversa: bool) {
        let m: [u8; 4] = if inversa { [14, 11, 13, 9] } else { [2, 3, 1, 1] };
        for col in 0..4 {
            let a = [e[col * 4], e[col * 4 + 1], e[col * 4 + 2], e[col * 4 + 3]];
            for fila in 0..4 {
                e[col * 4 + fila] = mul(a[0], m[(4 - fila) % 4])
                    ^ mul(a[1], m[(5 - fila) % 4])
                    ^ mul(a[2], m[(6 - fila) % 4])
                    ^ mul(a[3], m[(7 - fila) % 4]);
            }
        }
    }

    pub fn cifrar_bloque(&self, bloque: &mut [u8; 16]) {
        Self::sumar(bloque, &self.rondas[0]);
        for r in 1..11 {
            for b in bloque.iter_mut() {
                *b = SBOX[*b as usize];
            }
            Self::desplazar(bloque, false);
            if r != 10 {
                Self::mezclar(bloque, false);
            }
            Self::sumar(bloque, &self.rondas[r]);
        }
    }

    pub fn descifrar_bloque(&self, bloque: &mut [u8; 16]) {
        Self::sumar(bloque, &self.rondas[10]);
        for r in (0..10).rev() {
            Self::desplazar(bloque, true);
            for b in bloque.iter_mut() {
                *b = self.inversa[*b as usize];
            }
            Self::sumar(bloque, &self.rondas[r]);
            if r != 0 {
                Self::mezclar(bloque, true);
            }
        }
    }
}

/// ECB con relleno PKCS#7 (lo que usan los Tuya 3.3). `None` si el largo o el relleno no cuadran.
pub fn descifrar_ecb(clave: &[u8; 16], datos: &[u8]) -> Option<Vec<u8>> {
    if datos.is_empty() || !datos.len().is_multiple_of(16) {
        return None;
    }
    let aes = Aes128::nueva(clave);
    let mut salida = Vec::with_capacity(datos.len());
    for trozo in datos.as_chunks::<16>().0 {
        let mut b = *trozo;
        aes.descifrar_bloque(&mut b);
        salida.extend_from_slice(&b);
    }
    let relleno = *salida.last()? as usize;
    if relleno == 0 || relleno > 16 || salida[salida.len() - relleno..].iter().any(|&x| x as usize != relleno) {
        return None;
    }
    salida.truncate(salida.len() - relleno);
    Some(salida)
}

/// ECB con relleno PKCS#7 (para las pruebas: armar un aviso como el de un aparato).
#[cfg(test)]
pub fn cifrar_ecb(clave: &[u8; 16], datos: &[u8]) -> Vec<u8> {
    let aes = Aes128::nueva(clave);
    let relleno = 16 - datos.len() % 16;
    let mut entrada = datos.to_vec();
    entrada.extend(std::iter::repeat_n(relleno as u8, relleno));
    let mut salida = Vec::with_capacity(entrada.len());
    for trozo in entrada.as_chunks::<16>().0 {
        let mut b = *trozo;
        aes.cifrar_bloque(&mut b);
        salida.extend_from_slice(&b);
    }
    salida
}

/// La parte de cifrado de GCM (CTR desde el contador 2 con un IV de 12 bytes). Cifrar y descifrar son lo mismo.
/// No revisa la etiqueta: alcanza para leer un aviso de búsqueda, nunca para confiar en una orden.
pub fn ctr_gcm(clave: &[u8; 16], iv: &[u8; 12], datos: &[u8]) -> Vec<u8> {
    let aes = Aes128::nueva(clave);
    let mut salida = Vec::with_capacity(datos.len());
    for (i, trozo) in datos.chunks(16).enumerate() {
        let mut contador = [0u8; 16];
        contador[..12].copy_from_slice(iv);
        contador[12..].copy_from_slice(&((i as u32).wrapping_add(2)).to_be_bytes());
        aes.cifrar_bloque(&mut contador);
        salida.extend(trozo.iter().zip(contador.iter()).map(|(a, b)| a ^ b));
    }
    salida
}

#[cfg(test)]
pub(super) fn hex(t: &str) -> Vec<u8> {
    (0..t.len()).step_by(2).filter_map(|i| u8::from_str_radix(&t[i..i + 2], 16).ok()).collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn bloque(t: &str) -> [u8; 16] {
        let mut b = [0u8; 16];
        b.copy_from_slice(&hex(t));
        b
    }

    #[test]
    fn lan_aes_vectores_fips197() {
        // FIPS-197, apéndice C.1 y apéndice B.
        let aes = Aes128::nueva(&bloque("000102030405060708090a0b0c0d0e0f"));
        let mut b = bloque("00112233445566778899aabbccddeeff");
        aes.cifrar_bloque(&mut b);
        assert_eq!(b, bloque("69c4e0d86a7b0430d8cdb78070b4c55a"));
        aes.descifrar_bloque(&mut b);
        assert_eq!(b, bloque("00112233445566778899aabbccddeeff"));

        let aes = Aes128::nueva(&bloque("2b7e151628aed2a6abf7158809cf4f3c"));
        let mut b = bloque("3243f6a8885a308d313198a2e0370734");
        aes.cifrar_bloque(&mut b);
        assert_eq!(b, bloque("3925841d02dc09fbdc118597196a0b32"));
        aes.descifrar_bloque(&mut b);
        assert_eq!(b, bloque("3243f6a8885a308d313198a2e0370734"));
    }

    #[test]
    fn lan_aes_gcm_caso_3() {
        // "The Galois/Counter Mode of Operation", caso de prueba 3 (el texto cifrado, sin la etiqueta).
        let clave = bloque("feffe9928665731c6d6a8f9467308308");
        let mut iv = [0u8; 12];
        iv.copy_from_slice(&hex("cafebabefacedbaddecaf888"));
        let claro = hex(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255",
        );
        let cifrado = ctr_gcm(&clave, &iv, &claro);
        assert_eq!(
            cifrado,
            hex("42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e21d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e091473f5985")
        );
        assert_eq!(ctr_gcm(&clave, &iv, &cifrado), claro);
    }

    #[test]
    fn lan_aes_ecb_ida_y_vuelta() {
        let clave = bloque("000102030405060708090a0b0c0d0e0f");
        for largo in [0usize, 1, 15, 16, 17, 40] {
            let datos: Vec<u8> = (0..largo as u8).collect();
            let c = cifrar_ecb(&clave, &datos);
            assert_eq!(c.len() % 16, 0);
            assert_eq!(descifrar_ecb(&clave, &c), Some(datos));
        }
        assert_eq!(descifrar_ecb(&clave, &[1, 2, 3]), None);
    }
}
