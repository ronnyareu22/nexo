// Mi setup, sin pantalla: cómo se dibuja cada cosa (foto, tamaño, dónde apoya), la escena del escritorio
// (pared, mesa y tres capas de profundidad), los mapas de la primera vez y las escenas en palabras.
//
// La escena mide 16:10. Lo de la mesa apoya en una de tres capas: al fondo va más chico y más arriba, al frente
// más grande y más abajo. Lo de la pared (cuadro, repisa, tira de luz) va donde se suelte. Todo en % de la escena.
import type { Dispositivo, Tipo } from './tipos';
import { fotoDePantalla, modeloDe } from './catalogo';
import { activo, colorDeLuz, estadoEnPalabras, resumenObjetivo, tipoDe } from './hogar';
import type { Aparato as AparatoCasa, ClaseAparato, EstadoCasa, FormaLuz } from './tipos-hogar';
import type {
  Accion,
  Aparato,
  Aparatos,
  Deco,
  Escena,
  ItemMapa,
  LugarId,
  LugarMapa,
  PlataformaSetup,
  TipoAccion,
  TipoItem,
} from './tipos-setup';

// ---------------------------------------------------------------- Escena

/** El alto de un cuadro, en % del alto de la escena, por cada 1 % de ancho (16:10). */
export const PROPORCION = 1.6;
/** Donde termina la pared y empieza la mesa (% del alto). */
export const PARED = 56;
/** Dónde apoya cada capa (% del alto): fondo, medio y frente. */
export const BASES = [62, 71, 80] as const;
/** Lo del fondo se ve más chico. */
export const FACTOR = [0.82, 0.91, 1] as const;
export const NOMBRE_CAPA = ['Al fondo', 'Al medio', 'Al frente'] as const;
const X_MIN = 5;
const X_MAX = 95;

export const LUGARES: { id: LugarId; nombre: string }[] = [
  { id: 'casa', nombre: 'Casa' },
  { id: 'oficina', nombre: 'Oficina' },
];

/** La Mac es de la casa y el PC con Windows, de la oficina. */
export const lugarDe = (p: PlataformaSetup): LugarId => (p === 'windows' ? 'oficina' : 'casa');

/** Cómo se dibuja algo. */
export interface Forma {
  /** Foto sin fondo (cuadrada); si no hay, `ilustracion`. */
  foto: string | null;
  ilustracion: Tipo | 'torre' | null;
  /** Dónde está el objeto dentro de su cuadro: izquierda, arriba, derecha y abajo, de 0 a 1. */
  caja: readonly [number, number, number, number];
  /** Ancho del cuadro en % del ancho de la escena, con escala 1 y al frente. */
  tam: number;
  /** Va en la pared (no en la mesa). */
  pared?: boolean;
  /** Se aplasta a lo alto para que parezca acostado sobre la mesa (la alfombrilla, el teclado). */
  aplastar?: number;
}

const forma = (
  foto: string | null,
  caja: readonly [number, number, number, number],
  tam: number,
  extra: Partial<Forma> = {},
): Forma => ({ foto, ilustracion: null, caja, tam, ...extra });

/** Las decoraciones: fotos reales, solo visuales (nunca muestran un estado). */
export const DECOS: Record<Deco, { nombre: string; forma: Forma }> = {
  'planta-grande': { nombre: 'Maceta grande', forma: forma('/setup/planta-grande.png', [0.152, 0.04, 0.845, 0.958], 24) },
  'planta-pequena': { nombre: 'Suculentas', forma: forma('/setup/planta-pequena.png', [0.04, 0.235, 0.958, 0.762], 13) },
  lampara: { nombre: 'Lámpara de escritorio', forma: forma('/setup/lampara.png', [0.14, 0.04, 0.858, 0.958], 22) },
  taza: { nombre: 'Taza', forma: forma('/setup/taza.png', [0.04, 0.128, 0.958, 0.87], 8) },
  cuadro: { nombre: 'Cuadro', forma: forma('/setup/cuadro.png', [0.04, 0.19, 0.958, 0.807], 20, { pared: true }) },
  repisa: { nombre: 'Repisa', forma: forma('/setup/repisa.png', [0.04, 0.263, 0.958, 0.735], 22, { pared: true }) },
  'tira-luz': { nombre: 'Tira de luz', forma: forma('/setup/tira-luz.png', [0.04, 0.177, 0.958, 0.82], 14, { pared: true }) },
  'soporte-audifonos': {
    nombre: 'Soporte de audífonos',
    forma: forma('/setup/soporte-audifonos.png', [0.27, 0.04, 0.728, 0.958], 20),
  },
  alfombrilla: {
    nombre: 'Alfombrilla de escritorio',
    forma: forma('/setup/alfombrilla.png', [0.04, 0.19, 0.958, 0.807], 48, { aplastar: 0.3 }),
  },
  'base-monitor': { nombre: 'Base para monitor', forma: forma('/setup/base-monitor.png', [0.04, 0.31, 0.958, 0.688], 30) },
  parlante: { nombre: 'Parlante', forma: forma('/setup/parlante.png', [0.18, 0.04, 0.818, 0.958], 10) },
};

