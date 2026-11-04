//! Bluetooth en la Mac: CoreBluetooth con un delegado propio, en una cola privada (nunca la principal).
//!
//! - `buscar`: enciende un CBCentralManager, escucha anuncios durante el plazo y lo suelta todo al terminar.
//! - `escribir`: busca la luz por su identificador, conecta, encuentra la característica de escritura, manda las
//!   tramas y se desconecta. Todo con plazo.
//! - Las órdenes al gestor y al periférico van en su cola; las respuestas llegan por un canal.

use super::textos;
use super::tramas::{self, Anuncio, ESCRITURA, Protocolo, Recuerdo, uuid_corto};
use crate::sistema::hogar::modelo::Cambio;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::{Allocated, Retained, autoreleasepool};
use objc2::runtime::{AnyClass, AnyObject, MessageReceiver, NSObject, Sel};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSNumber, NSObjectProtocol, NSString, NSUUID};
use std::collections::HashMap;
use std::ptr::null_mut;
use std::sync::Mutex;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

#[link(name = "CoreBluetooth", kind = "framework")]
unsafe extern "C" {
    static CBCentralManagerOptionShowPowerAlertKey: &'static NSString;
    static CBCentralManagerScanOptionAllowDuplicatesKey: &'static NSString;
    static CBAdvertisementDataLocalNameKey: &'static NSString;
    static CBAdvertisementDataServiceUUIDsKey: &'static NSString;
    static CBAdvertisementDataManufacturerDataKey: &'static NSString;
    static CBAdvertisementDataServiceDataKey: &'static NSString;
}

// CBManagerState
const ESTADO_SIN_SOPORTE: isize = 2;
const ESTADO_SIN_PERMISO: isize = 3;
const ESTADO_APAGADO: isize = 4;
const ESTADO_ENCENDIDO: isize = 5;
// CBCharacteristicProperties y CBCharacteristicWriteType
const PUEDE_ESCRIBIR_SIN_RESPUESTA: usize = 0x04;
const CON_RESPUESTA: isize = 0;
const SIN_RESPUESTA: isize = 1;

/// Pausa entre tramas sin respuesta (`elkbledom` espera 0,1–0,3 s).
const PAUSA: Duration = Duration::from_millis(150);

/// Lo que avisa el delegado.
enum Evento {
    Estado(isize),
    Visto(Anuncio),
    /// La luz que se buscaba para manejarla.
    Hallado(Periferico),
    Conectado,
    NoConecto,
    Desconectado,
    Servicios(bool),
    /// Terminó de leer las características de un servicio (con o sin error: se revisa después).
    Caracteristicas,
    Escrito(bool),
}

/// Un periférico retenido que cruza de la cola al hilo que espera.
struct Periferico(Retained<AnyObject>);
// SAFETY: solo se usa para mandarle mensajes desde la cola de CoreBluetooth o mientras esa cola espera.
unsafe impl Send for Periferico {}

/// Un objeto de Objective-C para usar dentro de la cola.
#[derive(Clone, Copy)]
struct Ptr(*mut AnyObject);
// SAFETY: los objetos siguen vivos (los retiene `Sesion` o el periférico) mientras la cola los usa.
unsafe impl Send for Ptr {}
unsafe impl Sync for Ptr {}

impl Ptr {
    /// El puntero (por un método: así los cierres capturan `Ptr` entero y no solo su campo).
    fn o(self) -> *mut AnyObject {
        self.0
    }
}

struct Ivars {
    tx: Mutex<mpsc::Sender<Evento>>,
    /// Al manejar una luz: su identificador, en mayúsculas. Al buscar: nada.
    buscado: Option<String>,
}

