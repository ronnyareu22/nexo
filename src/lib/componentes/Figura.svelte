<!-- La foto del aparato si es un modelo conocido; si no, su ilustración. `sombra` false para tamaños chicos (filas). -->
<script lang="ts">
  import type { Dispositivo } from '$lib/tipos';
  import { nombreDe } from '$lib/tipos';
  import { modeloDe } from '$lib/catalogo';
  import Ilustracion from './Ilustracion.svelte';

  let { dispositivo: d, sombra = true }: { dispositivo: Dispositivo; sombra?: boolean } = $props();

  const modelo = $derived(modeloDe(d));
  let fallo = $state(false);
</script>

{#if modelo && !fallo}
  <img
    class="foto"
    class:sombra
    src={modelo.foto}
    alt={nombreDe(d)}
    draggable="false"
    decoding="async"
    onerror={() => (fallo = true)}
  />
{:else}
  <Ilustracion tipo={d.tipo} marca={d.fabricante} encendido={d.conectado} />
{/if}

<style>
  .foto {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  /* Una sombra de apoyo suave, como en una foto de producto. */
  .foto.sombra {
    filter: drop-shadow(0 12px 14px rgb(0 0 0 / 0.28));
  }
</style>
