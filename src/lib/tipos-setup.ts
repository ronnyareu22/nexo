// Tipos de Mi setup (src-tauri/src/sistema/setup.rs y escenas.rs), con `serde(rename_all = "camelCase")`.
import type { Tipo } from './tipos';

export type PlataformaSetup = 'mac' | 'windows' | 'otro';

/** Un aparato Bluetooth, en corto: lo que hace falta para dibujarlo y decir si está encendido. */
export interface Aparato {
  id: string;
  nombre: string;
  alias: string | null;
  tipo: Tipo;
  conectado: boolean;
  /** El nivel que lo resume: el general o, en audífonos sueltos, el más bajo de los dos. */
  bateria: number | null;
  fabricante: string | null;
  vendorId: string | null;
  productId: string | null;
}

export interface PantallaSetup {
  id: number;
  nombre: string;
  interna: boolean;
  brillo: number | null;
}

/** Lo que tiene un equipo (`GET /nexo/aparatos` en Nexo Drop). */
export interface Aparatos {
  /** Su nombre en Tailscale ("demo-sige"). */
  equipo: string;
  plataforma: PlataformaSetup;
  dispositivos: Aparato[];
  pantallas: PantallaSetup[];
}

/** Otro equipo del dueño y lo que tiene. */
export interface EquipoRemoto {
  nombre: string;
  ip: string;
  plataforma: PlataformaSetup;
  /** Nexo respondió en ese equipo. */
  enLinea: boolean;
  tailscaleEnLinea: boolean;
  /** Sus aparatos: los de ahora o, si no responde, los últimos que se vieron (null si nunca se vieron). */
  aparatos: Aparatos | null;
  /** `aparatos` es de ahora; false: es lo último que se vio. */
  alDia: boolean;
  /** Cuándo se vieron (segundos desde 1970). */
  visto: number | null;
  /** Por qué no se pudieron leer ahora. */
  error: string | null;
}

/** `setup_estado`. */
export interface EstadoSetup {
  local: Aparatos;
  remotos: EquipoRemoto[];
}

// --- Mapa ---

export type LugarId = 'casa' | 'oficina';

/** "equipo" es la computadora misma: la MacBook o la torre del PC. */
export type TipoItem = 'aparato' | 'pantalla' | 'equipo' | 'deco';

/** Las decoraciones: fotos en static/setup/<nombre>.png. Son solo visuales. */
export type Deco =
  | 'planta-grande'
  | 'planta-pequena'
  | 'lampara'
  | 'taza'
  | 'cuadro'
  | 'repisa'
  | 'tira-luz'
  | 'soporte-audifonos'
  | 'alfombrilla'
  | 'base-monitor'
  | 'parlante';

/** Algo puesto en el mapa. Las posiciones van en % de la escena. */
export interface ItemMapa {
  id: string;
  tipo: TipoItem;
  /** Dirección del aparato, nombre de la pantalla, nombre del equipo o clase de decoración. */
  ref: string;
  /** Centro, de 0 a 100 a lo ancho. */
  x: number;
  /** Solo lo de la pared: dónde apoya su borde de abajo, de 0 a 100 a lo alto. */
  y: number | null;
  /** 0 fondo, 1 medio, 2 frente. */
  capa: number;
  escala: number;
  /** El equipo al que pertenece; null en las decoraciones. */
  equipo: string | null;
}

export interface LugarMapa {
  items: ItemMapa[];
}

/** Un lugar en null todavía no se acomodó: se dibuja uno con los aparatos que haya. */
export interface MapaSetup {
  casa: LugarMapa | null;
  oficina: LugarMapa | null;
}

// --- Escenas ---

export type Accion =
  | { tipo: 'brillo'; pantalla: 'principal' | 'todas'; valor: number }
  | { tipo: 'conectar'; dispositivo: string; nombre?: string | null }
  | { tipo: 'desconectar'; dispositivo: string; nombre?: string | null }
  | { tipo: 'salida'; dispositivo: string; nombre?: string | null }
  | { tipo: 'volumen'; valor: number }
  | { tipo: 'volumenMicrofono'; valor: number }
  | { tipo: 'modoTrabajo'; activo: boolean }
  | { tipo: 'abrirApp'; nombre: string }
  | { tipo: 'luces' }
  | { tipo: 'desconocida' };

export type TipoAccion = Accion['tipo'];

export interface Escena {
  /** Vacío en una escena nueva: lo pone Rust al guardarla. */
  id: string;
  nombre: string;
  icono: string;
  acciones: Accion[];
}

export interface UltimaEscena {
  id: string;
  momento: number;
  ok: boolean;
}

/** `escenas`, `guardar_escena` y `borrar_escena`. */
export interface EstadoEscenas {
  escenas: Escena[];
  ultima: UltimaEscena | null;
}

/** Cómo salió un paso. `omitida`: no se ejecutó a propósito (las luces). */
export interface ResultadoAccion {
  accion: Accion;
  ok: boolean;
  error: string | null;
  omitida: boolean;
}

/** "escena-progreso": empieza el paso `indice` (desde 0) de `total`. */
export interface AvanceEscena {
  id: string;
  indice: number;
  total: number;
  accion: Accion;
}

/** "escena-activa": terminó (también si se activó desde el ícono de la barra de menús o la bandeja). */
export interface FinEscena {
  id: string;
  resultados: ResultadoAccion[];
}
