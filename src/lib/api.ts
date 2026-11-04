// Llamadas a la parte de Rust.
// Fuera de la app (por ejemplo `pnpm preview` en el navegador) usa datos de ejemplo para ver el diseño.
import { invoke } from '@tauri-apps/api/core';
import type {
  Atajo,
  Automatico,
  CambioSonido,
  DispositivoAudio,
  Estado,
  EstadoAjustes,
  Pantalla,
  Registro,
  Sonido,
} from './tipos';

const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const listarDispositivos = (): Promise<Estado> =>
  enApp ? invoke<Estado>('listar_dispositivos') : Promise.resolve(copiaEjemplo());

export const cambiarConexion = (id: string, conectar: boolean): Promise<void> =>
  enApp ? invoke<void>('cambiar_conexion', { id, conectar }) : conexionEjemplo(id, conectar);

/** Nombre propio dentro de Nexo (el del aparato no cambia). null o vacío vuelve al nombre real. */
export const ponerAlias = (id: string, alias: string | null): Promise<void> =>
  enApp ? invoke<void>('poner_alias', { id, alias }) : aliasEjemplo(id, alias);

/** Desempareja el aparato. Puede fallar con un mensaje en español; entonces conviene abrir Ajustes › Bluetooth. */
export const olvidarDispositivo = (id: string): Promise<void> =>
  enApp ? invoke<void>('olvidar_dispositivo', { id }) : olvidarEjemplo(id);

export const leerSonido = (id: string): Promise<Sonido> =>
  enApp ? invoke<Sonido>('sonido', { id }) : Promise.resolve({ ...sonidoEjemplo });

export const ajustarSonido = (id: string, cambio: CambioSonido): Promise<Sonido> =>
  enApp ? invoke<Sonido>('ajustar_sonido', { id, cambio }) : ajustarSonidoEjemplo(cambio);

/** Todos los aparatos de sonido del sistema: primero los que están en uso, luego los integrados y el resto por nombre. */
export const listarDispositivosAudio = (): Promise<DispositivoAudio[]> =>
  enApp ? invoke<DispositivoAudio[]>('dispositivos_audio') : Promise.resolve(copiaAudioEjemplo());

/** Usa el aparato como salida (los avisos del sistema lo siguen) o, con `entrada`, como micrófono. Devuelve la lista actualizada. */
export const usarDispositivoAudio = (uid: string, entrada: boolean): Promise<DispositivoAudio[]> =>
  enApp
    ? invoke<DispositivoAudio[]>('usar_dispositivo_audio', { uid, entrada })
    : cambiarAudioEjemplo(uid, entrada ? { accion: 'usarMicrofono' } : { accion: 'usarSalida' });

/** Volumen, silencio o volumen del micrófono de un aparato. Devuelve la lista actualizada. */
export const ajustarDispositivoAudio = (uid: string, cambio: CambioSonido): Promise<DispositivoAudio[]> =>
  enApp ? invoke<DispositivoAudio[]>('ajustar_dispositivo_audio', { uid, cambio }) : cambiarAudioEjemplo(uid, cambio);

/** "Probar sonido": un aviso corto por la salida en uso. */
export const probarSonido = (): Promise<void> =>
  enApp ? invoke<void>('probar_sonido') : new Promise((listo) => setTimeout(listo, 250));

/** Mac: el volumen de avisos y efectos (0–100). En Windows no hay uno aparte: null. */
export const leerVolumenAvisos = (): Promise<number | null> =>
  enApp ? invoke<number | null>('volumen_avisos') : Promise.resolve(enMacEjemplo ? avisosEjemplo : null);

/** Mac: cambia el volumen de avisos y efectos. */
export const ponerVolumenAvisos = (valor: number): Promise<void> =>
  enApp ? invoke<void>('poner_volumen_avisos', { valor }) : Promise.resolve(void (avisosEjemplo = valor));

/** Nombre de pila de la cuenta del sistema ("Ronny"); vacío si el sistema no da ninguno. */
export const leerNombreUsuario = (): Promise<string> =>
  enApp ? invoke<string>('nombre_usuario') : Promise.resolve('Ronny');

export const listarPantallas = (): Promise<Pantalla[]> =>
  enApp ? invoke<Pantalla[]>('listar_pantallas') : Promise.resolve(pantallasEjemplo);

export const ponerBrillo = (id: number, valor: number): Promise<number> =>
  enApp ? invoke<number>('poner_brillo', { id, valor }) : Promise.resolve(valor);

export const leerAjustes = (): Promise<EstadoAjustes> =>
  enApp ? invoke<EstadoAjustes>('leer_ajustes') : Promise.resolve(copiaAjustesEjemplo());

