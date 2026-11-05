// La casa, sin pantalla: colores de cada luz, nombres de clases y formas, estados en palabras y las escenas.
import type { Aparato, ClaseAparato, Destino, EstadoCasa, FormaLuz, Hsv, Objetivo, Proveedor } from './tipos-hogar';

// ---------------------------------------------------------------- Colores

/** Un color de Nexo en CSS (`v` es el brillo; para el resplandor se usa con brillo fijo). */
export function cssDeHsv(c: Hsv, brillo = c.v): string {
  const s = c.s / 100;
  const v = Math.max(0, Math.min(100, brillo)) / 100;
  const l = v * (1 - s / 2);
  const sl = l === 0 || l === 1 ? 0 : (v - l) / Math.min(l, 1 - l);
  return `hsl(${Math.round(c.h)} ${Math.round(sl * 100)}% ${Math.round(l * 100)}%)`;
}

const BLANCOS: [number, [number, number, number]][] = [
  [0, [255, 170, 86]],
  [50, [255, 228, 196]],
  [100, [214, 230, 255]],
];

/** El blanco de una temperatura: ámbar cálido (0), neutro (50) y frío azulado (100). */
export function cssDeBlanco(t: number): string {
  const x = Math.max(0, Math.min(100, t));
  const [a, b] = x <= 50 ? [BLANCOS[0], BLANCOS[1]] : [BLANCOS[1], BLANCOS[2]];
  const f = (x - a[0]) / (b[0] - a[0]);
  const c = a[1].map((v, i) => Math.round(v + (b[1][i] - v) * f));
  return `rgb(${c[0]} ${c[1]} ${c[2]})`;
}

/** El color con que brilla una luz encendida (para el resplandor, el relleno de su brillo y el mapa). */
export function colorDeLuz(a: Aparato): string {
  const e = a.estado;
  if (e.modo === 'color' && e.color) return cssDeHsv(e.color, 100);
  if (e.temperatura !== null) return cssDeBlanco(e.temperatura);
  return cssDeBlanco(30);
}

/** Muestras de color de un toque (tono y saturación; el brillo es el de la luz). */
export const MUESTRAS: { nombre: string; h: number; s: number }[] = [
  { nombre: 'Rojo', h: 0, s: 100 },
  { nombre: 'Naranja', h: 28, s: 100 },
  { nombre: 'Amarillo', h: 50, s: 100 },
  { nombre: 'Verde', h: 125, s: 90 },
  { nombre: 'Turquesa', h: 172, s: 95 },
  { nombre: 'Azul', h: 218, s: 95 },
  { nombre: 'Morado', h: 272, s: 85 },
  { nombre: 'Rosa', h: 322, s: 80 },
];

/** Pista del tono (el arco iris) y del blanco (cálido a frío), para el deslizador. */
export const PISTA_TONO =
  'linear-gradient(90deg, hsl(0 100% 55%), hsl(60 100% 50%), hsl(120 90% 45%), hsl(180 90% 45%), hsl(240 90% 60%), hsl(300 90% 55%), hsl(360 100% 55%))';
export const PISTA_BLANCO = `linear-gradient(90deg, ${cssDeBlanco(0)}, ${cssDeBlanco(50)}, ${cssDeBlanco(100)})`;

// ---------------------------------------------------------------- Nombres

const CLASES: Record<ClaseAparato, string> = {
  luz: 'Luz',
  enchufe: 'Enchufe',
  tv: 'Tele',
  cerradura: 'Cerradura',
  sensorPuerta: 'Sensor de puerta',
  cortina: 'Cortina',
  clima: 'Clima',
  otro: 'Aparato',
};

const FORMAS: Record<FormaLuz, string> = {
  tira: 'Tira LED',
  foco: 'Foco',
  barra: 'Lámpara de monitor',
  lampara: 'Lámpara',
};

export const FORMAS_LUZ: { valor: FormaLuz; nombre: string }[] = (Object.keys(FORMAS) as FormaLuz[]).map((f) => ({
  valor: f,
  nombre: FORMAS[f],
}));

/** "Tira LED", "Lámpara de monitor", "Enchufe", "Tele"… */
export const tipoDe = (a: { clase: ClaseAparato; forma: FormaLuz | null }) =>
  a.clase === 'luz' && a.forma ? FORMAS[a.forma] : CLASES[a.clase];

export const nombreClase = (c: ClaseAparato) => CLASES[c];

const PROVEEDORES: Record<Proveedor, string> = {
  tuya: 'Tuya / Smart Life',
  wiz: 'WiZ',
  yeelight: 'Yeelight',
  govee: 'Govee',
  hue: 'Philips Hue',
  lifx: 'LIFX',
  shelly: 'Shelly',
  nanoleaf: 'Nanoleaf',
  elgato: 'Elgato',
  wled: 'WLED',
  tasmota: 'Tasmota',
  bleElk: 'Bluetooth',
  bleTriones: 'Bluetooth',
  homeAssistant: 'Home Assistant',
  ninguno: '',
  ejemplo: 'Ejemplo',
};

/** Por dónde se maneja, en corto: "Tuya", "WiZ · en tu red", "Bluetooth", "Home Assistant". */
export function viaDe(a: Aparato): string {
  if (a.proveedor === 'tuya') return 'Tuya';
  if (a.via === 'bluetooth') return 'Bluetooth';
  if (a.proveedor === 'homeAssistant') return 'Home Assistant';
  return `${PROVEEDORES[a.proveedor] || a.marca || 'En tu red'} · en tu red`;
}

