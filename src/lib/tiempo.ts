// Fechas y duraciones en español claro, y la estimación de batería.

export const ahora = () => Date.now() / 1000;

export function inicioDeHoy(): number {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d.getTime() / 1000;
}

/** "45 min", "1 h 20 min", "3 h". */
export function duracion(segundos: number): string {
  if (segundos < 60) return 'menos de 1 min';
  const h = Math.floor(segundos / 3600);
  const m = Math.round((segundos % 3600) / 60);
  if (h === 0) return `${m} min`;
  return m ? `${h} h ${m} min` : `${h} h`;
}

const horaDe = (d: Date) => d.toLocaleTimeString('es', { hour: 'numeric', minute: '2-digit' });

/** "hace un momento", "hace 12 min", "hoy a las 14:32", "ayer a las 21:10", "lunes a las 9:05", "12 oct". */
export function cuando(t: number): string {
  const diferencia = ahora() - t;
  if (diferencia < 60) return 'hace un momento';
  if (diferencia < 3600) return `hace ${Math.round(diferencia / 60)} min`;
  const d = new Date(t * 1000);
  const hoy = inicioDeHoy();
  if (t >= hoy) return `hoy a las ${horaDe(d)}`;
  if (t >= hoy - 86400) return `ayer a las ${horaDe(d)}`;
  if (t >= hoy - 6 * 86400) return `${d.toLocaleDateString('es', { weekday: 'long' })} a las ${horaDe(d)}`;
  return d.toLocaleDateString('es', { day: 'numeric', month: 'short' });
}

/** Solo la hora: "14:32". */
export const hora = (t: number) => horaDe(new Date(t * 1000));

/** Segundos de uso desde `desde` hasta ahora, sumando las sesiones. */
export function usoDesde(sesiones: [number, number | null][], desde: number): number {
  const fin = ahora();
  return sesiones.reduce((total, [inicio, termino]) => {
    const a = Math.max(inicio, desde);
    const b = Math.min(termino ?? fin, fin);
    return total + Math.max(0, b - a);
  }, 0);
}

/**
 * Horas de batería que quedan, según cómo bajó en las últimas 3 horas (recta de mínimos cuadrados).
 * Solo si hay al menos 30 min de datos y bajó 2 % o más; si no, null.
 */
export function horasRestantes(muestras: [number, number][]): number | null {
  const desde = ahora() - 3 * 3600;
  const m = muestras.filter(([t]) => t >= desde);
  if (m.length < 2) return null;
  const lapso = m[m.length - 1][0] - m[0][0];
  if (lapso < 1800 || m[0][1] - m[m.length - 1][1] < 2) return null;
  const n = m.length;
  const tx = m.reduce((s, [t]) => s + t, 0) / n;
  const ny = m.reduce((s, [, v]) => s + v, 0) / n;
  const pendiente =
    m.reduce((s, [t, v]) => s + (t - tx) * (v - ny), 0) / m.reduce((s, [t]) => s + (t - tx) ** 2, 0);
  if (!(pendiente < 0)) return null;
  return m[n - 1][1] / (-pendiente * 3600);
}

/** Cuándo terminó la última sesión cerrada (la última vez que se usó); null si no hay registro. */
export function ultimaVez(sesiones: [number, number | null][]): number | null {
  for (let i = sesiones.length - 1; i >= 0; i--) {
    const fin = sesiones[i][1];
    if (fin !== null) return fin;
  }
  return null;
}
