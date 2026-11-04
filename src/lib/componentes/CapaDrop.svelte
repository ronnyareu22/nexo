<!--
  Capa de Nexo Drop: al arrastrar archivos sobre cualquier parte de la ventana, la cubre entera con
  "Suelta para enviar a {destino}". Al soltar, manda a tu otro equipo y lleva a la sección Drop para ver el avance.
  Usa el arrastre nativo de Tauri (rutas reales). En la sección Drop no se muestra (allí se ilumina su zona),
  pero soltar en cualquier lado también envía.
  Superficie sólida con el anillo turquesa y la aurora quieta detrás: nada se mueve.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { alArrastrarArchivos, enviarDrop } from '$lib/api-sistema';
  import { destinoActual, drop, leerDestinos } from '$lib/drop.svelte';
  import Icono from './Icono.svelte';

  let {
    visible,
    onenviar,
    onerror,
  }: {
    /** Si se muestra la capa (en la sección Drop no hace falta). */
    visible: boolean;
    /** Se empezó a enviar: lleva a la sección Drop. */
    onenviar: () => void;
    onerror: (texto: string) => void;
  } = $props();

  const destino = $derived(destinoActual());

  async function soltar(rutas: string[]) {
    if (!rutas.length) return;
    await leerDestinos();
    const d = destinoActual();
    if (!d) {
      onerror('No encontré otro equipo tuyo con Nexo en Tailscale.');
      return;
    }
    try {
      await enviarDrop(rutas, d.ip);
      onenviar();
    } catch (e) {
      onerror(String(e));
    }
  }

  onMount(() => {
    leerDestinos();
    return alArrastrarArchivos((a) => {
      if (a.tipo === 'encima') {
        drop.arrastrando = true;
        drop.cantidad = a.rutas.length;
        leerDestinos();
      } else if (a.tipo === 'fuera') {
        drop.arrastrando = false;
      } else {
        drop.arrastrando = false;
        soltar(a.rutas);
      }
    });
  });
</script>

{#if visible && drop.arrastrando}
  <div class="capa" role="status" aria-live="polite">
    <div class="marco">
      <span class="icono" aria-hidden="true"><Icono nombre="drop" tamano={34} /></span>
      {#if destino}
        <p class="titulo">Suelta para enviar a {destino.nombre}</p>
        <p class="detalle">
          {drop.cantidad === 1 ? '1 elemento' : drop.cantidad > 1 ? `${drop.cantidad} elementos` : 'Archivos o carpetas'} ·
          llegan a Descargas › Nexo Drop
          {#if !destino.enLinea}<br /><span class="aviso">{destino.nombre} no responde: revisa que esté encendida y con Nexo abierto.</span>{/if}
        </p>
      {:else}
        <p class="titulo">No hay a quién enviar</p>
        <p class="detalle">Abre Nexo en tu otro equipo, con Tailscale conectado y tu misma cuenta.</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .capa {
    position: fixed;
    z-index: 70;
    inset: 0;
    display: grid;
    padding: 16px;
    background: var(--fondo);
  }
  .marco {
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 6px;
    padding: 32px;
    border-radius: var(--radio-xl);
    text-align: center;
    background:
      radial-gradient(ellipse 340px 220px at 50% 58%, var(--aurora-1), transparent),
      radial-gradient(ellipse 260px 200px at 36% 70%, var(--aurora-2), transparent),
      radial-gradient(ellipse 260px 200px at 64% 70%, var(--aurora-3), transparent),
      var(--superficie);
    box-shadow:
      inset 0 0 0 2px var(--primario),
      0 0 0 1px var(--borde-tarjeta);
  }
  .icono {
    display: grid;
    place-items: center;
    width: 72px;
    height: 72px;
    margin-bottom: 10px;
    border-radius: 20px;
    color: var(--primario-texto);
    background: var(--elegido-bg);
    box-shadow: inset 0 0 0 1px var(--elegido-borde);
  }
  .titulo {
    margin: 0;
    font: var(--letra-seccion);
    letter-spacing: -0.01em;
    color: var(--texto);
    overflow-wrap: anywhere;
  }
  .detalle {
    max-width: 460px;
    margin: 0;
    font: var(--letra-cuerpo);
    color: var(--texto-2);
  }
  .aviso {
    color: var(--aviso);
  }
</style>
