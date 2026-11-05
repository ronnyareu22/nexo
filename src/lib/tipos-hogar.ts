// Tipos de la casa (src-tauri/src/sistema/hogar.rs y hogar/modelo.rs), con `serde(rename_all = "camelCase")`.
// Valores de Nexo: brillo 1–100, temperatura 0 (cálida) – 100 (fría), color con tono 0–360 y saturación y brillo 0–100.

export type ClaseAparato = 'luz' | 'enchufe' | 'tv' | 'cerradura' | 'sensorPuerta' | 'cortina' | 'clima' | 'otro';
export type FormaLuz = 'tira' | 'foco' | 'barra' | 'lampara';
export type Proveedor =
  | 'tuya'
  | 'wiz'
  | 'yeelight'
  | 'govee'
  | 'hue'
  | 'lifx'
  | 'shelly'
  | 'nanoleaf'
  | 'elgato'
  | 'wled'
  /** Tasmota y OpenBeken (aparatos Tuya con firmware abierto): la misma API. */
  | 'tasmota'
  | 'bleElk'
  | 'bleTriones'
  | 'homeAssistant'
  | 'ninguno'
  | 'ejemplo';
export type Via = 'nube' | 'wifi' | 'bluetooth';
export type ModoLuz = 'blanco' | 'color' | 'escena';

export interface Hsv {
  h: number;
  s: number;
  v: number;
}

export interface Capacidades {
  encender: boolean;
  brillo: boolean;
  temperatura: boolean;
  color: boolean;
  volumen: boolean;
  silencio: boolean;
  entradas: boolean;
  abrir: boolean;
  posicion: boolean;
  soloLectura: boolean;
  /** Nunca con un toque ni por voz sin confirmar (abrir una cerradura). */
  requiereConfirmacion: boolean;
  /** Sabe decir cómo está (si no, Nexo recuerda lo último que le mandó). */
  informaEstado: boolean;
}

export interface EstadoAparato {
  encendido: boolean | null;
  brillo: number | null;
  temperatura: number | null;
  color: Hsv | null;
  modo: ModoLuz | null;
  abierto: boolean | null;
  posicion: number | null;
  volumen: number | null;
  silenciado: boolean | null;
  bateria: number | null;
  grados: number | null;
}

export interface Aparato {
  /** "tuya:<id>", "ha:light.sala", "wiz:<mac>", "ble:<dirección>"… */
  id: string;
  proveedor: Proveedor;
  via: Via;
  nombre: string;
  nombreOriginal: string;
  alias: string | null;
  clase: ClaseAparato;
  forma: FormaLuz | null;
  habitacion: string | null;
  marca: string | null;
  modelo: string | null;
  direccion: string | null;
  enLinea: boolean;
  /** Nexo lo maneja; si no, `nota` dice por qué. */
  controlable: boolean;
  capacidades: Capacidades;
  estado: EstadoAparato;
  oculto: boolean;
  entradas: string[];
  entrada: string | null;
  nota: string | null;
}

export interface Habitacion {
  id: string;
  nombre: string;
}

export interface Grupo {
  id: string;
  nombre: string;
  aparatos: string[];
  habitacion: string | null;
}

export interface EstadoCasa {
  aparatos: Aparato[];
  habitaciones: Habitacion[];
  grupos: Grupo[];
  tuya: { accessId: string; region: string; regionNombre: string } | null;
  homeAssistant: { url: string } | null;
  errorTuya: string | null;
  errorHa: string | null;
  actualizado: number | null;
  mostrarTodo: boolean;
  ocultos: number;
  ejemplo: boolean;
  /** Centros de datos de Tuya: [clave, nombre]. */
  regiones: [string, string][];
}

// --- Órdenes (la interfaz, las escenas, el menú del ícono y la voz) ---

export type Destino =
  | { tipo: 'todos'; clase?: ClaseAparato | null }
  | { tipo: 'aparato'; id: string }
  | { tipo: 'grupo'; id: string }
  | { tipo: 'habitacion'; id: string; clase?: ClaseAparato | null }
  | { tipo: 'nombre'; texto: string; clase?: ClaseAparato | null };

export type Accion =
  | { tipo: 'encender' }
  | { tipo: 'apagar' }
  | { tipo: 'alternar' }
  | { tipo: 'brillo'; valor: number }
  | { tipo: 'subir'; paso?: number | null }
  | { tipo: 'bajar'; paso?: number | null }
  | { tipo: 'temperatura'; valor: number }
  | { tipo: 'color'; h: number; s: number; v: number }
  | { tipo: 'abrir' }
  | { tipo: 'cerrar' }
  | { tipo: 'posicion'; valor: number }
  | { tipo: 'volumen'; valor: number }
  | { tipo: 'silenciar'; activo: boolean }
  | { tipo: 'entrada'; nombre: string }
  | { tipo: 'consultar' };

