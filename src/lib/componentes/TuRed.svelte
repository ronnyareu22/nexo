<!--
  Tu red (en Casa): todo lo que está conectado a tu Wi-Fi, sin cuentas ni nube (≈4 s, a pedido; lo que ve queda
  solo en memoria y nada sigue escuchando).
  - Mientras analiza: una línea que se llena una sola vez en 4 s (sin bucles) y, en la Mac, cómo responder al aviso
    de red local de macOS. Después, cuántos equipos hay y la IP del router.
  - Tres grupos (los vacíos no se muestran): Se manejan con Nexo · Se pueden agregar · Otros equipos en tu red.
  - Cada equipo: su ícono, nombre, marca (o "Dirección privada"), IP, la MAC con copiar, cómo está y, si se puede, la
    acción: Agregar, Vincular (VincularAparato.svelte), Conectar Home Assistant o, si es de Tuya, Ver opciones.
    Lo que solo se ve dice por qué, en tenue.
  Empieza a analizar al aparecer; quien lo usa lo vuelve a crear para analizar de nuevo.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { agregarAparato, analizarRed } from '$lib/api-hogar';
  import { esMac } from '$lib/api-sistema';
  import type { EquipoRed, EstadoCasa, GrupoRed, Hallazgo, ResultadoRed, TipoEquipo } from '$lib/tipos-hogar';
  import { tooltip } from '$lib/tooltip';
  import Icono, { type Nombre } from './Icono.svelte';
  import OpcionesTuya from './OpcionesTuya.svelte';
  import VincularAparato from './VincularAparato.svelte';

  let {
    onestado,
    onir,
    onerror,
    oncerrar,
    onanalizar,
  }: {
    /** La casa cambió (tras analizar, agregar o vincular). */
    onestado: (e: EstadoCasa) => void;
    onir: (seccion: string, id?: string) => void;
    onerror: (texto: string) => void;
    oncerrar: () => void;
    /** Analizar de nuevo (quien lo usa lo vuelve a crear). */
    onanalizar: () => void;
  } = $props();

  const uid = $props.id();

  let analizando = $state(true);
  let lleno = $state(false);
  let resultado = $state<ResultadoRed | null>(null);
  let casa = $state<EstadoCasa | null>(null);
  let agregando = $state<string | null>(null);
  /** Lo agregado o vinculado desde aquí (ids de sus hallazgos): queda en su lugar con "Ya está en Nexo". */
  let agregados = $state<string[]>([]);
  let vinculando = $state<Hallazgo | null>(null);
  /** Las filas de Tuya con sus opciones abiertas (por IP). */
  let abiertas = $state<string[]>([]);
  let copiada = $state<string | null>(null);
  let temporizadorCopia: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    // Un solo llenado de 4 s: se pone en marcha en el cuadro siguiente.
    requestAnimationFrame(() => requestAnimationFrame(() => (lleno = true)));
    let vivo = true;
    analizarRed()
      .then((r) => {
        if (!vivo) return;
        resultado = r;
        cambio(r.estado);
      })
      .catch((e) => vivo && onerror(String(e)))
      .finally(() => vivo && (analizando = false));
    return () => {
      vivo = false;
      clearTimeout(temporizadorCopia);
    };
  });

  function cambio(e: EstadoCasa) {
    casa = e;
    onestado(e);
  }

  async function agregar(h: Hallazgo) {
    agregando = h.id;
    try {
      cambio(await agregarAparato(h.id));
      agregados = [...agregados, h.id];
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

  function alternarOpciones(ip: string) {
    abiertas = abiertas.includes(ip) ? abiertas.filter((x) => x !== ip) : [...abiertas, ip];
  }

  /** Copia un texto: el portapapeles de la vista web y, si no deja, el método de siempre. */
  async function copiarTexto(texto: string) {
    try {
      await navigator.clipboard.writeText(texto);
      return;
    } catch {
      // Sin permiso para el portapapeles: se copia desde un campo oculto.
    }
    const area = document.createElement('textarea');
    area.value = texto;
    area.setAttribute('readonly', '');
    area.style.position = 'fixed';
    area.style.opacity = '0';
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand('copy');
    area.remove();
    if (!ok) throw new Error('No pude copiar la MAC.');
  }

  async function copiarMac(mac: string) {
    try {
      await copiarTexto(mac);
      copiada = mac;
      clearTimeout(temporizadorCopia);
      temporizadorCopia = setTimeout(() => (copiada = null), 1800);
    } catch {
      onerror('No pude copiar la MAC.');
    }
  }

  // ---------------------------------------------------------------- Lo que se ve

  const ICONOS: Record<TipoEquipo, Nombre> = {
    router: 'router',
    esteEquipo: 'portatil',
    telefono: 'telefono',
    computadora: 'portatil',
    tele: 'tele',
    parlante: 'parlante',
    luz: 'bombilla',
    enchufe: 'enchufe',
    camara: 'camara',
    impresora: 'impresora',
    consola: 'consola',
    desconocido: 'pregunta',
  };

  const GRUPOS: { id: GrupoRed; titulo: string }[] = [
    { id: 'nexo', titulo: 'Se manejan con Nexo' },
    { id: 'agregar', titulo: 'Se pueden agregar' },
    { id: 'otros', titulo: 'Otros equipos en tu red' },
  ];

  /** El router y este equipo primero; luego, por IP. */
  const numero = (ip: string) => ip.split('.').reduce((n, x) => n * 256 + (Number(x) || 0), 0);
  const primero = (e: EquipoRed) => (e.tipo === 'router' ? 0 : e.tipo === 'esteEquipo' ? 1 : 2);

  const porGrupo = $derived.by(() => {
    const m: Record<GrupoRed, EquipoRed[]> = { nexo: [], agregar: [], otros: [] };
    for (const e of resultado?.equipos ?? []) m[e.grupo]?.push(e);
    for (const l of Object.values(m)) l.sort((a, b) => primero(a) - primero(b) || numero(a.ip) - numero(b.ip));
    return m;
  });

  const resumen = $derived.by(() => {
    if (analizando) return 'Mirando qué está conectado a tu Wi-Fi…';
    if (!resultado) return 'No pude analizar tu red.';
    const n = resultado.equipos.length;
    if (!n) return 'No vi otros equipos en tu red.';
    return [`${n} ${n === 1 ? 'equipo' : 'equipos'} en tu red`, resultado.router ? `router ${resultado.router}` : null]
      .filter(Boolean)
      .join(' · ');
  });

  /** Marca (o "Dirección privada"), IP y cómo se anuncia, en una línea. */
  function linea(e: EquipoRed): string {
    const marca = e.marca ?? (e.privada ? 'Dirección privada' : null);
    return [marca, e.ip, e.nombreRed].filter(Boolean).join(' · ');
  }

  /** Los nombres en Nexo de lo que hay en esa IP ("Key Light y Tira del escritorio"). */
  function enNexo(ids: string[]): string {
    const n = ids.map((id) => casa?.aparatos.find((a) => a.id === id)?.nombre).filter((x): x is string => !!x);
    return n.length < 2 ? (n[0] ?? '') : `${n.slice(0, -1).join(', ')} y ${n[n.length - 1]}`;
  }

  /** Qué falta para agregarlo, en corto. */
  function comoSeAgrega(h: Hallazgo): string {
    switch (h.manejo.tipo) {
      case 'listo':
        return 'Listo para agregar';
      case 'vincular':
        return h.manejo.forma === 'boton' ? 'Se vincula con su botón' : 'Pide su contraseña';
      case 'homeAssistant':
        return 'Home Assistant en tu red';
      default:
        return '';
    }
  }
</script>

{#snippet fila(e: EquipoRed)}
  {@const h = e.hallazgo}
  {@const hecho = !!h && agregados.includes(h.id)}
  {@const deTuya = h?.manejo.tipo === 'cuentaTuya'}
  {@const abierta = abiertas.includes(e.ip)}
  {@const nombres = e.grupo === 'nexo' ? enNexo(e.aparatos) : ''}
  {@const conAccion = e.grupo === 'nexo' || hecho || deTuya || (!!h && e.grupo === 'agregar')}
  <li class="equipo">
    <span
      class="ficha-icono chica"
      class:tenue={e.grupo === 'otros'}
      style:--tono={e.grupo !== 'otros' && e.tipo === 'luz' ? 'var(--amarillo)' : undefined}
      aria-hidden="true"
    >
      <Icono nombre={ICONOS[e.tipo] ?? 'pregunta'} tamano={16} />
    </span>
    <span class="textos">
      <span class="nombre">{e.nombre}</span>
      <span class="detalle tabular">{linea(e)}</span>
      {#if e.mac}
        <span class="mac">
          <span class="tabular">MAC {e.mac}</span>
          <button
            type="button"
            class="copiar"
            class:hecho={copiada === e.mac}
            aria-label="Copiar la MAC de {e.nombre}"
            use:tooltip={copiada === e.mac ? 'Copiada' : 'Copiar MAC'}
            onclick={() => e.mac && copiarMac(e.mac)}
          >
            <Icono nombre={copiada === e.mac ? 'check' : 'copiar'} tamano={13} />
            {#if copiada === e.mac}<span>Copiada</span>{/if}
          </button>
        </span>
      {/if}
      {#if nombres}<span class="detalle">En Nexo: {nombres}</span>{/if}
      {#if e.motivo || deTuya}
        <span class="motivo">{e.motivo ?? 'Solo funciona con la nube de Tuya.'}</span>
      {/if}
    </span>
    {#if conAccion}
      <span class="manejo">
        {#if e.grupo === 'nexo' || hecho}
          <span class="estado ok"><Icono nombre="check-circulo" tamano={15} />{hecho ? 'Ya está en Nexo' : 'En Nexo'}</span>
        {:else if h && e.grupo === 'agregar'}
          {#if h.manejo.tipo === 'activar'}
            <span class="estado aviso-texto">{h.manejo.paso}</span>
          {:else if comoSeAgrega(h)}
            <span class="estado">{comoSeAgrega(h)}</span>
          {/if}
          {#if h.manejo.tipo === 'listo'}
            <button
              type="button"
              class="boton-primario chico"
              disabled={agregando !== null}
              aria-busy={agregando === h.id}
              onclick={() => agregar(h)}
            >
              {agregando === h.id ? 'Agregando…' : 'Agregar'}
            </button>
          {:else if h.manejo.tipo === 'vincular'}
            <button type="button" class="boton-primario chico" onclick={() => (vinculando = h)}>Vincular</button>
          {:else if h.manejo.tipo === 'homeAssistant'}
            {@const url = h.manejo.url}
            <button type="button" class="boton chico" onclick={() => conectarHa(url)}>Conectar Home Assistant</button>
          {/if}
        {:else if deTuya}
          <button
            type="button"
            class="boton chico"
            aria-expanded={abierta}
            aria-controls="{uid}-tuya-{e.ip}"
            onclick={() => alternarOpciones(e.ip)}
          >
            Ver opciones
            <span class="giro" class:abierta><Icono nombre="chevron-abajo" tamano={14} /></span>
          </button>
        {/if}
      </span>
    {/if}
    {#if deTuya && abierta}
      <div class="opciones"><OpcionesTuya id="{uid}-tuya-{e.ip}" {onir} /></div>
    {/if}
  </li>
{/snippet}

<section class="red lamina" aria-labelledby="{uid}-titulo" aria-busy={analizando}>
  <header class="cabeza">
    <span class="ficha-icono" aria-hidden="true"><Icono nombre="red" tamano={18} /></span>
    <div class="t-textos">
      <h2 class="t-titulo" id="{uid}-titulo">{analizando ? 'Analizando tu red' : 'Tu red'}</h2>
      <p class="t-sub" role="status">{resumen}</p>
    </div>
    {#if !analizando}
      <button type="button" class="boton" onclick={onanalizar}>
        <Icono nombre="actualizar" tamano={16} />
        Analizar de nuevo
      </button>
    {/if}
    <button type="button" class="cerrar" aria-label="Cerrar Tu red" onclick={oncerrar}>
      <Icono nombre="cerrar" tamano={16} />
    </button>
  </header>

  {#if analizando}
    <div class="avance" aria-hidden="true"><span class:lleno></span></div>
    {#if esMac}
      <p class="nota">
        La primera vez, macOS pregunta si Nexo puede buscar aparatos en tu red local: elige «Permitir».
      </p>
    {/if}
  {:else if resultado}
    {#each resultado.avisos as a (a)}
      <p class="aviso"><Icono nombre="alerta" tamano={15} /><span>{a}</span></p>
    {/each}

    {#each GRUPOS as g (g.id)}
      {@const lista = porGrupo[g.id]}
      {#if lista.length}
        <section class="grupo" aria-labelledby="{uid}-{g.id}">
          <h3 class="rotulo" id="{uid}-{g.id}">{g.titulo} · {lista.length}</h3>
          <ul class="lista">
            {#each lista as e (`${e.ip}-${e.mac}`)}
              {@render fila(e)}
            {/each}
          </ul>
        </section>
      {/if}
    {/each}

    {#if !resultado.equipos.length}
      <p class="nota">Revisa que este equipo esté conectado al Wi-Fi de tu casa y analiza de nuevo.</p>
    {/if}
    <p class="nota pista">
      <Icono nombre="info" tamano={14} />
      <span>Nexo mira solo tu red local, desde este equipo, y no guarda lo que ve.</span>
    </p>
  {/if}
</section>

<VincularAparato
  hallazgo={vinculando}
  onestado={cambio}
  onvinculado={(id) => (agregados = [...agregados, id])}
  oncerrar={() => (vinculando = null)}
/>

<style>
  .red {
    container-type: inline-size;
    display: grid;
    gap: 12px;
    padding: 14px 16px 16px;
    border-radius: var(--radio-lg);
  }
  .tabular {
    font-variant-numeric: tabular-nums;
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

  /* Se llena una sola vez en 4 s: es lo que tarda el análisis. */
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
    transition: transform 4s linear;
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

  .grupo {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .grupo h3 {
    margin: 4px 0 0;
    padding: 0 4px;
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
  .equipo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    padding: 8px 10px;
    border-radius: var(--radio-sm);
  }
  .equipo + .equipo {
    box-shadow: 0 -1px 0 var(--borde);
  }
  .equipo > .ficha-icono {
    align-self: flex-start;
    margin-top: 2px;
  }
  .textos {
    display: grid;
    gap: 1px;
    flex: 1 1 240px;
    min-width: 0;
  }
  .nombre {
    font-size: 14px;
    line-height: 19px;
    font-weight: 600;
    overflow-wrap: break-word;
  }
  .detalle,
  .mac {
    font: var(--letra-pequena);
    color: var(--texto-2);
    overflow-wrap: anywhere;
  }
  .mac {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 6px;
    color: var(--texto-3);
  }
  .copiar {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 22px;
    padding: 0 6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--texto-3);
    font-size: 11.5px;
    font-weight: 600;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .copiar:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .copiar.hecho {
    color: var(--exito);
  }
  .motivo {
    font: var(--letra-pequena);
    color: var(--texto-3);
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
  .aviso-texto {
    max-width: 320px;
    color: var(--aviso);
  }
  .giro {
    display: inline-grid;
    transition: transform var(--dur-color) ease;
  }
  .giro.abierta {
    transform: rotate(180deg);
  }
  .opciones {
    flex: 1 1 100%;
    min-width: 0;
    padding-left: 42px;
  }

  /* Angosto: la acción baja debajo del texto, alineada con él. */
  @container (max-width: 520px) {
    .manejo {
      flex-basis: 100%;
      justify-content: flex-start;
      margin-left: 0;
      padding-left: 42px;
    }
    .estado {
      text-align: left;
    }
    .opciones {
      padding-left: 0;
    }
  }
</style>
