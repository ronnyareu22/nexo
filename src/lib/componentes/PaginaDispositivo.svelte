<!--
  Página de un aparato: se abre al "entrar" desde Dispositivos (chevron, doble clic, Enter o "Ver todos los ajustes")
  o desde Inicio. Una tarjeta grande partida en dos:
  - a la izquierda, "Atrás", la foto, el nombre, el estado y Desconectar (o Conectar);
  - a la derecha, pestañas General · Audio · Avanzado.
    General: batería, controles de audio y conexión. Audio: el panel Sonido completo.
    Avanzado: automático, información, Renombrar y Olvidar.
  Con la ventana angosta (menos de 1000 px) la columna izquierda pasa a ser una cabecera compacta.
  Escape vuelve a la lista (lo maneja la vista, que sabe si hay un diálogo abierto).
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type { Dispositivo, Registro } from '$lib/tipos';
  import { baterias, esDeSonido, estadoDe, nombreDe, nombreTipo } from '$lib/tipos';
  import { abrirAjustesBluetooth } from '$lib/api';
  import Figura from './Figura.svelte';
  import EstadoChip from './EstadoChip.svelte';
  import PanelBateria from './PanelBateria.svelte';
  import PanelConexion from './PanelConexion.svelte';
  import PanelAutomatico from './PanelAutomatico.svelte';
  import ControlesAudio from './ControlesAudio.svelte';
  import Sonido from './Sonido.svelte';
  import Icono from './Icono.svelte';

  let {
    dispositivo: d,
    pendiente,
    registro,
    avisoOlvidar = null,
    onalternar,
    onrenombrar,
    onolvidar,
    onvolver,
    onerror,
  }: {
    dispositivo: Dispositivo;
    pendiente: boolean;
    registro: Registro | null;
    /** Si no se pudo olvidar desde Nexo: el motivo, con la salida a Ajustes › Bluetooth. */
    avisoOlvidar?: string | null;
    onalternar: () => void;
    onrenombrar: () => void;
    onolvidar: () => void;
    /** Vuelve a la lista de Dispositivos. */
    onvolver: () => void;
    onerror: (texto: string) => void;
  } = $props();

  const id = $props.id();
  const nombre = $derived(nombreDe(d));
  const listaBaterias = $derived(baterias(d));
  const deSonido = $derived(esDeSonido(d));

  type Pestana = 'general' | 'audio' | 'avanzado';
  // Teclado: cuando llegue su fase (DISENO.md, sección 9) va aquí, entre Audio y Avanzado, solo para teclados.
  const pestanas = $derived<{ id: Pestana; nombre: string }[]>([
    { id: 'general', nombre: 'General' },
    ...(deSonido ? [{ id: 'audio' as const, nombre: 'Audio' }] : []),
    { id: 'avanzado', nombre: 'Avanzado' },
  ]);
  let pestana = $state<Pestana>('general');
  const actual = $derived(pestanas.some((p) => p.id === pestana) ? pestana : 'general');

  let contenido = $state<HTMLDivElement>();
  let lista = $state<HTMLDivElement>();
  let titulo = $state<HTMLHeadingElement>();

  function elegirPestana(p: Pestana) {
    if (p === actual) return;
    pestana = p;
    contenido?.scrollTo({ top: 0 });
  }

  // Flechas, Inicio y Fin recorren las pestañas (y las eligen), como un selector segmentado.
  async function teclasPestanas(e: KeyboardEvent, i: number) {
    const n = pestanas.length;
    let j: number;
    if (e.key === 'ArrowRight') j = (i + 1) % n;
    else if (e.key === 'ArrowLeft') j = (i - 1 + n) % n;
    else if (e.key === 'Home') j = 0;
    else if (e.key === 'End') j = n - 1;
    else return;
    e.preventDefault();
    elegirPestana(pestanas[j].id);
    await tick();
    lista?.querySelector<HTMLElement>(`[data-pestana="${pestanas[j].id}"]`)?.focus();
  }

  // Al entrar, el foco va al nombre: el lector de pantalla anuncia dónde estás.
  onMount(() => titulo?.focus({ preventScroll: true }));

  const ajustesBluetooth = () => abrirAjustesBluetooth().catch((e) => onerror(String(e)));
</script>

