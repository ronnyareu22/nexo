<!-- Ajustes: apariencia, arranque, permisos, historial y "Acerca de", en filas como los Ajustes del Sistema.
     Mientras falte un permiso, se revisa cada 3 s para mostrar cuando ya está. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { abrirAjustesPrivacidad, borrarHistorial, esMac, leerArranque, leerPermisos, usarArranque } from '$lib/api-sistema';
  import type { Permisos, SeccionPrivacidad } from '$lib/tipos-sistema';
  import Icono from '$lib/componentes/Icono.svelte';
  import Interruptor from '$lib/componentes/Interruptor.svelte';
  import SelectorTema from '$lib/componentes/SelectorTema.svelte';
  import NexoBot from '$lib/componentes/NexoBot.svelte';
  import Dialogo from '$lib/componentes/Dialogo.svelte';

  let { onerror }: { onerror: (texto: string) => void } = $props();

  const VERSION = '0.1.0';

  // --- Arranque ---
  let arranque = $state<boolean | null>(null);
  let cambiandoArranque = $state(false);

  async function alternarArranque() {
    if (arranque === null) return;
    cambiandoArranque = true;
    try {
      arranque = await usarArranque(!arranque);
    } catch (e) {
      onerror(String(e));
    }
    cambiandoArranque = false;
  }

  // --- Permisos ---
  type Tono = 'si' | 'falta' | 'no';
  interface FilaPermiso {
    seccion: SeccionPrivacidad;
    nombre: string;
    para: string;
    /** Sin estado cuando el sistema no lo informa bien (Notificaciones): solo el botón, sin chip inventado. */
    tono: Tono | null;
    estado: string | null;
  }

  let permisos = $state<Permisos | null>(null);

  function estadoDe(valor: boolean | Permisos['bluetooth']): { tono: Tono; estado: string } {
    if (valor === true || valor === 'concedido') return { tono: 'si', estado: 'Concedido' };
    if (valor === 'denegado') return { tono: 'no', estado: 'Denegado' };
    if (valor === 'restringido') return { tono: 'no', estado: 'Bloqueado' };
    return { tono: 'falta', estado: 'Falta' };
  }

  const filas = $derived.by((): FilaPermiso[] => {
    if (!permisos) return [];
    const lista: FilaPermiso[] = [];
    if (esMac) {
      lista.push({
        seccion: 'accesibilidad',
        nombre: 'Accesibilidad',
        para: 'Para las teclas de brillo en el monitor bajo el mouse.',
        ...estadoDe(permisos.accesibilidad),
      });
    }
    lista.push(
      {
        seccion: 'bluetooth',
        nombre: 'Bluetooth',
        para: 'Para ver, conectar y desconectar tus aparatos.',
        ...estadoDe(permisos.bluetooth),
      },
      // El sistema siempre lo da por concedido aunque estén apagadas: no se muestra un estado que no es cierto.
      {
        seccion: 'notificaciones',
        nombre: 'Notificaciones',
        para: 'Para el aviso de batería baja. Revisa que estén permitidas para Nexo.',
        tono: null,
        estado: null,
      },
    );
    return lista;
  });
  const faltaAlguno = $derived(filas.some((f) => f.tono !== null && f.tono !== 'si'));

  function abrirPermiso(seccion: SeccionPrivacidad) {
    abrirAjustesPrivacidad(seccion).catch((e) => onerror(String(e)));
  }

  // --- Historial ---
  let preguntando = $state(false);
  let borrando = $state(false);
  let borrado = $state(false);

  async function borrar() {
    preguntando = false;
    borrando = true;
    try {
      await borrarHistorial();
      borrado = true;
    } catch (e) {
      onerror(String(e));
    }
    borrando = false;
  }

  onMount(() => {
    leerArranque()
      .then((a) => (arranque = a))
      .catch((e) => onerror(String(e)));

    const leer = () =>
      leerPermisos()
        .then((p) => (permisos = p))
        .catch(() => {});
    leer();
    const intervalo = setInterval(() => {
      if (faltaAlguno && document.visibilityState === 'visible') leer();
    }, 3000);
    return () => clearInterval(intervalo);
  });
</script>

