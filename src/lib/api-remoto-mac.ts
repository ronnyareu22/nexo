// Llamadas a Rust para manejar la Mac desde Nexo en Windows, por XDrop (sistema/remoto/mando.rs): su pantalla
// virtual, las opciones de la sesión y el modo trabajo. La Mac es la del perfil (perfil.svelte.ts): la que encontró
// Nexo entre tus equipos de Tailscale o, si duerme, la que recuerda.
// Fuera de la app usa una Mac de ejemplo: `?mac=apagada` no responde, `?mac=sin` no aparece en Tailscale,
// `?mac=vieja` tiene una versión anterior de Nexo y `?mac=libre` está sin sesión ni pantalla virtual.
import { invoke } from '@tauri-apps/api/core';
import type { CambioMac, DestinoDrop, EstadoMac, OtroEquipo } from './tipos-sistema';

const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** Tu Mac, a partir del otro equipo del perfil. `enLinea`: Nexo respondió allá en la última búsqueda. */
export function macDelPerfil(otro: OtroEquipo | null): DestinoDrop | null {
  if (!enApp && prueba === 'sin') return null;
  if (!otro || otro.plataforma !== 'mac') return null;
  return {
    nombre: otro.nombre,
    ip: otro.ip,
    plataforma: 'mac',
    enLinea: otro.nexo,
    tailscaleEnLinea: otro.enLinea,
    version: null,
  };
}

/** Cómo está la Mac: su pantalla virtual, la sesión de RustDesk, el modo trabajo y el reposo. */
export const leerMac = (ip: string): Promise<EstadoMac> =>
  enApp ? invoke<EstadoMac>('remoto_mac_estado', { ip }) : leerEjemplo();

/** Enciende (`activar`), apaga o cambia de resolución la pantalla virtual de la Mac. Devuelve cómo quedó. */
export const usarPantallaVirtualMac = (ip: string, activar: boolean, ancho?: number, alto?: number): Promise<EstadoMac> =>
  enApp
    ? invoke<EstadoMac>('remoto_mac_pantalla_virtual', { ip, activar, ancho: ancho ?? null, alto: alto ?? null })
    : virtualEjemplo(activar, ancho, alto);

/** "Crear al empezar", "Quitar al terminar" o el modo trabajo de la Mac. Devuelve cómo quedó. */
export const ajustarMac = (ip: string, cambio: CambioMac): Promise<EstadoMac> =>
  enApp ? invoke<EstadoMac>('remoto_mac_ajustar', { ip, cambio }) : ajustarEjemplo(cambio);

// --- Datos de ejemplo (solo fuera de la app) ---

const prueba = !enApp && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('mac') : null;
const libre = prueba === 'libre';

const ejemplo: EstadoMac = {
  equipo: 'demo-sige',
  plataforma: 'mac',
  pantallaVirtual: {
    activa: !libre,
    ancho: 1920,
    alto: 1080,
    disponible: true,
    quitarAlTerminar: true,
    crearAlEmpezar: false,
  },
  sesion: libre
    ? { activa: false, directa: false, rele: false, equipo: null }
    : { activa: true, directa: true, rele: false, equipo: 'innotech1017' },
  modoTrabajo: !libre,
  evitarReposo: true,
};

const copia = (): EstadoMac => ({ ...ejemplo, pantallaVirtual: { ...ejemplo.pantallaVirtual }, sesion: { ...ejemplo.sesion } });

const esperar = (ms: number) => new Promise((listo) => setTimeout(listo, ms));

async function leerEjemplo(): Promise<EstadoMac> {
  await esperar(300);
  if (prueba === 'apagada') throw 'Tu Mac no responde: revisa que Nexo esté abierto allá.';
  if (prueba === 'vieja') throw 'Tu Mac tiene una versión anterior de Nexo: actualízala para manejarla desde aquí.';
  return copia();
}

// Imita a la Mac: encender o cambiar la pantalla virtual tarda un momento.
async function virtualEjemplo(activar: boolean, ancho?: number, alto?: number): Promise<EstadoMac> {
  await esperar(900);
  ejemplo.pantallaVirtual.activa = activar;
  if (ancho !== undefined && alto !== undefined) {
    ejemplo.pantallaVirtual.ancho = ancho;
    ejemplo.pantallaVirtual.alto = alto;
  }
  return copia();
}

async function ajustarEjemplo(cambio: CambioMac): Promise<EstadoMac> {
  await esperar(200);
  const { modoTrabajo, ...virtual } = cambio;
  Object.assign(ejemplo.pantallaVirtual, virtual);
  if (modoTrabajo !== undefined) ejemplo.modoTrabajo = modoTrabajo;
  return copia();
}
