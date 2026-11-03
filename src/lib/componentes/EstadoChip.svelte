<!-- Estado de conexión: conectado (verde), conectando o desconectando (primario, parpadea) o desconectado (gris). -->
<script lang="ts">
  import type { EstadoConexion } from '$lib/tipos';

  let { estado }: { estado: EstadoConexion } = $props();

  const textos: Record<EstadoConexion, string> = {
    conectado: 'Conectado',
    conectando: 'Conectando…',
    desconectando: 'Desconectando…',
    desconectado: 'Desconectado',
  };
</script>

<span class="chip {estado}">
  <span class="punto"></span>
  {textos[estado]}
</span>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 26px;
    padding: 2px 11px 2px 9px;
    border-radius: 99px;
    font-size: 12.5px;
    line-height: 18px;
    font-weight: 600;
    background: var(--superficie-hundida);
    color: var(--texto-2);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .punto {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
  }
  .desconectado .punto {
    background: var(--texto-3);
  }

  .conectado {
    color: var(--exito);
    background: color-mix(in srgb, var(--exito) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--exito) 30%, transparent);
  }

  .conectando,
  .desconectando {
    color: var(--primario-texto);
    background: color-mix(in srgb, var(--primario) 12%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--primario) 30%, transparent);
  }
  .conectando .punto,
  .desconectando .punto {
    animation: parpadeo 900ms ease-in-out infinite;
  }
  @keyframes parpadeo {
    50% {
      opacity: 0.25;
    }
  }
</style>
