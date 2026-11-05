<!-- Ajustes: apariencia, tu nombre, arranque, widget de escritorio, permisos, tus equipos (el trabajo remoto:
     Automático, Siempre o Nunca, y qué equipo encontró Nexo), la casa (buscar, tu red y las cuentas opcionales),
     historial y "Acerca de", en filas como los Ajustes del Sistema. Mientras falte un permiso, se revisa cada 3 s
     para mostrar cuando ya está. Con `foco="casa"` (desde Casa o Mi setup) se baja a la sección de la casa. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    abrirAjustesPrivacidad,
    borrarHistorial,
    esMac,
    guardarNombre,
    leerArranque,
    leerPermisos,
    leerWidget,
    usarArranque,
    usarWidget,
  } from '$lib/api-sistema';
  import type { ModoRemoto, Permisos, SeccionPrivacidad } from '$lib/tipos-sistema';
  import { olvidarOtroEquipo, ponerModoRemoto } from '$lib/api-equipos';
  import { nombreTipo, perfil, ponerPerfil, recargar } from '$lib/perfil.svelte';
  import MenuDesplegable from '$lib/componentes/MenuDesplegable.svelte';
  import Icono from '$lib/componentes/Icono.svelte';
  import Interruptor from '$lib/componentes/Interruptor.svelte';
  import SelectorTema from '$lib/componentes/SelectorTema.svelte';
  import NexoBot from '$lib/componentes/NexoBot.svelte';
  import Dialogo from '$lib/componentes/Dialogo.svelte';
  import FilaActualizacion from '$lib/componentes/FilaActualizacion.svelte';
  import AjustesCasa from '$lib/componentes/AjustesCasa.svelte';

  let {
    nombre = '',
    onnombre,
    onerror,
    onir,
    foco = null,
  }: {
    /** El nombre con que Nexo saluda en Inicio. */
    nombre?: string;
    /** Se guardó un nombre nuevo (para que Inicio salude con él). */
    onnombre?: (nombre: string) => void;
    onerror: (texto: string) => void;
    /** Lleva a otra sección (la guía para conectar tus equipos, en Remoto; Casa). */
    onir?: (seccion: string, id?: string) => void;
    /** "casa": baja a la sección de la casa al abrir. */
    foco?: string | null;
  } = $props();

  // --- Tus equipos: el trabajo remoto ---
  // Lo que busca este equipo: en Windows tu Mac; en la Mac tu PC.
  const buscado = esMac ? 'PC' : 'Mac';
  const opcionesRemoto: { valor: ModoRemoto; nombre: string; detalle: string }[] = [
    { valor: 'auto', nombre: 'Automático', detalle: 'Según tus equipos' },
    { valor: 'siempre', nombre: 'Siempre', detalle: `Aunque no encuentre tu ${buscado}` },
    { valor: 'nunca', nombre: 'Nunca', detalle: 'Oculta el trabajo remoto' },
  ];
  let cambiandoModo = $state(false);
  let preguntandoOlvidar = $state(false);
  /** Se olvidó, pero sigue en Tailscale y se volvió a encontrar. */
  let volvio = $state(false);

  async function cambiarModo(valor: string) {
    if (cambiandoModo) return;
    cambiandoModo = true;
    try {
      ponerPerfil(await ponerModoRemoto(valor as ModoRemoto));
    } catch (e) {
      onerror(String(e));
    }
    cambiandoModo = false;
  }

  async function olvidar() {
    preguntandoOlvidar = false;
    try {
      const e = await olvidarOtroEquipo();
      ponerPerfil(e);
      volvio = !!e.otro;
    } catch (err) {
      onerror(String(err));
    }
  }

  const detectado = $derived.by((): { titulo: string; detalle: string; tono: 'ok' | 'apagado' } | null => {
    const e = perfil.estado;
    if (!e) return null;
    const o = e.otro;
    if (o) {
      const tipo = nombreTipo(o.plataforma);
      if (volvio)
        return {
          titulo: `Encontré tu ${tipo} ${o.nombre}`,
          detalle: 'Sigue en tu cuenta de Tailscale, así que la volví a encontrar. Para no usarla, elige Nunca.',
          tono: o.nexo ? 'ok' : 'apagado',
        };
      if (!o.encontrado)
        return {
          titulo: `Recuerdo tu ${tipo} ${o.nombre}`,
          detalle: 'Ahora no aparece en Tailscale. El trabajo remoto sigue listo para cuando vuelva.',
          tono: 'apagado',
        };
      return {
        titulo: `Encontré tu ${tipo} ${o.nombre}`,
        detalle: o.nexo ? 'En línea, con Nexo abierto.' : 'Ahora no responde: puede estar apagada o con Nexo cerrado.',
        tono: o.nexo ? 'ok' : 'apagado',
      };
    }
    return {
      titulo: 'No encontré otro equipo tuyo en Tailscale',
      detalle:
        e.tailscale === 'no instalado'
          ? 'Instala Tailscale en tus dos equipos con la misma cuenta.'
          : e.tailscale !== 'activo'
            ? 'Conecta Tailscale en este equipo para buscar tu ' + buscado + '.'
            : `Cuando aparezca tu ${buscado}, el trabajo remoto se enciende solo.`,
      tono: 'apagado',
    };
  });

  const VERSION = '0.1.0';

  // --- Tu nombre ---
  let cambiandoNombre = $state(false);
  let nombreNuevo = $state('');
  let errorNombre = $state<string | null>(null);

  function abrirNombre() {
    nombreNuevo = nombre;
    errorNombre = null;
    cambiandoNombre = true;
  }

  async function confirmarNombre() {
    const texto = nombreNuevo.trim();
    if (!texto) {
      errorNombre = 'Escribe tu nombre.';
      return;
    }
    try {
      const guardado = await guardarNombre(texto);
      onnombre?.(guardado);
      cambiandoNombre = false;
    } catch (e) {
      errorNombre = String(e);
    }
  }

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

  // --- Widget de escritorio ---
  let widget = $state<boolean | null>(null);
  let cambiandoWidget = $state(false);

  async function alternarWidget() {
    if (widget === null) return;
    cambiandoWidget = true;
    try {
      widget = await usarWidget(!widget);
    } catch (e) {
      onerror(String(e));
      widget = await leerWidget().catch(() => widget);
    }
    cambiandoWidget = false;
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
      lista.push({
        seccion: 'pantalla',
        nombre: 'Grabación de pantalla',
        para: 'Para las capturas con Imprimir Pantalla desde tu PC.',
        ...estadoDe(permisos.pantalla),
      });
    }
    lista.push(
      {
        seccion: 'bluetooth',
        nombre: 'Bluetooth',
        para: 'Para ver, conectar y desconectar tus aparatos.',
        ...estadoDe(permisos.bluetooth),
      },
      // En Mac llega el estado real. En Windows siempre llega concedido: no se muestra un estado que no es cierto.
      esMac
        ? {
            seccion: 'notificaciones',
            nombre: 'Notificaciones',
            para: 'Para el aviso de batería baja.',
            ...estadoDe(permisos.notificaciones),
          }
        : {
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
    // Se baja cuando ya llegaron los permisos y las cuentas (antes, la página todavía crece).
    if (foco === 'casa')
      setTimeout(() => document.getElementById('ajustes-casa')?.scrollIntoView({ block: 'start' }), 450);
    recargar();
    leerArranque()
      .then((a) => (arranque = a))
      .catch((e) => onerror(String(e)));
    leerWidget()
      .then((w) => (widget = w))
      .catch((e) => onerror(String(e)));
    // El widget también se enciende o se apaga desde el ícono y desde el propio widget: al volver, se relee.
    const alVolver = () => {
      if (!cambiandoWidget) leerWidget().then((w) => (widget = w)).catch(() => {});
    };
    window.addEventListener('focus', alVolver);

    const leer = () =>
      leerPermisos()
        .then((p) => (permisos = p))
        .catch(() => {});
    leer();
    const intervalo = setInterval(() => {
      if (faltaAlguno && document.visibilityState === 'visible') leer();
    }, 3000);
    return () => {
      clearInterval(intervalo);
      window.removeEventListener('focus', alVolver);
    };
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
            <div class="titulo">Tu nombre</div>
            <div class="ayuda">
              {nombre ? `Nexo te saluda como «${nombre}» en Inicio.` : 'Escríbelo para que Nexo te salude en Inicio.'}
            </div>
          </div>
          <button class="boton-vidrio boton" type="button" aria-label="Cambiar tu nombre" onclick={abrirNombre}>
            Cambiar
          </button>
        </div>
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
        {#if esMac}
          <!-- Los widgets nativos los agrega la persona desde macOS; aquí solo se explica cómo. -->
          <div class="fila">
            <div class="textos">
              <div class="titulo">Widgets de Nexo</div>
              <div class="ayuda">
                Brillo, Audífonos y Resumen, junto al Calendario y el Clima. Para ponerlos: clic derecho en el escritorio ›
                Editar widgets › busca «Nexo» y arrástralos.
              </div>
            </div>
          </div>
        {/if}
        <div class="fila">
          <div class="textos">
            <div class="titulo">{esMac ? 'Tarjeta de Nexo en el escritorio' : 'Widget en el escritorio'}</div>
            <div class="ayuda">Una tarjeta pequeña en el escritorio con tus audífonos, pantallas y Bluetooth.</div>
          </div>
          {#if widget !== null}
            <Interruptor
              encendido={widget}
              cargando={cambiandoWidget}
              etiqueta="Widget en el escritorio"
              oncambio={alternarWidget}
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
      <section class="panel vidrio" aria-labelledby="ajustes-equipos">
        <h2 class="rotulo" id="ajustes-equipos">Tus equipos</h2>
        <div class="fila con-menu">
          <div class="textos">
            <div class="titulo">Trabajo remoto</div>
            <div class="ayuda">
              {esMac
                ? 'Tus atajos de Windows en esta Mac, la captura con Imprimir Pantalla y la pantalla virtual.'
                : 'Conectarte a tu Mac con un clic, su pantalla virtual y tus atajos de Windows en ella.'}
            </div>
          </div>
          <div class="menu" aria-busy={cambiandoModo}>
            <MenuDesplegable
              opciones={opcionesRemoto}
              valor={perfil.estado?.modo ?? 'auto'}
              etiqueta="Cuándo se activa el trabajo remoto"
              oncambio={cambiarModo}
            />
          </div>
        </div>
        {#if detectado}
          <div class="fila">
            <div class="textos">
              <div class="titulo con-punto">
                <span class="punto-estado {detectado.tono}" aria-hidden="true"></span>
                <span>{detectado.titulo}</span>
              </div>
              <div class="ayuda">{detectado.detalle}</div>
            </div>
            {#if perfil.otro}
              <button class="boton-vidrio boton" type="button" onclick={() => (preguntandoOlvidar = true)}>
                Olvidar
              </button>
            {:else if onir}
              {@const ir = onir}
              <button class="boton-vidrio boton" type="button" onclick={() => ir('remoto')}>Ver la guía</button>
            {/if}
          </div>
        {:else}
          <div class="fila cargando" aria-busy="true"><span class="linea"></span><span class="solo-lector">Buscando tus equipos…</span></div>
        {/if}
      </section>

      <AjustesCasa {onerror} {onir} />

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
        <!-- Cuándo se compiló y, en Windows, buscar una versión nueva en tu Mac. -->
        <FilaActualizacion {onerror} />
      </section>
    </div>
  </div>
</div>

<Dialogo
  abierto={cambiandoNombre}
  titulo="Tu nombre"
  descripcion="Así te saludo en Inicio."
  icono="editar"
  textoConfirmar="Guardar"
  onconfirmar={confirmarNombre}
  oncancelar={() => (cambiandoNombre = false)}
>
  <input
    class="campo"
    type="text"
    bind:value={nombreNuevo}
    placeholder="Tu nombre"
    aria-label="Tu nombre"
    aria-invalid={!!errorNombre}
    aria-describedby={errorNombre ? 'ajustes-error-nombre' : undefined}
    maxlength="30"
    autocomplete="given-name"
    spellcheck="false"
    oninput={() => (errorNombre = null)}
    data-autofocus
  />
  {#if errorNombre}
    <p class="error-campo" id="ajustes-error-nombre" role="alert">{errorNombre}</p>
  {/if}
</Dialogo>

<Dialogo
  abierto={preguntandoOlvidar}
  titulo={perfil.otro ? `¿Olvidar tu ${nombreTipo(perfil.otro.plataforma)} ${perfil.otro.nombre}?` : '¿Olvidar este equipo?'}
  descripcion="Nexo deja de recordarla para el trabajo remoto. Si sigue en tu cuenta de Tailscale, la vuelve a encontrar."
  icono="papelera"
  textoConfirmar="Olvidar"
  textoCancelar="Cancelar"
  onconfirmar={olvidar}
  oncancelar={() => (preguntandoOlvidar = false)}
/>

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

  /* El modo del trabajo remoto: si el menú no cabe al lado del texto, baja a su propia línea. */
  .fila.con-menu {
    flex-wrap: wrap;
    row-gap: 10px;
  }
  .con-menu .textos {
    flex: 1 1 220px;
  }
  .menu {
    flex: 0 1 220px;
    min-width: 170px;
  }
  .con-punto {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .punto-estado {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--texto-3);
  }
  .punto-estado.ok {
    background: var(--exito);
  }
  .version {
    margin-left: 4px;
    font-weight: 500;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }

  /* Campo del diálogo "Tu nombre" (va dentro de Dialogo, que está al final de <body>) */
  .campo {
    width: 100%;
    height: 40px;
    padding: 0 12px;
    border: 1px solid var(--borde-fuerte);
    border-radius: var(--radio-sm);
    outline: none;
    background: var(--superficie);
    color: var(--texto);
    font: inherit;
    font-size: 14px;
    user-select: text;
    -webkit-user-select: text;
    transition: border-color var(--dur-color) ease;
  }
  .campo:focus {
    border-color: var(--primario);
  }
  .campo[aria-invalid='true'] {
    border-color: var(--error);
  }
  .error-campo {
    margin: 8px 2px 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--error);
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
