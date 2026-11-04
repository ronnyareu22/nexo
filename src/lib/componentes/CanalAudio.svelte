<!--
  Una tira del mezclador de Audio (Salida, Entrada o Avisos), de arriba abajo:
  el rótulo, la foto del aparato (o su ícono) con la aurora detrás, su nombre y cómo se conecta,
  el % en grande, el deslizador vertical y, al pie, el botón de silencio (o lo que mande quien la usa).
  Mientras la tocas, y 900 ms después de soltar, no sigue lo que llega del sistema: así la barra no rebota
  con una lectura vieja (igual que el brillo en TarjetaPantalla). Las diferencias de 1 % tampoco la mueven.
  Tiene siempre 5 filas (rótulo, aparato, cifra, barra y pie) y usa las del mezclador (subgrid): así las barras
  de todas las tiras quedan alineadas aunque un nombre baje de línea.
-->
<script lang="ts">
  import { untrack, type Snippet } from 'svelte';
  import { tooltip } from '$lib/tooltip';
  import Icono, { type Nombre } from './Icono.svelte';
  import DeslizadorVertical from './DeslizadorVertical.svelte';

  let {
    rotulo,
    nombre,
    detalle,
    foto = null,
    icono,
    valor,
    silenciado = null,
    nota,
    textoSilenciar = 'Silenciar',
    iconoSilencio = 'volumen',
    onvalor,
    onsilenciar,
    pie,
  }: {
    rotulo: string;
    nombre: string;
    /** Cómo se conecta ("Bluetooth", "Integrado"). */
    detalle?: string;
    foto?: string | null;
    icono: Nombre;
    /** 0–100; null si el aparato no deja cambiarlo. */
    valor: number | null;
    /** null si no se puede silenciar. */
    silenciado?: boolean | null;
    /** Lo que se explica si no se puede cambiar el volumen. */
    nota?: string;
    textoSilenciar?: string;
    /** El ícono del botón de silencio: volumen para la salida, micrófono para la entrada. */
    iconoSilencio?: Nombre;
    onvalor: (v: number) => void;
    onsilenciar?: () => void;
    /** Otro pie en lugar del botón de silencio. */
    pie?: Snippet;
  } = $props();

  let tocando = $state(false);
  let ultimoToque = 0;
  let nivel = $state(untrack(() => valor ?? 0));
  let fallo = $state(false);

  $effect(() => {
    const v = valor;
    if (v === null || tocando || performance.now() - ultimoToque < 900) return;
    if (Math.abs(v - untrack(() => nivel)) > 1 || v === 0 || v === 100) nivel = v;
  });

  // Si cambia la foto (otro aparato), se vuelve a intentar.
  $effect(() => {
    void foto;
    fallo = false;
  });

  function cambiar(v: number) {
    ultimoToque = performance.now();
    nivel = v;
    onvalor(v);
  }

  function alTocar(t: boolean) {
    tocando = t;
    ultimoToque = performance.now();
  }

  const mudo = $derived(silenciado === true);
  const controlable = $derived(valor !== null);
</script>

