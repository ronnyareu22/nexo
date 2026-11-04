//! Nexo Drop en Windows: el portapapeles (texto, imágenes y archivos copiados en el Explorador), imágenes con
//! WIC (miniaturas, PNG y mapas de bits) y abrir o mostrar con el Explorador.

use super::Copiado;
use std::path::{Path, PathBuf};
use std::time::Duration;
use windows::Win32::Foundation::{GENERIC_READ, GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA, IWICBitmapFrameDecode,
    IWICBitmapFrameEncode, IWICBitmapSource, IWICFormatConverter, IWICImagingFactory, IWICPalette, WICBitmapDitherTypeNone,
    WICBitmapEncoderNoCache, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::StructuredStorage::{CreateStreamOnHGlobal, GetHGlobalFromStream, IPropertyBag2};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, IStream, STATFLAG_NONAME, STATSTG,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

const LADO_MINIATURA: u32 = 128;
/// Imágenes más grandes que esto (en píxeles) no se pasan al portapapeles.
const MAX_PIXELES: u64 = 100_000_000;

// ---------------------------------------------------------------- Portapapeles

/// El portapapeles abierto; se cierra al salir. Si otra app lo tiene, se reintenta un momento.
struct Portapapeles;

impl Portapapeles {
    fn abrir() -> Option<Self> {
        for _ in 0..10 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Some(Portapapeles);
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        None
    }
}

impl Drop for Portapapeles {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

/// El formato "PNG" que usan Chrome, Office y la herramienta Recortes.
fn formato_png() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("PNG")) }
}

/// La marca que ponen los gestores de contraseñas para que lo copiado no se guarde en historiales.
fn formato_oculto() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")) }
}

/// Copia los bytes a memoria global y los deja en el portapapeles (ya abierto y vaciado).
fn poner_datos(formato: u32, bytes: &[u8]) -> bool {
    unsafe {
        let Ok(memoria) = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) else { return false };
        let destino = GlobalLock(memoria);
        if destino.is_null() {
            let _ = GlobalFree(Some(memoria));
            return false;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), destino as *mut u8, bytes.len());
        // Devuelve "error" cuando el bloqueo llega a cero: es lo esperado.
        let _ = GlobalUnlock(memoria);
        // Si sale bien, la memoria pasa a ser del sistema.
        if SetClipboardData(formato, Some(HANDLE(memoria.0))).is_err() {
            let _ = GlobalFree(Some(memoria));
            return false;
        }
        true
    }
}

/// Lee un formato del portapapeles (ya abierto).
fn leer_datos(formato: u32) -> Option<Vec<u8>> {
    unsafe {
        IsClipboardFormatAvailable(formato).ok()?;
        let h = GetClipboardData(formato).ok()?;
        let memoria = HGLOBAL(h.0);
        let tam = GlobalSize(memoria);
        let origen = GlobalLock(memoria);
        if origen.is_null() || tam == 0 {
            return None;
        }
        let datos = std::slice::from_raw_parts(origen as *const u8, tam).to_vec();
        let _ = GlobalUnlock(memoria);
        Some(datos)
    }
}

pub fn poner_texto(texto: &str) -> bool {
    let mut utf16: Vec<u16> = texto.encode_utf16().collect();
    utf16.push(0);
    let bytes: Vec<u8> = utf16.iter().flat_map(|c| c.to_le_bytes()).collect();
    let Some(_abierto) = Portapapeles::abrir() else { return false };
    unsafe { EmptyClipboard() }.is_ok() && poner_datos(CF_UNICODETEXT.0 as u32, &bytes)
}

/// Pone una imagen recibida en el portapapeles: como mapa de bits (para todas las apps) y, si es PNG, también
/// tal cual (las apps que entienden PNG conservan la transparencia).
pub fn poner_imagen(ruta: &Path) -> bool {
    let Some(dib) = dib_de_archivo(ruta) else { return false };
    let png = std::fs::read(ruta).ok().filter(|d| d.starts_with(b"\x89PNG\r\n\x1a\n"));
    let Some(_abierto) = Portapapeles::abrir() else { return false };
    if unsafe { EmptyClipboard() }.is_err() || !poner_datos(CF_DIB.0 as u32, &dib) {
        return false;
    }
    if let Some(png) = png {
        poner_datos(formato_png(), &png);
    }
    true
}

