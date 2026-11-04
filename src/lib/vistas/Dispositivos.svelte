<!--
  Dispositivos (DISENO.md, sección 7.2).
  - Lista: "Mis dispositivos", una tarjeta grande por aparato y "Agregar dispositivo" (abre Ajustes › Bluetooth).
    El buscador está en la barra de arriba (lo pone la ventana); aquí solo se filtra con `busqueda`.
  - A la derecha, el panel del aparato elegido. Un clic en la tarjeta lo elige.
  - "Entrar" abre la página con todos sus ajustes (`pagina`): con el chevron, doble clic, Enter sobre la tarjeta
    elegida o "Ver todos los ajustes". Atrás o Escape vuelven a la lista.
  - Con la ventana angosta (menos de 1000 px) no hay panel: elegir una tarjeta abre la página.
  Renombrar pone un alias dentro de Nexo; Olvidar desempareja (si el sistema no deja, ofrece Ajustes › Bluetooth).
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { MediaQuery } from 'svelte/reactivity';
  import type { Dispositivo, Estado, Registro } from '$lib/tipos';
  import { nombreDe } from '$lib/tipos';
  import { abrirAjustesBluetooth, leerHistoriales, olvidarDispositivo, ponerAlias } from '$lib/api';
  import { ajustesBluetoothTexto, textoAbrirBluetooth } from '$lib/api-sistema';
  import { ultimaVez } from '$lib/tiempo';
  import { tooltip } from '$lib/tooltip';
  import FilaDispositivo from '$lib/componentes/FilaDispositivo.svelte';
  import DetalleDispositivo from '$lib/componentes/DetalleDispositivo.svelte';
  import PaginaDispositivo from '$lib/componentes/PaginaDispositivo.svelte';
  import Dialogo from '$lib/componentes/Dialogo.svelte';
  import NexoBot from '$lib/componentes/NexoBot.svelte';
  import Icono from '$lib/componentes/Icono.svelte';

  let {
    estado,
    pendienteId,
    onalternar,
    seleccionInicial = null,
    onerror,
    onseleccion,
    onactualizar,
    busqueda = $bindable(''),
    pagina = $bindable(false),
  }: {
    estado: Estado | null;
    pendienteId: string | null;
    onalternar: (d: Dispositivo) => void;
    /** Aparato que se abre al llegar (por ejemplo desde Inicio o con `?ver=<id>`). */
    seleccionInicial?: string | null;
    onerror: (texto: string) => void;
    /** Avisa qué aparato quedó elegido, para volver a él al regresar a esta sección. */
    onseleccion?: (id: string | null) => void;
    /** Vuelve a leer los aparatos (después de renombrar u olvidar). */
    onactualizar?: () => Promise<void> | void;
    /** Lo que se escribe en el buscador de la barra de arriba. */
    busqueda?: string;
    /** true: se ve la página del aparato elegido en lugar de la lista. */
    pagina?: boolean;
  } = $props();

  // Debe coincidir con la media query de PaginaDispositivo.
  const angosta = new MediaQuery('max-width: 999.98px');

  let seleccionadoId = $state<string | null>(untrack(() => seleccionInicial));
  $effect(() => {
    const id = seleccionInicial;
    if (id) untrack(() => (seleccionadoId = id));
  });

  const dispositivos = $derived(estado?.dispositivos ?? []);
  const conectados = $derived(dispositivos.filter((d) => d.conectado).length);

  /** Minúsculas y sin tildes, para que "beats" encuentre "Beats Solo³" y "telefono" a "Teléfono". */
  const normalizar = (t: string) => t.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase();
  const consulta = $derived(busqueda.trim());
  const filtrados = $derived.by(() => {
    const q = normalizar(consulta);
    if (!q) return dispositivos;
    return dispositivos.filter((d) => normalizar(d.nombre).includes(q) || (d.alias ? normalizar(d.alias).includes(q) : false));
  });

  const seleccionado = $derived(dispositivos.find((d) => d.id === seleccionadoId) ?? null);
  // Con espacio para el panel siempre hay uno elegido: el que se tocó o, si no, el primero conectado.
  const elegido = $derived(
    seleccionado ?? (angosta.current ? null : (dispositivos.find((d) => d.conectado) ?? dispositivos[0] ?? null)),
  );
  // La página es siempre la de un aparato que se eligió (nunca la del primero por defecto).
  const enPagina = $derived(pagina && !!seleccionado);

  // Si el aparato de la página ya no está (se olvidó o no llegó), se vuelve a la lista.
  $effect(() => {
    if (pagina && estado && !seleccionado) pagina = false;
  });

  // Al escribir en el buscador con la página abierta, se vuelve a la lista para ver los resultados.
  let busquedaPrevia = untrack(() => busqueda.trim());
  $effect(() => {
    const q = consulta;
    if (q && q !== busquedaPrevia) untrack(() => (pagina = false));
    busquedaPrevia = q;
  });

  function elegir(id: string | null) {
    seleccionadoId = id;
    onseleccion?.(id);
  }

  function entrar(id: string) {
    elegir(id);
    pagina = true;
  }

  async function volver() {
    const id = elegido?.id;
    pagina = false;
    // De vuelta en la lista, el foco queda en la tarjeta del aparato.
    await tick();
    const tarjeta = id ? document.querySelector<HTMLElement>(`[data-dispositivo="${CSS.escape(id)}"] .abrir`) : null;
    tarjeta?.focus({ preventScroll: true });
    tarjeta?.scrollIntoView({ block: 'nearest' });
  }

  // Un clic elige; con la ventana angosta no hay panel, así que entra directo.
  function abrir(id: string) {
    if (angosta.current) entrar(id);
    else elegir(id);
  }

  // Historial de todos (para "usado ayer a las 21:10" y los paneles de la página).
  // Se lee al llegar, cuando cambia algo de la lista y cada 20 s con el panel a la vista (el vigía de Rust también va cada 20 s).
  let historiales = $state<Record<string, Registro>>({});
  const firma = $derived(dispositivos.map((d) => `${d.id}:${d.conectado ? 1 : 0}`).join('|'));
  async function cargarHistoriales() {
    const ids = untrack(() => dispositivos.map((d) => d.id));
    if (!ids.length) return;
    historiales = await leerHistoriales(ids);
  }
  $effect(() => {
    void firma;
    cargarHistoriales();
  });
  $effect(() => {
    const t = setInterval(() => document.visibilityState === 'visible' && cargarHistoriales(), 20_000);
    return () => clearInterval(t);
  });

  // --- Renombrar (alias dentro de Nexo) ---
  let renombrarAbierto = $state(false);
  let renombrando = $state<Dispositivo | null>(null);
  let alias = $state('');

  function abrirRenombrar(d: Dispositivo) {
    renombrando = d;
    alias = nombreDe(d);
    renombrarAbierto = true;
  }

  async function guardarAlias() {
    renombrarAbierto = false;
    const d = renombrando;
    if (!d) return;
    const texto = alias.trim();
    const nuevo = !texto || texto === d.nombre ? null : texto;
    if (nuevo === (d.alias?.trim() || null)) return;
    try {
      await ponerAlias(d.id, nuevo);
      await onactualizar?.();
    } catch (e) {
      onerror(String(e));
    }
  }

  // --- Olvidar (desemparejar) ---
  let olvidarAbierto = $state(false);
  let olvidando = $state<Dispositivo | null>(null);
  let avisoOlvidar = $state<{ id: string; texto: string } | null>(null);

  function abrirOlvidar(d: Dispositivo) {
    olvidando = d;
    olvidarAbierto = true;
  }

  async function confirmarOlvidar() {
    olvidarAbierto = false;
    const d = olvidando;
    if (!d) return;
    try {
      await olvidarDispositivo(d.id);
      avisoOlvidar = null;
      if (seleccionadoId === d.id) {
        elegir(null);
        pagina = false;
      }
      await onactualizar?.();
    } catch (e) {
      onerror(String(e));
      avisoOlvidar = { id: d.id, texto: `Nexo no pudo olvidarlo. Puedes hacerlo desde ${ajustesBluetoothTexto}.` };
    }
  }

  const ajustesBluetooth = () => abrirAjustesBluetooth().catch((e) => onerror(String(e)));

  function teclas(e: KeyboardEvent) {
    // Escape vuelve de la página a la lista, salvo que lo use un diálogo o un menú abierto.
    if (e.key !== 'Escape' || e.defaultPrevented || !enPagina || renombrarAbierto || olvidarAbierto) return;
    e.preventDefault();
    volver();
  }
