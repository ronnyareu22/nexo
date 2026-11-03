<!--
  Fila de un aparato (DISENO.md, sección 5): foto o ilustración, nombre (o su alias), el estado y la batería
  en una línea, y a la derecha un chevron o, si se pasa `onalternar`, el interruptor para conectar.
  Toda la fila abre el aparato (`onabrir`); la elegida va en turquesa suave. El hover solo cambia el fondo.
  `grande` es la tarjeta de Dispositivos: foto grande sin cuadro detrás y el chevron como botón propio
  (`onentrar`) que abre todos los ajustes. También entran el doble clic y Enter sobre la tarjeta ya elegida.
  Ningún texto se corta: si no cabe, baja de línea.
-->
<script lang="ts">
  import type { Dispositivo } from '$lib/tipos';
  import { nivelPrincipal, nombreDe } from '$lib/tipos';
  import { cuando } from '$lib/tiempo';
  import { tooltip } from '$lib/tooltip';
  import Figura from './Figura.svelte';
  import Interruptor from './Interruptor.svelte';
  import Icono from './Icono.svelte';

  let {
    dispositivo: d,
    pendiente = false,
    elegido = false,
    ultimaVez = null,
    grande = false,
    onabrir,
    onalternar,
    onentrar,
  }: {
    dispositivo: Dispositivo;
    pendiente?: boolean;
    elegido?: boolean;
    /** Cuándo se usó por última vez (segundos Unix), para "Desconectado · usado ayer a las 21:10". */
    ultimaVez?: number | null;
    /** Tarjeta grande de la sección Dispositivos. */
    grande?: boolean;
    onabrir: () => void;
    /** Si está, en lugar del chevron va un interruptor para conectar o desconectar. */
    onalternar?: () => void;
    /** Abre todos los ajustes del aparato: el chevron pasa a ser un botón, y se entra con doble clic o Enter. */
    onentrar?: () => void;
  } = $props();

  const id = $props.id();
  const nombre = $derived(nombreDe(d));
  const nivel = $derived(nivelPrincipal(d));
  const linea = $derived.by(() => {
    if (pendiente) return d.conectado ? 'Desconectando…' : 'Conectando…';
    if (d.conectado) return nivel !== null ? `Conectado • ${nivel}% batería` : 'Conectado';
    return ultimaVez ? `Desconectado · usado ${cuando(ultimaVez)}` : 'Desconectado';
  });
  const tono = $derived(
    pendiente ? 'pendiente' : d.conectado ? (nivel !== null && nivel <= 20 ? 'baja' : 'conectado') : 'apagado',
  );

  function tecla(e: KeyboardEvent) {
    // La primera vez Enter elige la tarjeta; sobre la ya elegida, entra.
    if (e.key === 'Enter' && elegido && onentrar) {
      e.preventDefault();
      onentrar();
    }
  }
</script>

<div class="fila" class:grande class:activa={elegido} class:apagado={!d.conectado} data-dispositivo={d.id}>
  <!-- Toda la fila abre el aparato; el interruptor y el chevron quedan por encima y funcionan aparte. -->
  <button
    class="abrir"
    onclick={onabrir}
    ondblclick={onentrar}
    onkeydown={tecla}
    aria-label={grande ? nombre : `Ver ${nombre}`}
    aria-describedby="{id}-linea"
    aria-current={elegido ? 'true' : undefined}
  ></button>

  <span class="figura" aria-hidden="true"><Figura dispositivo={d} sombra={false} /></span>

  <span class="textos">
    <span class="nombre">{nombre}</span>
    <span class="linea {tono}">
      <span class="punto" aria-hidden="true"></span>
      <span id="{id}-linea">{linea}</span>
    </span>
  </span>

  {#if onalternar}
    <span class="control">
      <Interruptor
        encendido={d.conectado}
        cargando={pendiente}
        etiqueta={d.conectado ? `Desconectar ${nombre}` : `Conectar ${nombre}`}
        oncambio={onalternar}
      />
    </span>
  {:else if onentrar}
    <button class="entrar" onclick={onentrar} aria-label="Abrir {nombre}" use:tooltip={'Ver todo'}>
      <Icono nombre="chevron" tamano={18} />
    </button>
  {:else}
    <span class="chevron" aria-hidden="true"><Icono nombre="chevron" tamano={16} /></span>
  {/if}
</div>

<style>
  .fila {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 76px;
    padding: 10px 14px 10px 10px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-md);
    background: var(--superficie);
    transition:
      background-color var(--dur-color) ease,
      border-color var(--dur-color) ease;
  }
  .fila.activa {
    background: var(--elegido-bg);
    border-color: var(--elegido-borde);
  }
  .fila:not(.activa):has(.abrir:hover) {
    background: var(--superficie-hover);
  }
  .fila:has(.abrir:active) {
    transform: scale(0.98);
  }

  .abrir {
    position: absolute;
    inset: -1px;
    z-index: 1;
    padding: 0;
    border: 0;
    border-radius: inherit;
    background: transparent;
  }

  .figura {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 56px;
    height: 56px;
    padding: 6px;
    border-radius: 14px;
    background: var(--superficie-elevada);
    pointer-events: none;
  }
  .figura :global(img),
  .figura :global(svg) {
    transition: opacity var(--dur-color) ease;
  }
  .apagado .figura :global(img),
  .apagado .figura :global(svg) {
    opacity: 0.6;
  }

  .textos {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 2px;
    pointer-events: none;
  }
  .nombre {
    overflow-wrap: anywhere;
    font-size: 14px;
    font-weight: 600;
    line-height: 20px;
    color: var(--texto);
  }
  .linea {
    display: flex;
    align-items: baseline;
    gap: 7px;
    min-width: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .punto {
    flex-shrink: 0;
    align-self: center;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--exito);
  }
  .apagado .punto {
    background: var(--texto-3);
  }
  .baja .punto {
    background: var(--aviso);
  }
  .pendiente .punto {
    background: var(--primario);
    animation: parpadeo 900ms ease-in-out infinite;
  }
  @keyframes parpadeo {
    50% {
      opacity: 0.25;
    }
  }

  .control {
    position: relative;
    z-index: 2;
    display: flex;
  }
  .chevron {
    display: grid;
    flex-shrink: 0;
    color: var(--texto-3);
    pointer-events: none;
  }
  .activa .chevron {
    color: var(--elegido-fg);
  }

  /* Botón del chevron: abre todos los ajustes. Queda encima del botón de la fila. */
  .entrar {
    position: relative;
    z-index: 2;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 36px;
    height: 36px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--texto-3);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .entrar:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .entrar:active {
    transform: scale(0.98);
  }
  .activa .entrar {
    color: var(--elegido-fg);
  }

  /* Tarjeta grande de Dispositivos: unos 120 px de alto, foto grande sin cuadro detrás. */
  .grande {
    gap: 16px;
    min-height: 120px;
    padding: 10px 12px 10px 14px;
    border-radius: 18px;
  }
  .grande .figura {
    width: clamp(72px, 26%, 100px);
    height: 100px;
    padding: 0;
    border-radius: 0;
    background: none;
  }
  .grande .textos {
    gap: 4px;
  }
  .grande .nombre {
    font-size: 17px;
    line-height: 23px;
    letter-spacing: -0.01em;
  }
  .grande .linea {
    font-size: 13px;
    line-height: 19px;
  }
</style>
