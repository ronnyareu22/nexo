// Lo que manda la parte de Rust (src-tauri/src/dispositivos/mod.rs).

export type Tipo =
  | 'audifonos'
  | 'diadema'
  | 'telefono'
  | 'teclado'
  | 'mouse'
  | 'parlante'
  | 'monitor'
  | 'luz'
  | 'portatil'
  | 'otro';

export interface Dispositivo {
  id: string;
  /** Nombre real del aparato, tal como lo da el Bluetooth. */
  nombre: string;
  /** Nombre que el usuario le puso dentro de Nexo; si está, se muestra en lugar de `nombre`. */
  alias: string | null;
  tipo: Tipo;
  conectado: boolean;
  bateria: number | null;
  bateriaIzquierda: number | null;
  bateriaDerecha: number | null;
  bateriaEstuche: number | null;
  fabricante: string | null;
  firmware: string | null;
  vendorId: string | null;
  productId: string | null;
}

export interface Sonido {
  tieneSalida: boolean;
  salidaEnUso: boolean;
  volumen: number | null;
  silenciado: boolean | null;
  tieneMicrofono: boolean;
  microfonoEnUso: boolean;
  volumenMicrofono: number | null;
}

export type CambioSonido =
  | { accion: 'usarSalida' }
  | { accion: 'volumen'; valor: number }
  | { accion: 'silenciar'; valor: boolean }
  | { accion: 'usarMicrofono' }
  | { accion: 'volumenMicrofono'; valor: number };

/** Cómo se conecta un aparato de sonido. */
export type TransporteAudio =
  | 'integrado'
  | 'bluetooth'
  | 'usb'
  | 'hdmi'
  | 'displayport'
  | 'airplay'
  | 'virtual'
  | 'otro';

/** Un aparato de sonido del sistema (no solo Bluetooth), para la página Audio. */
export interface DispositivoAudio {
  /** Identificador estable del sistema; con él se elige o se ajusta. */
  uid: string;
  /** Nombre tal como lo da el sistema ("Bocinas de MacBook Pro"). */
  nombre: string;
  /** Se puede elegir como salida (bocinas, audífonos). */
  salida: boolean;
  /** Se puede elegir como entrada (micrófono). */
  entrada: boolean;
  salidaEnUso: boolean;
  entradaEnUso: boolean;
  /** Volumen de salida 0–100; null si el aparato no lo deja leer. */
  volumen: number | null;
  silenciado: boolean | null;
  /** Volumen del micrófono 0–100; null si el aparato no lo deja leer. */
  volumenEntrada: number | null;
  transporte: TransporteAudio;
}

/** El nombre para mostrar: el alias si lo tiene, si no el nombre real. */
export const nombreDe = (d: Dispositivo) => d.alias?.trim() || d.nombre;

/** El nivel de batería que resume al aparato: el general o, en audífonos sueltos, el más bajo de los dos. */
export function nivelPrincipal(d: Dispositivo): number | null {
  if (d.bateria !== null) return d.bateria;
  const lados = [d.bateriaIzquierda, d.bateriaDerecha].filter((n): n is number => n !== null);
  return lados.length ? Math.min(...lados) : null;
}

/** Aparatos que pueden tener controles de sonido. */
export const esDeSonido = (d: Dispositivo) => ['audifonos', 'diadema', 'parlante'].includes(d.tipo);

export type EstadoConexion = 'conectado' | 'conectando' | 'desconectando' | 'desconectado';

/** Estado de conexión para mostrar, según si hay un cambio en curso. */
export const estadoDe = (d: Dispositivo, pendiente: boolean): EstadoConexion =>
  pendiente ? (d.conectado ? 'desconectando' : 'conectando') : d.conectado ? 'conectado' : 'desconectado';

export interface Estado {
  bluetoothEncendido: boolean;
  dispositivos: Dispositivo[];
}

export const nombreTipo: Record<Tipo, string> = {
  audifonos: 'Audífonos',
  diadema: 'Audífonos con micrófono',
  telefono: 'Teléfono',
  teclado: 'Teclado',
  mouse: 'Mouse',
  parlante: 'Parlante',
  monitor: 'Monitor',
  luz: 'Luz',
  portatil: 'Portátil',
  otro: 'Dispositivo Bluetooth',
};

/** Las baterías que tenga el aparato, con su nombre para mostrar. */
export function baterias(d: Dispositivo): { nombre: string; nivel: number }[] {
  const lista = [
    { nombre: 'Carga', nivel: d.bateria },
    { nombre: 'Izquierdo', nivel: d.bateriaIzquierda },
    { nombre: 'Derecho', nivel: d.bateriaDerecha },
    { nombre: 'Estuche', nivel: d.bateriaEstuche },
  ];
  return lista.filter((b): b is { nombre: string; nivel: number } => b.nivel !== null);
}

export interface Pantalla {
  id: number;
  nombre: string;
  interna: boolean;
  brillo: number | null;
  controlable: boolean;
}

export interface EstadoAjustes {
  teclasBrillo: boolean;
  /** Si macOS ya le dio a Nexo el permiso de Accesibilidad. */
  permiso: boolean;
}

/** Historial de uso de un aparato (lo lleva Nexo). Momentos en segundos Unix. */
export interface Registro {
  /** [inicio, fin]; fin null si sigue conectado. Última semana. */
  sesiones: [number, number | null][];
  /** [momento, nivel] de las últimas 24 h. */
  bateria: [number, number][];
}

export interface Automatico {
  usarSalidaAlConectar: boolean;
  avisarBateriaBaja: boolean;
}
