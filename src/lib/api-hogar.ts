// Llamadas a Rust para la casa (sistema/hogar.rs): estado, órdenes, búsqueda, cuentas, habitaciones y grupos.
// Fuera de la app usa una casa de ejemplo (la misma de hogar/ejemplo.rs); `?casa=` elige el caso:
//   (sin nada)  Tuya y Home Assistant conectados: tira RGB, barra del monitor, dos focos, enchufe, tele, puerta,
//               cerradura, cortina y dos luces más
//   nada        sin cuentas ni aparatos (el estado vacío)
//   tuya        solo la cuenta de Tuya
//   error       Tuya conectada, pero venció su plan de prueba
//   buscar      abre Casa buscando (la búsqueda de ejemplo tarda 1,5 s)
//   local       solo aparatos de tu red, sin cuentas: dos luces Hue, Key Light de Elgato y tira WLED (Escritorio),
//               enchufes Shelly y Tasmota, Nanoleaf y un foco LIFX sin respuesta
//   red         lo mismo, pero sin las luces Hue (su puente queda por vincular), y abre Casa con «Tu red»
//               (17 equipos de ejemplo en los tres grupos; el análisis tarda 1,8 s)
// Vincular de ejemplo: con botón contesta «todavía no» dos veces y a la tercera agrega; con contraseña, una de
// menos de 4 letras no es correcta.
import { invoke } from '@tauri-apps/api/core';
import type {
  Accion,
  Aparato,
  CambioAparato,
  Capacidades,
  EquipoRed,
  EstadoAparato,
  EstadoCasa,
  Grupo,
  Habitacion,
  Hallazgo,
  Orden,
  Proveedor,
  ResultadoBusqueda,
  ResultadoOrden,
  ResultadoRed,
  ResultadoVinculo,
} from './tipos-hogar';

const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** Para revisar el diseño fuera de la app (ver arriba). */
export const pruebaCasa: string | null =
  !enApp && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('casa') : null;

/** Cómo está la casa. Con `fresco`, Rust vuelve a preguntar si pasaron más de 15 s. */
export const leerCasa = (fresco = false): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_estado', { fresco }) : esperar(200).then(copia);

/** Una orden: a quién y qué. Abrir una cerradura vuelve con `requiereConfirmacion` hasta que llega `confirmado`. */
export const ejecutarEnCasa = (orden: Orden): Promise<ResultadoOrden> =>
  enApp ? invoke<ResultadoOrden>('hogar_ejecutar', { orden }) : esperar(250).then(() => ejecutarEjemplo(orden));

/** Busca en la red y por Bluetooth (≈6 s). */
export const buscarAparatos = (): Promise<ResultadoBusqueda> =>
  enApp ? invoke<ResultadoBusqueda>('hogar_buscar') : esperar(1500).then(buscarEjemplo);

/** Tu red: todo lo que hay en la misma red (≈4 s, a pedido; queda solo en memoria). */
export const analizarRed = (): Promise<ResultadoRed> =>
  enApp ? invoke<ResultadoRed>('hogar_red') : esperar(1800).then(redEjemplo);

/** Un intento de vincular algo encontrado: con el botón (Hue, Nanoleaf) o con su contraseña (Shelly, Tasmota).
 *  Con botón, quien llama repite cada ~2 s mientras `listo` sea false (unos 30 s). */
export const vincularAparato = (id: string, clave?: string): Promise<ResultadoVinculo> =>
  enApp
    ? invoke<ResultadoVinculo>('hogar_vincular', { id, clave: clave ?? null })
    : esperar(700).then(() => vincularEjemplo(id, clave));

export const agregarAparato = (id: string): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_agregar', { id }) : Promise.resolve(agregarEjemplo(id));

export const quitarAparato = (id: string): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_quitar', { id }) : Promise.resolve(cambiarEjemplo(() => (casa.aparatos = casa.aparatos.filter((a) => a.id !== id))));

/** Nombre (vacío vuelve al suyo), habitación (vacío: sin cuarto) o forma. */
export const editarAparato = (id: string, cambio: CambioAparato): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_editar', { id, cambio }) : Promise.resolve(editarEjemplo(id, cambio));

export const guardarGrupo = (grupo: Grupo): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_guardar_grupo', { grupo }) : Promise.resolve(grupoEjemplo(grupo));

export const borrarGrupo = (id: string): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_borrar_grupo', { id }) : Promise.resolve(cambiarEjemplo(() => (casa.grupos = casa.grupos.filter((g) => g.id !== id))));

export const guardarHabitacion = (habitacion: Habitacion): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_guardar_habitacion', { habitacion }) : Promise.resolve(habitacionEjemplo(habitacion));

export const borrarHabitacion = (id: string): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_borrar_habitacion', { id }) : Promise.resolve(borrarHabitacionEjemplo(id));

export const mostrarTodo = (activo: boolean): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_mostrar_todo', { activo }) : Promise.resolve(cambiarEjemplo(() => (casa.mostrarTodo = activo)));

/** Conecta Tuya. `region`: un centro de datos o "auto" (Detectar). El secreto va al llavero y no vuelve. */
export const conectarTuya = (accessId: string, accessSecret: string, region: string): Promise<EstadoCasa> =>
  enApp
    ? invoke<EstadoCasa>('hogar_tuya_conectar', { datos: { accessId, accessSecret, region } })
    : esperar(1200).then(() => conectarTuyaEjemplo(accessId, accessSecret, region));