export const esDeco = (ref: string): ref is Deco => Object.hasOwn(DECOS, ref);

/** Dónde está cada aparato dentro de su foto (medido en las fotos de static/dispositivos). */
const CAJA_FOTO: Record<string, readonly [number, number, number, number]> = {
  '/dispositivos/sony-wh-ch520.png': [0.195, 0.029, 0.803, 0.971],
  '/dispositivos/beats-solo3.png': [0.169, 0.029, 0.829, 0.971],
  '/dispositivos/macbook-pro-14.png': [0.04, 0.223, 0.958, 0.775],
  '/dispositivos/mx-master-4.png': [0.092, 0.04, 0.907, 0.958],
  '/dispositivos/steren-27.png': [0.04, 0.158, 0.958, 0.84],
  '/dispositivos/wave-keys.png': [0.04, 0.18, 0.958, 0.815],
};

/** Y dentro de cada ilustración (Ilustracion.svelte, 200×200; la torre la dibuja Escritorio.svelte). */
const CAJA_ILUSTRACION: Record<Tipo | 'torre', readonly [number, number, number, number]> = {
  audifonos: [0.13, 0.31, 0.87, 0.83],
  diadema: [0.13, 0.31, 0.87, 0.8],
  telefono: [0.32, 0.11, 0.68, 0.86],
  teclado: [0.09, 0.39, 0.91, 0.68],
  mouse: [0.32, 0.17, 0.68, 0.83],
  parlante: [0.3, 0.17, 0.7, 0.84],
  monitor: [0.12, 0.22, 0.88, 0.855],
  luz: [0.12, 0.15, 0.88, 0.855],
  portatil: [0.09, 0.21, 0.91, 0.73],
  otro: [0.28, 0.28, 0.72, 0.72],
  torre: [0.3, 0.08, 0.7, 0.9],
};

/** Tamaño de cada clase de aparato (ancho del cuadro, en % de la escena). */
const TAM: Record<Tipo | 'torre', number> = {
  audifonos: 16,
  diadema: 16,
  telefono: 13,
  teclado: 26,
  mouse: 7,
  parlante: 10,
  monitor: 38,
  luz: 14,
  portatil: 22,
  otro: 10,
  torre: 16,
};

function formaDe(foto: string | null, tipo: Tipo | 'torre'): Forma {
  const caja = (foto && CAJA_FOTO[foto]) || CAJA_ILUSTRACION[tipo];
  return {
    foto: foto && CAJA_FOTO[foto] ? foto : null,
    ilustracion: foto && CAJA_FOTO[foto] ? null : tipo,
    caja,
    tam: TAM[tipo],
    aplastar: tipo === 'teclado' ? 0.62 : undefined,
  };
}

/** Dónde está cada aparato de la casa dentro de su dibujo (IlustracionAparato, 200×200, con `enMapa`) y su tamaño.
 *  La tira, la barra del monitor, el enchufe y lo de puertas y ventanas van en la pared; lo demás, en la mesa. */
export function formaDeHogar(a: { clase: ClaseAparato; forma: FormaLuz | null }): Forma {
  const f = (caja: readonly [number, number, number, number], tam: number, pared = false): Forma => ({
    foto: null,
    ilustracion: null,
    caja,
    tam,
    pared,
  });
  if (a.clase === 'luz') {
    switch (a.forma) {
      case 'tira':
        return f([0.08, 0.465, 0.92, 0.535], 30, true);
      case 'barra':
        return f([0.15, 0.28, 0.85, 0.43], 20, true);
      case 'lampara':
        return f([0.27, 0.17, 0.73, 0.94], 15);
      default:
        return f([0.29, 0.19, 0.71, 0.88], 8);
    }
  }
  switch (a.clase) {
    case 'enchufe':
      return f([0.26, 0.22, 0.74, 0.84], 6, true);
    case 'tv':
      return f([0.07, 0.2, 0.93, 0.885], 30);
    case 'clima':
      return f([0.11, 0.28, 0.89, 0.54], 20, true);
    case 'cortina':
    case 'cerradura':
    case 'sensorPuerta':
      return f([0.13, 0.13, 0.87, 0.88], 12, true);
    default:
      return f([0.28, 0.28, 0.72, 0.72], 8);
  }
}

/** El borde de arriba que se ve (lo aplastado queda más bajo). */
export const arribaVisible = (f: Forma) => f.caja[3] - (f.caja[3] - f.caja[1]) * (f.aplastar ?? 1);

