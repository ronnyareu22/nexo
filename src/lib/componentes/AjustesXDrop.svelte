<!--
  Los ajustes de XDrop, detrás del engranaje de arriba a la derecha: una ventanita sólida que flota bajo el botón
  con "Recibir archivos", "Copiar imágenes recibidas al portapapeles", la carpeta donde llega todo con
  "Abrir carpeta" y, en Windows, la nota del cortafuegos. Se cierra con Escape, con un clic afuera o con el botón.
  Si la recepción está apagada, el engranaje lleva un punto ámbar.
  Para revisar el diseño fuera de la app, `?drop=ajustes` lo deja abierto.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { fade } from 'svelte/transition';
  import { abrirCarpetaDrop, ajustarDrop, esMac, pruebaDrop } from '$lib/api-sistema';
  import type { AjustesDrop, EstadoDrop } from '$lib/tipos-sistema';
  import { tooltip } from '$lib/tooltip';
  import Icono from './Icono.svelte';
  import Interruptor from './Interruptor.svelte';

  let {
    estado,
    oncambio,
    onerror,
  }: {
    estado: EstadoDrop | null;
    /** Cómo quedó todo después de un cambio. */
    oncambio: (estado: EstadoDrop) => void;
    onerror: (texto: string) => void;
  } = $props();

  const id = $props.id();
  const sinMovimiento = typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  let abierto = $state(pruebaDrop === 'ajustes');
  let caja = $state<HTMLDivElement>();
  let boton = $state<HTMLButtonElement>();

  async function abrir() {
    abierto = true;
    await tick();
    caja?.focus({ preventScroll: true });
  }

  function cerrar(volverAlBoton = false) {
    if (!abierto) return;
    abierto = false;
    if (volverAlBoton) boton?.focus();
  }

  // Abierto: un clic afuera o Escape lo cierran.
  $effect(() => {
    if (!abierto) return;
    const fuera = (e: PointerEvent) => {
      const t = e.target as Node | null;
      if (t && !caja?.contains(t) && !boton?.contains(t)) cerrar();
    };
    const tecla = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.preventDefault();
      cerrar(true);
    };
    document.addEventListener('pointerdown', fuera, true);
    document.addEventListener('keydown', tecla);
    return () => {
      document.removeEventListener('pointerdown', fuera, true);
      document.removeEventListener('keydown', tecla);
    };
  });

  let ocupado = $state<keyof AjustesDrop | null>(null);
  async function ajustar(clave: keyof AjustesDrop, valor: boolean) {
    if (!estado || ocupado) return;
    ocupado = clave;
    try {
      oncambio(await ajustarDrop({ [clave]: valor }));
    } catch (e) {
      onerror(String(e));
    }
    ocupado = null;
  }

  async function abrirCarpeta() {
    try {
      await abrirCarpetaDrop();
    } catch (e) {
      onerror(String(e));
    }
  }

  const ayudaRecibir = $derived.by(() => {
    const e = estado;
    if (!e) return '';
    if (!e.ajustes.recibir) return 'Apagado: tus otros equipos no pueden mandarte nada.';
    if (e.escuchando) return `Tus equipos pueden mandarte a ${e.nombre} (${e.escuchando}). Solo tus equipos en Tailscale.`;
    if (e.tailscale === 'no instalado') return 'Instala Tailscale para recibir.';
    return 'Se activa solo en cuanto Tailscale se conecte.';
  });

  const apagado = $derived(!!estado && !estado.ajustes.recibir);

  /** "/Users/ronny/Downloads/XDrop" → ["/Users/", "ronny/", "Downloads/", "XDrop"]: baja de línea tras cada barra. */
  const partesRuta = (ruta: string) => ruta.split(/(?<=[\\/])/);
</script>

