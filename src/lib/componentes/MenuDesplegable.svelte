<!-- Menú desplegable propio (nunca el <select> nativo). El botón muestra la opción elegida; la lista flota debajo
     (o arriba si no cabe), con la elegida marcada. Teclado: ↑ ↓ Inicio Fin, Enter o espacio para elegir, Escape para cerrar.
     Se cierra al hacer clic afuera, al desplazar o al cambiar el tamaño de la ventana. -->
<script lang="ts">
  import { tick } from 'svelte';
  import Icono, { type Nombre } from './Icono.svelte';

  let {
    opciones,
    valor,
    etiqueta,
    oncambio,
  }: {
    opciones: { valor: string; nombre: string; detalle?: string; icono?: Nombre }[];
    valor: string;
    /** Nombre del menú para los lectores de pantalla (el texto visible lo pone quien lo usa). */
    etiqueta: string;
    oncambio: (valor: string) => void;
  } = $props();

  const id = $props.id();
  const ALTO_MAXIMO = 300;
  const SEPARACION = 6;
  const MARGEN = 8;

  let abierto = $state(false);
  let activo = $state(0);
  let arriba = $state(false);
  let pos = $state({ left: 0, top: 0, bottom: 0, ancho: 0, alto: ALTO_MAXIMO });
  let boton = $state<HTMLButtonElement>();
  let lista = $state<HTMLUListElement>();

  const elegida = $derived(opciones.find((o) => o.valor === valor));

  async function abrir() {
    if (abierto || opciones.length === 0) return;
    const i = opciones.findIndex((o) => o.valor === valor);
    activo = i >= 0 ? i : 0;
    abierto = true;
    await tick();
    colocar();
    lista?.focus({ preventScroll: true });
    verActivo();
  }

  function cerrar(volverAlBoton = false) {
    if (!abierto) return;
    abierto = false;
    if (volverAlBoton) boton?.focus();
  }

  function elegir(i: number) {
    const o = opciones[i];
    cerrar(true);
    if (o && o.valor !== valor) oncambio(o.valor);
  }

  // Debajo del botón; si no cabe y arriba hay más lugar, arriba. Nunca se sale de la ventana.
  function colocar() {
    if (!boton || !lista) return;
    const r = boton.getBoundingClientRect();
    const natural = Math.min(lista.scrollHeight + 2, ALTO_MAXIMO);
    const abajo = window.innerHeight - r.bottom - SEPARACION - MARGEN;
    const encima = r.top - SEPARACION - MARGEN;
    arriba = abajo < natural && encima > abajo;
    const ancho = Math.max(r.width, 200);
    pos = {
      left: Math.max(MARGEN, Math.min(r.left, window.innerWidth - ancho - MARGEN)),
      top: r.bottom + SEPARACION,
      bottom: window.innerHeight - r.top + SEPARACION,
      ancho,
      alto: Math.max(Math.min(natural, arriba ? encima : abajo), 80),
    };
  }

  function verActivo() {
    (lista?.children[activo] as HTMLElement | undefined)?.scrollIntoView({ block: 'nearest' });
  }

  function indiceDe(e: Event): number {
    const op = (e.target as Element).closest<HTMLElement>('[role="option"]');
    return op ? Number(op.dataset.i) : -1;
  }

  function teclaBoton(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      abrir();
    }
  }

  function teclaLista(e: KeyboardEvent) {
    const ultimo = opciones.length - 1;
    switch (e.key) {
      case 'ArrowDown':
        activo = Math.min(activo + 1, ultimo);
        break;
      case 'ArrowUp':
        activo = Math.max(activo - 1, 0);
        break;
      case 'Home':
        activo = 0;
        break;
      case 'End':
        activo = ultimo;
        break;
      case 'Enter':
      case ' ':
        elegir(activo);
        break;
      case 'Escape':
        cerrar(true);
        break;
      case 'Tab':
        // Vuelve al botón y deja que el Tab siga su camino desde ahí.
        cerrar(true);
        return;
      default:
        return;
    }
    e.preventDefault();
    verActivo();
  }

  // Abierto: se cierra con un clic afuera, al desplazar lo de atrás o al cambiar el tamaño de la ventana.
  $effect(() => {
    if (!abierto) return;
    const clicAfuera = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!boton?.contains(t) && !lista?.contains(t)) cerrar();
    };
    const desplazar = (e: Event) => {
      if (lista && e.target instanceof Node && lista.contains(e.target)) return;
      cerrar();
    };
    const redimensionar = () => cerrar();
    document.addEventListener('pointerdown', clicAfuera, true);
    window.addEventListener('scroll', desplazar, { capture: true, passive: true });
    window.addEventListener('resize', redimensionar, { passive: true });
    return () => {
      document.removeEventListener('pointerdown', clicAfuera, true);
      window.removeEventListener('scroll', desplazar, { capture: true });
      window.removeEventListener('resize', redimensionar);
    };
  });
</script>

