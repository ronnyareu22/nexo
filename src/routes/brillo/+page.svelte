<!--
  Widget de brillo de una sola pantalla (lo abre Rust: src-tauri/src/sistema/widget_brillo.rs, una ventana por pantalla).
  Una tarjeta sólida de 248 × 84 dentro de una ventana transparente de 260 × 96: el sol y el nombre de la pantalla,
  el % en grande, la barra y − y +. El tono va de ámbar tenue a amarillo intenso, como en la tarjeta de Pantallas.
  - Se arrastra desde cualquier parte que no sea un control; Rust guarda dónde quedó.
  - Sigue el brillo con el aviso "brillo-cambio" (panel, teclas, menú, escenas) y una lectura de la caché cada 2 s
    mientras se ve. Mientras mueves la barra, y 900 ms después, no sigue lo que llega: así no rebota.
  - Clic derecho: abrir Pantallas en Nexo o quitar el widget.
  - Ningún texto se corta: si el nombre ocupa más líneas, la ventana crece.
  Fuera de la app: /brillo?id=2 muestra el Steren de ejemplo.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { fade } from 'svelte/transition';
  import { leerBrillos, listarPantallas, ponerBrillo } from '$lib/api';
  import { abrirSeccion, alCambiarTema } from '$lib/api-sistema';
  import { alCambiarBrillo, usarWidgetBrillo } from '$lib/api-monitor';
  import { pasoDeBrillo, tonoDeBrillo } from '$lib/brillo';
  import { enviadorDelUltimo } from '$lib/enviador';
  import { leerTema, type Tema } from '$lib/tema';
  import Deslizador from '$lib/componentes/Deslizador.svelte';
  import Icono from '$lib/componentes/Icono.svelte';

  /** Lectura de la caché de brillo mientras se ve (no toca el monitor). */
  const CADA = 2000;
  /** Tras soltar la barra, cuánto se ignora lo que llega. */
  const RESPIRO = 900;
  const ANCHO = 260;
  const ALTO = 96;
  const enApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

  function idDeLaDireccion(): number | null {
    const n = Number(new URLSearchParams(location.search).get('id'));
    return Number.isInteger(n) && n > 0 ? n : null;
  }

  let id = $state<number | null>(idDeLaDireccion());
  let nombre = $state('');
  let brillo = $state<number | null>(null);
  /** La pantalla está conectada y deja cambiar el brillo. */
  let disponible = $state(true);
  let leido = $state(false);

  const tono = $derived(tonoDeBrillo(brillo ?? 0));

  // --- Lo que llega del sistema: se ignora mientras tocas y un momento después; las diferencias de 1 % también. ---
  let tocando = false;
  let ultimoToque = 0;
  function llega(valor: number) {
    disponible = true;
    if (tocando || performance.now() - ultimoToque < RESPIRO) return;
    if (brillo === null || Math.abs(valor - brillo) > 1) brillo = valor;
  }

  async function leerPantalla() {
    if (id === null) {
      leido = true;
      disponible = false;
      return;
    }
    try {
      const p = (await listarPantallas()).find((x) => x.id === id);
      if (p) {
        nombre = p.nombre;
        disponible = p.controlable && p.brillo !== null;
        if (p.brillo !== null) llega(p.brillo);
      } else {
        disponible = false;
      }
    } catch {
      // Si falla, la próxima lectura lo corrige.
    }
    leido = true;
  }

  async function leerNivel() {
    if (id === null) return;
    try {
      const par = (await leerBrillos()).find(([x]) => x === id);
      if (par) llega(par[1]);
    } catch {
      // Igual que arriba.
    }
  }

  // --- Cambiar: siempre el último valor, sin fila de órdenes. ---
  const enviar = enviadorDelUltimo(
    (v: number) => (id === null ? Promise.resolve() : ponerBrillo(id, v)),
    () => {},
  );
  function cambiar(valor: number) {
    ultimoToque = performance.now();
    brillo = Math.max(0, Math.min(100, Math.round(valor)));
    enviar(brillo);
  }
  function alTocar(t: boolean) {
    tocando = t;
    ultimoToque = performance.now();
  }
  function paso(subir: boolean) {
    if (brillo !== null) cambiar(pasoDeBrillo(brillo, subir));
  }

  // --- Tema: el mismo del panel ---
  function ponerTema(tema: Tema) {
    const html = document.documentElement;
    if (tema === 'sistema') delete html.dataset.theme;
    else html.dataset.theme = tema === 'claro' ? 'light' : 'dark';
  }

  // --- Menú del clic derecho ---
  let menu = $state(false);
  let listaMenu = $state<HTMLDivElement>();

  async function abrirMenu(e: MouseEvent) {
    e.preventDefault();
    menu = true;
    await tick();
    listaMenu?.querySelector<HTMLButtonElement>('button')?.focus();
  }

  function elegir(accion: () => Promise<unknown>) {
    menu = false;
    accion().catch(() => {});
  }

  function teclaMenu(e: KeyboardEvent) {
    const opciones = [...(listaMenu?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
    const i = opciones.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'Escape') {
      e.preventDefault();
      menu = false;
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const paso = e.key === 'ArrowDown' ? 1 : -1;
      opciones[(i + paso + opciones.length) % opciones.length]?.focus();
    } else if (e.key === 'Tab') {
      menu = false;
    }
  }

  function alPresionarVentana(e: PointerEvent) {
    if (menu && !listaMenu?.contains(e.target as Node)) menu = false;
  }

  // --- Alto de la ventana: si el contenido no cabe en 96, la ventana crece (nada se corta). ---
  let contenido = $state<HTMLDivElement>();
  let altoPuesto = ALTO;
  async function ajustarAlto() {
    if (!enApp || !contenido) return;
    // 6 px de lienzo arriba y abajo, 10 px de relleno y 1 px de borde de la tarjeta.
    const alto = Math.max(ALTO, Math.ceil(contenido.offsetHeight + 2 * (6 + 10 + 1)));
    if (Math.abs(alto - altoPuesto) < 1) return;
    altoPuesto = alto;
    try {
      const { getCurrentWindow, LogicalSize } = await import('@tauri-apps/api/window');
      await getCurrentWindow().setSize(new LogicalSize(ANCHO, alto));
    } catch {
      // Sin permiso o sin ventana: se queda como está.
    }
  }

  onMount(() => {
    ponerTema(leerTema());
    // Si la dirección no trae la pantalla, sale de la etiqueta de la ventana (brillo-<id>).
    const listo =
      id === null && enApp
        ? import('@tauri-apps/api/window')
            .then(({ getCurrentWindow }) => {
              const n = Number(getCurrentWindow().label.replace(/^brillo-/, ''));
              if (Number.isInteger(n) && n > 0) id = n;
            })
            .catch(() => {})
        : Promise.resolve();
    listo.then(leerPantalla);

    const intervalo = setInterval(() => {
      if (document.visibilityState === 'visible') leerNivel();
    }, CADA);
    const alVolver = () => {
      if (document.visibilityState !== 'visible') return;
      ponerTema(leerTema());
      leerPantalla();
    };
    const alGuardar = (e: StorageEvent) => {
      if (e.key === 'nexo-tema') ponerTema(leerTema());
    };
    document.addEventListener('visibilitychange', alVolver);
    window.addEventListener('focus', alVolver);
    window.addEventListener('storage', alGuardar);
    const quitarBrillo = alCambiarBrillo((c) => {
      if (c.id === id) llega(c.nivel);
    });
    const quitarTema = alCambiarTema(ponerTema);
    const observador = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(() => ajustarAlto()) : null;
    if (contenido) observador?.observe(contenido);

    return () => {
      clearInterval(intervalo);
      document.removeEventListener('visibilitychange', alVolver);
      window.removeEventListener('focus', alVolver);
      window.removeEventListener('storage', alGuardar);
      quitarBrillo();
      quitarTema();
      observador?.disconnect();
    };
  });
</script>

<svelte:head>
  <title>Brillo{nombre ? ` de ${nombre}` : ''}</title>
</svelte:head>

<svelte:window onpointerdown={alPresionarVentana} onblur={() => (menu = false)} oncontextmenu={abrirMenu} />

<div class="lienzo">
  <div
    class="tarjeta"
    data-tauri-drag-region="deep"
    role="group"
    aria-label={nombre ? `Brillo de ${nombre}` : 'Brillo'}
    style:--tono={tono}
  >
    <div class="contenido" bind:this={contenido}>
      <div class="arriba">
        <span class="sol" aria-hidden="true"><Icono nombre="sol" tamano={18} /></span>
        <span class="nombre">{nombre || (leido ? 'Pantalla' : '')}</span>
        {#if disponible && brillo !== null}
          <span class="cifra">{brillo}<small>%</small></span>
        {/if}
      </div>

      {#if disponible && brillo !== null}
        <div class="control">
          <button class="redondo" type="button" onclick={() => paso(false)} disabled={brillo <= 0} aria-label="Bajar brillo">
            −
          </button>
          <Deslizador
            valor={brillo}
            color="amarillo"
            {tono}
            etiqueta={nombre ? `Brillo de ${nombre}` : 'Brillo'}
            oncambio={cambiar}
            ontocando={alTocar}
          />
          <button class="redondo" type="button" onclick={() => paso(true)} disabled={brillo >= 100} aria-label="Subir brillo">
            +
          </button>
        </div>
      {:else if leido}
        <p class="nota">Esta pantalla no está conectada o no deja cambiar el brillo.</p>
      {:else}
        <span class="hueso" aria-hidden="true"></span>
      {/if}
    </div>

    {#if menu}
      <div
        bind:this={listaMenu}
        class="menu flotante"
        role="menu"
        tabindex="-1"
        aria-label="Opciones del widget de brillo"
        data-tauri-drag-region="false"
        onkeydown={teclaMenu}
        transition:fade={{ duration: 120 }}
      >
        <button type="button" role="menuitem" onclick={() => elegir(() => abrirSeccion('pantallas'))}>
          Abrir Pantallas en Nexo
        </button>
        <button
          type="button"
          role="menuitem"
          onclick={() => elegir(() => (id === null ? Promise.resolve() : usarWidgetBrillo(id, false)))}
        >
          Quitar widget
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  /* La ventana es transparente: solo se ve la tarjeta, con sus esquinas redondas. */
  :global(html),
  :global(body) {
    background: transparent !important;
    overflow: hidden;
  }

  /* 6 px transparentes alrededor para la sombra corta de la tarjeta. */
  .lienzo {
    height: 100vh;
    padding: 6px;
  }

  .tarjeta {
    position: relative;
    display: flex;
    align-items: center;
    height: 100%;
    padding: 10px 12px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-md);
    background: var(--superficie);
    box-shadow:
      var(--sombra-panel),
      0 3px 8px -4px rgb(0 0 0 / 0.3);
  }

  .contenido {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
  }

  .arriba {
    display: flex;
    align-items: center;
    gap: 7px;
    min-height: 28px;
  }
  .sol {
    display: grid;
    flex-shrink: 0;
    color: var(--tono);
  }
  .nombre {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    line-height: 16px;
    font-weight: 600;
    color: var(--texto-2);
    overflow-wrap: anywhere;
  }
  .cifra {
    flex-shrink: 0;
    font-size: 26px;
    line-height: 28px;
    font-weight: 700;
    letter-spacing: -0.03em;
    font-variant-numeric: tabular-nums;
    color: var(--texto);
  }
  .cifra small {
    margin-left: 1px;
    font-size: 13px;
    font-weight: 600;
    color: var(--texto-2);
  }

  .control {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) 28px;
    align-items: center;
    gap: 8px;
  }
  .redondo {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0 0 2px;
    border: 0;
    border-radius: 50%;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--tono);
    font-size: 18px;
    line-height: 1;
    font-weight: 700;
    transition: background-color var(--dur-color) ease;
  }
  .redondo:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .redondo:active:not(:disabled) {
    transform: scale(0.96);
  }
  .redondo:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .nota {
    margin: 0;
    font-size: 12px;
    line-height: 16px;
    color: var(--texto-2);
  }

  /* Mientras llega la primera lectura: una silueta quieta. */
  .hueso {
    height: 10px;
    margin: 9px 0;
    border-radius: 6px;
    background: var(--superficie-hundida);
  }

  /* Clic derecho: el menú dentro de la tarjeta (la ventana es chica). */
  .menu {
    position: absolute;
    z-index: 3;
    top: 4px;
    right: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 176px;
    padding: 4px;
    border-radius: var(--radio-sm);
  }
  .menu:focus {
    outline: none;
  }
  .menu button {
    display: flex;
    align-items: center;
    min-height: 30px;
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
