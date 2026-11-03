<!-- Gráfica pequeña de la batería en el tiempo (últimas horas). Línea verde con su área suave. -->
<script lang="ts">
  let { muestras }: { muestras: [number, number][] } = $props();

  const ancho = 300;
  const alto = 56;
  const puntos = $derived.by(() => {
    if (muestras.length < 2) return '';
    const t0 = muestras[0][0];
    const t1 = muestras[muestras.length - 1][0];
    const lapso = Math.max(1, t1 - t0);
    // Escala a los valores reales (con 10 puntos de margen) para que se note cuánto bajó.
    const niveles = muestras.map(([, v]) => v);
    const minimo = Math.max(0, Math.min(...niveles) - 10);
    const maximo = Math.min(100, Math.max(...niveles) + 10);
    const rango = Math.max(1, maximo - minimo);
    return muestras
      .map(([t, v]) => `${(((t - t0) / lapso) * ancho).toFixed(1)},${(alto - 4 - ((v - minimo) / rango) * (alto - 8)).toFixed(1)}`)
      .join(' ');
  });
</script>

{#if puntos}
  <svg viewBox="0 0 {ancho} {alto}" preserveAspectRatio="none" role="img" aria-label="Batería en las últimas horas">
    <polygon points="0,{alto} {puntos} {ancho},{alto}" class="area" />
    <polyline points={puntos} class="linea" />
  </svg>
{/if}

<style>
  svg {
    display: block;
    width: 100%;
    height: 56px;
  }
  .area {
    fill: color-mix(in srgb, var(--verde) 14%, transparent);
  }
  .linea {
    fill: none;
    stroke: var(--verde);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
    stroke-linejoin: round;
    stroke-linecap: round;
  }
</style>
