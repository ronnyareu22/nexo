// Llamadas a Rust para Pantallas y Ajustes: modos de pantalla, pantalla principal, permisos, arranque, historial,
// la bienvenida de la primera vez y el widget de escritorio (más los avisos entre el panel y el widget).
// También Remoto: la sesión de RustDesk, los atajos de Windows, la captura liviana y el historial del portapapeles,
// y Nexo Drop: archivos, textos y enlaces entre tus equipos por Tailscale.
// Fuera de la app (por ejemplo `pnpm preview` en el navegador) usa datos de ejemplo para ver el diseño.
import { invoke } from '@tauri-apps/api/core';
import type {
  ArrastreArchivos,
  AtajoRemoto,
  Bienvenida,
  CambioDrop,
  CambioRemoto,
  CapturaRemota,
  DestinoDrop,
  DestinoPanel,
  EntradaDrop,
  EntradaPortapapeles,
  EnvioCopiado,
  EstadoDrop,
  EstadoRemoto,
  ModoPantalla,
  Permisos,
  Plataforma,
  ProgresoDrop,
  SeccionPrivacidad,
} from './tipos-sistema';
import type { Tema } from './tema';

const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const agente = typeof navigator !== 'undefined' ? navigator.userAgent : '';

/** Corre en macOS (los textos y algunas funciones cambian en Windows). */
export const esMac = /Mac/i.test(agente);

/** El sistema: en Mac el contenido llega hasta arriba (botones de la ventana encima); en Windows hay barra de título. */
export const plataforma: Plataforma = esMac ? 'mac' : /Windows/i.test(agente) ? 'windows' : 'otro';
/** Dónde se empareja en cada sistema, para los textos de ayuda. */
export const ajustesBluetoothTexto = esMac ? 'Ajustes › Bluetooth' : 'Configuración › Bluetooth';
export const textoAbrirBluetooth = esMac ? 'Abrir Ajustes de Bluetooth' : 'Abrir Configuración de Bluetooth';

/** Los modos de una pantalla (resolución y frecuencia), con el actual marcado. */
export const modosPantalla = (id: number): Promise<ModoPantalla[]> =>
  enApp ? invoke<ModoPantalla[]>('modos_pantalla', { id }) : Promise.resolve(modosEjemplo(id));

/** Cambia el modo de una pantalla. */
export const cambiarModoPantalla = (id: number, modo: number): Promise<void> =>
  enApp ? invoke<void>('cambiar_modo_pantalla', { id, modo }) : cambiarModoEjemplo(id, modo);

/** La vuelve la pantalla principal (barra de menús y Dock en Mac; barra de tareas en Windows). */
export const hacerPrincipal = (id: number): Promise<void> =>
  enApp ? invoke<void>('hacer_principal', { id }) : Promise.resolve();

/** Borra el historial de uso y de batería de todos los aparatos. Las reglas automáticas no cambian. */
export const borrarHistorial = (): Promise<void> => (enApp ? invoke<void>('borrar_historial') : Promise.resolve());

export const leerPermisos = (): Promise<Permisos> =>
  enApp ? invoke<Permisos>('permisos') : Promise.resolve({ ...permisosEjemplo });

/** Abre la sección del permiso en los ajustes del sistema. */
export const abrirAjustesPrivacidad = (seccion: SeccionPrivacidad): Promise<void> =>
  enApp ? invoke<void>('abrir_ajustes_privacidad', { seccion }) : Promise.resolve();

/** Pide un permiso: muestra el aviso del sistema si aún no se preguntó, o abre su sección de Ajustes si se negó.
 *  Devuelve cómo quedó al momento; si hay que activarlo en Ajustes, el cambio llega al volver a leer. */
export const pedirPermiso = (clave: SeccionPrivacidad): Promise<Permisos> =>
  enApp ? invoke<Permisos>('pedir_permiso', { clave }) : pedirPermisoEjemplo(clave);

/** Los permisos sin los que Nexo no sigue: en Mac, todos; en Windows, ninguno (el sistema no los pide). */
export const permisosRequeridos: SeccionPrivacidad[] =
  plataforma === 'mac' ? ['bluetooth', 'accesibilidad', 'pantalla', 'notificaciones'] : [];

/** Los permisos que solo dicen sí o no (se activan en Ajustes del Sistema, sin aviso de "permitir"). */
export const permisoDeSiONo = (clave: SeccionPrivacidad): clave is 'accesibilidad' | 'pantalla' =>
  clave === 'accesibilidad' || clave === 'pantalla';

/** Si ese permiso ya está concedido. */
export const permisoConcedido = (p: Permisos, clave: SeccionPrivacidad): boolean =>
  permisoDeSiONo(clave) ? p[clave] : p[clave] === 'concedido';

/** Los permisos obligatorios que todavía faltan. */
export const permisosFaltantes = (p: Permisos): SeccionPrivacidad[] =>
  permisosRequeridos.filter((clave) => !permisoConcedido(p, clave));

// --- Bienvenida de la primera vez ---

/** Si la bienvenida ya se hizo y el nombre para saludar (el guardado o el de la cuenta; puede venir vacío). */
export const leerBienvenida = (): Promise<Bienvenida> =>
  enApp ? invoke<Bienvenida>('leer_bienvenida') : Promise.resolve({ ...bienvenidaEjemplo });

