// Llamadas a Rust para Analítica: el estado de este equipo y el de tus otros equipos por XDrop.
// Fuera de la app (por ejemplo `pnpm preview`) usa datos de ejemplo: eres el PC de la oficina mirando tu Mac.
// `?analitica=mac` hace de la Mac mirando el PC, `?analitica=apagada` muestra la Mac sin responder y
// `?analitica=sin` deja solo este equipo.
import { invoke } from '@tauri-apps/api/core';
import type { Analitica, EquipoAnalitica, HistorialAnalitica, PlataformaEquipo } from './tipos-analitica';

const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** Para revisar el diseño fuera de la app (ver arriba). */
export const pruebaAnalitica: string | null =
  !enApp && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('analitica') : null;

/** La analítica de este equipo. Mientras se pida, Nexo sigue midiendo cada 2 s; si no, se detiene solo. */
export const leerAnaliticaLocal = (): Promise<Analitica> =>
  enApp ? invoke<Analitica>('analitica_local') : Promise.resolve(ejemplo(propia()));

/** La analítica de otro equipo tuyo (por XDrop). Falla con el motivo en palabras si no responde. */
export const leerAnaliticaRemota = (ip: string): Promise<Analitica> =>
  enApp
    ? invoke<Analitica>('analitica_remota', { ip })
    : pruebaAnalitica === 'apagada'
      ? new Promise((_, no) => setTimeout(() => no('No respondió.'), 400))
      : Promise.resolve(ejemplo(otra()));

/** Este equipo y tus otros equipos con Nexo (la búsqueda de XDrop, recordada 8 s). */
export const leerEquiposAnalitica = (): Promise<EquipoAnalitica[]> =>
  enApp ? invoke<EquipoAnalitica[]>('analitica_equipos') : Promise.resolve(equiposEjemplo());

// --- Datos de ejemplo (solo fuera de la app) ---

const MAC = { nombre: 'demo-sige', ip: '100.88.188.99', plataforma: 'mac' as const };
const PC = { nombre: 'innotech1017', ip: '100.118.228.87', plataforma: 'windows' as const };
const enMac = () => pruebaAnalitica === 'mac';
const propia = () => (enMac() ? MAC : PC);
const otra = () => (enMac() ? PC : MAC);

function equiposEjemplo(): EquipoAnalitica[] {
  const yo = propia();
  const el = otra();
  const lista: EquipoAnalitica[] = [{ nombre: yo.nombre, ip: null, plataforma: yo.plataforma, enLinea: true, propio: true }];
  if (pruebaAnalitica !== 'sin')
    lista.push({ nombre: el.nombre, ip: el.ip, plataforma: el.plataforma, enLinea: true, propio: false });
  return lista;
}

const GB = 1024 ** 3;
const CADA = 2000;

/** Un número entre 0 y 1 que siempre es el mismo para la misma muestra. */
function azar(k: number, semilla: number): number {
  const x = Math.sin(k * 12.9898 + semilla * 78.233) * 43758.5453;
  return x - Math.floor(x);
}

/** 5 minutos de muestras que avanzan con el reloj: cada 2 s entra una nueva. */
function historialEjemplo(plataforma: PlataformaEquipo, fin: number): HistorialAnalitica {
  const ultimo = Math.floor(fin / CADA);
  const momentos: number[] = [];
  const cpu: number[] = [];
  const memoria: number[] = [];
  const bajada: (number | null)[] = [];
  const subida: (number | null)[] = [];
  for (let k = ultimo - 149; k <= ultimo; k++) {
    momentos.push(k * CADA);
    if (plataforma === 'mac') {
      const pico = k % 41 < 3 ? 22 : 0;
      cpu.push(Math.round((9 + 4 * Math.sin(k / 9) + 6 * azar(k, 1) + pico) * 10) / 10);
      memoria.push(Math.round((62.5 + 1.2 * Math.sin(k / 23) + 0.5 * azar(k, 2)) * 10) / 10);
      // La Mac manda su pantalla por RustDesk: sube mucho más de lo que baja.
      subida.push(Math.round(1_350_000 + 450_000 * Math.sin(k / 6) + 380_000 * azar(k, 3)));
      bajada.push(Math.round(70_000 + 60_000 * azar(k, 4) + (k % 29 < 2 ? 900_000 : 0)));
    } else {
      cpu.push(Math.round((17 + 5 * Math.sin(k / 7) + 7 * azar(k, 5)) * 10) / 10);
      memoria.push(Math.round((71 + 1.5 * Math.sin(k / 19) + 0.6 * azar(k, 6)) * 10) / 10);
      bajada.push(null);
      subida.push(null);
    }
  }
  return { cada: CADA, momentos, cpu, memoria, bajada, subida };
}

