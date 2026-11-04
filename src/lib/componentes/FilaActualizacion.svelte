<!--
  Ajustes › Acerca de: cuándo se compiló este Nexo y, en Windows, "Buscar actualización" en tu Mac.
  Si hay una versión nueva, el botón pasa a "Actualizar" (lo mismo que la franja de arriba del panel).
  Va justo debajo de la fila de Nexo, por eso trae su propia línea de arriba.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    alHaberActualizacion,
    buscarActualizacion,
    fechaCompilacion,
    instalarActualizacion,
    leerActualizacion,
    type EstadoActualizacion,
  } from '$lib/api-actualizar';
  import Icono from '$lib/componentes/Icono.svelte';

  let { onerror }: { onerror: (texto: string) => void } = $props();

  let estado = $state<EstadoActualizacion | null>(null);
  let buscando = $state(false);
  let instalando = $state(false);
  // Lo que respondió "Buscar actualización" cuando no hubo nada nuevo.
  let resultado = $state<{ texto: string; ok: boolean } | null>(null);

  async function buscar() {
    if (buscando) return;
    buscando = true;
    resultado = null;
    try {
      const e = await buscarActualizacion();
      estado = e;
      if (!e.disponible)
        resultado = e.problema ? { texto: e.problema, ok: false } : { texto: 'Ya tienes la versión más nueva.', ok: true };
    } catch (e) {
      onerror(String(e));
    }
    buscando = false;
  }

  async function instalar() {
    if (instalando) return;
    instalando = true;
    try {
      // Si todo sale bien, Nexo se cierra y el instalador lo vuelve a abrir.
      await instalarActualizacion();
    } catch (e) {
      onerror(String(e));
      instalando = false;
    }
  }

  onMount(() => {
    leerActualizacion()
      .then((e) => {
        estado = e;
        instalando = e.instalando;
      })
      .catch(() => {});
    return alHaberActualizacion((e) => {
      estado = e;
      resultado = null;
    });
  });
</script>

{#if estado}
  <div class="fila">
    <div class="textos">
      <div class="titulo">Compilación</div>
      <div class="ayuda">
        {fechaCompilacion(estado.compiladoActual)} · <span class="id">{estado.build}</span>
      </div>
      {#if estado.disponible && estado.compiladoNuevo}
        <div class="ayuda nueva">
          Hay una versión nueva en {estado.desde ?? 'tu Mac'}, del {fechaCompilacion(estado.compiladoNuevo)}.
        </div>
      {:else if resultado}
        <div class="ayuda" class:hecho={resultado.ok} role="status">
          {#if resultado.ok}<Icono nombre="check" tamano={14} />{/if}
          {resultado.texto}
        </div>
      {/if}
    </div>
    {#if estado.busca}
      {#if estado.disponible}
        <button class="boton-primario boton" type="button" disabled={instalando} aria-busy={instalando} onclick={instalar}>
          {instalando ? 'Actualizando…' : 'Actualizar'}
        </button>
      {:else}
        <button class="boton-vidrio boton secundario" type="button" disabled={buscando} aria-busy={buscando} onclick={buscar}>
          {buscando ? 'Buscando…' : 'Buscar actualización'}
        </button>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 60px;
    padding: 12px 0;
    border-top: 1px solid var(--borde);
  }
  .textos {
    flex: 1;
    min-width: 0;
  }
  .titulo {
    font-weight: 600;
  }
  .ayuda {
    margin-top: 2px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .ayuda.nueva {
    color: var(--primario-texto);
  }
  .ayuda.hecho {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--exito);
  }
  .id {
    font-variant-numeric: tabular-nums;
  }
  .boton {
    flex-shrink: 0;
    min-height: 32px;
    padding: 0 14px;
  }
  .boton.secundario {
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .boton.secundario:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .boton:disabled {
    cursor: default;
    opacity: 0.6;
  }
</style>
