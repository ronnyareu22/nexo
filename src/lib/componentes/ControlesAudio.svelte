<!--
  Controles de sonido de un aparato, los mismos en el panel lateral y en la página del aparato:
  - volumen de salida (el botón redondo silencia) y micrófono del aparato, solo si está conectado;
  - los menús de salida y entrada del sistema (todos los aparatos de sonido, como en la página Audio).
  `variante="panel"`: dos grupos de filas (deslizadores y menús), para el panel lateral de Dispositivos.
  `variante="tarjeta"`: dos tarjetas hermanas, "Sonido" y "Salida y entrada", para la página del aparato.
  Una sola lectura para las dos: cada 2 s mientras se ve, sin pisar un deslizador que se mueve.
  Siempre se manda el último valor.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { ajustarSonido, leerSonido, listarDispositivosAudio, usarDispositivoAudio } from '$lib/api';
  import { esMac } from '$lib/api-sistema';
  import { enviadorDelUltimo } from '$lib/enviador';
  import { tooltip } from '$lib/tooltip';
  import type { Dispositivo, DispositivoAudio, Sonido, TransporteAudio } from '$lib/tipos';
  import Icono, { type Nombre } from './Icono.svelte';
  import Deslizador from './Deslizador.svelte';
  import MenuDesplegable from './MenuDesplegable.svelte';

  let {
    dispositivo: d,
    variante = 'panel',
    onerror,
  }: {
    dispositivo: Dispositivo;
    variante?: 'panel' | 'tarjeta';
    onerror: (texto: string) => void;
  } = $props();

  const id = $props.id();

  let sonido = $state<Sonido | null>(null);
  let lista = $state<DispositivoAudio[] | null>(null);
  // Mientras se arrastra un deslizador, o justo después de un cambio, la lectura periódica no pisa los valores.
  let tocando = false;
  let ultimoCambio = 0;
  let fallando = false;
  const RESPIRO = 800;

  async function leer() {
    const inicio = Date.now();
    const [s, l] = await Promise.allSettled([
      d.conectado ? leerSonido(d.id) : Promise.resolve(null),
      listarDispositivosAudio(),
    ]);
    // Si algo cambió durante la lectura (o hace muy poco), esta lectura ya es vieja.
    const vieja = tocando || ultimoCambio > inicio - RESPIRO;
    if (s.status === 'fulfilled' && !vieja) sonido = s.value;
    if (l.status === 'fulfilled' && !vieja) lista = l.value;
    const error = s.status === 'rejected' ? s.reason : l.status === 'rejected' ? l.reason : null;
    // Se avisa una sola vez, no en cada vuelta.
    if (error !== null && !fallando) onerror(String(error));
    fallando = error !== null;
  }

  // Al conectar o desconectar, se lee enseguida.
  $effect(() => {
    void d.conectado;
    untrack(leer);
  });
  onMount(() => {
    const intervalo = setInterval(() => document.visibilityState === 'visible' && leer(), 2000);
    return () => clearInterval(intervalo);
  });

  // Del aparato: solo con el aparato conectado.
  const propio = $derived(d.conectado ? sonido : null);
  const conVolumen = $derived(!!propio?.tieneSalida && propio.volumen !== null);
  const conMicrofono = $derived(!!propio?.tieneMicrofono && propio.volumenMicrofono !== null);

  // --- Volumen, silencio y micrófono del aparato ---
  const fallo = (e: unknown) => onerror(String(e));
  const enviarVolumen = enviadorDelUltimo((v: number) => ajustarSonido(d.id, { accion: 'volumen', valor: v }), fallo);
  const enviarMicrofono = enviadorDelUltimo(
    (v: number) => ajustarSonido(d.id, { accion: 'volumenMicrofono', valor: v }),
    fallo,
  );

  function volumen(v: number) {
    ultimoCambio = Date.now();
    if (sonido) sonido.volumen = v;
    enviarVolumen(v);
  }

  function microfono(v: number) {
    ultimoCambio = Date.now();
    if (sonido) sonido.volumenMicrofono = v;
    enviarMicrofono(v);
  }

  async function silenciar() {
    if (!sonido || sonido.silenciado === null) return;
    ultimoCambio = Date.now();
    const valor = !sonido.silenciado;
    sonido.silenciado = valor;
    try {
      const nuevo = await ajustarSonido(d.id, { accion: 'silenciar', valor });
      if (!tocando) sonido = nuevo;
    } catch (e) {
      onerror(String(e));
    }
  }

  // "Usar para el sonido": la salida del sistema pasa a este aparato.
  async function usarComoSalida() {
    ultimoCambio = Date.now();
    if (sonido) sonido.salidaEnUso = true;
    try {
      const nuevo = await ajustarSonido(d.id, { accion: 'usarSalida' });
      if (!tocando) sonido = nuevo;
      ultimoCambio = 0;
      await leer();
    } catch (e) {
      onerror(String(e));
    }
  }

  function alTocar(t: boolean) {
    tocando = t;
    ultimoCambio = Date.now();
  }

  // --- Salida y entrada del sistema ---
  const porNombre = (a: DispositivoAudio, b: DispositivoAudio) =>
    Number(b.transporte === 'integrado') - Number(a.transporte === 'integrado') || a.nombre.localeCompare(b.nombre, 'es');

  const salidas = $derived((lista ?? []).filter((x) => x.salida).sort(porNombre));
  const entradas = $derived((lista ?? []).filter((x) => x.entrada).sort(porNombre));

  const detalle: Record<TransporteAudio, string | undefined> = {
    integrado: 'Integrado',
    bluetooth: 'Bluetooth',
    usb: 'USB',
    hdmi: 'HDMI',
    displayport: 'DisplayPort',
    airplay: 'AirPlay',
    virtual: 'Virtual',
    otro: undefined,
  };

  function icono(x: DispositivoAudio, deEntrada: boolean): Nombre {
    switch (x.transporte) {
      case 'integrado':
      case 'hdmi':
      case 'displayport':
        return 'pantallas';
      case 'bluetooth':
        return 'audio';
      case 'usb':
        return 'conectar';
      case 'virtual':
        return 'ajustes';
      default:
        return deEntrada ? 'microfono' : 'volumen';
    }
  }

  const opcionesSalida = $derived(
    salidas.map((x) => ({ valor: x.uid, nombre: x.nombre, detalle: detalle[x.transporte], icono: icono(x, false) })),
  );
  const opcionesEntrada = $derived(
    entradas.map((x) => ({ valor: x.uid, nombre: x.nombre, detalle: detalle[x.transporte], icono: icono(x, true) })),
  );

  async function usar(uid: string, deEntrada: boolean) {
    ultimoCambio = Date.now();
    for (const x of lista ?? []) {
      if (deEntrada && x.entrada) x.entradaEnUso = x.uid === uid;
      if (!deEntrada && x.salida) x.salidaEnUso = x.uid === uid;
    }
    try {
      const nueva = await usarDispositivoAudio(uid, deEntrada);
      if (!tocando) lista = nueva;
    } catch (e) {
      onerror(String(e));
    }
  }

  const equipo = esMac ? 'la Mac' : 'la computadora';
  const ayudaSonido = $derived(
    !d.conectado
      ? 'Sin conexión'
      : !propio
        ? 'Leyendo el sonido…'
        : propio.salidaEnUso
          ? `El sonido de ${equipo} sale por aquí`
          : `El sonido de ${equipo} sale por otro aparato`,
  );
