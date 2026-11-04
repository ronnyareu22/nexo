// Estado de Nexo Drop compartido entre la vista Drop y la capa que aparece al arrastrar archivos sobre la ventana:
// los destinos (tus otros equipos con Nexo), cuál está elegido (se recuerda en este equipo) y si hay un arrastre.
import { leerDestinosDrop, pruebaDrop } from './api-sistema';
import type { DestinoDrop } from './tipos-sistema';

const CLAVE_DESTINO = 'nexo-drop-destino';

function destinoGuardado(): string | null {
  try {
    return localStorage.getItem(CLAVE_DESTINO);
  } catch {
    return null;
  }
}

export const drop = $state({
  destinos: [] as DestinoDrop[],
  /** Ya respondió Rust al menos una vez (antes se ven siluetas). */
  leidos: false,
  /** La dirección del destino elegido. */
  elegido: destinoGuardado(),
  /** Hay archivos arrastrándose sobre la ventana. */
  arrastrando: pruebaDrop === 'capa',
  /** Cuántos elementos se arrastran. */
  cantidad: pruebaDrop === 'capa' ? 3 : 0,
});

let ultimaLectura = 0;
let enCamino: Promise<void> | null = null;

/** Relee los destinos. Sin `forzar`, si se leyeron hace menos de 3 s, no vuelve a preguntar. */
export function leerDestinos(forzar = false): Promise<void> {
  if (enCamino) return enCamino;
  if (!forzar && drop.leidos && Date.now() - ultimaLectura < 3000) return Promise.resolve();
  enCamino = leerDestinosDrop()
    .then((lista) => {
      drop.destinos = lista;
      ultimaLectura = Date.now();
    })
    .catch(() => {
      // Si falla, se queda con lo último que supo.
    })
    .finally(() => {
      drop.leidos = true;
      enCamino = null;
    });
  return enCamino;
}

/** Elige el destino (y lo recuerda en este equipo). */
export function elegirDestino(ip: string) {
  drop.elegido = ip;
  try {
    localStorage.setItem(CLAVE_DESTINO, ip);
  } catch {
    // Sin almacenamiento: se recuerda hasta cerrar el panel.
  }
}

/** El destino en uso: el elegido si sigue en la lista; si no, el primero (los que responden van primero). */
export function destinoActual(): DestinoDrop | null {
  return drop.destinos.find((d) => d.ip === drop.elegido) ?? drop.destinos[0] ?? null;
}
