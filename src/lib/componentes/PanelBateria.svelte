<!--
  Tarjeta de batería de la página del aparato: anillo grande, el estado en palabras, cuánto le queda (estimado con
  el historial) y la gráfica de las últimas horas en escala real de 0 a 100 %, con su hora de inicio y "Ahora".
  Con varias baterías (izquierdo, derecho y estuche) van anillos medianos en fila.
  La gráfica ocupa el alto que sobre: si la tarjeta se estira para llenar la página, crece la gráfica, no el vacío.
  Con la tarjeta ancha (una sola columna) el anillo queda a la izquierda y la gráfica a la derecha.
-->
<script lang="ts" module>
  import type { Nombre } from './Icono.svelte';

  /** El nivel en palabras, para el detalle y la página. */
  export function textoBateria(nivel: number) {
    if (nivel >= 95) return 'Carga completa';
    if (nivel >= 60) return 'Batería alta';
    if (nivel >= 36) return 'Batería media';
    if (nivel >= 16) return 'Batería baja: cárgalo pronto';
    return 'Batería muy baja: cárgalo ya';
  }

  /** Mismo color que el anillo: verde, ámbar o rojo. */
  export const colorBateria = (nivel: number) => (nivel <= 15 ? 'var(--error)' : nivel < 40 ? 'var(--aviso)' : 'var(--exito)');

  /** El rayo solo con la carga completa; si no, la pila. */
  export const iconoBateria = (nivel: number): Nombre => (nivel >= 95 ? 'rayo' : 'bateria');
</script>

<script lang="ts">
  import type { Registro } from '$lib/tipos';
  import { ahora, cuando, hora, horasRestantes } from '$lib/tiempo';
  import Anillo from './Anillo.svelte';
  import Icono from './Icono.svelte';

  let {
    baterias,
    registro,
  }: { baterias: { nombre: string; nivel: number }[]; registro: Registro | null } = $props();

  const id = $props.id();

  // Se recalcula cada 30 s para que "hace 5 min" avance solo.
  let reloj = $state(ahora());
  $effect(() => {
    const t = setInterval(() => (reloj = ahora()), 30_000);
    return () => clearInterval(t);
  });

  const muestras = $derived((registro?.bateria ?? []).filter(([t]) => t >= Date.now() / 1000 - 12 * 3600));
  const quedan = $derived(horasRestantes(registro?.bateria ?? []));
  // Estimación en números redondos: es un cálculo, no una promesa.
  const estimacion = $derived.by(() => {
    if (quedan === null) return null;
    if (quedan >= 1.5) return `Quedan unas ${Math.round(quedan)} h de uso`;
    if (quedan >= 1) return 'Queda cerca de 1 h de uso';
    return `Quedan unos ${Math.max(1, Math.round(quedan * 60))} min de uso`;
  });
  const menor = $derived(Math.min(...baterias.map((b) => b.nivel)));
  const ultima = $derived(registro?.bateria.length ? registro.bateria[registro.bateria.length - 1][0] : null);
  const subtitulo = $derived.by(() => (void reloj, ultima ? `Última lectura ${cuando(ultima)}` : 'La informa el propio aparato'));

  // --- Gráfica: tiempo en x (de la primera lectura a la última) y nivel real en y (0 a 100 %). ---
  const trazo = $derived.by(() => {
    if (muestras.length < 2) return null;
    const t0 = muestras[0][0];
    const t1 = muestras[muestras.length - 1][0];
    const lapso = Math.max(1, t1 - t0);
    const puntos = muestras.map(([t, v]) => `${(((t - t0) / lapso) * 100).toFixed(2)},${(100 - v).toFixed(2)}`).join(' ');
    return {
      puntos,
      inicio: t0,
      primero: muestras[0][1],
      ultimo: muestras[muestras.length - 1][1],
    };
  });
</script>