<div class="ajustes">
  <button
    bind:this={boton}
    class="boton-redondo"
    class:abierto
    type="button"
    aria-label={apagado ? 'Ajustes de XDrop (la recepción está apagada)' : 'Ajustes de XDrop'}
    aria-expanded={abierto}
    aria-controls="{id}-caja"
    aria-haspopup="dialog"
    use:tooltip={abierto ? undefined : 'Ajustes de XDrop'}
    onclick={() => (abierto ? cerrar() : abrir())}
  >
    <Icono nombre="engranaje" tamano={18} />
    {#if apagado}<span class="punto" aria-hidden="true"></span>{/if}
  </button>

  {#if abierto}
    <div
      bind:this={caja}
      id="{id}-caja"
      class="caja flotante"
      role="dialog"
      aria-labelledby="{id}-titulo"
      tabindex="-1"
      transition:fade={{ duration: sinMovimiento ? 0 : 120 }}
    >
      <div class="t-cabeza">
        <span class="ficha-icono chica"><Icono nombre="engranaje" tamano={16} /></span>
        <div class="t-textos">
          <h2 class="t-titulo" id="{id}-titulo">Ajustes de XDrop</h2>
          <p class="t-sub">{estado ? `En este equipo: ${estado.nombre}` : 'En este equipo'}</p>
        </div>
        <button class="cerrar" type="button" aria-label="Cerrar" onclick={() => cerrar(true)}>
          <Icono nombre="cerrar" tamano={15} />
        </button>
      </div>

      {#if estado}
        {@const a = estado.ajustes}
        <div class="filas">
          <div class="fila">
            <div class="textos">
              <p class="titulo">Recibir archivos</p>
              <p class="ayuda">{ayudaRecibir}</p>
            </div>
            <Interruptor
              encendido={a.recibir}
              cargando={ocupado === 'recibir'}
              etiqueta="Recibir archivos"
              oncambio={() => ajustar('recibir', !a.recibir)}
            />
          </div>
          <div class="fila">
            <div class="textos">
              <p class="titulo">Copiar imágenes recibidas al portapapeles</p>
              <p class="ayuda">Si llega una sola imagen, queda lista para pegar.</p>
            </div>
            <Interruptor
              encendido={a.copiarImagenes}
              cargando={ocupado === 'copiarImagenes'}
              etiqueta="Copiar imágenes recibidas al portapapeles"
              oncambio={() => ajustar('copiarImagenes', !a.copiarImagenes)}
            />
          </div>
        </div>

        <div class="carpeta">
          <span class="carpeta__icono" aria-hidden="true"><Icono nombre="carpeta" tamano={18} /></span>
          <div class="textos">
            <p class="rotulo">Lo recibido queda en</p>
            <p class="ruta">
              {#each partesRuta(estado.carpeta ?? 'Descargas › XDrop') as parte, i (i)}{parte}<wbr />{/each}
            </p>
          </div>
          <button class="boton-vidrio boton" type="button" onclick={abrirCarpeta}>Abrir carpeta</button>
        </div>

        {#if !esMac}
          <p class="nota">
            <Icono nombre="info" tamano={15} />
            La primera vez, Windows puede preguntar si Nexo puede usar la red: permite el acceso en redes privadas.
          </p>
        {/if}
      {:else}
        <p class="ayuda cargando">Leyendo los ajustes…</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .ajustes {
    position: relative;
    flex-shrink: 0;
  }
  .boton-redondo {
    position: relative;
  }
  .boton-redondo.abierto {
    color: var(--elegido-fg);
    background: var(--elegido-bg);
    box-shadow:
      inset 0 0 0 1px var(--elegido-borde),
      var(--sombra-tarjeta);
  }
  /* Recepción apagada: un punto ámbar en la esquina. */
  .punto {
    position: absolute;
    top: 3px;
    right: 3px;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--aviso);
    box-shadow: 0 0 0 2px var(--fondo);
  }

  .caja {
    position: absolute;
    z-index: 30;
    top: calc(100% + 8px);
    right: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: min(390px, calc(100vw - 48px));
    padding: 14px 16px 16px;
    border-radius: 18px;
    outline: none;
    text-align: left;
  }
  .cerrar {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--texto-3);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .cerrar:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }

  .filas {
    border-radius: var(--radio-md);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .fila {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
  }
  .fila + .fila {
    border-top: 1px solid var(--borde);
  }
  .textos {
    flex: 1;
    min-width: 0;
  }
  .titulo {
    margin: 0;
    font-weight: 600;
    line-height: 19px;
  }
  .ayuda {
    margin: 2px 0 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
    overflow-wrap: anywhere;
  }
  .cargando {
    margin: 0;
  }

  .carpeta {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--radio-md);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .carpeta__icono {
    display: grid;
    flex-shrink: 0;
    color: var(--primario-texto);
  }
  .carpeta .rotulo {
    margin: 0 0 1px;
  }
  .ruta {
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto);
    overflow-wrap: break-word;
    user-select: text;
    -webkit-user-select: text;
    cursor: text;
  }
  .boton {
    flex-shrink: 0;
    min-height: 32px;
    padding: 0 12px;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .boton:hover {
    background: var(--superficie-hover);
  }

  .nota {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radio-sm);
    background: var(--superficie);
    color: var(--texto-2);
    font-size: 12.5px;
    line-height: 18px;
  }
  .nota :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
    color: var(--texto-3);
  }
</style>
