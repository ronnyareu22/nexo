<!--
  Controles de sonido de un aparato, los mismos en el panel lateral y en la página del aparato:
  - volumen de salida (el botón redondo silencia) y micrófono del aparato, solo si está conectado;
  - los menús de salida y entrada del sistema (todos los aparatos de sonido, como en la página Audio).
  `variante="panel"`: filas sueltas y los menús en una tarjeta agrupada (panel lateral de Dispositivos).
  `variante="tarjeta"`: todo dentro de la tarjeta "Controles de audio" (pestaña General de la página).
  Se lee cada 2 s mientras se ve, sin pisar un deslizador que se mueve. Siempre se manda el último valor.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { ajustarSonido, leerSonido, listarDispositivosAudio, usarDispositivoAudio } from '$lib/api';
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

  const rotuloSalida = $derived(variante === 'tarjeta' ? 'Dispositivo de salida' : 'Salida de audio');
</script>

{#snippet deslizadores()}
  {#if conVolumen && propio}
    {@const v = propio.volumen ?? 0}
    <div class="control">
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
      <div class="pista">
        <span class="etiqueta" aria-hidden="true">Volumen de salida</span>
        <Deslizador
          valor={v}
          color="celeste"
          etiqueta="Volumen de salida"
          apagado={propio.silenciado ?? false}
          oncambio={volumen}
          ontocando={alTocar}
        />
      </div>
      <span class="valor" class:apagado={propio.silenciado}>{v}%</span>
    </div>
  {/if}
  {#if conMicrofono && propio}
    {@const m = propio.volumenMicrofono ?? 0}
    <div class="control">
      <span class="redondo quieto" aria-hidden="true"><Icono nombre="microfono" tamano={18} /></span>
      <div class="pista">
        <span class="etiqueta" aria-hidden="true">Micrófono</span>
        <Deslizador valor={m} color="celeste" etiqueta="Micrófono" oncambio={microfono} ontocando={alTocar} />
      </div>
      <span class="valor">{m}%</span>
    </div>
  {/if}
{/snippet}

{#snippet menus(conIconos: boolean)}
  <div class="opcion">
    <span class="opcion__rotulo" id="{id}-salida">
      {#if conIconos}<Icono nombre="volumen" tamano={18} />{/if}
      {rotuloSalida}
    </span>
    <div class="opcion__menu">
      <MenuDesplegable
        opciones={opcionesSalida}
        valor={salidas.find((x) => x.salidaEnUso)?.uid ?? ''}
        etiqueta={rotuloSalida}
        oncambio={(uid) => usar(uid, false)}
      />
    </div>
  </div>
  <div class="opcion">
    <span class="opcion__rotulo" id="{id}-entrada">
      {#if conIconos}<Icono nombre="microfono" tamano={18} />{/if}
      Entrada de audio
    </span>
    <div class="opcion__menu">
      <MenuDesplegable
        opciones={opcionesEntrada}
        valor={entradas.find((x) => x.entradaEnUso)?.uid ?? ''}
        etiqueta="Entrada de audio"
        oncambio={(uid) => usar(uid, true)}
      />
    </div>
  </div>
{/snippet}

{#if variante === 'panel'}
  {@render deslizadores()}
  {#if lista}
    <div class="grupo">{@render menus(true)}</div>
  {/if}
{:else}
  <section class="panel vidrio tarjeta" aria-labelledby="{id}-titulo">
    <h2 id="{id}-titulo">Controles de audio</h2>
    {#if !d.conectado}
      <p class="nota">Conéctalo para cambiar su volumen y su micrófono.</p>
    {:else}
      {@render deslizadores()}
    {/if}
    {#if lista}
      <div class="menus" class:separados={conVolumen || conMicrofono || !d.conectado}>{@render menus(false)}</div>
    {/if}
  </section>
{/if}

<style>
  /* Fila de un deslizador: botón o ícono redondo, el rótulo sobre la pista y el valor a la derecha. */
  .control {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    align-items: center;
    column-gap: 12px;
  }
  .pista {
    display: grid;
    min-width: 0;
  }
  .etiqueta {
    font-size: 12.5px;
    line-height: 18px;
    font-weight: 500;
    color: var(--texto-2);
  }
  .redondo {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--celeste);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  button.redondo:hover {
    background: var(--superficie-hover);
  }
  button.redondo:active {
    transform: scale(0.98);
  }
  .redondo.silenciado {
    color: var(--texto-3);
  }
  .valor {
    min-width: 42px;
    align-self: end;
    padding-bottom: 3px;
    text-align: right;
    font-size: 14px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .valor.apagado {
    color: var(--texto-3);
  }

  /* Menús de salida y entrada: el rótulo a la izquierda y el menú a la derecha; si no cabe, el menú baja. */
  .opcion {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 12px;
    padding: 10px 0;
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
    flex: 1 1 200px;
    min-width: 0;
    max-width: 300px;
  }

  /* Panel lateral: los menús van en una tarjeta agrupada. */
  .grupo {
    padding: 2px 14px;
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }

  /* Página: la tarjeta "Controles de audio". */
  .tarjeta {
    display: grid;
    gap: 14px;
    padding: 18px 20px 8px;
    border-radius: var(--radio-lg);
  }
  .tarjeta h2 {
    margin: 0;
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
  }
  .nota {
    margin: 0;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto-2);
  }
  .menus .opcion:last-child {
    padding-bottom: 2px;
  }
  .menus.separados {
    padding-top: 4px;
    border-top: 1px solid var(--borde);
  }
</style>
