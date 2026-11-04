// Llamadas a Rust para la casa (sistema/hogar.rs): estado, órdenes, búsqueda, cuentas, habitaciones y grupos.
// Fuera de la app usa una casa de ejemplo (la misma de hogar/ejemplo.rs); `?casa=` elige el caso:
//   (sin nada)  Tuya y Home Assistant conectados: tira RGB, barra del monitor, dos focos, enchufe, tele, puerta,
//               cerradura, cortina y dos luces más
//   nada        sin cuentas ni aparatos (el estado vacío)
//   tuya        solo la cuenta de Tuya
//   error       Tuya conectada, pero venció su plan de prueba
//   buscar      abre Casa buscando (la búsqueda de ejemplo tarda 1,5 s)
import { invoke } from '@tauri-apps/api/core';
import type {
  Accion,
  Aparato,
  CambioAparato,
  Capacidades,
  EstadoAparato,
  EstadoCasa,
  Grupo,
  Habitacion,
  Hallazgo,
  Orden,
  ResultadoBusqueda,
  ResultadoOrden,
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

function nueva(): EstadoCasa {
  const p = pruebaCasa;
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

function hallazgosEjemplo(): Hallazgo[] {
  const base = (h: Partial<Hallazgo> & Pick<Hallazgo, 'id' | 'nombre' | 'marca'>): Hallazgo => ({
    via: 'wifi',
    proveedor: 'ninguno',
    clase: 'luz',
    forma: null,
    modelo: null,
    direccion: null,
    senal: null,
    version: null,
    capacidades: caps({ informaEstado: false }),
    estado: null,
    manejo: { tipo: 'listo' },
    agregado: false,
    ...h,
  });
  const conTuya = !!casa.tuya;
  return [
    base({
      id: 'wiz:a8bb50d2c3e4', nombre: 'Foco WiZ', marca: 'WiZ', proveedor: 'wiz', forma: 'foco', modelo: 'ESP01_SHRGB1C_31',
      direccion: '192.168.1.41', senal: -58, capacidades: luzCaps(true, true),
    }),
    base({
      id: 'govee:1f80c5323236724e', nombre: 'Tira Govee H6199', marca: 'Govee', proveedor: 'govee', forma: 'tira',
      modelo: 'H6199', direccion: '192.168.1.52', capacidades: luzCaps(true, true),
    }),
    base({
      id: 'ble:5f1c0a9e', nombre: 'ELK-BLEDOM', marca: 'ELK-BLEDOM', proveedor: 'bleElk', via: 'bluetooth', forma: 'tira',
      direccion: '5F1C0A9E', senal: -67, capacidades: { ...luzCaps(true, true), informaEstado: false },
    }),
    base({
      id: 'tuya:ejtira01', nombre: conTuya ? 'Tira del escritorio' : 'Aparato Tuya', marca: 'Tuya / Smart Life',
      proveedor: 'tuya', clase: conTuya ? 'luz' : 'otro', direccion: '192.168.1.37', version: '3.4',
      manejo: conTuya ? { tipo: 'listo' } : { tipo: 'cuentaTuya' }, agregado: conTuya,
    }),
    base({
      id: 'ble:9c2e7b10', nombre: 'Mi Monitor Light Bar', marca: 'Xiaomi', via: 'bluetooth', forma: 'barra', senal: -74,
      manejo: { tipo: 'pronto', motivo: 'Usa el cifrado propio de Xiaomi; Nexo no puede manejarla por Bluetooth.' },
    }),
    base({
      id: 'ssdp:samsung-qn55', nombre: 'Samsung QN55Q60', marca: 'Samsung', clase: 'tv', direccion: '192.168.1.60',
      manejo: { tipo: 'pronto', motivo: 'Detectado · pronto podrás manejarlo desde Nexo.' },
    }),
    base({
      id: 'ha:7f3e9a', nombre: 'Casa', marca: 'Home Assistant', proveedor: 'homeAssistant', clase: 'otro',
      direccion: '192.168.1.10', version: '2026.10.2', manejo: { tipo: 'homeAssistant', url: 'http://homeassistant.local:8123' },
      agregado: !!casa.homeAssistant,
    }),
  ].map((h) => ({ ...h, agregado: h.agregado || casa.aparatos.some((a) => a.id === h.id) }));
}

function buscarEjemplo(): ResultadoBusqueda {
  hallados = hallazgosEjemplo();
  return { hallazgos: structuredClone(hallados), avisos: [], segundos: 6, estado: copia() };
}

function agregarEjemplo(id: string): EstadoCasa {
  const h = hallados.find((x) => x.id === id);
  if (!h || h.manejo.tipo !== 'listo') throw 'Busca de nuevo: ya no lo veo en la lista.';
  if (!casa.aparatos.some((a) => a.id === id))
    casa.aparatos.push(
      ap({
        id: h.id, nombre: h.nombre, clase: h.clase, forma: h.forma, proveedor: h.proveedor, via: h.via, marca: h.marca,
        modelo: h.modelo, direccion: h.direccion, capacidades: h.capacidades,
        estado: est({ encendido: true, brillo: 80, temperatura: 40, modo: 'blanco' }),
      }),
    );
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
