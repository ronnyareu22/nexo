// Llamadas a Rust para tus equipos y el perfil de trabajo remoto (sistema/equipos.rs).
// Fuera de la app usa datos de ejemplo; `?perfil=` elige el caso (con `?plataforma=windows` o `mac`):
//   (sin nada)  tu otro equipo encontrado y todo listo
//   nada        sin Tailscale ni RustDesk
//   sesion      Tailscale instalado, falta entrar con la cuenta
//   mitad       Tailscale conectado y RustDesk listo; falta tu otro equipo
//   sinnexo     tu otro equipo en Tailscale, todavía sin Nexo
//   pendiente   todo listo menos RustDesk en este equipo (en la Mac, el acceso directo por IP)
//   apagada     tu otro equipo guardado, apagado
//   nunca       el trabajo remoto apagado en Ajustes
import { invoke } from '@tauri-apps/api/core';
import { escuchar, esMac } from './api-sistema';
import type { Descarga, EquipoTailscale, EstadoEquipos, ModoRemoto } from './tipos-sistema';

const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** Para revisar el diseño fuera de la app (ver arriba). */
export const pruebaPerfil: string | null =
  !enApp && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('perfil') : null;

/** Cómo están tus equipos. Con `fresco` (la guía a la vista), Rust vuelve a preguntar si pasaron más de 8 s. */
export const leerEquipos = (fresco = false): Promise<EstadoEquipos> =>
  enApp ? invoke<EstadoEquipos>('equipos_estado', { fresco }) : esperar(250).then(copia);

/** Automático, Siempre o Nunca. Devuelve cómo quedó todo. */
export const ponerModoRemoto = (modo: ModoRemoto): Promise<EstadoEquipos> =>
  enApp ? invoke<EstadoEquipos>('equipos_modo', { modo }) : Promise.resolve(cambiarEjemplo({ modo }));

/** Olvida el otro equipo guardado (si sigue en Tailscale, Nexo lo vuelve a encontrar). */
export const olvidarOtroEquipo = (): Promise<EstadoEquipos> =>
  enApp ? invoke<EstadoEquipos>('equipos_olvidar') : Promise.resolve(cambiarEjemplo({ otro: null }));

/** Abre la app de Tailscale (ahí se entra con la cuenta) y, si ya hay sesión, lo conecta. */
export const abrirTailscale = (): Promise<EstadoEquipos> =>
  enApp ? invoke<EstadoEquipos>('equipos_abrir_tailscale') : esperar(600).then(copia);

/** Abre en el navegador la descarga de Tailscale, Nexo o RustDesk. */
export const abrirDescarga = (cual: Descarga): Promise<void> =>
  enApp ? invoke<void>('equipos_abrir_enlace', { cual }) : Promise.resolve();

/** Rust encontró otro equipo o cambió el modo. */
export const alCambiarEquipos = (alLlegar: () => void) => escuchar<unknown>('equipos-cambio', () => alLlegar());

// --- Datos de ejemplo (solo fuera de la app) ---

const esperar = (ms: number) => new Promise((listo) => setTimeout(listo, ms));

const MAC = { nombre: 'demo-sige', ip: '100.88.188.99', plataforma: 'mac' as const };
const PC = { nombre: 'innotech1017', ip: '100.118.228.87', plataforma: 'windows' as const };

function ejemplo(): EstadoEquipos {
  const p = pruebaPerfil;
  const yo = esMac ? MAC : PC;
  const el = esMac ? PC : MAC;
  const tailscale = p === 'nada' ? 'no instalado' : p === 'sesion' ? 'sin sesion' : 'activo';
  const conOtro = !['nada', 'sesion', 'mitad'].includes(p ?? '');
  const nexo = conOtro && p !== 'sinnexo' && p !== 'apagada';
  const telefono: EquipoTailscale = { nombre: 'iphone-de-casa', ip: '100.101.7.30', plataforma: 'otro', enLinea: true, nexo: false };
  const equipos: EquipoTailscale[] =
    tailscale !== 'activo' ? [] : conOtro ? [{ ...el, enLinea: p !== 'apagada', nexo }, telefono] : [telefono];
  const rustdeskListo = p !== 'nada' && p !== 'pendiente';
  return {
    tailscale,
    propio: { nombre: yo.nombre, ip: tailscale === 'activo' ? yo.ip : null, plataforma: yo.plataforma },
    equipos,
    otro: conOtro
      ? { ...el, enLinea: p !== 'apagada', nexo, encontrado: p !== 'apagada', conNexo: p !== 'sinnexo' }
      : null,
    modo: p === 'nunca' ? 'nunca' : 'auto',
    activo: conOtro && p !== 'nunca',
    rustdesk: {
      instalado: esMac ? p !== 'nada' : rustdeskListo,
      accesoDirecto: esMac ? rustdeskListo : null,
    },
  };
}

const estadoEjemplo = ejemplo();

const copia = (): EstadoEquipos => ({
  ...estadoEjemplo,
  propio: { ...estadoEjemplo.propio },
  equipos: estadoEjemplo.equipos.map((e) => ({ ...e })),
  otro: estadoEjemplo.otro ? { ...estadoEjemplo.otro } : null,
  rustdesk: { ...estadoEjemplo.rustdesk },
});

function cambiarEjemplo(cambio: Partial<EstadoEquipos>): EstadoEquipos {
  Object.assign(estadoEjemplo, cambio);
  const m = estadoEjemplo.modo;
  estadoEjemplo.activo = m === 'siempre' || (m === 'auto' && !!estadoEjemplo.otro);
  return copia();
}
