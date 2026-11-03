<!-- Tarjeta grande de un aparato: foto, estado, interruptor de conexión y anillo de batería.
     (Las vistas usan FilaDispositivo; esta queda para mosaicos.) El hover solo cambia el fondo. -->
<script lang="ts">
  import type { Dispositivo } from '$lib/tipos';
  import { estadoDe, nombreDe, nombreTipo } from '$lib/tipos';
  import { modeloDe } from '$lib/catalogo';
  import Figura from './Figura.svelte';
  import Anillo from './Anillo.svelte';
  import Interruptor from './Interruptor.svelte';
  import EstadoChip from './EstadoChip.svelte';

  let {
    dispositivo: d,
    pendiente,
    onabrir,
    onalternar,
  }: { dispositivo: Dispositivo; pendiente: boolean; onabrir: () => void; onalternar: () => void } = $props();

  const conFoto = $derived(modeloDe(d) !== null);
  const nombre = $derived(nombreDe(d));
  const subtitulo = $derived([d.fabricante, nombreTipo[d.tipo]].filter(Boolean).join(' · '));
</script>

<article class="tarjeta vidrio" class:apagado={!d.conectado}>
  <!-- Toda la tarjeta abre el detalle; el interruptor queda por encima y funciona aparte. -->
  <button class="abrir" onclick={onabrir} aria-label="Ver {nombre}"></button>

  <div class="cabeza">
    <EstadoChip estado={estadoDe(d, pendiente)} />
    <Interruptor
      encendido={d.conectado}
      cargando={pendiente}
      etiqueta={d.conectado ? `Desconectar ${nombre}` : `Conectar ${nombre}`}
      oncambio={onalternar}
    />
  </div>

  <div class="escenario" class:con-foto={conFoto}>
    <div class="figura"><Figura dispositivo={d} /></div>
  </div>

  <div class="pie">
    <div class="textos">
      <div class="titulo">{nombre}</div>
      <div class="sub">{subtitulo}</div>
    </div>
    {#if d.bateria !== null}
      <Anillo nivel={d.bateria} tamano={46} grosor={4} />
    {/if}
  </div>
</article>

<style>
  .tarjeta {
    display: flex;
    flex-direction: column;
    padding: 14px 16px 18px;
    border-radius: var(--radio-lg);
    overflow: hidden;
    isolation: isolate;
  }

  .tarjeta {
    transition: background-color var(--dur-color) ease;
  }
  .tarjeta:has(.abrir:hover) {
    background: var(--superficie-hover);
  }

  .abrir {
    position: absolute;
    inset: 0;
    z-index: 1;
    padding: 0;
    border: 0;
    border-radius: inherit;
    background: transparent;
  }

  .cabeza {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    pointer-events: none;
  }
  .cabeza :global(.interruptor) {
    pointer-events: auto;
  }

  .escenario {
    position: relative;
    height: 168px;
    display: grid;
    place-items: center;
    pointer-events: none;
  }
  .figura {
    position: relative;
    width: 148px;
    height: 148px;
    transition:
      opacity var(--rapido),
      filter var(--rapido);
  }
  .apagado .figura {
    opacity: 0.55;
    filter: saturate(0.5);
  }

  .pie {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 46px;
    pointer-events: none;
  }
  .textos {
    min-width: 0;
  }
  .titulo,
  .sub {
    overflow-wrap: anywhere;
  }
  .titulo {
    font-size: 15px;
    font-weight: 650;
    line-height: 1.35;
  }
  .sub {
    margin-top: 2px;
    font-size: 12.5px;
    color: var(--texto-tenue);
    line-height: 1.35;
  }
</style>
