<!--
  Página de un aparato: se abre al "entrar" desde Dispositivos (chevron, doble clic, Enter o "Ver todos los ajustes")
  o desde Inicio. Todo a la vista, sin pestañas:
  - a la izquierda, una columna con "Atrás", la foto grande sobre la aurora, el nombre, la marca y el tipo, el estado,
    las acciones (Conectar o Desconectar · Renombrar · Olvidar) y, al pie, la información del aparato;
  - a la derecha, un tablero de tarjetas que llena el alto de la ventana:
    · aparatos de sonido: Batería y Conexión a la izquierda; Sonido, Salida y entrada y Automático a la derecha
      (sin batería, Automático pasa a la izquierda para que las columnas queden parejas);
    · los demás: Batería, Conexión y Automático en una sola columna.
    Cada columna se reparte el alto: crecen las gráficas, no los huecos.
  Si el tablero es angosto, las tarjetas van una debajo de otra en este orden: Batería, Sonido, Salida y entrada,
  Conexión y Automático. Con la ventana angosta (menos de 1000 px) la columna izquierda pasa a ser una cabecera.
  Escape vuelve a la lista (lo maneja la vista, que sabe si hay un diálogo abierto).
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { textoAbrirBluetooth } from '$lib/api-sistema';
  import type { Dispositivo, Registro } from '$lib/tipos';
  import { baterias, esDeSonido, estadoDe, nombreDe, nombreTipo } from '$lib/tipos';
  import { abrirAjustesBluetooth } from '$lib/api';
  import Figura from './Figura.svelte';
  import EstadoChip from './EstadoChip.svelte';
  import PanelBateria from './PanelBateria.svelte';
  import PanelConexion from './PanelConexion.svelte';
  import PanelAutomatico from './PanelAutomatico.svelte';
  import ControlesAudio from './ControlesAudio.svelte';
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
  const conBateria = $derived(listaBaterias.length > 0);
  const deSonido = $derived(esDeSonido(d));
  const meta = $derived([d.fabricante, nombreTipo[d.tipo]].filter(Boolean).join(' · '));
  // Automático va en la columna izquierda si la derecha no existe o si a la izquierda falta la batería.
  const automaticoIzquierda = $derived(!deSonido || !conBateria);

  let titulo = $state<HTMLHeadingElement>();
  // Al entrar, el foco va al nombre: el lector de pantalla anuncia dónde estás.
  onMount(() => titulo?.focus({ preventScroll: true }));

  const ajustesBluetooth = () => abrirAjustesBluetooth().catch((e) => onerror(String(e)));
</script>