/** Guarda el nombre con que Nexo saluda. Devuelve cómo quedó; si está vacío, falla con "Escribe tu nombre.". */
export const guardarNombre = (nombre: string): Promise<string> =>
  enApp ? invoke<string>('guardar_nombre', { nombre }) : guardarNombreEjemplo(nombre);

/** Marca la bienvenida como hecha: no vuelve a aparecer (salvo que falte un permiso). */
export const completarBienvenida = (): Promise<void> =>
  enApp ? invoke<void>('completar_bienvenida') : Promise.resolve(void (bienvenidaEjemplo.completa = true));

/** Si Nexo se abre al iniciar sesión. */
export const leerArranque = (): Promise<boolean> =>
  enApp ? invoke<boolean>('arranque') : Promise.resolve(arranqueEjemplo);

/** Activa o quita el arranque al iniciar sesión. Devuelve cómo quedó. */
export const usarArranque = (activar: boolean): Promise<boolean> =>
  enApp ? invoke<boolean>('usar_arranque', { activar }) : Promise.resolve((arranqueEjemplo = activar));

// --- Widget de escritorio ---

/** Si el widget de escritorio está encendido. */
export const leerWidget = (): Promise<boolean> =>
  enApp ? invoke<boolean>('widget_activo') : Promise.resolve(widgetEjemplo);

/** Enciende o apaga el widget de escritorio. Devuelve cómo quedó. */
export const usarWidget = (activar: boolean): Promise<boolean> =>
  enApp ? invoke<boolean>('usar_widget', { activar }) : Promise.resolve((widgetEjemplo = activar));

/** Abre Nexo (o lo trae al frente) en una sección; con `dispositivo`, en la página de ese aparato. */
export const abrirSeccion = (seccion: DestinoPanel['seccion'], dispositivo?: string): Promise<void> =>
  enApp ? invoke<void>('abrir_seccion', { seccion, dispositivo: dispositivo ?? null }) : Promise.resolve();

/** Abre Nexo donde quedó, sin cambiar de sección. */
export const abrirNexo = (): Promise<void> =>
  enApp ? invoke<void>('abrir_seccion', { seccion: '' }) : Promise.resolve();

/** Empieza a arrastrar la ventana actual (el widget, cuando se arrastra desde una casilla). */
export function moverVentana() {
  if (!enApp) return;
  import('@tauri-apps/api/window')
    .then(({ getCurrentWindow }) => getCurrentWindow().startDragging())
    .catch(() => {});
}

/** Escucha un aviso de Rust o de otra ventana. Devuelve con qué dejar de escuchar. Fuera de la app no hace nada. */
function escuchar<T>(evento: string, alLlegar: (valor: T) => void): () => void {
  if (!enApp) return () => {};
  const quitar = import('@tauri-apps/api/event')
    .then(({ listen }) => listen<T>(evento, (e) => alLlegar(e.payload)))
    .catch(() => () => {});
  return () => {
    quitar.then((f) => f());
  };
}

/** Panel: el widget pidió ir a una sección (Rust manda "nexo-ir" si el panel ya estaba abierto). */
export const alPedirSeccion = (alLlegar: (destino: DestinoPanel) => void) => escuchar('nexo-ir', alLlegar);

/** Widget: el vigía notó un cambio (se conectó algo, cambió una batería). */
export const alCambiarDispositivos = (alLlegar: () => void) => escuchar<null>('widget-actualizar', () => alLlegar());

/** Widget: se cambió el tema en el panel. */
export const alCambiarTema = (alLlegar: (tema: Tema) => void) => escuchar('nexo-tema', alLlegar);

/** Panel: le pasa al widget el tema recién elegido. */
export function avisarTemaAlWidget(tema: Tema) {
  if (!enApp) return;
  import('@tauri-apps/api/event')
    .then(({ emitTo }) => emitTo('widget', 'nexo-tema', tema))
    .catch(() => {});
}

// --- Remoto (RustDesk por Tailscale) ---

/** La sesión, Tailscale, RustDesk, los ajustes y la tabla de atajos. */
export const leerRemoto = (): Promise<EstadoRemoto> =>
  enApp ? invoke<EstadoRemoto>('remoto_estado') : Promise.resolve(copiaRemoto());

/** Cambia uno o varios ajustes del remoto (o el modo trabajo). Devuelve cómo quedó todo. */
export const ajustarRemoto = (cambio: CambioRemoto): Promise<EstadoRemoto> =>
  enApp ? invoke<EstadoRemoto>('remoto_ajustar', { cambio }) : ajustarRemotoEjemplo(cambio);

/** Mac: conecta Tailscale, abre RustDesk y enciende el modo trabajo. Puede tardar unos segundos. */
export const prepararRemoto = (): Promise<EstadoRemoto> =>
  enApp ? invoke<EstadoRemoto>('remoto_preparar') : prepararEjemplo();

/** Mac: captura liviana a mano. null si se canceló con Esc. */
export const capturarRemoto = (): Promise<CapturaRemota | null> =>
  enApp ? invoke<CapturaRemota | null>('remoto_capturar') : capturaEjemplo();