export const desconectarTuya = (): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_tuya_desconectar') : Promise.resolve(desconectarTuyaEjemplo());

/** Conecta Home Assistant con su dirección y un token de larga duración (al llavero; no vuelve). */
export const conectarHa = (url: string, token: string): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_ha_conectar', { datos: { url, token } }) : esperar(900).then(() => conectarHaEjemplo(url, token));

export const desconectarHa = (): Promise<EstadoCasa> =>
  enApp ? invoke<EstadoCasa>('hogar_ha_desconectar') : Promise.resolve(desconectarHaEjemplo());

/** Algo cambió desde el menú del ícono o una escena. Devuelve con qué dejar de escuchar. */
export function alCambiarCasa(alLlegar: () => void): () => void {
  if (!enApp) return () => {};
  const quitar = import('@tauri-apps/api/event')
    .then(({ listen }) => listen('casa-cambio', () => alLlegar()))
    .catch(() => () => {});
  return () => {
    quitar.then((f) => f());
  };
}

// --- Datos de ejemplo (solo fuera de la app) ---

const esperar = (ms: number) => new Promise((listo) => setTimeout(listo, ms));

const caps = (c: Partial<Capacidades>): Capacidades => ({
  encender: false,
  brillo: false,
  temperatura: false,
  color: false,
  volumen: false,
  silencio: false,
  entradas: false,
  abrir: false,
  posicion: false,
  soloLectura: false,
  requiereConfirmacion: false,
  informaEstado: true,
  ...c,
});
const luzCaps = (temperatura: boolean, color: boolean) => caps({ encender: true, brillo: true, temperatura, color });

const est = (e: Partial<EstadoAparato>): EstadoAparato => ({
  encendido: null,
  brillo: null,
  temperatura: null,
  color: null,
  modo: null,
  abierto: null,
  posicion: null,
  volumen: null,
  silenciado: null,
  bateria: null,
  grados: null,
  ...e,
});

function ap(a: Partial<Aparato> & Pick<Aparato, 'id' | 'nombre' | 'clase'>): Aparato {
  return {
    proveedor: 'tuya',
    via: 'nube',
    nombreOriginal: a.nombre,
    alias: null,
    forma: null,
    habitacion: null,
    marca: 'Tuya / Smart Life',
    modelo: null,
    direccion: null,
    enLinea: true,
    controlable: true,
    capacidades: caps({ encender: true }),
    estado: est({ encendido: false }),
    oculto: false,
    entradas: [],
    entrada: null,
    nota: null,
    ...a,
  };
}

const REGIONES: [string, string][] = [
  ['us', 'América (oeste)'],
  ['us-e', 'América (este)'],
  ['eu', 'Europa central'],
  ['eu-w', 'Europa occidental'],
  ['in', 'India'],
  ['sg', 'Singapur'],
  ['cn', 'China'],
];

const HABITACIONES: Habitacion[] = [
  { id: 'sala', nombre: 'Sala' },
  { id: 'cuarto', nombre: 'Cuarto' },
  { id: 'oficina', nombre: 'Oficina' },
  { id: 'cocina', nombre: 'Cocina' },
  { id: 'escritorio', nombre: 'Escritorio' },
];

function aparatosTuya(): Aparato[] {
  return [
    ap({
      id: 'tuya:ejtira01', nombre: 'Tira del escritorio', clase: 'luz', forma: 'tira', habitacion: 'escritorio',
      modelo: 'RGB Strip Light', capacidades: luzCaps(true, true),
      estado: est({ encendido: true, brillo: 70, temperatura: 50, color: { h: 190, s: 80, v: 70 }, modo: 'color' }),
    }),
    ap({
      id: 'tuya:ejbarra01', nombre: 'Barra del monitor', clase: 'luz', forma: 'barra', habitacion: 'escritorio',
      modelo: 'Screen Light Bar', capacidades: luzCaps(true, false),
      estado: est({ encendido: true, brillo: 62, temperatura: 38, modo: 'blanco' }),
    }),
    ap({
      id: 'tuya:ejfoco01', nombre: 'Foco de la sala', clase: 'luz', forma: 'foco', habitacion: 'sala',
      modelo: 'Smart Bulb A60', capacidades: luzCaps(true, false),
      estado: est({ encendido: true, brillo: 80, temperatura: 15, modo: 'blanco' }),
    }),
    ap({
      id: 'tuya:ejfoco02', nombre: 'Foco del cuarto', clase: 'luz', forma: 'foco', habitacion: 'cuarto',
      modelo: 'Smart Bulb A60', capacidades: luzCaps(true, false),
      estado: est({ encendido: false, brillo: 40, temperatura: 0, modo: 'blanco' }),
    }),
    ap({
      id: 'tuya:ejenchufe01', nombre: 'Enchufe del ventilador', clase: 'enchufe', habitacion: 'cuarto',
      modelo: 'Smart Plug', estado: est({ encendido: false }),
    }),
  ];
}

