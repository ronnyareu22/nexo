<!-- Estado de conexión: conectado (verde), conectando o desconectando (primario, parpadea) o desconectado (gris).
     El punto lleva un halo fino del mismo tono; `chico` para filas y tarjetas apretadas. -->
<script lang="ts">
  import type { EstadoConexion } from '$lib/tipos';

  let { estado, chico = false }: { estado: EstadoConexion; chico?: boolean } = $props();

  const textos: Record<EstadoConexion, string> = {
    conectado: 'Conectado',
    conectando: 'Conectando…',
    desconectando: 'Desconectando…',
    desconectado: 'Desconectado',
  };
</script>

<span class="chip {estado}" class:chico>
  <span class="punto" aria-hidden="true"></span>
  {textos[estado]}
</span>

<style>
  .chip {
    --tono: var(--texto-3);
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 26px;
    padding: 3px 11px 3px 9px;
    border-radius: 99px;
    font-size: 12.5px;
    line-height: 18px;
    font-weight: 600;
    letter-spacing: 0.005em;
    color: var(--texto-2);
    background: color-mix(in srgb, var(--tono) 10%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--tono) 22%, transparent);
  }
  .chico {
    min-height: 22px;
    gap: 6px;
    padding: 2px 9px 2px 7px;
    font-size: 12px;
    line-height: 16px;
  }
  .punto {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--tono);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--tono) 22%, transparent);
  }
  .chico .punto {
    width: 6px;
    height: 6px;
    box-shadow: 0 0 0 2.5px color-mix(in srgb, var(--tono) 22%, transparent);
  }

  .conectado {
    --tono: var(--exito);
    color: var(--exito);
  }
  .conectando,
  .desconectando {
    --tono: var(--primario);
    color: var(--primario-texto);
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