export const abrirRustDesk = (): Promise<void> => (enApp ? invoke<void>('remoto_abrir_rustdesk') : Promise.resolve());

/** Windows: abre RustDesk ya conectándose a la Mac. */
export const conectarMac = (): Promise<void> => (enApp ? invoke<void>('remoto_conectar_mac') : Promise.resolve());

/** Mac: abre Imágenes › Capturas remotas. */
export const abrirCapturas = (): Promise<void> => (enApp ? invoke<void>('remoto_abrir_capturas') : Promise.resolve());

/** Empezó o terminó una sesión de RustDesk. */
export const alCambiarRemoto = (alLlegar: () => void) => escuchar<unknown>('remoto-cambio', () => alLlegar());

/** Se hizo una captura liviana (con Imprimir pantalla o con el botón). */
export const alCapturar = (alLlegar: (c: CapturaRemota) => void) => escuchar('remoto-captura', alLlegar);

/** Las últimas copias del portapapeles, de la más nueva a la más vieja. */
export const leerPortapapeles = (): Promise<EntradaPortapapeles[]> =>
  enApp ? invoke<EntradaPortapapeles[]>('portapapeles_historial') : Promise.resolve(portapapelesEjemplo.map((e) => ({ ...e })));

/** Vuelve a poner esa copia en el portapapeles (y sube al principio). */
export const copiarDelHistorial = (id: number): Promise<void> =>
  enApp ? invoke<void>('portapapeles_copiar', { id }) : copiarEjemplo(id);

/** Borra el historial del portapapeles. */
export const borrarPortapapeles = (): Promise<void> =>
  enApp ? invoke<void>('portapapeles_borrar') : Promise.resolve(void portapapelesEjemplo.splice(0));

/** El historial del portapapeles cambió. */
export const alCambiarPortapapeles = (alLlegar: () => void) =>
  escuchar<unknown>('portapapeles-cambio', () => alLlegar());

// --- Nexo Drop (archivos, textos y enlaces entre tus equipos por Tailscale) ---

/** Tus otros equipos en Tailscale (Mac o Windows) y si Nexo responde en cada uno. Primero los que responden. */
export const leerDestinosDrop = (): Promise<DestinoDrop[]> =>
  enApp ? invoke<DestinoDrop[]>('drop_destinos') : Promise.resolve(destinosEjemplo.map((d) => ({ ...d })));

/** Manda archivos y carpetas (rutas reales) a ese destino. El avance llega por `alProgresoDrop`. */
export const enviarDrop = (rutas: string[], destino: string): Promise<{ lote: string }> =>
  enApp ? invoke<{ lote: string }>('drop_enviar', { rutas, destino }) : enviarDropEjemplo(rutas, destino);

/** Cancela un envío en curso o en cola. */
export const cancelarDrop = (lote: string): Promise<void> =>
  enApp ? invoke<void>('drop_cancelar', { lote }) : Promise.resolve(cancelarDropEjemplo(lote));

/** Lo que se está enviando o recibiendo ahora. */
export const leerEnviosDrop = (): Promise<ProgresoDrop[]> =>
  enApp ? invoke<ProgresoDrop[]>('drop_envios') : Promise.resolve([...enCursoEjemplo.values()].map((p) => ({ ...p })));

/** Manda un texto: llega al portapapeles del otro equipo. */
export const enviarTextoDrop = (destino: string, texto: string): Promise<void> =>
  enApp ? invoke<void>('drop_texto', { destino, texto }) : esperaEjemplo(600);

/** Abre un enlace (http o https) en el navegador del otro equipo. */
export const enviarEnlaceDrop = (destino: string, url: string): Promise<void> =>
  enApp ? invoke<void>('drop_enlace', { destino, url }) : enlaceDropEjemplo(destino, url);

/** Manda lo que tienes copiado: texto, una imagen o archivos copiados. */
export const enviarCopiadoDrop = (destino: string): Promise<EnvioCopiado> =>
  enApp ? invoke<EnvioCopiado>('drop_enviar_portapapeles', { destino }) : copiadoDropEjemplo(destino);

/** El historial de lo enviado y lo recibido (las últimas 40 entradas), con miniaturas de las imágenes. */
export const leerHistorialDrop = (): Promise<EntradaDrop[]> =>
  enApp ? invoke<EntradaDrop[]>('drop_historial') : Promise.resolve(historialDropEjemplo.map((e) => ({ ...e })));

/** Abre el archivo o la carpeta de esa entrada (o el enlace, en este equipo). */
export const abrirDrop = (id: number): Promise<void> => (enApp ? invoke<void>('drop_abrir', { id }) : Promise.resolve());

/** Muestra el archivo o la carpeta de esa entrada en Finder o en el Explorador. */
export const mostrarDrop = (id: number): Promise<void> => (enApp ? invoke<void>('drop_mostrar', { id }) : Promise.resolve());

/** Copia el texto o el enlace de esa entrada al portapapeles. */
export const copiarDrop = (id: number): Promise<void> => (enApp ? invoke<void>('drop_copiar', { id }) : Promise.resolve());

/** Borra el historial de Nexo Drop (los archivos recibidos no se tocan). */
export const borrarHistorialDrop = (): Promise<void> =>
  enApp ? invoke<void>('drop_borrar_historial') : Promise.resolve(void historialDropEjemplo.splice(0));

