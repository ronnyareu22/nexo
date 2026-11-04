<!--
  Tendencia de los últimos 5 minutos (Analítica): SVG a mano, sin librerías.
  Línea de 2 px con un área suave debajo (la serie principal) y, si hay, una segunda línea sin área.
  El tiempo va fijo de "hace 5 min" a "ahora": si recién se empezó a medir, la línea nace a la derecha.
  Si faltan muestras (el equipo dejó de medir un rato), la línea se corta ahí.
  Al pasar el mouse (o con las flechas, con el foco puesto) una línea vertical sigue la muestra más cercana y
  un recuadro dice cada valor y hace cuánto fue. Los puntos van en HTML para que no se deformen.
  Los colores salen de --serie-1 y --serie-2 (los pone la vista).
-->
<script lang="ts">
  export interface Serie {
    nombre: string;
    valores: (number | null)[];
    /** 'principal': línea con área; 'secundaria': solo línea. */
    tipo: 'principal' | 'secundaria';
  }

  let {
    momentos,
    series,
    ahora,
    formato,
    etiqueta,
    maximo = null,
    techoMinimo = 1,
    alto = 44,
  }: {
    momentos: number[];
    series: Serie[];
    /** El momento de la última medición (ms, el mismo reloj que `momentos`). */
    ahora: number;
    formato: (v: number) => string;
    /** Lo que dice el lector de pantalla. */
    etiqueta: string;
    /** Techo fijo (100 para porcentajes); sin él, el mayor valor con un poco de aire. */
    maximo?: number | null;
    /** Sin techo fijo, el techo nunca baja de esto (así lo quieto no parece mucho). */
    techoMinimo?: number;
    alto?: number;
  } = $props();

  const ANCHO = 300;
  const VENTANA = 5 * 60 * 1000;
  const ARRIBA = 3;
  const ABAJO = 1.5;

  const desde = $derived(ahora - VENTANA);
  const techo = $derived.by(() => {
    if (maximo !== null) return maximo;
    let mayor = 0;
    for (const s of series) for (const v of s.valores) if (v !== null && v > mayor) mayor = v;
    return Math.max(techoMinimo, mayor * 1.15);
  });

  const xDe = (t: number) => Math.min(ANCHO, Math.max(0, ((t - desde) / VENTANA) * ANCHO));
  const yDe = (v: number) => alto - ABAJO - (Math.min(v, techo) / techo) * (alto - ARRIBA - ABAJO);

  // Cada serie en tramos sin cortes: dos muestras a más de 3 pasos de distancia no se unen.
  const paso = $derived(momentos.length > 1 ? Math.max(1000, (momentos[momentos.length - 1] - momentos[0]) / (momentos.length - 1)) : 2000);
  function tramos(valores: (number | null)[]): [number, number][][] {
    const salida: [number, number][][] = [];
    let actual: [number, number][] = [];
    let anterior: number | null = null;
    momentos.forEach((t, i) => {
      const v = valores[i];
      if (t < desde || v === null || v === undefined) {
        if (actual.length) salida.push(actual);
        actual = [];
        anterior = null;
        return;
      }
      if (anterior !== null && t - anterior > paso * 3.5 && actual.length) {
        salida.push(actual);
        actual = [];
      }
      actual.push([xDe(t), yDe(v)]);
      anterior = t;
    });
    if (actual.length) salida.push(actual);
    return salida;
  }

  const dibujos = $derived(
    series.map((s) => {
      const partes = tramos(s.valores);
      const linea = partes
        .map((p) => (p.length === 1 ? `M${p[0][0]},${p[0][1]}h0.01` : p.map(([x, y], i) => `${i ? 'L' : 'M'}${x.toFixed(1)},${y.toFixed(1)}`).join('')))
        .join('');
      const area =
        s.tipo === 'principal'
          ? partes
              .filter((p) => p.length > 1)
              .map((p) => `M${p[0][0].toFixed(1)},${alto}${p.map(([x, y]) => `L${x.toFixed(1)},${y.toFixed(1)}`).join('')}L${p[p.length - 1][0].toFixed(1)},${alto}Z`)
              .join('')
          : '';
      return { s, linea, area };
    }),
  );

  // El último punto de cada serie (si es de esta ventana).
  const ultimos = $derived(
    series.map((s) => {
      for (let i = momentos.length - 1; i >= 0; i--) {
        const v = s.valores[i];
        if (v !== null && v !== undefined && momentos[i] >= desde) return { s, x: xDe(momentos[i]), y: yDe(v) };
      }
      return null;
    }),
  );

  // --- Lectura al pasar el mouse o con las flechas ---
  let elegido = $state<number | null>(null);
  let caja = $state<HTMLDivElement | undefined>();

  const visibles = $derived(momentos.map((t, i) => (t >= desde ? i : -1)).filter((i) => i >= 0));

  function cercano(fraccion: number): number | null {
    if (!visibles.length) return null;
    const t = desde + fraccion * VENTANA;
    let mejor = visibles[0];
    for (const i of visibles) if (Math.abs(momentos[i] - t) < Math.abs(momentos[mejor] - t)) mejor = i;
    return mejor;
  }

  function alMover(e: PointerEvent) {
    if (!caja) return;
    const r = caja.getBoundingClientRect();
    if (r.width <= 0) return;
    elegido = cercano(Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)));
  }

  function alTecla(e: KeyboardEvent) {
    if (!visibles.length) return;
    const pos = elegido === null ? visibles.length - 1 : visibles.indexOf(elegido);
    if (e.key === 'ArrowLeft') elegido = visibles[Math.max(0, pos - 1)];
    else if (e.key === 'ArrowRight') elegido = visibles[Math.min(visibles.length - 1, pos + 1)];
    else if (e.key === 'Escape') elegido = null;
    else return;
    e.preventDefault();
  }

  function haceCuanto(t: number): string {
    const s = Math.max(0, Math.round((ahora - t) / 1000));
    if (s < 3) return 'Ahora';
    if (s < 60) return `Hace ${s} s`;
    const m = Math.floor(s / 60);
    const r = s % 60;
    return r ? `Hace ${m} min ${r} s` : `Hace ${m} min`;
  }

  // Los oyentes van en una acción: el lienzo es una imagen para el lector de pantalla, con lectura opcional.
  function seguir(nodo: HTMLDivElement) {
    const salir = () => (elegido = null);
    nodo.addEventListener('pointermove', alMover);
    nodo.addEventListener('pointerleave', salir);
    nodo.addEventListener('keydown', alTecla);
    nodo.addEventListener('blur', salir);
    return {
      destroy() {
        nodo.removeEventListener('pointermove', alMover);
        nodo.removeEventListener('pointerleave', salir);
        nodo.removeEventListener('keydown', alTecla);
        nodo.removeEventListener('blur', salir);
      },
    };
  }

  const lectura = $derived.by(() => {
    if (elegido === null || elegido >= momentos.length) return null;
    const i = elegido;
    const x = xDe(momentos[i]);
    return {
      x,
      cuando: haceCuanto(momentos[i]),
      filas: series.map((s) => {
        const v = s.valores[i];
        return { s, texto: v === null || v === undefined ? 'Sin dato' : formato(v), y: v === null || v === undefined ? null : yDe(v) };
      }),
    };
  });

  const resumen = $derived.by(() => {
    const s = series[0];
    const vals = s ? (s.valores.filter((v, i) => v !== null && momentos[i] >= desde) as number[]) : [];
    if (!vals.length) return `${etiqueta}: todavía sin datos`;
    return `${etiqueta}, últimos 5 minutos: entre ${formato(Math.min(...vals))} y ${formato(Math.max(...vals))}`;
  });
