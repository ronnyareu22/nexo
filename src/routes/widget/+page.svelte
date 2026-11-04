<!--
  Widget de escritorio (lo abre Rust: src-tauri/src/sistema/widget.rs). Una tarjeta sólida con cuatro casillas:
  audífonos, pantallas, Bluetooth y Nexo Bot. Cada casilla abre Nexo en su sección. La tarjeta se arrastra desde
  sus bordes y, en una casilla, al mover el mouse con el botón apretado. El ⋯ abre "Abrir Nexo" y "Ocultar widget".
  Datos: los mismos comandos del panel, cada 30 s mientras se ve, al volver a verlo y cuando el vigía avisa un cambio.
  Ningún texto se corta: la ventana mide 568 × 150 y todo baja de línea si no cabe.
  Nexo Drop: soltar archivos sobre la tarjeta los manda a tu otro equipo (el avance se ve en Nexo › Drop).
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { fade } from 'svelte/transition';
  import { listarDispositivos, listarPantallas, ponerBrillo } from '$lib/api';
  import {
    abrirNexo,
    abrirSeccion,
    alArrastrarArchivos,
    alCambiarDispositivos,
    alCambiarTema,
    enviarDrop,
    moverVentana,
    usarWidget,
  } from '$lib/api-sistema';
  import { destinoActual, drop, leerDestinos } from '$lib/drop.svelte';
  import type { Dispositivo, Estado } from '$lib/tipos';
  import { esDeSonido, nivelPrincipal, nombreDe } from '$lib/tipos';
  import type { PantallaSistema } from '$lib/tipos-sistema';
  import { leerTema, type Tema } from '$lib/tema';
  import Icono, { type Nombre } from '$lib/componentes/Icono.svelte';
  import NexoBot, { type ExpresionNexoBot } from '$lib/componentes/NexoBot.svelte';

  /** Lectura normal mientras el widget se ve. */
  const CADA = 30_000;
  /** Pausa mínima entre lecturas que piden el foco, la visibilidad o el vigía. */
  const RESPIRO = 5_000;
  /** Cuánto hay que mover el mouse en una casilla para que sea arrastrar y no tocar. */
  const UMBRAL = 4;

  let estado = $state<Estado | null>(null);
  let pantallas = $state<PantallaSistema[] | null>(null);
  let leido = $state(false);

  // --- Lo que muestra cada casilla ---

  type Tono = 'exito' | 'aviso' | 'error' | 'primario' | 'tenue';
  // Los mismos cortes que el anillo de batería y que Inicio.
  const tonoBateria = (n: number): Tono => (n <= 15 ? 'error' : n < 40 ? 'aviso' : 'exito');
  const plural = (n: number, uno: string, varios: string) => (n === 1 ? uno : varios);

  interface Casilla {
    id: string;
    /** Para lectores de pantalla: qué es la casilla. */
    nombre: string;
    icono: Nombre;
    valor: string;
    /** Número o palabra corta en grande; un texto más largo va más chico. */
    grande: boolean;
    tono: Tono | null;
    detalle: string;
    extra?: string;
    /** Punto verde junto al detalle (aparato conectado). */
    punto?: boolean;
    abrir: () => void;
  }

  const dispositivos = $derived(estado?.dispositivos ?? []);
  const deSonido = $derived(dispositivos.filter(esDeSonido));
  const audifonos = $derived(deSonido.find((d) => d.conectado) ?? null);

  const sinDatos = (c: Omit<Casilla, 'valor' | 'grande' | 'tono' | 'detalle'>): Casilla => ({
    ...c,
    valor: 'Sin datos',
    grande: false,
    tono: 'tenue',
    detalle: 'Toca para abrir Nexo',
  });

  const casillaAudifonos = $derived.by((): Casilla => {
    const base = {
      id: 'audifonos',
      nombre: 'Audífonos',
      icono: 'audio' as const,
      abrir: () => ir('dispositivos', (audifonos ?? deSonido[0])?.id),
    };
    if (!estado) return sinDatos(base);
    const nivel = audifonos ? nivelPrincipal(audifonos) : null;
    if (audifonos && nivel !== null)
      return { ...base, valor: `${nivel} %`, grande: true, tono: tonoBateria(nivel), detalle: nombreDe(audifonos), punto: true };
    if (audifonos)
      return { ...base, valor: 'Conectados', grande: false, tono: 'exito', detalle: nombreDe(audifonos), punto: true };
    return { ...base, valor: 'Sin audífonos', grande: false, tono: 'tenue', detalle: 'Conecta tus audífonos' };
  });

  const casillaBluetooth = $derived.by((): Casilla => {
    const base = { id: 'bluetooth', nombre: 'Bluetooth', icono: 'bluetooth' as const, abrir: () => ir('dispositivos') };
    if (!estado) return sinDatos(base);
    const n = dispositivos.length;
    return estado.bluetoothEncendido
      ? { ...base, valor: 'Activo', grande: true, tono: 'exito', detalle: `${n} ${plural(n, 'dispositivo', 'dispositivos')}` }
      : { ...base, valor: 'Apagado', grande: true, tono: 'error', detalle: 'Enciéndelo para conectar' };
  });

  // Brillo: la pantalla principal (o la primera que deje cambiarlo), con − y + para bajarlo o subirlo de a 10.
  const pantallaBrillo = $derived(
    pantallas?.find((p) => p.principal && p.brillo !== null) ??
      pantallas?.find((p) => p.interna && p.brillo !== null) ??
      pantallas?.find((p) => p.brillo !== null) ??
      null,
  );
  let cambiandoBrillo = $state(false);
  async function pasoBrillo(subir: boolean) {
    const p = pantallaBrillo;
    if (!p || p.brillo === null || cambiandoBrillo) return;
    // Al múltiplo de 10 anterior o siguiente: 31 → 30 → 20…; 31 → 40.
    const nuevo = Math.min(100, Math.max(0, subir ? (Math.floor(p.brillo / 10) + 1) * 10 : Math.floor((p.brillo - 1) / 10) * 10));
    cambiandoBrillo = true;
    p.brillo = nuevo;
    try {
      p.brillo = await ponerBrillo(p.id, nuevo);
    } catch {
      // Si falla, la próxima lectura trae el valor real.
    }
    cambiandoBrillo = false;
  }

  const casillas = $derived([casillaAudifonos, casillaBluetooth]);

  // Nexo Bot: contento si todo va bien; un aviso tranquilo si una batería está en 20 % o menos.
  interface Bot {
    expresion: ExpresionNexoBot;
    etiqueta?: string;
    titulo: string;
    detalle: string;
  }
  const bot = $derived.by((): Bot => {
    if (!leido) return { expresion: 'cargando', etiqueta: 'Nexo Bot cargando', titulo: 'Un momento', detalle: 'Revisando tus dispositivos.' };
    if (!estado) return { expresion: 'error', titulo: 'Sin datos', detalle: 'Toca para abrir Nexo.' };
    if (!estado.bluetoothEncendido)
      return { expresion: 'bluetooth-apagado', titulo: 'Bluetooth apagado', detalle: 'Enciéndelo para usar tus aparatos.' };
    let baja: { d: Dispositivo; nivel: number } | null = null;
    for (const d of dispositivos) {
      const nivel = d.conectado ? nivelPrincipal(d) : null;
      if (nivel !== null && nivel <= 20 && (!baja || nivel < baja.nivel)) baja = { d, nivel };
    }
    if (baja)
      return {
        expresion: 'error',
        etiqueta: 'Nexo Bot preocupado: batería baja',
        titulo: 'Batería baja',
        detalle: `${nombreDe(baja.d)} al ${baja.nivel} %.`,
      };
    if (!dispositivos.length)
      return { expresion: 'sin-dispositivos', titulo: 'Sin dispositivos', detalle: 'Empareja uno en Bluetooth.' };
    return { expresion: 'feliz', titulo: '¡Todo listo!', detalle: 'Tus dispositivos funcionan perfecto.' };
  });

  // --- Datos ---

  let leyendo = false;
  let ultimaLectura = 0;

  async function actualizar(forzar = false) {
    if (leyendo || (!forzar && Date.now() - ultimaLectura < RESPIRO)) return;
    leyendo = true;
    const [e, p] = await Promise.allSettled([listarDispositivos(), listarPantallas()]);
    // Si una lectura falla, se queda lo último que se supo; la próxima vuelta lo corrige.
    if (e.status === 'fulfilled') estado = e.value;
    if (p.status === 'fulfilled') pantallas = p.value as PantallaSistema[];
    leido = true;
    ultimaLectura = Date.now();
    leyendo = false;
  }

  // --- Tema: el mismo del panel ---

  function ponerTema(tema: Tema) {
    const html = document.documentElement;
    if (tema === 'sistema') delete html.dataset.theme;
    else html.dataset.theme = tema === 'claro' ? 'light' : 'dark';
  }

  // --- Abrir Nexo desde una casilla, o arrastrar el widget desde ella ---

  let inicio: { x: number; y: number } | null = null;
  let arrastro = false;

  function alPresionar(e: PointerEvent) {
    arrastro = false;
    inicio = e.button === 0 && (e.target as Element).closest('.casilla') ? { x: e.screenX, y: e.screenY } : null;
  }

  function alMover(e: PointerEvent) {
    if (!inicio || !(e.buttons & 1)) return;
    if (Math.hypot(e.screenX - inicio.x, e.screenY - inicio.y) < UMBRAL) return;
    inicio = null;
    arrastro = true;
    moverVentana();
  }

  /** Abre la sección en Nexo, salvo que la casilla se haya usado para arrastrar. */
  function ir(seccion: 'inicio' | 'dispositivos' | 'pantallas', dispositivo?: string) {
    if (arrastro) {
      arrastro = false;
      return;
    }
    abrirSeccion(seccion, dispositivo).catch(() => {});
  }

  // --- Menú ⋯ ---

  let menu = $state(false);
  let botonMenu = $state<HTMLButtonElement>();
  let listaMenu = $state<HTMLDivElement>();

  async function abrirMenu() {
    menu = true;
    await tick();
    listaMenu?.querySelector<HTMLButtonElement>('button')?.focus();
  }

  function cerrarMenu(volverAlBoton = false) {
    if (!menu) return;
    menu = false;
    if (volverAlBoton) botonMenu?.focus();
  }

  function elegir(accion: () => Promise<unknown>) {
    cerrarMenu();
    accion().catch(() => {});
  }

  function teclaMenu(e: KeyboardEvent) {
    const opciones = [...(listaMenu?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
    const i = opciones.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'Escape') {
      e.preventDefault();
      cerrarMenu(true);
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const paso = e.key === 'ArrowDown' ? 1 : -1;
      opciones[(i + paso + opciones.length) % opciones.length]?.focus();
    } else if (e.key === 'Tab') {
      cerrarMenu();
    }
  }

  // --- Nexo Drop: soltar archivos sobre el widget ---
  let soltado = $state<{ texto: string; tono: 'ok' | 'aviso' } | null>(null);
  let temporizadorSoltado: ReturnType<typeof setTimeout> | undefined;
  const destinoDrop = $derived(destinoActual());

  function avisoSoltado(texto: string, tono: 'ok' | 'aviso') {
    soltado = { texto, tono };
    clearTimeout(temporizadorSoltado);
    temporizadorSoltado = setTimeout(() => (soltado = null), 4500);
  }

  async function soltarArchivos(rutas: string[]) {
    if (!rutas.length) return;
    await leerDestinos();
    const d = destinoActual();
    if (!d) {
      avisoSoltado('No encontré otro equipo tuyo con Nexo.', 'aviso');
      return;
    }
    try {
      await enviarDrop(rutas, d.ip);
      avisoSoltado(`Enviando a ${d.nombre}. Míralo en Nexo › XDrop.`, 'ok');
    } catch (e) {
      avisoSoltado(String(e), 'aviso');
    }
  }

  function verDrop() {
    soltado = null;
    abrirSeccion('drop');
  }

  // Un clic fuera del menú lo cierra.
  function alPresionarVentana(e: PointerEvent) {
    if (!menu) return;
    const t = e.target as Node;
    if (!listaMenu?.contains(t) && !botonMenu?.contains(t)) cerrarMenu();
  }

  onMount(() => {
    ponerTema(leerTema());
    actualizar(true);

    const intervalo = setInterval(() => {
      if (document.visibilityState === 'visible') actualizar(true);
    }, CADA);
    const alVolver = () => {
      if (document.visibilityState !== 'visible') return;
      ponerTema(leerTema());
      actualizar();
    };
    const alEnfocar = () => {
      ponerTema(leerTema());
      actualizar();
    };
    // El tema también llega guardado desde el panel (misma carpeta de datos).
    const alGuardar = (e: StorageEvent) => {
      if (e.key === 'nexo-tema') ponerTema(leerTema());
    };
    document.addEventListener('visibilitychange', alVolver);
    window.addEventListener('focus', alEnfocar);
    window.addEventListener('storage', alGuardar);
    const quitarVigia = alCambiarDispositivos(() => actualizar());
    const quitarTema = alCambiarTema(ponerTema);
    const quitarArrastre = alArrastrarArchivos((a) => {
      if (a.tipo === 'encima') {
        drop.arrastrando = true;
        drop.cantidad = a.rutas.length;
        leerDestinos();
      } else if (a.tipo === 'fuera') {
        drop.arrastrando = false;
      } else {
        drop.arrastrando = false;
        soltarArchivos(a.rutas);
      }
    });

    return () => {
      clearInterval(intervalo);
      document.removeEventListener('visibilitychange', alVolver);
      window.removeEventListener('focus', alEnfocar);
      window.removeEventListener('storage', alGuardar);
      quitarVigia();
      quitarTema();
      quitarArrastre();
      clearTimeout(temporizadorSoltado);
    };
  });
</script>

<svelte:head>
  <title>Widget de Nexo</title>
</svelte:head>

<svelte:window onpointerdown={alPresionarVentana} onblur={() => cerrarMenu()} />

{#snippet brillo()}
  <!-- Brillo: el número grande y dos botones claros. No es un botón entero para que − y + funcionen aparte. -->
  <div class="casilla brillo" role="group" aria-label="Brillo">
    <span class="icono sol"><Icono nombre="sol" tamano={22} /></span>
    {#if leido && pantallaBrillo && pantallaBrillo.brillo !== null}
      <span class="valor">{pantallaBrillo.brillo} %</span>
      <span class="pasos">
        <button type="button" class="paso" onclick={() => pasoBrillo(false)} aria-label="Bajar brillo" disabled={pantallaBrillo.brillo <= 0}>
          <Icono nombre="sol-bajo" tamano={15} /> −
        </button>
        <button type="button" class="paso" onclick={() => pasoBrillo(true)} aria-label="Subir brillo" disabled={pantallaBrillo.brillo >= 100}>
          <Icono nombre="sol" tamano={15} /> +
        </button>
      </span>
    {:else if leido}
      <span class="valor chico tenue">Brillo</span>
      <span class="detalle">Esta pantalla no deja cambiarlo</span>
    {:else}
      <span class="hueso"></span>
      <span class="hueso corto"></span>
    {/if}
  </div>
{/snippet}

<div class="lienzo">
  <div class="tarjeta" data-tauri-drag-region="deep" role="group" aria-label="Widget de Nexo">
    <div class="casillas" onpointerdown={alPresionar} onpointermove={alMover} onpointerup={() => (inicio = null)} role="presentation">
      {#each casillas as c, i (c.id)}
        {#if i === 1}{@render brillo()}{/if}
        <button class="casilla" type="button" onclick={c.abrir} aria-busy={!leido}>
          <span class="icono"><Icono nombre={c.icono} tamano={22} /></span>
          <span class="solo-lector">{c.nombre}:</span>
          {#if leido}
            <span class="valor {c.tono ?? ''}" class:chico={!c.grande}>{c.valor}</span>
            <span class="detalle">
              {#if c.punto}<span class="punto" aria-hidden="true"></span>{/if}{c.detalle}
            </span>
            {#if c.extra}<span class="detalle">{c.extra}</span>{/if}
          {:else}
            <span class="hueso"></span>
            <span class="hueso corto"></span>
          {/if}
        </button>
      {/each}

      <button class="casilla bot" type="button" onclick={() => ir('inicio')}>
        <NexoBot expresion={bot.expresion} etiqueta={bot.etiqueta} tamano={50} />
        <span class="titulo">{bot.titulo}</span>
        <span class="detalle">{bot.detalle}</span>
      </button>
    </div>

    <button
      bind:this={botonMenu}
      class="mas"
      type="button"
      aria-label="Opciones del widget"
      aria-haspopup="menu"
      aria-expanded={menu}
      onclick={() => (menu ? cerrarMenu() : abrirMenu())}
    >
      <Icono nombre="opciones" tamano={16} />
    </button>

    {#if menu}
      <div
        bind:this={listaMenu}
        class="menu flotante"
        role="menu"
        tabindex="-1"
        aria-label="Opciones del widget"
        data-tauri-drag-region="false"
        onkeydown={teclaMenu}
        transition:fade={{ duration: 120 }}
      >
        <button type="button" role="menuitem" onclick={() => elegir(abrirNexo)}>Abrir Nexo</button>
        <button type="button" role="menuitem" onclick={() => elegir(() => usarWidget(false))}>Ocultar widget</button>
      </div>
    {/if}

    {#if drop.arrastrando || soltado}
      <!-- Nexo Drop: tapa la tarjeta mientras se arrastra (y un momento después de soltar). Sólida y quieta. -->
      <button class="capa-drop" class:aviso={!drop.arrastrando && soltado?.tono === 'aviso'} type="button" onclick={verDrop}>
        <span class="capa-drop__icono" aria-hidden="true"><Icono nombre="drop" tamano={22} /></span>
        <span class="capa-drop__texto" role="status">
          {#if drop.arrastrando}
            {#if !drop.leidos}
              Buscando tu otro equipo…
            {:else if destinoDrop}
              Suelta para enviar a {destinoDrop.nombre}
            {:else}
              No hay a quién enviar: abre Nexo en tu otro equipo
            {/if}
          {:else}
            {soltado?.texto ?? ''}
          {/if}
        </span>
      </button>
    {/if}
  </div>
</div>

<style>
  /* La ventana es transparente: solo se ve la tarjeta, con sus esquinas redondas. */
  :global(html),
  :global(body) {
    background: transparent !important;
  }

  /* 6 px transparentes alrededor para la sombra corta de la tarjeta. */
  .lienzo {
    height: 100vh;
    padding: 6px;
  }

  .tarjeta {
    position: relative;
    height: 100%;
    padding: 7px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-lg);
    background: var(--superficie);
    /* Sombra corta para que quepa en los 6 px; sobre cualquier fondo de escritorio, el borde marca la orilla. */
    box-shadow:
      var(--sombra-panel),
      0 3px 8px -4px rgb(0 0 0 / 0.3);
  }

  /* Nexo Drop: tapa sólida con el anillo turquesa y la aurora quieta. */
  .capa-drop {
    position: absolute;
    z-index: 5;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 12px 20px;
    border: 0;
    border-radius: var(--radio-lg);
    background:
      radial-gradient(ellipse 200px 90px at 50% 100%, var(--aurora-1), transparent),
      radial-gradient(ellipse 160px 80px at 25% 100%, var(--aurora-2), transparent),
      var(--superficie);
    box-shadow: inset 0 0 0 2px var(--primario);
    color: var(--texto);
    text-align: left;
  }
  .capa-drop.aviso {
    box-shadow: inset 0 0 0 2px var(--aviso);
  }
  .capa-drop__icono {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 44px;
    height: 44px;
    border-radius: 13px;
    color: var(--primario-texto);
    background: var(--elegido-bg);
  }
  .capa-drop__texto {
    font-size: 14px;
    line-height: 19px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  /* Cuatro casillas; la de Nexo Bot, un poco más ancha para que su frase quepa en dos líneas. */
  .casillas {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr 1.3fr;
    gap: 6px;
    height: 100%;
  }

  .casilla {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-width: 0;
    padding: 8px 6px;
    border: 0;
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
    color: var(--texto);
    text-align: center;
    overflow-wrap: anywhere;
    transition: background-color var(--dur-color) ease;
  }
  .casilla:hover {
    background: var(--superficie-hover);
  }
  .casilla:active:not(.brillo) {
    transform: scale(0.98);
  }
  .casilla.brillo:hover {
    background: var(--superficie-elevada);
  }
  .sol {
    color: var(--amarillo);
  }
  .pasos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    width: 100%;
    margin-top: 6px;
  }
  .paso {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 3px;
    min-height: 28px;
    padding: 0 6px;
    border: 0;
    border-radius: 9px;
    background: var(--superficie-hover);
    color: var(--texto);
    font-size: 15px;
    font-weight: 700;
    transition: background-color var(--dur-color) ease;
  }
  .paso:hover:not(:disabled) {
    background: color-mix(in srgb, var(--amarillo) 22%, var(--superficie-hover));
  }
  .paso:active:not(:disabled) {
    transform: scale(0.96);
  }
  .paso:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .icono {
    display: flex;
    margin-bottom: 5px;
    color: var(--celeste);
  }

  .valor {
    font-size: 21px;
    line-height: 26px;
    font-weight: 700;
    letter-spacing: -0.01em;
    font-variant-numeric: tabular-nums;
  }
  .valor.chico {
    font-size: 15px;
    line-height: 20px;
    font-weight: 650;
    letter-spacing: 0;
  }
  .exito {
    color: var(--exito);
  }
  .aviso {
    color: var(--aviso);
  }
  .error {
    color: var(--error);
  }
  .primario {
    color: var(--primario-texto);
  }
  .tenue {
    color: var(--texto-2);
  }

  .detalle {
    margin-top: 1px;
    font-size: 11.5px;
    line-height: 15px;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .punto {
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-right: 5px;
    vertical-align: 1px;
    border-radius: 50%;
    background: var(--exito);
  }

  .bot .titulo {
    margin-top: 3px;
    font-size: 13px;
    line-height: 17px;
    font-weight: 700;
    color: var(--texto);
  }

  /* Mientras llega la primera lectura: siluetas quietas. */
  .hueso {
    width: 46px;
    height: 18px;
    margin: 4px 0;
    border-radius: 6px;
    background: var(--superficie-hundida);
  }
  .hueso.corto {
    width: 64px;
    height: 10px;
    margin: 2px 0 0;
  }

  /* ⋯ arriba a la derecha, sobre la esquina libre de la casilla de Nexo Bot. */
  .mas {
    position: absolute;
    z-index: 2;
    top: 11px;
    right: 11px;
    display: grid;
    place-items: center;
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
  .mas:hover,
  .mas[aria-expanded='true'] {
    background: var(--superficie-hover);
    color: var(--texto);
  }

  .menu {
    position: absolute;
    z-index: 3;
    top: 40px;
    right: 11px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 168px;
    padding: 5px;
    border-radius: var(--radio-sm);
  }
  .menu:focus {
    outline: none;
  }
  .menu button {
    display: flex;
    align-items: center;
    min-height: 32px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--texto);
    font-size: 13px;
    font-weight: 500;
    text-align: left;
    transition: background-color var(--dur-color) ease;
  }
  .menu button:hover,
  .menu button:focus-visible {
    background: var(--superficie-hover);
    box-shadow: none;
  }
</style>