export interface Orden {
  destino: Destino;
  accion: Accion;
  confirmado?: boolean;
}

export interface Falla {
  id: string;
  nombre: string;
  error: string;
}

export interface ResultadoOrden {
  ok: boolean;
  mensaje: string;
  hechos: string[];
  fallas: Falla[];
  requiereConfirmacion: boolean;
  estado: EstadoCasa;
}

/** Lo que se quiere de un aparato en una escena. */
export interface Objetivo {
  encendido: boolean;
  brillo: number | null;
  temperatura: number | null;
  color: Hsv | null;
}

// --- Buscar ---

export type Manejo =
  | { tipo: 'listo' }
  | { tipo: 'cuentaTuya' }
  | { tipo: 'activar'; paso: string }
  | { tipo: 'pronto'; motivo: string }
  | { tipo: 'homeAssistant'; url: string }
  /** Hay que vincularlo una vez: con el botón del aparato (Hue, Nanoleaf) o con su contraseña (Shelly, Tasmota). */
  | { tipo: 'vincular'; forma: FormaVinculo; paso: string };

export type FormaVinculo = 'boton' | 'clave';

export interface Hallazgo {
  id: string;
  via: Via;
  proveedor: Proveedor;
  clase: ClaseAparato;
  forma: FormaLuz | null;
  marca: string;
  nombre: string;
  modelo: string | null;
  direccion: string | null;
  /** La MAC ("a4:2b:b0:12:34:56"), si se sabe: con ella Nexo lo vuelve a encontrar si cambia de IP. */
  mac: string | null;
  /** dBm */
  senal: number | null;
  version: string | null;
  capacidades: Capacidades;
  estado: EstadoAparato | null;
  manejo: Manejo;
  /** Ya está en Nexo. */
  agregado: boolean;
}

export interface ResultadoBusqueda {
  hallazgos: Hallazgo[];
  avisos: string[];
  segundos: number;
  estado: EstadoCasa;
}

export interface CambioAparato {
  alias?: string | null;
  habitacion?: string | null;
  forma?: FormaLuz | null;
}

// --- Vincular (Hue, Nanoleaf: botón; Shelly, Tasmota: contraseña) ---

export interface ResultadoVinculo {
  /** Ya quedó en Nexo. Si no, todavía no se presionó el botón: se vuelve a intentar en un par de segundos. */
  listo: boolean;
  /** "Listo: agregué 5 luces del puente Hue." o "Todavía no veo el botón presionado." */
  mensaje: string;
  estado: EstadoCasa;
}

// --- Tu red ---

export type TipoEquipo =
  | 'router'
  | 'esteEquipo'
  | 'telefono'
  | 'computadora'
  | 'tele'
  | 'parlante'
  | 'luz'
  | 'enchufe'
  | 'camara'
  | 'impresora'
  | 'consola'
  | 'desconocido';

/** nexo: lo maneja Nexo · agregar: se puede agregar · otros: solo se ve. */
export type GrupoRed = 'nexo' | 'agregar' | 'otros';

export interface EquipoRed {
  ip: string;
  /** "a4:2b:b0:12:34:56" */
  mac: string | null;
  /** Dirección privada (la cambia el equipo solo): casi siempre un teléfono o una computadora. */
  privada: boolean;
  nombre: string;
  /** Cómo se anuncia en la red ("iPhone-de-Ana.local"), si no es el nombre que se muestra. */
  nombreRed: string | null;
  /** La marca por la MAC o por lo que contestó. */
  marca: string | null;
  tipo: TipoEquipo;
  grupo: GrupoRed;
  /** Por qué solo se ve (otros) o qué falta para agregarlo (agregar). */
  motivo: string | null;
  /** Para agregarlo o vincularlo (agregar), con su `manejo`. */
  hallazgo: Hallazgo | null;
  /** Los aparatos de Nexo en esta IP (nexo). */
  aparatos: string[];
}

export interface ResultadoRed {
  equipos: EquipoRed[];
  /** La IP de este equipo, la del router y el prefijo de la red (24 = hasta 254 equipos). */
  ip: string | null;
  router: string | null;
  prefijo: number | null;
  /** Lo que no se pudo revisar, en palabras. */
  avisos: string[];
  segundos: number;
  estado: EstadoCasa;
}
