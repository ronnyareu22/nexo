//! Lo que comparten Bluetooth y sonido en Windows: COM, las direcciones, los nodos PnP
//! de cada aparato Bluetooth y los puntos de sonido (endpoints) que le pertenecen.
//!
//! Un aparato Bluetooth aparece en el Administrador de dispositivos como varios nodos
//! (`BTHENUM\DEV_E89E1361CEB7\…`, `BTHENUM\{0000110B-…}_VID&0001004C_PID&2006\…_E89E1361CEB7_…`).
//! De ahí salen su marca, su modelo, su batería y su "contenedor", que es el mismo que tienen
//! sus salidas y micrófonos de sonido. Así se unen el Bluetooth y el sonido.

use std::ffi::c_void;

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GETIDLIST_FILTER_ENUMERATOR, CM_Get_DevNode_PropertyW, CM_Get_Device_ID_List_SizeW, CM_Get_Device_ID_ListW,
    CM_LOCATE_DEVNODE_NORMAL, CM_LOCATE_DEVNODE_PHANTOM, CM_Locate_DevNodeW, CR_BUFFER_SMALL, CR_SUCCESS,
};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_ContainerId;
use windows::Win32::Devices::Properties::{DEVPKEY_Device_ContainerId, DEVPROP_TYPE_GUID, DEVPROPTYPE};
use windows::Win32::Foundation::{DEVPROPKEY, PROPERTYKEY};
use windows::Win32::Media::Audio::{
    DEVICE_STATE, EDataFlow, IMMDevice, IMMDeviceEnumerator, IMMEndpoint, MMDeviceEnumerator, PKEY_AudioEndpoint_FormFactor, eAll,
    eCapture, eRender,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToGUID, PropVariantToStringAlloc, PropVariantToUInt32};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::core::{GUID, Interface, PCWSTR, PWSTR, w};

/// Batería que Windows guarda en el nodo del aparato o en el de manos libres (0–100).
const BATERIA: DEVPROPKEY = DEVPROPKEY { fmtid: GUID::from_u128(0x104ea319_6ee2_4701_bd47_8ddbf425bbe5), pid: 2 };

/// En cada punto de sonido: el nodo PnP del que sale, como "{1}.BTHENUM\{0000110B-…}\…_E89E1361CEB7_…".
pub const RUTA_DEL_APARATO: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0xb3f8fa53_0004_438e_9003_51a46e139bfc), pid: 2 };

/// Contenedores que no sirven para unir: el vacío y el del propio computador.
const SIN_CONTENEDOR: [GUID; 2] = [GUID::zeroed(), GUID::from_u128(0x00000000_0000_0000_ffff_ffffffffffff)];

// ---------------------------------------------------------------- COM

/// Prepara COM en este hilo (multihilo). Si ya estaba listo, o en el otro modo, igual sirve.
/// No se cierra: los hilos de Tauri se reusan y quedan listos para la próxima vez.
pub fn iniciar_com() {
    // S_OK, S_FALSE y RPC_E_CHANGED_MODE dejan COM usable en este hilo.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

/// Un texto que entrega COM (y que hay que liberar).
pub fn texto_com(texto: PWSTR) -> Option<String> {
    if texto.is_null() {
        return None;
    }
    let valor = unsafe { texto.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(texto.0 as *const c_void)) };
    valor
}

/// Texto terminado en cero para las funciones de Windows.
pub fn ancho(texto: &str) -> Vec<u16> {
    texto.encode_utf16().chain(Some(0)).collect()
}

// ---------------------------------------------------------------- Direcciones

/// "E8:9E:13:61:CE:B7" (o con guiones, o en minúsculas) → 0xE89E1361CEB7.
pub fn direccion_a_numero(direccion: &str) -> Option<u64> {
    let limpia: String = direccion.chars().filter(|c| !matches!(c, ':' | '-')).collect();
    if limpia.len() != 12 || !limpia.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(&limpia, 16).ok()
}

