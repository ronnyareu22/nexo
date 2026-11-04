<!--
  Buscar aparatos (en Casa): una búsqueda corta en la red y por Bluetooth (≈6 s, a pedido; nada queda escuchando).
  - Mientras busca: una línea que se llena una sola vez en 6 s (sin bucles) y, en la Mac, cómo responder al aviso de
    red local de macOS.
  - Lo encontrado: su dibujo, nombre, marca y qué es, la IP o "Bluetooth", la señal en palabras y si Nexo lo maneja:
    "Lista para usar" con Agregar, "Ya está en Nexo", conectar Tuya o Home Assistant, lo que falta activar en su
    app, o por qué todavía no.
  Empieza a buscar al aparecer; quien lo usa lo vuelve a crear para buscar de nuevo.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { agregarAparato, buscarAparatos } from '$lib/api-hogar';
  import { esMac } from '$lib/api-sistema';
  import { tipoDe } from '$lib/hogar';
  import type { EstadoCasa, Hallazgo } from '$lib/tipos-hogar';
  import Icono from './Icono.svelte';
  import IlustracionAparato from './IlustracionAparato.svelte';

  let {
    onestado,
    onir,
    onerror,
    oncerrar,
    onbuscar,
  }: {
    /** La casa cambió (tras buscar o agregar). */
    onestado: (e: EstadoCasa) => void;
    onir: (seccion: string, id?: string) => void;
    onerror: (texto: string) => void;
    oncerrar: () => void;
    /** Buscar de nuevo (quien lo usa lo vuelve a crear). */
    onbuscar: () => void;
  } = $props();

  let buscando = $state(true);
  let lleno = $state(false);
  let hallazgos = $state<Hallazgo[]>([]);
  let avisos = $state<string[]>([]);
  let agregando = $state<string | null>(null);

  onMount(() => {
    // Un solo llenado de 6 s: se pone en marcha en el cuadro siguiente.
    requestAnimationFrame(() => requestAnimationFrame(() => (lleno = true)));
    let vivo = true;
    buscarAparatos()
      .then((r) => {
        if (!vivo) return;
        hallazgos = r.hallazgos;
        avisos = r.avisos;
        onestado(r.estado);
      })
      .catch((e) => vivo && onerror(String(e)))
      .finally(() => vivo && (buscando = false));
    return () => {
      vivo = false;
    };
  });

  async function agregar(h: Hallazgo) {
    agregando = h.id;
    try {
      onestado(await agregarAparato(h.id));
      hallazgos = hallazgos.map((x) => (x.id === h.id ? { ...x, agregado: true } : x));
    } catch (e) {
      onerror(String(e));
    }
    agregando = null;
  }

  function conectarHa(url: string) {
    try {
      sessionStorage.setItem('nexo-ha-url', url);
    } catch {
      // Sin almacenamiento: se escribe a mano en Ajustes.
    }
    onir('ajustes', 'casa');
  }

  const senal = (dbm: number | null) =>
    dbm === null ? null : dbm >= -60 ? 'Señal buena' : dbm >= -75 ? 'Señal regular' : 'Señal débil';

  /** Marca, qué es, dónde y la señal, en una línea. */
  function linea(h: Hallazgo): string {
    const que = h.clase === 'otro' ? null : tipoDe(h);
    const donde = h.via === 'bluetooth' ? 'Bluetooth' : h.direccion;
    return [h.marca, que, donde, senal(h.senal)].filter(Boolean).join(' · ');
  }

  // Primero lo que se puede usar ya; luego lo que pide algo; al final lo que solo se ve.
  const peso = (h: Hallazgo) =>
    h.manejo.tipo === 'listo' ? (h.agregado ? 1 : 0) : h.manejo.tipo === 'pronto' ? 3 : 2;
  const ordenados = $derived([...hallazgos].sort((a, b) => peso(a) - peso(b)));
</script>