pub fn leer_copiado() -> Option<Copiado> {
    let (archivos, png, dib, texto, oculto) = {
        let _abierto = Portapapeles::abrir()?;
        let oculto = unsafe { IsClipboardFormatAvailable(formato_oculto()) }.is_ok();
        let archivos = leer_archivos();
        let png = if archivos.is_empty() { leer_datos(formato_png()) } else { None };
        let dib = if archivos.is_empty() && png.is_none() { leer_datos(CF_DIB.0 as u32) } else { None };
        let texto = if archivos.is_empty() && png.is_none() && dib.is_none() { leer_datos(CF_UNICODETEXT.0 as u32) } else { None };
        (archivos, png, dib, texto, oculto)
    };
    if !archivos.is_empty() {
        return Some(Copiado::Archivos(archivos));
    }
    if let Some(png) = png {
        return Some(Copiado::Imagen(png));
    }
    if let Some(dib) = dib {
        // El mapa de bits se pasa a PNG fuera del portapapeles (para soltarlo cuanto antes).
        return png_de_dib(&dib).map(Copiado::Imagen);
    }
    let datos = texto?;
    let utf16: Vec<u16> = datos.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).take_while(|c| *c != 0).collect();
    let texto = String::from_utf16_lossy(&utf16);
    (!texto.is_empty()).then_some(Copiado::Texto { texto, oculto })
}

/// Los archivos copiados en el Explorador (CF_HDROP), con el portapapeles ya abierto.
fn leer_archivos() -> Vec<PathBuf> {
    unsafe {
        if IsClipboardFormatAvailable(CF_HDROP.0 as u32).is_err() {
            return Vec::new();
        }
        let Ok(h) = GetClipboardData(CF_HDROP.0 as u32) else { return Vec::new() };
        let soltar = HDROP(h.0);
        let cantidad = DragQueryFileW(soltar, u32::MAX, None);
        let mut rutas = Vec::new();
        for i in 0..cantidad.min(10_000) {
            let largo = DragQueryFileW(soltar, i, None) as usize;
            if largo == 0 {
                continue;
            }
            let mut buf = vec![0u16; largo + 1];
            let n = DragQueryFileW(soltar, i, Some(buf.as_mut_slice())) as usize;
            rutas.push(PathBuf::from(String::from_utf16_lossy(&buf[..n.min(largo)])));
        }
        rutas
    }
}

// ---------------------------------------------------------------- Imágenes con WIC

fn fabrica() -> Option<IWICImagingFactory> {
    // S_OK, S_FALSE y RPC_E_CHANGED_MODE dejan COM usable en este hilo.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }.ok()
}

fn cuadro_de_archivo(f: &IWICImagingFactory, ruta: &Path) -> Option<IWICBitmapFrameDecode> {
    let nombre = HSTRING::from(ruta.as_os_str());
    unsafe {
        let decodificador = f.CreateDecoderFromFilename(&nombre, None, GENERIC_READ, WICDecodeMetadataCacheOnDemand).ok()?;
        decodificador.GetFrame(0).ok()
    }
}

fn tamano_de(fuente: &IWICBitmapSource) -> Option<(u32, u32)> {
    let (mut ancho, mut alto) = (0u32, 0u32);
    unsafe { fuente.GetSize(&mut ancho, &mut alto) }.ok()?;
    (ancho > 0 && alto > 0).then_some((ancho, alto))
}