define_class!(
    // SAFETY: NSObject no pide nada especial a sus subclases y `Delegado` no implementa Drop.
    #[unsafe(super(NSObject))]
    #[name = "NexoHogarBleDelegado"]
    #[ivars = Ivars]
    struct Delegado;

    impl Delegado {
        #[unsafe(method(centralManagerDidUpdateState:))]
        fn al_cambiar_estado(&self, central: *mut AnyObject) {
            if central.is_null() {
                return;
            }
            // SAFETY: `central` es el CBCentralManager; `state` es un NSInteger.
            let estado: isize = unsafe { msg_send![central, state] };
            self.avisar(Evento::Estado(estado));
        }

        #[unsafe(method(centralManager:didDiscoverPeripheral:advertisementData:RSSI:))]
        fn al_ver(&self, _central: *mut AnyObject, periferico: *mut AnyObject, datos: *mut AnyObject, rssi: *mut AnyObject) {
            if periferico.is_null() {
                return;
            }
            autoreleasepool(|_| {
                // SAFETY: los punteros son los que entrega CoreBluetooth durante esta llamada.
                let Some(a) = (unsafe { anuncio(periferico, datos, rssi) }) else { return };
                match &self.ivars().buscado {
                    Some(b) if a.direccion.eq_ignore_ascii_case(b) => {
                        // SAFETY: se retiene el periférico para usarlo después de esta llamada.
                        if let Some(p) = unsafe { Retained::retain(periferico) } {
                            self.avisar(Evento::Hallado(Periferico(p)));
                        }
                    }
                    Some(_) => {}
                    None => self.avisar(Evento::Visto(a)),
                }
            });
        }

        #[unsafe(method(centralManager:didConnectPeripheral:))]
        fn al_conectar(&self, _central: *mut AnyObject, _periferico: *mut AnyObject) {
            self.avisar(Evento::Conectado);
        }

        #[unsafe(method(centralManager:didFailToConnectPeripheral:error:))]
        fn al_no_conectar(&self, _central: *mut AnyObject, _periferico: *mut AnyObject, _error: *mut AnyObject) {
            self.avisar(Evento::NoConecto);
        }

        #[unsafe(method(centralManager:didDisconnectPeripheral:error:))]
        fn al_desconectar(&self, _central: *mut AnyObject, _periferico: *mut AnyObject, _error: *mut AnyObject) {
            self.avisar(Evento::Desconectado);
        }

        #[unsafe(method(peripheral:didDiscoverServices:))]
        fn al_ver_servicios(&self, _periferico: *mut AnyObject, error: *mut AnyObject) {
            self.avisar(Evento::Servicios(error.is_null()));
        }

        #[unsafe(method(peripheral:didDiscoverCharacteristicsForService:error:))]
        fn al_ver_caracteristicas(&self, _periferico: *mut AnyObject, _servicio: *mut AnyObject, _error: *mut AnyObject) {
            self.avisar(Evento::Caracteristicas);
        }

        #[unsafe(method(peripheral:didWriteValueForCharacteristic:error:))]
        fn al_escribir(&self, _periferico: *mut AnyObject, _caracteristica: *mut AnyObject, error: *mut AnyObject) {
            self.avisar(Evento::Escrito(error.is_null()));
        }
    }

    unsafe impl NSObjectProtocol for Delegado {}
);

impl Delegado {
    fn nuevo(tx: mpsc::Sender<Evento>, buscado: Option<String>) -> Retained<Self> {
        let este = Self::alloc().set_ivars(Ivars { tx: Mutex::new(tx), buscado });
        // SAFETY: `init` de NSObject sobre un objeto recién reservado.
        unsafe { msg_send![super(este), init] }
    }

    fn avisar(&self, e: Evento) {
        if let Ok(tx) = self.ivars().tx.lock() {
            let _ = tx.send(e);
        }
    }
}

// ---------------------------------------------------------------- Leer lo que llega

/// Si `obj` es de la clase `nombre` (o de una hija).
unsafe fn es(obj: *mut AnyObject, nombre: &std::ffi::CStr) -> bool {
    if obj.is_null() {
        return false;
    }
    let Some(clase) = AnyClass::get(nombre) else { return false };
    // SAFETY: `obj` es un objeto vivo; isKindOfClass: devuelve un BOOL.
    unsafe { msg_send![obj, isKindOfClass: clase] }
}

