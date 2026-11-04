// Fotos reales de los aparatos del dueño (fotos oficiales de cada marca, sin fondo).
// Se reconocen por "marca:modelo" del Bluetooth y, si eso no alcanza, por el nombre que da el aparato.
// Para sumar uno: prepara su foto con diseno/herramientas/quitar-fondo-borde.swift (--lado=400),
// guárdala en static/dispositivos/ y agrégala aquí.
import type { Dispositivo, Pantalla } from './tipos';

interface Modelo {
  foto: string;
  nombre: string;
}

const modelos: Record<string, Modelo> = {
  '0x004C:0x2006': { foto: '/dispositivos/beats-solo3.png', nombre: 'Beats Solo³' },
  '0x054C:0x0EAD': { foto: '/dispositivos/sony-wh-ch520.png', nombre: 'Sony WH-CH520' },
};

// Por nombre: los Logitech cambian de modelo según la conexión (Bluetooth o Logi Bolt), el nombre no.
const porNombre: { patron: RegExp; modelo: Modelo }[] = [
  { patron: /mx\s*master\s*4/i, modelo: { foto: '/dispositivos/mx-master-4.png', nombre: 'Logitech MX Master 4' } },
  { patron: /wave\s*keys/i, modelo: { foto: '/dispositivos/wave-keys.png', nombre: 'Logitech Wave Keys' } },
];

// Monitores conocidos, por el nombre que da la pantalla (EDID). El Steren de 27" IPS (MNT-027) aún no se ha
// visto conectado: cuando se vea su nombre real, se ajusta aquí.
const monitores: { patron: RegExp; foto: string; nombre: string }[] = [
  { patron: /steren|mnt[-\s]?0?27/i, foto: '/dispositivos/steren-27.png', nombre: 'Steren 27" IPS' },
];

const enMac = typeof navigator !== 'undefined' && /Mac/i.test(navigator.userAgent);

/** Foto de una pantalla: la MacBook Pro del dueño para la integrada de la Mac; el monitor por su nombre. */
export function fotoDePantalla(p: Pantalla): string | null {
  if (p.interna) return enMac ? '/dispositivos/macbook-pro-14.png' : null;
  return monitores.find((m) => m.patron.test(p.nombre))?.foto ?? null;
}

export function modeloDe(d: Dispositivo): Modelo | null {
  if (d.vendorId && d.productId) {
    const exacto = modelos[`${d.vendorId}:${d.productId}`.toUpperCase().replaceAll('0X', '0x')];
    if (exacto) return exacto;
  }
  return porNombre.find((m) => m.patron.test(d.nombre))?.modelo ?? null;
}
