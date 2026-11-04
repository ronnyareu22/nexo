<!-- Anillo de batería: se llena al aparecer y cambia de color según el nivel (verde, ámbar o rojo).
     La pista va hundida y el centro lleva una cara apenas teñida, para que el número se lea como un dial. -->
<script lang="ts">
  import { onMount } from 'svelte';

  let {
    nivel,
    tamano = 44,
    grosor = 4,
    etiqueta = 'Batería',
    texto = true,
  }: { nivel: number; tamano?: number; grosor?: number; etiqueta?: string; texto?: boolean } = $props();

  const radio = $derived((tamano - grosor) / 2);
  const vuelta = $derived(2 * Math.PI * radio);
  const color = $derived(nivel <= 15 ? 'var(--error)' : nivel < 40 ? 'var(--aviso)' : 'var(--exito)');

  // Empieza vacío y se llena al aparecer.
  let listo = $state(false);
  onMount(() => requestAnimationFrame(() => (listo = true)));
</script>

<div
  class="anillo"
  style:width="{tamano}px"
  style:height="{tamano}px"
  style:--color={color}
  style:--hueco="{grosor + Math.max(3, Math.round(tamano * 0.04))}px"
  role="img"
  aria-label="{etiqueta}: {nivel}%"
>
  {#if texto}<span class="cara" aria-hidden="true"></span>{/if}
  <svg viewBox="0 0 {tamano} {tamano}">
    <circle class="fondo" cx={tamano / 2} cy={tamano / 2} r={radio} stroke-width={grosor} />
    <circle
      class="carga"
      cx={tamano / 2}
      cy={tamano / 2}
      r={radio}
      stroke-width={grosor}
      stroke-dasharray={vuelta}
      stroke-dashoffset={listo ? vuelta * (1 - nivel / 100) : vuelta}
      transform="rotate(-90 {tamano / 2} {tamano / 2})"
    />
  </svg>
  {#if texto}
    <span class="valor" style:font-size="{Math.round(tamano * 0.27)}px">{nivel}<small>%</small></span>
  {/if}
</div>

<style>
  .anillo {
    position: relative;
    flex-shrink: 0;
    display: grid;
    place-items: center;
  }
  /* Cara interior: un disco sólido apenas teñido con el color del nivel. */
  .cara {
    position: absolute;
    inset: var(--hueco);
    border-radius: 50%;
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--color) 9%, var(--superficie-tope)),
      color-mix(in srgb, var(--color) 3%, var(--superficie-base))
    );
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color) 14%, transparent);
  }
  svg {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  circle {
    fill: none;
    stroke-linecap: round;
  }
  .fondo {
    stroke: var(--superficie-hundida);
  }
  .carga {
    stroke: var(--color);
    transition: stroke-dashoffset 900ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .valor {
    position: relative;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.03em;
    color: var(--texto);
  }
  small {
    margin-left: 1px;
    font-size: 0.55em;
    font-weight: 600;
    letter-spacing: 0;
    color: var(--texto-2);
  }
</style>
