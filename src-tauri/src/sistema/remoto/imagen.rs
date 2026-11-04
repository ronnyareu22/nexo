//! Imágenes con ImageIO (sin abrir procesos): medir, achicar y guardar como PNG.
//! Para achicar se usa la miniatura de ImageIO, que es rápida y no carga la imagen entera al tamaño original.

use std::ffi::{c_char, c_void};
use std::ptr::null;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDataCreate(alloc: *const c_void, bytes: *const u8, largo: isize) -> *const c_void;
    fn CFDataCreateMutable(alloc: *const c_void, capacidad: isize) -> *mut c_void;
    fn CFDataGetLength(datos: *const c_void) -> isize;
    fn CFDataGetBytePtr(datos: *const c_void) -> *const u8;
    fn CFDictionaryCreate(
        alloc: *const c_void,
        claves: *const *const c_void,
        valores: *const *const c_void,
        n: isize,
        cb_claves: *const c_void,
        cb_valores: *const c_void,
    ) -> *const c_void;
    fn CFDictionaryGetValue(diccionario: *const c_void, clave: *const c_void) -> *const c_void;
    fn CFNumberCreate(alloc: *const c_void, tipo: isize, valor: *const c_void) -> *const c_void;
    fn CFNumberGetValue(numero: *const c_void, tipo: isize, valor: *mut c_void) -> u8;
    fn CFStringCreateWithCString(alloc: *const c_void, texto: *const c_char, codificacion: u32) -> *const c_void;
    fn CFRelease(objeto: *const c_void);
    static kCFBooleanTrue: *const c_void;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
}

#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    fn CGImageSourceCreateWithData(datos: *const c_void, opciones: *const c_void) -> *const c_void;
    fn CGImageSourceCopyPropertiesAtIndex(fuente: *const c_void, indice: usize, opciones: *const c_void) -> *const c_void;
    fn CGImageSourceCreateThumbnailAtIndex(fuente: *const c_void, indice: usize, opciones: *const c_void) -> *const c_void;
    fn CGImageDestinationCreateWithData(
        datos: *mut c_void,
        tipo: *const c_void,
        cantidad: usize,
        opciones: *const c_void,
    ) -> *const c_void;
    fn CGImageDestinationAddImage(destino: *const c_void, imagen: *const c_void, propiedades: *const c_void);
    fn CGImageDestinationFinalize(destino: *const c_void) -> bool;
    static kCGImagePropertyPixelWidth: *const c_void;
    static kCGImagePropertyPixelHeight: *const c_void;
    static kCGImagePropertyDPIWidth: *const c_void;
    static kCGImageSourceCreateThumbnailFromImageAlways: *const c_void;
    static kCGImageSourceThumbnailMaxPixelSize: *const c_void;
    static kCGImageSourceCreateThumbnailWithTransform: *const c_void;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGImageGetWidth(imagen: *const c_void) -> usize;
    fn CGImageGetHeight(imagen: *const c_void) -> usize;
}

const NUMERO_I32: isize = 3; // kCFNumberSInt32Type
const NUMERO_F64: isize = 13; // kCFNumberDoubleType
const UTF8: u32 = 0x0800_0100; // kCFStringEncodingUTF8

/// Un objeto de CoreFoundation que se libera al salir.
struct Objeto(*const c_void);

impl Objeto {
    fn nuevo(p: *const c_void) -> Option<Self> {
        (!p.is_null()).then_some(Objeto(p))
    }
}

impl Drop for Objeto {
    fn drop(&mut self) {
        // SAFETY: es un objeto CF propio (Create/Copy) y no nulo.
        unsafe { CFRelease(self.0) }
    }
}

/// Tamaño en píxeles y ppp (72 = 1x; las capturas Retina dicen 144).
pub struct Medida {
    pub ancho: u32,
    pub alto: u32,
    pub dpi: f64,
}

fn fuente(datos: &[u8]) -> Option<(Objeto, Objeto)> {
    // SAFETY: CFDataCreate copia los bytes; la fuente queda con su propia copia.
    unsafe {
        let data = Objeto::nuevo(CFDataCreate(null(), datos.as_ptr(), datos.len() as isize))?;
        let fuente = Objeto::nuevo(CGImageSourceCreateWithData(data.0, null()))?;
        Some((data, fuente))
    }
}

