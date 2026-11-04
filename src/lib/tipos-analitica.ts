// Tipos de Analítica, los mismos `struct` de src-tauri/src/sistema/analitica.rs (camelCase).
// Lo que un sistema no informa llega en null y la interfaz dice "No disponible".

export type PlataformaEquipo = 'mac' | 'windows' | 'otro';

export interface Cpu {
  /** Uso de todo el equipo, de 0 a 100. */
  total: number;
  /** Uso de cada núcleo (Windows: vacío). */
  nucleos: number[];
  cantidad: number;
}

export type Presion = 'normal' | 'aviso' | 'critica';

export interface Memoria {
  /** Bytes. */
  total: number;
  usada: number;
  apps: number | null;
  cableada: number | null;
  comprimida: number | null;
  cache: number | null;
  libre: number;
  swapUsada: number | null;
  swapTotal: number | null;
  /** Solo Mac. */
  presion: Presion | null;
}

export type EstadoTermico = 'normal' | 'tibia' | 'caliente' | 'muy-caliente';

export interface Temperatura {
  /** El estado térmico de macOS; null donde no se sabe. */
  estado: EstadoTermico | null;
  /** Grados de la batería. */
  bateria: number | null;
}

export interface BateriaEquipo {
  nivel: number;
  cargando: boolean;
  conectada: boolean;
  completa: boolean;
  /** Para vaciarse (sin cargador) o para llenarse (cargando). */
  minutos: number | null;
  ciclos: number | null;
  /** Capacidad de hoy frente a la de fábrica, en %. */
  salud: number | null;
}

export interface Disco {
  total: number;
  libre: number;
}

/** Bytes por segundo. */
export interface Tramo {
  bajada: number;
  subida: number;
}

export interface Red extends Tramo {
  /** Lo que pasa por Tailscale (RustDesk directo y XDrop). */
  tailscale: Tramo | null;
}

/** Una app con todos sus procesos sumados. */
export interface ProcesoApp {
  nombre: string;
  /** Parte de todo el equipo, de 0 a 100. */
  cpu: number;
  /** Bytes. */
  memoria: number;
  procesos: number;
}

export interface Procesos {
  cpu: ProcesoApp[];
  memoria: ProcesoApp[];
  /** Solo se ven los del usuario (Mac). */
  soloTuyos: boolean;
}

export interface Transferencia {
  direccion: 'enviado' | 'recibido';
  equipo: string;
  nombre: string;
  total: number;
  avance: number | null;
  velocidad: number;
}

export interface SesionEquipo {
  rustdesk: boolean;
  activa: boolean;
  directa: boolean;
  rele: boolean;
  equipo: string | null;
  pantallaVirtual: { activa: boolean; ancho: number; alto: number } | null;
  xdrop: Transferencia[];
  /** caffeinate activo: la Mac no se dormirá. */
  sinReposo: boolean;
  /** null en Windows. */
  modoTrabajo: boolean | null;
}

/** Los últimos 5 minutos, en columnas. */
export interface HistorialAnalitica {
  /** Milisegundos entre muestras. */
  cada: number;
  /** Cuándo se tomó cada muestra (ms, reloj del equipo medido). */
  momentos: number[];
  cpu: (number | null)[];
  memoria: (number | null)[];
  bajada: (number | null)[];
  subida: (number | null)[];
}

export interface Analitica {
  equipo: string;
  plataforma: PlataformaEquipo;
  /** Cuándo se midió (ms, reloj del equipo medido). */
  momento: number;
  cpu: Cpu | null;
  memoria: Memoria | null;
  temperatura: Temperatura;
  bateria: BateriaEquipo | null;
  disco: Disco | null;
  red: Red | null;
  /** Segundos desde que arrancó. */
  encendida: number | null;
  procesos: Procesos;
  sesion: SesionEquipo;
  historial: HistorialAnalitica;
}

export interface EquipoAnalitica {
  nombre: string;
  /** null: este equipo. */
  ip: string | null;
  plataforma: PlataformaEquipo;
  /** Nexo respondió allá. */
  enLinea: boolean;
  propio: boolean;
}