export const usarTeclasBrillo = (activar: boolean): Promise<EstadoAjustes> =>
  enApp ? invoke<EstadoAjustes>('usar_teclas_brillo', { activar }) : cambiarAjustesEjemplo({ teclasBrillo: activar });

/** Windows: cambia el atajo de bajar o de subir el brillo. Rechaza los que usa Windows. */
export const ponerAtajoBrillo = (subir: boolean, atajo: Atajo): Promise<EstadoAjustes> =>
  enApp
    ? invoke<EstadoAjustes>('poner_atajo_brillo', { subir, atajo })
    : cambiarAjustesEjemplo(subir ? { atajoSubir: atajo } : { atajoBajar: atajo });

/** Windows: enciende o apaga "usar las teclas de brillo del portátil". */
export const usarTeclasPortatil = (activar: boolean): Promise<EstadoAjustes> =>
  enApp ? invoke<EstadoAjustes>('usar_teclas_portatil', { activar }) : cambiarAjustesEjemplo({ teclasPortatil: activar });

// Ajustes de ejemplo (fuera de la app): los atajos de fábrica y un portátil con monitor externo.
const ajustesEjemplo: EstadoAjustes = {
  teclasBrillo: true,
  permiso: true,
  atajoBajar: { vk: 0x70, ctrl: true, alt: true, shift: false, win: false },
  atajoSubir: { vk: 0x71, ctrl: true, alt: true, shift: false, win: false },
  teclasPortatil: true,
  hayPortatil: true,
};
const copiaAjustesEjemplo = (): EstadoAjustes => ({
  ...ajustesEjemplo,
  atajoBajar: { ...ajustesEjemplo.atajoBajar },
  atajoSubir: { ...ajustesEjemplo.atajoSubir },
});
function cambiarAjustesEjemplo(cambio: Partial<EstadoAjustes>): Promise<EstadoAjustes> {
  Object.assign(ajustesEjemplo, cambio);
  return Promise.resolve(copiaAjustesEjemplo());
}

export const leerBrillos = (): Promise<[number, number][]> =>
  enApp ? invoke<[number, number][]>('brillos') : Promise.resolve(pantallasEjemplo.map((p) => [p.id, p.brillo ?? 0]));

export const nivelAviso = (): Promise<number> => (enApp ? invoke<number>('nivel_aviso') : Promise.resolve(62));

export const leerHistorial = (id: string): Promise<Registro> =>
  enApp ? invoke<Registro>('historial', { id }) : Promise.resolve(historialEjemplo(id));

/** El historial de varios aparatos a la vez. Los que fallan se omiten (la próxima lectura los trae). */
export async function leerHistoriales(ids: string[]): Promise<Record<string, Registro>> {
  const leidos = await Promise.allSettled(ids.map((id) => leerHistorial(id)));
  const salida: Record<string, Registro> = {};
  leidos.forEach((r, i) => {
    if (r.status === 'fulfilled') salida[ids[i]] = r.value;
  });
  return salida;
}

export const leerAutomatico = (id: string): Promise<Automatico> =>
  enApp ? invoke<Automatico>('automatico', { id }) : Promise.resolve({ usarSalidaAlConectar: true, avisarBateriaBaja: true });

export const guardarAutomatico = (id: string, ajustes: Automatico): Promise<Automatico> =>
  enApp ? invoke<Automatico>('guardar_automatico', { id, ajustes }) : Promise.resolve(ajustes);

export const abrirAjustesBluetooth = (): Promise<void> =>
  enApp ? invoke<void>('abrir_ajustes_bluetooth') : Promise.resolve();

/** Abre los ajustes de sonido del sistema. */
export const abrirAjustesSonido = (): Promise<void> =>
  enApp ? invoke<void>('abrir_ajustes_sonido') : Promise.resolve();

// --- Datos de ejemplo (solo fuera de la app) ---

/** Cada aparato de ejemplo con su propia historia, para que Inicio muestre una actividad creíble. */
function historialEjemplo(id: string): Registro {
  const t = Math.floor(Date.now() / 1000);
  const medianoche = new Date();
  medianoche.setHours(0, 0, 0, 0);
  const ayer = medianoche.getTime() / 1000 - 86400;
  const d = ejemplo.dispositivos.find((x) => x.id === id);
  if (id === 'E8:9E:13:61:CE:B7') {
    const bateria: [number, number][] = [];
    for (let i = 0; i <= 36; i++) bateria.push([t - (36 - i) * 300, Math.round(92 - i * 0.5)]);
    const sesiones: [number, number | null][] = [
      [t - 26 * 3600, t - 23.5 * 3600],
      [t - 5 * 3600, t - 4 * 3600],
      [t - 3 * 3600, d?.conectado ? null : t - 60],
    ];
    return { sesiones, bateria };
  }
  if (id === 'AC:BC:B5:02:84:E1') {
    // Ayer de 19:05 a 21:10.
    const sesiones: [number, number | null][] = [[ayer + 19 * 3600 + 300, ayer + 21 * 3600 + 600]];
    if (d?.conectado) sesiones.push([t - 120, null]);
    return { sesiones, bateria: [] };
  }
  return { sesiones: [[t - 4 * 86400, t - 4 * 86400 + 3600]], bateria: [] };
}

