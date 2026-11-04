<!--
  Audio: un mezclador. A la izquierda, tiras verticales para la salida en uso, el micrófono en uso y (en Mac)
  los avisos del sistema, cada una con su foto, su % en grande, la barra vertical y el silencio al pie.
  A la derecha, los aparatos de sonido como tarjetas: tocar una la vuelve la salida o la entrada del sistema.
  Todos los aparatos de sonido, no solo Bluetooth. La lista se lee cada 2 s mientras se ve; una lectura que
  empezó antes de un cambio (o justo después) no pisa lo que acabas de hacer. El volumen de avisos se lee al
  entrar y al volver a la ventana (es un proceso del sistema: no se pregunta cada 2 s).
  Nada pide permiso de micrófono: no hay medidores en vivo.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    abrirAjustesSonido,
    ajustarDispositivoAudio,
    leerVolumenAvisos,
    listarDispositivosAudio,
    ponerVolumenAvisos,
    probarSonido,
    usarDispositivoAudio,
  } from '$lib/api';
  import { esMac } from '$lib/api-sistema';
  import { fotoDeSonido } from '$lib/catalogo';
  import { enviadorDelUltimo } from '$lib/enviador';
  import { tooltip } from '$lib/tooltip';
  import type { DispositivoAudio, TransporteAudio } from '$lib/tipos';
  import Icono, { type Nombre } from '$lib/componentes/Icono.svelte';
  import CanalAudio from '$lib/componentes/CanalAudio.svelte';

  let { onerror }: { onerror: (texto: string) => void } = $props();

  let lista = $state<DispositivoAudio[] | null>(null);
  let fallando = $state(false);
  // Una lectura que empezó antes de un cambio (o hasta 900 ms después) ya es vieja: no se usa.
  let ultimoCambio = 0;
  const RESPIRO = 900;

  const orden = (a: DispositivoAudio, b: DispositivoAudio) =>
    Number(b.transporte === 'integrado') - Number(a.transporte === 'integrado') || a.nombre.localeCompare(b.nombre, 'es');

  const salidas = $derived((lista ?? []).filter((d) => d.salida).sort(orden));
  const entradas = $derived((lista ?? []).filter((d) => d.entrada).sort(orden));
  const salida = $derived(salidas.find((d) => d.salidaEnUso));
  const entrada = $derived(entradas.find((d) => d.entradaEnUso));
  const microIntegrado = $derived(entradas.find((d) => d.transporte === 'integrado' && !d.entradaEnUso));

  const transportes: Record<TransporteAudio, string | undefined> = {
    integrado: 'Integrado',
    bluetooth: 'Bluetooth',
    usb: 'USB',
    hdmi: 'HDMI',
    displayport: 'DisplayPort',
    airplay: 'AirPlay',
    virtual: 'Virtual',
    otro: undefined,
  };

  function icono(d: DispositivoAudio, deEntrada: boolean): Nombre {
    if (d.transporte === 'bluetooth') return 'audio';
    if (deEntrada) return 'microfono';
    if (d.transporte === 'hdmi' || d.transporte === 'displayport' || d.transporte === 'airplay') return 'pantallas';
    if (d.transporte === 'integrado' && esMac) return 'portatil';
    return 'volumen';
  }

  const foto = (d: DispositivoAudio) => fotoDeSonido(d.nombre, d.transporte);

  /** "Bluetooth · 72 %", "Integrado · Silenciado". */
  function resumen(d: DispositivoAudio, deEntrada: boolean): string {
    const nivel = deEntrada ? d.volumenEntrada : d.volumen;
    const mudo = deEntrada ? d.silenciadoEntrada : d.silenciado;
    const partes = [transportes[d.transporte]];
    if (mudo) partes.push('Silenciado');
    else if (nivel !== null) partes.push(`${nivel}\u00a0%`);
    return partes.filter(Boolean).join(' · ');
  }

  // --- Lectura ---
  async function leer() {
    const inicio = Date.now();
    try {
      const nueva = await listarDispositivosAudio();
      fallando = false;
      if (ultimoCambio > inicio - RESPIRO) return;
      lista = nueva;
    } catch (e) {
      // Se avisa una sola vez, no en cada vuelta.
      if (!fallando) onerror(String(e));
      fallando = true;
    }
  }

  // --- Elegir la salida o la entrada ---
  async function usar(uid: string, deEntrada: boolean) {
    ultimoCambio = Date.now();
    for (const d of lista ?? []) {
      if (deEntrada && d.entrada) d.entradaEnUso = d.uid === uid;
      if (!deEntrada && d.salida) d.salidaEnUso = d.uid === uid;
    }
    try {
      lista = await usarDispositivoAudio(uid, deEntrada);
      ultimoCambio = Date.now();
    } catch (e) {
      onerror(String(e));
      leer();
    }
  }

  // --- Silencio ---
  async function silenciar(d: DispositivoAudio, deEntrada: boolean) {
    ultimoCambio = Date.now();
    const valor = !(deEntrada ? d.silenciadoEntrada : d.silenciado);
    if (deEntrada) d.silenciadoEntrada = valor;
    else d.silenciado = valor;
    try {
      lista = await ajustarDispositivoAudio(d.uid, { accion: deEntrada ? 'silenciarMicrofono' : 'silenciar', valor });
      ultimoCambio = Date.now();
    } catch (e) {
      onerror(String(e));
      leer();
    }
  }

  const puedeSalida = $derived(!!salida && salida.silenciado !== null);
  const puedeEntrada = $derived(!!entrada && entrada.silenciadoEntrada !== null);
  const todoSilenciado = $derived(
    (puedeSalida || puedeEntrada) &&
      (!puedeSalida || salida?.silenciado === true) &&
      (!puedeEntrada || entrada?.silenciadoEntrada === true),
  );

  /** Silencia (o devuelve) la salida y el micrófono en uso, uno detrás del otro. */
  async function silenciarTodo() {
    const valor = !todoSilenciado;
    const s = puedeSalida ? salida : undefined;
    const m = puedeEntrada ? entrada : undefined;
    ultimoCambio = Date.now();
    if (s) s.silenciado = valor;
    if (m) m.silenciadoEntrada = valor;
    try {
      let nueva: DispositivoAudio[] | null = null;
      if (s) nueva = await ajustarDispositivoAudio(s.uid, { accion: 'silenciar', valor });
      if (m) nueva = await ajustarDispositivoAudio(m.uid, { accion: 'silenciarMicrofono', valor });
      if (nueva) lista = nueva;
      ultimoCambio = Date.now();
    } catch (e) {
      onerror(String(e));
      leer();
    }
  }

  // --- Volúmenes: siempre el último valor, sin fila (el sonido sigue al dedo) ---
  const fallo = (e: unknown) => onerror(String(e));
  const enviarVolumen = enviadorDelUltimo(
    ({ uid, valor }: { uid: string; valor: number }) => ajustarDispositivoAudio(uid, { accion: 'volumen', valor }),
    fallo,
  );
  const enviarMicrofono = enviadorDelUltimo(
    ({ uid, valor }: { uid: string; valor: number }) => ajustarDispositivoAudio(uid, { accion: 'volumenMicrofono', valor }),
    fallo,
  );

  function volumen(d: DispositivoAudio, valor: number) {
    ultimoCambio = Date.now();
    d.volumen = valor;
    enviarVolumen({ uid: d.uid, valor });
  }

  function volumenMicrofono(d: DispositivoAudio, valor: number) {
    ultimoCambio = Date.now();
    d.volumenEntrada = valor;
    enviarMicrofono({ uid: d.uid, valor });
  }

  // --- Avisos y efectos (solo Mac) ---
  let avisos = $state<number | null>(null);
  let ultimoAviso = 0;
  const enviarAvisos = enviadorDelUltimo((v: number) => ponerVolumenAvisos(v), fallo);

  function cambiarAvisos(v: number) {
    ultimoAviso = Date.now();
    avisos = v;
    enviarAvisos(v);
  }

  async function leerAvisos() {
    if (!esMac) return;
    const inicio = Date.now();
    try {
      const v = await leerVolumenAvisos();
      if (v !== null && ultimoAviso < inicio - RESPIRO) avisos = v;
    } catch {
      // Sin volumen de avisos, la tira no aparece.
    }
  }

  // --- Probar sonido y ajustes del sistema ---
  let probando = $state(false);
  async function probar() {
    if (probando) return;
    probando = true;
    const inicio = Date.now();
    try {
      await probarSonido();
    } catch (e) {
      onerror(String(e));
    }
    // Se ve "Sonando…" al menos medio segundo, aunque el sonido sea más corto.
    setTimeout(() => (probando = false), Math.max(0, 500 - (Date.now() - inicio)));
  }

  async function ajustesSistema() {
    try {
      await abrirAjustesSonido();
    } catch (e) {
      onerror(String(e));
    }
  }

  const canales = $derived(Number(!!salida) + Number(!!entrada) + Number(avisos !== null && !!salida));

  onMount(() => {
    leer();
    leerAvisos();
    const intervalo = setInterval(() => document.visibilityState === 'visible' && leer(), 2000);
    const alVolver = () => {
      if (document.visibilityState !== 'visible') return;
      leer();
      leerAvisos();
    };
    document.addEventListener('visibilitychange', alVolver);
    window.addEventListener('focus', alVolver);
    return () => {
      clearInterval(intervalo);
      document.removeEventListener('visibilitychange', alVolver);
      window.removeEventListener('focus', alVolver);
    };
  });