// ---------------------------------------------------------------- De los datos a lo que se dibuja

/** Un aparato como lo esperan el catálogo y las ilustraciones. */
export const comoDispositivo = (a: Aparato): Dispositivo => ({
  id: a.id,
  nombre: a.nombre,
  alias: a.alias,
  tipo: a.tipo,
  conectado: a.conectado,
  bateria: a.bateria,
  bateriaIzquierda: null,
  bateriaDerecha: null,
  bateriaEstuche: null,
  fabricante: a.fabricante,
  firmware: null,
  vendorId: a.vendorId,
  productId: a.productId,
});

export const nombreAparato = (a: Aparato) => a.alias?.trim() || modeloDe(comoDispositivo(a))?.nombre || a.nombre;

/** Lo que se sabe de un equipo para dibujar sus cosas. */
export interface Fuente {
  aparatos: Aparatos;
  /** Este equipo. */
  local: boolean;
  /** Nexo responde y los datos son de ahora. */
  vivo: boolean;
  /** Cuándo se vieron los datos (solo si no son de ahora). */
  visto: number | null;
}

/** Algo del mapa, listo para dibujar. */
export interface Pieza {
  item: ItemMapa;
  forma: Forma;
  nombre: string;
  /** "Audífonos · En innotech1017" */
  detalle: string;
  /** null en las decoraciones (no muestran estado). */
  estado: 'encendido' | 'apagado' | null;
  /** "Conectado", "Sin conectar", "Encendida"… */
  estadoTexto: string;
  bateria: number | null;
  /** Es de este equipo: al tocarlo se abre su página en Nexo. */
  local: boolean;
  /** Los datos no son de ahora (el equipo está apagado o sin Nexo). */
  viejo: boolean;
  visto: number | null;
  /** Para las ilustraciones (marca y led). */
  tipoAparato: Tipo | null;
  marca: string | null;
  /** El id real del aparato de este equipo (para abrir su página). */
  aparatoId: string | null;
  /** Un aparato de la casa (una luz): cómo dibujarlo y con qué color brilla. */
  hogar: {
    id: string;
    clase: ClaseAparato;
    forma: FormaLuz | null;
    encendido: boolean;
    color: string | null;
    abierto: boolean | null;
    controlable: boolean;
  } | null;
}

const NOMBRE_TIPO: Record<Tipo, string> = {
  audifonos: 'Audífonos',
  diadema: 'Audífonos',
  telefono: 'Teléfono',
  teclado: 'Teclado',
  mouse: 'Mouse',
  parlante: 'Parlante',
  monitor: 'Monitor',
  luz: 'Luz',
  portatil: 'Portátil',
  otro: 'Dispositivo Bluetooth',
};

/** El dibujo de un equipo: la MacBook, una laptop con Windows o la torre del PC. */
export function formaDeEquipo(a: Aparatos): Forma {
  const conPantalla = a.pantallas.some((p) => p.interna);
  if (a.plataforma === 'mac') return formaDe(conPantalla ? '/dispositivos/macbook-pro-14.png' : null, conPantalla ? 'portatil' : 'monitor');
  return formaDe(null, conPantalla ? 'portatil' : 'torre');
}

export const fotoMonitor = (nombre: string) =>
  fotoDePantalla({ id: 0, nombre, interna: false, brillo: null, controlable: false });

/** De dónde sale cada cosa del mapa. Si no se encuentra el equipo (por ejemplo, cambió de nombre) y el aparato
 *  es de este equipo, se usa este. */
function fuenteDe(item: ItemMapa, fuentes: Map<string, Fuente>, local: Fuente | null): Fuente | null {
  const f = item.equipo ? fuentes.get(item.equipo) : undefined;
  if (f) return f;
  if (!local) return null;
  if (item.tipo === 'aparato' && local.aparatos.dispositivos.some((d) => d.id === item.ref)) return local;
  if (item.tipo === 'pantalla' && local.aparatos.pantallas.some((p) => p.nombre === item.ref)) return local;
  return null;
}

/** Lo que se dibuja de cada cosa del mapa. null si ya no existe (un aparato olvidado) o no hay datos.
 *  `local` va solo en el lugar de este equipo: ahí, lo que no se encuentra por nombre se busca en este equipo. */