function aparatosHa(): Aparato[] {
  const ha = { proveedor: 'homeAssistant' as const, via: 'wifi' as const, marca: 'Home Assistant' };
  return [
    ap({
      ...ha, id: 'ha:light.lampara_sala', nombre: 'Lámpara de pie', clase: 'luz', forma: 'lampara', habitacion: 'sala',
      capacidades: luzCaps(true, false), estado: est({ encendido: true, brillo: 71, temperatura: 19, modo: 'blanco' }),
    }),
    ap({
      ...ha, id: 'ha:light.foco_cocina', nombre: 'Foco de la cocina', clase: 'luz', forma: 'foco', habitacion: 'cocina',
      capacidades: caps({ encender: true, brillo: true }), estado: est({ encendido: false, brillo: 78, modo: 'blanco' }),
    }),
    ap({
      ...ha, id: 'ha:media_player.tele_sala', nombre: 'Tele de la sala', clase: 'tv', habitacion: 'sala',
      capacidades: caps({ encender: true, volumen: true, silencio: true, entradas: true }),
      estado: est({ encendido: true, volumen: 30, silenciado: false }),
      entradas: ['HDMI 1', 'HDMI 2', 'Netflix', 'YouTube'], entrada: 'HDMI 1',
    }),
    ap({
      ...ha, id: 'ha:cover.cortina_sala', nombre: 'Cortina de la sala', clase: 'cortina', habitacion: 'sala',
      capacidades: caps({ abrir: true, posicion: true }), estado: est({ abierto: true, posicion: 70 }),
    }),
    ap({
      ...ha, id: 'ha:lock.cerradura_entrada', nombre: 'Cerradura de la entrada', clase: 'cerradura', habitacion: 'area-entrada',
      capacidades: caps({ abrir: true, requiereConfirmacion: true }), estado: est({ abierto: false }),
    }),
    ap({
      ...ha, id: 'ha:binary_sensor.puerta_entrada', nombre: 'Puerta de entrada', clase: 'sensorPuerta', habitacion: 'area-entrada',
      controlable: false, capacidades: caps({ soloLectura: true }), estado: est({ abierto: false, bateria: 90 }),
    }),
  ];
}

// Los aparatos de tu red (`?casa=local` y `?casa=red`): ids y direcciones de ejemplo.
const HUE = 'hue:ecb5fa1c204e';
const ELGATO = 'elgato:3c6a9d14b2c8';
const WLED = 'wled:a842e3c91f20';

function aparatosLocales(conHue: boolean): Aparato[] {
  const red = (proveedor: Proveedor, marca: string, direccion: string) => ({ proveedor, marca, direccion, via: 'wifi' as const });
  const hue = red('hue', 'Philips Hue', '192.168.1.20');
  return [
    ...(conHue
      ? [
          ap({
            ...hue, id: `${HUE}-1`, nombre: 'Foco de la sala', clase: 'luz', forma: 'foco', habitacion: 'sala',
            modelo: 'Hue White and Color A19', capacidades: luzCaps(true, true),
            estado: est({ encendido: true, brillo: 80, temperatura: 30, modo: 'blanco' }),
          }),
          ap({
            ...hue, id: `${HUE}-2`, nombre: 'Lámpara de pie', clase: 'luz', forma: 'lampara', habitacion: 'sala',
            modelo: 'Hue White Ambiance', capacidades: luzCaps(true, false),
            estado: est({ encendido: false, brillo: 60, temperatura: 20, modo: 'blanco' }),
          }),
        ]
      : []),
    ap({
      ...red('elgato', 'Elgato', '192.168.1.31'), id: ELGATO, nombre: 'Key Light', clase: 'luz', forma: 'barra',
      habitacion: 'escritorio', modelo: 'Elgato Key Light', capacidades: luzCaps(true, false),
      estado: est({ encendido: true, brillo: 65, temperatura: 55, modo: 'blanco' }),
    }),
    ap({
      ...red('wled', 'WLED', '192.168.1.32'), id: WLED, nombre: 'Tira del escritorio', clase: 'luz', forma: 'tira',
      habitacion: 'escritorio', modelo: 'WLED 0.15', capacidades: luzCaps(false, true),
      estado: est({ encendido: true, brillo: 70, color: { h: 190, s: 80, v: 70 }, modo: 'color' }),
    }),
    ap({
      ...red('shelly', 'Shelly', '192.168.1.40'), id: 'shelly:e868e7a1b3c4', nombre: 'Enchufe del ventilador',
      clase: 'enchufe', habitacion: 'cuarto', modelo: 'Shelly Plus Plug S', estado: est({ encendido: false }),
    }),
    ap({
      ...red('tasmota', 'Tasmota', '192.168.1.42'), id: 'tasmota:dc4f22a5e610', nombre: 'Enchufe de la cafetera',
      clase: 'enchufe', habitacion: 'cocina', modelo: 'Sonoff S31', estado: est({ encendido: true }),
    }),
    ap({
      ...red('nanoleaf', 'Nanoleaf', '192.168.1.44'), id: 'nanoleaf:00552d8e4f1a', nombre: 'Paneles de la oficina',
      clase: 'luz', forma: 'lampara', habitacion: 'oficina', modelo: 'Nanoleaf Shapes', capacidades: luzCaps(true, true),
      estado: est({ encendido: true, brillo: 45, color: { h: 272, s: 70, v: 45 }, modo: 'color' }),
    }),
    ap({
      ...red('lifx', 'LIFX', '192.168.1.46'), id: 'lifx:d073d5a21b9c', nombre: 'Foco del cuarto', clase: 'luz',
      forma: 'foco', habitacion: 'cuarto', modelo: 'LIFX A19', enLinea: false, capacidades: luzCaps(true, true),
      estado: est({ encendido: false, brillo: 50, temperatura: 30, modo: 'blanco' }),
    }),
  ];
}