/** El género de cada clase, para decir "encendida" o "encendido". */
const femenina = (c: ClaseAparato) => c === 'luz' || c === 'tv' || c === 'cerradura' || c === 'cortina' || c === 'clima';
const g = (c: ClaseAparato, masc: string, fem: string) => (femenina(c) ? fem : masc);

/** Cómo está, en una línea ("Encendida · 70 %", "Cerrada", "Abierta al 70 %", "Sin respuesta"). */
export function estadoEnPalabras(a: Aparato): string {
  if (!a.enLinea) return 'Sin respuesta';
  const e = a.estado;
  switch (a.clase) {
    case 'cerradura':
      return e.abierto === null ? 'Sin datos' : e.abierto ? 'Abierta' : 'Cerrada con llave';
    case 'sensorPuerta':
      return e.abierto === null ? 'Sin datos' : e.abierto ? 'Abierta' : 'Cerrada';
    case 'cortina':
      if (e.posicion !== null) return e.posicion === 0 ? 'Cerrada' : e.posicion === 100 ? 'Abierta' : `Abierta al ${e.posicion}\u00a0%`;
      return e.abierto === null ? 'Sin datos' : e.abierto ? 'Abierta' : 'Cerrada';
    case 'tv':
      if (!e.encendido) return 'Apagada';
      return [
        'Encendida',
        a.entrada,
        e.silenciado ? 'Sin sonido' : e.volumen !== null ? `Volumen ${e.volumen}` : null,
      ]
        .filter(Boolean)
        .join(' · ');
    case 'clima':
      return [e.encendido ? 'Encendido' : 'Apagado', e.grados !== null ? `${Math.round(e.grados)} °C` : null].filter(Boolean).join(' · ');
    default:
      if (e.encendido === null) return a.controlable ? 'Sin datos' : '';
      if (!e.encendido) return g(a.clase, 'Apagado', 'Apagada');
      return e.brillo !== null && a.capacidades.brillo
        ? `${g(a.clase, 'Encendido', 'Encendida')} · ${e.brillo}\u00a0%`
        : g(a.clase, 'Encendido', 'Encendida');
  }
}

/** Está "prendido" para el mapa y los resúmenes (una puerta abierta también llama la atención). */
export const activo = (a: Aparato) =>
  a.enLinea && (a.clase === 'sensorPuerta' || a.clase === 'cerradura' ? a.estado.abierto === true : a.estado.encendido === true);

// ---------------------------------------------------------------- Habitaciones y grupos

/** La habitación del escritorio: la de Mi setup. */
export const ESCRITORIO = 'escritorio';

/** Las luces del escritorio: las de la habitación Escritorio o las de un grupo con ese nombre. */
export function delEscritorio(c: EstadoCasa): Aparato[] {
  const grupo = c.grupos.find((g) => normalizar(g.nombre) === 'escritorio');
  return c.aparatos.filter(
    (a) => !a.oculto && (a.habitacion === ESCRITORIO || !!grupo?.aparatos.includes(a.id)) && (a.clase === 'luz' || a.clase === 'enchufe'),
  );
}

export function normalizar(t: string): string {
  return t
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9ñ]+/g, ' ')
    .trim();
}

export const hayCuentas = (c: EstadoCasa | null) => !!c && (!!c.tuya || !!c.homeAssistant || c.aparatos.length > 0);

// ---------------------------------------------------------------- Escenas

/** A quién va un paso, en palabras ("Todas las luces", "Escritorio", "Tira del escritorio"). */
export function nombreDestino(d: Destino | null | undefined, c: EstadoCasa | null, guardado?: string | null): string {
  if (!d) return guardado || 'Elige qué aparatos';
  switch (d.tipo) {
    case 'todos':
      return d.clase === 'luz' ? 'Todas las luces' : d.clase ? `Todos: ${nombreClase(d.clase).toLowerCase()}` : 'Todo';
    case 'aparato':
      return c?.aparatos.find((a) => a.id === d.id)?.nombre ?? guardado ?? 'Un aparato';
    case 'grupo':
      return c?.grupos.find((g) => g.id === d.id)?.nombre ?? guardado ?? 'Un grupo';
    case 'habitacion': {
      const h = c?.habitaciones.find((x) => x.id === d.id)?.nombre ?? guardado;
      return h ? `Luces de ${h}` : 'Una habitación';
    }
    case 'nombre':
      return d.texto;
  }
}

/** El estado que pide un paso, en corto ("Encendidas al 85 %, blanco frío", "Apagadas", "Azul al 40 %"). */
export function resumenObjetivo(o: Objetivo | null | undefined): string {
  if (!o) return 'Elige cómo deben quedar';
  if (!o.encendido) return 'Apagar';
  const partes = ['Encender'];
  if (o.color) {
    const m = MUESTRAS.reduce((x, y) => (Math.abs(y.h - o.color!.h) < Math.abs(x.h - o.color!.h) ? y : x));
    partes.push(`en ${m.nombre.toLowerCase()}`);
  } else if (o.temperatura !== null) {
    partes.push(o.temperatura < 34 ? 'blanco cálido' : o.temperatura > 66 ? 'blanco frío' : 'blanco neutro');
  }
  if (o.brillo !== null) partes.push(`al ${o.brillo}\u00a0%`);
  return partes.join(' ').replace('Encender blanco', 'Encender en blanco');
}