export function resolver(
  item: ItemMapa,
  fuentes: Map<string, Fuente>,
  local: Fuente | null,
  casa: EstadoCasa | null = null,
): Pieza | null {
  const base = {
    item,
    bateria: null,
    local: false,
    viejo: false,
    visto: null,
    tipoAparato: null,
    marca: null,
    aparatoId: null,
    hogar: null,
  };
  if (item.tipo === 'hogar') {
    const a = casa?.aparatos.find((x) => x.id === item.ref);
    if (!a) return null;
    const lugar = casa?.habitaciones.find((h) => h.id === a.habitacion)?.nombre;
    return {
      ...base,
      forma: formaDeHogar(a),
      nombre: a.nombre,
      detalle: lugar ? `${tipoDe(a)} · ${lugar}` : tipoDe(a),
      estado: activo(a) ? 'encendido' : 'apagado',
      estadoTexto: estadoEnPalabras(a) || 'Detectado',
      local: true,
      viejo: !a.enLinea,
      hogar: {
        id: a.id,
        clase: a.clase,
        forma: a.forma,
        encendido: a.enLinea && a.estado.encendido === true,
        color: a.clase === 'luz' ? colorDeLuz(a) : null,
        abierto: a.estado.abierto,
        controlable: a.controlable && a.capacidades.encender,
      },
    };
  }
  if (item.tipo === 'deco') {
    if (!esDeco(item.ref)) return null;
    const d = DECOS[item.ref];
    return { ...base, forma: d.forma, nombre: d.nombre, detalle: 'Decoración', estado: null, estadoTexto: '' };
  }
  if (item.tipo === 'equipo') {
    // Si este equipo cambió de nombre (por ejemplo, sin Tailscale), en su propio lugar sigue siendo él.
    const f = fuentes.get(item.ref) ?? local;
    if (!f) return null;
    const plataforma = f.aparatos.plataforma === 'mac' ? 'Mac' : f.aparatos.plataforma === 'windows' ? 'PC con Windows' : 'Equipo';
    return {
      ...base,
      forma: formaDeEquipo(f.aparatos),
      nombre: f.aparatos.equipo || item.ref,
      detalle: f.local ? `${plataforma} · Este equipo` : plataforma,
      estado: f.vivo ? 'encendido' : 'apagado',
      estadoTexto: f.vivo ? 'Encendida, con Nexo' : 'Apagada o sin Nexo',
      local: f.local,
      viejo: !f.vivo,
      visto: f.visto,
    };
  }
  const f = fuenteDe(item, fuentes, local);
  if (!f) return null;
  const donde = f.local ? 'En este equipo' : `En ${f.aparatos.equipo}`;
  if (item.tipo === 'pantalla') {
    const p = f.aparatos.pantallas.find((x) => x.nombre === item.ref);
    const foto = fotoMonitor(item.ref);
    const encendida = f.vivo && !!p;
    return {
      ...base,
      forma: formaDe(foto, 'monitor'),
      nombre: item.ref,
      detalle: `Monitor · ${donde}`,
      estado: encendida ? 'encendido' : 'apagado',
      estadoTexto: !f.vivo
        ? 'Sin datos de ahora'
        : p
          ? p.brillo !== null
            ? `Conectado · brillo ${p.brillo} %`
            : 'Conectado'
          : 'Sin conectar',
      local: f.local,
      viejo: !f.vivo,
      visto: f.visto,
    };
  }
  const a = f.aparatos.dispositivos.find((d) => d.id === item.ref);
  if (!a) return null;
  const d = comoDispositivo(a);
  const foto = modeloDe(d)?.foto ?? null;
  const encendido = f.vivo && a.conectado;
  return {
    ...base,
    forma: formaDe(foto, a.tipo),
    nombre: nombreAparato(a),
    detalle: `${NOMBRE_TIPO[a.tipo]} · ${donde}`,
    estado: encendido ? 'encendido' : 'apagado',
    estadoTexto: !f.vivo ? 'Sin datos de ahora' : a.conectado ? 'Conectado' : 'Sin conectar',
    bateria: encendido ? a.bateria : null,
    local: f.local,
    viejo: !f.vivo,
    visto: f.visto,
    tipoAparato: a.tipo,
    marca: a.fabricante,
    aparatoId: f.local ? a.id : null,
  };
}

// ---------------------------------------------------------------- Ubicar en la escena

/** Una pieza ya ubicada: ancho de su cuadro (% del ancho), dónde apoya (% del alto) y su orden. */
export interface Ubicada extends Pieza {
  ancho: number;
  base: number;
  z: number;
  /** Cuelga del soporte de audífonos (sin sombra de apoyo). */
  colgada: boolean;
  /** Apoya sobre otra cosa (el soporte o la base del monitor). */
  sobre: string | null;
}

const altoDe = (p: { ancho: number }) => p.ancho * PROPORCION;
const cimaDe = (p: Ubicada) => p.base - p.forma.caja[3] * altoDe(p);
const anchoVisible = (p: { ancho: number; forma: Forma }) => (p.forma.caja[2] - p.forma.caja[0]) * p.ancho;

/** El gancho del soporte de audífonos (la barra negra arriba, un poco a la izquierda). */
function gancho(s: Ubicada) {
  return { x: s.item.x + (0.41 - 0.5) * s.ancho, y: cimaDe(s) + 0.13 * altoDe(s) };
}