</script>

<!-- Un deslizador: el botón (o ícono) redondo, la pista y el valor. Con `rotulo`, el nombre va arriba. -->
{#snippet volumenSalida(rotulo: boolean)}
  {#if conVolumen && propio}
    {@const v = propio.volumen ?? 0}
    <div class="control" class:con-rotulo={rotulo}>
      {#if propio.silenciado !== null}
        <button
          class="redondo"
          class:silenciado={propio.silenciado}
          type="button"
          aria-pressed={propio.silenciado}
          aria-label={propio.silenciado ? 'Quitar silencio' : 'Silenciar'}
          use:tooltip={propio.silenciado ? 'Quitar silencio' : 'Silenciar'}
          onclick={silenciar}
        >
          <Icono nombre={propio.silenciado || v === 0 ? 'silencio' : 'volumen'} tamano={18} />
        </button>
      {:else}
        <span class="redondo quieto" aria-hidden="true"><Icono nombre="volumen" tamano={18} /></span>
      {/if}
      {#if rotulo}<span class="nombre" aria-hidden="true">Volumen de salida</span>{/if}
      <div class="pista">
        <Deslizador
          valor={v}
          color="celeste"
          etiqueta="Volumen de salida"
          apagado={propio.silenciado ?? false}
          oncambio={volumen}
          ontocando={alTocar}
        />
      </div>
      <span class="valor" class:apagado={propio.silenciado}>{v}<small>%</small></span>
    </div>
  {/if}
{/snippet}

{#snippet volumenMicrofono(rotulo: boolean)}
  {#if conMicrofono && propio}
    {@const m = propio.volumenMicrofono ?? 0}
    <div class="control" class:con-rotulo={rotulo}>
      <span class="redondo quieto" aria-hidden="true" use:tooltip={rotulo ? undefined : 'Micrófono'}>
        <Icono nombre="microfono" tamano={18} />
      </span>
      {#if rotulo}<span class="nombre" aria-hidden="true">Micrófono</span>{/if}
      <div class="pista">
        <Deslizador valor={m} color="celeste" etiqueta="Micrófono" oncambio={microfono} ontocando={alTocar} />
      </div>
      <span class="valor">{m}<small>%</small></span>
    </div>
  {/if}
{/snippet}

<!-- Los menús de salida y entrada del sistema. `apilados`: el rótulo arriba y el menú a lo ancho. -->
{#snippet menus(apilados: boolean)}
  <div class="opcion" class:apilada={apilados}>
    <span class="opcion__rotulo">
      <Icono nombre="volumen" tamano={17} />
      Salida de audio
    </span>
    <div class="opcion__menu">
      {#if lista}
        <MenuDesplegable
          opciones={opcionesSalida}
          valor={salidas.find((x) => x.salidaEnUso)?.uid ?? ''}
          etiqueta="Salida de audio"
          oncambio={(uid) => usar(uid, false)}
        />
      {:else}
        <span class="silueta-menu" aria-hidden="true"></span>
      {/if}
    </div>
  </div>
  <div class="opcion" class:apilada={apilados}>
    <span class="opcion__rotulo">
      <Icono nombre="microfono" tamano={17} />
      Entrada de audio
    </span>
    <div class="opcion__menu">
      {#if lista}
        <MenuDesplegable
          opciones={opcionesEntrada}
          valor={entradas.find((x) => x.entradaEnUso)?.uid ?? ''}
          etiqueta="Entrada de audio"
          oncambio={(uid) => usar(uid, true)}
        />
      {:else}
        <span class="silueta-menu" aria-hidden="true"></span>
      {/if}
    </div>
  </div>
{/snippet}

{#if variante === 'panel'}
  {#if conVolumen || conMicrofono}
    <div class="grupo inserto deslizadores">
      {@render volumenSalida(false)}
      {@render volumenMicrofono(false)}
    </div>
  {/if}
  <div class="grupo inserto">{@render menus(false)}</div>
{:else}
  <!-- Tarjeta 1: el sonido del propio aparato -->
  <section class="panel vidrio" aria-labelledby="{id}-sonido">
    <header class="t-cabeza">
      <span class="ficha-icono" class:tenue={!d.conectado} style:--tono="var(--celeste)"><Icono nombre="volumen" tamano={19} /></span>
      <div class="t-textos">
        <h2 class="t-titulo" id="{id}-sonido">Sonido</h2>
        <p class="t-sub">{ayudaSonido}</p>
      </div>
      {#if propio?.tieneSalida}
        {#if propio.salidaEnUso}
          <span class="en-uso"><Icono nombre="check" tamano={14} /> En uso</span>
        {:else}
          <button class="usar" type="button" onclick={usarComoSalida}>Usar para el sonido</button>
        {/if}
      {/if}
    </header>

    {#if !d.conectado}
      <div class="vacio">
        <span class="ficha-icono tenue grande"><Icono nombre="silencio" tamano={22} /></span>
        <p>Conéctalo para cambiar su volumen y su micrófono.</p>
      </div>
    {:else if propio && !conVolumen && !conMicrofono}
      <div class="vacio">
        <span class="ficha-icono tenue grande"><Icono nombre="volumen" tamano={22} /></span>
        <p>{esMac ? 'macOS' : 'Windows'} todavía no lo muestra como salida de sonido ni como micrófono. Si recién se conectó, espera unos segundos.</p>
      </div>
    {:else}
      <div class="controles">
        {@render volumenSalida(true)}
        {@render volumenMicrofono(true)}
      </div>
    {/if}
  </section>

  <!-- Tarjeta 2: salida y entrada del sistema -->
  <section class="panel vidrio" aria-labelledby="{id}-salidas">
    <header class="t-cabeza">
      <span class="ficha-icono" style:--tono="var(--celeste)"><Icono nombre="audio" tamano={19} /></span>
      <div class="t-textos">
        <h2 class="t-titulo" id="{id}-salidas">Salida y entrada</h2>
        <p class="t-sub">Para todo el sonido de {equipo}</p>
      </div>
    </header>
    <div class="menus">{@render menus(true)}</div>
  </section>
{/if}

<style>
  /* ---------- Fila de un deslizador ---------- */
  .control {
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr) auto;
    align-items: center;
    column-gap: 12px;
    min-height: 52px;
  }
  /* Con rótulo: el nombre arriba, la pista abajo; el botón y el valor se alinean con la pista. */
  .control.con-rotulo {
    grid-template-columns: 36px minmax(0, 1fr) auto;
    grid-template-rows: auto auto;
    row-gap: 4px;
  }
  .con-rotulo .redondo {
    grid-row: 1 / 3;
  }
  .con-rotulo .nombre {
    grid-column: 2;
    font-size: 13px;
    line-height: 18px;
    font-weight: 600;
    color: var(--texto);
  }
  .con-rotulo .pista {
    grid-column: 2;
    grid-row: 2;
  }
  .con-rotulo .valor {
    grid-column: 3;
    grid-row: 1 / 3;
  }
  .pista {
    min-width: 0;
  }
  .redondo {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: color-mix(in srgb, var(--celeste) 12%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--celeste) 18%, transparent);
    color: var(--celeste);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  button.redondo:hover {
    background: color-mix(in srgb, var(--celeste) 20%, transparent);
  }
  button.redondo:active {
    transform: scale(0.98);
  }
  .redondo.silenciado {
    background: var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-3);
  }
  .valor {
    min-width: 46px;
    text-align: right;
    font-size: 15px;
    line-height: 20px;
    font-weight: 650;
    letter-spacing: -0.01em;
    font-variant-numeric: tabular-nums;
  }
  .valor small {
    margin-left: 1px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--texto-2);
  }
  .valor.apagado {
    color: var(--texto-3);
  }

  /* ---------- Menús de salida y entrada ---------- */
  /* En fila: el rótulo a la izquierda y el menú a la derecha; si no cabe, el menú baja. */
  .opcion {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 12px;
    min-height: 56px;
    padding: 9px 0;
  }
  .opcion + .opcion {
    border-top: 1px solid var(--borde);
  }
  .opcion__rotulo {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    flex: 0 1 auto;
    font-size: 13.5px;
    line-height: 19px;
    font-weight: 500;
  }
  .opcion__rotulo :global(svg) {
    flex-shrink: 0;
    color: var(--texto-2);
  }
  .opcion__menu {
    flex: 1 1 180px;
    min-width: 0;
    max-width: 240px;
  }
  /* Apilada (tarjeta de la página): el rótulo arriba y el menú a lo ancho. */
  .opcion.apilada {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
    min-height: 0;
    padding: 0;
  }
  .opcion.apilada + .opcion.apilada {
    border-top: 0;
  }
  .apilada .opcion__rotulo {
    font-size: 12.5px;
    line-height: 17px;
    font-weight: 600;
    color: var(--texto-2);
    gap: 8px;
  }
  .apilada .opcion__menu {
    flex: none;
    max-width: none;
  }
  .silueta-menu {
    display: block;
    height: 38px;
    border-radius: var(--radio-sm);
    background: var(--superficie-elevada);
    animation: respirar 1.4s ease-in-out infinite;
  }
  @keyframes respirar {
    50% {
      opacity: 0.55;
    }
  }

  /* ---------- Panel lateral: grupos de filas ---------- */
  .grupo {
    padding: 0 14px;
  }
  .grupo.deslizadores {
    padding: 6px 14px 6px 10px;
  }
  .grupo.deslizadores .control + .control {
    border-top: 1px solid var(--borde);
  }

  /* ---------- Página: las dos tarjetas ---------- */
  .panel {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 18px;
    border-radius: var(--radio-lg);
  }
  /* Si la tarjeta se estira, los controles se reparten el alto (sin hueco abajo). */
  .controles,
  .menus {
    flex: 1 1 auto;
    display: flex;
    flex-direction: column;
    justify-content: space-evenly;
    gap: 16px;
  }
  .menus {
    justify-content: center;
  }
  .controles .control {
    min-height: 0;
  }
  .en-uso {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 10px 0 8px;
    border-radius: 99px;
    background: color-mix(in srgb, var(--exito) 12%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--exito) 26%, transparent);
    color: var(--exito);
    font-size: 12px;
    font-weight: 600;
  }
  .usar {
    flex-shrink: 0;
    min-height: 30px;
    padding: 0 12px;
    border: 0;
    border-radius: 99px;
    background: var(--boton-fondo);
    color: var(--boton-texto);
    font-size: 12.5px;
    font-weight: 650;
    transition: background-color var(--dur-color) ease;
  }
  .usar:hover {
    background: var(--boton-fondo-hover);
  }
  .usar:active {
    transform: scale(0.98);
  }
  .vacio {
    flex: 1 1 auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 8px 12px 4px;
    text-align: center;
  }
  .vacio p {
    margin: 0;
    max-width: 280px;
    font: var(--letra-pequena);
    color: var(--texto-2);
  }
  .ficha-icono.grande {
    width: 44px;
    height: 44px;
    border-radius: 14px;
  }
</style>
