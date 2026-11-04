// Los códigos VCP (MCCS) que Nexo muestra en "Ajustes del monitor", con sus nombres en español.

export const VCP = {
  fabrica: 0x04,
  contraste: 0x12,
  color: 0x14,
  rojo: 0x16,
  verde: 0x18,
  azul: 0x1a,
  entrada: 0x60,
  volumen: 0x62,
  nitidez: 0x87,
  silencio: 0x8d,
  modoImagen: 0xdc,
} as const;

/** Silencio (0x8D): 1 = silenciado, 2 = con sonido. */
export const SILENCIADO = 1;
export const CON_SONIDO = 2;

const hex = (v: number) => v.toString(16).toUpperCase().padStart(2, '0');

/** Temperatura de cada preset de color (0x14), para saber cuál es el más cálido. */
export const KELVIN: Record<number, number> = {
  0x03: 4000,
  0x04: 5000,
  0x05: 6500,
  0x06: 7500,
  0x07: 8200,
  0x08: 9300,
  0x09: 10000,
  0x0a: 11500,
};

/** Los modos "Usuario": ahí se ajustan rojo, verde y azul a mano. */
export const MODOS_USUARIO = [0x0b, 0x0c, 0x0d];

const COLORES: Record<number, string> = {
  0x01: 'sRGB',
  0x02: 'Nativo del monitor',
  0x03: 'Cálido (4000 K)',
  0x04: 'Cálido (5000 K)',
  0x05: 'Natural (6500 K)',
  0x06: '7500 K',
  0x07: '8200 K',
  0x08: 'Frío (9300 K)',
  0x09: 'Frío (10 000 K)',
  0x0a: 'Frío (11 500 K)',
  0x0b: 'Usuario',
  0x0c: 'Usuario 2',
  0x0d: 'Usuario 3',
};

const ENTRADAS: Record<number, string> = {
  0x01: 'VGA',
  0x02: 'VGA 2',
  0x03: 'DVI',
  0x04: 'DVI 2',
  0x0f: 'DisplayPort 1',
  0x10: 'DisplayPort 2',
  0x11: 'HDMI 1',
  0x12: 'HDMI 2',
};

const MODOS_IMAGEN: Record<number, string> = {
  0x00: 'Estándar',
  0x01: 'Productividad',
  0x02: 'Mixto',
  0x03: 'Cine',
  0x04: 'Personalizado',
  0x05: 'Juegos',
  0x06: 'Deportes',
  0x07: 'Profesional',
  0x08: 'Estándar con ahorro medio',
  0x09: 'Estándar con ahorro alto',
  0x0a: 'Demostración',
  0xf0: 'Contraste dinámico',
};

export const nombreColor = (v: number) => COLORES[v] ?? `Modo de color ${hex(v)}`;
export const nombreEntrada = (v: number) => ENTRADAS[v] ?? `Entrada ${hex(v)}`;
export const nombreModoImagen = (v: number) => MODOS_IMAGEN[v] ?? `Modo ${hex(v)}`;

/** El preset más cálido que ofrece el monitor (para "Luz cálida"), o null si no tiene ninguno con temperatura. */
export function masCalido(valores: number[]): number | null {
  const conK = valores.filter((v) => v in KELVIN);
  if (!conK.length) return null;
  return conK.reduce((a, b) => (KELVIN[b] < KELVIN[a] ? b : a));
}

/** Cómo se ve el valor de un control continuo: en % si va de 0 a 100 o es grande; "3 de 10" si tiene pocos pasos. */
export function textoContinuo(actual: number, maximo: number): string {
  if (maximo <= 0) return String(actual);
  if (maximo === 100 || maximo > 20) return `${Math.round((actual / maximo) * 100)} %`;
  return `${actual} de ${maximo}`;
}
