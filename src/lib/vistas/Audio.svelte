<!-- Audio: por dónde sale el sonido del sistema y qué micrófono se usa, con sus volúmenes.
     Todos los aparatos de sonido (no solo Bluetooth). Se lee cada 2 s mientras se ve, sin pisar un deslizador que se mueve. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { ajustarDispositivoAudio, listarDispositivosAudio, usarDispositivoAudio } from '$lib/api';
  import { enviadorDelUltimo } from '$lib/enviador';
  import { tooltip } from '$lib/tooltip';
  import type { DispositivoAudio, TransporteAudio } from '$lib/tipos';
  import Icono, { type Nombre } from '$lib/componentes/Icono.svelte';
  import Deslizador from '$lib/componentes/Deslizador.svelte';
  import MenuDesplegable from '$lib/componentes/MenuDesplegable.svelte';

  let { onerror }: { onerror: (texto: string) => void } = $props();

  let lista = $state<DispositivoAudio[] | null>(null);
  let fallando = $state(false);
  // Mientras se arrastra un deslizador, o justo después de un cambio, la lectura periódica no pisa los valores.
  let tocando = false;
  let ultimoCambio = 0;
  const RESPIRO = 800;

  const porNombre = (a: DispositivoAudio, b: DispositivoAudio) =>
    Number(b.transporte === 'integrado') - Number(a.transporte === 'integrado') || a.nombre.localeCompare(b.nombre, 'es');

  const salidas = $derived((lista ?? []).filter((d) => d.salida).sort(porNombre));
  const entradas = $derived((lista ?? []).filter((d) => d.entrada).sort(porNombre));
  const salida = $derived(salidas.find((d) => d.salidaEnUso));
  const entrada = $derived(entradas.find((d) => d.entradaEnUso));

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

  function icono(d: DispositivoAudio, deEntrada: boolean): Nombre {
    switch (d.transporte) {
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
    salidas.map((d) => ({ valor: d.uid, nombre: d.nombre, detalle: detalle[d.transporte], icono: icono(d, false) })),
  );
  const opcionesEntrada = $derived(
    entradas.map((d) => ({ valor: d.uid, nombre: d.nombre, detalle: detalle[d.transporte], icono: icono(d, true) })),
  );

  async function leer() {
    const inicio = Date.now();
    try {
      const nueva = await listarDispositivosAudio();
      fallando = false;
      // Si algo cambió durante la lectura (o hace muy poco), esta lectura ya es vieja.
      if (tocando || ultimoCambio > inicio - RESPIRO) return;
      lista = nueva;
    } catch (e) {
      // Se avisa una sola vez, no en cada vuelta.
      if (!fallando) onerror(String(e));
      fallando = true;
    }
  }

  function aplicar(nueva: DispositivoAudio[]) {
    if (!tocando) lista = nueva;
  }

  async function usar(uid: string, deEntrada: boolean) {
    ultimoCambio = Date.now();
    for (const d of lista ?? []) {
      if (deEntrada && d.entrada) d.entradaEnUso = d.uid === uid;
      if (!deEntrada && d.salida) d.salidaEnUso = d.uid === uid;
    }
    try {
      aplicar(await usarDispositivoAudio(uid, deEntrada));
    } catch (e) {
      onerror(String(e));
      leer();
    }
  }

  async function silenciar(d: DispositivoAudio) {
    ultimoCambio = Date.now();
    const valor = !d.silenciado;
    d.silenciado = valor;
    try {
      aplicar(await ajustarDispositivoAudio(d.uid, { accion: 'silenciar', valor }));
    } catch (e) {
      onerror(String(e));
      leer();
    }
  }

  // Siempre el último valor, sin fila: el sonido sigue al dedo.
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

  function alTocar(t: boolean) {
    tocando = t;
    ultimoCambio = Date.now();
  }

  onMount(() => {
    leer();
    const intervalo = setInterval(() => document.visibilityState === 'visible' && leer(), 2000);
    return () => clearInterval(intervalo);
  });
</script>

<div class="vista">
  <header class="cabecera">
    <h1>Audio</h1>
    <p>Por dónde sale el sonido y qué micrófono se usa.</p>
  </header>

  {#if lista === null}
    {#if fallando}
      <section class="panel vidrio vacio">
        <span class="vacio__icono"><Icono nombre="alerta" tamano={20} /></span>
        <p>No se pudo leer el sonido del sistema. Nexo lo vuelve a intentar solo.</p>
      </section>
    {:else}
      <div class="paneles" aria-busy="true">
        <span class="solo-lector">Cargando el sonido…</span>
        <div class="silueta vidrio"></div>
        <div class="silueta vidrio"></div>
      </div>
    {/if}
  {:else}
    <div class="paneles">
      <!-- Salida -->
      <section class="panel vidrio" aria-labelledby="audio-salida">
        <h2 class="rotulo" id="audio-salida">Salida</h2>
        {#if salidas.length === 0}
          <p class="ayuda vacia">No hay ninguna salida de sonido conectada.</p>
        {:else}
          <div class="fila">
            <span class="fila__rotulo">Sale por</span>
            <div class="fila__menu">
              <MenuDesplegable
                opciones={opcionesSalida}
                valor={salida?.uid ?? ''}
                etiqueta="Sale por"
                oncambio={(uid) => usar(uid, false)}
              />
            </div>
          </div>

          {#if salida}
            {@const s = salida}
            {#if s.volumen !== null}
              <div class="fila control">
                {#if s.silenciado !== null}
                  <button
                    class="redondo"
                    class:silenciado={s.silenciado}
                    type="button"
                    aria-pressed={s.silenciado}
                    aria-label={s.silenciado ? 'Quitar silencio' : 'Silenciar'}
                    use:tooltip={s.silenciado ? 'Quitar silencio' : 'Silenciar'}
                    onclick={() => silenciar(s)}
                  >
                    <Icono nombre={s.silenciado || s.volumen === 0 ? 'silencio' : 'volumen'} tamano={18} />
                  </button>
                {:else}
                  <span class="redondo quieto"><Icono nombre="volumen" tamano={18} /></span>
                {/if}
                <Deslizador
                  valor={s.volumen}
                  color="celeste"
                  etiqueta="Volumen de {s.nombre}"
                  apagado={s.silenciado ?? false}
                  oncambio={(v) => volumen(s, v)}
                  ontocando={alTocar}
                />
                <span class="valor" class:apagado={s.silenciado}>{s.volumen} %</span>
              </div>
            {:else}
              <p class="ayuda nota">Este aparato no permite cambiar el volumen desde aquí.</p>
            {/if}
          {/if}
        {/if}
      </section>

      <!-- Entrada -->
      <section class="panel vidrio" aria-labelledby="audio-entrada">
        <h2 class="rotulo" id="audio-entrada">Entrada</h2>
        {#if entradas.length === 0}
          <p class="ayuda vacia">No hay ningún micrófono conectado.</p>
        {:else}
          <div class="fila">
            <span class="fila__rotulo">Escucha por</span>
            <div class="fila__menu">
              <MenuDesplegable
                opciones={opcionesEntrada}
                valor={entrada?.uid ?? ''}
                etiqueta="Escucha por"
                oncambio={(uid) => usar(uid, true)}
              />
            </div>
          </div>

          {#if entrada}
            {@const m = entrada}
            {#if m.volumenEntrada !== null}
              <div class="fila control">
                <span class="redondo quieto"><Icono nombre="microfono" tamano={18} /></span>
                <Deslizador
                  valor={m.volumenEntrada}
                  color="celeste"
                  etiqueta="Nivel de entrada de {m.nombre}"
                  oncambio={(v) => volumenMicrofono(m, v)}
                  ontocando={alTocar}
                />
                <span class="valor">{m.volumenEntrada} %</span>
              </div>
            {/if}
            {#if m.transporte === 'bluetooth'}
              <div class="aviso-calidad">
                <span class="aviso-calidad__icono"><Icono nombre="info" tamano={16} /></span>
                <p>Con un micrófono Bluetooth, el sonido de esos audífonos baja de calidad mientras se usa.</p>
              </div>
            {/if}
          {/if}
        {/if}
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

  .paneles {
    display: grid;
    gap: 16px;
    align-items: start;
  }
  /* Con la ventana ancha, salida y entrada lado a lado para no dejar espacio muerto. */
  @media (min-width: 1360px) {
    .paneles {
      grid-template-columns: 1fr 1fr;
    }
  }

  .panel {
    padding: 14px 20px 6px;
    border-radius: var(--radio-lg);
  }
  .rotulo {
    margin: 0 0 4px;
  }

  .fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 56px;
    padding: 10px 0;
  }
  .fila + .fila,
  .fila + .nota,
  .fila + .aviso-calidad {
    border-top: 1px solid var(--borde);
  }
  .fila__rotulo {
    flex-shrink: 0;
    font-weight: 500;
  }
  .fila__menu {
    width: min(320px, 66%);
  }

  .control {
    display: grid;
    grid-template-columns: 36px 1fr 48px;
    gap: 12px;
  }
  .redondo {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
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
  .redondo.silenciado {
    color: var(--texto-3);
  }
  .redondo.quieto {
    color: var(--texto-3);
  }
  .valor {
    text-align: right;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .valor.apagado {
    color: var(--texto-3);
  }

  .ayuda {
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .nota {
    padding: 14px 0;
  }
  .vacia {
    padding: 8px 0 16px;
  }

  .aviso-calidad {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 0 14px;
  }
  .aviso-calidad__icono {
    display: grid;
    flex-shrink: 0;
    width: 36px;
    place-items: center;
    margin-top: 1px;
    color: var(--texto-3);
  }
  .aviso-calidad p {
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }

  .vacio {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 18px 20px;
  }
  .vacio__icono {
    display: grid;
    color: var(--aviso);
  }
  .vacio p {
    margin: 0;
    color: var(--texto-2);
  }

  /* Siluetas mientras llega la lista: quietas. */
  .silueta {
    height: 168px;
    border-radius: var(--radio-lg);
  }
</style>