/// La imagen pasada a 32 bits BGRA (lo que esperan el PNG y el mapa de bits).
fn a_bgra(f: &IWICImagingFactory, fuente: &IWICBitmapSource) -> Option<IWICFormatConverter> {
    unsafe {
        let conversor = f.CreateFormatConverter().ok()?;
        conversor
            .Initialize(
                fuente,
                &GUID_WICPixelFormat32bppBGRA,
                WICBitmapDitherTypeNone,
                None::<&IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .ok()?;
        Some(conversor)
    }
}

/// Codifica la imagen como PNG.
fn codificar_png(f: &IWICImagingFactory, fuente: &IWICBitmapSource, ancho: u32, alto: u32) -> Option<Vec<u8>> {
    unsafe {
        let flujo: IStream = CreateStreamOnHGlobal(HGLOBAL(std::ptr::null_mut()), true).ok()?;
        let codificador = f.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null()).ok()?;
        codificador.Initialize(&flujo, WICBitmapEncoderNoCache).ok()?;
        let mut cuadro: Option<IWICBitmapFrameEncode> = None;
        let mut opciones: Option<IPropertyBag2> = None;
        codificador.CreateNewFrame(&mut cuadro, &mut opciones).ok()?;
        let cuadro = cuadro?;
        cuadro.Initialize(opciones.as_ref()).ok()?;
        cuadro.SetSize(ancho, alto).ok()?;
        let mut formato = GUID_WICPixelFormat32bppBGRA;
        cuadro.SetPixelFormat(&mut formato).ok()?;
        cuadro.WriteSource(fuente, std::ptr::null()).ok()?;
        cuadro.Commit().ok()?;
        codificador.Commit().ok()?;

        let mut info = STATSTG::default();
        flujo.Stat(&mut info, STATFLAG_NONAME).ok()?;
        let memoria = GetHGlobalFromStream(&flujo).ok()?;
        let largo = (info.cbSize as usize).min(GlobalSize(memoria));
        let origen = GlobalLock(memoria);
        if origen.is_null() || largo == 0 {
            return None;
        }
        let png = std::slice::from_raw_parts(origen as *const u8, largo).to_vec();
        let _ = GlobalUnlock(memoria);
        Some(png)
    }
}

/// Miniatura de una imagen (PNG de 128 px como mucho) como data URL.
pub fn miniatura(ruta: &Path) -> Option<String> {
    let f = fabrica()?;
    let cuadro = cuadro_de_archivo(&f, ruta)?;
    let fuente: IWICBitmapSource = windows::core::Interface::cast(&cuadro).ok()?;
    let (ancho, alto) = tamano_de(&fuente)?;
    let escala = (LADO_MINIATURA as f64 / ancho.max(alto) as f64).min(1.0);
    let (a, b) = (((ancho as f64 * escala).round() as u32).max(1), ((alto as f64 * escala).round() as u32).max(1));
    let png = unsafe {
        let escalador = f.CreateBitmapScaler().ok()?;
        escalador.Initialize(&fuente, a, b, WICBitmapInterpolationModeFant).ok()?;
        let escalada: IWICBitmapSource = windows::core::Interface::cast(&escalador).ok()?;
        let bgra = a_bgra(&f, &escalada)?;
        let bgra: IWICBitmapSource = windows::core::Interface::cast(&bgra).ok()?;
        codificar_png(&f, &bgra, a, b)?
    };
    Some(format!("data:image/png;base64,{}", super::rutas::base64(&png)))
}

/// Un mapa de bits para el portapapeles (CF_DIB): encabezado de 40 bytes y filas de abajo hacia arriba.
fn dib_de_archivo(ruta: &Path) -> Option<Vec<u8>> {
    let f = fabrica()?;
    let cuadro = cuadro_de_archivo(&f, ruta)?;
    let fuente: IWICBitmapSource = windows::core::Interface::cast(&cuadro).ok()?;
    let (ancho, alto) = tamano_de(&fuente)?;
    if ancho as u64 * alto as u64 > MAX_PIXELES {
        return None;
    }
    let bgra = a_bgra(&f, &fuente)?;
    let paso = ancho as usize * 4;
    let mut pixeles = vec![0u8; paso * alto as usize];
    unsafe { bgra.CopyPixels(std::ptr::null(), paso as u32, &mut pixeles) }.ok()?;

    let mut dib = Vec::with_capacity(40 + pixeles.len());
    dib.extend_from_slice(&40u32.to_le_bytes()); // biSize
    dib.extend_from_slice(&(ancho as i32).to_le_bytes()); // biWidth
    dib.extend_from_slice(&(alto as i32).to_le_bytes()); // biHeight (positivo: de abajo hacia arriba)
    dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    dib.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
    dib.extend_from_slice(&(pixeles.len() as u32).to_le_bytes()); // biSizeImage
    dib.extend_from_slice(&[0u8; 16]); // resolución y paleta: sin datos
    for fila in pixeles.chunks_exact(paso).rev() {
        dib.extend_from_slice(fila);
    }
    Some(dib)
}

