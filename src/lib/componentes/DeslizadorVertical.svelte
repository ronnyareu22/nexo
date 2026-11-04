<!--
  Deslizador vertical 0–100 (el mezclador de Audio). Una pista alta y redondeada que se llena desde abajo
  con el color de su familia, con una tapa blanca en el borde del nivel y una escala a un lado.
  - Puntero: se toca para saltar y se arrastra; captura el puntero, así el "soltar" llega aunque sueltes afuera.
  - Teclado: ↑ ↓ de a 1, Re Pág y Av Pág de a 10, Inicio al mínimo y Fin al máximo.
  - Avisa cada valor con `oncambio` y si lo están tocando con `ontocando`; quien lo usa decide cómo enviarlo.
  Solo cambia el alto del relleno: nada se anima por su cuenta.
-->
<script lang="ts">
  let {
    valor,
    etiqueta,
    color = 'primario',
    apagado = false,
    deshabilitado = false,
    escala = true,
    oncambio,
    ontocando,
  }: {
    valor: number;
    etiqueta: string;
    /** primario (turquesa) para el nivel; celeste o amarillo si hace falta otra familia */
    color?: 'primario' | 'celeste' | 'amarillo' | 'verde';
    /** Silenciado: el relleno pasa a gris, pero se puede seguir moviendo. */
    apagado?: boolean;
    /** El aparato no deja cambiarlo: queda quieto y tenue. */
    deshabilitado?: boolean;
    /** Las marcas de 0, 50 y 100 a la izquierda. */
    escala?: boolean;
    oncambio: (v: number) => void;
    ontocando?: (tocando: boolean) => void;
  } = $props();

  let pista = $state<HTMLDivElement>();
  let arrastrando = $state(false);

  const limitar = (v: number) => Math.max(0, Math.min(100, Math.round(v)));
  const parte = $derived(limitar(valor) / 100);

  function cambiar(v: number) {
    const nuevo = limitar(v);
    if (nuevo !== valor) oncambio(nuevo);
  }

  /** El valor bajo el puntero: 100 arriba, 0 abajo. */
  function desde(e: PointerEvent): number {
    if (!pista) return valor;
    const r = pista.getBoundingClientRect();
    return r.height > 0 ? (1 - (e.clientY - r.top) / r.height) * 100 : valor;
  }

  function abajo(e: PointerEvent & { currentTarget: HTMLDivElement }) {
    if (deshabilitado || e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    e.currentTarget.focus({ preventScroll: true });
    arrastrando = true;
    ontocando?.(true);
    cambiar(desde(e));
  }

  function mover(e: PointerEvent) {
    if (arrastrando) cambiar(desde(e));
  }

  function soltar() {
    if (!arrastrando) return;
    arrastrando = false;
    ontocando?.(false);
  }

  function tecla(e: KeyboardEvent) {
    if (deshabilitado) return;
    const pasos: Record<string, number> = { ArrowUp: 1, ArrowRight: 1, ArrowDown: -1, ArrowLeft: -1, PageUp: 10, PageDown: -10 };
    if (e.key in pasos) cambiar(valor + pasos[e.key]);
    else if (e.key === 'Home') cambiar(0);
    else if (e.key === 'End') cambiar(100);
    else return;
    e.preventDefault();
  }
</script>

<div
  class="deslizador"
  class:apagado
  class:deshabilitado
  class:arrastrando
  role="slider"
  tabindex={deshabilitado ? -1 : 0}
  aria-label={etiqueta}
  aria-orientation="vertical"
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={limitar(valor)}
  aria-valuetext={apagado ? `${limitar(valor)} %, silenciado` : `${limitar(valor)} %`}
  aria-disabled={deshabilitado}
  style:--parte={parte}
  style:--relleno="var(--{color})"
  onpointerdown={abajo}
  onpointermove={mover}
  onpointerup={soltar}
  onpointercancel={soltar}
  onlostpointercapture={soltar}
  onkeydown={tecla}
>
  {#if escala}
    <div class="escala" aria-hidden="true">
      {#each [100, 75, 50, 25, 0] as marca (marca)}
        <span class="marca" class:mayor={marca % 50 === 0} style:--en={marca / 100}>
          {#if marca % 50 === 0}<span class="marca__cifra">{marca}</span>{/if}
        </span>
      {/each}
    </div>
  {/if}
  <div class="pista" bind:this={pista}>
    <div class="relleno"></div>
    <span class="tapa"></span>
  </div>
</div>

<style>
  .deslizador {
    position: relative;
    display: flex;
    justify-content: center;
    height: 100%;
    min-height: 0;
    outline: none;
    cursor: ns-resize;
    touch-action: none;
  }
  .deslizador.deshabilitado {
    cursor: not-allowed;
  }

  /* La pista: hundida, alta y redondeada. El relleno crece desde abajo. */
  .pista {
    position: relative;
    flex-shrink: 0;
    width: 60px;
    height: 100%;
    border-radius: 20px;
    background: var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
    overflow: hidden;
    transition: box-shadow var(--dur-color) ease;
  }
  .relleno {
    position: absolute;
    inset: 0;
    background: var(--relleno);
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.35);
    transform: scaleY(var(--parte));
    transform-origin: 50% 100%;
    transition: background-color var(--dur-color) ease;
  }
  .apagado .relleno {
    background: color-mix(in srgb, var(--texto-3) 70%, var(--superficie-hundida));
  }
  .deshabilitado .pista {
    opacity: 0.5;
  }
  .deshabilitado .relleno {
    background: var(--texto-3);
  }

  /* La tapa: una rayita blanca justo bajo el borde del nivel (nunca se sale de la pista). */
  .tapa {
    position: absolute;
    left: 50%;
    bottom: clamp(7px, calc(var(--parte) * 100% - 11px), calc(100% - 11px));
    width: 26px;
    height: 4px;
    margin-left: -13px;
    border-radius: 99px;
    background: #ffffff;
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.25);
  }
  .deshabilitado .tapa {
    display: none;
  }

  .deslizador:focus-visible .pista {
    box-shadow:
      inset 0 0 0 1px var(--borde),
      var(--foco);
  }

  /* La escala: marcas finas a la izquierda de la pista, con 0, 50 y 100. Cuelga al lado: no ensancha nada. */
  .escala {
    position: absolute;
    top: 0;
    bottom: 0;
    right: calc(50% + 30px + 6px);
    width: 30px;
    pointer-events: none;
  }
  .marca {
    position: absolute;
    right: 0;
    bottom: calc(var(--en) * (100% - 2px));
    width: 6px;
    height: 1.5px;
    border-radius: 1px;
    background: var(--borde-fuerte);
  }
  .marca.mayor {
    width: 10px;
    background: var(--texto-3);
  }
  .marca__cifra {
    position: absolute;
    right: 13px;
    top: -7px;
    font-size: 10.5px;
    line-height: 14px;
    font-weight: 600;
    color: var(--texto-3);
    font-variant-numeric: tabular-nums;
  }
  /* En una tira angosta (el contenedor es la barra de la tira del mezclador): primero se van las cifras y luego
     las marcas. */
  @container (max-width: 135px) {
    .marca__cifra {
      display: none;
    }
  }
  @container (max-width: 93px) {
    .escala {
      display: none;
    }
  }
</style>
