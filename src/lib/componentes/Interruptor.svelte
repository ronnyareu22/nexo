<!-- Interruptor (encendido/apagado): píldora con perilla blanca de resorte, encendido en el primario. Gira mientras conecta. -->
<script lang="ts">
  import { tooltip } from '$lib/tooltip';

  let {
    encendido,
    cargando = false,
    etiqueta,
    grande = false,
    oncambio,
  }: {
    encendido: boolean;
    cargando?: boolean;
    etiqueta: string;
    grande?: boolean;
    oncambio: () => void;
  } = $props();
</script>

<button
  class="interruptor"
  class:encendido
  class:cargando
  class:grande
  role="switch"
  aria-checked={encendido}
  aria-label={etiqueta}
  aria-busy={cargando}
  use:tooltip={etiqueta}
  disabled={cargando}
  onclick={(e) => {
    e.stopPropagation();
    oncambio();
  }}
>
  <span class="perilla">
    {#if cargando}<span class="giro"></span>{/if}
  </span>
</button>

<style>
  .interruptor {
    --ancho: 50px;
    --alto: 30px;
    --perilla: 24px;
    position: relative;
    flex-shrink: 0;
    width: var(--ancho);
    height: var(--alto);
    padding: 0;
    border: 0;
    border-radius: 99px;
    background: color-mix(in srgb, var(--texto-3) 32%, var(--superficie-hundida));
    box-shadow: inset 0 0 0 1px var(--borde);
    transition:
      background-color 200ms ease,
      box-shadow 200ms ease;
  }
  .grande {
    --ancho: 62px;
    --alto: 36px;
    --perilla: 30px;
  }
  .encendido {
    background: var(--primario);
    box-shadow: none;
  }
  .interruptor:focus-visible {
    box-shadow: var(--foco);
  }

  .perilla {
    position: absolute;
    top: 3px;
    left: 3px;
    width: var(--perilla);
    height: var(--perilla);
    border-radius: 99px;
    display: grid;
    place-items: center;
    background: #ffffff;
    box-shadow:
      0 0 0 0.5px rgb(0 0 0 / 0.1),
      0 1px 2px rgb(0 0 0 / 0.18),
      0 3px 8px -2px rgb(0 0 0 / 0.25);
    transition:
      transform 420ms var(--resorte),
      width 200ms ease;
  }
  .encendido .perilla {
    transform: translateX(calc(var(--ancho) - var(--perilla) - 6px));
  }
  /* Al presionar, la perilla se estira como en iOS. */
  .interruptor:active:not(:disabled) .perilla {
    width: calc(var(--perilla) + 6px);
  }
  .encendido:active:not(:disabled) .perilla {
    transform: translateX(calc(var(--ancho) - var(--perilla) - 12px));
  }

  .cargando {
    cursor: progress;
  }
  .giro {
    width: 55%;
    height: 55%;
    border-radius: 50%;
    border: 2px solid rgb(0 0 0 / 0.12);
    border-top-color: var(--primario);
    animation: girar 700ms linear infinite;
  }
  @keyframes girar {
    to {
      transform: rotate(360deg);
    }
  }
</style>