const enMacEjemplo = typeof navigator !== 'undefined' && /Mac/i.test(navigator.userAgent);
// `?audio=bt`: el micrófono de los audífonos en uso (para ver el aviso de calidad de Audio).
const microBt = !enApp && typeof location !== 'undefined' && new URLSearchParams(location.search).get('audio') === 'bt';
let avisosEjemplo = 60;

// Lo que hay hoy en la Mac del dueño (los volúmenes son de ejemplo).
const audioEjemplo: DispositivoAudio[] = [
  {
    uid: 'E8-9E-13-61-CE-B7:output', nombre: 'WH-CH520', salida: true, entrada: false,
    salidaEnUso: true, entradaEnUso: false, volumen: 72, silenciado: false, volumenEntrada: null, silenciadoEntrada: null, transporte: 'bluetooth',
  },
  {
    uid: 'BuiltInMicrophoneDevice', nombre: 'Micrófono de MacBook Pro', salida: false, entrada: true,
    salidaEnUso: false, entradaEnUso: !microBt, volumen: null, silenciado: null, volumenEntrada: 35, silenciadoEntrada: false, transporte: 'integrado',
  },
  {
    uid: 'BuiltInSpeakerDevice', nombre: 'Bocinas de MacBook Pro', salida: true, entrada: false,
    salidaEnUso: false, entradaEnUso: false, volumen: 81, silenciado: false, volumenEntrada: null, silenciadoEntrada: null, transporte: 'integrado',
  },
  {
    uid: 'CA4B84B0-E2A8-4393-9482-C60500000003', nombre: 'Micrófono de iPhone de Ronny', salida: false, entrada: true,
    salidaEnUso: false, entradaEnUso: false, volumen: null, silenciado: null, volumenEntrada: null, silenciadoEntrada: null, transporte: 'otro',
  },
  {
    uid: 'E8-9E-13-61-CE-B7:input', nombre: 'WH-CH520', salida: false, entrada: true,
    salidaEnUso: false, entradaEnUso: microBt, volumen: null, silenciado: null, volumenEntrada: 68, silenciadoEntrada: false, transporte: 'bluetooth',
  },
];

/** Mismo orden que Rust: en uso, integrados y luego por nombre. */
const rangoAudio = (d: DispositivoAudio) => (d.salidaEnUso || d.entradaEnUso ? 0 : 2) + (d.transporte === 'integrado' ? 0 : 1);
const copiaAudioEjemplo = () =>
  audioEjemplo.map((d) => ({ ...d })).sort((a, b) => rangoAudio(a) - rangoAudio(b) || a.nombre.localeCompare(b.nombre, 'es'));

function cambiarAudioEjemplo(uid: string, cambio: CambioSonido): Promise<DispositivoAudio[]> {
  const d = audioEjemplo.find((x) => x.uid === uid);
  if (!d) return Promise.reject('Ese aparato de sonido ya no está conectado');
  const sinSalida = !d.salida && ['usarSalida', 'volumen', 'silenciar'].includes(cambio.accion);
  const sinEntrada = !d.entrada && ['usarMicrofono', 'volumenMicrofono', 'silenciarMicrofono'].includes(cambio.accion);
  if (sinSalida) return Promise.reject('Este aparato no tiene salida de sonido activa');
  if (sinEntrada) return Promise.reject('Este aparato no tiene micrófono activo');
  if (cambio.accion === 'usarSalida') for (const x of audioEjemplo) x.salidaEnUso = x === d;
  if (cambio.accion === 'usarMicrofono') for (const x of audioEjemplo) x.entradaEnUso = x === d;
  if (cambio.accion === 'volumen') d.volumen = cambio.valor;
  if (cambio.accion === 'silenciar') d.silenciado = cambio.valor;
  if (cambio.accion === 'volumenMicrofono') d.volumenEntrada = cambio.valor;
  if (cambio.accion === 'silenciarMicrofono') d.silenciadoEntrada = cambio.valor;
  return Promise.resolve(copiaAudioEjemplo());
}

const pantallasEjemplo: Pantalla[] = [
  { id: 1, nombre: 'Pantalla de la MacBook', interna: true, brillo: 40, controlable: true },
  { id: 2, nombre: 'Steren 27" IPS', interna: false, brillo: 70, controlable: true },
];

