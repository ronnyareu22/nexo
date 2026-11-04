<!-- Atajo de las teclas de brillo: con el mouse sobre un monitor externo, las teclas de brillo de la Mac cambian ese
     monitor. Pide el permiso de Accesibilidad: si falta, lo dice y ofrece abrirlo, y revisa cada 3 s hasta que esté.
     En Windows son dos atajos sobre cualquier pantalla (sin permiso que pedir; de fábrica Ctrl + Alt + F1 / F2), que
     se muestran como teclas y se cambian con «Cambiar»: la siguiente combinación que se presione (Esc cancela).
     Si la PC es un portátil, también el ajuste de sus propias teclas de brillo (pasan al monitor bajo el mouse).
     `dentro` lo dibuja como filas sueltas, para ponerlo dentro de otro panel (Automatizaciones). -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { leerAjustes, ponerAtajoBrillo, usarTeclasBrillo, usarTeclasPortatil } from '$lib/api';
  import { abrirAjustesPrivacidad, esMac } from '$lib/api-sistema';
  import { atajoDe, mismoAtajo, problemaAtajo, teclasDe, textoAtajo } from '$lib/atajos';
  import type { Atajo, EstadoAjustes } from '$lib/tipos';
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
  /** Windows: los dos atajos, para dibujarlos como teclas. */
  const atajos = $derived(
    ajustes
      ? [
          { subir: false, nombre: 'Bajar brillo', atajo: ajustes.atajoBajar },
          { subir: true, nombre: 'Subir brillo', atajo: ajustes.atajoSubir },
        ]
      : [],
  );

  // --- Cambiar un atajo: se espera la siguiente combinación (Esc cancela). ---
  /** Cuál se está grabando: false bajar, true subir. */
  let grabando = $state<boolean | null>(null);
  let guardandoAtajo = $state(false);
  /** Lo que se dice mientras se graba o al terminar (también para el lector de pantalla). */
  let avisoAtajo = $state('');
  let avisoError = $state(false);
  let botonCancelar = $state<HTMLButtonElement | null>(null);
  let cambiandoPortatil = $state(false);

  function empezarGrabar(subir: boolean) {
    grabando = subir;
    avisoAtajo = '';
    avisoError = false;
  }

  function cancelarGrabar() {
    grabando = null;
    avisoAtajo = '';
    avisoError = false;
  }

  function avisar(texto: string, error = false) {
    avisoAtajo = texto;
    avisoError = error;
  }

  async function guardarAtajo(subir: boolean, atajo: Atajo) {
    grabando = null;
    guardandoAtajo = true;
    try {
      ajustes = await ponerAtajoBrillo(subir, atajo);
      avisar(`Listo: ${textoAtajo(atajo)} ${subir ? 'sube' : 'baja'} el brillo.`);
    } catch (e) {
      avisar(String(e), true);
    }
    guardandoAtajo = false;
  }

  function alTeclear(e: KeyboardEvent) {
    const subir = grabando;
    if (subir === null || !ajustes) return;
    // Mientras se graba, ninguna tecla hace lo suyo (Tab, Enter, atajos de la ventana…).
    e.preventDefault();
    e.stopPropagation();
    if (e.repeat) return;
    if (e.key === 'Escape' && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
      cancelarGrabar();
      return;
    }
    const atajo = atajoDe(e);
    if (!atajo) return; // solo un modificador: se espera la tecla que sigue
    const problema = problemaAtajo(atajo);
    if (problema) {
      avisar(problema, true);
      return;
    }
    const otro = subir ? ajustes.atajoBajar : ajustes.atajoSubir;
    if (mismoAtajo(atajo, otro)) {
      avisar(`${textoAtajo(atajo)} ya es el atajo para ${subir ? 'bajar' : 'subir'} el brillo.`, true);
      return;
    }
    guardarAtajo(subir, atajo);
  }

  // Solo mientras se graba: se escucha antes que nadie (fase de captura) y se suelta al terminar.
  $effect(() => {
    if (grabando === null) return;
    botonCancelar?.focus();
    window.addEventListener('keydown', alTeclear, true);
    return () => window.removeEventListener('keydown', alTeclear, true);
  });

  async function alternarPortatil() {
    if (!ajustes) return;
    cambiandoPortatil = true;
    try {
      ajustes = await usarTeclasPortatil(!ajustes.teclasPortatil);
    } catch (e) {
      onerror(String(e));
    }
    cambiandoPortatil = false;
  }

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
          Pon el mouse sobre una pantalla y usa estos atajos para bajar o subir su brillo. Puedes asignarlos a teclas
          de tu Wave Keys o botones del MX Master 4 en Logi Options+.
        </div>
      {/if}
    </div>
    <Interruptor encendido={a.teclasBrillo} cargando={cambiando} etiqueta={titulo} oncambio={alternar} />
  </div>

  {#if !esMac}
    <div class="atajos" class:apagado={!a.teclasBrillo}>
      {#each atajos as f (f.subir)}
        <div class="atajo" class:grabando={grabando === f.subir}>
          <span class="atajo__nombre">{f.nombre}</span>
          {#if grabando === f.subir}
            <span class="atajo__espera">Presiona la combinación…</span>
            <button class="boton-vidrio boton" type="button" bind:this={botonCancelar} onclick={cancelarGrabar}>
              Cancelar
            </button>
          {:else}
            <span class="teclas" role="img" aria-label={textoAtajo(f.atajo)}>
              {#each teclasDe(f.atajo) as t (t)}<kbd aria-hidden="true">{t}</kbd>{/each}
            </span>
            <button
              class="boton-vidrio boton"
              type="button"
              disabled={guardandoAtajo}
              aria-label="Cambiar el atajo para {f.nombre.toLowerCase()}"
              onclick={() => empezarGrabar(f.subir)}
            >
              Cambiar
            </button>
          {/if}
        </div>
      {/each}
      <p class="atajos__aviso" class:error={avisoError} role="status" aria-live="polite">
        {#if grabando !== null && !avisoAtajo}
          Usa Ctrl, Alt o Win con otra tecla, o una tecla F sola. Esc cancela.
        {:else}
          {avisoAtajo}
        {/if}
      </p>
    </div>

    {#if a.hayPortatil}
      <div class="fila">
        <span class="ficha" class:activa={a.teclasBrillo && a.teclasPortatil}><Icono nombre="sol" tamano={18} /></span>
        <div class="textos">
          <div class="titulo">Usar las teclas de brillo del portátil</div>
          <div class="ayuda">
            También funcionan las teclas de brillo de tu portátil cuando el mouse está sobre el otro monitor. Windows
            muestra su propio aviso igual.
          </div>
        </div>
        <Interruptor
          encendido={a.teclasPortatil}
          cargando={cambiandoPortatil}
          etiqueta="Usar las teclas de brillo del portátil"
          oncambio={alternarPortatil}
        />
      </div>
    {/if}
  {/if}

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

  /* ---------- Atajos (Windows) ---------- */
  .atajos {
    padding: 0 0 10px 50px;
    transition: opacity var(--dur-color) ease;
  }
  .atajos.apagado {
    opacity: 0.55;
  }
  .atajo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    min-height: 40px;
    padding: 4px 0;
  }
  .atajo__nombre {
    flex: 0 0 104px;
    font-size: 12.5px;
    color: var(--texto-2);
  }
  .atajo__espera {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--amarillo-texto);
  }
  .atajo .teclas {
    flex: 1;
    min-width: 0;
    flex-wrap: wrap;
  }
  .atajos__aviso {
    min-height: 18px;
    margin: 2px 0 0;
    font-size: 12px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .atajos__aviso.error {
    color: var(--aviso);
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
