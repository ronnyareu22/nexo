// Llamadas a Rust para Mi setup: los aparatos de tus equipos, el mapa de la casa y la oficina, y las escenas.
// Fuera de la app (por ejemplo `pnpm preview` en el navegador) usa datos de ejemplo para ver el diseño:
// `?seccion=setup` muestra todo; con `&setup=editar` abre el mapa para acomodarlo, `&setup=apagada` muestra el
// PC de la oficina apagado (con lo último que se vio), `&setup=sin` sin otro equipo, `&setup=fallo` una escena
// a la que le falla un paso, `&setup=escena` abre el editor de la primera escena y `&setup=aparatos`, además,
// la lista para agregar un dispositivo.
import { invoke } from '@tauri-apps/api/core';
import { pruebaCasa } from './api-hogar';
import type { Objetivo } from './tipos-hogar';
import type {
  Accion,
  AvanceEscena,
  Escena,
  EstadoEscenas,
  EstadoSetup,
  FinEscena,
  MapaSetup,
  ResultadoAccion,
} from './tipos-setup';

export const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** Para revisar el diseño fuera de la app (ver arriba). */
export const pruebaSetup: string | null =
  !enApp && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('setup') : null;

/** Este equipo y los otros equipos tuyos con lo que tiene cada uno (los otros se recuerdan 10 s). */
export const leerSetup = (): Promise<EstadoSetup> =>
  enApp ? invoke<EstadoSetup>('setup_estado') : Promise.resolve(setupEjemplo());

/** Lo que acomodaste en la casa y la oficina. Un lugar en null todavía no se acomodó. */
export const leerMapa = (): Promise<MapaSetup> =>
  enApp ? invoke<MapaSetup>('setup_mapa') : Promise.resolve(structuredClone(mapaEjemplo));

/** Guarda el mapa y devuelve cómo quedó (Rust lo revisa: números dentro de la escena, sin repetidos). */
export const guardarMapa = (mapa: MapaSetup): Promise<MapaSetup> =>
  enApp ? invoke<MapaSetup>('setup_guardar_mapa', { mapa }) : guardarMapaEjemplo(mapa);

/** Las escenas (la primera vez, las cuatro de fábrica) y la última que se activó. */
export const leerEscenas = (): Promise<EstadoEscenas> =>
  enApp ? invoke<EstadoEscenas>('escenas') : Promise.resolve(copiaEscenas());

/** Guarda una escena nueva (id vacío) o cambia la que tiene ese id. Puede fallar con el motivo en palabras. */
export const guardarEscena = (escena: Escena): Promise<EstadoEscenas> =>
  enApp ? invoke<EstadoEscenas>('guardar_escena', { escena }) : guardarEscenaEjemplo(escena);

export const borrarEscena = (id: string): Promise<EstadoEscenas> =>
  enApp ? invoke<EstadoEscenas>('borrar_escena', { id }) : borrarEscenaEjemplo(id);

/** Corre los pasos en orden y devuelve cómo salió cada uno. El avance llega por `alAvanceEscena`. */
export const activarEscena = (id: string): Promise<ResultadoAccion[]> =>
  enApp ? invoke<ResultadoAccion[]>('activar_escena', { id }) : activarEjemplo(id);

/** Escucha un aviso de Rust. Devuelve con qué dejar de escuchar. */
function escuchar<T>(evento: string, alLlegar: (valor: T) => void): () => void {
  const quitar = import('@tauri-apps/api/event')
    .then(({ listen }) => listen<T>(evento, (e) => alLlegar(e.payload)))
    .catch(() => () => {});
  return () => {
    quitar.then((f) => f());
  };
}

/** Empieza un paso de una escena (también si se activó desde el ícono de la barra de menús o la bandeja). */
export function alAvanceEscena(alLlegar: (a: AvanceEscena) => void): () => void {
  if (enApp) return escuchar('escena-progreso', alLlegar);
  oyentesAvance.add(alLlegar);
  return () => oyentesAvance.delete(alLlegar);
}

/** Terminó una escena. */
export function alFinEscena(alLlegar: (f: FinEscena) => void): () => void {
  if (enApp) return escuchar('escena-activa', alLlegar);
  oyentesFin.add(alLlegar);
  return () => oyentesFin.delete(alLlegar);
}