/** Abre Descargas › Nexo Drop. */
export const abrirCarpetaDrop = (): Promise<void> => (enApp ? invoke<void>('drop_abrir_carpeta') : Promise.resolve());

/** Cómo está Nexo Drop en este equipo: ajustes y si escucha. */
export const leerAjustesDrop = (): Promise<EstadoDrop> =>
  enApp ? invoke<EstadoDrop>('drop_ajustes') : Promise.resolve(copiaEstadoDrop());

/** Cambia "Recibir archivos" o "Copiar imágenes recibidas". Devuelve cómo quedó. */
export const ajustarDrop = (cambio: CambioDrop): Promise<EstadoDrop> =>
  enApp ? invoke<EstadoDrop>('drop_ajustar', { cambio }) : ajustarDropEjemplo(cambio);

/** El avance de lo que se envía o se recibe. */
export function alProgresoDrop(alLlegar: (p: ProgresoDrop) => void): () => void {
  if (enApp) return escuchar('drop-progreso', alLlegar);
  oyentesProgresoEjemplo.add(alLlegar);
  return () => oyentesProgresoEjemplo.delete(alLlegar);
}

/** El historial cambió: llegó algo o se terminó de mandar un texto o un enlace. */
export function alCambiarHistorialDrop(alLlegar: () => void): () => void {
  const quitar = [escuchar<unknown>('drop-recibido', () => alLlegar()), escuchar<unknown>('drop-enviado', () => alLlegar())];
  return () => quitar.forEach((q) => q());
}

/** Archivos arrastrados sobre la ventana (con sus rutas reales, por el arrastre nativo de Tauri). */
export function alArrastrarArchivos(alLlegar: (a: ArrastreArchivos) => void): () => void {
  if (!enApp) return () => {};
  const quitar = import('@tauri-apps/api/webview')
    .then(({ getCurrentWebview }) =>
      getCurrentWebview().onDragDropEvent(({ payload }) => {
        if (payload.type === 'enter') alLlegar({ tipo: 'encima', rutas: payload.paths });
        else if (payload.type === 'drop') alLlegar({ tipo: 'soltar', rutas: payload.paths });
        else if (payload.type === 'leave') alLlegar({ tipo: 'fuera' });
      }),
    )
    .catch(() => () => {});
  return () => {
    quitar.then((f) => f());
  };
}

/** Para revisar el diseño fuera de la app: `?drop=capa` (arrastrando sobre la ventana), `?drop=enviando`,
 *  `?drop=sin` (sin otro equipo), `?drop=apagada` (el otro equipo sin Nexo) y `?drop=varios` (dos destinos). */
export const pruebaDrop: string | null =
  !enApp && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('drop') : null;

// --- Datos de ejemplo (solo fuera de la app) ---

let arranqueEjemplo = true;
let widgetEjemplo = true;

// Para revisar la bienvenida en el navegador: `?bienvenida=1` la abre desde el principio y `?bienvenida=permisos`
// como si ya se hubiera hecho pero faltara un permiso. Con cualquiera de los dos, los permisos empiezan faltando.
const pruebaBienvenida = !enApp && typeof location !== 'undefined' && new URLSearchParams(location.search).has('bienvenida');

const bienvenidaEjemplo: Bienvenida = { completa: true, nombre: 'Ronny' };

const permisosEjemplo: Permisos = pruebaBienvenida
  ? { accesibilidad: false, bluetooth: 'pendiente', notificaciones: 'denegado', pantalla: false }
  : { accesibilidad: true, bluetooth: 'concedido', notificaciones: 'concedido', pantalla: true };

// Imita al sistema: un permiso pendiente se concede al responder el aviso (enseguida); uno negado, la
// Accesibilidad o la Grabación de pantalla se activan en Ajustes, así que llegan un rato después.
function pedirPermisoEjemplo(clave: SeccionPrivacidad): Promise<Permisos> {
  if (permisoDeSiONo(clave)) {
    const deSiONo = clave;
    setTimeout(() => (permisosEjemplo[deSiONo] = true), 2500);
  } else {
    const conAviso = clave;
    if (permisosEjemplo[conAviso] === 'denegado') setTimeout(() => (permisosEjemplo[conAviso] = 'concedido'), 2500);
    else if (permisosEjemplo[conAviso] === 'pendiente') permisosEjemplo[conAviso] = 'concedido';
  }
  return Promise.resolve({ ...permisosEjemplo });
}

function guardarNombreEjemplo(nombre: string): Promise<string> {
  const limpio = nombre.trim().slice(0, 30);
  if (!limpio) return Promise.reject('Escribe tu nombre.');
  bienvenidaEjemplo.nombre = limpio;
  return Promise.resolve(limpio);
}

/** Arma los modos de ejemplo: cada tamaño con sus frecuencias. */
function modos(
  primerId: number,
  tamanos: { ancho: number; alto: number; retina: boolean; hz: number[] }[],
  actual: [number, number, number],
): ModoPantalla[] {
  let id = primerId;
  return tamanos.flatMap((t) =>
    t.hz.map((hz) => ({
      id: id++,
      ancho: t.ancho,
      alto: t.alto,
      pixelesAncho: t.retina ? t.ancho * 2 : t.ancho,
      pixelesAlto: t.retina ? t.alto * 2 : t.alto,
      hz,
      retina: t.retina,
      actual: t.ancho === actual[0] && t.alto === actual[1] && hz === actual[2],
    })),
  );
}

