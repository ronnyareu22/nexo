// Llamadas a Rust para la pantalla virtual de la Mac: una segunda pantalla para verla en otra ventana de RustDesk.
// Fuera de la app (por ejemplo `pnpm preview`) usa datos de ejemplo: viene encendida, para ver su tarjeta en
// Pantallas; `?virtual=no` la muestra apagada y `?virtual=nodisponible` como en un macOS que no deja crearla.
import { invoke } from '@tauri-apps/api/core';
import { escuchar } from './api-sistema';
import type { Pantalla } from './tipos';
import type { CambioPantallaVirtual, EstadoPantallaVirtual } from './tipos-sistema';

const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** Las resoluciones que se pueden elegir, en el orden de `RESOLUCIONES` (sistema/pantalla_virtual.rs). */
export const RESOLUCIONES_VIRTUALES: { ancho: number; alto: number; detalle: string }[] = [
  { ancho: 1920, alto: 1080, detalle: 'Full HD, la recomendada' },
  { ancho: 1600, alto: 900, detalle: 'Más liviana para la conexión' },
  { ancho: 2560, alto: 1440, detalle: 'Más espacio, pide más a la conexión' },
  { ancho: 1440, alto: 900, detalle: 'La más liviana' },
];

/** La clave de una resolución en los menús ("1920x1080"). */
export const claveResolucion = (ancho: number, alto: number) => `${ancho}x${alto}`;

/** Las opciones del menú de resolución (con su detalle). */
export const opcionesResolucionVirtual = RESOLUCIONES_VIRTUALES.map((r) => ({
  valor: claveResolucion(r.ancho, r.alto),
  nombre: `${r.ancho} × ${r.alto}`,
  detalle: r.detalle,
}));

/** La resolución de una clave del menú, si es de la lista. */
export function resolucionDe(clave: string): { ancho: number; alto: number } | null {
  const r = RESOLUCIONES_VIRTUALES.find((x) => claveResolucion(x.ancho, x.alto) === clave);
  return r ? { ancho: r.ancho, alto: r.alto } : null;
}

/** Si existe, su resolución y las dos opciones de la sesión remota. */
export const leerPantallaVirtual = (): Promise<EstadoPantallaVirtual> =>
  enApp ? invoke<EstadoPantallaVirtual>('pantalla_virtual_estado') : Promise.resolve({ ...ejemplo });

/** La crea (`activar`), le cambia la resolución o la quita. Devuelve cómo quedó. */
export const usarPantallaVirtual = (activar: boolean, ancho?: number, alto?: number): Promise<EstadoPantallaVirtual> =>
  enApp
    ? invoke<EstadoPantallaVirtual>('pantalla_virtual', { activar, ancho: ancho ?? null, alto: alto ?? null })
    : usarEjemplo(activar, ancho, alto);

/** "Crear al empezar una sesión remota" y "Quitar al terminar la sesión remota". */
export const ajustarPantallaVirtual = (cambio: CambioPantallaVirtual): Promise<EstadoPantallaVirtual> =>
  enApp ? invoke<EstadoPantallaVirtual>('pantalla_virtual_ajustar', { cambio }) : ajustarEjemplo(cambio);

/** Cambió la pantalla virtual (desde el panel, el menú del ícono o la sesión remota). */
export function alCambiarPantallaVirtual(alLlegar: (e: EstadoPantallaVirtual) => void): () => void {
  if (enApp) return escuchar('pantalla-virtual-cambio', alLlegar);
  oyentes.add(alLlegar);
  return () => void oyentes.delete(alLlegar);
}

// --- Datos de ejemplo (solo fuera de la app) ---

const prueba = !enApp && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('virtual') : null;

/** El número de la pantalla virtual de ejemplo (la MacBook es 1 y el Steren 2). */
const ID_EJEMPLO = 3;

const ejemplo: EstadoPantallaVirtual = {
  activa: prueba !== 'no' && prueba !== 'nodisponible',
  ancho: 1920,
  alto: 1080,
  disponible: prueba !== 'nodisponible',
  id: prueba !== 'no' && prueba !== 'nodisponible' ? ID_EJEMPLO : null,
  quitarAlTerminar: false,
  crearAlEmpezar: false,
};

const oyentes = new Set<(e: EstadoPantallaVirtual) => void>();

function avisarEjemplo() {
  for (const o of oyentes) o({ ...ejemplo });
}

/** Fuera de la app: la pantalla virtual de ejemplo para la lista de pantallas, si está encendida. */
export const pantallasVirtualesEjemplo = (): Pantalla[] =>
  ejemplo.activa
    ? [{ id: ID_EJEMPLO, nombre: 'Nexo · Pantalla virtual', interna: false, brillo: null, controlable: false, virtual: true }]
    : [];

// Imita a macOS: tarda un momento en crearla o cambiarla.
function usarEjemplo(activar: boolean, ancho?: number, alto?: number): Promise<EstadoPantallaVirtual> {
  if (!ejemplo.disponible) return Promise.reject('Este macOS no deja crear pantallas virtuales.');
  if (ancho !== undefined && alto !== undefined && !resolucionDe(claveResolucion(ancho, alto)))
    return Promise.reject(`La pantalla virtual no tiene ${ancho} × ${alto}. Elige una de la lista.`);
  return new Promise((listo) =>
    setTimeout(() => {
      ejemplo.activa = activar;
      ejemplo.id = activar ? ID_EJEMPLO : null;
      if (ancho !== undefined && alto !== undefined) {
        ejemplo.ancho = ancho;
        ejemplo.alto = alto;
      }
      avisarEjemplo();
      listo({ ...ejemplo });
    }, 700),
  );
}

function ajustarEjemplo(cambio: CambioPantallaVirtual): Promise<EstadoPantallaVirtual> {
  if (cambio.quitarAlTerminar !== undefined) ejemplo.quitarAlTerminar = cambio.quitarAlTerminar;
  if (cambio.crearAlEmpezar !== undefined) ejemplo.crearAlEmpezar = cambio.crearAlEmpezar;
  avisarEjemplo();
  return Promise.resolve({ ...ejemplo });
}