function nueva(): EstadoCasa {
  const p = pruebaCasa;
  if (p === 'local' || p === 'red')
    return {
      aparatos: aparatosLocales(p === 'local'),
      habitaciones: [...HABITACIONES],
      grupos: [{ id: 'g1', nombre: 'Escritorio', aparatos: [ELGATO, WLED], habitacion: 'escritorio' }],
      tuya: null,
      homeAssistant: null,
      errorTuya: null,
      errorHa: null,
      actualizado: Math.round(Date.now() / 1000),
      mostrarTodo: false,
      ocultos: 0,
      ejemplo: true,
      regiones: REGIONES,
    };
  const sinNada = p === 'nada';
  const conHa = !sinNada && p !== 'tuya' && p !== 'error';
  return {
    aparatos: sinNada ? [] : [...aparatosTuya(), ...(conHa ? aparatosHa() : [])],
    habitaciones: conHa ? [...HABITACIONES, { id: 'area-entrada', nombre: 'Entrada' }] : [...HABITACIONES],
    grupos: sinNada ? [] : [{ id: 'g1', nombre: 'Escritorio', aparatos: ['tuya:ejtira01', 'tuya:ejbarra01'], habitacion: 'escritorio' }],
    tuya: sinNada ? null : { accessId: 'qz8k4w2m7rtyx9pj3hvd', region: 'us', regionNombre: 'América (oeste)' },
    homeAssistant: conHa ? { url: 'http://homeassistant.local:8123' } : null,
    errorTuya:
      p === 'error'
        ? 'Venció el plan de prueba de tu proyecto de Tuya. Renuévalo gratis en iot.tuya.com (Cloud › tu proyecto › Service API).'
        : null,
    errorHa: null,
    actualizado: Math.round(Date.now() / 1000),
    mostrarTodo: false,
    ocultos: conHa ? 1 : 0,
    ejemplo: true,
    regiones: REGIONES,
  };
}

let casa: EstadoCasa = nueva();
let hallados: Hallazgo[] = [];

const copia = (): EstadoCasa => JSON.parse(JSON.stringify(casa)) as EstadoCasa;

function cambiarEjemplo(f: () => void): EstadoCasa {
  f();
  return copia();
}

const limitar = (n: number, min: number, max: number) => Math.max(min, Math.min(max, Math.round(n)));

function destinos(o: Orden): Aparato[] {
  const d = o.destino;
  const vivos = casa.aparatos.filter((a) => a.controlable && !a.oculto);
  switch (d.tipo) {
    case 'todos':
      return vivos.filter((a) => !d.clase || a.clase === d.clase);
    case 'aparato':
      return casa.aparatos.filter((a) => a.id === d.id);
    case 'grupo':
      return casa.aparatos.filter((a) => casa.grupos.find((g) => g.id === d.id)?.aparatos.includes(a.id));
    case 'habitacion':
      return vivos.filter((a) => a.habitacion === d.id && (!d.clase || a.clase === d.clase));
    case 'nombre':
      return vivos.filter((a) => a.nombre.toLowerCase().includes(d.texto.toLowerCase()));
  }
}

function aplicar(a: Aparato, accion: Accion): string | null {
  const e = a.estado;
  switch (accion.tipo) {
    case 'encender':
      e.encendido = true;
      break;
    case 'apagar':
      e.encendido = false;
      break;
    case 'alternar':
      e.encendido = !e.encendido;
      break;
    case 'brillo':
      e.encendido = true;
      e.brillo = limitar(accion.valor, 1, 100);
      if (e.modo === 'color' && e.color) e.color.v = e.brillo;
      break;
    case 'subir':
    case 'bajar': {
      const paso = (accion.paso ?? 20) * (accion.tipo === 'subir' ? 1 : -1);
      if (a.clase === 'tv') e.volumen = limitar((e.volumen ?? 0) + paso / 4, 0, 100);
      else e.brillo = limitar((e.brillo ?? 50) + paso, 1, 100);
      break;
    }
    case 'temperatura':
      e.encendido = true;
      e.temperatura = limitar(accion.valor, 0, 100);
      e.modo = 'blanco';
      break;
    case 'color':
      e.encendido = true;
      e.color = { h: accion.h, s: accion.s, v: accion.v };
      e.brillo = accion.v;
      e.modo = 'color';
      break;
    case 'abrir':
      e.abierto = true;
      if (a.clase === 'cortina') e.posicion = 100;
      break;
    case 'cerrar':
      e.abierto = false;
      if (a.clase === 'cortina') e.posicion = 0;
      break;
    case 'posicion':
      e.posicion = limitar(accion.valor, 0, 100);
      e.abierto = e.posicion > 0;
      break;
    case 'volumen':
      e.volumen = limitar(accion.valor, 0, 100);
      break;
    case 'silenciar':
      e.silenciado = accion.activo;
      break;
    case 'entrada':
      a.entrada = accion.nombre;
      break;
    case 'consultar':
      return null;
  }
  return null;
}