/** La cara de arriba de la base del monitor. */
const superficie = (b: Ubicada) => cimaDe(b) + 0.4 * altoDe(b);

const esAudifono = (p: Pieza) => p.tipoAparato === 'audifonos' || p.tipoAparato === 'diadema';
const esApoyable = (p: Pieza) => p.item.tipo === 'pantalla' || p.item.tipo === 'equipo';
const esSoporte = (p: Pieza) => p.item.tipo === 'deco' && p.item.ref === 'soporte-audifonos';
const esBase = (p: Pieza) => p.item.tipo === 'deco' && p.item.ref === 'base-monitor';

/** Lo de la pared: su borde de abajo dentro de la pared y el de arriba dentro de la escena. */
export function acotarPared(y: number, ancho: number, f: Forma): number {
  const alto = (f.caja[3] - f.caja[1]) * ancho * PROPORCION;
  return Math.min(PARED - 1, Math.max(1 + alto, y));
}

export const acotarX = (x: number) => Math.min(X_MAX, Math.max(X_MIN, x));

/** La capa más cercana a una altura (% del alto). Arriba de la mesa, el fondo. */
export function capaCercana(y: number): number {
  let mejor = 0;
  BASES.forEach((b, i) => {
    if (Math.abs(b - y) < Math.abs(BASES[mejor] - y)) mejor = i;
  });
  return mejor;
}

export function anchoDe(item: ItemMapa, f: Forma): number {
  return f.tam * (f.pared ? 1 : FACTOR[item.capa] ?? 1) * item.escala;
}

/** Ubica cada pieza: tamaño por capa y escala, los audífonos en su soporte, el monitor en su base, y el orden. */
export function ubicar(piezas: Pieza[]): Ubicada[] {
  const lista: Ubicada[] = piezas.map((p) => {
    const ancho = anchoDe(p.item, p.forma);
    const base = p.forma.pared ? acotarPared(p.item.y ?? 30, ancho, p.forma) : BASES[p.item.capa] ?? BASES[1];
    return { ...p, ancho, base, z: 0, colgada: false, sobre: null };
  });
  const soportes = lista.filter(esSoporte);
  const bases = lista.filter(esBase);
  const ocupados = new Set<string>();
  for (const p of lista) {
    if (p.item.tipo === 'aparato' && esAudifono(p)) {
      const s = soportes.find(
        (s) => !ocupados.has(s.item.id) && s.item.capa === p.item.capa && Math.abs(p.item.x - gancho(s).x) < 4,
      );
      if (s) {
        ocupados.add(s.item.id);
        const g = gancho(s);
        const alto = altoDe(p);
        p.base = g.y - 0.03 * alto + (p.forma.caja[3] - p.forma.caja[1]) * alto;
        p.colgada = true;
        p.sobre = s.item.id;
      }
    } else if (esApoyable(p)) {
      const b = bases.find((b) => b.item.capa === p.item.capa && Math.abs(p.item.x - b.item.x) < anchoVisible(b) / 2);
      if (b) {
        p.base = superficie(b);
        p.sobre = b.item.id;
      }
    }
  }
  // Orden: la pared, y en la mesa del fondo al frente; dentro de cada capa, lo acostado, los soportes, lo demás
  // y lo que va encima de otra cosa.
  const rango = (p: Ubicada) => {
    if (p.forma.pared) return 0;
    const dentro = p.item.ref === 'alfombrilla' ? 0 : esSoporte(p) || esBase(p) ? 1 : p.sobre ? 3 : 2;
    return 10 + p.item.capa * 10 + dentro;
  };
  // El orden va en z-index y la lista queda como llegó: si los nodos se movieran al arrastrar, el navegador
  // soltaría el puntero a mitad de camino.
  const ordenados = [...lista].sort((a, b) => rango(a) - rango(b) || a.base - b.base || a.item.x - b.item.x);
  ordenados.forEach((p, i) => (p.z = i + 1));
  return lista;
}

/** Al soltar algo cerca del soporte o de la base del monitor, se acomoda justo encima. */
export function encajar(item: ItemMapa, ubicadas: Ubicada[]): Partial<ItemMapa> | null {
  const p = ubicadas.find((u) => u.item.id === item.id);
  if (!p) return null;
  if (item.tipo === 'aparato' && esAudifono(p)) {
    const s = ubicadas.find((s) => esSoporte(s) && s.item.capa === item.capa && Math.abs(item.x - gancho(s).x) < 5);
    if (s) return { x: Math.round(gancho(s).x * 10) / 10 };
  }
  if (esApoyable(p)) {
    const b = ubicadas.find((b) => esBase(b) && b.item.capa === item.capa && Math.abs(item.x - b.item.x) < 6);
    if (b) return { x: b.item.x };
  }
  return null;
}