// Las dos pantallas de la Mac del dueño: la MacBook Pro de 14" y el Steren de 27".
const modosDeEjemplo: Record<number, ModoPantalla[]> = {
  1: modos(
    100,
    [
      { ancho: 1800, alto: 1169, retina: true, hz: [120, 60, 50, 48] },
      { ancho: 1512, alto: 982, retina: true, hz: [120, 60, 50, 48] },
      { ancho: 1352, alto: 878, retina: true, hz: [120, 60, 50, 48] },
      { ancho: 1147, alto: 745, retina: true, hz: [120, 60, 50, 48] },
    ],
    [1512, 982, 120],
  ),
  2: modos(
    200,
    [
      { ancho: 1920, alto: 1080, retina: false, hz: [75, 60, 50] },
      { ancho: 1680, alto: 1050, retina: false, hz: [60] },
      { ancho: 1600, alto: 900, retina: false, hz: [60] },
      { ancho: 1440, alto: 900, retina: false, hz: [60] },
      { ancho: 1280, alto: 720, retina: false, hz: [60, 50] },
      { ancho: 960, alto: 540, retina: true, hz: [75, 60] },
    ],
    [1920, 1080, 60],
  ),
};

const modosEjemplo = (id: number): ModoPantalla[] => (modosDeEjemplo[id] ?? []).map((m) => ({ ...m }));

function cambiarModoEjemplo(id: number, modo: number): Promise<void> {
  const lista = modosDeEjemplo[id];
  if (!lista?.some((m) => m.id === modo)) return Promise.reject('Esa pantalla ya no tiene ese modo');
  for (const m of lista) m.actual = m.id === modo;
  return Promise.resolve();
}

// --- Remoto de ejemplo ---
// `?remoto=sin` muestra la Mac sin sesión y `?remoto=rele` una sesión por el relé de RustDesk.

const pruebaRemoto = !enApp && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('remoto') : null;

// La misma tabla que `ATAJOS` en sistema/remoto.rs.
const atajosEjemplo: AtajoRemoto[] = (
  [
    ['Edición', ['Ctrl', 'C'], ['⌘', 'C'], 'Copiar'],
    ['Edición', ['Ctrl', 'V'], ['⌘', 'V'], 'Pegar'],
    ['Edición', ['Ctrl', 'X'], ['⌘', 'X'], 'Cortar'],
    ['Edición', ['Ctrl', 'Z'], ['⌘', 'Z'], 'Deshacer'],
    ['Edición', ['Ctrl', 'Y'], ['⌘', '⇧', 'Z'], 'Rehacer'],
    ['Edición', ['Ctrl', 'A'], ['⌘', 'A'], 'Seleccionar todo'],
    ['Edición', ['Ctrl', 'S'], ['⌘', 'S'], 'Guardar'],
    ['Edición', ['Ctrl', 'F'], ['⌘', 'F'], 'Buscar'],
    ['Edición', ['Ctrl', 'P'], ['⌘', 'P'], 'Imprimir'],
    ['Edición', ['Ctrl', '⌫'], ['⌥', '⌫'], 'Borrar la palabra anterior'],
    ['Ventanas y pestañas', ['Ctrl', 'N'], ['⌘', 'N'], 'Ventana o documento nuevo'],
    ['Ventanas y pestañas', ['Ctrl', 'T'], ['⌘', 'T'], 'Pestaña nueva'],
    ['Ventanas y pestañas', ['Ctrl', 'W'], ['⌘', 'W'], 'Cerrar la pestaña'],
    ['Ventanas y pestañas', ['Ctrl', 'Shift', 'T'], ['⌘', '⇧', 'T'], 'Reabrir la pestaña cerrada'],
    ['Ventanas y pestañas', ['Ctrl', 'R'], ['⌘', 'R'], 'Recargar'],
    ['Ventanas y pestañas', ['Ctrl', 'L'], ['⌘', 'L'], 'Ir a la barra de direcciones'],
    ['Moverse en el texto', ['Inicio'], ['⌘', '←'], 'Principio de la línea'],
    ['Moverse en el texto', ['Fin'], ['⌘', '→'], 'Final de la línea'],
    ['Moverse en el texto', ['Ctrl', 'Inicio'], ['⌘', '↑'], 'Principio del documento'],
    ['Moverse en el texto', ['Ctrl', 'Fin'], ['⌘', '↓'], 'Final del documento'],
    ['Escritorios y apps', ['Ctrl', '← →'], ['⌃', '← →'], 'Cambiar de escritorio'],
    ['Escritorios y apps', ['Ctrl', 'Win', '← →'], ['⌃', '← →'], 'Cambiar de escritorio, con el atajo de Windows'],
    ['Escritorios y apps', ['Ctrl', '↑'], ['⌃', '↑'], 'Mission Control: todos los escritorios'],
    ['Escritorios y apps', ['Alt', 'Tab'], ['⌘', '⇥'], 'Cambiar de app'],
    ['En la Terminal', ['Ctrl', 'C'], ['⌃', 'C'], 'Detener lo que corre (no se traduce)'],
    ['En la Terminal', ['Ctrl', 'Shift', 'C'], ['⌘', 'C'], 'Copiar'],
    ['En la Terminal', ['Ctrl', 'Shift', 'V'], ['⌘', 'V'], 'Pegar'],
  ] as [string, string[], string[], string][]
).map(([grupo, windows, mac, accion]) => ({ grupo, windows, mac, accion }));

