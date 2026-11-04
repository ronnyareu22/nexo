<!--
  La ventana de Nexo: barra lateral a la izquierda (los botones de macOS quedan encima de ella) y, a la derecha,
  una fila fina con el buscador global, Actualizar y el botón de tema, y debajo la vista.
  Aquí se cargan los datos y se pasan a las vistas: aparatos y pantallas cada 5 s (solo con el panel a la vista),
  el nombre del usuario una vez y el brillo 4 veces por segundo solo en Inicio y Pantallas.
  Para revisar el diseño: `?ver=<id>` abre Dispositivos con ese aparato en el panel, `?pagina=<id>` abre su
  página completa y `?seccion=<id>` abre una sección. El widget de escritorio abre el panel con esas mismas
  direcciones o, si ya está abierto, le manda "nexo-ir".
  Antes del panel va la puerta de entrada: la bienvenida la primera vez y, en Mac, el paso de permisos si falta
  alguno obligatorio. Mientras se ve, no se cargan datos ni se lee el brillo. `?bienvenida=1` la fuerza para
  revisarla (y `?bienvenida=permisos`, fuera de la app, simula que falta un permiso).
  En Windows hay barra de título del sistema: <html data-plataforma="windows"> achica el espacio de arriba.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { fade } from 'svelte/transition';
  import { cambiarConexion, leerBrillos, leerNombreUsuario, listarDispositivos, listarPantallas } from '$lib/api';
  import {
    alPedirSeccion,
    avisarTemaAlWidget,
    leerBienvenida,
    leerPermisos,
    permisosFaltantes,
    permisosRequeridos,
    plataforma,
  } from '$lib/api-sistema';
  import type { Tema } from '$lib/tema';
  import type { Dispositivo, Estado, Pantalla } from '$lib/tipos';
  import type { Permisos } from '$lib/tipos-sistema';
  import { nivelPrincipal, nombreDe } from '$lib/tipos';
  import { tooltip } from '$lib/tooltip';
  import BarraLateral, { type EstadoGeneral } from '$lib/componentes/BarraLateral.svelte';
  import Buscador from '$lib/componentes/Buscador.svelte';
  import BotonTema from '$lib/componentes/BotonTema.svelte';
  import Icono, { type Nombre } from '$lib/componentes/Icono.svelte';
  import VistaInicio from '$lib/vistas/Inicio.svelte';
  import VistaDispositivos from '$lib/vistas/Dispositivos.svelte';
  import VistaAudio from '$lib/vistas/Audio.svelte';
  import VistaPantallas from '$lib/vistas/Pantallas.svelte';
  import VistaRemoto from '$lib/vistas/Remoto.svelte';
  import VistaAutomatizaciones from '$lib/vistas/Automatizaciones.svelte';
  import VistaAjustes from '$lib/vistas/Ajustes.svelte';
  import VistaBienvenida from '$lib/vistas/Bienvenida.svelte';

  type Seccion = 'inicio' | 'dispositivos' | 'audio' | 'pantallas' | 'remoto' | 'automatizaciones' | 'ajustes';

  // Antes de pintar: en Mac los botones de la ventana van sobre la barra lateral; en Windows no (BarraLateral).
  document.documentElement.dataset.plataforma = plataforma;

  // Widgets y Teclado y ratón todavía no (decisión del dueño, 3 nov).
  const secciones: { id: Seccion; nombre: string; icono: Nombre }[] = [
    { id: 'inicio', nombre: 'Inicio', icono: 'inicio' },
    { id: 'dispositivos', nombre: 'Dispositivos', icono: 'dispositivos' },
    { id: 'audio', nombre: 'Audio', icono: 'audio' },
    { id: 'pantallas', nombre: 'Pantallas', icono: 'pantallas' },
    { id: 'remoto', nombre: 'Remoto', icono: 'remoto' },
    { id: 'automatizaciones', nombre: 'Automatizaciones', icono: 'automatizaciones' },
    { id: 'ajustes', nombre: 'Ajustes', icono: 'engranaje' },
  ];
  const esSeccion = (s: unknown): s is Seccion => secciones.some((x) => x.id === s);

  // La última sección se recuerda en este equipo.
  const CLAVE_SECCION = 'nexo-seccion';
  function seccionGuardada(): Seccion {
    try {
      const s = localStorage.getItem(CLAVE_SECCION);
      return esSeccion(s) ? s : 'inicio';
    } catch {
      return 'inicio';
    }
  }

  // Para revisar el diseño: ?ver=<id> abre un aparato en el panel de Dispositivos, ?pagina=<id> su página
  // completa y ?seccion=<id> una sección.
  const parametros = new URLSearchParams(location.search);
  const ver = parametros.get('ver');
  const paginaPedida = parametros.get('pagina');
  const pedida = parametros.get('seccion');
  const aparatoPedido = paginaPedida ?? ver;
  let seccion = $state<Seccion>(
    aparatoPedido ? 'dispositivos' : esSeccion(pedida) ? pedida : seccionGuardada(),
  );
  // El aparato elegido en Dispositivos: se mantiene al ir y volver de otras secciones.
  let dispositivoElegido = $state<string | null>(aparatoPedido);
  // Si Dispositivos muestra la página completa del aparato (true) o la lista con el panel (false).
  let paginaDispositivo = $state(!!paginaPedida);
  // Lo escrito en el buscador de arriba; lo filtra Dispositivos.
  let busqueda = $state('');

  // --- Puerta de entrada ---
  // 'revisando' mientras responde Rust (ventana en blanco, sin asomar el panel), 'bienvenida' la primera vez,
  // 'permisos' si ya se hizo pero falta un permiso obligatorio (Mac) y 'panel' para lo de siempre.
  type Entrada = 'revisando' | 'bienvenida' | 'permisos' | 'panel';
  let entrada = $state<Entrada>('revisando');
  let nombreBienvenida = $state('');
  let permisosEntrada = $state<Permisos | null>(null);
  const forzarBienvenida = parametros.get('bienvenida') === '1';

  /** Lee los permisos y dice si falta alguno obligatorio. En Windows no hay ninguno. */
  async function faltaPermiso(): Promise<boolean> {
    if (!permisosRequeridos.length) return false;
    try {
      const p = await leerPermisos();
      permisosEntrada = p;
      return permisosFaltantes(p).length > 0;
    } catch {
      return false;
    }
  }

  async function revisarEntrada() {
    try {
      const b = await leerBienvenida();
      nombreBienvenida = b.nombre.trim();
      if (!nombre) nombre = nombreBienvenida;
      if (!b.completa || forzarBienvenida) {
        entrada = 'bienvenida';
        return;
      }
    } catch {
      // Si Rust no responde, la ventana no se queda en blanco: sigue, pero los permisos se revisan igual.
      if (forzarBienvenida) {
        entrada = 'bienvenida';
        return;
      }
    }
    entrada = (await faltaPermiso()) ? 'permisos' : 'panel';
  }

  // Terminó la bienvenida (va a Inicio) o volvió el permiso que faltaba (sigue donde estaba).
  function alTerminarEntrada(nombreNuevo: string) {
    if (nombreNuevo) nombre = nombreNuevo;
    if (entrada === 'bienvenida') ir('inicio');
    entrada = 'panel';
  }

  let estado = $state<Estado | null>(null);
  let pantallas = $state<Pantalla[]>([]);
  let nombre = $state('');
  let aviso = $state<string | null>(null);
  let girando = $state(false);
  let pendienteId = $state<string | null>(null);

  const sinMovimiento =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const dur = (ms: number) => (sinMovimiento ? 0 : ms);

  const titulo = $derived(secciones.find((s) => s.id === seccion)?.nombre ?? '');

  // Estado general en la tarjeta del pie de la barra lateral.
  const estadoGeneral = $derived.by((): EstadoGeneral | undefined => {
    if (!estado) return undefined;
    if (!estado.bluetoothEncendido)
      return { titulo: 'Bluetooth apagado', detalle: 'Enciéndelo para conectar', tono: 'error' };
    let baja: { d: Dispositivo; nivel: number } | null = null;
    for (const d of estado.dispositivos) {
      const nivel = d.conectado ? nivelPrincipal(d) : null;
      if (nivel !== null && nivel <= 20 && (!baja || nivel < baja.nivel)) baja = { d, nivel };
    }
    if (baja) return { titulo: 'Batería baja', detalle: `${nombreDe(baja.d)} al ${baja.nivel} %`, tono: 'aviso' };
    return { titulo: 'Bluetooth activo', detalle: 'Todo en perfecto estado', tono: 'ok' };
  });

  // Cambia de sección. A Dispositivos con un aparato abre su página completa (así se entra desde Inicio);
  // sin aparato (la barra lateral, "Ver todos") vuelve a la lista.
  function ir(destino: string, id?: string) {
    if (!esSeccion(destino)) return;
    if (destino === 'dispositivos') {
      if (id) dispositivoElegido = id;
      paginaDispositivo = !!id;
    } else {
      // El buscador es de Dispositivos: al irse a otra sección queda vacío.
      busqueda = '';
    }
    seccion = destino;
    try {
      localStorage.setItem(CLAVE_SECCION, destino);
    } catch {
      // Sin almacenamiento: se recuerda hasta cerrar el panel.
    }
  }

  // Al escribir en el buscador se va a la lista de Dispositivos (cierra la página de un aparato).
  function alBuscar() {
    if (seccion !== 'dispositivos') ir('dispositivos');
    else paginaDispositivo = false;
  }

  let temporizadorAviso: ReturnType<typeof setTimeout> | undefined;
  function mostrarAviso(texto: string) {
    aviso = texto;
    clearTimeout(temporizadorAviso);
    temporizadorAviso = setTimeout(() => (aviso = null), 6000);
  }

  async function actualizar() {
    try {
      const [dispositivosNuevos, pantallasNuevas] = await Promise.all([listarDispositivos(), listarPantallas()]);
      estado = dispositivosNuevos;
      pantallas = pantallasNuevas;
    } catch (e) {
      mostrarAviso(String(e));
    }
  }

  async function actualizarAMano() {
    if (girando) return;
    girando = true;
    await Promise.all([actualizar(), new Promise((r) => setTimeout(r, 500))]);
    girando = false;
  }

  async function alternar(d: Dispositivo) {
    if (pendienteId) return;
    pendienteId = d.id;
    try {
      await cambiarConexion(d.id, !d.conectado);
    } catch (e) {
      mostrarAviso(String(e));
    }
    await actualizar();
    pendienteId = null;
  }

  onMount(() => {
    revisarEntrada();
    // Una casilla del widget pidió una sección con el panel ya abierto.
    const dejarDeEscuchar = alPedirSeccion((d) => ir(d.seccion, d.dispositivo ?? undefined));
    // El widget sigue el tema que se elige aquí.
    const alCambiarTema = (e: Event) => avisarTemaAlWidget((e as CustomEvent<Tema>).detail);
    window.addEventListener('nexo-tema', alCambiarTema);
    return () => {
      clearTimeout(temporizadorAviso);
      dejarDeEscuchar();
      window.removeEventListener('nexo-tema', alCambiarTema);
    };
  });

  // Los datos se cargan solo con el panel a la vista (no durante la bienvenida).
  $effect(() => {
    if (entrada !== 'panel') return;
    untrack(() => {
      actualizar();
      if (!nombre)
        leerNombreUsuario()
          .then((n) => (nombre = n.trim()))
          .catch(() => {});
    });
    // Se refresca solo mientras el panel está a la vista; al volver a verlo, enseguida.
    const intervalo = setInterval(() => {
      if (document.visibilityState === 'visible' && !pendienteId) actualizar();
    }, 5000);
    const alVolver = () => {
      if (document.visibilityState === 'visible' && !pendienteId) actualizar();
    };
    document.addEventListener('visibilitychange', alVolver);
    // Mac: si al volver a la ventana falta un permiso obligatorio, Nexo se bloquea de nuevo hasta que vuelva.
    const alEnfocar = async () => {
      if (await faltaPermiso()) entrada = 'permisos';
    };
    if (permisosRequeridos.length) window.addEventListener('focus', alEnfocar);
    return () => {
      clearInterval(intervalo);
      document.removeEventListener('visibilitychange', alVolver);
      window.removeEventListener('focus', alEnfocar);
    };
  });

  // El brillo se lee 4 veces por segundo solo en Inicio y Pantallas, para seguir las teclas al instante.
  $effect(() => {
    if (entrada !== 'panel') return;
    if (seccion !== 'inicio' && seccion !== 'pantallas') return;
    const brillo = setInterval(async () => {
      if (document.visibilityState !== 'visible' || !pantallas.length) return;
      try {
        for (const [id, valor] of await leerBrillos()) {
          const p = pantallas.find((x) => x.id === id);
          if (p && p.brillo !== valor) p.brillo = valor;
        }
      } catch {
        // Si falla una lectura, la siguiente lo corrige.
      }
    }, 250);
    return () => clearInterval(brillo);
  });
