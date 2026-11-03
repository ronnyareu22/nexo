// Fotos de modelos conocidos. La clave es "marca:modelo" tal como la da el Bluetooth.
// Para sumar un aparato: prepara su foto con diseno/herramientas/preparar-imagen.swift,
// guárdala en static/dispositivos/ y agrégala aquí.
import type { Dispositivo } from './tipos';

interface Modelo {
  foto: string;
  nombre: string;
}

const modelos: Record<string, Modelo> = {
  '0x004C:0x2006': { foto: '/dispositivos/beats-solo3.png', nombre: 'Beats Solo³' },
  '0x054C:0x0EAD': { foto: '/dispositivos/sony-wh-ch520.png', nombre: 'Sony WH-CH520' },
};

export function modeloDe(d: Dispositivo): Modelo | null {
  if (!d.vendorId || !d.productId) return null;
  return modelos[`${d.vendorId}:${d.productId}`.toUpperCase().replaceAll('0X', '0x')] ?? null;
}