<article class="pagina" aria-labelledby="{id}-nombre">
  <!-- Columna izquierda: quién es, cómo está y qué se le puede hacer -->
  <aside class="lado vidrio">
    <button class="atras" type="button" onclick={onvolver}>
      <Icono nombre="atras" tamano={16} /> Atrás
    </button>

    <div class="escenario" class:apagado={!d.conectado}>
      <div class="figura"><Figura dispositivo={d} sombra={false} /></div>
    </div>

    <div class="quien">
      <h1 id="{id}-nombre" bind:this={titulo} tabindex="-1">{nombre}</h1>
      {#if meta}<p class="meta">{meta}</p>{/if}
      <EstadoChip estado={estadoDe(d, pendiente)} />
    </div>

    <div class="barra-acciones acciones" role="group" aria-label="Acciones de {nombre}">
      {#if d.conectado}
        <button type="button" onclick={onalternar} disabled={pendiente} aria-busy={pendiente}>
          {#if pendiente}<span class="girador" aria-hidden="true"></span>{:else}<Icono nombre="desconectar" tamano={20} />{/if}
          <span>{pendiente ? 'Desconectando…' : 'Desconectar'}</span>
        </button>
      {:else}
        <button type="button" class="primaria" onclick={onalternar} disabled={pendiente} aria-busy={pendiente}>
          {#if pendiente}<span class="girador" aria-hidden="true"></span>{:else}<Icono nombre="conectar" tamano={20} />{/if}
          <span>{pendiente ? 'Conectando…' : 'Conectar'}</span>
        </button>
      {/if}
      <button type="button" onclick={onrenombrar}>
        <Icono nombre="editar" tamano={20} />
        <span>Renombrar</span>
      </button>
      <button type="button" class="peligro" onclick={onolvidar}>
        <Icono nombre="papelera" tamano={20} />
        <span>Olvidar</span>
      </button>
    </div>

    {#if avisoOlvidar}
      <div class="aviso" role="status">
        <Icono nombre="info" tamano={18} />
        <div>
          <p>{avisoOlvidar}</p>
          <button class="enlace" type="button" onclick={ajustesBluetooth}>{textoAbrirBluetooth}</button>
        </div>
      </div>
    {/if}

    <section class="info" aria-labelledby="{id}-info">
      <h2 class="rotulo" id="{id}-info">Información</h2>
      <dl class="inserto">
        {#if d.alias?.trim()}<div><dt>Nombre real</dt><dd>{d.nombre}</dd></div>{/if}
        {#if d.fabricante}<div><dt>Marca</dt><dd>{d.fabricante}</dd></div>{/if}
        <div><dt>Tipo</dt><dd>{nombreTipo[d.tipo]}</dd></div>
        {#if d.firmware}<div><dt>Firmware</dt><dd class="numeros">{d.firmware}</dd></div>{/if}
        <div><dt>Dirección</dt><dd class="mono">{d.id}</dd></div>
      </dl>
    </section>
  </aside>

  <!-- Tablero: todas las tarjetas a la vista -->
  <div class="tablero">
    <div class="rejilla" class:dos={deSonido}>
      <div class="columna">
        {#if conBateria}
          <div class="pieza bateria"><PanelBateria baterias={listaBaterias} {registro} /></div>
        {/if}
        <div class="pieza conexion"><PanelConexion dispositivo={d} {pendiente} {registro} /></div>
        {#if automaticoIzquierda}
          <div class="pieza automatico"><PanelAutomatico dispositivo={d} {onerror} /></div>
        {/if}
      </div>
      {#if deSonido}
        <div class="columna">
          <div class="audio"><ControlesAudio dispositivo={d} variante="tarjeta" {onerror} /></div>
          {#if !automaticoIzquierda}
            <div class="pieza automatico"><PanelAutomatico dispositivo={d} {onerror} /></div>
          {/if}
        </div>
      {/if}
    </div>
  </div>
</article>

<style>
  .pagina {
    display: grid;
    grid-template-columns: clamp(288px, 29%, 340px) minmax(0, 1fr);
    gap: 16px;
    height: 100%;
    min-height: 0;
  }

  /* ---------- Columna izquierda ---------- */
  .lado {
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 16px 18px 18px;
    border-radius: var(--radio-lg);
    isolation: isolate;
  }
  .atras {
    align-self: flex-start;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 32px;
    padding: 0 13px 0 8px;
    border: 0;
    border-radius: var(--radio-pildora);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .atras :global(svg) {
    color: var(--texto-2);
  }
  .atras:hover {
    background: var(--superficie-hover);
  }
  .atras:active {
    transform: scale(0.98);
  }

  /* La foto sobre la aurora (turquesa y azul) y su sombra de apoyo: degradados quietos, sin filtros.
     El escenario toma el alto que sobre en la columna, para que no queden huecos. */
  .escenario {
    position: relative;
    flex: 1 1 0;
    min-height: 150px;
    max-height: 340px;
    margin: -8px -6px 0;
    border-radius: 20px;
    background:
      radial-gradient(28% 5% at 50% 88%, var(--sombra-apoyo), transparent 100%),
      radial-gradient(46% 44% at 50% 52%, var(--aurora-1), transparent 72%),
      radial-gradient(40% 42% at 28% 38%, var(--aurora-2), transparent 70%),
      radial-gradient(38% 40% at 74% 32%, var(--aurora-3), transparent 70%);
  }
  /* La foto se ajusta a la caja del escenario, sea del alto que sea. */
  .figura {
    position: absolute;
    inset: 7% 10% 9%;
    transition: opacity var(--dur-vista) ease;
  }
  .apagado .figura {
    opacity: 0.6;
  }

  .quien {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    text-align: center;
  }
  h1 {
    margin: 0;
    max-width: 100%;
    overflow-wrap: anywhere;
    font: var(--letra-titulo);
    letter-spacing: -0.025em;
  }
  h1:focus {
    outline: none;
  }
  .meta {
    margin: 0 0 8px;
    font-size: 13px;
    line-height: 18px;
    color: var(--texto-2);
  }

  .acciones {
    flex-shrink: 0;
  }

  /* La información va al pie: el espacio que sobre queda entre las acciones y ella, no debajo. */
  .info {
    flex-shrink: 0;
    display: grid;
    gap: 8px;
    margin-top: auto;
  }
  .info h2 {
    margin: 0;
    padding-left: 2px;
  }
  dl {
    margin: 0;
    padding: 0 14px;
  }
  dl > div {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 16px;
    min-height: 38px;
    padding: 9px 0;
    font-size: 13px;
    line-height: 19px;
  }
  dl > div + div {
    border-top: 1px solid var(--borde);
  }
  dt {
    flex-shrink: 0;
    color: var(--texto-2);
  }
  dd {
    min-width: 0;
    margin: 0;
    text-align: right;
    font-weight: 600;
    overflow-wrap: anywhere;
    user-select: text;
    -webkit-user-select: text;
  }
  .numeros {
    font-variant-numeric: tabular-nums;
  }
  .mono {
    font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace;
    font-size: 12px;
    font-weight: 500;
  }

  .aviso {
    flex-shrink: 0;
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

  /* ---------- Tablero ---------- */
  /* 1 px de aire para que el canto de las tarjetas (va por fuera) no lo recorte el desplazamiento. */
  .tablero {
    container: tablero / inline-size;
    min-width: 0;
    min-height: 0;
    margin: -1px;
    padding: 1px;
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  /* Angosto: una sola columna con el orden de lectura (Batería, Sonido, Salida y entrada, Conexión, Automático). */
  .rejilla {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .columna,
  .audio {
    display: contents;
  }
  @container tablero (max-width: 599.98px) {
    .bateria {
      order: 1;
    }
    .audio > :global(section:first-child) {
      order: 2;
    }
    .audio > :global(section:last-child) {
      order: 3;
    }
    .conexion {
      order: 4;
    }
    .automatico {
      order: 5;
    }
  }
  .pieza {
    display: flex;
    flex-direction: column;
  }
  .pieza > :global(section) {
    flex: 1 1 auto;
  }

  /* Con espacio: las tarjetas llenan el alto. Dos columnas para los aparatos de sonido. */
  @container tablero (min-width: 600px) {
    .rejilla {
      display: grid;
      grid-template-columns: minmax(0, 1fr);
      gap: 16px;
      min-height: 100%;
    }
    .rejilla.dos {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .columna {
      display: flex;
      flex-direction: column;
      gap: 16px;
      min-width: 0;
    }
    /* Lo que sobra de alto se lo llevan las tarjetas que tienen algo que crecer (gráficas, barras, el sonido);
       Salida y entrada crece poco y Automático nada. */
    .pieza,
    .audio > :global(section:first-child) {
      flex: 1 0 auto;
    }
    .audio > :global(section:last-child) {
      flex: 0.4 0 auto;
    }
    .automatico {
      flex-grow: 0;
    }
  }

  /* ---------- Ventana angosta: la columna izquierda pasa a ser una cabecera ---------- */
  @media (max-width: 999.98px) {
    .pagina {
      display: flex;
      flex-direction: column;
      gap: 16px;
      margin: -1px;
      padding: 1px;
      overflow-y: auto;
      overscroll-behavior: contain;
    }
    .lado {
      flex-shrink: 0;
      overflow: visible;
      display: grid;
      grid-template-columns: 128px minmax(0, 1fr);
      grid-template-areas:
        'atras atras'
        'foto quien'
        'foto acciones'
        'info info';
      align-items: center;
      gap: 12px 18px;
      padding: 14px 16px 16px;
    }
    .atras {
      grid-area: atras;
      justify-self: start;
    }
    .escenario {
      grid-area: foto;
      align-self: stretch;
      min-height: 124px;
      margin: 0;
    }
    .figura {
      inset: 8px;
    }
    .quien {
      grid-area: quien;
      align-items: flex-start;
      text-align: left;
    }
    h1 {
      font: var(--letra-seccion);
      letter-spacing: -0.02em;
    }
    .meta {
      margin-bottom: 4px;
    }
    .acciones {
      grid-area: acciones;
    }
    .aviso {
      grid-column: 1 / -1;
    }
    .info {
      grid-area: info;
      margin-top: 0;
    }
    /* La información en dos columnas, para que las tarjetas asomen antes. */
    dl {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      column-gap: 28px;
    }
    dl > div + div {
      border-top: 0;
    }
    dl > div:nth-child(n + 3) {
      border-top: 1px solid var(--borde);
    }
    .tablero {
      flex-shrink: 0;
      margin: 0;
      padding: 0;
      overflow: visible;
    }
  }
</style>