function ejecutarEjemplo(o: Orden): ResultadoOrden {
  const lista = destinos(o);
  const base = { hechos: [] as string[], fallas: [], requiereConfirmacion: false };
  if (!lista.length) return { ...base, ok: false, mensaje: 'No encontré ese aparato en tu casa.', estado: copia() };
  if (o.accion.tipo === 'abrir' && lista.some((a) => a.capacidades.requiereConfirmacion) && !o.confirmado)
    return {
      ...base,
      ok: false,
      requiereConfirmacion: true,
      mensaje: `¿Abrir ${lista.map((a) => a.nombre).join(' y ')}? Confírmalo en Nexo.`,
      estado: copia(),
    };
  for (const a of lista) aplicar(a, o.accion);
  return { ...base, ok: true, hechos: lista.map((a) => a.id), mensaje: 'Listo.', estado: copia() };
}

/** Un hallazgo de ejemplo: una luz de la red, lista para usar, salvo que se diga otra cosa. */
const hallazgo = (h: Partial<Hallazgo> & Pick<Hallazgo, 'id' | 'nombre' | 'marca'>): Hallazgo => ({
  via: 'wifi',
  proveedor: 'ninguno',
  clase: 'luz',
  forma: null,
  modelo: null,
  direccion: null,
  mac: null,
  senal: null,
  version: null,
  capacidades: caps({ informaEstado: false }),
  estado: null,
  manejo: { tipo: 'listo' },
  agregado: false,
  ...h,
});

// Lo que sale en "Buscar aparatos" y en "Tu red" con el mismo id.
const puenteHue = () =>
  hallazgo({
    id: HUE, nombre: 'Puente Hue', marca: 'Philips Hue', proveedor: 'hue', clase: 'otro', modelo: 'BSB002',
    direccion: '192.168.1.20', mac: 'ec:b5:fa:1c:20:4e',
    manejo: { tipo: 'vincular', forma: 'boton', paso: 'Presiona el botón redondo del puente Hue.' },
  });
const wledRepisa = () =>
  hallazgo({
    id: 'wled:a842e3d0c5b7', nombre: 'WLED de la repisa', marca: 'WLED', proveedor: 'wled', forma: 'tira',
    modelo: 'WLED 0.15', direccion: '192.168.1.33', mac: 'a8:42:e3:d0:c5:b7', capacidades: luzCaps(false, true),
  });
const shellyConClave = () =>
  hallazgo({
    id: 'shelly:c8f09e4b2a10', nombre: 'Shelly Plus 1PM', marca: 'Shelly', proveedor: 'shelly', clase: 'enchufe',
    modelo: 'SNSW-001P16EU', direccion: '192.168.1.41', mac: 'c8:f0:9e:4b:2a:10',
    capacidades: caps({ encender: true }),
    manejo: { tipo: 'vincular', forma: 'clave', paso: 'Escribe la contraseña que le pusiste en la app de Shelly.' },
  });
const aparatoTuya = () => {
  const conTuya = !!casa.tuya;
  return hallazgo({
    id: 'tuya:ejtira01', nombre: conTuya ? 'Tira del escritorio' : 'Aparato Tuya', marca: 'Tuya / Smart Life',
    proveedor: 'tuya', clase: conTuya ? 'luz' : 'otro', direccion: '192.168.1.37', mac: 'd8:1f:12:6a:90:3e',
    version: '3.4', manejo: conTuya ? { tipo: 'listo' } : { tipo: 'cuentaTuya' }, agregado: conTuya,
  });
};

/** Ya está en Nexo: el mismo id, o un aparato del mismo proveedor en esa IP (las luces de un puente). */
const yaEsta = (h: Hallazgo) =>
  casa.aparatos.some((a) => a.id === h.id || (!!h.direccion && a.direccion === h.direccion && a.proveedor === h.proveedor));