// --- Datos de ejemplo (solo fuera de la app) ---

const haceHoras = (h: number) => Math.round(Date.now() / 1000 - h * 3600);
const SONY = 'E8:9E:13:61:CE:B7';

function setupEjemplo(): EstadoSetup {
  const apagada = pruebaSetup === 'apagada';
  return {
    local: {
      equipo: 'demo-sige',
      plataforma: 'mac',
      dispositivos: [
        {
          id: SONY, nombre: 'WH-CH520', alias: null, tipo: 'diadema', conectado: true, bateria: 100,
          fabricante: 'Sony', vendorId: '0x054C', productId: '0x0EAD',
        },
        {
          id: 'AC:BC:B5:02:84:E1', nombre: 'Ronny Beats Solo³', alias: 'Beats del estudio', tipo: 'audifonos',
          conectado: false, bateria: null, fabricante: 'Beats', vendorId: '0x004C', productId: '0x2006',
        },
        {
          id: 'EC:28:D3:02:FE:AB', nombre: 'iPhone de Ronny', alias: null, tipo: 'telefono', conectado: false,
          bateria: null, fabricante: 'Apple', vendorId: null, productId: null,
        },
      ],
      pantallas: [{ id: 1, nombre: 'Pantalla integrada', interna: true, brillo: 40 }],
    },
    remotos:
      pruebaSetup === 'sin'
        ? []
        : [
            {
              nombre: 'innotech1017',
              ip: '100.118.228.87',
              plataforma: 'windows',
              enLinea: !apagada,
              tailscaleEnLinea: !apagada,
              alDia: !apagada,
              visto: apagada ? haceHoras(2) : Math.round(Date.now() / 1000),
              error: null,
              aparatos: {
                equipo: 'innotech1017',
                plataforma: 'windows',
                dispositivos: [
                  {
                    id: 'D4:12:7A:5C:0B:41', nombre: 'MX Master 4', alias: null, tipo: 'mouse', conectado: true,
                    bateria: 85, fabricante: 'Logitech', vendorId: '0x046D', productId: null,
                  },
                  {
                    id: 'D4:12:7A:5C:0B:42', nombre: 'Wave Keys', alias: null, tipo: 'teclado', conectado: true,
                    bateria: 70, fabricante: 'Logitech', vendorId: '0x046D', productId: null,
                  },
                ],
                pantallas: [{ id: 2, nombre: 'Steren 27" IPS', interna: false, brillo: 70 }],
              },
            },
          ],
  };
}

let mapaEjemplo: MapaSetup = { casa: null, oficina: null };

function guardarMapaEjemplo(mapa: MapaSetup): Promise<MapaSetup> {
  mapaEjemplo = structuredClone(mapa);
  return new Promise((listo) => setTimeout(() => listo(structuredClone(mapaEjemplo)), 300));
}

/** El paso de luces que Rust suma a las escenas de fábrica al conectar las luces (escenas.rs, `sumar_luces`). */
const luces = (objetivo: Objetivo): Accion[] =>
  pruebaCasa === 'nada' ? [] : [{ tipo: 'luces', destino: { tipo: 'todos', clase: 'luz' }, objetivo, nombre: 'todas las luces' }];

