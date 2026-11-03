// Llamadas a Rust para Pantallas y Ajustes: modos de pantalla, pantalla principal, permisos, arranque e historial.
// Fuera de la app (por ejemplo `pnpm preview` en el navegador) usa datos de ejemplo para ver el diseño.
import { invoke } from '@tauri-apps/api/core';
import type { ModoPantalla, Permisos, SeccionPrivacidad } from './tipos-sistema';

const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** Corre en macOS (los textos y algunas funciones cambian en Windows). */
export const esMac = typeof navigator !== 'undefined' && /Mac/i.test(navigator.userAgent);

/** Los modos de una pantalla (resolución y frecuencia), con el actual marcado. */
export const modosPantalla = (id: number): Promise<ModoPantalla[]> =>
  enApp ? invoke<ModoPantalla[]>('modos_pantalla', { id }) : Promise.resolve(modosEjemplo(id));

/** Cambia el modo de una pantalla. */
export const cambiarModoPantalla = (id: number, modo: number): Promise<void> =>
  enApp ? invoke<void>('cambiar_modo_pantalla', { id, modo }) : cambiarModoEjemplo(id, modo);

/** La vuelve la pantalla principal (barra de menús y Dock en Mac; barra de tareas en Windows). */
export const hacerPrincipal = (id: number): Promise<void> =>
  enApp ? invoke<void>('hacer_principal', { id }) : Promise.resolve();

/** Borra el historial de uso y de batería de todos los aparatos. Las reglas automáticas no cambian. */
export const borrarHistorial = (): Promise<void> => (enApp ? invoke<void>('borrar_historial') : Promise.resolve());

export const leerPermisos = (): Promise<Permisos> =>
  enApp ? invoke<Permisos>('permisos') : Promise.resolve({ ...permisosEjemplo });

/** Abre la sección del permiso en los ajustes del sistema. */
export const abrirAjustesPrivacidad = (seccion: SeccionPrivacidad): Promise<void> =>
  enApp ? invoke<void>('abrir_ajustes_privacidad', { seccion }) : Promise.resolve();

/** Si Nexo se abre al iniciar sesión. */
export const leerArranque = (): Promise<boolean> =>
  enApp ? invoke<boolean>('arranque') : Promise.resolve(arranqueEjemplo);

/** Activa o quita el arranque al iniciar sesión. Devuelve cómo quedó. */
export const usarArranque = (activar: boolean): Promise<boolean> =>
  enApp ? invoke<boolean>('usar_arranque', { activar }) : Promise.resolve((arranqueEjemplo = activar));

// --- Datos de ejemplo (solo fuera de la app) ---

let arranqueEjemplo = true;

// Como en el escritorio real: las notificaciones siempre llegan como concedidas (por eso Ajustes no muestra su estado).
const permisosEjemplo: Permisos = { accesibilidad: true, bluetooth: 'concedido', notificaciones: 'concedido' };

/** Arma los modos de ejemplo: cada tamaño con sus frecuencias. */
function modos(
  primerId: number,
  tamanos: { ancho: number; alto: number; retina: boolean; hz: number[] }[],
  actual: [number, number, number],
): ModoPantalla[] {
  let id = primerId;
  return tamanos.flatMap((t) =>
    t.hz.map((hz) => ({
      id: id++,
      ancho: t.ancho,
      alto: t.alto,
      pixelesAncho: t.retina ? t.ancho * 2 : t.ancho,
      pixelesAlto: t.retina ? t.alto * 2 : t.alto,
      hz,
      retina: t.retina,
      actual: t.ancho === actual[0] && t.alto === actual[1] && hz === actual[2],
    })),
  );
}

// Las dos pantallas de la Mac del dueño: la MacBook Pro de 14" y el Steren de 27".
const modosDeEjemplo: Record<number, ModoPantalla[]> = {
  1: modos(
    100,
    [
      { ancho: 1800, alto: 1169, retina: true, hz: [120, 60, 50, 48] },
      { ancho: 1512, alto: 982, retina: true, hz: [120, 60, 50, 48] },
      { ancho: 1352, alto: 878, retina: true, hz: [120, 60, 50, 48] },
      { ancho: 1147, alto: 745, retina: true, hz: [120, 60, 50, 48] },
    ],
    [1512, 982, 120],
  ),
  2: modos(
    200,
    [
      { ancho: 1920, alto: 1080, retina: false, hz: [75, 60, 50] },
      { ancho: 1680, alto: 1050, retina: false, hz: [60] },
      { ancho: 1600, alto: 900, retina: false, hz: [60] },
      { ancho: 1440, alto: 900, retina: false, hz: [60] },
      { ancho: 1280, alto: 720, retina: false, hz: [60, 50] },
      { ancho: 960, alto: 540, retina: true, hz: [75, 60] },
    ],
    [1920, 1080, 60],
  ),
};

const modosEjemplo = (id: number): ModoPantalla[] => (modosDeEjemplo[id] ?? []).map((m) => ({ ...m }));

function cambiarModoEjemplo(id: number, modo: number): Promise<void> {
  const lista = modosDeEjemplo[id];
  if (!lista?.some((m) => m.id === modo)) return Promise.reject('Esa pantalla ya no tiene ese modo');
  for (const m of lista) m.actual = m.id === modo;
  return Promise.resolve();
}
