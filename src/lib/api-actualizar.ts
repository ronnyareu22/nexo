// Actualizaciones de Nexo en Windows desde tu Mac, por XDrop (ver src-tauri/src/sistema/actualizar.rs).
// Fuera de la app (pnpm preview) usa datos de ejemplo; `?actualizacion=1` simula una versión nueva lista.
import { invoke } from '@tauri-apps/api/core';

/** Lo que devuelve `actualizacion_estado` (y llega con "actualizacion-disponible"). Fechas en segundos UTC. */
export interface EstadoActualizacion {
  /** Una Mac tuya ofrece una compilación más nueva que esta. */
  disponible: boolean;
  compiladoActual: number;
  compiladoNuevo: number | null;
  /** La Mac que la ofrece ("demo-sige"). */
  desde: string | null;
  /** El id corto de esta compilación (el commit). */
  build: string;
  /** Si este equipo busca actualizaciones: solo Nexo en Windows. */
  busca: boolean;
  revisado: number | null;
  /** Por qué no se pudo revisar la última vez. */
  problema: string | null;
  instalando: boolean;
}

/** El avance de "Actualizar" ("actualizacion-progreso"). */
export interface AvanceActualizacion {
  fase: 'descargando' | 'verificando' | 'instalando' | 'error';
  porcentaje: number;
}

const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** Cómo está: esta compilación y, si hay, la nueva que ofrece la Mac. */
export const leerActualizacion = (): Promise<EstadoActualizacion> =>
  enApp ? invoke<EstadoActualizacion>('actualizacion_estado') : Promise.resolve({ ...ejemplo });

/** "Buscar actualización": pregunta ahora a tus Mac. */
export const buscarActualizacion = (): Promise<EstadoActualizacion> =>
  enApp
    ? invoke<EstadoActualizacion>('actualizacion_buscar')
    : new Promise((listo) => setTimeout(() => listo({ ...ejemplo, revisado: Date.now() / 1000 }), 900));

/** Baja la versión nueva, la revisa y la instala. Nexo se cierra y el instalador lo vuelve a abrir. */
export const instalarActualizacion = (): Promise<void> =>
  enApp ? invoke<void>('actualizar_instalar') : new Promise((listo) => setTimeout(listo, 1500));

function escuchar<T>(evento: string, alLlegar: (valor: T) => void): () => void {
  if (!enApp) return () => {};
  const quitar = import('@tauri-apps/api/event')
    .then(({ listen }) => listen<T>(evento, (e) => alLlegar(e.payload)))
    .catch(() => () => {});
  return () => {
    quitar.then((f) => f());
  };
}

/** La revisión encontró una versión nueva. Devuelve con qué dejar de escuchar. */
export const alHaberActualizacion = (alLlegar: (e: EstadoActualizacion) => void) =>
  escuchar('actualizacion-disponible', alLlegar);

/** Avanza la descarga o la instalación. */
export const alAvanzarActualizacion = (alLlegar: (a: AvanceActualizacion) => void) =>
  escuchar('actualizacion-progreso', alLlegar);

/** "3 nov 2026, 21:16". */
export const fechaCompilacion = (segundos: number): string =>
  segundos > 0
    ? new Date(segundos * 1000).toLocaleString('es', {
        day: 'numeric',
        month: 'short',
        year: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
      })
    : 'sin fecha';

// --- Datos de ejemplo (solo fuera de la app) ---

const pruebaActualizacion =
  !enApp && typeof location !== 'undefined' && new URLSearchParams(location.search).get('actualizacion') === '1';

const ejemplo: EstadoActualizacion = {
  disponible: pruebaActualizacion,
  compiladoActual: Math.floor(Date.now() / 1000) - 2 * 86400,
  compiladoNuevo: pruebaActualizacion ? Math.floor(Date.now() / 1000) - 3600 : null,
  desde: pruebaActualizacion ? 'demo-sige' : null,
  build: 'ejemplo',
  busca: pruebaActualizacion,
  revisado: null,
  problema: null,
  instalando: false,
};