function ejemplo(equipo: { nombre: string; plataforma: PlataformaEquipo }): Analitica {
  const momento = Date.now();
  const h = historialEjemplo(equipo.plataforma, momento);
  const cpuAhora = h.cpu[h.cpu.length - 1] ?? 0;
  const memAhora = h.memoria[h.memoria.length - 1] ?? 0;
  if (equipo.plataforma === 'mac') {
    const total = 24 * GB;
    const usada = Math.round((total * memAhora) / 100);
    const k = Math.floor(momento / CADA);
    return {
      equipo: equipo.nombre,
      plataforma: 'mac',
      momento,
      cpu: {
        total: cpuAhora,
        cantidad: 15,
        nucleos: Array.from({ length: 15 }, (_, i) => Math.round(azar(k, 10 + i) * (i < 5 ? 48 : 22) * 10) / 10),
      },
      memoria: {
        total,
        usada,
        apps: usada - 5.4 * GB,
        cableada: 3.1 * GB,
        comprimida: 2.3 * GB,
        cache: 5.6 * GB,
        libre: total - usada - 5.6 * GB,
        swapUsada: 0,
        swapTotal: 0,
        presion: 'normal',
      },
      temperatura: { estado: 'normal', bateria: 31.4 },
      bateria: { nivel: 80, cargando: false, conectada: true, completa: false, minutos: null, ciclos: 56, salud: 96 },
      disco: { total: 994_662_584_320, libre: 412_318_420_992 },
      red: {
        bajada: h.bajada[h.bajada.length - 1] ?? 0,
        subida: h.subida[h.subida.length - 1] ?? 0,
        tailscale: {
          bajada: Math.round((h.bajada[h.bajada.length - 1] ?? 0) * 0.82),
          subida: Math.round((h.subida[h.subida.length - 1] ?? 0) * 0.97),
        },
      },
      encendida: 9 * 3600 + 14 * 60 + Math.round((momento / 1000) % 60),
      procesos: {
        soloTuyos: true,
        cpu: [
          { nombre: 'RustDesk', cpu: 6.1, memoria: 812 * 1024 ** 2, procesos: 3 },
          { nombre: 'Google Chrome', cpu: 2.4, memoria: 3.2 * GB, procesos: 18 },
          { nombre: 'Antigravity IDE', cpu: 1.3, memoria: 2.1 * GB, procesos: 9 },
          { nombre: 'node', cpu: 0.9, memoria: 296 * 1024 ** 2, procesos: 2 },
          { nombre: 'Nexo', cpu: 0.4, memoria: 98 * 1024 ** 2, procesos: 1 },
          { nombre: 'Terminal', cpu: 0.2, memoria: 142 * 1024 ** 2, procesos: 1 },
        ],
        memoria: [
          { nombre: 'Google Chrome', cpu: 2.4, memoria: 3.2 * GB, procesos: 18 },
          { nombre: 'Antigravity IDE', cpu: 1.3, memoria: 2.1 * GB, procesos: 9 },
          { nombre: 'RustDesk', cpu: 6.1, memoria: 812 * 1024 ** 2, procesos: 3 },
          { nombre: 'Mail', cpu: 0.1, memoria: 388 * 1024 ** 2, procesos: 2 },
          { nombre: 'node', cpu: 0.9, memoria: 296 * 1024 ** 2, procesos: 2 },
          { nombre: 'Terminal', cpu: 0.2, memoria: 142 * 1024 ** 2, procesos: 1 },
        ],
      },
      sesion: {
        rustdesk: true,
        activa: true,
        directa: true,
        rele: false,
        equipo: PC.nombre,
        pantallaVirtual: { activa: true, ancho: 1920, alto: 1080 },
        xdrop: [
          {
            direccion: 'recibido',
            equipo: PC.nombre,
            nombre: 'Informe mensual de ventas.pdf',
            total: 1,
            avance: 0.62,
            velocidad: 4_800_000,
          },
        ],
        sinReposo: true,
        modoTrabajo: true,
      },
      historial: h,
    };
  }
  const total = 16 * GB;
  const usada = Math.round((total * memAhora) / 100);
  return {
    equipo: equipo.nombre,
    plataforma: 'windows',
    momento,
    cpu: { total: cpuAhora, cantidad: 8, nucleos: [] },
    memoria: {
      total,
      usada,
      apps: null,
      cableada: null,
      comprimida: null,
      cache: null,
      libre: total - usada,
      swapUsada: null,
      swapTotal: null,
      presion: null,
    },
    temperatura: { estado: null, bateria: null },
    bateria: { nivel: 64, cargando: true, conectada: true, completa: false, minutos: null, ciclos: null, salud: null },
    disco: { total: 511_101_964_288, libre: 138_412_384_256 },
    red: null,
    encendida: 2 * 86400 + 3 * 3600 + 20 * 60,
    procesos: {
      soloTuyos: false,
      cpu: [
        { nombre: 'rustdesk', cpu: 7.8, memoria: 402 * 1024 ** 2, procesos: 2 },
        { nombre: 'chrome', cpu: 4.2, memoria: 2.6 * GB, procesos: 24 },
        { nombre: 'ms-teams', cpu: 2.9, memoria: 1.1 * GB, procesos: 6 },
        { nombre: 'MsMpEng', cpu: 1.4, memoria: 310 * 1024 ** 2, procesos: 1 },
        { nombre: 'explorer', cpu: 0.6, memoria: 188 * 1024 ** 2, procesos: 1 },
        { nombre: 'Nexo', cpu: 0.3, memoria: 74 * 1024 ** 2, procesos: 1 },
      ],
      memoria: [
        { nombre: 'chrome', cpu: 4.2, memoria: 2.6 * GB, procesos: 24 },
        { nombre: 'ms-teams', cpu: 2.9, memoria: 1.1 * GB, procesos: 6 },
        { nombre: 'Code', cpu: 0.2, memoria: 840 * 1024 ** 2, procesos: 11 },
        { nombre: 'rustdesk', cpu: 7.8, memoria: 402 * 1024 ** 2, procesos: 2 },
        { nombre: 'MsMpEng', cpu: 1.4, memoria: 310 * 1024 ** 2, procesos: 1 },
        { nombre: 'explorer', cpu: 0.6, memoria: 188 * 1024 ** 2, procesos: 1 },
      ],
    },
    sesion: {
      rustdesk: true,
      activa: false,
      directa: false,
      rele: false,
      equipo: null,
      pantallaVirtual: null,
      xdrop: [],
      sinReposo: false,
      modoTrabajo: null,
    },
    historial: h,
  };
}