/// 0xE89E1361CEB7 → "E8:9E:13:61:CE:B7", igual que en la Mac.
pub fn numero_a_direccion(numero: u64) -> String {
    (0..6).rev().map(|i| format!("{:02X}", (numero >> (i * 8)) & 0xFF)).collect::<Vec<_>>().join(":")
}

/// La dirección tal como aparece dentro de los ID de Windows: "E89E1361CEB7".
pub fn direccion_en_ids(numero: u64) -> String {
    format!("{:012X}", numero & 0xFFFF_FFFF_FFFF)
}

// ---------------------------------------------------------------- Nodos PnP

/// Los ID de los nodos Bluetooth (clásicos y LE) que Windows conoce, para buscar los de cada aparato.
pub struct NodosBluetooth {
    /// (ID tal cual, ID en mayúsculas).
    ids: Vec<(String, String)>,
}

impl NodosBluetooth {
    /// Solo lee la lista de ID (unos pocos milisegundos); las propiedades se piden después, por aparato.
    pub fn leer() -> Self {
        let ids = [w!("BTHENUM"), w!("BTHLE"), w!("BTHLEDEVICE")]
            .into_iter()
            .flat_map(lista_de_ids)
            .map(|id| {
                let mayusculas = id.to_uppercase();
                (id, mayusculas)
            })
            .collect();
        Self { ids }
    }

    /// Los nodos de un aparato: los que llevan su dirección ("E89E1361CEB7") en el ID.
    fn de<'a>(&'a self, clave: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        self.ids.iter().filter(move |(_, m)| m.contains(clave)).map(|(o, m)| (o.as_str(), m.as_str()))
    }

    /// Marca y modelo, de un ID como `…_VID&0001004C_PID&2006…` o `…_DEV_VID&02046D_PID&B023…`.
    pub fn marca_y_modelo(&self, clave: &str) -> Option<(u16, u16)> {
        self.de(clave).find_map(|(_, m)| marca_y_modelo(m))
    }

    /// La batería que muestra Windows, si el aparato la informa. Primero se mira el nodo
    /// del aparato y el de manos libres, que es donde suele estar.
    pub fn bateria(&self, clave: &str) -> Option<u8> {
        let mut nodos: Vec<_> = self.de(clave).collect();
        nodos.sort_by_key(|(_, m)| {
            if m.contains("\\DEV_") {
                0
            } else if m.contains("{0000111E") {
                1
            } else {
                2
            }
        });
        nodos.into_iter().find_map(|(id, _)| {
            // Solo nodos presentes: los que quedaron de antes pueden tener un valor viejo.
            let nodo = ubicar(id, false)?;
            let (_, datos) = propiedad(nodo, &BATERIA)?;
            datos.first().copied().filter(|&nivel| nivel <= 100)
        })
    }

    /// Los contenedores del aparato (casi siempre uno): sus puntos de sonido llevan el mismo.
    pub fn contenedores(&self, clave: &str) -> Vec<GUID> {
        let mut lista = Vec::new();
        for (id, _) in self.de(clave) {
            let Some(nodo) = ubicar(id, true) else { continue };
            let Some((tipo, datos)) = propiedad(nodo, &DEVPKEY_Device_ContainerId) else { continue };
            if tipo != DEVPROP_TYPE_GUID || datos.len() < 16 {
                continue;
            }
            let contenedor = unsafe { std::ptr::read_unaligned(datos.as_ptr() as *const GUID) };
            if !SIN_CONTENEDOR.contains(&contenedor) && !lista.contains(&contenedor) {
                lista.push(contenedor);
            }
        }
        lista
    }
}

