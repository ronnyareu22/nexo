<!-- Ajustes automáticos de un aparato: usarlo para el sonido al conectarse y avisar con batería baja. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { guardarAutomatico, leerAutomatico } from '$lib/api';
  import type { Automatico, Dispositivo } from '$lib/tipos';
  import { esDeSonido } from '$lib/tipos';
  import Interruptor from './Interruptor.svelte';

  let { dispositivo: d, onerror }: { dispositivo: Dispositivo; onerror: (texto: string) => void } = $props();

  let ajustes = $state<Automatico | null>(null);
  onMount(() => {
    leerAutomatico(d.id)
      .then((a) => (ajustes = a))
      .catch((e) => onerror(String(e)));
  });

  async function cambiar(campo: keyof Automatico) {
    if (!ajustes) return;
    const nuevo = { ...ajustes, [campo]: !ajustes[campo] };
    ajustes = nuevo;
    try {
      ajustes = await guardarAutomatico(d.id, nuevo);
    } catch (e) {
      onerror(String(e));
    }
  }
</script>

{#if ajustes}
  <section class="panel vidrio">
    <h2>Automático</h2>
    {#if esDeSonido(d)}
      <div class="fila">
        <div class="textos">
          <div class="titulo">Usar para el sonido al conectarse</div>
          <div class="ayuda">Al encenderlos, el sonido de la Mac pasa a estos audífonos.</div>
        </div>
        <Interruptor
          encendido={ajustes.usarSalidaAlConectar}
          etiqueta="Usar para el sonido al conectarse"
          oncambio={() => cambiar('usarSalidaAlConectar')}
        />
      </div>
    {/if}
    <div class="fila" class:separada={esDeSonido(d)}>
      <div class="textos">
        <div class="titulo">Avisarme con batería baja</div>
        <div class="ayuda">Una notificación de macOS cuando baje del 20 %.</div>
      </div>
      <Interruptor
        encendido={ajustes.avisarBateriaBaja}
        etiqueta="Avisarme con batería baja"
        oncambio={() => cambiar('avisarBateriaBaja')}
      />
    </div>
  </section>
{/if}

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
  .fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .separada {
    margin-top: 16px;
    padding-top: 16px;
    border-top: 1px solid var(--borde);
  }
  .textos {
    flex: 1;
    min-width: 0;
  }
  .titulo {
    font-weight: 650;
  }
  .ayuda {
    margin-top: 2px;
    font-size: 12.5px;
    line-height: 1.4;
    color: var(--texto-2);
  }
</style>