<section class="busqueda lamina" aria-labelledby="busqueda-titulo" aria-busy={buscando}>
  <header class="cabeza">
    <span class="ficha-icono" aria-hidden="true"><Icono nombre="buscar" tamano={18} /></span>
    <div class="t-textos">
      <h2 class="t-titulo" id="busqueda-titulo">{buscando ? 'Buscando aparatos' : 'Lo que encontré'}</h2>
      <p class="t-sub" role="status">
        {buscando
          ? 'Buscando en tu red y por Bluetooth…'
          : hallazgos.length
            ? `${hallazgos.length} ${hallazgos.length === 1 ? 'aparato' : 'aparatos'} en tu red y cerca de ti.`
            : 'No encontré aparatos nuevos.'}
      </p>
    </div>
    {#if !buscando}
      <button type="button" class="boton" onclick={onbuscar}>
        <Icono nombre="actualizar" tamano={16} />
        Buscar de nuevo
      </button>
    {/if}
    <button type="button" class="cerrar" aria-label="Cerrar la búsqueda" onclick={oncerrar}>
      <Icono nombre="cerrar" tamano={16} />
    </button>
  </header>

  {#if buscando}
    <div class="avance" aria-hidden="true"><span class:lleno></span></div>
    {#if esMac}
      <p class="nota">
        La primera vez, macOS pregunta si Nexo puede buscar aparatos en tu red local: elige «Permitir».
      </p>
    {/if}
  {:else}
    {#each avisos as a (a)}
      <p class="aviso"><Icono nombre="alerta" tamano={15} /><span>{a}</span></p>
    {/each}

    {#if ordenados.length}
      <ul class="lista">
        {#each ordenados as h (h.id)}
          <li class="hallazgo">
            <span class="mini" aria-hidden="true">
              <IlustracionAparato
                clase={h.clase}
                forma={h.forma}
                encendido={h.estado?.encendido === true}
                color={null}
              />
            </span>
            <span class="textos">
              <span class="nombre">{h.nombre}</span>
              <span class="detalle">{linea(h)}</span>
            </span>
            <span class="manejo">
              {#if h.manejo.tipo === 'listo'}
                {#if h.agregado}
                  <span class="estado ok"><Icono nombre="check-circulo" tamano={15} />Ya está en Nexo</span>
                {:else}
                  <span class="estado ok">Lista para usar</span>
                  <button
                    type="button"
                    class="boton-primario chico"
                    disabled={agregando !== null}
                    aria-busy={agregando === h.id}
                    onclick={() => agregar(h)}
                  >
                    {agregando === h.id ? 'Agregando…' : 'Agregar'}
                  </button>
                {/if}
              {:else if h.manejo.tipo === 'cuentaTuya'}
                <span class="estado">Conecta tu cuenta Tuya para manejarla</span>
                <button type="button" class="boton chico" onclick={() => onir('ajustes', 'casa')}>Conectar Tuya</button>
              {:else if h.manejo.tipo === 'homeAssistant'}
                {@const url = h.manejo.url}
                {#if h.agregado}
                  <span class="estado ok"><Icono nombre="check-circulo" tamano={15} />Ya está conectado</span>
                {:else}
                  <span class="estado">Home Assistant en tu red</span>
                  <button type="button" class="boton chico" onclick={() => conectarHa(url)}>Conectar Home Assistant</button>
                {/if}
              {:else if h.manejo.tipo === 'activar'}
                <span class="estado aviso-texto">{h.manejo.paso}</span>
              {:else}
                <span class="estado tenue">{h.manejo.motivo}</span>
              {/if}
            </span>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="nota">No encontré aparatos nuevos. Revisa que estén encendidos y en la misma red Wi-Fi que este equipo.</p>
    {/if}
    <p class="nota pista">
      <Icono nombre="info" tamano={14} />
      <span>¿Tienes luces Yeelight o Govee? Activa «Control LAN» en su app para que Nexo las encuentre.</span>
    </p>
  {/if}
</section>

<style>
  .busqueda {
    display: grid;
    gap: 12px;
    padding: 14px 16px 16px;
    border-radius: var(--radio-lg);
  }
  .cabeza {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 12px;
  }
  .cabeza .t-textos {
    flex: 1 1 220px;
  }
  .boton {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-height: 34px;
    padding: 0 14px;
    border: 0;
    border-radius: var(--radio-sm);
    background: var(--relleno-tarjeta);
    box-shadow:
      inset 0 1px 0 var(--luz-tope),
      0 0 0 1px var(--borde-tarjeta);
    color: var(--texto);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .boton:hover {
    background: var(--superficie-hover);
  }
  .chico {
    min-height: 30px;
    padding: 0 12px;
    font-size: 12.5px;
  }
  .cerrar {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--texto-2);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .cerrar:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }

  /* Se llena una sola vez en 6 s: es lo que tarda la búsqueda. */
  .avance {
    height: 4px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--primario) 18%, transparent);
    overflow: hidden;
  }
  .avance span {
    display: block;
    height: 100%;
    background: var(--primario);
    transform: scaleX(0.02);
    transform-origin: left center;
    transition: transform 6s linear;
  }
  .avance span.lleno {
    transform: scaleX(1);
  }
  @media (prefers-reduced-motion: reduce) {
    .avance span {
      transition: none;
      transform: scaleX(0.5);
    }
  }

  .nota,
  .aviso {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
    overflow-wrap: break-word;
  }
  .nota :global(svg),
  .aviso :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
  }
  .pista :global(svg) {
    color: var(--info);
  }
  .aviso {
    color: var(--aviso);
  }

  .lista {
    display: grid;
    gap: 2px;
    margin: 0;
    padding: 4px;
    list-style: none;
    border-radius: var(--radio-md);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .hallazgo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    padding: 8px 10px;
    border-radius: var(--radio-sm);
  }
  .hallazgo + .hallazgo {
    box-shadow: 0 -1px 0 var(--borde);
  }
  .mini {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 44px;
    height: 44px;
    padding: 4px;
    border-radius: 12px;
    background: color-mix(in srgb, #fff 6%, var(--superficie-elevada));
  }
  .textos {
    display: grid;
    flex: 1 1 200px;
    min-width: 0;
  }
  .nombre {
    font-size: 14px;
    line-height: 19px;
    font-weight: 600;
    overflow-wrap: break-word;
  }
  .detalle {
    font: var(--letra-pequena);
    color: var(--texto-2);
    overflow-wrap: break-word;
  }
  .manejo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 6px 10px;
    flex: 0 1 auto;
    max-width: 100%;
    margin-left: auto;
  }
  .estado {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12.5px;
    line-height: 17px;
    font-weight: 600;
    color: var(--texto-2);
    text-align: right;
    overflow-wrap: break-word;
  }
  .estado.ok {
    color: var(--exito);
  }
  .estado.tenue {
    max-width: 320px;
    font-weight: 500;
    color: var(--texto-3);
  }
  .aviso-texto {
    max-width: 320px;
    color: var(--aviso);
  }
</style>
