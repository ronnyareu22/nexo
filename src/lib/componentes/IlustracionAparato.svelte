<!--
  Ilustración de un aparato de la casa, en el mismo grafito que las de los aparatos Bluetooth (Ilustracion.svelte).
  - Luces (tira, foco, lámpara de monitor, lámpara): encendidas, brillan con su color real (`color`) y un resplandor
    quieto alrededor; apagadas, el vidrio queda gris.
  - Enchufe (led verde), tele (pantalla encendida), cerradura y puerta (cerradas o abiertas), cortina (cuánto está
    abierta), clima y "otro".
  Sin animaciones: solo cambia cuando cambia el estado. Es decorativa (el texto lo pone quien la usa).
-->
<script lang="ts">
  import type { ClaseAparato, FormaLuz } from '$lib/tipos-hogar';

  let {
    clase,
    forma = null,
    encendido = false,
    color = null,
    abierto = null,
    posicion = null,
    enMapa = false,
  }: {
    clase: ClaseAparato;
    forma?: FormaLuz | null;
    encendido?: boolean;
    /** El color de la luz encendida (CSS). */
    color?: string | null;
    abierto?: boolean | null;
    /** Cortina: 0–100. */
    posicion?: number | null;
    /** En el mapa de Mi setup: la barra va sin su monitor (el monitor ya está dibujado). */
    enMapa?: boolean;
  } = $props();

  const uid = $props.id();
  const cuerpo = `ap-cuerpo-${uid}`;
  const halo = `ap-halo-${uid}`;
  const cono = `ap-cono-${uid}`;
  const sombra = `ap-sombra-${uid}`;
  const luz = $derived(color ?? '#ffd59a');
  const prendida = $derived(clase === 'luz' && encendido);
  const abierta = $derived(posicion !== null ? posicion / 100 : abierto ? 1 : 0);
</script>

