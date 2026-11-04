<!--
  Remoto: la Mac desde el PC del trabajo, con RustDesk por Tailscale.
  Arriba, el estado de la sesión con su ilustración (el PC de la oficina, la Mac de la casa y Tailscale al medio:
  conectada, lista o apagada), las acciones y los servicios en fichas.
  Debajo, a lo ancho, los atajos de Windows que Nexo traduce en la Mac (con espacio, en dos columnas de filas).
  Luego, en dos columnas parejas: la captura con Imprimir Pantalla y la comodidad durante la sesión, y el
  portapapeles liviano con su historial.
  En Windows solo se conecta: la traducción, la captura y el portapapeles los hace Nexo en la Mac.
  Se relee al llegar "remoto-cambio" y cada 5 s mientras se ve; el historial, al llegar "portapapeles-cambio".
  Superficies sólidas, el hover solo cambia el color y ningún texto se corta: todo baja de línea.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    abrirAjustesPrivacidad,
    abrirCapturas,
    abrirRustDesk,
    ajustarRemoto,
    alCambiarPortapapeles,
    alCambiarRemoto,
    alCapturar,
    borrarPortapapeles,
    capturarRemoto,
    conectarMac,
    copiarDelHistorial,
    esMac,
    leerPortapapeles,
    leerRemoto,
    pedirPermiso,
    prepararRemoto,
  } from '$lib/api-sistema';
  import type {
    AjustesRemoto,
    AtajoRemoto,
    CambioRemoto,
    CapturaRemota,
    EntradaPortapapeles,
    EstadoRemoto,
  } from '$lib/tipos-sistema';
  import { cuando } from '$lib/tiempo';
  import { tooltip } from '$lib/tooltip';
  import Icono, { type Nombre } from '$lib/componentes/Icono.svelte';
  import Interruptor from '$lib/componentes/Interruptor.svelte';
  import Dialogo from '$lib/componentes/Dialogo.svelte';
  import IlustracionEscena from '$lib/componentes/IlustracionEscena.svelte';

  let { onerror }: { onerror: (texto: string) => void } = $props();

  let remoto = $state<EstadoRemoto | null>(null);
  let historial = $state<EntradaPortapapeles[] | null>(null);

  // --- Lectura ---
  // Cada cambio de ajuste sube la versión: una lectura que empezó antes no pisa lo recién elegido.
  let version = 0;
  let leyendo = false;
  async function leer() {
    if (leyendo) return;
    leyendo = true;
    const desde = version;
    try {
      const r = await leerRemoto();
      if (desde === version) remoto = r;
    } catch (e) {
      if (!remoto) onerror(String(e));
    }
    leyendo = false;
  }

  async function leerHistorial() {
    if (!esMac) return;
    try {
      historial = await leerPortapapeles();
    } catch {
      historial ??= [];
    }
  }

  // --- Ajustes ---
  type Clave = keyof AjustesRemoto | 'modoTrabajo';
  let ocupado = $state<Clave | null>(null);

  async function ajustar(clave: Clave, valor: boolean) {
    if (!remoto || ocupado) return;
    ocupado = clave;
    version++;
    const cambio: CambioRemoto = {};
    cambio[clave] = valor;
    try {
      remoto = await ajustarRemoto(cambio);
    } catch (e) {
      onerror(String(e));
    }
    ocupado = null;
  }

  // --- Estado de arriba ---
  type Tono = 'ok' | 'aviso' | 'apagado';

  const resumen = $derived.by((): { titulo: string; detalle: string; tono: Tono } | null => {
    const r = remoto;
    if (!r) return null;
    if (!esMac)
      return {
        titulo: 'Tu Mac, a un clic',
        detalle: !r.rustdesk
          ? 'Instala RustDesk desde rustdesk.com y vuelve aquí: el botón se activa solo.'
          : r.tailscale !== 'activo'
            ? 'Enciende Tailscale en esta PC para llegar directo a tu Mac.'
            : `Abre RustDesk conectado directo a tu Mac por Tailscale (${r.ipMac}).`,
        tono: r.rustdesk && r.tailscale === 'activo' ? 'ok' : 'apagado',
      };
    if (r.activa && r.rele)
      return {
        titulo: 'Conectado por el servidor de RustDesk',
        detalle: `Va más lento. Para ir directo, en RustDesk escribe ${r.ipMac} en lugar del ID.`,
        tono: 'aviso',
      };
    if (r.activa)
      return {
        titulo: `Conectado desde ${r.equipo ?? r.ipRemota ?? 'tu PC'}`,
        detalle: r.directa
          ? `Directo por Tailscale${r.equipo && r.ipRemota ? ` · ${r.ipRemota}` : ''}`
          : 'Directo por la red',
        tono: 'ok',
      };
    return {
      titulo: 'Sin sesión remota',
      detalle: r.modoTrabajo
        ? `Lista para el trabajo. Desde el PC, en RustDesk escribe ${r.ipMac}.`
        : 'Cuando te conectes desde el trabajo, lo verás aquí.',
      tono: 'apagado',
    };
  });

  // La línea entre el PC y la Mac: viva (directa), por el relé o sin sesión.
  const ruta = $derived.by((): { estado: 'viva' | 'rele' | 'apagada'; via: string | null } => {
    const r = remoto;
    if (!r || !esMac || !r.activa) return { estado: 'apagada', via: null };
    if (r.rele) return { estado: 'rele', via: 'Relé' };
    return { estado: 'viva', via: r.directa ? 'Tailscale' : 'Red' };
  });

  // La ilustración de arriba: conectada (línea firme y visto), lista (línea punteada) o apagada (sin sesión).
  const escenaRemoto = $derived(
    ruta.estado === 'viva'
      ? 'remoto-conectado'
      : ruta.estado === 'rele' || resumen?.tono === 'ok' || remoto?.modoTrabajo
        ? 'remoto'
        : 'remoto-sin',
  );

  const servicios = $derived.by((): { nombre: string; valor: string; tono: Tono }[] => {
    const r = remoto;
    if (!r) return [];
    const tailscale: Record<EstadoRemoto['tailscale'], [string, Tono]> = {
      activo: ['Conectado', 'ok'],
      detenido: ['Detenido', 'aviso'],
      'no instalado': ['No instalado', 'apagado'],
    };
    const [valorTs, tonoTs] = tailscale[r.tailscale];
    const lista: { nombre: string; valor: string; tono: Tono }[] = [{ nombre: 'Tailscale', valor: valorTs, tono: tonoTs }];
    if (esMac) {
      lista.push({
        nombre: 'RustDesk',
        valor: r.rustdesk ? 'Abierto' : r.rustdeskInstalado ? 'Cerrado' : 'No instalado',
        tono: r.rustdesk ? 'ok' : r.rustdeskInstalado ? 'aviso' : 'apagado',
      });
      const despierta = r.ajustes.evitarReposo && (r.activa || r.modoTrabajo);
      lista.push({ nombre: 'Reposo', valor: despierta ? 'La Mac no se dormirá' : 'Normal', tono: despierta ? 'ok' : 'apagado' });
    } else {
      lista.push({ nombre: 'RustDesk', valor: r.rustdesk ? 'Instalado' : 'No instalado', tono: r.rustdesk ? 'ok' : 'aviso' });
    }
    return lista;
  });

  let preparando = $state(false);
  let avisoAccion = $state<{ texto: string; tono: Tono } | null>(null);
  let temporizadorAviso: ReturnType<typeof setTimeout> | undefined;
  function mostrarAviso(texto: string, tono: Tono) {
    avisoAccion = { texto, tono };
    clearTimeout(temporizadorAviso);
    temporizadorAviso = setTimeout(() => (avisoAccion = null), 9000);
  }

  async function preparar() {
    if (preparando) return;
    preparando = true;
    version++;
    try {
      const r = await prepararRemoto();
      remoto = r;
      if (r.tailscale !== 'activo')
        mostrarAviso(
          r.tailscale === 'no instalado'
            ? 'Instala Tailscale para entrar directo desde el trabajo.'
            : 'Tailscale no se conectó: haz clic en su ícono de la barra de menús y pulsa Connect.',
          'aviso',
        );
      else if (!r.rustdesk) mostrarAviso('RustDesk no se abrió. Ábrelo desde Aplicaciones.', 'aviso');
      else mostrarAviso('Listo: Tailscale conectado, RustDesk abierto y la Mac no se dormirá.', 'ok');
    } catch (e) {
      onerror(String(e));
    }
    preparando = false;
  }

  async function abrirRD() {
    try {
      await abrirRustDesk();
    } catch (e) {
      onerror(String(e));
    }
  }

  let conectando = $state(false);
  async function conectar() {
    if (conectando) return;
    conectando = true;
    try {
      await conectarMac();
      mostrarAviso('Abriendo RustDesk hacia tu Mac…', 'ok');
    } catch (e) {
      onerror(String(e));
    }
    setTimeout(() => (conectando = false), 2500);
  }

  // --- Atajos ---
  // Recogidos: lo de todos los días (los primeros de Edición y los escritorios). "Ver todos" muestra la tabla entera.
  let todos = $state(false);
  const grupos = $derived.by(() => {
    const mapa = new Map<string, AtajoRemoto[]>();
    for (const a of remoto?.atajos ?? []) {
      const lista = mapa.get(a.grupo) ?? [];
      lista.push(a);
      mapa.set(a.grupo, lista);
    }
    const lista = [...mapa].map(([nombre, atajos]) => ({ nombre, atajos }));
    if (todos) return lista;
    const breves = lista
      .filter((g) => g.nombre === 'Edición' || g.nombre === 'Escritorios y apps')
      .map((g) => (g.nombre === 'Edición' ? { ...g, atajos: g.atajos.slice(0, 5) } : g));
    return breves.length ? breves : lista.slice(0, 2);
  });
  const totalAtajos = $derived(remoto?.atajos.length ?? 0);

  /** Las teclas en palabras, para lectores de pantalla ("Comando más C"). */
  const NOMBRES_TECLAS: Record<string, string> = {
    '⌘': 'Comando',
    '⌥': 'Opción',
    '⌃': 'Control',
    '⇧': 'Mayúsculas',
    '⌫': 'Borrar',
    '⇥': 'Tab',
    '← →': 'flecha izquierda o derecha',
    '←': 'flecha izquierda',
    '→': 'flecha derecha',
    '↑': 'flecha arriba',
    '↓': 'flecha abajo',
  };
  const enPalabras = (teclas: string[]) => teclas.map((t) => NOMBRES_TECLAS[t] ?? t).join(' más ');
  const esSimbolo = (t: string) => /^[⌘⌥⌃⇧⌫⇥←→↑↓ ]+$/.test(t);

  // --- Captura ---
  let capturando = $state(false);
  let ultimaCaptura = $state<CapturaRemota | null>(null);
  let capturaCancelada = $state(false);

  async function probarCaptura() {
    if (capturando) return;
    capturando = true;
    capturaCancelada = false;
    try {
      const c = await capturarRemoto();
      if (c) ultimaCaptura = c;
      else capturaCancelada = true;
    } catch (e) {
      onerror(String(e));
    }
    capturando = false;
  }

  async function abrirCarpeta() {
    try {
      await abrirCapturas();
    } catch (e) {
      onerror(String(e));
    }
  }

  async function pedirPantalla() {
    try {
      await pedirPermiso('pantalla');
    } catch (e) {
      onerror(String(e));
    }
  }

  function abrirAccesibilidad() {
    abrirAjustesPrivacidad('accesibilidad').catch((e) => onerror(String(e)));
  }

  // --- Portapapeles ---
  const LIMITE_VISTA = 100;
  const LIMITE_TOOLTIP = 600;

  /** Las dos primeras líneas con texto, hasta unos 100 caracteres (cortando en un espacio). Sin "…": se dice cuánto falta. */
  function vista(e: EntradaPortapapeles): { texto: string; faltan: number } {
    const completo = e.texto ?? '';
    const lineas = completo
      .replace(/\r/g, '')
      .split('\n')
      .filter((l) => l.trim() !== '');
    let texto = lineas.slice(0, 2).join('\n');
    if (texto.length > LIMITE_VISTA) {
      const espacio = texto.lastIndexOf(' ', LIMITE_VISTA);
      texto = texto.slice(0, espacio > LIMITE_VISTA * 0.6 ? espacio : LIMITE_VISTA).trimEnd();
    }
    const total = e.largo ?? [...completo].length;
    return { texto, faltan: Math.max(0, total - [...texto].length) };
  }

  /** El texto completo en el tooltip, solo si no se ve entero. */
  function tooltipDe(e: EntradaPortapapeles, faltan: number): string {
    if (faltan <= 0) return '';
    const completo = e.texto ?? '';
    if ([...completo].length <= LIMITE_TOOLTIP) return completo;
    const corto = [...completo].slice(0, LIMITE_TOOLTIP).join('');
    return `${corto} (y ${miles((e.largo ?? 0) - LIMITE_TOOLTIP)} caracteres más)`;
  }

  const miles = (n: number) => n.toLocaleString('es');

  let copiadoId = $state<number | null>(null);
  let copiandoId = $state<number | null>(null);
  let temporizadorCopia: ReturnType<typeof setTimeout> | undefined;

  async function copiar(e: EntradaPortapapeles) {
    if (copiandoId !== null) return;
    copiandoId = e.id;
    try {
      await copiarDelHistorial(e.id);
      copiadoId = e.id;
      clearTimeout(temporizadorCopia);
      temporizadorCopia = setTimeout(() => (copiadoId = null), 1800);
      await leerHistorial();
    } catch (err) {
      onerror(String(err));
    }
    copiandoId = null;
  }

  let preguntandoBorrar = $state(false);
  async function borrarHistorial() {
    preguntandoBorrar = false;
    try {
      await borrarPortapapeles();
      historial = [];
    } catch (e) {
      onerror(String(e));
    }
  }

  onMount(() => {
    leer();
    leerHistorial();
    const quitar = [
      alCambiarRemoto(leer),
      alCapturar((c) => {
        ultimaCaptura = c;
        capturaCancelada = false;
      }),
      alCambiarPortapapeles(leerHistorial),
    ];
    // Cada 5 s mientras se ve; al volver a la ventana (por ejemplo, desde Ajustes del Sistema), enseguida.
    const intervalo = setInterval(() => {
      if (document.visibilityState === 'visible') leer();
    }, 5000);
    const alVolver = () => {
      if (document.visibilityState !== 'visible') return;
      leer();
      leerHistorial();
    };
    document.addEventListener('visibilitychange', alVolver);
    window.addEventListener('focus', alVolver);
    return () => {
      for (const q of quitar) q();
      clearInterval(intervalo);
      clearTimeout(temporizadorAviso);
      clearTimeout(temporizadorCopia);
      document.removeEventListener('visibilitychange', alVolver);
      window.removeEventListener('focus', alVolver);
    };
  });
