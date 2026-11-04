// El perfil de trabajo remoto, compartido por las vistas: Tailscale, tus otros equipos, cuáles tienen Nexo y si
// Remoto está encendido (Nexo encontró tu Mac o tu PC, o se eligió "Siempre" en Ajustes).
// Se lee al abrir una vista que lo usa y, solo con la guía a la vista, cada 10 s (`fresco`). Rust avisa con
// "equipos-cambio" cuando encuentra otro equipo o cambia el modo.
import { alCambiarEquipos, leerEquipos } from './api-equipos';
import type { EquipoTailscale, EstadoEquipos, OtroEquipo } from './tipos-sistema';

class Perfil {
  /** null hasta la primera respuesta de Rust. */
  estado = $state<EstadoEquipos | null>(null);

  get leido(): boolean {
    return this.estado !== null;
  }
  /** El perfil de trabajo remoto está encendido. */
  get activo(): boolean {
    return this.estado?.activo ?? false;
  }
  /** Tu Mac (en Windows) o tu PC (en la Mac), encontrada ahora o recordada. */
  get otro(): OtroEquipo | null {
    return this.estado?.otro ?? null;
  }
  get equipos(): EquipoTailscale[] {
    return this.estado?.equipos ?? [];
  }
  get tailscale() {
    return this.estado?.tailscale ?? null;
  }
  /** Algún otro equipo tuyo tiene Nexo abierto ahora. */
  get conNexo(): boolean {
    return this.equipos.some((e) => e.nexo);
  }

  /** Vuelve a leer. Varios pedidos a la vez comparten la misma lectura. */
  recargar(fresco = false): Promise<void> {
    return recargar(fresco);
  }
}

export const perfil = new Perfil();

let enCamino: Promise<void> | null = null;
let escuchando = false;

export function recargar(fresco = false): Promise<void> {
  if (!escuchando) {
    escuchando = true;
    // Dura lo que dura el panel: una sola escucha para todas las vistas.
    alCambiarEquipos(() => recargar());
  }
  if (enCamino) return enCamino;
  enCamino = leerEquipos(fresco)
    .then((e) => {
      perfil.estado = e;
    })
    .catch(() => {
      // Si falla, queda lo último que se supo.
    })
    .finally(() => {
      enCamino = null;
    });
  return enCamino;
}

/** Pone lo que devolvió una acción (cambiar el modo, olvidar, abrir Tailscale). */
export function ponerPerfil(e: EstadoEquipos) {
  perfil.estado = e;
}

/** "Mac", "PC" o "computadora". */
export function nombreTipo(plataforma: string | undefined): string {
  return plataforma === 'mac' ? 'Mac' : plataforma === 'windows' ? 'PC' : 'computadora';
}

/** "tu Mac", "tu PC": lo que busca este equipo para el trabajo remoto. */
export function loQueBusca(propia: string | undefined): string {
  return propia === 'mac' ? 'tu PC con Windows' : 'tu Mac';
}