<svg viewBox="0 0 200 200" class="ilus" class:prendida style:--luz={luz} role="img" aria-hidden="true">
  <defs>
    <linearGradient id={cuerpo} x1="0" y1="0" x2="0.4" y2="1">
      <stop offset="0" stop-color="var(--grafito-1)" />
      <stop offset="0.55" stop-color="var(--grafito-2)" />
      <stop offset="1" stop-color="var(--grafito-3)" />
    </linearGradient>
    <radialGradient id={halo} cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" style="stop-color: var(--luz); stop-opacity: 0.55" />
      <stop offset="0.55" style="stop-color: var(--luz); stop-opacity: 0.16" />
      <stop offset="1" style="stop-color: var(--luz); stop-opacity: 0" />
    </radialGradient>
    <linearGradient id={cono} x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" style="stop-color: var(--luz); stop-opacity: 0.5" />
      <stop offset="1" style="stop-color: var(--luz); stop-opacity: 0" />
    </linearGradient>
    <radialGradient id={sombra} cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="#000" stop-opacity="0.35" />
      <stop offset="1" stop-color="#000" stop-opacity="0" />
    </radialGradient>
  </defs>

  {#if clase === 'luz' && forma === 'tira'}
    {#if prendida}<ellipse cx="100" cy="100" rx="98" ry="46" fill="url(#{halo})" />{/if}
    <rect x="16" y="93" width="168" height="14" rx="7" fill="url(#{cuerpo})" />
    {#each Array.from({ length: 9 }, (_, i) => i) as i (i)}
      <rect x={24 + i * 18} y="97" width="8" height="6" rx="2" class="led-luz" />
    {/each}
  {:else if clase === 'luz' && forma === 'barra'}
    <!-- El borde de arriba del monitor y la barra encima, con su luz hacia el escritorio. -->
    {#if prendida}<path d="M38 70 L162 70 L194 196 L6 196 Z" fill="url(#{cono})" />{/if}
    {#if !enMapa}
      <rect x="14" y="78" width="172" height="104" rx="8" fill="var(--grafito-3)" opacity="0.55" />
      <rect x="22" y="86" width="156" height="90" rx="4" fill="var(--grafito-2)" opacity="0.5" />
    {/if}
    <rect x="88" y="62" width="24" height="24" rx="4" fill="url(#{cuerpo})" />
    <rect x="30" y="56" width="140" height="14" rx="7" fill="url(#{cuerpo})" />
    <rect x="38" y="66" width="124" height="4" rx="2" class="led-luz" />
  {:else if clase === 'luz' && forma === 'lampara'}
    {#if prendida}<path d="M66 84 L134 84 L176 186 L24 186 Z" fill="url(#{cono})" />{/if}
    <ellipse cx="100" cy="186" rx="44" ry="6" fill="url(#{sombra})" />
    <rect x="96" y="80" width="8" height="100" rx="4" fill="url(#{cuerpo})" />
    <ellipse cx="100" cy="180" rx="32" ry="7" fill="url(#{cuerpo})" />
    <path d="M70 34 L130 34 L146 86 L54 86 Z" fill="url(#{cuerpo})" />
    <ellipse cx="100" cy="86" rx="46" ry="5" class="led-luz" />
  {:else if clase === 'luz'}
    <!-- Foco -->
    {#if prendida}<circle cx="100" cy="84" r="92" fill="url(#{halo})" />{/if}
    <circle cx="100" cy="82" r="44" class="vidrio" />
    <path d="M80 118 Q100 128 120 118 L116 136 L84 136 Z" class="vidrio" />
    <ellipse cx="86" cy="66" rx="12" ry="16" fill="#fff" opacity="0.32" />
    <rect x="82" y="136" width="36" height="30" rx="5" fill="url(#{cuerpo})" />
    <rect x="82" y="144" width="36" height="3" fill="var(--grafito-3)" />
    <rect x="82" y="153" width="36" height="3" fill="var(--grafito-3)" />
    <path d="M90 166 L110 166 L104 176 L96 176 Z" fill="var(--grafito-3)" />
  {:else if clase === 'enchufe'}
    <ellipse cx="100" cy="180" rx="50" ry="7" fill="url(#{sombra})" />
    <rect x="52" y="44" width="96" height="124" rx="26" fill="url(#{cuerpo})" />
    <rect x="62" y="54" width="10" height="100" rx="5" fill="#fff" opacity="0.08" />
    <circle cx="100" cy="108" r="30" fill="var(--grafito-3)" />
    <rect x="86" y="96" width="7" height="22" rx="3.5" fill="#06080b" />
    <rect x="107" y="96" width="7" height="22" rx="3.5" fill="#06080b" />
    <circle cx="100" cy="66" r="4" class="led" class:encendido />
  {:else if clase === 'tv'}
    <ellipse cx="100" cy="182" rx="70" ry="7" fill="url(#{sombra})" />
    <rect x="14" y="40" width="172" height="108" rx="8" fill="url(#{cuerpo})" />
    <rect x="20" y="46" width="160" height="96" rx="4" class="pantalla" class:encendido />
    <path d="M86 148 L80 172 H120 L114 148 Z" fill="url(#{cuerpo})" />
    <rect x="62" y="170" width="76" height="7" rx="3.5" fill="url(#{cuerpo})" />
  {:else if clase === 'cerradura'}
    <ellipse cx="100" cy="180" rx="48" ry="7" fill="url(#{sombra})" />
    <path
      d={abierto ? 'M76 74 V58 a24 24 0 0 1 48 0 V40' : 'M76 92 V66 a24 24 0 0 1 48 0 V92'}
      fill="none"
      stroke="url(#{cuerpo})"
      stroke-width="12"
      stroke-linecap="round"
    />
    <rect x="56" y="88" width="88" height="82" rx="16" fill="url(#{cuerpo})" />
    <circle cx="100" cy="122" r="9" fill="#06080b" />
    <rect x="96" y="124" width="8" height="22" rx="4" fill="#06080b" />
    <circle cx="132" cy="100" r="4" class="estado-puerta" class:abierto />
  {:else if clase === 'sensorPuerta'}
    <rect x="58" y="26" width="84" height="150" rx="4" fill="none" stroke="url(#{cuerpo})" stroke-width="8" />
    {#if abierto}
      <path d="M62 30 L110 44 L110 160 L62 172 Z" fill="url(#{cuerpo})" />
      <circle cx="100" cy="104" r="4" fill="var(--grafito-1)" />
    {:else}
      <rect x="62" y="30" width="76" height="142" fill="url(#{cuerpo})" />
      <circle cx="126" cy="104" r="4" fill="var(--grafito-1)" />
    {/if}
    <rect x="140" y="84" width="14" height="32" rx="4" fill="var(--grafito-1)" />
    <circle cx="147" cy="94" r="3" class="estado-puerta" class:abierto />
  {:else if clase === 'cortina'}
    <rect x="34" y="34" width="132" height="132" rx="4" class="ventana" />
    <rect x="26" y="26" width="148" height="8" rx="4" fill="url(#{cuerpo})" />
    <rect x="34" y="34" width={66 * (1 - abierta) + 8} height="134" fill="url(#{cuerpo})" />
    <rect x={166 - (66 * (1 - abierta) + 8)} y="34" width={66 * (1 - abierta) + 8} height="134" fill="url(#{cuerpo})" />
  {:else if clase === 'clima'}
    <rect x="22" y="56" width="156" height="52" rx="14" fill="url(#{cuerpo})" />
    <rect x="36" y="92" width="128" height="6" rx="3" fill="var(--grafito-3)" />
    <circle cx="160" cy="72" r="3.5" class="led" class:encendido />
    {#if encendido}
      {#each [60, 100, 140] as x (x)}
        <path d="M{x} 118 q -8 14 0 28 q 8 14 0 28" fill="none" class="aire" stroke-width="5" stroke-linecap="round" />
      {/each}
    {/if}
  {:else}
    <ellipse cx="100" cy="180" rx="50" ry="7" fill="url(#{sombra})" />
    <rect x="56" y="56" width="88" height="88" rx="24" fill="url(#{cuerpo})" />
    <circle cx="100" cy="100" r="14" fill="none" stroke="var(--texto-3)" stroke-width="5" />
    <circle cx="132" cy="68" r="3" class="led" class:encendido />
  {/if}
</svg>

<style>
  .ilus {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .vidrio {
    fill: color-mix(in srgb, var(--texto-3) 38%, var(--superficie-elevada));
    transition: fill var(--dur-color) ease;
  }
  .prendida .vidrio {
    fill: color-mix(in srgb, var(--luz) 88%, #fff);
  }
  .led-luz {
    fill: var(--grafito-1);
    transition: fill var(--dur-color) ease;
  }
  .prendida .led-luz {
    fill: color-mix(in srgb, var(--luz) 80%, #fff);
  }
  .led {
    fill: var(--grafito-1);
  }
  .led.encendido {
    fill: var(--exito);
  }
  .pantalla {
    fill: #0b0f14;
  }
  .pantalla.encendido {
    fill: color-mix(in srgb, var(--info) 38%, #10233a);
  }
  .estado-puerta {
    fill: var(--exito);
  }
  .estado-puerta.abierto {
    fill: var(--aviso);
  }
  .ventana {
    fill: color-mix(in srgb, var(--celeste) 22%, var(--superficie-elevada));
  }
  .aire {
    stroke: color-mix(in srgb, var(--celeste) 60%, transparent);
  }
</style>