function hallazgosEjemplo(): Hallazgo[] {
  return [
    hallazgo({
      id: 'wiz:a8bb50d2c3e4', nombre: 'Foco WiZ', marca: 'WiZ', proveedor: 'wiz', forma: 'foco', modelo: 'ESP01_SHRGB1C_31',
      direccion: '192.168.1.48', mac: 'a8:bb:50:d2:c3:e4', senal: -58, capacidades: luzCaps(true, true),
    }),
    puenteHue(),
    hallazgo({
      id: 'nanoleaf:00552d9a7713', nombre: 'Nanoleaf Lines', marca: 'Nanoleaf', proveedor: 'nanoleaf', forma: 'lampara',
      direccion: '192.168.1.45', mac: '00:55:2d:9a:77:13', capacidades: luzCaps(true, true),
      manejo: {
        tipo: 'vincular',
        forma: 'boton',
        paso: 'Mantén presionado el botón de encendido del Nanoleaf de 5 a 7 segundos, hasta que sus luces parpadeen.',
      },
    }),
    hallazgo({
      id: 'elgato:3c6a9d2277e1', nombre: 'Elgato Light Strip', marca: 'Elgato', proveedor: 'elgato', forma: 'tira',
      direccion: '192.168.1.35', mac: '3c:6a:9d:22:77:e1', capacidades: luzCaps(true, true),
    }),
    wledRepisa(),
    shellyConClave(),
    hallazgo({
      id: 'tasmota:84f3eb6c0d22', nombre: 'Foco con OpenBeken', marca: 'OpenBeken', proveedor: 'tasmota', forma: 'foco',
      direccion: '192.168.1.43', mac: '84:f3:eb:6c:0d:22', capacidades: luzCaps(true, true),
    }),
    hallazgo({
      id: 'lifx:d073d5b40e17', nombre: 'LIFX Mini', marca: 'LIFX', proveedor: 'lifx', forma: 'foco', modelo: 'LIFX Mini Color',
      direccion: '192.168.1.47', mac: 'd0:73:d5:b4:0e:17', capacidades: luzCaps(true, true),
    }),
    hallazgo({
      id: 'govee:1f80c5323236724e', nombre: 'Tira Govee H6199', marca: 'Govee', proveedor: 'govee', forma: 'tira',
      modelo: 'H6199', direccion: '192.168.1.52', capacidades: luzCaps(true, true),
    }),
    hallazgo({
      id: 'ble:5f1c0a9e', nombre: 'ELK-BLEDOM', marca: 'ELK-BLEDOM', proveedor: 'bleElk', via: 'bluetooth', forma: 'tira',
      direccion: '5F1C0A9E', senal: -67, capacidades: { ...luzCaps(true, true), informaEstado: false },
    }),
    aparatoTuya(),
    hallazgo({
      id: 'ble:9c2e7b10', nombre: 'Mi Monitor Light Bar', marca: 'Xiaomi', via: 'bluetooth', forma: 'barra', senal: -74,
      manejo: { tipo: 'pronto', motivo: 'Usa el cifrado propio de Xiaomi; Nexo no puede manejarla por Bluetooth.' },
    }),
    hallazgo({
      id: 'ssdp:samsung-qn55', nombre: 'Samsung QN55Q60', marca: 'Samsung', clase: 'tv', direccion: '192.168.1.60',
      manejo: { tipo: 'pronto', motivo: 'Detectado · pronto podrás manejarlo desde Nexo.' },
    }),
    hallazgo({
      id: 'ha:7f3e9a', nombre: 'Casa', marca: 'Home Assistant', proveedor: 'homeAssistant', clase: 'otro',
      direccion: '192.168.1.10', version: '2026.10.2', manejo: { tipo: 'homeAssistant', url: 'http://homeassistant.local:8123' },
      agregado: !!casa.homeAssistant,
    }),
  ].map((h) => ({ ...h, agregado: h.agregado || yaEsta(h) }));
}

/** Suma lo encontrado a la lista con la que se agrega o se vincula (lo nuevo reemplaza a lo viejo). */
function recordar(lista: Hallazgo[]) {
  hallados = [...hallados.filter((x) => !lista.some((h) => h.id === x.id)), ...lista];
}

function buscarEjemplo(): ResultadoBusqueda {
  const lista = hallazgosEjemplo();
  recordar(lista);
  return { hallazgos: structuredClone(lista), avisos: [], segundos: 6, estado: copia() };
}