// ---------------------------------------------------------------- Lo de la primera vez

/** El id de un aparato, pantalla o equipo en el mapa: uno por cosa y por lugar. */
export const claveItem = (tipo: TipoItem, equipo: string | null, ref: string) => `${tipo}:${equipo ?? ''}:${ref}`;

let contador = 0;
export const idDeco = () => `deco-${Date.now().toString(36)}-${(contador++).toString(36)}`;

export function nuevoItem(
  tipo: TipoItem,
  ref: string,
  equipo: string | null,
  donde: { x: number; capa?: number; y?: number | null },
): ItemMapa {
  return {
    id: tipo === 'deco' ? idDeco() : claveItem(tipo, equipo, ref),
    tipo,
    ref,
    x: donde.x,
    y: donde.y ?? null,
    capa: donde.capa ?? 1,
    escala: 1,
    equipo: tipo === 'deco' ? null : equipo,
  };
}

const deco = (ref: Deco, x: number, capa: number, y: number | null = null) => nuevoItem('deco', ref, null, { x, capa, y });

/** Dónde queda colgado un audífono en un soporte puesto en `x` (misma capa, escala 1). */
function xEnSoporte(x: number, capa: number) {
  return Math.round((x + (0.41 - 0.5) * DECOS['soporte-audifonos'].forma.tam * FACTOR[capa]) * 10) / 10;
}

const elegir = (lista: Aparato[], patron: RegExp) => lista.find((d) => patron.test(d.nombre)) ?? lista[0];

/** Dónde va cada luz del escritorio la primera vez: la tira en la pared, la barra sobre el monitor. */
function ponerLuces(items: ItemMapa[], luces: AparatoCasa[], hayMonitor: boolean) {
  let focos = 0;
  for (const a of luces) {
    const id = (donde: { x: number; capa?: number; y?: number | null }) => items.push(nuevoItem('hogar', a.id, null, donde));
    if (a.clase === 'enchufe') id({ x: 6, y: 50, capa: 0 });
    else if (a.forma === 'tira') id({ x: 70, y: 22, capa: 0 });
    else if (a.forma === 'barra') id(hayMonitor ? { x: 50, y: 28, capa: 0 } : { x: 48, y: 52, capa: 0 });
    else if (a.forma === 'lampara') id({ x: 20, capa: 0 });
    else if (a.clase === 'luz') id({ x: 36 + 26 * focos++, capa: 0 });
  }
}

/** El mapa de la primera vez, con los aparatos que de verdad hay (y las decoraciones). Con `luces`, las del
 *  escritorio (van en el lugar de este equipo). */
export function porDefecto(lugar: LugarId, equipo: Aparatos | null, luces: AparatoCasa[] = []): LugarMapa {
  const items: ItemMapa[] = [];
  const nombre = equipo?.equipo ?? null;
  const de = (tipo: Tipo[]) => equipo?.dispositivos.filter((d) => tipo.includes(d.tipo)) ?? [];
  const externas = equipo?.pantallas.filter((p) => !p.interna) ?? [];

  if (lugar === 'casa') {
    items.push(deco('cuadro', 30, 0, 34), deco('lampara', 10, 0), deco('planta-grande', 90, 0));
    if (equipo && nombre) {
      if (externas[0]) items.push(nuevoItem('pantalla', externas[0].nombre, nombre, { x: 50, capa: 0 }));
      items.push(nuevoItem('equipo', nombre, nombre, { x: 48, capa: 1 }));
    }
    items.push(deco('soporte-audifonos', 70, 1));
    const audio = de(['diadema', 'audifonos']);
    const primero = elegir(audio, /wh-ch520|sony/i);
    if (primero && nombre) {
      items.push(nuevoItem('aparato', primero.id, nombre, { x: xEnSoporte(70, 1), capa: 1 }));
      const otro = audio.find((d) => d.id !== primero.id);
      if (otro) items.push(nuevoItem('aparato', otro.id, nombre, { x: 84, capa: 1 }));
    }
    const telefono = de(['telefono'])[0];
    if (telefono && nombre) items.push(nuevoItem('aparato', telefono.id, nombre, { x: 30, capa: 2 }));
    items.push(deco('planta-pequena', 14, 2), deco('taza', 91, 2));
    ponerLuces(items, luces, !!externas[0]);
  } else {
    items.push(deco('tira-luz', 80, 0, 34), deco('lampara', 12, 0), deco('base-monitor', 50, 0));
    if (equipo && nombre) {
      if (externas[0]) items.push(nuevoItem('pantalla', externas[0].nombre, nombre, { x: 50, capa: 0 }));
      const laptop = equipo.pantallas.some((p) => p.interna);
      items.push(nuevoItem('equipo', nombre, nombre, laptop ? { x: 22, capa: 1 } : { x: 88, capa: 0 }));
      if (externas[1]) items.push(nuevoItem('pantalla', externas[1].nombre, nombre, { x: 76, capa: 0 }));
    }
    items.push(deco('alfombrilla', 54, 2));
    const teclado = elegir(de(['teclado']), /wave\s*keys/i);
    if (teclado && nombre) items.push(nuevoItem('aparato', teclado.id, nombre, { x: 50, capa: 2 }));
    const mouse = elegir(de(['mouse']), /mx\s*master/i);
    if (mouse && nombre) items.push(nuevoItem('aparato', mouse.id, nombre, { x: 70, capa: 2 }));
    items.push(deco('planta-pequena', 88, 2));
    ponerLuces(items, luces, !!externas[0]);
  }
  // Ids fijos para las decoraciones: el mapa de la primera vez se rehace con cada lectura y no debe parpadear.
  return { items: items.map((it, i) => (it.tipo === 'deco' ? { ...it, id: `deco-${lugar}-${i}` } : it)) };
}