</script>

{#snippet pieAvisos()}
  <p class="pie-nota">Son una parte del volumen general</p>
{/snippet}

{#snippet tarjeta(d: DispositivoAudio, deEntrada: boolean)}
  {@const enUso = deEntrada ? d.entradaEnUso : d.salidaEnUso}
  {@const f = foto(d)}
  {@const r = resumen(d, deEntrada)}
  <button
    class="aparato"
    class:elegido={enUso}
    type="button"
    aria-pressed={enUso}
    aria-label="{deEntrada ? 'Escuchar por' : 'Sacar el sonido por'} {d.nombre}"
    onclick={() => !enUso && usar(d.uid, deEntrada)}
  >
    <span class="aparato__foto" aria-hidden="true">
      {#if f}
        <img src={f} alt="" draggable="false" decoding="async" />
      {:else}
        <Icono nombre={icono(d, deEntrada)} tamano={20} />
      {/if}
    </span>
    <span class="aparato__textos">
      <span class="aparato__nombre">{d.nombre}</span>
      <span class="aparato__detalle">{enUso ? (r ? `En uso · ${r}` : 'En uso') : r || 'Disponible'}</span>
    </span>
    <span class="aparato__marca" aria-hidden="true">
      {#if enUso}<Icono nombre="check-circulo" tamano={20} />{:else}<span class="aro"></span>{/if}
    </span>
  </button>
{/snippet}

<div class="vista">
  <header class="cabecera">
    <h1>Audio</h1>
    <p>Cuánto suena, por dónde sale y qué micrófono escucha.</p>
  </header>

  {#if lista === null}
    {#if fallando}
      <section class="panel-vacio vidrio">
        <span class="panel-vacio__icono"><Icono nombre="alerta" tamano={20} /></span>
        <p>No se pudo leer el sonido del sistema. Nexo lo vuelve a intentar solo.</p>
      </section>
    {:else}
      <div class="cuerpo" aria-busy="true">
        <span class="solo-lector">Cargando el sonido…</span>
        <div class="silueta vidrio"></div>
        <div class="silueta vidrio"></div>
      </div>
    {/if}
  {:else}
    <div class="cuerpo">
      <!-- Mezclador -->
      <section class="mezclador vidrio" aria-labelledby="audio-mezclador">
        <div class="t-cabeza">
          <span class="ficha-icono" style:--tono="var(--celeste)"><Icono nombre="onda" tamano={19} /></span>
          <div class="t-textos">
            <h2 class="t-titulo" id="audio-mezclador">Mezclador</h2>
            <p class="t-sub">Arrastra las barras o usa las flechas del teclado.</p>
          </div>
          <button
            class="pildora"
            class:activa={todoSilenciado}
            type="button"
            aria-pressed={todoSilenciado}
            disabled={!puedeSalida && !puedeEntrada}
            use:tooltip={todoSilenciado ? 'Devuelve el sonido y el micrófono' : 'Silencia la salida y el micrófono en uso'}
            onclick={silenciarTodo}
          >
            <Icono nombre={todoSilenciado ? 'silencio' : 'volumen'} tamano={16} />
            {todoSilenciado ? 'Quitar silencio' : 'Silenciar todo'}
          </button>
        </div>

        {#if canales === 0}
          <p class="mezclador__vacio">No hay ninguna salida ni micrófono en uso.</p>
        {:else}
          <div class="canales" style:--canales={canales}>
            {#if salida}
              {@const s = salida}
              <CanalAudio
                rotulo="Salida"
                nombre={s.nombre}
                detalle={transportes[s.transporte]}
                foto={foto(s)}
                icono={icono(s, false)}
                valor={s.volumen}
                silenciado={s.silenciado}
                textoSilenciar="Silenciar la salida"
                nota="Este aparato no deja cambiar el volumen desde aquí. Usa sus botones."
                onvalor={(v) => volumen(s, v)}
                onsilenciar={() => silenciar(s, false)}
              />
            {/if}
            {#if entrada}
              {@const m = entrada}
              <CanalAudio
                rotulo="Entrada"
                nombre={m.nombre}
                detalle={transportes[m.transporte]}
                foto={foto(m)}
                icono={icono(m, true)}
                valor={m.volumenEntrada}
                silenciado={m.silenciadoEntrada}
                textoSilenciar="Silenciar el micrófono"
                iconoSilencio="microfono"
                nota="Este micrófono no deja cambiar su nivel desde aquí."
                onvalor={(v) => volumenMicrofono(m, v)}
                onsilenciar={() => silenciar(m, true)}
              />
            {/if}
            {#if avisos !== null && salida}
              <CanalAudio
                rotulo="Avisos"
                nombre="Sonidos del sistema"
                detalle="Alertas y efectos"
                icono="campana"
                valor={avisos}
                onvalor={cambiarAvisos}
                pie={pieAvisos}
              />
            {/if}
          </div>
        {/if}
      </section>

      <!-- Aparatos de sonido -->
      <section class="aparatos vidrio" aria-label="Aparatos de sonido">
        <div class="grupo">
          <div class="t-cabeza">
            <span class="ficha-icono chica" style:--tono="var(--celeste)"><Icono nombre="volumen" tamano={17} /></span>
            <div class="t-textos">
              <h2 class="t-titulo">Salidas</h2>
              <p class="t-sub">Toca una para que el sonido salga por ahí.</p>
            </div>
            <span class="cuenta">{salidas.length}</span>
          </div>
          {#if salidas.length}
            <div class="rejilla">
              {#each salidas as d (d.uid)}{@render tarjeta(d, false)}{/each}
            </div>
          {:else}
            <p class="vacia">No hay ninguna salida de sonido conectada.</p>
          {/if}
        </div>

        <div class="grupo">
          <div class="t-cabeza">
            <span class="ficha-icono chica" style:--tono="var(--celeste)"><Icono nombre="microfono" tamano={17} /></span>
            <div class="t-textos">
              <h2 class="t-titulo">Entradas</h2>
              <p class="t-sub">El micrófono que escuchan tus apps.</p>
            </div>
            <span class="cuenta">{entradas.length}</span>
          </div>
          {#if entradas.length}
            <div class="rejilla">
              {#each entradas as d (d.uid)}{@render tarjeta(d, true)}{/each}
            </div>
          {:else}
            <p class="vacia">No hay ningún micrófono conectado.</p>
          {/if}
        </div>

        {#if entrada?.transporte === 'bluetooth'}
          {@const bt = entrada}
          <div class="nota">
            <span class="nota__icono"><Icono nombre="info" tamano={16} /></span>
            <div class="nota__textos">
              <p>
                Con el micrófono de <b>{bt.nombre}</b> en uso, el sonido de esos audífonos baja de calidad: por Bluetooth no van
                las dos cosas a la vez con buena calidad.
              </p>
              {#if microIntegrado}
                {@const mi = microIntegrado}
                <button class="boton-texto" type="button" onclick={() => usar(mi.uid, true)}>Usar {mi.nombre}</button>
              {/if}
            </div>
          </div>
        {/if}

        <div class="pie-aparatos">
          <div class="barra-acciones">
            <button type="button" disabled={probando || !salida} aria-busy={probando} onclick={probar}>
              {#if probando}<span class="girador" aria-hidden="true"></span>{:else}<Icono nombre="onda" tamano={19} />{/if}
              {probando ? 'Sonando…' : 'Probar sonido'}
            </button>
            <button type="button" onclick={ajustesSistema}>
              <Icono nombre="engranaje" tamano={19} />
              {esMac ? 'Ajustes de Sonido' : 'Configuración de sonido'}
            </button>
          </div>
        </div>
      </section>
    </div>
  {/if}
</div>

<style>
  .vista {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
    min-height: 100%;
    container-type: inline-size;
  }
  .cabecera h1 {
    margin: 0;
    font: var(--letra-titulo);
    letter-spacing: -0.01em;
  }
  .cabecera p {
    margin: 4px 0 0;
    color: var(--texto-2);
  }

  /* Mezclador y aparatos: uno debajo del otro en ventanas angostas; lado a lado (y a todo el alto) en las anchas. */
  .cuerpo {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
    min-height: 0;
  }
  @container (min-width: 760px) {
    .cuerpo {
      grid-template-columns: minmax(400px, 1.05fr) minmax(0, 1fr);
    }
  }

  /* ---------- Mezclador ---------- */
  .mezclador {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
    padding: 18px;
    border-radius: var(--radio-lg);
  }
  .mezclador .t-cabeza {
    flex-wrap: wrap;
    padding: 0 2px;
  }
  /* Cada tira usa estas 5 filas (CanalAudio, con subgrid): rótulo, aparato, cifra, barra y pie. */
  .canales {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(var(--canales), minmax(0, 1fr));
    grid-template-rows: auto auto auto minmax(200px, 1fr) auto;
    gap: 12px 10px;
    min-height: 470px;
  }
  .mezclador__vacio {
    margin: 0;
    padding: 24px 4px;
    color: var(--texto-2);
  }
  .pie-nota {
    display: grid;
    place-items: center;
    margin: 0;
    font-size: 11.5px;
    line-height: 15px;
    color: var(--texto-3);
    text-align: center;
  }

  /* Silenciar todo: píldora; encendida en rojo suave, como el silencio de cada tira. */
  .pildora {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    flex-shrink: 0;
    min-height: 34px;
    padding: 0 14px 0 12px;
    border: 0;
    border-radius: var(--radio-pildora);
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font-size: 12.5px;
    font-weight: 600;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .pildora:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .pildora:active:not(:disabled) {
    transform: scale(0.98);
  }
  .pildora:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .pildora.activa {
    color: var(--error);
    background: color-mix(in srgb, var(--error) 12%, var(--superficie));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--error) 35%, transparent);
  }

  /* ---------- Aparatos ---------- */
  .aparatos {
    display: flex;
    flex-direction: column;
    gap: 22px;
    min-width: 0;
    padding: 18px 20px 20px;
    border-radius: var(--radio-lg);
  }
  .grupo {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .cuenta {
    display: grid;
    place-items: center;
    min-width: 26px;
    height: 22px;
    padding: 0 7px;
    border-radius: var(--radio-pildora);
    background: var(--superficie-elevada);
    color: var(--texto-2);
    font-size: 12px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .rejilla {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 10px;
  }
  .aparato {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 10px 12px 10px 10px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-md);
    background: var(--superficie-inserto);
    color: var(--texto);
    text-align: left;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .aparato:not(.elegido):hover {
    background: var(--superficie-hover);
  }
  .aparato:active {
    transform: scale(0.98);
  }
  .aparato.elegido {
    cursor: default;
  }
  .aparato__foto {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 46px;
    height: 46px;
    border-radius: 13px;
    background: var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-2);
  }
  .elegido .aparato__foto {
    background:
      radial-gradient(closest-side at 50% 60%, var(--aurora-1), transparent),
      var(--superficie-hundida);
    color: var(--primario-texto);
  }
  .aparato__foto img {
    width: 38px;
    height: 38px;
    object-fit: contain;
    filter: drop-shadow(0 3px 4px var(--sombra-apoyo));
  }
  .aparato__textos {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 1px;
  }
  .aparato__nombre {
    font-size: 13.5px;
    line-height: 18px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .aparato__detalle {
    font-size: 12px;
    line-height: 16px;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .elegido .aparato__detalle {
    color: color-mix(in srgb, var(--primario-texto) 80%, var(--texto-2));
  }
  .aparato__marca {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 22px;
    color: var(--primario-texto);
  }
  .aro {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1.5px var(--borde-fuerte);
  }
  .vacia {
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }

  .nota {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radio-md);
    background: color-mix(in srgb, var(--info) 8%, var(--superficie-inserto));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--info) 22%, transparent);
  }
  .nota__icono {
    display: grid;
    flex-shrink: 0;
    margin-top: 1px;
    color: var(--info);
  }
  .nota__textos {
    display: grid;
    justify-items: start;
    gap: 6px;
  }
  .nota p {
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .nota b {
    color: var(--texto);
    font-weight: 600;
  }
  .boton-texto {
    margin-left: -8px;
    padding: 4px 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--primario-texto);
    font-size: 12.5px;
    font-weight: 600;
    text-align: left;
    transition: background-color var(--dur-color) ease;
  }
  .boton-texto:hover {
    background: var(--superficie-hover);
  }

  /* Al pie del panel: probar el sonido y los ajustes del sistema. */
  .pie-aparatos {
    margin-top: auto;
  }
  .pie-aparatos .barra-acciones > button :global(svg) {
    color: var(--celeste);
  }

  /* ---------- Vacío y siluetas ---------- */
  .panel-vacio {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 18px 20px;
    border-radius: var(--radio-lg);
  }
  .panel-vacio__icono {
    display: grid;
    color: var(--aviso);
  }
  .panel-vacio p {
    margin: 0;
    color: var(--texto-2);
  }
  .silueta {
    min-height: 420px;
    border-radius: var(--radio-lg);
  }
</style>