/// Un mapa de bits del portapapeles (CF_DIB) pasado a PNG: se le pone el encabezado de archivo BMP y lo lee WIC.
fn png_de_dib(dib: &[u8]) -> Option<Vec<u8>> {
    let u32_en = |i: usize| dib.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let cabecera = u32_en(0)? as usize;
    let bits = dib.get(14..16).map(|b| u16::from_le_bytes([b[0], b[1]]))?;
    let compresion = u32_en(16)?;
    let colores = u32_en(32)? as usize;
    // Con el encabezado corto, las máscaras de color van aparte (BI_BITFIELDS y BI_ALPHABITFIELDS).
    let mascaras = match (cabecera, compresion) {
        (40, 3) => 12,
        (40, 6) => 16,
        _ => 0,
    };
    let paleta = if bits <= 8 { if colores == 0 { 1usize << bits } else { colores } } else { colores } * 4;
    let desplazamiento = 14 + cabecera + mascaras + paleta;
    let total = 14 + dib.len();
    if desplazamiento >= total {
        return None;
    }
    let mut bmp = Vec::with_capacity(total);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(total as u32).to_le_bytes());
    bmp.extend_from_slice(&[0u8; 4]);
    bmp.extend_from_slice(&(desplazamiento as u32).to_le_bytes());
    bmp.extend_from_slice(dib);

    let f = fabrica()?;
    // `bmp` vive hasta el final de la función: el flujo de WIC lo lee sin copiarlo.
    unsafe {
        let flujo = f.CreateStream().ok()?;
        flujo.InitializeFromMemory(&bmp).ok()?;
        let decodificador = f.CreateDecoderFromStream(&flujo, std::ptr::null(), WICDecodeMetadataCacheOnDemand).ok()?;
        let cuadro = decodificador.GetFrame(0).ok()?;
        let fuente: IWICBitmapSource = windows::core::Interface::cast(&cuadro).ok()?;
        let (ancho, alto) = tamano_de(&fuente)?;
        let bgra = a_bgra(&f, &fuente)?;
        let bgra: IWICBitmapSource = windows::core::Interface::cast(&bgra).ok()?;
        codificar_png(&f, &bgra, ancho, alto)
    }
}

// ---------------------------------------------------------------- Abrir y mostrar

fn shell_abrir(destino: &HSTRING) -> Result<(), String> {
    let r = unsafe { ShellExecuteW(None, w!("open"), destino, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    // Devuelve un número mayor que 32 si pudo.
    if r.0 as isize > 32 { Ok(()) } else { Err("Windows no pudo abrirlo.".into()) }
}

/// Abre un archivo con su app, o una carpeta en el Explorador.
pub fn abrir(ruta: &Path) -> Result<(), String> {
    shell_abrir(&HSTRING::from(ruta.as_os_str()))
}

/// Abre un enlace (ya revisado: http o https) en el navegador.
pub fn abrir_enlace(url: &str) -> Result<(), String> {
    shell_abrir(&HSTRING::from(url))
}

/// Muestra el archivo o la carpeta en el Explorador, ya elegido.
pub fn mostrar(ruta: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // explorer espera "/select," pegado a la ruta entre comillas; por eso va sin el entrecomillado de Rust.
    std::process::Command::new("explorer")
        .raw_arg(format!("/select,\"{}\"", ruta.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("No pude abrir el Explorador ({e})."))
}

/// El nombre del equipo ("INNOTECH1017"), en minúsculas como lo muestra Tailscale.
pub fn nombre_equipo() -> String {
    std::env::var("COMPUTERNAME").map(|n| n.to_lowercase()).ok().filter(|n| !n.is_empty()).unwrap_or_else(|| "PC".into())
}
