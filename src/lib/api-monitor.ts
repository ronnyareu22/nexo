// Llamadas a Rust para los ajustes del monitor (DDC/CI: brillo mínimo, contraste, color, entrada…)
// y los widgets de brillo de una sola pantalla.
// Fuera de la app (por ejemplo `pnpm preview` en el navegador) usa datos de ejemplo: el Steren 27" del dueño.
import { invoke } from '@tauri-apps/api/core';
import { escuchar } from './api-sistema';
import type { AjustesMonitor, CambioBrillo, TeclasPantalla, ValorVcp } from './tipos-sistema';

const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** Lee los ajustes del monitor por DDC/CI. Tarda un poco la primera vez (el monitor manda su lista). */
export const ajustesMonitor = (id: number): Promise<AjustesMonitor> =>
  enApp ? invoke<AjustesMonitor>('ajustes_monitor', { id }) : ajustesEjemplo(id);

/** Cambia un ajuste del monitor por su código VCP. Devuelve cómo quedó. */
export const ponerVcp = (id: number, codigo: number, valor: number): Promise<ValorVcp> =>
  enApp ? invoke<ValorVcp>('poner_vcp', { id, codigo, valor }) : ponerVcpEjemplo(id, codigo, valor);

/** Cambia el brillo mínimo (0–40 %) de un monitor externo. Devuelve el brillo en la escala de Nexo. */
export const ponerMinimoBrillo = (id: number, valor: number): Promise<number> =>
  enApp ? invoke<number>('poner_minimo_brillo', { id, valor }) : minimoEjemplo(id, valor);

/** Cómo responde una pantalla a las teclas de brillo (si responde y de cuánto es el paso). */
export const teclasPantalla = (id: number): Promise<TeclasPantalla> =>
  enApp ? invoke<TeclasPantalla>('teclas_pantalla', { id }) : Promise.resolve({ ...teclasEjemplo(id) });

/** Cambia si una pantalla responde a las teclas de brillo y su paso (3, 6 o 10 %). Devuelve cómo quedó. */
export const ponerTeclasPantalla = (id: number, t: TeclasPantalla): Promise<TeclasPantalla> =>
  enApp ? invoke<TeclasPantalla>('poner_teclas_pantalla', { id, ...t }) : ponerTeclasEjemplo(id, t);

/** Las pantallas que tienen su widget de brillo encendido. */
export const leerWidgetsBrillo = (): Promise<number[]> =>
  enApp ? invoke<number[]>('widgets_brillo') : Promise.resolve([...widgetsEjemplo]);

/** Enciende o apaga el widget de brillo de una pantalla. Devuelve cómo quedó. */
export const usarWidgetBrillo = (id: number, activar: boolean): Promise<boolean> =>
  enApp ? invoke<boolean>('usar_widget_brillo', { id, activar }) : widgetEjemplo(id, activar);

/** Cambió el brillo de una pantalla (desde cualquier lado: panel, teclas, menú, escenas, widgets). */
export const alCambiarBrillo = (alLlegar: (c: CambioBrillo) => void) => escuchar('brillo-cambio', alLlegar);

/** Se encendió o apagó un widget de brillo (desde el menú del ícono o el panel). */
export const alCambiarWidgetsBrillo = (alLlegar: (ids: number[]) => void) => escuchar('widgets-brillo', alLlegar);

/** Fuera de la app, `?monitor=abierto` abre "Ajustes del monitor" para revisar el diseño. */
export const pruebaMonitorAbierto =
  !enApp && typeof location !== 'undefined' && new URLSearchParams(location.search).get('monitor') === 'abierto';

// --- Datos de ejemplo (solo fuera de la app) ---

const widgetsEjemplo = new Set<number>();

function widgetEjemplo(id: number, activar: boolean): Promise<boolean> {
  if (activar) widgetsEjemplo.add(id);
  else widgetsEjemplo.delete(id);
  return Promise.resolve(activar);
}

const esperar = (ms: number) => new Promise((listo) => setTimeout(listo, ms));

// El Steren 27" IPS (MNT-027) de la oficina: lo que suele dar un monitor de esa clase. Solo para el diseño.
const minimosEjemplo: Record<number, number> = { 2: 10 };
const valoresEjemplo: Record<number, ValorVcp> = {
  0x12: { actual: 50, maximo: 100 },
  0x14: { actual: 0x05, maximo: 0x0b },
  0x16: { actual: 50, maximo: 100 },
  0x18: { actual: 50, maximo: 100 },
  0x1a: { actual: 50, maximo: 100 },
  0x60: { actual: 0x11, maximo: 0x12 },
  0x62: { actual: 40, maximo: 100 },
  0x87: { actual: 5, maximo: 10 },
  0x8d: { actual: 2, maximo: 2 },
  0xdc: { actual: 0x00, maximo: 0x05 },
};
const capacidadesEjemplo = [
  { codigo: 0x04, valores: [] },
  { codigo: 0x12, valores: [] },
  { codigo: 0x14, valores: [0x01, 0x04, 0x05, 0x06, 0x08, 0x0b] },
  { codigo: 0x16, valores: [] },
  { codigo: 0x18, valores: [] },
  { codigo: 0x1a, valores: [] },
  { codigo: 0x60, valores: [0x0f, 0x11, 0x12] },
  { codigo: 0x62, valores: [] },
  { codigo: 0x87, valores: [] },
  { codigo: 0x8d, valores: [0x01, 0x02] },
  { codigo: 0xdc, valores: [0x00, 0x03, 0x05] },
];

async function ajustesEjemplo(id: number): Promise<AjustesMonitor> {
  await esperar(500);
  if (id !== 2) throw 'La pantalla integrada no tiene estos ajustes.';
  return {
    minimo: minimosEjemplo[id] ?? 10,
    capacidades: capacidadesEjemplo.map((c) => ({ ...c, valores: [...c.valores] })),
    valores: Object.fromEntries(Object.entries(valoresEjemplo).map(([k, v]) => [k, { ...v }])),
    conLista: true,
  };
}

async function ponerVcpEjemplo(id: number, codigo: number, valor: number): Promise<ValorVcp> {
  await esperar(60);
  if (id !== 2) throw 'Este monitor no responde. Activa DDC/CI en su menú.';
  const v = valoresEjemplo[codigo];
  if (codigo === 0x04) {
    for (const [k, x] of Object.entries({ 0x12: 50, 0x14: 0x05, 0x16: 50, 0x18: 50, 0x1a: 50, 0x87: 5 }))
      valoresEjemplo[Number(k)].actual = x;
    return { actual: 1, maximo: 0 };
  }
  if (!v) throw 'Este monitor no tiene ese ajuste.';
  v.actual = Math.min(valor, v.maximo);
  return { ...v };
}

const teclasGuardadas: Record<number, TeclasPantalla> = {};
const teclasEjemplo = (id: number): TeclasPantalla => teclasGuardadas[id] ?? { responde: true, paso: 6 };
function ponerTeclasEjemplo(id: number, t: TeclasPantalla): Promise<TeclasPantalla> {
  teclasGuardadas[id] = { responde: t.responde, paso: [3, 6, 10].includes(t.paso) ? t.paso : 6 };
  return Promise.resolve({ ...teclasGuardadas[id] });
}

function minimoEjemplo(id: number, valor: number): Promise<number> {
  minimosEjemplo[id] = Math.max(0, Math.min(40, Math.round(valor)));
  return Promise.resolve(70);
}
