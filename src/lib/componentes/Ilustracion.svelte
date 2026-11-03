<!--
  Ilustración de cada tipo de aparato, en grafito con reflejos suaves, al estilo de Logi Options+.
  `encendido` enciende el led de color; `marca` agrega detalles (Beats en rojo).
-->
<script lang="ts">
  import type { Tipo } from '$lib/tipos';

  let {
    tipo,
    marca = null,
    encendido = false,
  }: { tipo: Tipo; marca?: string | null; encendido?: boolean } = $props();

  const uid = $props.id();
  const cuerpo = `cuerpo-${uid}`;
  const brillo = `brillo-${uid}`;
  const pantalla = `pantalla-${uid}`;
  const sombra = `sombra-${uid}`;
  const esBeats = $derived(marca === 'Beats');
</script>

<svg viewBox="0 0 200 200" class="ilustracion" class:encendido role="img" aria-hidden="true">
  <defs>
    <linearGradient id={cuerpo} x1="0" y1="0" x2="0.4" y2="1">
      <stop offset="0" stop-color="var(--grafito-1)" />
      <stop offset="0.55" stop-color="var(--grafito-2)" />
      <stop offset="1" stop-color="var(--grafito-3)" />
    </linearGradient>
    <linearGradient id={brillo} x1="0" y1="0" x2="1" y2="0">
      <stop offset="0" stop-color="#fff" stop-opacity="0.28" />
      <stop offset="1" stop-color="#fff" stop-opacity="0" />
    </linearGradient>
    <!-- Pantalla encendida en un tono sobrio (sin degradados de colores). -->
    <linearGradient id={pantalla} x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#2a4a63" />
      <stop offset="1" stop-color="#16283a" />
    </linearGradient>
    <radialGradient id={sombra} cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="#000" stop-opacity="0.45" />
      <stop offset="1" stop-color="#000" stop-opacity="0" />
    </radialGradient>
  </defs>

  <ellipse cx="100" cy="182" rx="64" ry="8" fill="url(#{sombra})" />

  {#if tipo === 'audifonos' || tipo === 'diadema'}
    {@const sobreOreja = tipo === 'diadema'}
    <!-- Diadema -->
    <path d="M44 116 C44 46, 156 46, 156 116" fill="none" stroke="url(#{cuerpo})" stroke-width="13" stroke-linecap="round" />
    <path d="M55 106 C58 66, 142 66, 145 106" fill="none" stroke="var(--grafito-3)" stroke-width="5" stroke-linecap="round" opacity="0.8" />
    <path d="M52 92 C60 62, 92 54, 112 56" fill="none" stroke="url(#{brillo})" stroke-width="3" stroke-linecap="round" />
    <!-- Copas -->
    {#each [{ x: 26, izquierda: true }, { x: 138, izquierda: false }] as copa (copa.x)}
      {@const ancho = sobreOreja ? 36 : 38}
      {@const alto = sobreOreja ? 52 : 66}
      {@const y = sobreOreja ? 108 : 100}
      <!-- La almohadilla va detrás de la copa y asoma por el lado de la oreja. -->
      <rect x={copa.izquierda ? copa.x + ancho - 10 : copa.x - 8} y={y + 5} width="18" height={alto - 10} rx="8" fill="var(--grafito-3)" />
      <rect x={copa.x} {y} width={ancho} height={alto} rx={ancho / 2} fill="url(#{cuerpo})" />
      <rect x={copa.x + 6} y={y + 6} width="8" height={alto - 22} rx="4" fill="url(#{brillo})" />
    {/each}
    {#if esBeats}
      <circle cx="45" cy={sobreOreja ? 134 : 133} r="10" fill="#e5333b" />
      <circle cx="45" cy={sobreOreja ? 134 : 133} r="4.5" fill="none" stroke="#fff" stroke-width="2" />
    {/if}
    {#if sobreOreja}
      <circle cx="157" cy="150" r="2.4" fill="var(--grafito-3)" />
    {/if}
    <circle class="led" cx="157" cy={sobreOreja ? 140 : 150} r="3" />
  {:else if tipo === 'telefono'}
    <rect x="64" y="22" width="72" height="150" rx="17" fill="url(#{cuerpo})" />
    <rect x="69" y="27" width="62" height="140" rx="13" fill="url(#{pantalla})" opacity="0.95" />
    <rect x="69" y="27" width="62" height="140" rx="13" fill="url(#{brillo})" opacity="0.5" />
    <rect x="87" y="34" width="26" height="8" rx="4" fill="#06080b" />
    <circle class="led" cx="100" cy="158" r="0" />
  {:else if tipo === 'teclado'}
    <rect x="18" y="78" width="164" height="58" rx="11" fill="url(#{cuerpo})" />
    {#each [0, 1, 2] as fila (fila)}
      {#each Array.from({ length: 11 }, (_, i) => i) as col (col)}
        <rect x={27 + col * 13.6} y={86 + fila * 14} width="10.5" height="10" rx="2.5" fill="var(--grafito-3)" />
      {/each}
    {/each}
    <rect x="58" y="128" width="84" height="5" rx="2.5" fill="var(--grafito-3)" />
    <circle class="led" cx="170" cy="84" r="2.6" />
  {:else if tipo === 'mouse'}
    <path d="M100 34 C130 34, 136 64, 136 104 C136 146, 122 166, 100 166 C78 166, 64 146, 64 104 C64 64, 70 34, 100 34Z" fill="url(#{cuerpo})" />
    <path d="M100 36 V86" stroke="var(--grafito-3)" stroke-width="2.5" />
    <rect x="96" y="52" width="8" height="20" rx="4" fill="var(--grafito-3)" />
    <path d="M74 66 C76 50, 86 42, 96 40" fill="none" stroke="url(#{brillo})" stroke-width="4" stroke-linecap="round" />
    <circle class="led" cx="100" cy="150" r="2.6" />
  {:else if tipo === 'parlante'}
    <rect x="60" y="34" width="80" height="134" rx="26" fill="url(#{cuerpo})" />
    <circle cx="100" cy="76" r="18" fill="var(--grafito-3)" />
    <circle cx="100" cy="76" r="8" fill="var(--grafito-2)" />
    <circle cx="100" cy="128" r="26" fill="var(--grafito-3)" />
    <circle cx="100" cy="128" r="12" fill="var(--grafito-2)" />
    <rect x="68" y="44" width="8" height="70" rx="4" fill="url(#{brillo})" />
    <circle class="led" cx="100" cy="46" r="2.6" />
  {:else if tipo === 'monitor' || tipo === 'luz'}
    {#if tipo === 'luz'}
      <path d="M58 40 L142 40 L176 120 L24 120 Z" fill="#ffd27a" opacity="0.12" />
      <rect x="54" y="30" width="92" height="9" rx="4.5" fill="url(#{cuerpo})" />
      <rect x="60" y="37" width="80" height="3" rx="1.5" class="luz-barra" />
    {/if}
    <rect x="24" y="44" width="152" height="98" rx="9" fill="url(#{cuerpo})" />
    <rect x="30" y="50" width="140" height="84" rx="5" fill="url(#{pantalla})" opacity="0.9" />
    <rect x="30" y="50" width="140" height="84" rx="5" fill="url(#{brillo})" opacity="0.45" />
    <path d="M88 142 L84 166 H116 L112 142 Z" fill="url(#{cuerpo})" />
    <rect x="66" y="164" width="68" height="7" rx="3.5" fill="url(#{cuerpo})" />
    <circle class="led" cx="166" cy="138" r="1.8" />
  {:else if tipo === 'portatil'}
    <rect x="34" y="42" width="132" height="88" rx="9" fill="url(#{cuerpo})" />
    <rect x="40" y="48" width="120" height="76" rx="4" fill="url(#{pantalla})" opacity="0.9" />
    <rect x="40" y="48" width="120" height="76" rx="4" fill="url(#{brillo})" opacity="0.45" />
    <rect x="91" y="48" width="18" height="5" rx="2.5" fill="#06080b" />
    <path d="M18 134 H182 L174 146 H26 Z" fill="url(#{cuerpo})" />
    <rect x="86" y="134" width="28" height="4" rx="2" fill="var(--grafito-3)" />
    <circle class="led" cx="100" cy="150" r="0" />
  {:else}
    <rect x="56" y="56" width="88" height="88" rx="24" fill="url(#{cuerpo})" />
    <path d="M86 84 L114 112 L100 124 V76 L114 88 L86 116" fill="none" stroke="var(--texto-suave)" stroke-width="5" stroke-linecap="round" stroke-linejoin="round" />
    <circle class="led" cx="132" cy="68" r="2.8" />
  {/if}
</svg>

<style>
  .ilustracion {
    width: 100%;
    height: 100%;
    overflow: visible;
    transition:
      filter var(--rapido),
      opacity var(--rapido);
  }
  .led {
    fill: var(--grafito-3);
    transition: fill var(--rapido);
  }
  .encendido .led {
    fill: var(--exito);
  }
  .luz-barra {
    fill: #fff3c4;
  }
</style>