<section class="panel vidrio" aria-labelledby="{id}-titulo">
  <header class="t-cabeza">
    <span class="ficha-icono" style:--tono="var(--exito)"><Icono nombre="bateria" tamano={19} /></span>
    <div class="t-textos">
      <h2 class="t-titulo" id="{id}-titulo">Batería</h2>
      <p class="t-sub">{subtitulo}</p>
    </div>
  </header>

  <div class="cuerpo">
    {#if baterias.length === 1}
      {@const nivel = baterias[0].nivel}
      <div class="principal">
        <Anillo {nivel} tamano={104} grosor={9} />
        <div class="textos">
          <div class="estado">
            <span class="icono" style:color={colorBateria(nivel)}><Icono nombre={iconoBateria(nivel)} tamano={18} /></span>
            {textoBateria(nivel)}
          </div>
          <div class="quedan">
            <Icono nombre="reloj" tamano={15} />
            <span>{estimacion ?? 'Nexo calcula cuánto le queda después de un rato de uso'}</span>
          </div>
        </div>
      </div>
    {:else}
      <div class="varias">
        <div class="anillos">
          {#each baterias as b (b.nombre)}
            <div class="bateria">
              <Anillo nivel={b.nivel} tamano={76} grosor={7} etiqueta={b.nombre} />
              <span>{b.nombre}</span>
            </div>
          {/each}
        </div>
        <div class="estado chico">
          <span class="icono" style:color={colorBateria(menor)}><Icono nombre={iconoBateria(menor)} tamano={17} /></span>
          {textoBateria(menor)}
        </div>
        <div class="quedan">
          <Icono nombre="reloj" tamano={15} />
          <span>{estimacion ?? 'Nexo calcula cuánto le queda después de un rato de uso'}</span>
        </div>
      </div>
    {/if}

    <div class="grafica">
      <div class="grafica__cabeza">
        <span class="rotulo">Últimas horas</span>
        {#if trazo}<span class="rango">{trazo.primero} % → {trazo.ultimo} %</span>{/if}
      </div>
      {#if trazo}
        <div class="lienzo" role="img" aria-label="Batería en las últimas horas: de {trazo.primero} % a {trazo.ultimo} %">
          <span class="guia" style:top="0%"><b>100</b></span>
          <span class="guia" style:top="50%"><b>50</b></span>
          <span class="guia base" style:top="100%"><b>0</b></span>
          <svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
            <defs>
              <linearGradient id="{id}-area" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0" style:stop-color="var(--exito)" stop-opacity="0.26" />
                <stop offset="1" style:stop-color="var(--exito)" stop-opacity="0" />
              </linearGradient>
            </defs>
            <polygon points="0,100 {trazo.puntos} 100,100" fill="url(#{id}-area)" />
            <polyline points={trazo.puntos} class="linea" />
          </svg>
          <span class="fin" style:top="{100 - trazo.ultimo}%" aria-hidden="true"></span>
        </div>
        <div class="ejes" aria-hidden="true">
          <span>{hora(trazo.inicio)}</span>
          <span>Ahora</span>
        </div>
      {:else}
        <div class="sin-datos">
          <Icono nombre="onda" tamano={18} />
          <span>La gráfica aparece después de un rato de uso.</span>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .panel {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
    border-radius: var(--radio-lg);
  }
  .cuerpo {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .principal {
    display: flex;
    align-items: center;
    gap: 18px;
  }
  .textos {
    flex: 1 1 0;
    min-width: 0;
    display: grid;
    gap: 8px;
  }
  .estado {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 17px;
    line-height: 23px;
    font-weight: 700;
    letter-spacing: -0.015em;
  }
  .estado.chico {
    font-size: 15px;
    line-height: 21px;
    font-weight: 650;
  }
  .icono {
    display: grid;
    flex-shrink: 0;
  }
  .quedan {
    display: flex;
    align-items: flex-start;
    gap: 7px;
    font: var(--letra-pequena);
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .quedan :global(svg) {
    flex-shrink: 0;
    margin-top: 1.5px;
    color: var(--texto-3);
  }
  .varias {
    display: grid;
    gap: 10px;
  }
  .anillos {
    display: flex;
    flex-wrap: wrap;
    gap: 20px;
    margin-bottom: 4px;
  }
  .bateria {
    display: grid;
    justify-items: center;
    gap: 8px;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--texto-2);
  }

  /* ---------- Gráfica ---------- */
  .grafica {
    flex: 1 1 auto;
    min-height: 116px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 14px 10px;
    border-radius: var(--radio-md);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .grafica__cabeza {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }
  .rango {
    font-size: 12px;
    line-height: 16px;
    font-weight: 600;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  /* El lienzo deja 28 px a la izquierda para las guías (100, 50, 0). */
  .lienzo {
    position: relative;
    flex: 1 1 auto;
    min-height: 64px;
    margin: 6px 4px 0 28px;
  }
  .lienzo svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .linea {
    fill: none;
    stroke: var(--exito);
    stroke-width: 2.25;
    vector-effect: non-scaling-stroke;
    stroke-linejoin: round;
    stroke-linecap: round;
  }
  .guia {
    position: absolute;
    left: 0;
    right: 0;
    height: 0;
    border-top: 1px dashed color-mix(in srgb, var(--texto-3) 35%, transparent);
  }
  .guia.base {
    border-top-style: solid;
  }
  .guia b {
    position: absolute;
    right: calc(100% + 8px);
    top: -8px;
    font-size: 10.5px;
    line-height: 16px;
    font-weight: 600;
    color: var(--texto-3);
    font-variant-numeric: tabular-nums;
  }
  .fin {
    position: absolute;
    left: 100%;
    width: 9px;
    height: 9px;
    margin: -4.5px 0 0 -4.5px;
    border-radius: 50%;
    background: var(--exito);
    box-shadow:
      0 0 0 2px var(--superficie-inserto),
      0 0 0 5px color-mix(in srgb, var(--exito) 22%, transparent);
  }
  .ejes {
    display: flex;
    justify-content: space-between;
    margin-left: 28px;
    font-size: 11px;
    line-height: 16px;
    font-weight: 500;
    color: var(--texto-3);
    font-variant-numeric: tabular-nums;
  }
  .sin-datos {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 12px;
    text-align: center;
    font: var(--letra-pequena);
    color: var(--texto-2);
  }
  .sin-datos :global(svg) {
    color: var(--texto-3);
  }

  /* Tarjeta ancha (una sola columna): el anillo a la izquierda y la gráfica a la derecha. */
  @container (min-width: 620px) {
    .cuerpo {
      flex-direction: row;
      align-items: stretch;
    }
    .principal,
    .varias {
      flex: 0 1 300px;
      align-self: center;
    }
    .grafica {
      flex: 1 1 0;
    }
  }
</style>
