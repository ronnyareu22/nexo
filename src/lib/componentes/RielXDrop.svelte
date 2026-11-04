<!--
  El riel de acciones de XDrop, al lado de la zona para soltar: una columna de botones parejos, cada uno con
  su ícono, su nombre y una ayuda corta. Abajo, si este equipo está recibiendo.
  Para sumar una acción: agrega una fila a `acciones`; si necesita un campo, ponle `campo` y su bloque abajo.
  "Abrir enlace allá" y "Escribir una nota" abren su campo justo debajo de su fila, sin salir del riel.
  Sin destino, solo queda activa "Abrir carpeta XDrop".
  Cuando XDrop llena la ventana y al riel le sobra alto, las filas se reparten el alto como mosaicos, con el ícono
  encima del texto (igual que la barra de acciones de Dispositivos).
  Para revisar el diseño fuera de la app: `?drop=enlace` o `?drop=nota` abren su campo.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import {
    abrirCarpetaDrop,
    enviarCapturaDrop,
    enviarCopiadoDrop,
    enviarEnlaceDrop,
    enviarTextoDrop,
    esMac,
    pruebaDrop,
  } from '$lib/api-sistema';
  import type { DestinoDrop, EstadoDrop } from '$lib/tipos-sistema';
  import Icono, { type Nombre } from './Icono.svelte';

  let {
    destino,
    estado,
    onresultado,
    onenviado,
    onerror,
  }: {
    destino: DestinoDrop | null | undefined;
    estado: EstadoDrop | null;
    /** Cómo salió una acción, para mostrarlo bajo la zona. */
    onresultado: (texto: string, tono: 'ok' | 'aviso') => void;
    /** Se mandó algo: hay que releer el historial. */
    onenviado: () => void;
    onerror: (texto: string) => void;
  } = $props();

  type Campo = 'enlace' | 'nota';
  interface Accion {
    id: string;
    icono: Nombre;
    titulo: string;
    /** El texto mientras trabaja ("Enviando…"). */
    ocupadoTexto?: string;
    ayuda: string;
    /** Sin otro equipo no sirve. */
    necesitaDestino: boolean;
    campo?: Campo;
    hacer?: () => Promise<void>;
  }

  const miles = (n: number) => n.toLocaleString('es');
  const otro = $derived(destino?.nombre ?? 'tu otro equipo');

  // --- Las acciones (en este orden) ---
  const acciones = $derived<Accion[]>(
    [
      {
        id: 'copiado',
        icono: 'portapapeles' as Nombre,
        titulo: 'Enviar lo que copiaste',
        ocupadoTexto: 'Enviando…',
        ayuda: 'Texto, imagen o archivos',
        necesitaDestino: true,
        hacer: enviarCopiado,
      },
      {
        id: 'enlace',
        icono: 'conectar' as Nombre,
        titulo: 'Abrir enlace allá',
        ayuda: `En el navegador de ${otro}`,
        necesitaDestino: true,
        campo: 'enlace' as Campo,
      },
      // La captura de zona es de la Mac (screencapture); en Windows no se muestra.
      ...(esMac
        ? [
            {
              id: 'captura',
              icono: 'captura' as Nombre,
              titulo: 'Enviar captura',
              ocupadoTexto: 'Elige la zona…',
              ayuda: 'Eliges una zona de la pantalla',
              necesitaDestino: true,
              hacer: enviarCaptura,
            },
          ]
        : []),
      {
        id: 'nota',
        icono: 'editar' as Nombre,
        titulo: 'Escribir una nota',
        ayuda: 'Llega a su portapapeles',
        necesitaDestino: true,
        campo: 'nota' as Campo,
      },
      {
        id: 'carpeta',
        icono: 'carpeta' as Nombre,
        titulo: 'Abrir carpeta XDrop',
        ayuda: 'Descargas › XDrop',
        necesitaDestino: false,
        hacer: abrirCarpeta,
      },
    ],
  );

  let abierta = $state<Campo | null>(pruebaDrop === 'enlace' || pruebaDrop === 'nota' ? pruebaDrop : null);
  let ocupada = $state<string | null>(null);
  let campoEnlace = $state<HTMLInputElement>();
  let campoNota = $state<HTMLTextAreaElement>();

  async function pulsar(a: Accion) {
    if (a.campo) {
      abierta = abierta === a.campo ? null : a.campo;
      await tick();
      if (abierta === 'enlace') campoEnlace?.focus();
      if (abierta === 'nota') campoNota?.focus();
      return;
    }
    if (!a.hacer || ocupada) return;
    ocupada = a.id;
    try {
      await a.hacer();
    } finally {
      ocupada = null;
    }
  }

  // --- Lo que hace cada una ---
  async function enviarCopiado() {
    const d = destino;
    if (!d) return;
    try {
      const r = await enviarCopiadoDrop(d.ip);
      if (r.tipo === 'texto') onresultado(`Texto enviado: ya está en el portapapeles de ${d.nombre}.`, 'ok');
      else if (r.tipo === 'imagen') onresultado(`Enviando la imagen a ${d.nombre}.`, 'ok');
      else onresultado(`Enviando ${r.cantidad === 1 ? '1 elemento' : `${miles(r.cantidad)} elementos`} a ${d.nombre}.`, 'ok');
      onenviado();
    } catch (e) {
      onresultado(String(e), 'aviso');
    }
  }

  async function enviarCaptura() {
    const d = destino;
    if (!d) return;
    try {
      const r = await enviarCapturaDrop(d.ip);
      if (r) {
        onresultado(`Enviando la captura a ${d.nombre}.`, 'ok');
        onenviado();
      } else onresultado('Captura cancelada: no se envió nada.', 'aviso');
    } catch (e) {
      onresultado(String(e), 'aviso');
    }
  }

  async function abrirCarpeta() {
    try {
      await abrirCarpetaDrop();
    } catch (e) {
      onerror(String(e));
    }
  }

  let enlace = $state('');
  let abriendoEnlace = $state(false);
  async function abrirEnlace(evento: SubmitEvent) {
    evento.preventDefault();
    const d = destino;
    const texto = enlace.trim();
    if (!d || !texto || abriendoEnlace) return;
    // "tailscale.com" también sirve: se le pone https://.
    const url = /^https?:\/\//i.test(texto) ? texto : `https://${texto}`;
    abriendoEnlace = true;
    try {
      await enviarEnlaceDrop(d.ip, url);
      enlace = '';
      abierta = null;
      onresultado(`Listo: se abrió en ${d.nombre}.`, 'ok');
      onenviado();
    } catch (e) {
      onresultado(String(e), 'aviso');
    }
    abriendoEnlace = false;
  }

  let nota = $state('');
  let enviandoNota = $state(false);
  async function enviarNota(evento?: SubmitEvent) {
    evento?.preventDefault();
    const d = destino;
    if (!d || !nota.trim() || enviandoNota) return;
    enviandoNota = true;
    try {
      await enviarTextoDrop(d.ip, nota);
      nota = '';
      abierta = null;
      onresultado(`Nota enviada: ya está en el portapapeles de ${d.nombre}.`, 'ok');
      onenviado();
    } catch (e) {
      onresultado(String(e), 'aviso');
    }
    enviandoNota = false;
  }

  // ⌘↩ (Ctrl+Enter en Windows) manda la nota; Enter solo baja de línea.
  function teclaNota(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      enviarNota();
    }
  }

  const recepcion = $derived.by((): { texto: string; tono: 'ok' | 'aviso' | 'tenue' } => {
    const e = estado;
    if (!e) return { texto: 'Revisando la recepción…', tono: 'tenue' };
    if (!e.ajustes.recibir) return { texto: 'Recepción apagada', tono: 'aviso' };
    if (e.escuchando) return { texto: `Recibes en ${e.nombre}`, tono: 'ok' };
    return { texto: 'Esperando a Tailscale para recibir', tono: 'aviso' };
  });
