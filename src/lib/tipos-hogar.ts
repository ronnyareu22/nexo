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
  | { tipo: 'homeAssistant'; url: string };

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