const remotoEjemplo: EstadoRemoto = {
  activa: pruebaRemoto !== 'sin',
  ipRemota: pruebaRemoto === 'sin' ? null : pruebaRemoto === 'rele' ? '40.160.225.24' : '100.118.228.87',
  equipo: pruebaRemoto === 'sin' || pruebaRemoto === 'rele' ? null : 'innotech1017',
  directa: pruebaRemoto !== 'sin' && pruebaRemoto !== 'rele',
  rele: pruebaRemoto === 'rele',
  tailscale: 'activo',
  rustdesk: true,
  rustdeskInstalado: true,
  ajustes: { traducir: true, captura: true, evitarReposo: true, brilloMinimo: true, portapapelesLiviano: true },
  atajos: atajosEjemplo,
  modoTrabajo: false,
  permisoTeclas: true,
  permisoPantalla: true,
  capturaRemotaAbierta: false,
  ipMac: '100.88.188.99',
};

const copiaRemoto = (): EstadoRemoto => ({ ...remotoEjemplo, ajustes: { ...remotoEjemplo.ajustes } });

function ajustarRemotoEjemplo(cambio: CambioRemoto): Promise<EstadoRemoto> {
  const { modoTrabajo, ...ajustes } = cambio;
  Object.assign(remotoEjemplo.ajustes, ajustes);
  if (modoTrabajo !== undefined) remotoEjemplo.modoTrabajo = modoTrabajo;
  return Promise.resolve(copiaRemoto());
}

function prepararEjemplo(): Promise<EstadoRemoto> {
  remotoEjemplo.modoTrabajo = true;
  return new Promise((listo) => setTimeout(() => listo(copiaRemoto()), 1500));
}

const capturaEjemplo = (): Promise<CapturaRemota | null> =>
  new Promise((listo) =>
    setTimeout(
      () =>
        listo({
          ancho: 1512,
          alto: 982,
          kb: 214,
          archivo: '/Users/ronny/Pictures/Capturas remotas/Captura 2026-11-03 a las 18.42.10.png',
          copiada: true,
        }),
      1200,
    ),
  );

// Una ventana dibujada, en lugar de una captura de verdad.
const miniaturaEjemplo =
  'data:image/svg+xml;utf8,' +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="160" height="104" viewBox="0 0 160 104">' +
      '<rect width="160" height="104" fill="#e9eef5"/><rect width="160" height="16" fill="#cbd5e1"/>' +
      '<circle cx="9" cy="8" r="2.6" fill="#ef4444"/><circle cx="17" cy="8" r="2.6" fill="#f59e0b"/>' +
      '<circle cx="25" cy="8" r="2.6" fill="#22c55e"/><rect x="10" y="26" width="64" height="8" rx="3" fill="#00b894"/>' +
      '<rect x="10" y="42" width="140" height="5" rx="2.5" fill="#94a3b8"/><rect x="10" y="53" width="120" height="5" rx="2.5" fill="#94a3b8"/>' +
      '<rect x="10" y="64" width="132" height="5" rx="2.5" fill="#94a3b8"/><rect x="10" y="80" width="56" height="14" rx="4" fill="#2563eb"/></svg>',
  );

const haceMin = (min: number) => Math.round(Date.now() / 1000 - min * 60);

const textoLargoEjemplo =
  'Hola equipo, les comparto el resumen de la reunión: el despliegue queda para el jueves a las 18:00, ' +
  'primero se actualiza la base de datos y después el servidor web. Si algo falla, se vuelve a la versión ' +
  'anterior con el respaldo de las 17:30. Cualquier duda me escriben por aquí.';

const portapapelesEjemplo: EntradaPortapapeles[] = [
  { id: 6, tipo: 'texto', texto: 'git push origin main', largo: 20, miniatura: null, ancho: null, alto: null, momento: haceMin(1), origen: 'mac' },
  { id: 5, tipo: 'imagen', texto: null, largo: null, miniatura: miniaturaEjemplo, ancho: 1512, alto: 982, momento: haceMin(4), origen: 'mac' },
  { id: 4, tipo: 'texto', texto: textoLargoEjemplo, largo: textoLargoEjemplo.length, miniatura: null, ancho: null, alto: null, momento: haceMin(12), origen: 'remoto' },
  { id: 3, tipo: 'texto', texto: 'docker compose up -d web\ndocker compose logs -f web', largo: 50, miniatura: null, ancho: null, alto: null, momento: haceMin(26), origen: 'mac' },
  { id: 2, tipo: 'texto', texto: 'https://tailscale.com/kb/1080/cli', largo: 33, miniatura: null, ancho: null, alto: null, momento: haceMin(48), origen: 'remoto' },
];