</script>

<nav class="riel vidrio" class:con-campo={!!abierta && !!destino} aria-label="Acciones de XDrop">
  <p class="rotulo">Acciones</p>
  <ul class="lista">
    {#each acciones as a (a.id)}
      {@const apagada = a.necesitaDestino && !destino}
      {@const trabajando = ocupada === a.id}
      <li class="item" class:abierta={!!a.campo && abierta === a.campo}>
        <button
          class="fila"
          type="button"
          disabled={apagada || (!!ocupada && !a.campo)}
          aria-busy={trabajando}
          aria-expanded={a.campo ? abierta === a.campo : undefined}
          onclick={() => pulsar(a)}
        >
          <span class="ficha-icono chica" class:tenue={apagada} aria-hidden="true">
            {#if trabajando}<span class="girador"></span>{:else}<Icono nombre={a.icono} tamano={16} />{/if}
          </span>
          <span class="textos">
            <span class="titulo">{trabajando && a.ocupadoTexto ? a.ocupadoTexto : a.titulo}</span>
            <span class="ayuda">{a.ayuda}</span>
          </span>
          {#if a.campo}
            <span class="chevron" aria-hidden="true"><Icono nombre="chevron-abajo" tamano={15} /></span>
          {/if}
        </button>

        {#if a.campo === 'enlace' && abierta === 'enlace' && destino}
          <form class="campo-caja" onsubmit={abrirEnlace}>
            <label class="solo-lector" for="xdrop-enlace">Enlace para abrir en {otro}</label>
            <input
              bind:this={campoEnlace}
              id="xdrop-enlace"
              class="campo"
              type="text"
              inputmode="url"
              autocomplete="off"
              spellcheck="false"
              placeholder="https://"
              bind:value={enlace}
            />
            <button class="boton-primario" type="submit" disabled={!enlace.trim() || abriendoEnlace} aria-busy={abriendoEnlace}>
              Abrir allá
            </button>
          </form>
        {:else if a.campo === 'nota' && abierta === 'nota' && destino}
          <form class="campo-caja" onsubmit={enviarNota}>
            <label class="solo-lector" for="xdrop-nota">Nota para {otro}</label>
            <textarea
              bind:this={campoNota}
              id="xdrop-nota"
              class="campo area"
              rows="3"
              spellcheck="true"
              placeholder="Escribe algo para pegar allá"
              bind:value={nota}
              onkeydown={teclaNota}
            ></textarea>
            <div class="nota-pie">
              <span class="atajo">{esMac ? '⌘ ↩' : 'Ctrl ↩'} para enviar</span>
              <button class="boton-primario" type="submit" disabled={!nota.trim() || enviandoNota} aria-busy={enviandoNota}>
                Enviar nota
              </button>
            </div>
          </form>
        {/if}
      </li>
    {/each}
  </ul>

  <p class="recepcion {recepcion.tono}" role="status">
    <span class="punto" aria-hidden="true"></span>
    {recepcion.texto}
  </p>
</nav>

<style>
  .riel {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    padding: 10px 8px 8px;
    border-radius: var(--radio-lg);
  }
  .riel > .rotulo {
    margin: 0;
    padding: 2px 10px 6px;
  }
  .lista {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .item {
    border-radius: 14px;
    transition: background-color var(--dur-color) ease;
  }
  .item.abierta {
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
  }

  /* Cada fila: ícono, nombre y ayuda; todas del mismo ancho. El hover solo cambia el fondo. */
  .fila {
    position: relative;
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    min-height: 54px;
    padding: 8px 10px;
    border: 0;
    border-radius: 14px;
    background: transparent;
    color: var(--texto);
    text-align: left;
    transition: background-color var(--dur-color) ease;
  }
  .fila:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .fila:active:not(:disabled) {
    transform: scale(0.98);
  }
  .fila:disabled {
    cursor: default;
  }
  .fila:disabled .textos {
    opacity: 0.55;
  }
  .fila:focus-visible {
    box-shadow: inset 0 0 0 2px var(--primario);
  }
  .ficha-icono .girador {
    width: 15px;
    height: 15px;
  }
  .textos {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .titulo {
    font-size: 13.5px;
    line-height: 18px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .ayuda {
    font-size: 12px;
    line-height: 16px;
    color: var(--texto-2);
    overflow-wrap: anywhere;
  }
  .chevron {
    display: grid;
    flex-shrink: 0;
    color: var(--texto-3);
    transition: transform var(--dur-color) ease;
  }
  .abierta .chevron {
    transform: rotate(180deg);
  }

  /* El campo que se abre bajo su fila. */
  .campo-caja {
    display: grid;
    gap: 8px;
    margin: 0;
    padding: 2px 10px 10px;
  }
  .campo {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    min-width: 0;
    height: 36px;
    padding: 0 12px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-sm);
    outline: none;
    background: var(--superficie);
    color: var(--texto);
    font: inherit;
    font-size: 13.5px;
    user-select: text;
    -webkit-user-select: text;
    transition:
      border-color var(--dur-color) ease,
      box-shadow var(--dur-color) ease;
  }
  .campo.area {
    height: auto;
    min-height: 76px;
    padding: 9px 12px;
    line-height: 19px;
    resize: vertical;
  }
  .campo::placeholder {
    color: var(--texto-3);
  }
  .campo:focus {
    border-color: var(--primario);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--primario) 20%, transparent);
  }
  .nota-pie {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .atajo {
    font-size: 11.5px;
    color: var(--texto-3);
  }
  .boton-primario:disabled {
    cursor: default;
    opacity: 0.6;
  }

  /* Al pie: si este equipo está recibiendo. */
  .recepcion {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: auto 0 0;
    padding: 12px 10px 4px;
    border-top: 1px solid var(--borde);
    font-size: 12px;
    line-height: 16px;
    font-weight: 600;
    color: var(--texto-2);
    overflow-wrap: anywhere;
  }
  .punto {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--texto-3);
  }
  .recepcion.ok .punto {
    background: var(--exito);
  }
  .recepcion.aviso .punto {
    background: var(--aviso);
  }

  /* Cuando XDrop llena la ventana, el riel vive en un contenedor "riel" con alto fijo. Si sobra alto y no hay
     un campo abierto, las filas se reparten el alto como mosaicos, con separadores finos y el ícono encima del texto.
     Con un campo abierto vuelven a ser filas, así el campo siempre cabe. */
  @container riel (min-height: 560px) {
    .riel:not(.con-campo) .lista {
      flex: 1;
      gap: 0;
    }
    .riel:not(.con-campo) .item {
      flex: 1;
      display: flex;
      flex-direction: column;
      min-height: 0;
    }
    .riel:not(.con-campo) .item + .item {
      box-shadow: 0 -1px 0 var(--borde);
    }
    .riel:not(.con-campo) .fila {
      flex: 1;
      flex-direction: column;
      justify-content: center;
      gap: 7px;
      min-height: 80px;
      padding: 8px 14px;
      text-align: center;
    }
    .riel:not(.con-campo) .fila .ficha-icono {
      width: 38px;
      height: 38px;
      border-radius: 12px;
    }
    .riel:not(.con-campo) .textos {
      flex: 0 0 auto;
      justify-items: center;
    }
    .riel:not(.con-campo) .chevron {
      position: absolute;
      top: 12px;
      right: 12px;
    }
  }
</style>