<div class="menu">
  <button
    bind:this={boton}
    class="menu__boton"
    type="button"
    aria-haspopup="listbox"
    aria-expanded={abierto}
    aria-controls={abierto ? `${id}-lista` : undefined}
    aria-label="{etiqueta}: {elegida?.nombre ?? 'sin elegir'}"
    disabled={opciones.length === 0}
    onclick={() => (abierto ? cerrar() : abrir())}
    onkeydown={teclaBoton}
  >
    {#if elegida?.icono}<Icono nombre={elegida.icono} tamano={16} />{/if}
    <span class="menu__valor" class:vacio={!elegida}>
      {elegida?.nombre ?? (opciones.length ? 'Elige una opción' : 'Sin opciones')}
    </span>
    <span class="menu__chevron"><Icono nombre="chevron-abajo" tamano={16} /></span>
  </button>

  {#if abierto}
    <ul
      bind:this={lista}
      id="{id}-lista"
      class="menu__lista flotante"
      class:arriba
      role="listbox"
      tabindex="-1"
      aria-label={etiqueta}
      aria-activedescendant="{id}-op-{activo}"
      style:left="{pos.left}px"
      style:top={arriba ? undefined : `${pos.top}px`}
      style:bottom={arriba ? `${pos.bottom}px` : undefined}
      style:width="{pos.ancho}px"
      style:max-height="{pos.alto}px"
      onkeydown={teclaLista}
      onclick={(e) => {
        const i = indiceDe(e);
        if (i >= 0) elegir(i);
      }}
      onpointermove={(e) => {
        const i = indiceDe(e);
        if (i >= 0) activo = i;
      }}
      onfocusout={(e) => {
        const destino = e.relatedTarget as Node | null;
        if (!destino || (destino !== boton && !lista?.contains(destino))) cerrar();
      }}
    >
      {#each opciones as o, i (o.valor)}
        <li
          id="{id}-op-{i}"
          data-i={i}
          class="menu__op"
          class:elegido={o.valor === valor}
          class:activo={i === activo}
          role="option"
          aria-selected={o.valor === valor}
        >
          {#if o.icono}<Icono nombre={o.icono} tamano={16} />{/if}
          <span class="menu__textos">
            <span class="menu__nombre">{o.nombre}</span>
            {#if o.detalle}<span class="menu__detalle">{o.detalle}</span>{/if}
          </span>
          {#if o.valor === valor}<span class="menu__check"><Icono nombre="check" tamano={15} /></span>{/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .menu {
    position: relative;
    min-width: 0;
  }

  .menu__boton {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 38px;
    padding: 7px 10px 7px 12px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-sm);
    background: var(--superficie-elevada);
    color: var(--texto);
    font-size: 13.5px;
    font-weight: 500;
    text-align: left;
    transition:
      background-color var(--dur-color) ease,
      border-color var(--dur-color) ease;
  }
  .menu__boton :global(svg) {
    flex-shrink: 0;
    color: var(--texto-2);
  }
  .menu__boton:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .menu__boton[aria-expanded='true'] {
    border-color: var(--elegido-borde);
  }
  .menu__boton:disabled {
    cursor: default;
    opacity: 0.6;
  }
  /* Nombres largos pasan a otra línea: nunca se cortan. */
  .menu__valor {
    flex: 1;
    min-width: 0;
    line-height: 18px;
    overflow-wrap: anywhere;
  }
  .menu__valor.vacio {
    color: var(--texto-3);
  }
  .menu__chevron {
    display: grid;
    margin-left: auto;
  }
  .menu__chevron :global(svg) {
    color: var(--texto-3);
  }

  .menu__lista {
    position: fixed;
    z-index: 50;
    margin: 0;
    padding: 4px;
    list-style: none;
    border-radius: 12px;
    overflow-y: auto;
    overscroll-behavior: contain;
    animation: aparecer 120ms ease-out;
  }
  .menu__lista.arriba {
    animation-name: aparecer-arriba;
  }
  .menu__lista:focus {
    outline: none;
  }
  @keyframes aparecer {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  @keyframes aparecer-arriba {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }

  .menu__op {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 34px;
    padding: 6px 10px;
    border-width: 1px;
    border-style: solid;
    border-radius: 8px;
    font-size: 13.5px;
    line-height: 18px;
    cursor: pointer;
    transition: background-color var(--dur-color) ease;
  }
  .menu__op :global(svg) {
    flex-shrink: 0;
  }
  .menu__op:not(.elegido) {
    border-color: transparent;
    background: transparent;
    color: var(--texto);
  }
  .menu__op:not(.elegido) :global(svg) {
    color: var(--texto-2);
  }
  /* La opción bajo el mouse o el teclado: solo cambia el color. */
  .menu__op.activo:not(.elegido) {
    background: var(--superficie-hover);
  }
  .menu__op.activo.elegido {
    background: color-mix(in srgb, var(--primario) 22%, var(--superficie));
  }
  .menu__textos {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .menu__nombre,
  .menu__detalle {
    overflow-wrap: anywhere;
  }
  .elegido .menu__nombre {
    font-weight: 600;
  }
  .menu__detalle {
    font-size: 12px;
    line-height: 16px;
    color: var(--texto-3);
  }
  .menu__check {
    display: grid;
    margin-left: auto;
  }
</style>