function copiarEjemplo(id: number): Promise<void> {
  const i = portapapelesEjemplo.findIndex((e) => e.id === id);
  if (i < 0) return Promise.reject('Esa copia ya no está en el historial.');
  const [e] = portapapelesEjemplo.splice(i, 1);
  portapapelesEjemplo.unshift({ ...e, momento: haceMin(0) });
  return Promise.resolve();
}

// --- Nexo Drop de ejemplo ---

const esperaEjemplo = (ms: number): Promise<void> => new Promise((listo) => setTimeout(listo, ms));

const destinosEjemplo: DestinoDrop[] =
  pruebaDrop === 'sin'
    ? []
    : [
        esMac
          ? { nombre: 'innotech1017', ip: '100.118.228.87', plataforma: 'windows', enLinea: pruebaDrop !== 'apagada', tailscaleEnLinea: pruebaDrop !== 'apagada', version: '0.1.0' }
          : { nombre: 'demo-sige', ip: '100.88.188.99', plataforma: 'mac', enLinea: pruebaDrop !== 'apagada', tailscaleEnLinea: pruebaDrop !== 'apagada', version: '0.1.0' },
        ...(pruebaDrop === 'varios'
          ? [{ nombre: 'oficina-sala', ip: '100.71.40.12', plataforma: 'windows' as const, enLinea: false, tailscaleEnLinea: false, version: null }]
          : []),
      ];

const estadoDropEjemplo: EstadoDrop = {
  ajustes: { recibir: true, copiarImagenes: true },
  escuchando: esMac ? '100.88.188.99' : '100.118.228.87',
  nombre: esMac ? 'demo-sige' : 'innotech1017',
  plataforma: esMac ? 'mac' : 'windows',
  carpeta: esMac ? '/Users/ronny/Downloads/Nexo Drop' : 'C:\\Users\\ronny\\Downloads\\Nexo Drop',
  tailscale: 'activo',
  puerto: 47821,
};

const copiaEstadoDrop = (): EstadoDrop => ({ ...estadoDropEjemplo, ajustes: { ...estadoDropEjemplo.ajustes } });

function ajustarDropEjemplo(cambio: CambioDrop): Promise<EstadoDrop> {
  Object.assign(estadoDropEjemplo.ajustes, cambio);
  if (cambio.recibir !== undefined) estadoDropEjemplo.escuchando = cambio.recibir ? (esMac ? '100.88.188.99' : '100.118.228.87') : null;
  return Promise.resolve(copiaEstadoDrop());
}

const oyentesProgresoEjemplo = new Set<(p: ProgresoDrop) => void>();
const enCursoEjemplo = new Map<string, ProgresoDrop>();
const cancelados = new Set<string>();
let siguienteDropEjemplo = 100;

function publicarEjemplo(p: ProgresoDrop) {
  if (p.estado === 'listo' || p.estado === 'cancelado' || p.estado === 'error') enCursoEjemplo.delete(p.lote);
  else enCursoEjemplo.set(p.lote, p);
  for (const o of oyentesProgresoEjemplo) o({ ...p });
}

/** Imita un envío: 12 MB por archivo a unos 6 MB/s, con avisos cada décimo de segundo. */
function simularEnvio(lote: string, nombres: string[], equipo: string) {
  const porArchivo = 12_400_000;
  const p: ProgresoDrop = {
    lote,
    direccion: 'enviado',
    equipo,
    indice: 0,
    total: nombres.length,
    nombre: nombres[0] ?? '',
    enviados: 0,
    tamano: porArchivo,
    enviadosLote: 0,
    tamanoLote: porArchivo * nombres.length,
    velocidad: 6_200_000,
    estado: 'transfiriendo',
    error: null,
  };
  publicarEjemplo(p);
  const paso = setInterval(() => {
    if (cancelados.has(lote)) {
      clearInterval(paso);
      publicarEjemplo({ ...p, estado: 'cancelado' });
      return;
    }
    const n = Math.min(620_000, p.tamano - p.enviados);
    p.enviados += n;
    p.enviadosLote += n;
    if (p.enviados >= p.tamano) {
      if (p.indice + 1 >= p.total) {
        clearInterval(paso);
        publicarEjemplo({ ...p, estado: 'listo' });
        historialDropEjemplo.unshift({
          id: siguienteDropEjemplo++,
          direccion: 'enviado',
          tipo: nombres.length > 1 ? 'varios' : 'archivo',
          nombre: nombres[0] ?? '',
          cantidad: nombres.length,
          total: nombres.length,
          tamano: p.tamanoLote,
          equipo,
          momento: Math.round(Date.now() / 1000),
          ruta: null,
          texto: null,
          estado: 'completo',
          error: null,
          lote,
          miniatura: null,
          existe: false,
        });
        return;
      }
      p.indice += 1;
      p.nombre = nombres[p.indice] ?? '';
      p.enviados = 0;
    }
    publicarEjemplo(p);
  }, 100);
}

function enviarDropEjemplo(rutas: string[], destino: string): Promise<{ lote: string }> {
  const lote = `ejemplo-${siguienteDropEjemplo++}`;
  const equipo = destinosEjemplo.find((d) => d.ip === destino)?.nombre ?? destino;
  simularEnvio(lote, rutas.map((r) => r.split(/[\\/]/).pop() ?? r), equipo);
  return Promise.resolve({ lote });
}