unsafe fn texto(obj: *mut AnyObject) -> Option<String> {
    // SAFETY: se revisa que sea un NSString antes de leerlo.
    unsafe { es(obj, c"NSString").then(|| (*(obj as *const NSString)).to_string()) }
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

unsafe fn bytes(obj: *mut AnyObject) -> Option<Vec<u8>> {
    // SAFETY: se revisa que sea un NSData antes de copiarlo.
    unsafe { es(obj, c"NSData").then(|| (*(obj as *const NSData)).to_vec()) }
}

/// El UUID de un CBUUID, en corto si es de los de 16 bits ("FFF0").
unsafe fn uuid(cbuuid: *mut AnyObject) -> Option<String> {
    if cbuuid.is_null() {
        return None;
    }
    // SAFETY: CBUUID responde a UUIDString con un NSString.
    unsafe {
        let s: *mut AnyObject = msg_send![cbuuid, UUIDString];
        texto(s).map(|t| uuid_corto(&t))
    }
}

/// Los elementos de un NSArray (vacío si no lo es).
unsafe fn elementos(arreglo: *mut AnyObject) -> Vec<*mut AnyObject> {
    // SAFETY: se revisa la clase; count y objectAtIndex: son de NSArray.
    unsafe {
        if !es(arreglo, c"NSArray") {
            return Vec::new();
        }
        let n: usize = msg_send![arreglo, count];
        (0..n.min(64)).map(|i| msg_send![arreglo, objectAtIndex: i]).collect()
    }
}

/// Lo que anuncia un periférico: su identificador, nombre, señal, servicios y datos.
unsafe fn anuncio(p: *mut AnyObject, datos: *mut AnyObject, rssi: *mut AnyObject) -> Option<Anuncio> {
    // SAFETY: `p` es un CBPeripheral, `datos` un NSDictionary (o nil) y `rssi` un NSNumber (o nil).
    unsafe {
        let id: *mut AnyObject = msg_send![p, identifier];
        if id.is_null() {
            return None;
        }
        let s: *mut AnyObject = msg_send![id, UUIDString];
        let direccion = texto(s)?;
        let mut a = Anuncio { direccion, ..Anuncio::default() };
        if es(rssi, c"NSNumber") {
            let v: isize = msg_send![rssi, integerValue];
            // 127: el sistema no la sabe.
            if (-127..0).contains(&v) {
                a.senal = Some(v as i16);
            }
        }
        if es(datos, c"NSDictionary") {
            let n: *mut AnyObject = msg_send![datos, objectForKey: CBAdvertisementDataLocalNameKey];
            a.nombre = texto(n);
            let lista: *mut AnyObject = msg_send![datos, objectForKey: CBAdvertisementDataServiceUUIDsKey];
            a.servicios = elementos(lista).into_iter().filter_map(|u| uuid(u)).collect();
            let f: *mut AnyObject = msg_send![datos, objectForKey: CBAdvertisementDataManufacturerDataKey];
            if let Some(b) = bytes(f).filter(|b| b.len() >= 2) {
                a.fabricante.push((u16::from_le_bytes([b[0], b[1]]), b[2..].to_vec()));
            }
            let sd: *mut AnyObject = msg_send![datos, objectForKey: CBAdvertisementDataServiceDataKey];
            if es(sd, c"NSDictionary") {
                let llaves: *mut AnyObject = msg_send![sd, allKeys];
                for k in elementos(llaves) {
                    let d: *mut AnyObject = msg_send![sd, objectForKey: k];
                    if let (Some(u), Some(b)) = (uuid(k), bytes(d)) {
                        a.datos_servicio.push((u, b));
                    }
                }
            }
        }
        if a.nombre.is_none() {
            let n: *mut AnyObject = msg_send![p, name];
            a.nombre = texto(n);
        }
        Some(a)
    }
}

// ---------------------------------------------------------------- Sesión

/// El gestor de CoreBluetooth con su delegado y su cola. Al cerrarse no queda nada escuchando.
struct Sesion {
    gestor: Retained<AnyObject>,
    delegado: Retained<Delegado>,
    cola: DispatchRetained<DispatchQueue>,
    rx: mpsc::Receiver<Evento>,
}

/// Si el permiso de Bluetooth ya se negó, ni se intenta.
fn revisar_permiso() -> Result<(), String> {
    match crate::sistema::permisos::bluetooth() {
        "denegado" | "restringido" => Err(sin_permiso()),
        _ => Ok(()),
    }
}

fn sin_permiso() -> String {
    "Nexo no tiene permiso para usar el Bluetooth. Actívalo en Ajustes del Sistema › Privacidad y seguridad › Bluetooth.".into()
}

impl Sesion {
    fn abrir(buscado: Option<String>) -> Result<Sesion, String> {
        let clase = AnyClass::get(c"CBCentralManager").ok_or("Esta Mac no tiene Bluetooth.")?;
        let cola = DispatchQueue::new("com.ronny.nexo.hogar.ble", None);
        let (tx, rx) = mpsc::channel();
        let delegado = Delegado::nuevo(tx, buscado);
        // Sin el aviso de macOS "Enciende el Bluetooth": Nexo lo dice con sus palabras.
        let no = NSNumber::numberWithBool(false);
        // SAFETY: la constante es un NSString de CoreBluetooth.
        let llave: &NSString = unsafe { CBCentralManagerOptionShowPowerAlertKey };
        let opciones = NSDictionary::<NSString, NSNumber>::from_slices(&[llave], &[&*no]);
        let puntero_cola = DispatchRetained::as_ptr(&cola).as_ptr().cast::<AnyObject>();
        let gestor = autoreleasepool(|_| {
            // SAFETY: alloc + initWithDelegate:queue:options: con un delegado, una cola de dispatch y un diccionario.
            unsafe {
                let nuevo: Allocated<AnyObject> = msg_send![clase, alloc];
                let listo: Option<Retained<AnyObject>> =
                    msg_send![nuevo, initWithDelegate: &*delegado, queue: puntero_cola, options: &*opciones];
                listo
            }
        })
        .ok_or("No pude usar el Bluetooth de esta Mac.")?;
        Ok(Sesion { gestor, delegado, cola, rx })
    }

    fn gestor(&self) -> Ptr {
        Ptr(Retained::as_ptr(&self.gestor) as *mut AnyObject)
    }

    fn delegado(&self) -> Ptr {
        Ptr(Retained::as_ptr(&self.delegado) as *mut AnyObject)
    }

    /// Corre algo en la cola de CoreBluetooth y espera a que termine.
    fn en_cola(&self, f: impl FnOnce() + Send) {
        self.cola.exec_sync(move || autoreleasepool(|_| f()));
    }

    /// Espera el primer evento que `f` acepte, hasta `limite`.
    fn esperar<T>(&self, limite: Instant, mut f: impl FnMut(Evento) -> Option<T>) -> Option<T> {
        loop {
            let resto = limite.checked_duration_since(Instant::now())?;
            match self.rx.recv_timeout(resto) {
                Ok(e) => {
                    if let Some(r) = f(e) {
                        return Some(r);
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return None,
            }
        }
    }

    /// El Bluetooth tiene que estar encendido y con permiso.
    fn esperar_encendido(&self) -> Result<(), String> {
        let limite = Instant::now() + Duration::from_secs(3);
        let r = self.esperar(limite, |e| match e {
            Evento::Estado(ESTADO_ENCENDIDO) => Some(Ok(())),
            Evento::Estado(ESTADO_APAGADO) => Some(Err(textos::APAGADO.to_string())),
            Evento::Estado(ESTADO_SIN_PERMISO) => Some(Err(sin_permiso())),
            Evento::Estado(ESTADO_SIN_SOPORTE) => Some(Err("Esta Mac no tiene Bluetooth de bajo consumo.".to_string())),
            _ => None,
        });
        r.unwrap_or_else(|| {
            Err("El Bluetooth no respondió. Si macOS pidió permiso, acéptalo y vuelve a intentar.".into())
        })
    }

    /// Busca todo. Con repetidos: el nombre suele llegar en un segundo anuncio (la respuesta al escaneo).
    fn empezar_a_buscar(&self) {
        let g = self.gestor();
        // SAFETY: la constante es un NSString de CoreBluetooth.
        let llave: &NSString = unsafe { CBCentralManagerScanOptionAllowDuplicatesKey };
        let si = NSNumber::numberWithBool(true);
        let opciones = NSDictionary::<NSString, NSNumber>::from_slices(&[llave], &[&*si]);
        let o = Ptr(Retained::as_ptr(&opciones) as *mut AnyObject);
        self.en_cola(move || {
            let nulo = null_mut::<AnyObject>();
            // SAFETY: scanForPeripheralsWithServices:options: con servicios nil (todos) y un diccionario de opciones.
            let _: () = unsafe { msg_send![g.o(), scanForPeripheralsWithServices: nulo, options: o.o()] };
        });
    }

    fn dejar_de_buscar(&self) {
        let g = self.gestor();
        // SAFETY: stopScan no recibe nada.
        self.en_cola(move || {
            let _: () = unsafe { msg_send![g.o(), stopScan] };
        });
    }

    /// Suelta el delegado en la cola (así no llega nada más) y libera el gestor, el delegado y la cola.
    fn cerrar(self) {
        let g = self.gestor();
        self.en_cola(move || {
            let nulo = null_mut::<AnyObject>();
            // SAFETY: setDelegate: nil deja de avisar.
            let _: () = unsafe { msg_send![g.o(), setDelegate: nulo] };
        });
    }
}

// ---------------------------------------------------------------- Buscar

pub fn buscar(plazo: Duration) -> Result<Vec<Anuncio>, String> {
    revisar_permiso()?;
    let inicio = Instant::now();
    let sesion = Sesion::abrir(None)?;
    let r = escuchar(&sesion, inicio, plazo);
    sesion.cerrar();
    r
}

fn escuchar(s: &Sesion, inicio: Instant, plazo: Duration) -> Result<Vec<Anuncio>, String> {
    s.esperar_encendido()?;
    s.empezar_a_buscar();
    // El plazo cuenta desde que se pidió, pero se escucha al menos 2 s.
    let fin = (inicio + plazo).max(Instant::now() + Duration::from_secs(2));
    let mut vistos: HashMap<String, Anuncio> = HashMap::new();
    s.esperar(fin, |e| {
        if let Evento::Visto(a) = e {
            match vistos.get_mut(&a.direccion) {
                Some(previo) => previo.juntar(a),
                None => {
                    vistos.insert(a.direccion.clone(), a);
                }
            }
        }
        None::<()>
    });
    s.dejar_de_buscar();
    Ok(vistos.into_values().collect())
}

// ---------------------------------------------------------------- Manejar

pub fn escribir(direccion: &str, preferido: Protocolo, cambio: Cambio, recuerdo: Recuerdo) -> Result<(), String> {
    revisar_permiso()?;
    let limite = Instant::now() + Duration::from_secs(8);
    let sesion = Sesion::abrir(Some(direccion.to_ascii_uppercase()))?;
    let r = manejar(&sesion, direccion, preferido, &cambio, &recuerdo, limite);
    sesion.cerrar();
    r
}

fn manejar(s: &Sesion, direccion: &str, preferido: Protocolo, c: &Cambio, r: &Recuerdo, limite: Instant) -> Result<(), String> {
    s.esperar_encendido()?;
    let periferico = encontrar(s, direccion, limite)?;
    let p = Ptr(Retained::as_ptr(&periferico.0) as *mut AnyObject);
    let (g, d) = (s.gestor(), s.delegado());
    s.en_cola(move || {
        let nulo = null_mut::<AnyObject>();
        // SAFETY: el periférico avisa al mismo delegado; connectPeripheral:options: sin opciones.
        unsafe {
            let _: () = msg_send![p.o(), setDelegate: d.o()];
            let _: () = msg_send![g.o(), connectPeripheral: p.o(), options: nulo];
        }
    });
    let conectado = s.esperar(limite, |e| match e {
        Evento::Conectado => Some(true),
        Evento::NoConecto => Some(false),
        _ => None,
    });
    let resultado = match conectado {
        Some(true) => escribir_conectado(s, p, preferido, c, r, limite),
        Some(false) => Err(textos::NO_ENCONTRADA.to_string()),
        None => Err(textos::NO_RESPONDE.to_string()),
    };
    // Siempre se desconecta: la tira queda libre para su app y para la próxima orden.
    s.en_cola(move || {
        let nulo = null_mut::<AnyObject>();
        // SAFETY: cancelar una conexión (aunque no se haya completado) y soltar el delegado del periférico.
        unsafe {
            let _: () = msg_send![g.o(), cancelPeripheralConnection: p.o()];
            let _: () = msg_send![p.o(), setDelegate: nulo];
        }
    });
    let _ = s.esperar(Instant::now() + Duration::from_secs(1), |e| matches!(e, Evento::Desconectado).then_some(()));
    drop(periferico);
    resultado
}

/// El periférico por su identificador: el que ya conoce el sistema o, si no, buscándolo un momento.
fn encontrar(s: &Sesion, direccion: &str, limite: Instant) -> Result<Periferico, String> {
    let uuid = NSUUID::from_string(&NSString::from_str(direccion)).ok_or(textos::NO_ENCONTRADA)?;
    let lista = NSArray::from_retained_slice(&[uuid]);
    let lista = Ptr(Retained::as_ptr(&lista) as *mut AnyObject);
    let g = s.gestor();
    let mut conocido: Option<Periferico> = None;
    s.en_cola(|| {
        // SAFETY: retrievePeripheralsWithIdentifiers: devuelve un NSArray de CBPeripheral.
        unsafe {
            let encontrados: *mut AnyObject = msg_send![g.o(), retrievePeripheralsWithIdentifiers: lista.o()];
            if let Some(&primero) = elementos(encontrados).first() {
                conocido = Retained::retain(primero).map(Periferico);
            }
        }
    });
    if let Some(p) = conocido {
        return Ok(p);
    }
    s.empezar_a_buscar();
    let hasta = limite.min(Instant::now() + Duration::from_secs(5));
    let hallado = s.esperar(hasta, |e| match e {
        Evento::Hallado(p) => Some(p),
        _ => None,
    });
    s.dejar_de_buscar();
    hallado.ok_or_else(|| textos::NO_ENCONTRADA.to_string())
}

/// Ya conectada: encuentra dónde escribir y manda las tramas.
fn escribir_conectado(s: &Sesion, p: Ptr, preferido: Protocolo, c: &Cambio, r: &Recuerdo, limite: Instant) -> Result<(), String> {
    s.en_cola(move || {
        let nulo = null_mut::<AnyObject>();
        // SAFETY: discoverServices: nil pide todos los servicios.
        let _: () = unsafe { msg_send![p.o(), discoverServices: nulo] };
    });
    match s.esperar(limite, |e| match e {
        Evento::Servicios(ok) => Some(ok),
        _ => None,
    }) {
        Some(true) => {}
        Some(false) => return Err(textos::SIN_ESCRITURA.into()),
        None => return Err(textos::NO_RESPONDE.into()),
    }

    // Los servicios que sirven y sus características.
    let buscados: Vec<&str> = ESCRITURA.iter().map(|(_, servicio, _)| *servicio).collect();
    let mut servicios: Vec<(String, Ptr)> = Vec::new();
    s.en_cola(|| {
        // SAFETY: `services` es un NSArray de CBService; cada uno responde a UUID.
        unsafe {
            let lista: *mut AnyObject = msg_send![p.o(), services];
            for sv in elementos(lista) {
                let u: *mut AnyObject = msg_send![sv, UUID];
                if let Some(u) = uuid(u).filter(|u| buscados.contains(&u.as_str())) {
                    servicios.push((u, Ptr(sv)));
                }
            }
        }
    });
    if servicios.is_empty() {
        return Err(textos::SIN_ESCRITURA.into());
    }
    for (_, sv) in &servicios {
        let sv = *sv;
        s.en_cola(move || {
            let nulo = null_mut::<AnyObject>();
            // SAFETY: discoverCharacteristics:forService: con nil pide todas.
            let _: () = unsafe { msg_send![p.o(), discoverCharacteristics: nulo, forService: sv.o()] };
        });
    }
    let mut faltan = servicios.len();
    s.esperar(limite, |e| {
        if matches!(e, Evento::Caracteristicas) {
            faltan -= 1;
        }
        (faltan == 0).then_some(())
    });

    // La característica de escritura: primero la del protocolo esperado.
    let mut candidatas: Vec<(Protocolo, &str, &str)> = ESCRITURA.to_vec();
    candidatas.sort_by_key(|(prot, _, _)| *prot != preferido);
    let mut elegida: Option<(Protocolo, Ptr, usize)> = None;
    s.en_cola(|| {
        for (prot, servicio, caracteristica) in &candidatas {
            let Some((_, sv)) = servicios.iter().find(|(u, _)| u == servicio) else { continue };
            // SAFETY: `characteristics` es un NSArray de CBCharacteristic; `properties` es un NSUInteger.
            unsafe {
                let lista: *mut AnyObject = msg_send![sv.o(), characteristics];
                for ch in elementos(lista) {
                    let u: *mut AnyObject = msg_send![ch, UUID];
                    if uuid(u).as_deref() == Some(*caracteristica) {
                        let propiedades: usize = msg_send![ch, properties];
                        elegida = Some((*prot, Ptr(ch), propiedades));
                        return;
                    }
                }
            }
        }
    });
    let (protocolo, ch, propiedades) = elegida.ok_or(textos::SIN_ESCRITURA)?;

    // El nombre decide la variante de ELK (ELK-BLEDOB, MELK…).
    let mut nombre: Option<String> = None;
    s.en_cola(|| {
        // SAFETY: `name` es un NSString o nil.
        nombre = unsafe { texto(msg_send![p.o(), name]) };
    });
    let sin_respuesta = propiedades & PUEDE_ESCRIBIR_SIN_RESPUESTA != 0;
    for trama in tramas::armar(protocolo, nombre.as_deref(), c, r) {
        let tipo = if sin_respuesta { SIN_RESPUESTA } else { CON_RESPUESTA };
        s.en_cola(move || {
            let datos = NSData::with_bytes(&trama);
            let datos = Retained::as_ptr(&datos) as *mut AnyObject;
            // `type` es palabra de Rust: el selector se arma a mano.
            let sel = Sel::register(c"writeValue:forCharacteristic:type:");
            // SAFETY: writeValue:forCharacteristic:type: con un NSData, un CBCharacteristic y un NSInteger.
            let _: () = unsafe { MessageReceiver::send_message(p.o(), sel, (datos, ch.o(), tipo)) };
        });
        if sin_respuesta {
            std::thread::sleep(PAUSA);
        } else {
            match s.esperar(limite, |e| match e {
                Evento::Escrito(ok) => Some(ok),
                _ => None,
            }) {
                Some(true) => {}
                Some(false) => return Err("La luz no aceptó la orden.".into()),
                None => return Err(textos::NO_RESPONDE.into()),
            }
        }
    }
    Ok(())
}