</script>

{#snippet teclas(lista: string[], lado: 'windows' | 'mac')}
  <span class="teclas {lado}" aria-label="{lado === 'mac' ? 'En la Mac' : 'En Windows'}: {enPalabras(lista)}">
    {#each lista as t, i (i)}<kbd class:simbolo={esSimbolo(t)} aria-hidden="true">{t}</kbd>{/each}
  </span>
{/snippet}

{#snippet ficha(icono: Nombre, activa: boolean)}
  <span class="ficha" class:activa aria-hidden="true"><Icono nombre={icono} tamano={18} /></span>
{/snippet}

<div class="vista">
  <header class="cabecera">
    <h1>Remoto</h1>
    <p>
      {esMac
        ? 'Usa tu Mac desde el PC del trabajo con RustDesk y Tailscale.'
        : 'Conéctate a tu Mac con RustDesk por Tailscale.'}
    </p>
  </header>

  {#if remoto && resumen}
    {@const r = remoto}
    <!-- Estado de la sesión: la ilustración a la izquierda; el estado, las acciones y los servicios a la derecha -->
    <section class="estado vidrio" aria-labelledby="remoto-estado">
      <div class="estado__escena {ruta.estado}" aria-hidden="true">
        <IlustracionEscena nombre={escenaRemoto} icono="remoto" />
        {#if ruta.via}<span class="estado__via">{ruta.via}</span>{/if}
      </div>

      <div class="estado__cuerpo">
        <div class="estado__textos" role="status" aria-live="polite">
          <h2 class="estado__titulo" id="remoto-estado">
            <span class="punto {resumen.tono}" aria-hidden="true"></span>{resumen.titulo}
          </h2>
          <p class="estado__detalle">{resumen.detalle}</p>
        </div>

        <div class="estado__acciones">
          {#if esMac}
            <button
              class={r.activa ? 'boton-vidrio boton' : 'boton-primario'}
              type="button"
              disabled={preparando}
              aria-busy={preparando}
              onclick={preparar}
            >
              {preparando ? 'Preparando…' : 'Preparar para el trabajo'}
            </button>
            <button class="boton-vidrio boton" type="button" onclick={abrirRD}>Abrir RustDesk</button>
          {:else}
            <button
              class="boton-primario"
              type="button"
              disabled={conectando || !r.rustdesk}
              aria-busy={conectando}
              onclick={conectar}
            >
              <Icono nombre="conectar" tamano={17} />
              Conectar a mi Mac
            </button>
            <button class="boton-vidrio boton" type="button" disabled={!r.rustdesk} onclick={abrirRD}>Abrir RustDesk</button>
          {/if}
        </div>

        <ul class="servicios" aria-label="Servicios">
          {#each servicios as s (s.nombre)}
            <li class="servicio {s.tono}">
              <span class="punto {s.tono}" aria-hidden="true"></span>
              <span class="servicio__nombre">{s.nombre}</span>
              <span class="servicio__valor">{s.valor}</span>
            </li>
          {/each}
          <li class="servicio">
            <span class="servicio__nombre">Tu Mac</span>
            <span class="servicio__valor direccion">{r.ipMac}</span>
          </li>
        </ul>

        {#if avisoAccion}
          <p class="aviso-accion {avisoAccion.tono}" role="status">
            <Icono nombre={avisoAccion.tono === 'ok' ? 'check-circulo' : 'info'} tamano={16} />
            {avisoAccion.texto}
          </p>
        {/if}
      </div>
    </section>

    <!-- Atajos: a lo ancho; con espacio, las filas van en dos columnas -->
    <section class="panel vidrio panel-atajos" aria-labelledby="remoto-atajos">
      <div class="cabeza">
        {@render ficha('teclado', esMac && r.ajustes.traducir && r.permisoTeclas)}
        <div class="textos">
          <h2 class="titulo-panel" id="remoto-atajos">Atajos de Windows en tu Mac</h2>
          <p class="ayuda">
            {esMac
              ? 'Usa los atajos de siempre desde el PC: Nexo los traduce solo en lo que llega por RustDesk. El teclado de tu Mac no cambia.'
              : 'La traducción la hace Nexo en tu Mac: desde aquí, conéctate y usa los atajos de siempre.'}
          </p>
        </div>
        {#if esMac}
          <Interruptor
            encendido={r.ajustes.traducir}
            cargando={ocupado === 'traducir'}
            etiqueta="Traducir atajos"
            oncambio={() => ajustar('traducir', !r.ajustes.traducir)}
          />
        {/if}
      </div>

      {#if esMac && !r.permisoTeclas}
        <div class="permiso">
          <Icono nombre="alerta" tamano={16} />
          <p>Falta el permiso de <b>Accesibilidad</b>: sin él, Nexo no puede traducir ni escuchar Imprimir Pantalla.</p>
          <button class="boton-vidrio boton" type="button" onclick={abrirAccesibilidad}>Abrir Accesibilidad</button>
        </div>
      {/if}

      <div class="grupos" class:apagado={esMac && !r.ajustes.traducir}>
        {#each grupos as g (g.nombre)}
          <div class="grupo">
            <h3 class="rotulo">{g.nombre}</h3>
            <ul class="lista-atajos">
              {#each g.atajos as a (a.windows.join('+') + a.accion)}
                <li class="atajo">
                  {@render teclas(a.windows, 'windows')}
                  <span class="flecha" aria-hidden="true"><Icono nombre="flecha" tamano={13} /></span>
                  {@render teclas(a.mac, 'mac')}
                  <span class="accion">{a.accion}</span>
                </li>
              {/each}
            </ul>
          </div>
        {/each}
      </div>
      {#if totalAtajos > 9}
        <button class="mas-atajos" type="button" aria-expanded={todos} onclick={() => (todos = !todos)}>
          {todos ? 'Ver menos' : `Ver los ${totalAtajos} atajos`}
          <span class="giro" class:abierto={todos}><Icono nombre="chevron-abajo" tamano={15} /></span>
        </button>
      {/if}
    </section>

    <!-- Los demás paneles en dos columnas parejas: captura y la sesión a la izquierda, el portapapeles a la derecha -->
    <div class="paneles">
      <div class="columna">
        <!-- Captura -->
        <section class="panel vidrio" aria-labelledby="remoto-captura">
          <div class="cabeza">
            {@render ficha('captura', !esMac || (r.ajustes.captura && r.permisoPantalla && r.permisoTeclas))}
            <div class="textos">
              <h2 class="titulo-panel" id="remoto-captura">Captura con Imprimir Pantalla</h2>
              <p class="ayuda">
                {esMac
                  ? 'Selecciona una zona; se copia liviana para pegar en Windows y se guarda en Imágenes › Capturas remotas.'
                  : 'Con tu Mac en RustDesk, pulsa Imprimir Pantalla: eliges una zona y llega liviana a tu portapapeles.'}
              </p>
            </div>
            {#if esMac}
              <Interruptor
                encendido={r.ajustes.captura}
                cargando={ocupado === 'captura'}
                etiqueta="Captura con Imprimir Pantalla"
                oncambio={() => ajustar('captura', !r.ajustes.captura)}
              />
            {/if}
          </div>

          <div class="guia" aria-label="Imprimir Pantalla en el PC llega a la Mac como F13">
            <span class="guia__paso">
              <kbd aria-hidden="true">Impr Pant</kbd>
              <span>en tu PC</span>
            </span>
            <span class="flecha" aria-hidden="true"><Icono nombre="flecha" tamano={14} /></span>
            <span class="guia__paso">
              <kbd class="mac" aria-hidden="true">F13</kbd>
              <span>en tu Mac</span>
            </span>
            <span class="flecha" aria-hidden="true"><Icono nombre="flecha" tamano={14} /></span>
            <span class="guia__paso">
              <span class="guia__icono"><Icono nombre="portapapeles" tamano={16} /></span>
              <span>lista para pegar</span>
            </span>
          </div>

          {#if esMac}
            {#if !r.permisoPantalla}
              <div class="permiso">
                <Icono nombre="alerta" tamano={16} />
                <p>
                  Falta el permiso de <b>Grabación de pantalla</b>. Actívalo para Nexo; macOS te pedirá volver a abrirlo.
                </p>
                <button class="boton-vidrio boton" type="button" onclick={pedirPantalla}>Abrir Ajustes</button>
              </div>
            {/if}
            {#if r.capturaRemotaAbierta}
              <div class="permiso info">
                <Icono nombre="info" tamano={16} />
                <p>
                  La app <b>Captura remota</b> sigue abierta. Ciérrala desde su ícono de la barra de menús: Nexo ya hace lo
                  mismo y así Imprimir Pantalla no captura dos veces.
                </p>
              </div>
            {/if}

            <div class="botones">
              <button
                class="boton-vidrio boton"
                type="button"
                disabled={capturando || !r.permisoPantalla}
                aria-busy={capturando}
                onclick={probarCaptura}
              >
                <Icono nombre="captura" tamano={16} />
                {capturando ? 'Selecciona una zona…' : 'Probar captura'}
              </button>
              <button class="boton-vidrio boton" type="button" onclick={abrirCarpeta}>
                <Icono nombre="carpeta" tamano={16} />
                Abrir carpeta
              </button>
            </div>
            {#if ultimaCaptura}
              <p class="resultado" role="status">
                <Icono nombre="check-circulo" tamano={16} />
                {ultimaCaptura.copiada ? 'Copiada' : 'Guardada'} · {ultimaCaptura.ancho} × {ultimaCaptura.alto} px ·
                {miles(ultimaCaptura.kb)} KB
              </p>
            {:else if capturaCancelada}
              <p class="resultado neutro" role="status">Cancelaste la captura. Pulsa Esc cuando quieras salir sin capturar.</p>
            {/if}
          {/if}
        </section>
        {#if esMac}
          <!-- Comodidad durante la sesión -->
          <section class="panel vidrio" aria-labelledby="remoto-sesion">
            <h2 class="rotulo" id="remoto-sesion">Durante la sesión</h2>
            <div class="fila">
              {@render ficha('luna', r.ajustes.evitarReposo)}
              <div class="textos">
                <div class="titulo">Evitar que la Mac se duerma</div>
                <div class="ayuda">Mientras estés conectado, con el cargador puesto. Al terminar, todo vuelve a lo normal.</div>
              </div>
              <Interruptor
                encendido={r.ajustes.evitarReposo}
                cargando={ocupado === 'evitarReposo'}
                etiqueta="Evitar que la Mac se duerma"
                oncambio={() => ajustar('evitarReposo', !r.ajustes.evitarReposo)}
              />
            </div>
            <div class="fila">
              {@render ficha('sol-bajo', r.ajustes.brilloMinimo)}
              <div class="textos">
                <div class="titulo">Bajar el brillo al mínimo</div>
                <div class="ayuda">La pantalla de la MacBook baja a 0 y vuelve a como estaba al terminar.</div>
              </div>
              <Interruptor
                encendido={r.ajustes.brilloMinimo}
                cargando={ocupado === 'brilloMinimo'}
                etiqueta="Bajar el brillo al mínimo"
                oncambio={() => ajustar('brilloMinimo', !r.ajustes.brilloMinimo)}
              />
            </div>
            {#if r.modoTrabajo}
              <div class="fila">
                {@render ficha('encendido', true)}
                <div class="textos">
                  <div class="titulo">Modo trabajo</div>
                  <div class="ayuda">
                    {r.ajustes.evitarReposo
                      ? 'La Mac no se dormirá aunque todavía no te hayas conectado.'
                      : 'Activo, pero «Evitar que la Mac se duerma» está apagado.'}
                  </div>
                </div>
                <button
                  class="boton-vidrio boton"
                  type="button"
                  disabled={ocupado === 'modoTrabajo'}
                  onclick={() => ajustar('modoTrabajo', false)}
                >
                  Terminar
                </button>
              </div>
            {/if}
          </section>
        {/if}
      </div>
      {#if esMac}
        <div class="columna">
          <!-- Portapapeles -->
          <section class="panel vidrio" aria-labelledby="remoto-portapapeles">
            <div class="cabeza">
              {@render ficha('portapapeles', r.ajustes.portapapelesLiviano)}
              <div class="textos">
                <h2 class="titulo-panel" id="remoto-portapapeles">Portapapeles</h2>
                <p class="ayuda">Tus últimas 12 copias, para volver a copiarlas. Las contraseñas no se guardan.</p>
              </div>
            </div>
            <div class="fila">
              <div class="textos">
                <div class="titulo">Reducir imágenes al copiar (evita que RustDesk se congele)</div>
                <div class="ayuda">Durante la sesión, las imágenes grandes pasan a 1600 px antes de llegar a Windows.</div>
              </div>
              <Interruptor
                encendido={r.ajustes.portapapelesLiviano}
                cargando={ocupado === 'portapapelesLiviano'}
                etiqueta="Reducir imágenes al copiar"
                oncambio={() => ajustar('portapapelesLiviano', !r.ajustes.portapapelesLiviano)}
              />
            </div>

            <div class="historial-cabeza">
              <h3 class="rotulo">Historial</h3>
              {#if historial?.length}
                <button class="boton-texto peligro" type="button" onclick={() => (preguntandoBorrar = true)}>
                  Borrar historial
                </button>
              {/if}
            </div>

            {#if historial === null}
              <ul class="copias" aria-busy="true">
                {#each [0, 1, 2] as i (i)}<li class="copia silueta"><span class="miniatura"></span><span class="linea"></span></li>{/each}
              </ul>
            {:else if historial.length === 0}
              <p class="vacio">Todavía no hay copias. Lo que copies en la Mac, o te llegue desde el PC, aparece aquí.</p>
            {:else}
              <ul class="copias">
                {#each historial as e (e.id)}
                  {@const v = vista(e)}
                  <li class="copia">
                    {#if e.tipo === 'imagen' && e.miniatura}
                      <img
                        class="miniatura"
                        src={e.miniatura}
                        alt="Imagen copiada de {e.ancho} × {e.alto} píxeles"
                        width="64"
                        height="44"
                        draggable="false"
                      />
                    {:else}
                      <span class="miniatura texto" aria-hidden="true"><Icono nombre="portapapeles" tamano={18} /></span>
                    {/if}
                    <div class="copia__cuerpo">
                      {#if e.tipo === 'imagen'}
                        <p class="copia__texto">Imagen · {e.ancho} × {e.alto} px</p>
                      {:else}
                        <p class="copia__texto" use:tooltip={tooltipDe(e, v.faltan)}>
                          {v.texto}{#if v.faltan > 0}<span class="faltan"> y {miles(v.faltan)} caracteres más</span>{/if}
                        </p>
                      {/if}
                      <p class="copia__meta">
                        <Icono nombre={e.origen === 'remoto' ? 'pantallas' : 'portatil'} tamano={13} />
                        {e.origen === 'remoto' ? 'Desde el PC' : 'En la Mac'} · {cuando(e.momento)}
                      </p>
                    </div>
                    <button
                      class="boton-vidrio boton copiar"
                      class:hecho={copiadoId === e.id}
                      type="button"
                      disabled={copiandoId !== null}
                      aria-label={e.tipo === 'imagen' ? 'Copiar esta imagen' : 'Copiar este texto'}
                      onclick={() => copiar(e)}
                    >
                      <Icono nombre={copiadoId === e.id ? 'check' : 'copiar'} tamano={15} />
                      {copiadoId === e.id ? 'Copiado' : 'Copiar'}
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          </section>
        </div>
      {/if}
    </div>
  {:else}
    <!-- Mientras responde Rust: siluetas quietas del estado y de los paneles. -->
    <div class="silueta-estado vidrio" aria-busy="true"><span class="solo-lector">Revisando el remoto…</span></div>
    <div class="silueta-panel vidrio"></div>
  {/if}
</div>

<Dialogo
  abierto={preguntandoBorrar}
  titulo="¿Borrar el historial del portapapeles?"
  descripcion="Se borran las copias que guarda Nexo. Lo que tienes copiado ahora no cambia."
  icono="papelera"
  tono="peligro"
  textoConfirmar="Borrar"
  textoCancelar="Cancelar"
  onconfirmar={borrarHistorial}
  oncancelar={() => (preguntandoBorrar = false)}
/>

<style>
  .vista {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
    container-type: inline-size;
  }
  .cabecera h1 {
    margin: 0;
    font-size: 26px;
    line-height: 32px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .cabecera p {
    margin: 4px 0 0;
    color: var(--texto-2);
  }

  /* ---------- Estado ---------- */
  /* La ilustración a la izquierda, en un escenario apenas teñido; el estado y las acciones a la derecha. */
  .estado {
    display: grid;
    grid-template-columns: minmax(240px, 320px) minmax(0, 1fr);
    align-items: center;
    gap: 16px 24px;
    padding: 12px 22px 12px 12px;
    border-radius: var(--radio-lg);
  }
  .estado__escena {
    position: relative;
    height: 196px;
    padding: 8px 6px 0;
    border-radius: 16px;
    background: color-mix(in srgb, var(--primario) 6%, var(--superficie-inserto));
    overflow: hidden;
  }
  .estado__escena.viva {
    background: color-mix(in srgb, var(--exito) 8%, var(--superficie-inserto));
  }
  .estado__escena.apagada {
    background: var(--superficie-inserto);
  }
  /* Por dónde va la sesión, al pie del dibujo: verde si es directa, ámbar por el relé. */
  .estado__via {
    position: absolute;
    left: 50%;
    bottom: 10px;
    transform: translateX(-50%);
    padding: 1px 10px;
    border-radius: var(--radio-pildora);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--exito) 45%, transparent);
    color: var(--exito);
    font-size: 11px;
    line-height: 18px;
    font-weight: 600;
    white-space: nowrap;
  }
  .rele .estado__via {
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--aviso) 45%, transparent);
    color: var(--aviso);
  }

  .estado__cuerpo {
    display: grid;
    gap: 14px;
    min-width: 0;
    padding: 8px 0;
  }
  .estado__titulo {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0;
    font-size: 20px;
    line-height: 26px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .estado__detalle {
    margin: 4px 0 0 18px;
    color: var(--texto-2);
  }
  .punto {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--texto-3);
  }
  .punto.ok {
    background: var(--exito);
  }
  .punto.aviso {
    background: var(--aviso);
  }

  .estado__acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  /* Los servicios como fichas chicas y parejas. */
  .servicios {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .servicio {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 3px 11px 3px 10px;
    border-radius: var(--radio-pildora);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    font-size: 12.5px;
    line-height: 18px;
  }
  .servicio .punto {
    width: 6px;
    height: 6px;
  }
  .servicio__nombre {
    color: var(--texto-3);
  }
  .servicio__valor {
    font-weight: 600;
    color: var(--texto-2);
  }
  .servicio.ok .servicio__valor {
    color: var(--exito);
  }
  .servicio.aviso .servicio__valor {
    color: var(--aviso);
  }
  .direccion {
    color: var(--texto);
    font-variant-numeric: tabular-nums;
    user-select: text;
    -webkit-user-select: text;
    cursor: text;
  }

  .aviso-accion {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radio-sm);
    font-size: 12.5px;
    line-height: 18px;
    background: var(--superficie-elevada);
    color: var(--texto-2);
  }
  .aviso-accion :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
  }
  .aviso-accion.ok :global(svg) {
    color: var(--exito);
  }
  .aviso-accion.aviso :global(svg) {
    color: var(--aviso);
  }

  /* Angosto: la ilustración arriba, a lo ancho, y el estado debajo. */
  @container (max-width: 640px) {
    .estado {
      grid-template-columns: minmax(0, 1fr);
      padding: 12px 12px 14px;
    }
    .estado__escena {
      height: 172px;
    }
    .estado__cuerpo {
      padding: 0 8px;
    }
  }

  /* ---------- Paneles de abajo ---------- */
  .paneles,
  .columna {
    display: grid;
    gap: 16px;
    align-content: start;
    min-width: 0;
  }
  /* Con espacio, dos columnas parejas (si hay portapapeles: en Windows queda solo la captura). */
  @container (min-width: 900px) {
    .paneles:has(> .columna + .columna) {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }
  }

  /* ---------- Paneles ---------- */
  .panel {
    padding: 16px 20px 8px;
    border-radius: var(--radio-lg);
    min-width: 0;
  }
  .rotulo {
    margin: 0 0 4px;
  }
  .cabeza {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding-bottom: 12px;
  }
  .cabeza :global(.interruptor) {
    margin-top: 2px;
  }
  .titulo-panel {
    margin: 0;
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
  }
  .textos {
    flex: 1;
    min-width: 0;
  }
  .titulo {
    font-weight: 600;
  }
  .ayuda {
    margin: 2px 0 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
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
    color: var(--primario-texto);
    background: var(--elegido-bg);
  }

  .fila {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 60px;
    padding: 12px 0;
    border-top: 1px solid var(--borde);
  }
  .rotulo + .fila {
    border-top: 0;
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
  .boton:disabled,
  .boton-primario:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .boton-texto {
    padding: 4px 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--texto-2);
    font-size: 12.5px;
    font-weight: 600;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .boton-texto:hover {
    background: var(--superficie-hover);
  }
  .boton-texto.peligro {
    color: var(--error);
  }

  .permiso {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0 0 12px;
    padding: 10px 12px;
    border-radius: var(--radio-sm);
    background: color-mix(in srgb, var(--aviso) 9%, var(--superficie));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--aviso) 25%, transparent);
  }
  .permiso > :global(svg) {
    flex-shrink: 0;
    color: var(--aviso);
  }
  .permiso.info {
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .permiso.info > :global(svg) {
    color: var(--texto-3);
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

  /* ---------- Teclas ---------- */
  .teclas {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
  kbd {
    display: inline-grid;
    place-items: center;
    min-width: 26px;
    height: 26px;
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
  kbd.simbolo {
    font-size: 13.5px;
    font-weight: 500;
  }
  .mac kbd,
  kbd.mac {
    color: var(--primario-texto);
    background: var(--elegido-bg);
    box-shadow:
      inset 0 0 0 1px var(--elegido-borde),
      inset 0 -1.5px 0 var(--elegido-borde);
  }
  .flecha {
    display: grid;
    place-items: center;
    color: var(--texto-3);
  }

  /* Una tabla pareja para todos los grupos: cada fila es [teclas de Windows] → [teclas de la Mac] y la acción
     justo después. Las columnas miden lo que pide la combinación más larga (subgrid en cada nivel), sin huecos
     fijos, y quedan alineadas de un grupo a otro. */
  .grupos {
    display: grid;
    grid-template-columns: max-content 14px max-content minmax(0, 1fr);
    column-gap: 10px;
    row-gap: 14px;
    padding: 2px 0 8px;
  }
  .grupo,
  .lista-atajos {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
  }
  /* Con la traducción apagada, la tabla queda de referencia: el lado de la Mac pierde el color. */
  .grupos.apagado .mac kbd {
    color: var(--texto-2);
    background: var(--superficie-elevada);
    box-shadow:
      inset 0 0 0 1px var(--borde),
      inset 0 -1.5px 0 var(--borde);
  }
  .grupo .rotulo {
    grid-column: 1 / -1;
    margin: 0 0 2px;
  }
  .lista-atajos {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .atajo {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: center;
    padding: 6px 0;
    border-top: 1px solid var(--borde);
  }
  .atajo:first-child {
    border-top: 0;
  }
  .accion {
    font-size: 13px;
    line-height: 18px;
    color: var(--texto);
    overflow-wrap: break-word;
  }
  /* Ancho: dos columnas de filas por grupo, con un respiro entre las dos mitades. */
  @container (min-width: 900px) {
    .grupos {
      grid-template-columns:
        max-content 14px max-content minmax(0, 1fr)
        28px
        max-content 14px max-content minmax(0, 1fr);
    }
    .atajo:nth-child(odd) {
      grid-column: 1 / 5;
    }
    .atajo:nth-child(even) {
      grid-column: 6 / 10;
    }
    .atajo:nth-child(2) {
      border-top: 0;
    }
  }
  /* Muy angosto: la acción baja debajo de las teclas. */
  @container (max-width: 520px) {
    .grupos {
      grid-template-columns: max-content 14px max-content;
    }
    .accion {
      grid-column: 1 / -1;
    }
  }

  .mas-atajos {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 10px -8px;
    padding: 6px 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--primario-texto);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .mas-atajos:hover {
    background: var(--superficie-hover);
  }
  .giro {
    display: grid;
    transition: transform var(--dur-vista) ease;
  }
  .giro.abierto {
    transform: rotate(180deg);
  }

  /* ---------- Captura ---------- */
  .guia {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 12px;
    margin: 0 0 14px;
    padding: 12px 14px;
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
  }
  .guia__paso {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .guia__icono {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde-fuerte);
    color: var(--texto-2);
  }
  .botones {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding-bottom: 12px;
  }
  .resultado {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 12px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .resultado :global(svg) {
    flex-shrink: 0;
    color: var(--exito);
  }

  /* ---------- Portapapeles ---------- */
  .historial-cabeza {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 0 4px;
    border-top: 1px solid var(--borde);
  }
  .historial-cabeza .rotulo {
    margin: 0;
  }
  .copias {
    margin: 0;
    padding: 0 0 6px;
    list-style: none;
  }
  .copia {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
  }
  .copia + .copia {
    border-top: 1px solid var(--borde);
  }
  .miniatura {
    flex-shrink: 0;
    width: 64px;
    height: 44px;
    border-radius: 8px;
    object-fit: cover;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  img.miniatura {
    border: 1px solid var(--borde);
  }
  .miniatura.texto {
    display: grid;
    place-items: center;
    color: var(--texto-3);
  }
  .copia__cuerpo {
    flex: 1;
    min-width: 0;
  }
  .copia__texto {
    margin: 0;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .faltan {
    color: var(--texto-3);
    white-space: normal;
  }
  .copia__meta {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 5px;
    margin: 3px 0 0;
    font-size: 12px;
    line-height: 16px;
    color: var(--texto-3);
  }
  .copiar {
    gap: 6px;
    padding: 0 12px;
  }
  .copiar.hecho {
    color: var(--exito);
  }
  .vacio {
    margin: 0;
    padding: 6px 0 14px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }

  /* ---------- Siluetas mientras carga (quietas) ---------- */
  .silueta-estado {
    height: 220px;
    border-radius: var(--radio-lg);
  }
  .silueta-panel {
    height: 320px;
    border-radius: var(--radio-lg);
  }
  .copia.silueta .linea {
    flex: 1;
    max-width: 60%;
    height: 12px;
    border-radius: 6px;
    background: var(--superficie-elevada);
  }
</style>