fn numero(diccionario: *const c_void, clave: *const c_void) -> Option<f64> {
    // SAFETY: Get no transfiere la propiedad; el valor de esas claves es un CFNumber.
    unsafe {
        let v = CFDictionaryGetValue(diccionario, clave);
        if v.is_null() {
            return None;
        }
        let mut x = 0f64;
        (CFNumberGetValue(v, NUMERO_F64, &mut x as *mut f64 as *mut c_void) != 0).then_some(x)
    }
}

/// Mide una imagen leyendo solo su encabezado.
pub fn medir(datos: &[u8]) -> Option<Medida> {
    let (_data, fuente) = fuente(datos)?;
    // SAFETY: fuente válida; las claves son constantes de ImageIO.
    unsafe {
        let props = Objeto::nuevo(CGImageSourceCopyPropertiesAtIndex(fuente.0, 0, null()))?;
        let ancho = numero(props.0, kCGImagePropertyPixelWidth)?;
        let alto = numero(props.0, kCGImagePropertyPixelHeight)?;
        let dpi = numero(props.0, kCGImagePropertyDPIWidth).filter(|d| *d > 0.0).unwrap_or(72.0);
        Some(Medida { ancho: ancho as u32, alto: alto as u32, dpi })
    }
}

/// La imagen con su lado más largo en `lado_max` píxeles como mucho (nunca la agranda), como PNG.
/// Devuelve el PNG y su tamaño.
pub fn achicar_png(datos: &[u8], lado_max: u32) -> Option<(Vec<u8>, u32, u32)> {
    let (_data, fuente) = fuente(datos)?;
    let lado = lado_max.clamp(1, i32::MAX as u32) as i32;
    // SAFETY: todos los objetos se crean aquí y se liberan al salir (Objeto); las claves son de ImageIO.
    unsafe {
        let tamano = Objeto::nuevo(CFNumberCreate(null(), NUMERO_I32, &lado as *const i32 as *const c_void))?;
        let claves = [
            kCGImageSourceCreateThumbnailFromImageAlways,
            kCGImageSourceThumbnailMaxPixelSize,
            kCGImageSourceCreateThumbnailWithTransform,
        ];
        let valores = [kCFBooleanTrue, tamano.0, kCFBooleanTrue];
        let opciones = Objeto::nuevo(CFDictionaryCreate(
            null(),
            claves.as_ptr(),
            valores.as_ptr(),
            claves.len() as isize,
            &kCFTypeDictionaryKeyCallBacks as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const c_void,
        ))?;
        let imagen = Objeto::nuevo(CGImageSourceCreateThumbnailAtIndex(fuente.0, 0, opciones.0))?;
        let ancho = CGImageGetWidth(imagen.0) as u32;
        let alto = CGImageGetHeight(imagen.0) as u32;
        let png = codificar_png(imagen.0)?;
        Some((png, ancho, alto))
    }
}

/// Codifica una CGImage como PNG.
///
/// # Safety
/// `imagen` debe ser una CGImage válida.
unsafe fn codificar_png(imagen: *const c_void) -> Option<Vec<u8>> {
    unsafe {
        let salida = CFDataCreateMutable(null(), 0);
        let _salida = Objeto::nuevo(salida as *const c_void)?;
        let tipo = Objeto::nuevo(CFStringCreateWithCString(null(), c"public.png".as_ptr(), UTF8))?;
        let destino = Objeto::nuevo(CGImageDestinationCreateWithData(salida, tipo.0, 1, null()))?;
        CGImageDestinationAddImage(destino.0, imagen, null());
        if !CGImageDestinationFinalize(destino.0) {
            return None;
        }
        let largo = CFDataGetLength(salida as *const c_void);
        let bytes = CFDataGetBytePtr(salida as *const c_void);
        if bytes.is_null() || largo <= 0 {
            return None;
        }
        Some(std::slice::from_raw_parts(bytes, largo as usize).to_vec())
    }
}