<article class="pagina" aria-labelledby="{id}-nombre">
  <!-- Columna izquierda: quién es y cómo está -->
  <div class="lado">
    <button class="atras" type="button" onclick={onvolver}>
      <Icono nombre="atras" tamano={16} /> Atrás
    </button>

    <div class="identidad">
      <div class="escenario" class:apagado={!d.conectado}>
        <div class="figura"><Figura dispositivo={d} sombra={false} /></div>
      </div>
      <div class="quien">
        <h1 id="{id}-nombre" bind:this={titulo} tabindex="-1">{nombre}</h1>
        <EstadoChip estado={estadoDe(d, pendiente)} />
        {#if d.conectado}
          <button class="pildora" type="button" onclick={onalternar} disabled={pendiente} aria-busy={pendiente}>
            <Icono nombre="desconectar" tamano={16} /> Desconectar
          </button>
        {:else}
          <button class="pildora primaria" type="button" onclick={onalternar} disabled={pendiente} aria-busy={pendiente}>
            <Icono nombre="conectar" tamano={16} /> Conectar
          </button>
        {/if}
      </div>
    </div>
  </div>

  <!-- Columna derecha: pestañas y su contenido -->
  <div class="principal">
    <div class="pestanas" role="tablist" aria-label="Ajustes de {nombre}" bind:this={lista}>
      {#each pestanas as p, i (p.id)}
        <button
          class="pestana"
          class:activa={actual === p.id}
          type="button"
          role="tab"
          id="{id}-pestana-{p.id}"
          data-pestana={p.id}
          aria-selected={actual === p.id}
          aria-controls="{id}-contenido"
          tabindex={actual === p.id ? 0 : -1}
          onclick={() => elegirPestana(p.id)}
          onkeydown={(e) => teclasPestanas(e, i)}
        >
          {p.nombre}
        </button>
      {/each}
    </div>

    <div
      class="contenido"
      bind:this={contenido}
      id="{id}-contenido"
      role="tabpanel"
      aria-labelledby="{id}-pestana-{actual}"
    >
      {#if actual === 'general'}
        <!-- Orden: Batería, Controles de audio, Conexión. Con espacio, en dos columnas (batería y conexión a la izquierda). -->
        <div class="general" class:dos={deSonido && listaBaterias.length > 0}>
          <div class="columna">
            {#if listaBaterias.length}
              <div class="pieza bateria"><PanelBateria baterias={listaBaterias} {registro} /></div>
            {/if}
            <div class="pieza conexion"><PanelConexion dispositivo={d} {pendiente} {registro} /></div>
          </div>
          {#if deSonido}
            <div class="columna">
              <div class="pieza audio"><ControlesAudio dispositivo={d} variante="tarjeta" {onerror} /></div>
            </div>
          {/if}
        </div>
      {:else if actual === 'audio'}
        <div class="angosto">
          {#if d.conectado}
            <Sonido id={d.id} {onerror} />
          {:else}
            <section class="panel vidrio nota-panel">
              <h2>Sonido</h2>
              <p>Conéctalo para elegirlo como salida de sonido y cambiar su volumen y su micrófono.</p>
            </section>
          {/if}
        </div>
      {:else}
        <div class="angosto avanzado">
          <PanelAutomatico dispositivo={d} {onerror} />

          <section class="panel vidrio" aria-labelledby="{id}-info">
            <h2 id="{id}-info">Información</h2>
            <dl>
              {#if d.alias?.trim()}<dt>Nombre real</dt><dd>{d.nombre}</dd>{/if}
              {#if d.fabricante}<dt>Marca</dt><dd>{d.fabricante}</dd>{/if}
              <dt>Tipo</dt><dd>{nombreTipo[d.tipo]}</dd>
              {#if d.firmware}<dt>Firmware</dt><dd>{d.firmware}</dd>{/if}
              <dt>Dirección</dt><dd class="mono">{d.id}</dd>
            </dl>
          </section>

          {#if avisoOlvidar}
            <div class="aviso" role="status">
              <Icono nombre="info" tamano={18} />
              <div>
                <p>{avisoOlvidar}</p>
                <button class="enlace" type="button" onclick={ajustesBluetooth}>Abrir Ajustes de Bluetooth</button>
              </div>
            </div>
          {/if}

          <div class="acciones">
            <button class="accion" type="button" onclick={onrenombrar}>
              <Icono nombre="editar" tamano={17} /> Renombrar
            </button>
            <button class="accion olvidar" type="button" onclick={onolvidar}>
              <Icono nombre="papelera" tamano={17} /> Olvidar
            </button>
          </div>
        </div>
      {/if}
    </div>
  </div>
</article>

<style>
  .pagina {
    display: grid;
    grid-template-columns: 300px minmax(0, 1fr);
    height: 100%;
    min-height: 0;
    overflow: hidden;
    border: 1px solid var(--borde);
    border-radius: var(--radio-xl);
    background: var(--superficie);
  }

  /* ---------- Columna izquierda ---------- */
  .lado {
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px 20px 24px;
    border-right: 1px solid var(--borde);
  }
  .atras {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 34px;
    padding: 0 14px 0 10px;
    border: 0;
    border-radius: var(--radio-pildora);
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .atras:hover {
    background: var(--superficie-hover);
  }
  .atras:active {
    transform: scale(0.98);
  }

  .identidad {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
  }
  /* La foto sobre un resplandor turquesa y azul, pintado una vez (sin filtros ni animación). */
  .escenario {
    display: grid;
    place-items: center;
    width: 100%;
    height: 240px;
    background:
      radial-gradient(50% 48% at 50% 56%, var(--aurora-1), transparent 72%),
      radial-gradient(42% 46% at 30% 40%, var(--aurora-2), transparent 70%),
      radial-gradient(40% 44% at 72% 36%, var(--aurora-3), transparent 70%);
  }
  .figura {
    width: 216px;
    height: 216px;
    transition: opacity var(--dur-vista) ease;
  }
  .apagado .figura {
    opacity: 0.6;
  }

  .quien {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    width: 100%;
    text-align: center;
  }
  h1 {
    margin: 0;
    max-width: 100%;
    overflow-wrap: anywhere;
    font-size: 22px;
    line-height: 28px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  h1:focus {
    outline: none;
  }
  .pildora {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 38px;
    margin-top: 6px;
    padding: 0 18px 0 14px;
    border: 1px solid var(--borde-fuerte);
    border-radius: var(--radio-pildora);
    background: transparent;
    color: var(--texto);
    font-size: 13.5px;
    font-weight: 600;
    transition:
      background-color var(--dur-color) ease,
      border-color var(--dur-color) ease;
  }
  .pildora:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .pildora:active:not(:disabled) {
    transform: scale(0.98);
  }
  .pildora:disabled {
    opacity: 0.6;
    cursor: progress;
  }
  .pildora.primaria {
    border-color: transparent;
    background: var(--primario);
    color: var(--sobre-primario);
  }
  .pildora.primaria:hover:not(:disabled) {
    background: var(--primario-hover);
  }

  /* ---------- Columna derecha ---------- */
  .principal {
    min-height: 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .pestanas {
    flex-shrink: 0;
    align-self: flex-start;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: 18px 24px 0;
    padding: 4px;
    border-radius: var(--radio-pildora);
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .pestana {
    min-height: 32px;
    padding: 0 18px;
    border: 1px solid transparent;
    border-radius: var(--radio-pildora);
    background: transparent;
    color: var(--texto-2);
    font-size: 13.5px;
    font-weight: 600;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .pestana:hover:not(.activa) {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .pestana.activa {
    border-color: var(--elegido-borde);
    background: var(--elegido-bg);
    color: var(--elegido-fg);
  }

  .contenido {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 18px 24px 24px;
    container-type: inline-size;
  }

  /* Las tarjetas van dentro de la página: superficie elevada, sin borde ni sombra. */
  .contenido :global(section.vidrio) {
    padding: 18px 20px;
    border-radius: 18px;
    background: var(--superficie-elevada);
    box-shadow: none;
  }

  /* General: una columna (Batería, Controles de audio, Conexión) y, con espacio, dos. */
  .general {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .columna {
    display: contents;
  }
  .bateria {
    order: 1;
  }
  .audio {
    order: 2;
  }
  .conexion {
    order: 3;
  }
  @container (min-width: 860px) {
    .general.dos {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      align-items: start;
      max-width: 1160px;
    }
    .general.dos .columna {
      display: flex;
      flex-direction: column;
      gap: 16px;
    }
  }

  .angosto {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  h2 {
    margin: 0 0 12px;
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
  }
  .nota-panel p {
    margin: 0;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto-2);
  }

  dl {
    display: grid;
    grid-template-columns: minmax(96px, auto) minmax(0, 1fr);
    gap: 10px 16px;
    margin: 0;
    font-size: 13.5px;
    line-height: 19px;
  }
  dt {
    color: var(--texto-2);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
    user-select: text;
    -webkit-user-select: text;
  }
  .mono {
    font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace;
    font-size: 12.5px;
  }

  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  .accion {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 40px;
    padding: 0 18px 0 14px;
    border: 0;
    border-radius: var(--radio-sm);
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font-size: 13.5px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .accion:hover {
    background: var(--superficie-hover);
  }
  .accion:active {
    transform: scale(0.98);
  }
  .accion.olvidar {
    color: var(--error);
  }

  .aviso {
    display: flex;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radio-md);
    background: color-mix(in srgb, var(--aviso) 10%, var(--superficie));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--aviso) 30%, transparent);
    color: var(--aviso);
  }
  .aviso :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
  }
  .aviso p {
    margin: 0;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto);
  }
  .enlace {
    margin-top: 4px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--primario-texto);
    font-size: 13px;
    font-weight: 600;
  }
  .enlace:hover {
    text-decoration: underline;
  }

  /* ---------- Ventana angosta: la columna izquierda pasa a ser una cabecera compacta ---------- */
  @media (max-width: 999.98px) {
    .pagina {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: auto minmax(0, 1fr);
    }
    .lado {
      overflow: visible;
      gap: 10px;
      padding: 14px 20px 16px;
      border-right: 0;
      border-bottom: 1px solid var(--borde);
    }
    .identidad {
      flex-direction: row;
      align-items: center;
      gap: 18px;
    }
    .escenario {
      flex-shrink: 0;
      width: 132px;
      height: 120px;
    }
    .figura {
      width: 112px;
      height: 112px;
    }
    .quien {
      align-items: flex-start;
      gap: 8px;
      text-align: left;
    }
    .pildora {
      margin-top: 2px;
    }
    .pestanas {
      margin: 16px 20px 0;
    }
    .contenido {
      padding: 16px 20px 20px;
    }
  }
</style>