</script>

{#if entrada === 'revisando'}
  <!-- Un instante en blanco mientras Rust responde: ni el panel ni la bienvenida asoman antes de tiempo. -->
  <div class="espera" data-tauri-drag-region aria-busy="true"><span class="solo-lector">Abriendo Nexo…</span></div>
{:else if entrada === 'bienvenida'}
  <VistaBienvenida nombreInicial={nombreBienvenida} onlisto={alTerminarEntrada} />
{:else if entrada === 'permisos'}
  <VistaBienvenida
    soloPermisos
    nombreInicial={nombre || nombreBienvenida}
    permisosIniciales={permisosEntrada}
    onlisto={alTerminarEntrada}
  />
{:else}
  <div class="app" in:fade={{ duration: dur(220) }}>
    <BarraLateral {secciones} actual={seccion} onelegir={ir} estado={estadoGeneral} />

    <div class="principal">
      <!-- Arriba: el buscador a la izquierda y, a la derecha, Actualizar y el tema. Lo vacío arrastra la ventana. -->
      <header class="barra" data-tauri-drag-region>
        <!-- Cada sección ya muestra su título grande; aquí queda solo para lectores de pantalla. -->
        <h1 class="solo-lector">{titulo}</h1>
        <Buscador bind:valor={busqueda} onescribir={alBuscar} />
        <span class="hueco" data-tauri-drag-region></span>
        <div class="acciones" data-tauri-drag-region>
          <button
            class="boton-redondo"
            class:girando
            onclick={actualizarAMano}
            aria-label="Actualizar"
            aria-busy={girando}
            use:tooltip={'Actualizar'}
          >
            <Icono nombre="actualizar" tamano={18} />
          </button>
          <BotonTema />
        </div>
      </header>

      <main>
        {#key seccion}
          <div
            class="vista"
            class:fija={seccion === 'dispositivos'}
            class:sin-desborde={seccion === 'inicio'}
            in:fade={{ duration: dur(220) }}
          >
            {#if seccion === 'inicio'}
              <VistaInicio {estado} {pantallas} {nombre} onir={ir} onalternar={alternar} {pendienteId} />
            {:else if seccion === 'dispositivos'}
              <VistaDispositivos
                {estado}
                {pendienteId}
                onalternar={alternar}
                seleccionInicial={dispositivoElegido}
                onerror={mostrarAviso}
                onseleccion={(id) => (dispositivoElegido = id)}
                onactualizar={actualizar}
                bind:busqueda
                bind:pagina={paginaDispositivo}
              />
            {:else if seccion === 'audio'}
              <VistaAudio onerror={mostrarAviso} />
            {:else if seccion === 'pantallas'}
              <VistaPantallas {pantallas} onerror={mostrarAviso} />
            {:else if seccion === 'remoto'}
              <VistaRemoto onerror={mostrarAviso} />
            {:else if seccion === 'automatizaciones'}
              <VistaAutomatizaciones dispositivos={estado?.dispositivos ?? []} onerror={mostrarAviso} />
            {:else if seccion === 'ajustes'}
              <VistaAjustes {nombre} onnombre={(n) => (nombre = n)} onerror={mostrarAviso} />
            {/if}
          </div>
        {/key}
      </main>
    </div>

    {#if aviso}
      <div class="aviso flotante" role="alert" transition:fade={{ duration: dur(160) }}>
        <Icono nombre="alerta" tamano={17} />
        <span>{aviso}</span>
        <button class="cerrar" onclick={() => (aviso = null)} aria-label="Cerrar aviso">
          <Icono nombre="cerrar" tamano={15} />
        </button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .espera {
    height: 100vh;
    background: var(--fondo);
  }

  .app {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    height: 100vh;
    background: var(--fondo);
  }

  .principal {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  /* Fila de arriba: alineada con la zona de arrastre de la barra lateral (52 px). En Windows, bajo la barra de
     título del sistema, mide lo mismo y su centro coincide con el de la marca de la barra lateral. */
  .barra {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
    height: 52px;
    padding: 0 32px;
  }
  .hueco {
    flex: 1;
    align-self: stretch;
  }
  .acciones {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
  /* Mientras actualiza, el ícono gira (es un indicador de carga). */
  .girando :global(svg) {
    animation: girar 700ms linear infinite;
  }
  @keyframes girar {
    to {
      transform: rotate(360deg);
    }
  }

  main {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .vista {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    padding: 8px 32px 32px;
  }
  /* En Inicio la aurora de la mascota puede pasarse del borde: se recorta ahí, sin barra horizontal. */
  .vista.sin-desborde {
    overflow-x: hidden;
  }
  /* Dispositivos maneja su propio desplazamiento (lista y detalle por separado). */
  .vista.fija {
    overflow: hidden;
    padding-bottom: 24px;
  }

  .aviso {
    position: fixed;
    z-index: 40;
    left: 50%;
    bottom: 22px;
    translate: -50% 0;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: min(560px, calc(100vw - 40px));
    padding: 10px 10px 10px 14px;
    border-radius: var(--radio-md);
    color: var(--texto);
    font-size: 13px;
  }
  .aviso > :global(svg:first-child) {
    flex-shrink: 0;
    color: var(--error);
  }
  .cerrar {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 26px;
    height: 26px;
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
</style>
