// Tipos del sistema que manda la parte de Rust para Pantallas y Ajustes (modos de pantalla, permisos, arranque,
// widget), para la bienvenida, para Remoto (RustDesk por Tailscale, atajos, captura y portapapeles) y para
// Nexo Drop (archivos, textos y enlaces entre tus equipos).
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
  seccion: 'inicio' | 'dispositivos' | 'audio' | 'pantallas' | 'remoto' | 'drop' | 'automatizaciones' | 'ajustes';
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

// --- Nexo Drop (`sistema/drop.rs`) ---

export type PlataformaEquipo = 'mac' | 'windows' | 'otro';

/** Otro equipo tuyo en Tailscale al que se le puede mandar (`drop_destinos`). */
export interface DestinoDrop {
  /** Su nombre en Tailscale ("innotech1017"). */
  nombre: string;
  /** Su dirección de Tailscale; con ella se manda. */
  ip: string;
  plataforma: PlataformaEquipo;
  /** Nexo respondió en ese equipo. */
  enLinea: boolean;
  /** Tailscale lo ve conectado (aunque Nexo no responda). */
  tailscaleEnLinea: boolean;
  version: string | null;
}

/** Ajustes de Nexo Drop, guardados en ajustes.json › drop. Los dos vienen encendidos. */
export interface AjustesDrop {
  /** El receptor: apagado, este equipo no escucha en la red. */
  recibir: boolean;
  /** Si llega una sola imagen, también queda en el portapapeles. */
  copiarImagenes: boolean;
}

export type CambioDrop = Partial<AjustesDrop>;

/** `drop_ajustes`: cómo está Nexo Drop en este equipo. */
export interface EstadoDrop {
  ajustes: AjustesDrop;
  /** La dirección donde escucha ahora; null si no escucha. */
  escuchando: string | null;
  /** Este equipo en Tailscale. */
  nombre: string;
  plataforma: PlataformaEquipo;
  /** Dónde queda lo recibido (Descargas › Nexo Drop). */
  carpeta: string | null;
  tailscale: EstadoTailscale;
  puerto: number;
}

export type EstadoTransferencia = 'esperando' | 'transfiriendo' | 'listo' | 'cancelado' | 'error';

/** Avance de un envío o de algo que llega ("drop-progreso"). */
export interface ProgresoDrop {
  lote: string;
  /** "enviado": sale de aquí; "recibido": llega aquí. */
  direccion: 'enviado' | 'recibido';
  /** El otro equipo. */
  equipo: string;
  /** El archivo en curso (desde 0) y cuántos son. */
  indice: number;
  total: number;
  /** Ruta relativa del archivo en curso ("Fotos/a.jpg"). */
  nombre: string;
  /** Bytes de este archivo. */
  enviados: number;
  tamano: number;
  /** Bytes de todo el envío (al recibir no se sabe el total: 0). */
  enviadosLote: number;
  tamanoLote: number;
  /** Bytes por segundo. */
  velocidad: number;
  estado: EstadoTransferencia;
  error: string | null;
}

/** Una entrada del historial de Nexo Drop (`drop_historial`). */
export interface EntradaDrop {
  id: number;
  direccion: 'enviado' | 'recibido';
  tipo: 'archivo' | 'carpeta' | 'varios' | 'texto' | 'enlace';
  /** El archivo o la carpeta; en "varios", el primero. */
  nombre: string;
  /** Cuántos archivos llegaron o salieron, y cuántos tenía el envío. */
  cantidad: number;
  total: number;
  tamano: number;
  equipo: string;
  /** Segundos desde 1970. */
  momento: number;
  /** Dónde quedó (o de dónde salió). */
  ruta: string | null;
  /** El texto o el enlace; null si era un texto oculto (una contraseña). */
  texto: string | null;
  estado: 'completo' | 'en curso' | 'cancelado' | 'error';
  error: string | null;
  lote: string | null;
  /** Miniatura de la imagen (data URL). */
  miniatura: string | null;
  /** Si el archivo sigue donde quedó. */
  existe: boolean;
}

/** `drop_enviar_portapapeles`: qué se mandó. */
export interface EnvioCopiado {
  tipo: 'texto' | 'imagen' | 'archivos';
  lote: string | null;
  cantidad: number;
}

/** Archivos arrastrados sobre la ventana de Nexo (con sus rutas reales). */
export type ArrastreArchivos =
  | { tipo: 'encima'; rutas: string[] }
  | { tipo: 'fuera' }
  | { tipo: 'soltar'; rutas: string[] };