/// Todos los ID de un enumerador ("BTHENUM"…), presentes o no.
fn lista_de_ids(enumerador: PCWSTR) -> Vec<String> {
    // Si aparece un aparato entre las dos llamadas, la lista crece: se reintenta.
    for _ in 0..3 {
        let mut tam = 0u32;
        let r = unsafe { CM_Get_Device_ID_List_SizeW(&mut tam, enumerador, CM_GETIDLIST_FILTER_ENUMERATOR) };
        if r != CR_SUCCESS || tam <= 1 {
            return Vec::new();
        }
        let mut buf = vec![0u16; tam as usize];
        let r = unsafe { CM_Get_Device_ID_ListW(enumerador, &mut buf, CM_GETIDLIST_FILTER_ENUMERATOR) };
        if r == CR_SUCCESS {
            return buf.split(|&c| c == 0).filter(|s| !s.is_empty()).map(String::from_utf16_lossy).collect();
        }
        if r != CR_BUFFER_SMALL {
            return Vec::new();
        }
    }
    Vec::new()
}

/// El nodo PnP con ese ID. Con `fantasma`, también los que no están presentes.
pub fn ubicar(id: &str, fantasma: bool) -> Option<u32> {
    let texto = ancho(id);
    let mut nodo = 0u32;
    let modo = if fantasma { CM_LOCATE_DEVNODE_PHANTOM } else { CM_LOCATE_DEVNODE_NORMAL };
    let r = unsafe { CM_Locate_DevNodeW(&mut nodo, PCWSTR(texto.as_ptr()), modo) };
    (r == CR_SUCCESS).then_some(nodo)
}

/// Una propiedad de un nodo PnP: su tipo y sus bytes.
pub fn propiedad(nodo: u32, clave: &DEVPROPKEY) -> Option<(DEVPROPTYPE, Vec<u8>)> {
    let mut tipo = DEVPROPTYPE(0);
    let mut tam = 0u32;
    let r = unsafe { CM_Get_DevNode_PropertyW(nodo, clave, &mut tipo, None, &mut tam, 0) };
    if r != CR_BUFFER_SMALL || tam == 0 || tam > 64 * 1024 {
        return None;
    }
    let mut datos = vec![0u8; tam as usize];
    let r = unsafe { CM_Get_DevNode_PropertyW(nodo, clave, &mut tipo, Some(datos.as_mut_ptr()), &mut tam, 0) };
    if r != CR_SUCCESS {
        return None;
    }
    datos.truncate(tam as usize);
    Some((tipo, datos))
}

fn marca_y_modelo(id: &str) -> Option<(u16, u16)> {
    let vid = hex_tras(id, "VID&")?;
    let pid = hex_tras(id, "PID&")?;
    // "0001004C" u "02046D": delante va de dónde sale el número; la marca son los últimos 4 dígitos.
    let marca = u16::from_str_radix(&vid[vid.len() - 4..], 16).ok()?;
    let modelo = u16::from_str_radix(&pid[..4], 16).ok()?;
    (marca != 0 || modelo != 0).then_some((marca, modelo))
}

/// Los dígitos hexadecimales que siguen a `marca` (al menos 4).
fn hex_tras<'a>(texto: &'a str, marca: &str) -> Option<&'a str> {
    let inicio = texto.find(marca)? + marca.len();
    let resto = &texto[inicio..];
    let fin = resto.find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(resto.len());
    (fin >= 4).then(|| &resto[..fin])
}

// ---------------------------------------------------------------- Puntos de sonido

/// Una salida o un micrófono de Windows (un "endpoint" de sonido).
pub struct Punto {
    pub id: String,
    pub aparato: IMMDevice,
    /// true si es salida; false si es micrófono.
    pub salida: bool,
    /// Forma del punto (EndpointFormFactor): audífonos, manos libres, HDMI…
    pub forma: Option<u32>,
}

pub fn enumerador() -> Option<IMMDeviceEnumerator> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.ok()
}

pub fn id_de(aparato: &IMMDevice) -> Option<String> {
    texto_com(unsafe { aparato.GetId() }.ok()?)
}

