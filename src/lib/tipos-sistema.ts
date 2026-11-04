// Tipos del sistema que manda la parte de Rust para Pantallas y Ajustes (modos de pantalla, permisos, arranque,
// widget), para la bienvenida y para Remoto (RustDesk por Tailscale, atajos, captura y portapapeles).
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

/** Permisos del sistema que usa Nexo. "restringido": lo bloquea un perfil del equipo. */
export interface Permisos {
  /** Para las teclas de brillo en el monitor bajo el mouse (solo Mac). */
  accesibilidad: boolean;
  bluetooth: EstadoPermiso;
  /** En Mac trae el estado real; en Windows siempre llega concedido. */
  notificaciones: EstadoPermiso;
  /** Grabación de pantalla, para la captura con Imprimir pantalla desde el PC (solo Mac; en Windows, true). */
  pantalla: boolean;
}

/** Sección de Privacidad y seguridad que se abre en los ajustes del sistema; también es la clave de `pedir_permiso`. */
export type SeccionPrivacidad = 'accesibilidad' | 'bluetooth' | 'notificaciones' | 'pantalla';

/** La bienvenida de la primera vez (`leer_bienvenida`). */
export interface Bienvenida {
  /** Ya se terminó una vez en este equipo. */
  completa: boolean;
  /** El nombre guardado o, si no hay, el nombre de pila de la cuenta del sistema. Puede venir vacío. */
  nombre: string;
}

/** El sistema en que corre Nexo (cambia la barra de título del panel). */
export type Plataforma = 'mac' | 'windows' | 'otro';

/** Adónde abrir el panel: una sección y, en Dispositivos, la página de un aparato (`Destino` en lib.rs). */
export interface DestinoPanel {
  seccion: 'inicio' | 'dispositivos' | 'audio' | 'pantallas' | 'remoto' | 'automatizaciones' | 'ajustes';
  dispositivo?: string | null;
}

// --- Remoto (`sistema/remoto.rs`) ---

/** Ajustes del remoto, guardados en ajustes.json › remoto. Todos vienen encendidos. */
export interface AjustesRemoto {
  /** Traducir los atajos de Windows que llegan por RustDesk. */
  traducir: boolean;
  /** Imprimir pantalla del PC (F13 en la Mac) hace una captura liviana. */
  captura: boolean;
  /** Durante la sesión, la Mac no se duerme. */
  evitarReposo: boolean;
  /** Durante la sesión, la pantalla de la MacBook baja al mínimo. */
  brilloMinimo: boolean;
  /** Durante la sesión, las imágenes grandes que se copian se achican para que RustDesk no se congele. */
  portapapelesLiviano: boolean;
}

/** Lo que cambia `remoto_ajustar`; lo que no viene queda igual. */
export type CambioRemoto = Partial<AjustesRemoto> & { modoTrabajo?: boolean };

/** Un atajo de Windows y su equivalente en la Mac (la tabla viene de Rust, la misma que traduce). */
export interface AtajoRemoto {
  grupo: string;
  windows: string[];
  mac: string[];
  accion: string;
}

export type EstadoTailscale = 'activo' | 'detenido' | 'no instalado';

/** `remoto_estado`: la sesión de RustDesk, Tailscale, los ajustes y la tabla de atajos. */
export interface EstadoRemoto {
  activa: boolean;
  /** El otro lado: el PC si es directa; el servidor de RustDesk si va por el relé. */
  ipRemota: string | null;
  /** El nombre del PC en Tailscale, si se sabe. */
  equipo: string | null;
  /** Directa por Tailscale (100.x). */
  directa: boolean;
  /** Por el relé de RustDesk (más lenta). */
  rele: boolean;
  tailscale: EstadoTailscale;
  /** Mac: RustDesk está abierto. Windows: está instalado. */
  rustdesk: boolean;
  rustdeskInstalado: boolean;
  ajustes: AjustesRemoto;
  atajos: AtajoRemoto[];
  /** La Mac no se duerme aunque todavía no haya sesión ("Preparar para el trabajo"). */
  modoTrabajo: boolean;
  /** Accesibilidad: para traducir atajos y escuchar Imprimir pantalla. */
  permisoTeclas: boolean;
  /** Grabación de pantalla: para la captura. */
  permisoPantalla: boolean;
  /** La app aparte "Captura remota" sigue abierta (Nexo ya hace lo mismo). */
  capturaRemotaAbierta: boolean;
  /** La dirección de la Mac en Tailscale (la que se escribe en RustDesk). */
  ipMac: string;
}

/** Una captura liviana ya copiada. */
export interface CapturaRemota {
  ancho: number;
  alto: number;
  kb: number;
  /** Dónde se guardó la copia; null si no se pudo guardar. */
  archivo: string | null;
  copiada: boolean;
}

/** Una copia del historial del portapapeles. */
export interface EntradaPortapapeles {
  id: number;
  tipo: 'texto' | 'imagen';
  /** Los primeros 2000 caracteres. */
  texto: string | null;
  /** Cuántos caracteres tiene el texto completo. */
  largo: number | null;
  /** Miniatura de la imagen (data URL). */
  miniatura: string | null;
  ancho: number | null;
  alto: number | null;
  /** Segundos desde 1970. */
  momento: number;
  /** "remoto": llegó desde el PC (estimación). */
  origen: 'mac' | 'remoto';
}