</script>

<svelte:window onkeydown={teclas} />

{#if estado && !estado.bluetoothEncendido}
  <div class="vacio">
    <NexoBot expresion="bluetooth-apagado" tamano={150} />
    <h2>El Bluetooth está apagado</h2>
    <p>Enciéndelo para ver y conectar tus dispositivos.</p>
    <button class="secundario" onclick={ajustesBluetooth}>{textoAbrirBluetooth}</button>
  </div>
{:else if estado && dispositivos.length === 0}
  <div class="vacio">
    <NexoBot expresion="sin-dispositivos" tamano={150} />
    <h2>Aún no hay dispositivos</h2>
    <p>Empareja tus audífonos, teclado o mouse en {ajustesBluetoothTexto} y aparecerán aquí.</p>
    <button class="boton-primario" onclick={ajustesBluetooth}><Icono nombre="mas" tamano={16} /> Agregar dispositivo</button>
  </div>
{:else if enPagina && elegido}
  {#key elegido.id}
    <PaginaDispositivo
      dispositivo={elegido}
      pendiente={pendienteId === elegido.id}
      registro={historiales[elegido.id] ?? null}
      avisoOlvidar={avisoOlvidar?.id === elegido.id ? avisoOlvidar.texto : null}
      onalternar={() => elegido && onalternar(elegido)}
      onrenombrar={() => elegido && abrirRenombrar(elegido)}
      onolvidar={() => elegido && abrirOlvidar(elegido)}
      onvolver={volver}
      {onerror}
    />
  {/key}
{:else}
  <div class="dispositivos" class:angosta={angosta.current}>
    <div class="columna-lista">
      <header class="encabezado">
        <div class="encabezado__textos">
          <h1>Mis dispositivos</h1>
          <p>Gestiona tus dispositivos conectados</p>
        </div>
        {#if estado && dispositivos.length}
          <span class="resumen" class:alguno={conectados > 0}>
            <span class="punto" aria-hidden="true"></span>
            {conectados} de {dispositivos.length} {conectados === 1 ? 'conectado' : 'conectados'}
          </span>
        {/if}
      </header>

      <div class="lista">
        {#if !estado}
          <ul aria-busy="true" aria-label="Cargando dispositivos">
            {#each [0, 1, 2] as i (i)}
              <li class="fila-cargando"><span class="foto"></span><span class="lineas"><span></span><span></span></span></li>
            {/each}
          </ul>
        {:else}
          {#if filtrados.length}
            <ul aria-label="Mis dispositivos">
              {#each filtrados as d (d.id)}
                <li>
                  <FilaDispositivo
                    dispositivo={d}
                    grande
                    pendiente={pendienteId === d.id}
                    elegido={elegido?.id === d.id}
                    ultimaVez={historiales[d.id] ? ultimaVez(historiales[d.id].sesiones) : null}
                    onabrir={() => abrir(d.id)}
                    onentrar={() => entrar(d.id)}
                  />
                </li>
              {/each}
            </ul>
          {:else}
            <div class="sin-resultados" role="status">
              <p>Nada coincide con «{consulta}».</p>
              <button class="secundario" onclick={() => (busqueda = '')}>Borrar búsqueda</button>
            </div>
          {/if}
          <button class="agregar" onclick={ajustesBluetooth} use:tooltip={`El emparejamiento se hace en ${ajustesBluetoothTexto}`}>
            <span class="ficha-icono chica"><Icono nombre="mas" tamano={17} /></span>
            <span class="agregar__textos">
              <span class="agregar__titulo">Agregar dispositivo</span>
              <span class="agregar__sub">Se empareja en {ajustesBluetoothTexto}</span>
            </span>
          </button>
        {/if}
      </div>
    </div>

    {#if !angosta.current}
      {#if elegido}
        <div class="columna-detalle">
          {#key elegido.id}
            <DetalleDispositivo
              dispositivo={elegido}
              pendiente={pendienteId === elegido.id}
              avisoOlvidar={avisoOlvidar?.id === elegido.id ? avisoOlvidar.texto : null}
              onalternar={() => elegido && onalternar(elegido)}
              onrenombrar={() => elegido && abrirRenombrar(elegido)}
              onolvidar={() => elegido && abrirOlvidar(elegido)}
              onentrar={() => elegido && entrar(elegido.id)}
              {onerror}
            />
          {/key}
        </div>
      {:else if !estado}
        <div class="columna-detalle"><div class="detalle-cargando" aria-hidden="true"></div></div>
      {/if}
    {/if}
  </div>
{/if}

<Dialogo
  abierto={renombrarAbierto}
  titulo="Renombrar"
  descripcion={renombrando
    ? `El nombre cambia solo dentro de Nexo. Si lo dejas vacío, vuelve a «${renombrando.nombre}».`
    : undefined}
  icono="editar"
  textoConfirmar="Guardar"
  onconfirmar={guardarAlias}
  oncancelar={() => (renombrarAbierto = false)}
>
  <input
    class="campo"
    type="text"
    bind:value={alias}
    placeholder={renombrando?.nombre}
    aria-label="Nombre en Nexo"
    maxlength="40"
    autocomplete="off"
    spellcheck="false"
    data-autofocus
  />
</Dialogo>

<Dialogo
  abierto={olvidarAbierto}
  titulo={olvidando ? `¿Olvidar ${nombreDe(olvidando)}?` : 'Olvidar'}
  descripcion={`Tendrás que emparejarlo de nuevo desde ${ajustesBluetoothTexto}.`}
  icono="papelera"
  tono="peligro"
  textoConfirmar="Olvidar"
  onconfirmar={confirmarOlvidar}
  oncancelar={() => (olvidarAbierto = false)}
/>

<style>
  .dispositivos {
    height: 100%;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) clamp(360px, 38%, 440px);
    gap: 20px;
  }
  .dispositivos.angosta {
    grid-template-columns: minmax(0, 1fr);
  }

  /* Lista */
  .columna-lista {
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .encabezado {
    flex-shrink: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 8px 16px;
    margin: 4px 0 20px;
  }
  .encabezado__textos {
    min-width: 0;
  }
  h1 {
    margin: 0;
    font: var(--letra-titulo);
    letter-spacing: -0.025em;
  }
  .encabezado p {
    margin: 4px 0 0;
    font: var(--letra-cuerpo);
    color: var(--texto-2);
  }
  /* Resumen: cuántos hay conectados, con el punto verde si alguno lo está. */
  .resumen {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 30px;
    padding: 0 12px 0 10px;
    border-radius: var(--radio-pildora);
    background: var(--relleno-tarjeta);
    box-shadow:
      inset 0 1px 0 var(--luz-tope),
      0 0 0 1px var(--borde-tarjeta),
      var(--sombra-tarjeta);
    font-size: 12.5px;
    font-weight: 600;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .resumen .punto {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--texto-3);
  }
  .resumen.alguno .punto {
    background: var(--exito);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--exito) 22%, transparent);
  }

  .lista {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    /* Aire para que el anillo de foco, la sombra y la presión de la tarjeta no queden cortados por el borde. */
    margin: -8px;
    padding: 8px 8px 12px;
  }
  ul {
    display: grid;
    gap: 12px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .sin-resultados {
    display: grid;
    justify-items: start;
    gap: 10px;
    padding: 14px 4px 6px;
  }
  .sin-resultados p {
    margin: 0;
    font-size: 14px;
    line-height: 21px;
    color: var(--texto-2);
    overflow-wrap: anywhere;
  }
  /* Agregar: borde punteado, la ficha turquesa con el "+" y dónde se empareja. */
  .agregar {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    min-height: 68px;
    margin-top: 12px;
    padding: 12px 16px;
    border: 1.5px dashed color-mix(in srgb, var(--texto-3) 45%, transparent);
    border-radius: 20px;
    background: transparent;
    color: var(--texto);
    text-align: left;
    transition:
      background-color var(--dur-color) ease,
      border-color var(--dur-color) ease;
  }
  .agregar:hover {
    background: var(--superficie-hover);
    border-color: var(--elegido-borde);
  }
  .agregar__textos {
    display: grid;
    gap: 1px;
    min-width: 0;
  }
  .agregar__titulo {
    font-size: 14px;
    line-height: 20px;
    font-weight: 600;
  }
  .agregar__sub {
    font-size: 12.5px;
    line-height: 17px;
    color: var(--texto-2);
    overflow-wrap: break-word;
  }
  .agregar:active {
    transform: scale(0.98);
  }

  /* Panel del elegido */
  .columna-detalle {
    min-height: 0;
    height: 100%;
  }

  /* Cargando: siluetas de las tarjetas y del panel */
  .fila-cargando {
    display: flex;
    align-items: center;
    gap: 16px;
    height: 116px;
    padding: 10px 14px;
    border-radius: 20px;
    background: var(--relleno-tarjeta);
    box-shadow:
      0 0 0 1px var(--borde-tarjeta),
      var(--sombra-tarjeta);
  }
  .fila-cargando .foto {
    width: 92px;
    height: 92px;
    border-radius: 18px;
    background: var(--superficie-elevada);
  }
  .lineas {
    display: grid;
    gap: 10px;
    flex: 1;
  }
  .lineas span {
    height: 12px;
    width: 46%;
    border-radius: 6px;
    background: var(--superficie-elevada);
  }
  .lineas span + span {
    width: 68%;
  }
  .detalle-cargando {
    height: 100%;
    border-radius: var(--radio-lg);
    background: var(--relleno-tarjeta);
    box-shadow:
      0 0 0 1px var(--borde-tarjeta),
      var(--sombra-tarjeta);
  }
  .fila-cargando,
  .detalle-cargando {
    animation: respirar 1.4s ease-in-out infinite;
  }
  @keyframes respirar {
    50% {
      opacity: 0.55;
    }
  }

  /* Vacíos: la mascota y una salida clara */
  .vacio {
    display: grid;
    justify-items: center;
    align-content: center;
    gap: 6px;
    min-height: 100%;
    max-width: 400px;
    margin: 0 auto;
    padding-bottom: 40px;
    text-align: center;
  }
  .vacio h2 {
    margin: 12px 0 0;
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .vacio p {
    margin: 0 0 14px;
    font-size: 13.5px;
    color: var(--texto-2);
  }
  .secundario {
    min-height: 36px;
    padding: 0 16px;
    border: 0;
    border-radius: var(--radio-sm);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .secundario:hover {
    background: var(--superficie-hover);
  }

  /* Campo del diálogo Renombrar (va dentro de Dialogo, que está al final de <body>) */
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
</style>
