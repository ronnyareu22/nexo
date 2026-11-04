<!--
  Franja de arriba en el panel, solo en Nexo para Windows: hay una versión nueva en tu Mac.
  "Actualizar" la baja, revisa su huella y la instala; Nexo se cierra y vuelve a abrirse solo.
  Calma: superficie sólida con un toque del primario, sin animaciones en bucle. Se puede dejar para después.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    alAvanzarActualizacion,
    alHaberActualizacion,
    instalarActualizacion,
    leerActualizacion,
    type AvanceActualizacion,
    type EstadoActualizacion,
  } from '$lib/api-actualizar';
  import Icono from '$lib/componentes/Icono.svelte';

  let estado = $state<EstadoActualizacion | null>(null);
  let avance = $state<AvanceActualizacion | null>(null);
  let error = $state<string | null>(null);
  let ocupado = $state(false);
  // "Más tarde": se esconde hasta que vuelva a abrirse el panel (o llegue otra versión).
  let descartada = $state<number | null>(null);

  const visible = $derived(
    !!estado?.busca &&
      (ocupado || !!error || (estado.disponible && descartada !== estado.compiladoNuevo)),
  );

  const titulo = $derived.by(() => {
    if (error) return 'No se pudo actualizar';
    if (!ocupado) return 'Nueva versión de Nexo lista';
    if (!avance || avance.fase === 'descargando') return `Descargando… ${avance?.porcentaje ?? 0} %`;
    if (avance.fase === 'verificando') return 'Revisando la descarga…';
    return 'Instalando';
  });

  const detalle = $derived.by(() => {
    if (error) return error;
    if (ocupado && avance?.fase === 'instalando') return 'Nexo se cierra y vuelve a abrirse solo.';
    return estado?.desde ? `desde ${estado.desde}` : 'desde tu Mac';
  });

  async function actualizar() {
    if (ocupado) return;
    ocupado = true;
    error = null;
    avance = { fase: 'descargando', porcentaje: 0 };
    try {
      // Si todo sale bien, Nexo se cierra aquí mismo.
      await instalarActualizacion();
    } catch (e) {
      error = String(e);
      ocupado = false;
      avance = null;
    }
  }

  function despues() {
    error = null;
    descartada = estado?.compiladoNuevo ?? null;
  }

  onMount(() => {
    leerActualizacion()
      .then((e) => {
        estado = e;
        if (e.instalando) ocupado = true;
      })
      .catch(() => {});
    const dejarDisponible = alHaberActualizacion((e) => (estado = e));
    const dejarAvance = alAvanzarActualizacion((a) => {
      if (ocupado && a.fase !== 'error') avance = a;
    });
    return () => {
      dejarDisponible();
      dejarAvance();
    };
  });
</script>

{#if visible}
  <div class="aviso" class:error={!!error} role="status" aria-live="polite">
    <span class="icono" aria-hidden="true"><Icono nombre={error ? 'alerta' : 'actualizar'} tamano={16} /></span>
    <p class="textos">
      <span class="titulo">{titulo}</span>
      <span class="detalle">· {detalle}</span>
    </p>
    {#if ocupado}
      <span
        class="progreso"
        role="progressbar"
        aria-label="Avance de la actualización"
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow={avance?.porcentaje ?? 0}
      >
        <span class="relleno" style:width="{avance?.porcentaje ?? 0}%"></span>
      </span>
    {:else}
      <button class="boton-primario accion" type="button" onclick={actualizar}>
        {error ? 'Reintentar' : 'Actualizar'}
      </button>
      <button class="cerrar" type="button" onclick={despues} aria-label="Más tarde">
        <Icono nombre="cerrar" tamano={15} />
      </button>
    {/if}
  </div>
{/if}

<style>
  .aviso {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
    min-height: 48px;
    margin: 0 32px 8px;
    padding: 6px 8px 6px 14px;
    border: 1px solid var(--elegido-borde);
    border-radius: var(--radio-md);
    background: var(--elegido-bg);
    color: var(--texto);
    font-size: 13px;
  }
  .aviso.error {
    border-color: color-mix(in srgb, var(--error) 35%, transparent);
    background: color-mix(in srgb, var(--error) 10%, var(--superficie));
  }
  .icono {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    color: var(--primario-texto);
  }
  .error .icono {
    color: var(--error);
  }
  .textos {
    flex: 1;
    min-width: 0;
    margin: 0;
    line-height: 18px;
  }
  .titulo {
    font-weight: 600;
  }
  .detalle {
    color: var(--texto-2);
  }
  .accion {
    min-height: 32px;
    padding: 0 14px;
  }
  .progreso {
    flex-shrink: 0;
    width: 140px;
    height: 6px;
    margin-right: 8px;
    overflow: hidden;
    border-radius: var(--radio-pildora);
    background: var(--superficie-hundida);
  }
  .relleno {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--primario);
    transition: width var(--dur-color) linear;
  }
  .cerrar {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--texto-3);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .cerrar:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .cerrar:focus-visible,
  .accion:focus-visible {
    outline: none;
    box-shadow: var(--foco);
  }
  @media (prefers-reduced-motion: reduce) {
    .relleno {
      transition: none;
    }
  }
</style>