// El sonido de los WH-CH520 de ejemplo (mismos volúmenes que en `audioEjemplo`).
const sonidoEjemplo: Sonido = {
  tieneSalida: true,
  salidaEnUso: true,
  volumen: 72,
  silenciado: false,
  tieneMicrofono: true,
  microfonoEnUso: false,
  volumenMicrofono: 68,
};

/** Aplica el cambio a los datos de ejemplo, para que silencio y volúmenes no vuelvan atrás en la siguiente lectura. */
function ajustarSonidoEjemplo(cambio: CambioSonido): Promise<Sonido> {
  const s = sonidoEjemplo;
  if (cambio.accion === 'usarSalida') s.salidaEnUso = true;
  if (cambio.accion === 'usarMicrofono') s.microfonoEnUso = true;
  if (cambio.accion === 'volumen') s.volumen = cambio.valor;
  if (cambio.accion === 'silenciar') s.silenciado = cambio.valor;
  if (cambio.accion === 'volumenMicrofono') s.volumenMicrofono = cambio.valor;
  return Promise.resolve({ ...s });
}

const ejemplo: Estado = {
  bluetoothEncendido: true,
  dispositivos: [
    {
      id: 'E8:9E:13:61:CE:B7', nombre: 'WH-CH520', alias: null, tipo: 'diadema', conectado: true,
      bateria: 100, bateriaIzquierda: null, bateriaDerecha: null, bateriaEstuche: null,
      fabricante: 'Sony', firmware: '16.0.0', vendorId: '0x054C', productId: '0x0EAD',
    },
    {
      id: 'AC:BC:B5:02:84:E1', nombre: 'Ronny Beats Solo³', alias: 'Beats del estudio', tipo: 'audifonos', conectado: false,
      bateria: null, bateriaIzquierda: null, bateriaDerecha: null, bateriaEstuche: null,
      fabricante: 'Beats', firmware: '7.8.8', vendorId: '0x004C', productId: '0x2006',
    },
    // Los Logitech de la oficina (en la PC con Windows); aquí solo para revisar el diseño.
    {
      id: 'D4:12:7A:5C:0B:41', nombre: 'MX Master 4', alias: null, tipo: 'mouse', conectado: true,
      bateria: 85, bateriaIzquierda: null, bateriaDerecha: null, bateriaEstuche: null,
      fabricante: 'Logitech', firmware: null, vendorId: '0x046D', productId: null,
    },
    {
      id: 'D4:12:7A:5C:0B:42', nombre: 'Wave Keys', alias: null, tipo: 'teclado', conectado: true,
      bateria: 70, bateriaIzquierda: null, bateriaDerecha: null, bateriaEstuche: null,
      fabricante: 'Logitech', firmware: null, vendorId: '0x046D', productId: null,
    },
    {
      id: 'EC:28:D3:02:FE:AB', nombre: 'iPhone de Ronny', alias: null, tipo: 'telefono', conectado: false,
      bateria: null, bateriaIzquierda: null, bateriaDerecha: null, bateriaEstuche: null,
      fabricante: 'Apple', firmware: null, vendorId: null, productId: null,
    },
  ],
};

/** Copia nueva en cada lectura, como llega de Rust. */
const copiaEjemplo = (): Estado => ({
  bluetoothEncendido: ejemplo.bluetoothEncendido,
  dispositivos: ejemplo.dispositivos.map((d) => ({ ...d })),
});

const dispositivoEjemplo = (id: string) => ejemplo.dispositivos.find((d) => d.id === id);

function conexionEjemplo(id: string, conectar: boolean): Promise<void> {
  const d = dispositivoEjemplo(id);
  if (!d) return Promise.reject('Ese dispositivo ya no está emparejado');
  return new Promise((listo) =>
    setTimeout(() => {
      d.conectado = conectar;
      if (d.tipo === 'diadema' || d.tipo === 'audifonos') d.bateria = conectar ? (d.bateria ?? 80) : null;
      listo();
    }, 900),
  );
}

function aliasEjemplo(id: string, alias: string | null): Promise<void> {
  const d = dispositivoEjemplo(id);
  if (!d) return Promise.reject('Ese dispositivo ya no está emparejado');
  d.alias = alias?.trim() || null;
  return Promise.resolve();
}

function olvidarEjemplo(id: string): Promise<void> {
  const i = ejemplo.dispositivos.findIndex((d) => d.id === id);
  if (i < 0) return Promise.reject('Ese dispositivo ya no está emparejado');
  ejemplo.dispositivos.splice(i, 1);
  return Promise.resolve();
}