/// true si el punto es una salida, false si es un micrófono.
pub fn es_salida(aparato: &IMMDevice) -> Option<bool> {
    let sentido: EDataFlow = unsafe { aparato.cast::<IMMEndpoint>().ok()?.GetDataFlow() }.ok()?;
    if sentido == eRender {
        Some(true)
    } else if sentido == eCapture {
        Some(false)
    } else {
        None
    }
}

pub fn tienda(aparato: &IMMDevice) -> Option<IPropertyStore> {
    unsafe { aparato.OpenPropertyStore(STGM_READ) }.ok()
}

pub fn texto(tienda: &IPropertyStore, clave: &PROPERTYKEY) -> Option<String> {
    let mut valor = unsafe { tienda.GetValue(clave) }.ok()?;
    let texto = unsafe { PropVariantToStringAlloc(&valor) }.ok().and_then(texto_com);
    let _ = unsafe { PropVariantClear(&mut valor) };
    texto.filter(|t| !t.trim().is_empty())
}

pub fn guid(tienda: &IPropertyStore, clave: &PROPERTYKEY) -> Option<GUID> {
    let mut valor = unsafe { tienda.GetValue(clave) }.ok()?;
    let guid = unsafe { PropVariantToGUID(&valor) }.ok();
    let _ = unsafe { PropVariantClear(&mut valor) };
    guid
}

pub fn numero(tienda: &IPropertyStore, clave: &PROPERTYKEY) -> Option<u32> {
    let mut valor = unsafe { tienda.GetValue(clave) }.ok()?;
    let numero = unsafe { PropVariantToUInt32(&valor) }.ok();
    let _ = unsafe { PropVariantClear(&mut valor) };
    numero
}

/// El contenedor de un punto de sonido. Si su tienda no lo tiene, se mira su nodo PnP
/// (`SWD\MMDEVAPI\{0.0.0.00000000}.{…}`).
fn contenedor_de(id: &str, tienda: &IPropertyStore) -> Option<GUID> {
    guid(tienda, &PKEY_Device_ContainerId).or_else(|| {
        let nodo = ubicar(&format!("SWD\\MMDEVAPI\\{id}"), true)?;
        let (tipo, datos) = propiedad(nodo, &DEVPKEY_Device_ContainerId)?;
        (tipo == DEVPROP_TYPE_GUID && datos.len() >= 16).then(|| unsafe { std::ptr::read_unaligned(datos.as_ptr() as *const GUID) })
    })
}

/// Las salidas y micrófonos de un aparato Bluetooth en los estados pedidos (DEVICE_STATE_*).
/// Se reconocen por la dirección en el nodo del que salen (A2DP) o por el contenedor (manos libres).
pub fn puntos_bluetooth(e: &IMMDeviceEnumerator, direccion: u64, estados: u32) -> Vec<Punto> {
    let clave = direccion_en_ids(direccion);
    let contenedores = NodosBluetooth::leer().contenedores(&clave);
    let Ok(coleccion) = (unsafe { e.EnumAudioEndpoints(eAll, DEVICE_STATE(estados)) }) else {
        return Vec::new();
    };
    let cantidad = unsafe { coleccion.GetCount() }.unwrap_or(0);
    (0..cantidad)
        .filter_map(|i| {
            let aparato = unsafe { coleccion.Item(i) }.ok()?;
            let id = id_de(&aparato)?;
            let tienda = tienda(&aparato)?;
            let ruta = texto(&tienda, &RUTA_DEL_APARATO).unwrap_or_default().to_uppercase();
            let mio = ruta.contains(&clave) || contenedor_de(&id, &tienda).is_some_and(|c| contenedores.contains(&c));
            if !mio {
                return None;
            }
            Some(Punto {
                salida: es_salida(&aparato)?,
                forma: numero(&tienda, &PKEY_AudioEndpoint_FormFactor),
                id,
                aparato,
            })
        })
        .collect()
}
