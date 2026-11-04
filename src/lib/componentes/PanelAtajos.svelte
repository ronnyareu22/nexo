<!-- Atajo de las teclas de brillo: con el mouse sobre un monitor externo, las teclas de brillo de la Mac cambian ese
     monitor. Pide el permiso de Accesibilidad: si falta, lo dice y ofrece abrirlo, y revisa cada 3 s hasta que esté.
     En Windows son Ctrl + Alt + F1 / F2 sobre cualquier pantalla (sin permiso que pedir); se muestran como teclas.
     `dentro` lo dibuja como filas sueltas, para ponerlo dentro de otro panel (Automatizaciones). -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { leerAjustes, usarTeclasBrillo } from '$lib/api';
  import { abrirAjustesPrivacidad, esMac } from '$lib/api-sistema';
  import type { EstadoAjustes } from '$lib/tipos';
  import Interruptor from './Interruptor.svelte';
  import Icono from './Icono.svelte';

  let { onerror, dentro = false }: { onerror: (texto: string) => void; dentro?: boolean } = $props();

  let ajustes = $state<EstadoAjustes | null>(null);
  let cambiando = $state(false);
  // Windows no pide permiso: nunca se muestra el aviso.
  const faltaPermiso = $derived(esMac && !!ajustes && !ajustes.permiso);
  const titulo = esMac
    ? 'Teclas de brillo en el monitor bajo el mouse'
    : 'Brillo con el teclado en el monitor bajo el mouse';
  /** Las combinaciones de Windows, para dibujarlas como teclas. */
  const combos = [
    { teclas: ['Ctrl', 'Alt', 'F1'], accion: 'Bajar' },
    { teclas: ['Ctrl', 'Alt', 'F2'], accion: 'Subir' },
  ];

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
    if (!esMac) return;
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
      <div class="titulo">{titulo}</div>
      {#if esMac}
        <div class="ayuda">
          Con el mouse sobre un monitor externo, las teclas de brillo cambian ese monitor. Sobre la pantalla de la Mac
          funcionan como siempre.
        </div>
      {:else}
        <div class="ayuda">
          Pon el mouse sobre una pantalla y usa Ctrl + Alt + F1 para bajar y Ctrl + Alt + F2 para subir el brillo.
          Puedes asignar esas combinaciones a teclas de tu Wave Keys o botones del MX Master 4 en Logi Options+.
        </div>
        <!-- Lo mismo que dice la ayuda, en teclas: el lector de pantalla ya lo leyó arriba. -->
        <div class="combos" class:apagado={!a.teclasBrillo} aria-hidden="true">
          {#each combos as c (c.accion)}
            <span class="combo">
              <span class="teclas">{#each c.teclas as t (t)}<kbd>{t}</kbd>{/each}</span>
              <span>{c.accion}</span>
            </span>
          {/each}
        </div>
      {/if}
    </div>
    <Interruptor encendido={a.teclasBrillo} cargando={cambiando} etiqueta={titulo} oncambio={alternar} />
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

  /* ---------- Teclas (Windows) ---------- */
  .combos {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 18px;
    margin-top: 10px;
    transition: opacity var(--dur-color) ease;
  }
  .combos.apagado {
    opacity: 0.55;
  }
  .combo {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--texto-2);
  }
  .teclas {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  kbd {
    display: inline-grid;
    place-items: center;
    min-width: 24px;
    height: 24px;
    padding: 0 7px;
    border-radius: 7px;
    background: var(--superficie);
    box-shadow:
      inset 0 0 0 1px var(--borde-fuerte),
      inset 0 -1.5px 0 var(--borde-fuerte);
    color: var(--texto);
    font-family: var(--fuente);
    font-size: 12px;
    line-height: 1;
    font-weight: 600;
    white-space: nowrap;
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