<section class="canal" class:mudo aria-label="{rotulo}: {nombre}">
  <p class="rotulo">{rotulo}</p>

  <div class="identidad">
    <div class="retrato" aria-hidden="true">
      {#if foto && !fallo}
        <img src={foto} alt="" draggable="false" decoding="async" onerror={() => (fallo = true)} />
      {:else}
        <span class="retrato__icono"><Icono nombre={icono} tamano={26} /></span>
      {/if}
    </div>
    <p class="nombre">{nombre}</p>
    {#if detalle}<span class="detalle">{detalle}</span>{/if}
  </div>

  <p class="cifra" class:tenue={mudo || !controlable} aria-hidden="true">
    {#if controlable}{nivel}<small>%</small>{:else}—{/if}
  </p>

  <div class="control">
    {#if controlable}
      <DeslizadorVertical
        valor={nivel}
        etiqueta="Volumen de {rotulo.toLowerCase()}: {nombre}"
        apagado={mudo}
        oncambio={cambiar}
        ontocando={alTocar}
      />
    {:else}
      <div class="control__barra">
        <DeslizadorVertical valor={0} etiqueta="Volumen de {nombre}" deshabilitado oncambio={() => {}} />
      </div>
      {#if nota}<p class="nota">{nota}</p>{/if}
    {/if}
  </div>

  <div class="pie">
    {#if pie}
      {@render pie()}
    {:else if silenciado !== null && onsilenciar}
      <button
        class="silencio"
        class:activo={mudo}
        type="button"
        aria-pressed={mudo}
        use:tooltip={mudo ? 'Quitar silencio' : textoSilenciar}
        onclick={onsilenciar}
      >
        <Icono nombre={mudo ? 'silencio' : iconoSilencio} tamano={16} />
        {mudo ? 'Silenciado' : 'Silenciar'}
      </button>
    {:else}
      <p class="sin-silencio">No se puede silenciar desde aquí</p>
    {/if}
  </div>
</section>

<style>
  /* En el mezclador ocupa sus 5 filas (subgrid). Sin contención aquí: con ella el subgrid no funciona. */
  .canal {
    grid-row: span 5;
    display: grid;
    grid-template-rows: subgrid;
    justify-items: center;
    align-items: start;
    min-width: 0;
    min-height: 0;
    padding: 14px 12px 12px;
    border-radius: 18px;
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    text-align: center;
  }
  .rotulo {
    justify-self: stretch;
    margin: 0;
  }

  .identidad {
    display: grid;
    justify-items: center;
    gap: 6px;
    min-width: 0;
    max-width: 100%;
  }
  /* El aparato sobre una superficie hundida con la aurora quieta detrás (DISENO.md 3). */
  .retrato {
    display: grid;
    place-items: center;
    width: 76px;
    height: 76px;
    border-radius: 22px;
    background:
      radial-gradient(closest-side at 50% 58%, var(--aurora-1), transparent),
      radial-gradient(closest-side at 30% 80%, var(--aurora-2), transparent),
      var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .retrato img {
    width: 62px;
    height: 62px;
    object-fit: contain;
    filter: drop-shadow(0 6px 8px var(--sombra-apoyo));
  }
  .retrato__icono {
    display: grid;
    color: var(--primario-texto);
  }
  .mudo .retrato img {
    filter: grayscale(0.6) drop-shadow(0 6px 8px var(--sombra-apoyo));
    opacity: 0.75;
  }
  .nombre {
    margin: 0;
    font-size: 13.5px;
    line-height: 18px;
    font-weight: 650;
    letter-spacing: -0.01em;
    color: var(--texto);
    overflow-wrap: anywhere;
  }
  .detalle {
    display: inline-flex;
    align-items: center;
    min-height: 20px;
    padding: 2px 8px;
    border-radius: 10px;
    line-height: 15px;
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-2);
    font-size: 11px;
    font-weight: 600;
  }

  .cifra {
    margin: 2px 0 0;
    font-size: 34px;
    line-height: 38px;
    font-weight: 700;
    letter-spacing: -0.03em;
    font-variant-numeric: tabular-nums;
    color: var(--texto);
  }
  .cifra small {
    margin-left: 1px;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0;
    color: var(--texto-2);
  }
  .cifra.tenue {
    color: var(--texto-3);
  }

  /* El deslizador ocupa todo el alto que sobra (la fila 1fr del mezclador). Es el contenedor que mira la escala
     del deslizador para esconder sus cifras en tiras angostas. */
  .control {
    container-type: inline-size;
    justify-self: stretch;
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 200px;
  }
  .control > :global(.deslizador),
  .control__barra {
    flex: 1;
    min-height: 0;
  }
  .nota {
    margin: 0;
    font-size: 11.5px;
    line-height: 16px;
    color: var(--texto-2);
    overflow-wrap: anywhere;
  }

  .pie {
    justify-self: stretch;
    align-self: end;
    display: grid;
    min-height: 38px;
  }
  .silencio {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    min-height: 38px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--radio-sm);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font-size: 12.5px;
    font-weight: 600;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .silencio:hover {
    background: var(--superficie-hover);
  }
  .silencio:active {
    transform: scale(0.98);
  }
  /* Silenciado: el botón se "enciende" en rojo suave, como en una consola. */
  .silencio.activo {
    color: var(--error);
    background: color-mix(in srgb, var(--error) 12%, var(--superficie));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--error) 35%, transparent);
  }
  .sin-silencio {
    display: grid;
    place-items: center;
    margin: 0;
    font-size: 11.5px;
    line-height: 15px;
    color: var(--texto-3);
  }
</style>