function cancelarDropEjemplo(lote: string) {
  cancelados.add(lote);
}

async function enlaceDropEjemplo(destino: string, url: string): Promise<void> {
  await esperaEjemplo(500);
  historialDropEjemplo.unshift({
    id: siguienteDropEjemplo++,
    direccion: 'enviado',
    tipo: 'enlace',
    nombre: 'Enlace',
    cantidad: 1,
    total: 1,
    tamano: url.length,
    equipo: destinosEjemplo.find((d) => d.ip === destino)?.nombre ?? destino,
    momento: Math.round(Date.now() / 1000),
    ruta: null,
    texto: url,
    estado: 'completo',
    error: null,
    lote: null,
    miniatura: null,
    existe: false,
  });
}

async function copiadoDropEjemplo(destino: string): Promise<EnvioCopiado> {
  await esperaEjemplo(400);
  const lote = `ejemplo-${siguienteDropEjemplo++}`;
  simularEnvio(lote, ['Imagen copiada.png'], destinosEjemplo.find((d) => d.ip === destino)?.nombre ?? destino);
  return { tipo: 'imagen', lote, cantidad: 1 };
}

// `?drop=enviando`: una carpeta a mitad de camino.
if (pruebaDrop === 'enviando' && destinosEjemplo[0]) {
  simularEnvio(
    'ejemplo-carpeta',
    Array.from({ length: 24 }, (_, i) => `Presentación cliente/diapositivas/lamina-${String(i + 1).padStart(2, '0')}.png`),
    destinosEjemplo[0].nombre,
  );
}

// Una foto dibujada, en lugar de una imagen de verdad.
const fotoEjemplo =
  'data:image/svg+xml;utf8,' +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="128" height="96" viewBox="0 0 128 96">' +
      '<rect width="128" height="96" fill="#bfe3f5"/><circle cx="96" cy="26" r="11" fill="#fbbf24"/>' +
      '<path d="M0 78 L34 44 L58 66 L82 40 L128 82 V96 H0z" fill="#16a34a"/>' +
      '<path d="M0 88 L40 60 L76 92 H0z" fill="#15803d"/></svg>',
  );

const equipoOtro = esMac ? 'innotech1017' : 'demo-sige';

const historialDropEjemplo: EntradaDrop[] = [
  { id: 9, direccion: 'recibido', tipo: 'archivo', nombre: 'Captura de pantalla 2026-11-03 182455.png', cantidad: 1, total: 1, tamano: 412_000, equipo: equipoOtro, momento: haceMin(2), ruta: '/Users/ronny/Downloads/Nexo Drop/Captura.png', texto: null, estado: 'completo', error: null, lote: 'a', miniatura: miniaturaEjemplo, existe: true },
  { id: 8, direccion: 'enviado', tipo: 'carpeta', nombre: 'Propuesta SIGE', cantidad: 14, total: 14, tamano: 38_600_000, equipo: equipoOtro, momento: haceMin(9), ruta: '/Users/ronny/Documents/Propuesta SIGE', texto: null, estado: 'completo', error: null, lote: 'b', miniatura: null, existe: true },
  { id: 7, direccion: 'recibido', tipo: 'texto', nombre: 'Texto', cantidad: 1, total: 1, tamano: 74, equipo: equipoOtro, momento: haceMin(16), ruta: null, texto: 'ssh deploy@10.20.0.14 -p 2222\ncd /srv/sige && git pull && docker compose up -d', estado: 'completo', error: null, lote: null, miniatura: null, existe: false },
  { id: 6, direccion: 'enviado', tipo: 'enlace', nombre: 'Enlace', cantidad: 1, total: 1, tamano: 52, equipo: equipoOtro, momento: haceMin(31), ruta: null, texto: 'https://tailscale.com/kb/1080/cli#status', estado: 'completo', error: null, lote: null, miniatura: null, existe: false },
  { id: 5, direccion: 'recibido', tipo: 'archivo', nombre: 'vista-del-cerro.jpg', cantidad: 1, total: 1, tamano: 2_840_000, equipo: equipoOtro, momento: haceMin(75), ruta: '/Users/ronny/Downloads/Nexo Drop/vista-del-cerro.jpg', texto: null, estado: 'completo', error: null, lote: 'c', miniatura: fotoEjemplo, existe: true },
  { id: 4, direccion: 'enviado', tipo: 'varios', nombre: 'Factura octubre.pdf', cantidad: 3, total: 3, tamano: 1_260_000, equipo: equipoOtro, momento: haceMin(140), ruta: null, texto: null, estado: 'completo', error: null, lote: 'd', miniatura: null, existe: false },
  { id: 3, direccion: 'enviado', tipo: 'archivo', nombre: 'respaldo-base-datos-2026-11-02.sql.gz', cantidad: 0, total: 1, tamano: 0, equipo: equipoOtro, momento: haceMin(300), ruta: '/Users/ronny/Desktop/respaldo-base-datos-2026-11-02.sql.gz', texto: null, estado: 'error', error: `No pude conectar con ${equipoOtro}. Revisa que esté encendida y con Nexo abierto.`, lote: 'e', miniatura: null, existe: true },
];