function redEjemplo(): ResultadoRed {
  const eq = (e: Partial<EquipoRed> & Pick<EquipoRed, 'ip' | 'nombre' | 'tipo' | 'grupo'>): EquipoRed => ({
    mac: null,
    privada: false,
    nombreRed: null,
    marca: null,
    motivo: null,
    hallazgo: null,
    aparatos: [],
    ...e,
  });
  const tele = 'Es una tele. Pronto podrás manejarla desde Nexo.';
  const lista: EquipoRed[] = [
    eq({ ip: '192.168.1.1', mac: 'f4:92:bf:3a:10:01', nombre: 'Router', marca: 'TP-Link', tipo: 'router', grupo: 'otros',
      motivo: 'Es tu router: reparte internet en tu casa.' }),
    eq({ ip: '192.168.1.23', mac: '3c:22:fb:8e:41:07', nombre: 'Este equipo', nombreRed: 'MacBook-Pro.local', marca: 'Apple',
      tipo: 'esteEquipo', grupo: 'otros', motivo: 'Es este equipo.' }),
    eq({ ip: '192.168.1.57', mac: '9a:3f:1e:c4:77:20', privada: true, nombre: 'Teléfono', nombreRed: 'iPhone.local',
      tipo: 'telefono', grupo: 'otros', motivo: 'Es un teléfono o una tableta.' }),
    eq({ ip: '192.168.1.70', mac: '90:dd:5d:12:aa:31', nombre: 'Apple TV de la sala', marca: 'Apple', tipo: 'tele',
      grupo: 'otros', motivo: tele }),
    eq({ ip: '192.168.1.60', mac: '8c:79:f5:3e:61:0b', nombre: 'Samsung QN55Q60', marca: 'Samsung', tipo: 'tele',
      grupo: 'otros', motivo: tele }),
    eq({ ip: '192.168.1.80', mac: '74:3a:ef:5c:80:42', nombre: 'HP LaserJet M110w', marca: 'HP', tipo: 'impresora',
      grupo: 'otros', motivo: 'Es una impresora.' }),
    eq({ ip: '192.168.1.82', mac: '1c:53:f9:0e:82:6d', nombre: 'Parlante de la cocina', nombreRed: 'Nest-Mini.local',
      marca: 'Google', tipo: 'parlante', grupo: 'otros', motivo: 'Es un parlante con Chromecast. Pronto podrás manejarlo desde Nexo.' }),
    eq({ ip: '192.168.1.20', mac: 'ec:b5:fa:1c:20:4e', nombre: 'Puente Hue', marca: 'Philips Hue', tipo: 'luz',
      grupo: 'agregar', hallazgo: puenteHue() }),
    eq({ ip: '192.168.1.33', mac: 'a8:42:e3:d0:c5:b7', nombre: 'WLED de la repisa', nombreRed: 'wled-repisa.local',
      marca: 'Espressif', tipo: 'luz', grupo: 'agregar', hallazgo: wledRepisa() }),
    eq({ ip: '192.168.1.41', mac: 'c8:f0:9e:4b:2a:10', nombre: 'Shelly Plus 1PM', marca: 'Shelly', tipo: 'enchufe',
      grupo: 'agregar', hallazgo: shellyConClave() }),
    eq({ ip: '192.168.1.31', mac: '3c:6a:9d:14:b2:c8', nombre: 'Elgato Key Light', marca: 'Elgato', tipo: 'luz', grupo: 'nexo' }),
    eq({ ip: '192.168.1.32', mac: 'a8:42:e3:c9:1f:20', nombre: 'WLED', marca: 'Espressif', tipo: 'luz', grupo: 'nexo' }),
    eq({ ip: '192.168.1.40', mac: 'e8:68:e7:a1:b3:c4', nombre: 'Shelly Plus Plug S', marca: 'Shelly', tipo: 'enchufe', grupo: 'nexo' }),
    eq({ ip: '192.168.1.42', mac: 'dc:4f:22:a5:e6:10', nombre: 'Sonoff S31 con Tasmota', marca: 'Espressif', tipo: 'enchufe', grupo: 'nexo' }),
    eq({ ip: '192.168.1.44', mac: '00:55:2d:8e:4f:1a', nombre: 'Nanoleaf Shapes', marca: 'Nanoleaf', tipo: 'luz', grupo: 'nexo' }),
    eq({ ip: '192.168.1.37', mac: 'd8:1f:12:6a:90:3e', nombre: 'Enchufe inteligente', marca: 'Tuya', tipo: 'enchufe',
      grupo: 'otros', motivo: 'Solo funciona con la nube de Tuya.', hallazgo: aparatoTuya() }),
    eq({ ip: '192.168.1.90', mac: '24:6f:28:b1:5e:93', nombre: 'Equipo sin nombre', marca: 'Espressif', tipo: 'desconocido',
      grupo: 'otros', motivo: 'No sé qué es: no contestó a nada que Nexo conozca.' }),
  ];
  // Lo que Nexo ya maneja en esa IP pasa a «Se manejan con Nexo»; lo marcado así sin aparatos no se muestra.
  const equipos = lista
    .map((e) => {
      const suyos = casa.aparatos
        .filter((a) => a.direccion === e.ip || (e.hallazgo && a.id === e.hallazgo.id))
        .map((a) => a.id);
      return suyos.length ? { ...e, grupo: 'nexo' as const, motivo: null, hallazgo: null, aparatos: suyos } : e;
    })
    .filter((e) => e.grupo !== 'nexo' || e.aparatos.length);
  recordar(equipos.flatMap((e) => (e.hallazgo ? [e.hallazgo] : [])));
  return {
    equipos: structuredClone(equipos),
    ip: '192.168.1.23',
    router: '192.168.1.1',
    prefijo: 24,
    avisos: [],
    segundos: 4,
    estado: copia(),
  };
}

/** Pasa a Nexo lo encontrado, con lo que se sabe de él. */
function sumar(h: Hallazgo, extra: Partial<Aparato> = {}) {
  const id = extra.id ?? h.id;
  if (casa.aparatos.some((a) => a.id === id)) return;
  casa.aparatos.push(
    ap({
      id, nombre: h.nombre, clase: h.clase, forma: h.forma, proveedor: h.proveedor, via: h.via, marca: h.marca,
      modelo: h.modelo, direccion: h.direccion, capacidades: h.capacidades,
      estado: est({ encendido: true, brillo: 80, temperatura: 40, modo: 'blanco' }),
      ...extra,
    }),
  );
}

/** Intentos con el botón, por aparato: a la tercera "se presionó". */
const intentos = new Map<string, number>();

