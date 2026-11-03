<!--
  Panel de batería de la página del aparato: anillo grande, el estado en palabras con su ícono, cuánto le queda
  (estimado con el historial) y, si hay al menos dos lecturas, la gráfica de las últimas horas.
  Con varias baterías (izquierdo, derecho y estuche) van anillos chicos en fila.
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
  import { horasRestantes } from '$lib/tiempo';
  import Anillo from './Anillo.svelte';
  import Grafica from './Grafica.svelte';
  import Icono from './Icono.svelte';

  let {
    baterias,
    registro,
  }: { baterias: { nombre: string; nivel: number }[]; registro: Registro | null } = $props();

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
</script>

<section class="panel vidrio">
  <h2>Batería</h2>

  {#if baterias.length === 1}
    {@const nivel = baterias[0].nivel}
    <div class="principal">
      <Anillo {nivel} tamano={96} grosor={8} />
      <div class="textos">
        <div class="estado">
          <span class="icono" style:color={colorBateria(nivel)}><Icono nombre={iconoBateria(nivel)} tamano={20} /></span>
          {textoBateria(nivel)}
        </div>
        <div class="quedan">
          <Icono nombre="reloj" tamano={15} />
          <span>{estimacion ?? 'Nexo calcula cuánto le queda después de un rato de uso'}</span>
        </div>
      </div>
    </div>
  {:else}
    <div class="anillos">
      {#each baterias as b (b.nombre)}
        <div class="bateria">
          <Anillo nivel={b.nivel} tamano={72} grosor={6} etiqueta={b.nombre} />
          <span>{b.nombre}</span>
        </div>
      {/each}
    </div>
    <div class="estado chico">
      <span class="icono" style:color={colorBateria(menor)}><Icono nombre={iconoBateria(menor)} tamano={18} /></span>
      {textoBateria(menor)}
    </div>
    <div class="quedan">
      <Icono nombre="reloj" tamano={15} />
      <span>{estimacion ?? 'Nexo calcula cuánto le queda después de un rato de uso'}</span>
    </div>
  {/if}

  {#if muestras.length >= 2}
    <div class="grafica">
      <div class="rotulo-grafica">Últimas horas</div>
      <Grafica {muestras} />
    </div>
  {/if}
</section>

<style>
  .panel {
    border-radius: var(--radio-lg);
    padding: 18px 20px;
  }
  h2 {
    margin: 0 0 14px;
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
  }
  .principal {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 14px 20px;
  }
  .textos {
    flex: 1 1 180px;
    min-width: 0;
    display: grid;
    gap: 6px;
  }
  .estado {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 17px;
    line-height: 24px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .estado.chico {
    margin-top: 16px;
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
  }
  .icono {
    display: grid;
    flex-shrink: 0;
  }
  .quedan {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .quedan :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
    color: var(--texto-3);
  }
  .estado.chico + .quedan {
    margin-top: 4px;
  }
  .anillos {
    display: flex;
    flex-wrap: wrap;
    gap: 22px;
  }
  .bateria {
    display: grid;
    justify-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--texto-2);
  }
  .grafica {
    margin-top: 16px;
    padding-top: 14px;
    border-top: 1px solid var(--borde);
  }
  .rotulo-grafica {
    margin-bottom: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--texto-2);
  }
</style>
