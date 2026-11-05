<!--
  Diálogo mínimo (DISENO.md, sección 5): ícono, título, qué pasa, contenido opcional (por ejemplo un campo)
  y dos botones a lo ancho. El de confirmar es turquesa, o rojo si borra (`tono="peligro"`).
  Escape o un clic fuera cancelan. El foco queda dentro: empieza en el elemento con `data-autofocus`
  del contenido o, si no hay, en Cancelar (lo más seguro). Enter dentro de un campo confirma.
  `ocupado` deja el de confirmar quieto mientras algo trabaja; `soloConfirmar` deja un solo botón (un aviso de listo).
  Se dibuja al final de <body> para quedar encima de todo, y solo aparece con un fundido.
-->
<script lang="ts">
  import { tick, type Snippet } from 'svelte';
  import { fade } from 'svelte/transition';
  import Icono, { type Nombre } from './Icono.svelte';

  let {
    abierto,
    titulo,
    descripcion,
    icono,
    tono = 'normal',
    textoConfirmar,
    textoCancelar = 'Cancelar',
    ocupado = false,
    soloConfirmar = false,
    onconfirmar,
    oncancelar,
    children,
  }: {
    abierto: boolean;
    titulo: string;
    descripcion?: string;
    icono?: Nombre;
    tono?: 'normal' | 'peligro';
    textoConfirmar: string;
    textoCancelar?: string;
    /** Algo está trabajando: confirmar queda desactivado. */
    ocupado?: boolean;
    /** Sin Cancelar: un solo botón a lo ancho. */
    soloConfirmar?: boolean;
    onconfirmar: () => void;
    oncancelar: () => void;
    children?: Snippet;
  } = $props();

  const id = $props.id();
  const sinMovimiento =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const dur = sinMovimiento ? 0 : 140;

  let tarjeta = $state<HTMLDivElement | null>(null);
  let cancelar = $state<HTMLButtonElement | null>(null);
  let confirmar = $state<HTMLButtonElement | null>(null);

  // Al abrir: guarda dónde estaba el foco y lo pone en el campo marcado o en Cancelar. Al cerrar, lo devuelve.
  $effect(() => {
    if (!abierto) return;
    const previo = document.activeElement as HTMLElement | null;
    tick().then(() => {
      const campo = tarjeta?.querySelector<HTMLElement>('[data-autofocus]');
      (campo ?? cancelar ?? confirmar)?.focus();
      if (campo instanceof HTMLInputElement) campo.select();
    });
    return () => {
      if (previo?.isConnected) previo.focus();
    };
  });

  // Si el botón con el foco se desactiva o desaparece (ocupado, solo confirmar), el foco vuelve al diálogo:
  // así Escape y Tab siguen funcionando.
  $effect(() => {
    if (!abierto) return;
    void ocupado;
    void soloConfirmar;
    tick().then(() => {
      const activo = document.activeElement as HTMLButtonElement | null;
      if (!tarjeta || (tarjeta.contains(activo) && !activo?.disabled)) return;
      (cancelar ?? confirmar)?.focus();
    });
  });

  /** Lleva el nodo al final de <body>: así ningún contenedor con desplazamiento o contención lo recorta. */
  function alCuerpo(nodo: HTMLElement) {
    document.body.appendChild(nodo);
    return {
      destroy: () => nodo.remove(),
    };
  }

  const enfocables = () =>
    Array.from(
      tarjeta?.querySelectorAll<HTMLElement>(
        'button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])',
      ) ?? [],
    );

  function teclas(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      oncancelar();
      return;
    }
    if (e.key !== 'Tab') return;
    // El foco da la vuelta dentro del diálogo.
    const lista = enfocables();
    if (!lista.length) return;
    const primero = lista[0];
    const ultimo = lista[lista.length - 1];
    if (e.shiftKey && document.activeElement === primero) {
      e.preventDefault();
      ultimo.focus();
    } else if (!e.shiftKey && document.activeElement === ultimo) {
      e.preventDefault();
      primero.focus();
    }
  }
</script>

{#if abierto}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="capa"
    use:alCuerpo
    transition:fade={{ duration: dur }}
    onkeydown={teclas}
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) oncancelar();
    }}
  >
    <div
      bind:this={tarjeta}
      class="dialogo flotante"
      class:peligro={tono === 'peligro'}
      role="dialog"
      aria-modal="true"
      aria-labelledby="{id}-titulo"
      aria-describedby={descripcion ? `${id}-texto` : undefined}
      aria-busy={ocupado}
    >
      <!-- Un formulario para que Enter dentro de un campo confirme. -->
      <form
        class="formulario"
        onsubmit={(e) => {
          e.preventDefault();
          onconfirmar();
        }}
      >
        {#if icono}
          <span class="icono" aria-hidden="true"><Icono nombre={icono} tamano={22} /></span>
        {/if}
        <h2 id="{id}-titulo">{titulo}</h2>
        {#if descripcion}<p id="{id}-texto">{descripcion}</p>{/if}
        {#if children}<div class="contenido">{@render children()}</div>{/if}

        <div class="botones" class:uno={soloConfirmar}>
          {#if !soloConfirmar}
            <button type="button" class="boton secundario" bind:this={cancelar} onclick={oncancelar}>{textoCancelar}</button>
          {/if}
          <button type="submit" class="boton confirmar" bind:this={confirmar} disabled={ocupado} aria-busy={ocupado}>
            {textoConfirmar}
          </button>
        </div>
      </form>
    </div>
  </div>
{/if}

<style>
  .capa {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(4 8 14 / 0.5);
  }

  .dialogo {
    width: min(360px, 100%);
    padding: 24px 20px 20px;
    border-radius: var(--radio-lg);
    text-align: center;
  }
  .formulario {
    display: flex;
    flex-direction: column;
    align-items: center;
    margin: 0;
  }

  .icono {
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    margin-bottom: 14px;
    border-radius: 50%;
    color: var(--primario-texto);
    background: color-mix(in srgb, var(--primario) 14%, transparent);
  }
  .peligro .icono {
    color: var(--error);
    background: color-mix(in srgb, var(--error) 14%, transparent);
  }

  h2 {
    margin: 0;
    font-size: 16px;
    line-height: 22px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  p {
    margin: 6px 0 0;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto-2);
  }
  .contenido {
    width: 100%;
    margin-top: 16px;
    text-align: left;
  }

  .botones {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    width: 100%;
    margin-top: 20px;
  }
  .boton {
    min-height: 38px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--radio-sm);
    font-size: 13.5px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .botones.uno {
    grid-template-columns: 1fr;
  }
  .boton:active:not(:disabled) {
    transform: scale(0.98);
  }
  .boton:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .secundario {
    background: var(--superficie-hundida);
    color: var(--texto);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .secundario:hover {
    background: var(--superficie-hover);
  }
  .confirmar {
    background: var(--boton-fondo);
    color: var(--boton-texto);
  }
  .confirmar:hover:not(:disabled) {
    background: var(--boton-fondo-hover);
  }
  .peligro .confirmar {
    background: var(--error);
    color: #ffffff;
  }
  .peligro .confirmar:hover {
    background: color-mix(in srgb, var(--error) 88%, #000000);
  }
</style>
