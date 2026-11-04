<!--
  Fila de un aparato (DISENO.md, sección 5): foto o ilustración, nombre (o su alias), el estado con la pila chica
  y su nivel, y a la derecha un chevron o, si se pasa `onalternar`, el interruptor para conectar.
  `acciones` agrega atajos propios del aparato antes del interruptor (por ejemplo, silenciar).
  Toda la fila abre el aparato (`onabrir`); la elegida va en turquesa suave. El hover solo cambia el fondo.
  Variantes:
  - `grande`: la tarjeta de Dispositivos; foto grande sin cuadro, marca y tipo, y el chevron como botón propio
    (`onentrar`) que abre todos los ajustes. También entran el doble clic y Enter sobre la tarjeta ya elegida.
  - `agrupada`: fila sin borde ni fondo propio, para listas dentro de una tarjeta (Inicio).
  Ningún texto se corta: si no cabe, baja de línea.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Dispositivo } from '$lib/tipos';
  import { nivelPrincipal, nombreDe, nombreTipo } from '$lib/tipos';
  import { cuando } from '$lib/tiempo';
  import { tooltip } from '$lib/tooltip';
  import Figura from './Figura.svelte';
  import Interruptor from './Interruptor.svelte';
  import Icono from './Icono.svelte';
  import { colorBateria } from './PanelBateria.svelte';

  let {
    dispositivo: d,
    pendiente = false,
    elegido = false,
    ultimaVez = null,
    grande = false,
    agrupada = false,
    acciones,
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
    /** Fila sin borde ni fondo, dentro de una tarjeta. */
    agrupada?: boolean;
    /** Atajos propios del aparato, a la izquierda del interruptor o del chevron. */
    acciones?: Snippet<[Dispositivo]>;
    onabrir: () => void;
    /** Si está, en lugar del chevron va un interruptor para conectar o desconectar. */
    onalternar?: () => void;
    /** Abre todos los ajustes del aparato: el chevron pasa a ser un botón, y se entra con doble clic o Enter. */
    onentrar?: () => void;
  } = $props();

  const id = $props.id();
  const nombre = $derived(nombreDe(d));
  const nivel = $derived(nivelPrincipal(d));
  const meta = $derived([d.fabricante, nombreTipo[d.tipo]].filter(Boolean).join(' · '));
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

<div class="fila" class:grande class:agrupada class:activa={elegido} class:apagado={!d.conectado} data-dispositivo={d.id}>
  <!-- Toda la fila abre el aparato; el interruptor, los atajos y el chevron quedan por encima y funcionan aparte. -->
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
    {#if grande && meta}<span class="meta">{meta}</span>{/if}
    <span class="linea {tono}">
      <span class="punto" aria-hidden="true"></span>
      <span id="{id}-linea" class="linea__texto">
        {#if pendiente}
          {d.conectado ? 'Desconectando…' : 'Conectando…'}
        {:else if d.conectado}
          Conectado{#if nivel !== null}
            <span class="separador" aria-hidden="true">·</span>
            <span class="nivel">
              <span class="mini-bateria" style:--nivel="{nivel}%" style:--tono={colorBateria(nivel)} aria-hidden="true"></span>
              {nivel} % batería
            </span>
          {/if}
        {:else}
          Desconectado{#if ultimaVez}<span class="separador" aria-hidden="true">·</span> usado {cuando(ultimaVez)}{/if}
        {/if}
      </span>
    </span>
  </span>

  {#if acciones}
    <span class="acciones">{@render acciones(d)}</span>
  {/if}

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
    border-radius: var(--radio-md);
    background: var(--relleno-tarjeta);
    box-shadow:
      inset 0 1px 0 var(--luz-tope),
      0 0 0 1px var(--borde-tarjeta),
      var(--sombra-tarjeta);
    transition: background-color var(--dur-color) ease;
  }
  .fila.activa {
    background: var(--elegido-bg);
    box-shadow:
      inset 0 0 0 1px var(--elegido-borde),
      var(--sombra-tarjeta);
  }
  .fila:not(.activa):has(.abrir:hover) {
    background: var(--superficie-hover);
  }
  .fila:has(.abrir:active) {
    transform: scale(0.98);
  }

  .abrir {
    position: absolute;
    inset: 0;
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
    background: var(--superficie-inserto);
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
    letter-spacing: -0.005em;
    color: var(--texto);
  }
  .meta {
    font-size: 12.5px;
    line-height: 17px;
    color: var(--texto-3);
    overflow-wrap: break-word;
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
  .linea__texto {
    min-width: 0;
    overflow-wrap: break-word;
  }
  .separador {
    margin: 0 5px;
    color: var(--texto-3);
  }
  .nivel {
    white-space: nowrap;
  }
  .punto {
    flex-shrink: 0;
    align-self: center;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--exito);
    box-shadow: 0 0 0 2.5px color-mix(in srgb, var(--exito) 20%, transparent);
  }
  .apagado .punto {
    background: var(--texto-3);
    box-shadow: none;
  }
  .baja .punto {
    background: var(--aviso);
    box-shadow: 0 0 0 2.5px color-mix(in srgb, var(--aviso) 20%, transparent);
  }
  .pendiente .punto {
    background: var(--primario);
    box-shadow: none;
    animation: parpadeo 900ms ease-in-out infinite;
  }
  @keyframes parpadeo {
    50% {
      opacity: 0.25;
    }
  }

  .acciones {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
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
    min-height: 116px;
    padding: 10px 12px 10px 14px;
    border-radius: 20px;
  }
  .grande .figura {
    width: clamp(72px, 24%, 96px);
    height: 92px;
    padding: 0;
    border-radius: 0;
    background: none;
  }
  .grande .textos {
    gap: 3px;
  }
  .grande .nombre {
    font-size: 16.5px;
    line-height: 22px;
    letter-spacing: -0.015em;
  }
  .grande .linea {
    margin-top: 3px;
    font-size: 13px;
    line-height: 19px;
  }

  /* Agrupada: sin borde ni fondo propios; el hover pinta solo la fila. */
  .agrupada {
    min-height: 68px;
    padding: 8px 10px 8px 6px;
    border-radius: 14px;
    background: transparent;
    box-shadow: none;
  }
  .agrupada .figura {
    width: 64px;
    height: 52px;
    padding: 0;
    border-radius: 0;
    background: none;
  }
</style>