// ---------------------------------------------------------------- Escenas en palabras

/** Los íconos que se pueden elegir para una escena (los mismos de `ICONOS` en escenas.rs). */
export const ICONOS_ESCENA = [
  'audio',
  'remoto',
  'luna',
  'camara',
  'microfono',
  'sol',
  'portatil',
  'pantallas',
  'teclado',
  'rayo',
  'bombilla',
  'reloj',
  'inicio',
  'setup',
] as const;

export type IconoEscena = (typeof ICONOS_ESCENA)[number];
export const iconoEscena = (i: string): IconoEscena =>
  (ICONOS_ESCENA as readonly string[]).includes(i) ? (i as IconoEscena) : 'rayo';

export const PASOS: { tipo: Exclude<TipoAccion, 'desconocida'>; nombre: string; icono: string; soloMac?: boolean }[] = [
  { tipo: 'brillo', nombre: 'Brillo', icono: 'sol' },
  { tipo: 'conectar', nombre: 'Conectar', icono: 'conectar' },
  { tipo: 'desconectar', nombre: 'Desconectar', icono: 'desconectar' },
  { tipo: 'salida', nombre: 'Sonido por', icono: 'audio' },
  { tipo: 'volumen', nombre: 'Volumen', icono: 'volumen' },
  { tipo: 'volumenMicrofono', nombre: 'Micrófono', icono: 'microfono' },
  { tipo: 'modoTrabajo', nombre: 'Modo trabajo', icono: 'remoto', soloMac: true },
  { tipo: 'abrirApp', nombre: 'Abrir una app', icono: 'widgets' },
  { tipo: 'luces', nombre: 'Luces', icono: 'bombilla' },
];

export const pasoDe = (t: TipoAccion) => PASOS.find((p) => p.tipo === t);

/** Un paso nuevo con valores razonables. */
export function accionNueva(tipo: Exclude<TipoAccion, 'desconocida'>, aparato?: { id: string; nombre: string }): Accion {
  switch (tipo) {
    case 'brillo':
      return { tipo, pantalla: 'todas', valor: 70 };
    case 'conectar':
    case 'desconectar':
    case 'salida':
      return { tipo, dispositivo: aparato?.id ?? '', nombre: aparato?.nombre ?? null };
    case 'volumen':
      return { tipo, valor: 50 };
    case 'volumenMicrofono':
      return { tipo, valor: 70 };
    case 'modoTrabajo':
      return { tipo, activo: true };
    case 'abrirApp':
      return { tipo, nombre: '' };
    case 'luces':
      return {
        tipo,
        destino: { tipo: 'todos', clase: 'luz' },
        objetivo: { encendido: true, brillo: 80, temperatura: null, color: null },
        nombre: 'todas las luces',
      };
  }
}

/** El nombre del aparato de un paso: el de la lista de ahora o el que se guardó con el paso. */
export type NombrePor = (id: string, guardado?: string | null) => string;

function aparatoDe(a: Accion, nombrePor: NombrePor) {
  return 'dispositivo' in a ? nombrePor(a.dispositivo, a.nombre) : '';
}

/** El brillo de una pantalla por su nombre ("nombre:Steren 27\" IPS"). */
export const brilloDe = (nombre: string) => `nombre:${nombre}` as const;
/** La pantalla de un paso de brillo, si es una por su nombre. */
export const pantallaDe = (a: Accion): string | null =>
  a.tipo === 'brillo' && a.pantalla.startsWith('nombre:') ? a.pantalla.slice('nombre:'.length) : null;