function vincularEjemplo(id: string, clave?: string): ResultadoVinculo {
  const h = hallados.find((x) => x.id === id);
  if (!h || h.manejo.tipo !== 'vincular') throw 'Busca de nuevo: ya no lo veo en la lista.';
  if (h.manejo.forma === 'boton') {
    const n = (intentos.get(id) ?? 0) + 1;
    intentos.set(id, n);
    if (n < 3) return { listo: false, mensaje: 'Todavía no veo el botón presionado.', estado: copia() };
    intentos.delete(id);
  } else if ((clave ?? '').trim().length < 4) {
    throw 'La contraseña no es correcta.';
  }
  if (h.proveedor === 'hue') {
    const luz = { clase: 'luz' as const, capacidades: luzCaps(true, false) };
    sumar(h, { ...luz, id: `${h.id}-1`, nombre: 'Foco de la sala', forma: 'foco', habitacion: 'sala', capacidades: luzCaps(true, true) });
    sumar(h, { ...luz, id: `${h.id}-2`, nombre: 'Lámpara de pie', forma: 'lampara', habitacion: 'sala' });
    sumar(h, {
      ...luz, id: `${h.id}-3`, nombre: 'Foco de la cocina', forma: 'foco', habitacion: 'cocina',
      estado: est({ encendido: false, brillo: 70, temperatura: 30, modo: 'blanco' }),
    });
    return { listo: true, mensaje: 'Listo: agregué 3 luces del puente Hue.', estado: copia() };
  }
  sumar(h);
  return { listo: true, mensaje: `Listo: agregué ${h.nombre}.`, estado: copia() };
}

function agregarEjemplo(id: string): EstadoCasa {
  const h = hallados.find((x) => x.id === id);
  if (!h || h.manejo.tipo !== 'listo') throw 'Busca de nuevo: ya no lo veo en la lista.';
  sumar(h);
  return copia();
}

function editarEjemplo(id: string, c: CambioAparato): EstadoCasa {
  const a = casa.aparatos.find((x) => x.id === id);
  if (!a) throw 'Ese aparato ya no está en tu casa.';
  if (c.alias !== undefined) {
    const t = (c.alias ?? '').trim();
    a.alias = t || null;
    a.nombre = t || a.nombreOriginal;
  }
  if (c.habitacion !== undefined) a.habitacion = c.habitacion || null;
  if (c.forma) a.forma = c.forma;
  return copia();
}

function grupoEjemplo(g: Grupo): EstadoCasa {
  const nombre = g.nombre.trim();
  if (!nombre) throw 'Ponle un nombre al grupo.';
  if (!g.aparatos.length) throw 'Elige al menos un aparato para el grupo.';
  const i = casa.grupos.findIndex((x) => x.id && x.id === g.id);
  const limpio = { ...g, nombre };
  if (i >= 0) casa.grupos[i] = limpio;
  else casa.grupos.push({ ...limpio, id: `g${Date.now()}` });
  return copia();
}

function habitacionEjemplo(h: Habitacion): EstadoCasa {
  const nombre = h.nombre.trim();
  if (!nombre) throw 'Ponle un nombre a la habitación.';
  const i = casa.habitaciones.findIndex((x) => x.id === h.id);
  if (i >= 0) casa.habitaciones[i] = { ...h, nombre };
  else casa.habitaciones.push({ id: `h${Date.now()}`, nombre });
  return copia();
}

function borrarHabitacionEjemplo(id: string): EstadoCasa {
  casa.habitaciones = casa.habitaciones.filter((h) => h.id !== id);
  for (const a of casa.aparatos) if (a.habitacion === id) a.habitacion = null;
  return copia();
}

function conectarTuyaEjemplo(id: string, secreto: string, region: string): EstadoCasa {
  if (id.trim().length < 8 || secreto.trim().length < 8) throw 'Revisa el Access ID y el Access Secret: cópialos completos, sin espacios.';
  const r = region === 'auto' ? 'us' : region;
  casa.tuya = { accessId: id.trim(), region: r, regionNombre: REGIONES.find((x) => x[0] === r)?.[1] ?? '' };
  casa.errorTuya = null;
  if (!casa.aparatos.some((a) => a.proveedor === 'tuya')) casa.aparatos.unshift(...aparatosTuya());
  if (!casa.grupos.length)
    casa.grupos.push({ id: 'g1', nombre: 'Escritorio', aparatos: ['tuya:ejtira01', 'tuya:ejbarra01'], habitacion: 'escritorio' });
  return copia();
}

function desconectarTuyaEjemplo(): EstadoCasa {
  casa.tuya = null;
  casa.errorTuya = null;
  casa.aparatos = casa.aparatos.filter((a) => a.proveedor !== 'tuya');
  return copia();
}

function conectarHaEjemplo(url: string, token: string): EstadoCasa {
  if (!/^https?:\/\/|^[\w.-]+(:\d+)?$/.test(url.trim())) throw 'Escribe la dirección de tu Home Assistant, por ejemplo http://homeassistant.local:8123.';
  if (token.trim().length < 20) throw 'Revisa el token: cópialo completo, sin espacios.';
  casa.homeAssistant = { url: url.trim().replace(/\/$/, '') };
  casa.errorHa = null;
  if (!casa.aparatos.some((a) => a.proveedor === 'homeAssistant')) casa.aparatos.push(...aparatosHa());
  if (!casa.habitaciones.some((h) => h.id === 'area-entrada')) casa.habitaciones.push({ id: 'area-entrada', nombre: 'Entrada' });
  casa.ocultos = 1;
  return copia();
}

function desconectarHaEjemplo(): EstadoCasa {
  casa.homeAssistant = null;
  casa.aparatos = casa.aparatos.filter((a) => a.proveedor !== 'homeAssistant');
  casa.ocultos = 0;
  return copia();
}
