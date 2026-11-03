// Tipos del sistema que manda la parte de Rust para Pantallas y Ajustes (modos de pantalla, permisos, arranque).
// Van con `serde(rename_all = "camelCase")`, igual que tipos.ts.
import type { Pantalla } from './tipos';

/** Una pantalla con el dato de si es la principal (la que tiene la barra de menús o la barra de tareas). */
export type PantallaSistema = Pantalla & { principal: boolean };

/** Un modo de una pantalla: tamaño en puntos, píxeles reales y frecuencia. */
export interface ModoPantalla {
  /** Identificador del modo dentro de esa pantalla; con él se cambia. */
  id: number;
  /** Tamaño en puntos (lo que se ve como "1512 × 982"). */
  ancho: number;
  alto: number;
  /** Píxeles reales del modo (en Retina, el doble de los puntos). */
  pixelesAncho: number;
  pixelesAlto: number;
  /** Frecuencia en Hz; 0 si el sistema no la informa. */
  hz: number;
  /** Modo Retina (HiDPI): cada punto usa más de un píxel. */
  retina: boolean;
  /** Es el modo en uso. */
  actual: boolean;
}

export type EstadoPermiso = 'concedido' | 'denegado' | 'pendiente' | 'restringido';

/** Permisos del sistema que usa Nexo. */
export interface Permisos {
  /** Para las teclas de brillo en el monitor bajo el mouse (solo Mac). */
  accesibilidad: boolean;
  bluetooth: 'concedido' | 'denegado' | 'pendiente' | 'restringido';
  notificaciones: 'concedido' | 'denegado' | 'pendiente';
}

/** Sección de Privacidad y seguridad que se abre en los ajustes del sistema. */
export type SeccionPrivacidad = 'accesibilidad' | 'bluetooth' | 'notificaciones';