/** Un paso en corto, para el resumen ("Brillo 70 %"). */
function corto(a: Accion, nombrePor: NombrePor): string {
  switch (a.tipo) {
    case 'brillo': {
      const p = pantallaDe(a);
      if (p) return `Brillo de ${p} ${a.valor} %`;
      return a.pantalla === 'principal' ? `Brillo de la principal ${a.valor} %` : `Brillo ${a.valor} %`;
    }
    case 'conectar':
      return `Conecta ${aparatoDe(a, nombrePor)}`;
    case 'desconectar':
      return `Desconecta ${aparatoDe(a, nombrePor)}`;
    case 'salida':
      return `Sonido por ${aparatoDe(a, nombrePor)}`;
    case 'volumen':
      return `Volumen ${a.valor} %`;
    case 'volumenMicrofono':
      return `Micrófono ${a.valor} %`;
    case 'modoTrabajo':
      return a.activo ? 'Modo trabajo' : 'Sin modo trabajo';
    case 'abrirApp':
      return `Abre ${a.nombre}`;
    case 'luces':
      return lucesCorto(a);
    case 'desconocida':
      return 'Un paso de una versión más nueva';
  }
}

/** Lo que hace una escena en una línea: "Brillo 70 % · WH-CH520 para el sonido · Volumen 40 %".
 *  Conectar y usar para el sonido el mismo aparato se dicen juntos. */
export function resumen(e: Escena, nombrePor: NombrePor): string {
  const partes: string[] = [];
  const acciones = e.acciones;
  for (let i = 0; i < acciones.length; i++) {
    const a = acciones[i];
    const b = acciones[i + 1];
    if (a.tipo === 'conectar' && b?.tipo === 'salida' && b.dispositivo === a.dispositivo) {
      partes.push(`${aparatoDe(a, nombrePor)} para el sonido`);
      i++;
      continue;
    }
    partes.push(corto(a, nombrePor));
  }
  return partes.join(' · ');
}

/** Lo que está haciendo ahora ("Conectando WH-CH520"). */
export function haciendo(a: Accion, nombrePor: NombrePor): string {
  switch (a.tipo) {
    case 'brillo': {
      const p = pantallaDe(a);
      return p ? `Poniendo el brillo de ${p} al ${a.valor} %` : `Poniendo el brillo al ${a.valor} %`;
    }
    case 'conectar':
      return `Conectando ${aparatoDe(a, nombrePor)}`;
    case 'desconectar':
      return `Desconectando ${aparatoDe(a, nombrePor)}`;
    case 'salida':
      return `Pasando el sonido a ${aparatoDe(a, nombrePor)}`;
    case 'volumen':
      return `Poniendo el volumen al ${a.valor} %`;
    case 'volumenMicrofono':
      return `Poniendo el micrófono al ${a.valor} %`;
    case 'modoTrabajo':
      return a.activo ? 'Encendiendo el modo trabajo' : 'Apagando el modo trabajo';
    case 'abrirApp':
      return `Abriendo ${a.nombre}`;
    case 'luces':
      return `Ajustando ${a.nombre || 'las luces'}`;
    case 'desconocida':
      return 'Un paso nuevo';
  }
}

/** Un paso como orden, para decir cuál falló ("Conectar WH-CH520"). */
export function paso(a: Accion, nombrePor: NombrePor): string {
  switch (a.tipo) {
    case 'brillo': {
      const p = pantallaDe(a);
      return p ? `Brillo de ${p} al ${a.valor} %` : `Brillo al ${a.valor} %`;
    }
    case 'conectar':
      return `Conectar ${aparatoDe(a, nombrePor)}`;
    case 'desconectar':
      return `Desconectar ${aparatoDe(a, nombrePor)}`;
    case 'salida':
      return `Usar ${aparatoDe(a, nombrePor)} para el sonido`;
    case 'volumen':
      return `Volumen al ${a.valor} %`;
    case 'volumenMicrofono':
      return `Micrófono al ${a.valor} %`;
    case 'modoTrabajo':
      return a.activo ? 'Encender el modo trabajo' : 'Apagar el modo trabajo';
    case 'abrirApp':
      return `Abrir ${a.nombre}`;
    case 'luces':
      if (!a.objetivo || !a.destino) return 'Luces sin elegir';
      return `${a.objetivo.encendido ? 'Encender' : 'Apagar'} ${a.nombre || 'las luces'}`;
    case 'desconocida':
      return 'Un paso nuevo';
  }
}

/** El paso de luces en corto, para el resumen ("Todas las luces al 85 %", "Apaga el escritorio"). */
function lucesCorto(a: Extract<Accion, { tipo: 'luces' }>): string {
  if (!a.destino || !a.objetivo) return 'Luces sin elegir';
  const quien = a.nombre || 'las luces';
  if (!a.objetivo.encendido) return `Apaga ${quien}`;
  const como = resumenObjetivo(a.objetivo).replace(/^Encender ?/, '');
  const Quien = quien.charAt(0).toUpperCase() + quien.slice(1);
  return como ? `${Quien} ${como}` : `Enciende ${quien}`;
}
