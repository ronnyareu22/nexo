<!--
  Escritorio: la escena de un lugar (casa u oficina) vista de frente, con la profundidad fingida y barata de un
  diorama: la pared, la mesa (su cara de arriba y su canto) y tres capas. Lo del fondo va más chico y más arriba.
  Cada cosa es una foto sin fondo con su sombra de apoyo; lo encendido lleva un resplandor verde debajo y lo
  apagado se ve tenue. Las decoraciones no muestran estado.
  - A la vista: tocar un aparato de este equipo abre su página en Nexo; uno del otro equipo muestra su ficha.
  - Al editar: se arrastra (a lo ancho, y hacia atrás o adelante entre capas) y se agranda desde la esquina.
    Con el teclado: flechas para mover, + y − para el tamaño, Supr para quitar y Esc para soltar la elección.
  Sin bucles ni animaciones: se vuelve a dibujar solo cuando cambia algo. El hover solo cambia el contorno.
-->
<script lang="ts">
  import type { Ubicada } from '$lib/setup';
  import { PROPORCION, acotarX, arribaVisible, capaCercana } from '$lib/setup';
  import { cuando } from '$lib/tiempo';
  import Ilustracion from './Ilustracion.svelte';
  import Icono from './Icono.svelte';

  type Cambio = { x?: number; y?: number; capa?: number; escala?: number };

  let {
    piezas,
    etiqueta,
    editando = false,
    seleccion = null,
    tenue = false,
    aviso = null,
    ficha = null,
    escena = $bindable(),
    onelegir,
    onmover,
    onsoltar,
    onquitar,
    onabrir,
    onficha,
  }: {
    piezas: Ubicada[];
    /** Para los lectores de pantalla: "Escritorio de la casa". */
    etiqueta: string;
    editando?: boolean;
    seleccion?: string | null;
    /** El equipo de este lugar está apagado: todo se ve tenue. */
    tenue?: boolean;
    /** Un aviso sobre la pared ("innotech1017 está apagada o sin Nexo"). */
    aviso?: string | null;
    /** La pieza con la ficha abierta (aparatos del otro equipo). */
    ficha?: string | null;
    escena?: HTMLDivElement;
    onelegir?: (id: string | null) => void;
    onmover?: (id: string, cambio: Cambio) => void;
    onsoltar?: (id: string) => void;
    onquitar?: (id: string) => void;
    /** Un aparato, pantalla o equipo de este equipo: abrir su página. */
    onabrir?: (p: Ubicada) => void;
    onficha?: (id: string | null) => void;
  } = $props();

  const uid = $props.id();

  // --- Arrastrar y agrandar (solo al editar) ---
  let arrastre: {
    id: string;
    modo: 'mover' | 'tamano';
    cx: number;
    cy: number;
    x0: number;
    base0: number;
    pared: boolean;
    escala0: number;
    anchoPx: number;
    movio: boolean;
  } | null = null;

  function empezar(e: PointerEvent, p: Ubicada, modo: 'mover' | 'tamano') {
    if (!editando || e.button !== 0 || !escena) return;
    e.preventDefault();
    e.stopPropagation();
    onelegir?.(p.item.id);
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    const ancho = escena.getBoundingClientRect().width;
    arrastre = {
      id: p.item.id,
      modo,
      cx: e.clientX,
      cy: e.clientY,
      x0: p.item.x,
      base0: p.base,
      pared: !!p.forma.pared,
      escala0: p.item.escala,
      anchoPx: Math.max(8, ((p.forma.caja[2] - p.forma.caja[0]) * p.ancho * ancho) / 100),
      movio: false,
    };
    if (modo === 'mover') (e.currentTarget as HTMLElement).focus({ preventScroll: true });
  }

  function mover(e: PointerEvent) {
    const a = arrastre;
    if (!a || !escena) return;
    if (!a.movio && Math.hypot(e.clientX - a.cx, e.clientY - a.cy) < 3) return;
    a.movio = true;
    const r = escena.getBoundingClientRect();
    if (a.modo === 'tamano') {
      const escala = (a.escala0 * (a.anchoPx + 2 * (e.clientX - a.cx))) / a.anchoPx;
      onmover?.(a.id, { escala: Math.round(escala * 20) / 20 });
      return;
    }
    const dx = ((e.clientX - a.cx) / r.width) * 100;
    const dy = ((e.clientY - a.cy) / r.height) * 100;
    const x = acotarX(a.x0 + dx);
    // Lo de la mesa salta suave a la capa más cercana; lo de la pared va donde se suelte.
    onmover?.(a.id, a.pared ? { x, y: a.base0 + dy } : { x, capa: capaCercana(a.base0 + dy) });
  }

  function soltar() {
    if (arrastre?.movio && arrastre.modo === 'mover') onsoltar?.(arrastre.id);
    arrastre = null;
  }

  function teclas(e: KeyboardEvent, p: Ubicada) {
    if (!editando) return;
    const paso = e.shiftKey ? 5 : 1;
    const id = p.item.id;
    const pared = !!p.forma.pared;
    switch (e.key) {
      case 'ArrowLeft':
        onmover?.(id, { x: acotarX(p.item.x - paso) });
        break;
      case 'ArrowRight':
        onmover?.(id, { x: acotarX(p.item.x + paso) });
        break;
      case 'ArrowUp':
        onmover?.(id, pared ? { y: p.base - paso } : { capa: Math.max(0, p.item.capa - 1) });
        break;
      case 'ArrowDown':
        onmover?.(id, pared ? { y: p.base + paso } : { capa: Math.min(2, p.item.capa + 1) });
        break;
      case '+':
      case '=':
        onmover?.(id, { escala: p.item.escala + 0.1 });
        break;
      case '-':
        onmover?.(id, { escala: p.item.escala - 0.1 });
        break;
      case 'Delete':
      case 'Backspace':
        onquitar?.(id);
        break;
      case 'Escape':
        onelegir?.(null);
        (e.currentTarget as HTMLElement).blur();
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  // --- A la vista ---
  function tocar(p: Ubicada) {
    if (editando) return;
    if (p.local && p.estado !== null) onabrir?.(p);
    else if (p.estado !== null) onficha?.(ficha === p.item.id ? null : p.item.id);
  }

  const abierta = $derived(ficha ? (piezas.find((p) => p.item.id === ficha) ?? null) : null);
  /** La ficha va sobre la pieza o, si no cabe arriba, debajo. */
  const fichaArriba = $derived(
    abierta ? abierta.base - (abierta.forma.caja[3] - arribaVisible(abierta.forma)) * abierta.ancho * PROPORCION > 32 : true,
  );
  const cimaFicha = $derived(
    abierta ? abierta.base - (abierta.forma.caja[3] - arribaVisible(abierta.forma)) * abierta.ancho * PROPORCION : 0,
  );

  // Con la ficha abierta: Esc o un clic afuera la cierran.
  $effect(() => {
    if (!abierta) return;
    const fuera = (e: PointerEvent) => {
      const t = e.target as Element | null;
      if (t?.closest?.('.ficha') || t?.closest?.('.toque')) return;
      onficha?.(null);
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onficha?.(null);
    };
    document.addEventListener('pointerdown', fuera, true);
    window.addEventListener('keydown', esc);
    return () => {
      document.removeEventListener('pointerdown', fuera, true);
      window.removeEventListener('keydown', esc);
    };
  });

  const etiquetaDe = (p: Ubicada) =>
    p.estado === null
      ? p.nombre
      : `${p.nombre}: ${p.estadoTexto}${p.bateria !== null ? `, batería ${p.bateria} %` : ''}`;
</script>

<!-- La torre del PC, en el mismo grafito que las ilustraciones de los aparatos. -->
{#snippet torre(encendida: boolean)}
  <svg viewBox="0 0 200 200" class="torre" class:encendida aria-hidden="true">
    <defs>
      <linearGradient id="torre-{uid}" x1="0" y1="0" x2="0.4" y2="1">
        <stop offset="0" stop-color="var(--grafito-1)" />
        <stop offset="0.55" stop-color="var(--grafito-2)" />
        <stop offset="1" stop-color="var(--grafito-3)" />
      </linearGradient>
    </defs>
    <rect x="60" y="16" width="80" height="158" rx="10" fill="url(#torre-{uid})" />
    <rect x="67" y="24" width="5" height="142" rx="2.5" fill="#ffffff" opacity="0.08" />
    <circle cx="100" cy="40" r="7" fill="var(--grafito-3)" />
    <circle class="led" cx="100" cy="40" r="2.6" />
    <rect x="82" y="62" width="36" height="4" rx="2" fill="var(--grafito-3)" />
    <rect x="82" y="72" width="36" height="4" rx="2" fill="var(--grafito-3)" />
    <rect x="70" y="172" width="12" height="7" rx="2" fill="var(--grafito-3)" />
    <rect x="118" y="172" width="12" height="7" rx="2" fill="var(--grafito-3)" />
  </svg>
{/snippet}

<div class="marco">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="escena"
    class:tenue
    class:editando
    bind:this={escena}
    role="group"
    aria-label={etiqueta}
    onpointerdown={(e) => {
      if (editando && e.target === e.currentTarget) onelegir?.(null);
    }}
  >
    <!-- Pared, mesa y piso: dibujo fijo. La aurora es un resplandor quieto sobre la pared. -->
    <div class="fondo" aria-hidden="true">
      <svg viewBox="0 0 160 100" preserveAspectRatio="none">
        <defs>
          <linearGradient id="mesa-{uid}" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stop-color="var(--s-mesa-fondo)" />
            <stop offset="1" stop-color="var(--s-mesa)" />
          </linearGradient>
          <radialGradient id="piso-{uid}" cx="0.5" cy="0.5" r="0.5">
            <stop offset="0" stop-color="var(--sombra-apoyo)" />
            <stop offset="1" stop-color="var(--sombra-apoyo)" stop-opacity="0" />
          </radialGradient>
        </defs>
        <rect x="0" y="0" width="160" height="100" fill="var(--s-pared)" />
        <rect x="0" y="92" width="160" height="8" fill="var(--s-piso)" />
        <ellipse cx="80" cy="96" rx="76" ry="3.2" fill="url(#piso-{uid})" />
        <rect x="11" y="89" width="2.6" height="8" rx="0.6" fill="var(--s-pata)" />
        <rect x="146.4" y="89" width="2.6" height="8" rx="0.6" fill="var(--s-pata)" />
        <rect x="0" y="55.4" width="160" height="0.6" fill="var(--s-zocalo)" />
        <polygon points="14.4,56 145.6,56 155.2,84 4.8,84" fill="url(#mesa-{uid})" />
        <rect x="4.8" y="84" width="150.4" height="5.4" rx="0.8" fill="var(--s-canto)" />
        <rect x="4.8" y="84" width="150.4" height="0.5" fill="var(--s-brillo)" />
      </svg>
      <span class="aurora"></span>
    </div>

    {#each piezas as p (p.item.id)}
      {@const elegida = editando && seleccion === p.item.id}
      {@const interactiva = editando || p.estado !== null}
      <div
        class="pieza"
        class:apagada={p.estado === 'apagado'}
        class:deco={p.estado === null}
        class:pared={p.forma.pared}
        class:plano={(p.forma.aplastar ?? 1) < 0.5}
        class:elegida
        style:left="{p.item.x}%"
        style:top="{p.base}%"
        style:width="{p.ancho}%"
        style:z-index={elegida ? 900 : p.z}
        style:--izq={p.forma.caja[0]}
        style:--der={p.forma.caja[2]}
        style:--arriba={arribaVisible(p.forma)}
        style:--abajo={p.forma.caja[3]}
        style:--aplastar={p.forma.aplastar ?? 1}
      >
        {#if !p.forma.pared && !p.colgada}
          <span class="apoyo" class:brillo={p.estado === 'encendido'} aria-hidden="true"></span>
        {/if}
        <span class="visual" aria-hidden="true">
          {#if p.forma.foto}
            <img src={p.forma.foto} alt="" draggable="false" decoding="async" />
          {:else if p.forma.ilustracion === 'torre'}
            {@render torre(p.estado === 'encendido')}
          {:else if p.forma.ilustracion}
            <Ilustracion tipo={p.forma.ilustracion} marca={p.marca} encendido={p.estado === 'encendido'} />
          {/if}
        </span>
        {#if interactiva}
          <button
            type="button"
            class="toque"
            aria-label={editando ? `${p.nombre}. Flechas para mover, más y menos para el tamaño, Suprimir para quitar.` : etiquetaDe(p)}
            aria-pressed={editando ? elegida : undefined}
            aria-expanded={!editando && !p.local ? ficha === p.item.id : undefined}
            onclick={() => tocar(p)}
            onfocus={() => editando && onelegir?.(p.item.id)}
            onkeydown={(e) => teclas(e, p)}
            onpointerdown={(e) => empezar(e, p, 'mover')}
            onpointermove={mover}
            onpointerup={soltar}
            onpointercancel={soltar}
          ></button>
        {/if}
        {#if p.estado !== null && !editando}
          <span class="estado" class:encendido={p.estado === 'encendido'} aria-hidden="true">
            <span class="punto"></span>{#if p.bateria !== null}<span class="nivel">{p.bateria} %</span>{/if}
          </span>
        {/if}
        {#if elegida}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <span
            class="asa"
            aria-hidden="true"
            onpointerdown={(e) => empezar(e, p, 'tamano')}
            onpointermove={mover}
            onpointerup={soltar}
            onpointercancel={soltar}
          ></span>
        {/if}
      </div>
    {/each}

    {#if aviso}
      <p class="aviso flotante" role="status">{aviso}</p>
    {/if}
  </div>

  {#if abierta && !editando}
    <div
      class="ficha flotante"
      class:abajo={!fichaArriba}
      role="dialog"
      aria-label={abierta.nombre}
      style:left="clamp(116px, {abierta.item.x}%, calc(100% - 116px))"
      style:top="{fichaArriba ? cimaFicha : abierta.base}%"
    >
      <div class="ficha__cabeza">
        <strong>{abierta.nombre}</strong>
        <button type="button" class="ficha__cerrar" aria-label="Cerrar" onclick={() => onficha?.(null)}>
          <Icono nombre="cerrar" tamano={14} />
        </button>
      </div>
      <p class="ficha__detalle">{abierta.detalle}</p>
      <p class="ficha__estado">
        <span class="punto" class:encendido={abierta.estado === 'encendido'} aria-hidden="true"></span>{abierta.estadoTexto}
      </p>
      {#if abierta.bateria !== null}<p class="ficha__detalle">Batería al {abierta.bateria} %</p>{/if}
      {#if abierta.viejo && abierta.visto}<p class="ficha__detalle">Lo último que se vio: {cuando(abierta.visto)}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .marco {
    position: relative;
  }

  /* Colores de la escena, sacados de los tokens: pared y piso fríos, mesa con un toque cálido de madera. */
  .escena {
    --s-pared: color-mix(in srgb, var(--superficie-elevada) 78%, var(--fondo));
    --s-piso: color-mix(in srgb, var(--superficie-hundida) 70%, var(--fondo));
    --s-mesa: color-mix(in srgb, var(--amarillo) 10%, var(--superficie-hover));
    --s-mesa-fondo: color-mix(in srgb, var(--amarillo) 6%, var(--superficie-elevada));
    --s-canto: color-mix(in srgb, var(--amarillo) 8%, var(--superficie-base));
    --s-brillo: var(--luz-tope);
    --s-pata: var(--superficie-hundida);
    --s-zocalo: var(--borde);
    position: relative;
    isolation: isolate;
    aspect-ratio: 16 / 10;
    overflow: hidden;
    border-radius: var(--radio-md);
    user-select: none;
    -webkit-user-select: none;
  }
  .escena.editando {
    touch-action: none;
  }
  .fondo {
    position: absolute;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    box-shadow: inset 0 0 0 1px var(--borde-tarjeta);
    border-radius: inherit;
  }
  .fondo svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  /* La aurora, quieta, sobre la pared detrás del centro de la mesa. */
  .aurora {
    position: absolute;
    inset: 0 0 44% 0;
    background:
      radial-gradient(38% 72% at 50% 100%, var(--aurora-1), transparent),
      radial-gradient(30% 56% at 72% 96%, var(--aurora-2), transparent),
      radial-gradient(26% 44% at 26% 98%, var(--aurora-3), transparent);
  }

  /* Cada cosa: un cuadro con la foto; su borde de abajo visible apoya en `top`. */
  .pieza {
    position: absolute;
    aspect-ratio: 1;
    transform: translate(-50%, calc(var(--abajo) * -100%));
    pointer-events: none;
  }
  .visual {
    position: absolute;
    inset: 0;
    display: block;
    transform: scaleY(var(--aplastar));
    transform-origin: 50% calc(var(--abajo) * 100%);
    transition:
      opacity var(--dur-color) ease,
      filter var(--dur-color) ease;
  }
  .visual img,
  .visual :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .pared .visual {
    filter: drop-shadow(0 6px 8px rgb(0 0 0 / 0.16));
  }
  .apagada .visual {
    opacity: 0.45;
    filter: grayscale(0.55);
  }
  .escena.tenue .pieza {
    opacity: 0.55;
  }

  /* Sombra de apoyo bajo lo visible; lo encendido, un resplandor verde. */
  .apoyo {
    position: absolute;
    left: calc((var(--izq) + (var(--der) - var(--izq)) * 0.06) * 100%);
    width: calc((var(--der) - var(--izq)) * 88%);
    top: calc(var(--abajo) * 100% - (var(--der) - var(--izq)) * 4%);
    height: calc((var(--der) - var(--izq)) * 8%);
    border-radius: 50%;
    background: radial-gradient(closest-side, var(--sombra-apoyo), transparent);
  }
  .apoyo.brillo {
    left: calc((var(--izq) - (var(--der) - var(--izq)) * 0.08) * 100%);
    width: calc((var(--der) - var(--izq)) * 116%);
    top: calc(var(--abajo) * 100% - (var(--der) - var(--izq)) * 8%);
    height: calc((var(--der) - var(--izq)) * 16%);
    background:
      radial-gradient(closest-side, color-mix(in srgb, var(--exito) 55%, transparent), transparent),
      radial-gradient(closest-side, var(--sombra-apoyo), transparent);
  }
  .plano .apoyo {
    display: none;
  }

  /* Lo que se toca: solo lo visible (los márgenes de la foto dejan pasar el clic a lo de atrás). */
  .toque {
    position: absolute;
    left: calc(var(--izq) * 100%);
    top: calc(var(--arriba) * 100%);
    width: calc((var(--der) - var(--izq)) * 100%);
    height: calc((var(--abajo) - var(--arriba)) * 100%);
    padding: 0;
    border: 0;
    border-radius: 10px;
    background: transparent;
    pointer-events: auto;
    cursor: pointer;
    transition: box-shadow var(--dur-color) ease;
  }
  .toque:hover {
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--primario) 40%, transparent);
  }
  .editando .toque {
    cursor: grab;
  }
  .editando .toque:active {
    cursor: grabbing;
  }
  .elegida .toque,
  .elegida .toque:hover {
    box-shadow: 0 0 0 2px var(--primario);
  }
  .toque:focus-visible {
    box-shadow: var(--foco);
  }

  /* Asa para agrandar: en la esquina de arriba a la derecha de lo elegido. */
  .asa {
    position: absolute;
    left: calc(var(--der) * 100%);
    top: calc(var(--arriba) * 100%);
    width: 14px;
    height: 14px;
    translate: -50% -50%;
    border-radius: 50%;
    background: #ffffff;
    box-shadow:
      0 0 0 2px var(--primario),
      0 1px 3px rgb(0 0 0 / 0.3);
    pointer-events: auto;
    cursor: nesw-resize;
  }

  /* Estado: un punto (verde si está encendido) y la batería si se sabe. */
  .estado {
    position: absolute;
    left: 50%;
    top: calc(var(--abajo) * 100%);
    translate: -50% 5px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px;
    border-radius: 99px;
    background: var(--superficie);
    box-shadow: 0 0 0 1px var(--borde-tarjeta);
    font-size: 10.5px;
    line-height: 13px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--texto-2);
    white-space: nowrap;
  }
  .punto {
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--texto-3);
  }
  .encendido .punto,
  .punto.encendido {
    background: var(--exito);
    box-shadow: 0 0 0 2.5px color-mix(in srgb, var(--exito) 22%, transparent);
  }
  .estado.encendido {
    color: var(--texto);
  }

  .aviso {
    position: absolute;
    z-index: 950;
    top: 7%;
    left: 50%;
    translate: -50% 0;
    max-width: 86%;
    margin: 0;
    padding: 7px 12px;
    border-radius: var(--radio-sm);
    font-size: 12.5px;
    line-height: 17px;
    font-weight: 550;
    color: var(--texto);
    text-align: center;
    overflow-wrap: break-word;
  }

  /* Ficha de un aparato del otro equipo. */
  .ficha {
    position: absolute;
    z-index: 960;
    width: 220px;
    translate: -50% calc(-100% - 10px);
    padding: 10px 12px 11px;
    border-radius: var(--radio-sm);
    font-size: 12.5px;
    line-height: 18px;
  }
  .ficha.abajo {
    translate: -50% 26px;
  }
  .ficha__cabeza {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .ficha__cabeza strong {
    flex: 1;
    min-width: 0;
    font-size: 13.5px;
    line-height: 19px;
    font-weight: 650;
    overflow-wrap: break-word;
  }
  .ficha__cerrar {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--texto-3);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .ficha__cerrar:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .ficha p {
    margin: 2px 0 0;
    overflow-wrap: break-word;
  }
  .ficha__detalle {
    color: var(--texto-2);
  }
  .ficha__estado {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }

  /* La torre: el led se enciende como en las ilustraciones. */
  .torre .led {
    fill: var(--grafito-3);
  }
  .torre.encendida .led {
    fill: var(--exito);
  }
</style>