// Las mismas cuatro que arma Rust la primera vez en la Mac (escenas.rs, `por_defecto`), con sus luces.
const escenasEjemplo: EstadoEscenas = {
  escenas: [
    {
      id: 'estudio', nombre: 'Modo estudio', icono: 'audio',
      acciones: [
        { tipo: 'brillo', pantalla: 'todas', valor: 70 },
        { tipo: 'conectar', dispositivo: SONY, nombre: 'WH-CH520' },
        { tipo: 'salida', dispositivo: SONY, nombre: 'WH-CH520' },
        { tipo: 'volumen', valor: 40 },
        ...luces({ encendido: true, brillo: 85, temperatura: 70, color: null }),
      ],
    },
    {
      id: 'remoto', nombre: 'Modo remoto', icono: 'remoto',
      acciones: [
        { tipo: 'modoTrabajo', activo: true },
        { tipo: 'brillo', pantalla: 'todas', valor: 0 },
        ...luces({ encendido: false, brillo: null, temperatura: null, color: null }),
      ],
    },
    {
      id: 'noche', nombre: 'Modo noche', icono: 'luna',
      acciones: [
        { tipo: 'brillo', pantalla: 'todas', valor: 15 },
        { tipo: 'volumen', valor: 20 },
        ...luces({ encendido: true, brillo: 20, temperatura: 0, color: null }),
      ],
    },
    {
      id: 'reunion', nombre: 'Modo reunión', icono: 'camara',
      acciones: [
        { tipo: 'conectar', dispositivo: SONY, nombre: 'WH-CH520' },
        { tipo: 'salida', dispositivo: SONY, nombre: 'WH-CH520' },
        { tipo: 'volumenMicrofono', valor: 80 },
        { tipo: 'volumen', valor: 60 },
        ...luces({ encendido: true, brillo: 90, temperatura: 50, color: null }),
      ],
    },
  ],
  ultima: null,
};

const copiaEscenas = (): EstadoEscenas => structuredClone(escenasEjemplo);

function guardarEscenaEjemplo(escena: Escena): Promise<EstadoEscenas> {
  const nombre = escena.nombre.trim();
  if (!nombre) return Promise.reject('Ponle un nombre a la escena.');
  const acciones = escena.acciones.filter((a) => a.tipo !== 'desconocida');
  if (!acciones.length) return Promise.reject('Agrega al menos un paso.');
  const limpia: Escena = { ...structuredClone(escena), nombre, acciones: structuredClone(acciones) };
  const i = escenasEjemplo.escenas.findIndex((e) => e.id && e.id === escena.id);
  if (i >= 0) escenasEjemplo.escenas[i] = limpia;
  else escenasEjemplo.escenas.push({ ...limpia, id: `e${Date.now()}` });
  return Promise.resolve(copiaEscenas());
}

function borrarEscenaEjemplo(id: string): Promise<EstadoEscenas> {
  escenasEjemplo.escenas = escenasEjemplo.escenas.filter((e) => e.id !== id);
  if (escenasEjemplo.ultima?.id === id) escenasEjemplo.ultima = null;
  return Promise.resolve(copiaEscenas());
}

const oyentesAvance = new Set<(a: AvanceEscena) => void>();
const oyentesFin = new Set<(f: FinEscena) => void>();
let activando = false;

/** Imita a Rust: un paso cada 0,7 s, con avisos. Con `?setup=fallo`, conectar falla. */
async function activarEjemplo(id: string): Promise<ResultadoAccion[]> {
  const escena = escenasEjemplo.escenas.find((e) => e.id === id);
  if (!escena) throw 'Esa escena ya no existe.';
  if (activando) throw 'Ya se está activando una escena; espera a que termine.';
  activando = true;
  const resultados: ResultadoAccion[] = [];
  try {
    for (const [indice, accion] of escena.acciones.entries()) {
      for (const o of oyentesAvance) o({ id, indice, total: escena.acciones.length, accion });
      await new Promise((r) => setTimeout(r, 700));
      resultados.push(resultadoEjemplo(accion));
    }
  } finally {
    activando = false;
  }
  escenasEjemplo.ultima = { id, momento: Math.round(Date.now() / 1000), ok: resultados.every((r) => r.ok) };
  for (const o of oyentesFin) o({ id, resultados });
  return resultados;
}

function resultadoEjemplo(accion: Accion): ResultadoAccion {
  if (accion.tipo === 'luces' && (!accion.destino || !accion.objetivo))
    return { accion, ok: true, error: 'Elige en el editor qué luces y cómo deben quedar.', omitida: true };
  if (accion.tipo === 'luces' && pruebaCasa === 'nada')
    return { accion, ok: false, error: 'Conecta tus luces en Ajustes › Casa para que este paso corra.', omitida: false };
  if (accion.tipo === 'conectar' && pruebaSetup === 'fallo')
    return { accion, ok: false, error: 'No se pudo conectar. Revisa que esté encendido y cerca.', omitida: false };
  return { accion, ok: true, error: null, omitida: false };
}