<div class="vista">
  <header class="cabecera">
    <h1>Ajustes</h1>
  </header>

  <div class="columnas">
    <div class="columna">
      <section class="panel vidrio" aria-labelledby="ajustes-apariencia">
        <h2 class="rotulo" id="ajustes-apariencia">Apariencia</h2>
        <div class="fila">
          <div class="textos">
            <div class="titulo">Tema</div>
            <div class="ayuda">Como el sistema, claro u oscuro.</div>
          </div>
          <SelectorTema />
        </div>
      </section>

      <section class="panel vidrio" aria-labelledby="ajustes-general">
        <h2 class="rotulo" id="ajustes-general">General</h2>
        <div class="fila">
          <div class="textos">
            <div class="titulo">Abrir Nexo al iniciar sesión</div>
            <div class="ayuda">
              {esMac ? 'Queda en la barra de menús, listo para usar.' : 'Queda en la bandeja, listo para usar.'}
            </div>
          </div>
          {#if arranque !== null}
            <Interruptor
              encendido={arranque}
              cargando={cambiandoArranque}
              etiqueta="Abrir Nexo al iniciar sesión"
              oncambio={alternarArranque}
            />
          {/if}
        </div>
      </section>

      <section class="panel vidrio" aria-labelledby="ajustes-permisos">
        <h2 class="rotulo" id="ajustes-permisos">Permisos</h2>
        {#if permisos}
          {#each filas as f (f.seccion)}
            <div class="fila">
              <div class="textos">
                <div class="titulo">{f.nombre}</div>
                <div class="ayuda">{f.para}</div>
              </div>
              <div class="permiso">
                {#if f.tono}<span class="chip {f.tono}"><span class="punto"></span>{f.estado}</span>{/if}
                <button
                  class="boton-vidrio boton"
                  type="button"
                  aria-label="Abrir ajustes de {f.nombre}"
                  onclick={() => abrirPermiso(f.seccion)}
                >
                  Abrir ajustes
                </button>
              </div>
            </div>
          {/each}
        {:else}
          <div class="fila cargando" aria-busy="true"><span class="linea"></span><span class="solo-lector">Revisando permisos…</span></div>
        {/if}
      </section>
    </div>

    <div class="columna">
      <section class="panel vidrio" aria-labelledby="ajustes-historial">
        <h2 class="rotulo" id="ajustes-historial">Historial</h2>
        <div class="fila">
          <div class="textos">
            <div class="titulo">Borrar historial de uso</div>
            {#if borrado}
              <div class="ayuda hecho"><Icono nombre="check" tamano={14} /> Listo, el historial quedó en blanco.</div>
            {:else}
              <div class="ayuda">
                Nexo guarda en este equipo cuándo usas cada aparato y cómo baja su batería.
              </div>
            {/if}
          </div>
          <button
            class="boton-vidrio boton peligro"
            type="button"
            disabled={borrando}
            aria-label="Borrar historial de uso"
            onclick={() => (preguntando = true)}
          >
            Borrar…
          </button>
        </div>
      </section>

      <section class="panel vidrio" aria-labelledby="ajustes-acerca">
        <h2 class="rotulo" id="ajustes-acerca">Acerca de</h2>
        <div class="fila acerca">
          <NexoBot expresion="feliz" tamano={64} />
          <div class="textos">
            <div class="titulo">Nexo <span class="version">{VERSION}</span></div>
            <div class="ayuda">Tu centro de control de dispositivos.</div>
          </div>
        </div>
      </section>
    </div>
  </div>
</div>

<Dialogo
  abierto={preguntando}
  titulo="¿Borrar el historial de uso?"
  descripcion="Se borran las horas de uso y la batería guardadas de todos tus aparatos. Tus reglas automáticas no cambian. No se puede deshacer."
  icono="papelera"
  tono="peligro"
  textoConfirmar="Borrar"
  textoCancelar="Cancelar"
  onconfirmar={borrar}
  oncancelar={() => (preguntando = false)}
/>

<style>
  .vista {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
  }
  .cabecera h1 {
    margin: 0;
    font-size: 26px;
    line-height: 32px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  .columnas,
  .columna {
    display: grid;
    gap: 16px;
    align-content: start;
    min-width: 0;
  }
  /* Con la ventana ancha, dos columnas para no dejar espacio muerto. */
  @media (min-width: 1360px) {
    .columnas {
      grid-template-columns: 1fr 1fr;
    }
  }

  .panel {
    padding: 14px 20px 4px;
    border-radius: var(--radio-lg);
    min-width: 0;
  }
  .rotulo {
    margin: 0 0 4px;
  }

  .fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 60px;
    padding: 12px 0;
  }
  .fila + .fila {
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
  .ayuda.hecho {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--exito);
  }

  .permiso {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 10px 0 8px;
    border-radius: var(--radio-pildora);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
  }
  .punto {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }
  .chip.si {
    color: var(--exito);
    background: color-mix(in srgb, var(--exito) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--exito) 30%, transparent);
  }
  .chip.falta {
    color: var(--aviso);
    background: color-mix(in srgb, var(--aviso) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--aviso) 30%, transparent);
  }
  .chip.no {
    color: var(--error);
    background: color-mix(in srgb, var(--error) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--error) 30%, transparent);
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
  .boton:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .boton.peligro {
    color: var(--error);
  }

  .acerca {
    justify-content: flex-start;
    gap: 14px;
  }
  .version {
    margin-left: 4px;
    font-weight: 500;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }

  /* Mientras llegan los permisos: una línea quieta del alto de una fila. */
  .cargando {
    min-height: 60px;
  }
  .linea {
    width: 55%;
    height: 12px;
    border-radius: 6px;
    background: var(--superficie-elevada);
  }
</style>
