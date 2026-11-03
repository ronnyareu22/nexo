<!-- Atajo de las teclas de brillo: con el mouse sobre un monitor externo, las teclas de brillo de la Mac cambian ese
     monitor. Pide el permiso de Accesibilidad: si falta, lo dice y ofrece abrirlo, y revisa cada 3 s hasta que esté.
     `dentro` lo dibuja como filas sueltas, para ponerlo dentro de otro panel (Automatizaciones). -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { leerAjustes, usarTeclasBrillo } from '$lib/api';
  import { abrirAjustesPrivacidad } from '$lib/api-sistema';
  import type { EstadoAjustes } from '$lib/tipos';
  import Interruptor from './Interruptor.svelte';
  import Icono from './Icono.svelte';

  let { onerror, dentro = false }: { onerror: (texto: string) => void; dentro?: boolean } = $props();

  let ajustes = $state<EstadoAjustes | null>(null);
  let cambiando = $state(false);
  const faltaPermiso = $derived(!!ajustes && !ajustes.permiso);

  async function alternar() {
    if (!ajustes) return;
    cambiando = true;
    try {
      ajustes = await usarTeclasBrillo(!ajustes.teclasBrillo);
    } catch (e) {
      onerror(String(e));
    }
    cambiando = false;
  }

  function abrirPermiso() {
    abrirAjustesPrivacidad('accesibilidad').catch((e) => onerror(String(e)));
  }

  onMount(() => {
    const leer = () =>
      leerAjustes()
        .then((a) => (ajustes = a))
        .catch(() => {});
    leer();
    // Mientras falte el permiso, se revisa cada 3 s para mostrar cuando ya está.
    const intervalo = setInterval(() => {
      if (faltaPermiso && document.visibilityState === 'visible') leer();
    }, 3000);
    return () => clearInterval(intervalo);
  });
</script>

{#snippet contenido(a: EstadoAjustes)}
  <div class="fila">
    <span class="ficha" class:activa={a.teclasBrillo && a.permiso}><Icono nombre="teclado" tamano={18} /></span>
    <div class="textos">
      <div class="titulo">Teclas de brillo en el monitor bajo el mouse</div>
      <div class="ayuda">
        Con el mouse sobre un monitor externo, las teclas de brillo cambian ese monitor. Sobre la pantalla de la Mac
        funcionan como siempre.
      </div>
    </div>
    <Interruptor
      encendido={a.teclasBrillo}
      cargando={cambiando}
      etiqueta="Teclas de brillo en el monitor bajo el mouse"
      oncambio={alternar}
    />
  </div>

  {#if faltaPermiso}
    <div class="permiso" class:aviso={a.teclasBrillo}>
      <span class="permiso__icono"><Icono nombre={a.teclasBrillo ? 'alerta' : 'info'} tamano={17} /></span>
      <p>
        {a.teclasBrillo ? 'Falta el permiso de Accesibilidad.' : 'Necesita el permiso de Accesibilidad.'}
        Actívalo para <b>Nexo</b> en Ajustes del Sistema › Privacidad y seguridad › Accesibilidad.
      </p>
      <button class="boton-vidrio boton" type="button" onclick={abrirPermiso}>Abrir Accesibilidad</button>
    </div>
  {/if}
{/snippet}

{#if ajustes}
  {#if dentro}
    {@render contenido(ajustes)}
  {:else}
    <section class="panel vidrio" aria-label="Atajo de brillo">
      <h2 class="rotulo">Atajo</h2>
      {@render contenido(ajustes)}
    </section>
  {/if}
{/if}

<style>
  .panel {
    padding: 14px 20px 6px;
    border-radius: var(--radio-lg);
  }
  .rotulo {
    margin: 0 0 4px;
  }

  .fila {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 0;
  }
  .fila,
  .permiso {
    border-top: 1px solid var(--borde);
  }
  .rotulo + .fila {
    border-top: 0;
  }

  .ficha {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    background: var(--superficie-elevada);
    color: var(--texto-3);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .ficha.activa {
    color: var(--amarillo-texto);
    background: color-mix(in srgb, var(--amarillo) 14%, var(--superficie));
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

  .permiso {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 0;
  }
  .permiso__icono {
    display: grid;
    flex-shrink: 0;
    width: 36px;
    place-items: center;
    color: var(--texto-3);
  }
  .permiso.aviso .permiso__icono {
    color: var(--aviso);
  }
  .permiso p {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .permiso b {
    color: var(--texto);
    font-weight: 600;
  }

  .boton {
    flex-shrink: 0;
    min-height: 32px;
    padding: 0 14px;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .boton:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
</style>