</script>

<div class="tendencia">
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="lienzo"
    bind:this={caja}
    style:height="{alto}px"
    role="img"
    aria-label={resumen}
    tabindex="0"
    use:seguir
  >
    <svg viewBox="0 0 {ANCHO} {alto}" preserveAspectRatio="none" aria-hidden="true">
      <line class="base" x1="0" x2={ANCHO} y1={alto - 0.5} y2={alto - 0.5} />
      {#each dibujos as d (d.s.nombre)}
        {#if d.area}<path class="area" d={d.area} />{/if}
        <path class="linea {d.s.tipo}" d={d.linea} />
      {/each}
    </svg>

    {#each ultimos as u, i (i)}
      {#if u && !lectura}
        <span class="punto {u.s.tipo}" style:left="{(u.x / ANCHO) * 100}%" style:top="{u.y}px"></span>
      {/if}
    {/each}

    {#if lectura}
      <span class="guia" style:left="{(lectura.x / ANCHO) * 100}%"></span>
      {#each lectura.filas as f (f.s.nombre)}
        {#if f.y !== null}
          <span class="punto {f.s.tipo}" style:left="{(lectura.x / ANCHO) * 100}%" style:top="{f.y}px"></span>
        {/if}
      {/each}
      <div class="lectura flotante" class:derecha={lectura.x > ANCHO / 2} style:left="{(lectura.x / ANCHO) * 100}%">
        {#each lectura.filas as f (f.s.nombre)}
          <span class="lectura__fila">
            <span class="clave {f.s.tipo}" aria-hidden="true"></span>
            <strong>{f.texto}</strong>
            {#if series.length > 1}<span class="lectura__nombre">{f.s.nombre}</span>{/if}
          </span>
        {/each}
        <span class="lectura__cuando">{lectura.cuando}</span>
      </div>
    {/if}
  </div>
  <div class="eje" aria-hidden="true">
    <span>Hace 5 min</span>
    <span>Ahora</span>
  </div>
</div>

<style>
  .tendencia {
    display: grid;
    gap: 4px;
    min-width: 0;
  }
  .lienzo {
    position: relative;
    border-radius: 4px;
    outline: none;
    cursor: crosshair;
    touch-action: none;
  }
  .lienzo:focus-visible {
    box-shadow: var(--foco);
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .base {
    stroke: var(--borde);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }
  .area {
    fill: color-mix(in srgb, var(--serie-1) 12%, transparent);
  }
  .linea {
    fill: none;
    stroke-width: 2;
    stroke-linejoin: round;
    stroke-linecap: round;
    vector-effect: non-scaling-stroke;
  }
  .linea.principal {
    stroke: var(--serie-1);
  }
  .linea.secundaria {
    stroke: var(--serie-2);
  }

  /* Puntos de 8 px con un aro del color de la tarjeta, para que se lean sobre la línea. */
  .punto {
    position: absolute;
    width: 8px;
    height: 8px;
    margin: -4px 0 0 -4px;
    border-radius: 50%;
    box-shadow: 0 0 0 2px var(--superficie-tope);
    pointer-events: none;
  }
  .punto.principal {
    background: var(--serie-1);
  }
  .punto.secundaria {
    background: var(--serie-2);
  }
  .guia {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    margin-left: -0.5px;
    background: var(--borde-fuerte);
    pointer-events: none;
  }

  /* El recuadro va arriba del gráfico, del lado donde haya espacio. */
  .lectura {
    position: absolute;
    z-index: 5;
    bottom: calc(100% + 8px);
    display: grid;
    gap: 3px;
    min-width: max-content;
    padding: 7px 10px;
    border-radius: 10px;
    font-size: 12px;
    line-height: 16px;
    color: var(--texto-2);
    pointer-events: none;
    translate: -12px 0;
  }
  .lectura.derecha {
    translate: calc(-100% + 12px) 0;
  }
  .lectura__fila {
    display: flex;
    align-items: center;
    gap: 7px;
    white-space: nowrap;
  }
  .lectura strong {
    font-weight: 650;
    color: var(--texto);
    font-variant-numeric: tabular-nums;
  }
  .clave {
    width: 12px;
    height: 2px;
    border-radius: 2px;
  }
  .clave.principal {
    background: var(--serie-1);
  }
  .clave.secundaria {
    background: var(--serie-2);
  }
  .lectura__cuando {
    color: var(--texto-3);
    font-size: 11.5px;
  }

  .eje {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    line-height: 14px;
    color: var(--texto-3);
  }
</style>
