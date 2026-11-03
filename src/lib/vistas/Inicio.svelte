<!--
  Inicio (DISENO.md, sección 7.1): saludo con Nexo Bot sobre una aurora turquesa, un resumen en 4 tarjetas
  que llevan a su sección, los primeros aparatos con su interruptor y la actividad reciente del historial real.
  La aurora son solo degradados quietos (sin desenfoque ni animación). Ningún texto se corta: todo baja de línea.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { Dispositivo, Estado, Pantalla, Registro } from '$lib/tipos';
  import { esDeSonido, nivelPrincipal, nombreDe } from '$lib/tipos';
  import { leerHistoriales } from '$lib/api';
  import { ahora, cuando, ultimaVez } from '$lib/tiempo';
  import FilaDispositivo from '$lib/componentes/FilaDispositivo.svelte';
  import NexoBot from '$lib/componentes/NexoBot.svelte';
  import Icono, { type Nombre } from '$lib/componentes/Icono.svelte';

  let {
    estado,
    pantallas,
    nombre,
    onir,
    onalternar,
    pendienteId,
  }: {
    estado: Estado | null;
    pantallas: Pantalla[];
    nombre: string;
    onir: (seccion: string, id?: string) => void;
    onalternar: (d: Dispositivo) => void;
    pendienteId: string | null;
  } = $props();

  const MAXIMO_FILAS = 4;
  const MAXIMO_EVENTOS = 6;

  const dispositivos = $derived(estado?.dispositivos ?? []);
  const conectados = $derived(dispositivos.filter((d) => d.conectado));
  const deSonido = $derived(dispositivos.filter(esDeSonido));
  const audifonos = $derived(deSonido.find((d) => d.conectado) ?? null);
  // La pantalla integrada manda; si no hay, el primer monitor que deje leer el brillo.
  const principal = $derived(
    pantallas.find((p) => p.interna && p.brillo !== null) ?? pantallas.find((p) => p.brillo !== null) ?? null,
  );

  // Primero los conectados, pero se muestran en el orden de siempre para que nada salte al conectar.
  const filas = $derived.by(() => {
    const elegidos = new Set(
      [...conectados, ...dispositivos.filter((d) => !d.conectado)].slice(0, MAXIMO_FILAS).map((d) => d.id),
    );
    return dispositivos.filter((d) => elegidos.has(d.id));
  });

  const plural = (n: number, uno: string, varios: string) => (n === 1 ? uno : varios);
  // Los mismos cortes que el anillo de batería: rojo, ámbar o verde.
  const tonoBateria = (n: number): Tono => (n <= 15 ? 'error' : n < 40 ? 'aviso' : 'exito');

  type Tono = 'exito' | 'aviso' | 'error' | 'primario' | 'tenue';

  // Cada tarjeta: ícono, rótulo, un valor (número grande y/o texto) y una línea de detalle.
  interface Tarjeta {
    id: string;
    rotulo: string;
    icono: Nombre;
    /** Línea chica sobre el valor, como "Conectado". */
    previo?: { texto: string; tono: Tono };
    /** Número grande, como "3" o "100 %". */
    numero?: { texto: string; tono: Tono };
    /** Texto del valor ("en total", "Activo"); con tono va en negrita y en color. */
    valor?: { texto: string; tono?: Tono };
    detalle: string;
    ir: () => void;
  }

  const tarjetas = $derived.by((): Tarjeta[] => {
    const nivel = audifonos ? nivelPrincipal(audifonos) : null;
    const bt = estado?.bluetoothEncendido ?? true;
    const n = dispositivos.length;

    const sonido: Tarjeta = {
      id: 'audifonos',
      rotulo: 'Audífonos',
      icono: 'audio',
      detalle: '',
      ir: () => onir('dispositivos', (audifonos ?? deSonido[0])?.id),
    };
    if (audifonos && nivel !== null) {
      sonido.previo = { texto: 'Conectado', tono: 'exito' };
      sonido.numero = { texto: `${nivel} %`, tono: tonoBateria(nivel) };
      sonido.detalle = nombreDe(audifonos);
    } else if (audifonos) {
      sonido.valor = { texto: 'Conectado', tono: 'exito' };
      sonido.detalle = nombreDe(audifonos);
    } else if (deSonido.length) {
      sonido.valor = { texto: 'Sin conectar', tono: 'tenue' };
      sonido.detalle = `${deSonido.length} ${plural(deSonido.length, 'emparejado', 'emparejados')}`;
    } else {
      sonido.valor = { texto: 'Ninguno', tono: 'tenue' };
      sonido.detalle = 'Sin audífonos emparejados';
    }

    return [
      sonido,
      {
        id: 'pantallas',
        rotulo: pantallas.length === 1 ? 'Pantalla' : 'Pantallas',
        icono: 'pantallas',
        ...(pantallas.length
          ? {
              numero: { texto: String(pantallas.length), tono: 'primario' as const },
              valor: { texto: plural(pantallas.length, 'activa', 'activas') },
            }
          : { valor: { texto: 'Ninguna', tono: 'tenue' as const } }),
        detalle: principal?.brillo != null ? `Brillo al ${principal.brillo} %` : 'Brillo no disponible',
        ir: () => onir('pantallas'),
      },
      {
        id: 'bluetooth',
        rotulo: 'Bluetooth',
        icono: 'bluetooth',
        valor: bt ? { texto: 'Activo', tono: 'exito' } : { texto: 'Apagado', tono: 'error' },
        detalle: bt ? `${n} ${plural(n, 'dispositivo', 'dispositivos')}` : 'Enciéndelo para conectar',
        ir: () => onir('dispositivos'),
      },
      {
        id: 'dispositivos',
        rotulo: 'Dispositivos',
        icono: 'dispositivos',
        ...(n
          ? { numero: { texto: String(n), tono: 'primario' as const }, valor: { texto: 'en total' } }
          : { valor: { texto: 'Ninguno', tono: 'tenue' as const } }),
        detalle: n
          ? `${conectados.length} ${plural(conectados.length, 'conectado', 'conectados')}`
          : 'Sin emparejar',
        ir: () => onir('dispositivos'),
      },
    ];
  });

  // --- Historial: para "usado ayer a las 21:10" y la actividad reciente ---
  let historiales = $state<Record<string, Registro>>({});
  const firma = $derived(dispositivos.map((d) => `${d.id}:${d.conectado ? 1 : 0}`).join('|'));
  async function cargarHistoriales() {
    const ids = untrack(() => dispositivos.map((d) => d.id));
    if (!ids.length) return;
    historiales = await leerHistoriales(ids);
  }
  $effect(() => {
    void firma;
    cargarHistoriales();
  });

  // "hace 12 min" avanza solo; el historial se relee cada minuto con el panel a la vista.
  let reloj = $state(ahora());
  $effect(() => {
    let vueltas = 0;
    const t = setInterval(() => {
      if (document.visibilityState !== 'visible') return;
      reloj = ahora();
      if (++vueltas % 2 === 0) cargarHistoriales();
    }, 30_000);
    return () => clearInterval(t);
  });

  interface Evento {
    clave: string;
    nombre: string;
    conecto: boolean;
    momento: number;
  }

  // Conexiones y desconexiones de las últimas 48 h, de la más nueva a la más vieja.
  const eventos = $derived.by((): Evento[] => {
    const desde = reloj - 48 * 3600;
    const lista: Evento[] = [];
    for (const d of dispositivos) {
      const r = historiales[d.id];
      if (!r) continue;
      for (const [inicio, fin] of r.sesiones) {
        if (inicio >= desde) lista.push({ clave: `${d.id}-${inicio}-c`, nombre: nombreDe(d), conecto: true, momento: inicio });
        if (fin !== null && fin >= desde)
          lista.push({ clave: `${d.id}-${fin}-d`, nombre: nombreDe(d), conecto: false, momento: fin });
      }
    }
    return lista.sort((a, b) => b.momento - a.momento).slice(0, MAXIMO_EVENTOS);
  });
  const textoMomento = (t: number) => (void reloj, cuando(t));
</script>

<div class="inicio">
  <header class="saludo">
    <div class="saludo__texto">
      <h1>{nombre ? `Hola, ${nombre}` : 'Hola'} <span aria-hidden="true">👋</span></h1>
      <p>Gestiona tus dispositivos de forma simple y rápida.</p>
    </div>
    <div class="saludo__bot"><NexoBot expresion="saludo" tamano={170} /></div>
  </header>

  <div class="resumen">
    {#each tarjetas as t (t.id)}
      <button class="tarjeta" onclick={t.ir} aria-busy={!estado}>
        <span class="tarjeta__icono"><Icono nombre={t.icono} tamano={26} /></span>
        <span class="tarjeta__rotulo">{t.rotulo}</span>
        {#if estado}
          {#if t.previo}
            <span class="tarjeta__previo {t.previo.tono}">{t.previo.texto}</span>
          {/if}
          <span class="tarjeta__valor">
            {#if t.numero}
              <span class="tarjeta__numero {t.numero.tono}">{t.numero.texto}</span>
            {/if}
            {#if t.valor}
              <span class="tarjeta__texto {t.valor.tono ?? ''}" class:fuerte={!!t.valor.tono}>{t.valor.texto}</span>
            {/if}
          </span>
          <span class="tarjeta__detalle">{t.detalle}</span>
        {:else}
          <span class="tarjeta__cargando"></span>
          <span class="tarjeta__cargando corto"></span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="columnas">
    <section class="seccion" aria-labelledby="inicio-dispositivos">
      <div class="seccion__cabeza">
        <h2 id="inicio-dispositivos">Tus dispositivos</h2>
        {#if dispositivos.length}
          <button class="enlace" onclick={() => onir('dispositivos')}>
            Ver todos <Icono nombre="chevron" tamano={14} />
          </button>
        {/if}
      </div>

      {#if !estado}
        <ul aria-busy="true" aria-label="Cargando dispositivos">
          {#each [0, 1, 2] as i (i)}
            <li class="fila-cargando"><span class="foto"></span><span class="lineas"><span></span><span></span></span></li>
          {/each}
        </ul>
      {:else if !estado.bluetoothEncendido}
        <p class="tranquilo">El Bluetooth está apagado. Enciéndelo para ver tus dispositivos.</p>
      {:else if !dispositivos.length}
        <p class="tranquilo">Todavía no hay dispositivos emparejados.</p>
      {:else}
        <ul>
          {#each filas as d (d.id)}
            <li>
              <FilaDispositivo
                dispositivo={d}
                pendiente={pendienteId === d.id}
                ultimaVez={historiales[d.id] ? ultimaVez(historiales[d.id].sesiones) : null}
                onabrir={() => onir('dispositivos', d.id)}
                onalternar={() => onalternar(d)}
              />
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section class="seccion" aria-labelledby="inicio-actividad">
      <div class="seccion__cabeza">
        <h2 id="inicio-actividad">Actividad reciente</h2>
      </div>
      {#if eventos.length}
        <ol class="actividad">
          {#each eventos as e (e.clave)}
            <li class="evento">
              <span class="evento__icono" class:conecto={e.conecto} aria-hidden="true">
                <Icono nombre={e.conecto ? 'conectar' : 'desconectar'} tamano={15} />
              </span>
              <span class="evento__texto"><b>{e.nombre}</b> {e.conecto ? 'se conectó' : 'se desconectó'}</span>
              <span class="evento__momento">{textoMomento(e.momento)}</span>
            </li>
          {/each}
        </ol>
      {:else if estado}
        <p class="tranquilo">Sin actividad en las últimas 48 horas.</p>
      {/if}
    </section>
  </div>
</div>

<style>
  /* Contexto propio: las auroras (z-index -1) quedan detrás del contenido y delante del fondo. */
  .inicio {
    container-type: inline-size;
    isolation: isolate;
    max-width: 1120px;
    margin: 0 auto;
    padding-bottom: 12px;
  }

  /* ---------- Saludo ---------- */
  .saludo {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    min-height: 170px;
  }
  /* Un velo muy suave en la parte derecha del saludo; se apaga antes de tocar los bordes. */
  .saludo::before {
    content: '';
    position: absolute;
    z-index: -1;
    top: -8px;
    bottom: -40px;
    left: 30%;
    right: 0;
    pointer-events: none;
    background: radial-gradient(
      closest-side at 72% 42%,
      color-mix(in srgb, var(--aurora-3) 60%, transparent),
      transparent
    );
  }
  .saludo__texto {
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-size: 33px;
    line-height: 40px;
    font-weight: 700;
    letter-spacing: -0.025em;
    overflow-wrap: break-word;
  }
  .saludo p {
    margin: 8px 0 0;
    font-size: 15px;
    line-height: 22px;
    color: var(--texto-2);
  }
  .saludo__bot {
    position: relative;
    flex-shrink: 0;
    display: flex;
    width: 170px;
    height: 170px;
    margin-right: 24px;
  }
  /* Aurora detrás de Nexo Bot: un arco azul que sube desde sus pies, el centro turquesa y un toque celeste
     a la izquierda. Cada degradado termina en transparente dentro de su caja, así no deja cortes. */
  .saludo__bot::before {
    content: '';
    position: absolute;
    z-index: -1;
    left: -135px;
    top: -8px;
    width: 400px;
    height: 320px;
    pointer-events: none;
    background:
      radial-gradient(
        ellipse 120px 105px at 55% 34%,
        var(--aurora-1),
        color-mix(in srgb, var(--aurora-1) 45%, transparent) 55%,
        transparent
      ),
      radial-gradient(
        ellipse 160px 150px at 50% 52%,
        transparent 48%,
        var(--aurora-2) 72%,
        transparent
      ),
      radial-gradient(ellipse 110px 80px at 28% 45%, var(--aurora-3), transparent);
  }

  /* ---------- Resumen: 4 tarjetas (2 si es angosto) ---------- */
  .resumen {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 14px;
    margin-top: 10px;
  }
  .tarjeta {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    min-width: 0;
    min-height: 120px;
    padding: 18px;
    border: 1px solid var(--borde);
    border-radius: 18px;
    background: var(--superficie);
    text-align: left;
    overflow-wrap: break-word;
    transition: background-color var(--dur-color) ease;
  }
  .tarjeta:hover {
    background: var(--superficie-hover);
  }
  .tarjeta:active {
    transform: scale(0.98);
  }
  .tarjeta > * {
    max-width: 100%;
  }
  .tarjeta__icono {
    display: flex;
    margin-bottom: 12px;
    color: var(--celeste);
  }
  .tarjeta__rotulo {
    font-size: 14.5px;
    line-height: 20px;
    font-weight: 600;
    color: var(--texto);
  }
  .tarjeta__previo {
    margin-top: 4px;
    font-size: 12.5px;
    line-height: 16px;
    font-weight: 600;
  }
  .tarjeta__valor {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    column-gap: 5px;
    margin-top: 2px;
    font-size: 15px;
    line-height: 26px;
    font-weight: 500;
    color: var(--texto);
  }
  .tarjeta__numero {
    font-size: 21px;
    font-weight: 700;
    letter-spacing: -0.01em;
    font-variant-numeric: tabular-nums;
  }
  .tarjeta__texto.fuerte {
    font-size: 16px;
    font-weight: 700;
  }
  .tarjeta__texto.tenue {
    font-weight: 600;
    color: var(--texto-2);
  }
  .tarjeta__detalle {
    margin-top: 2px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  /* Colores del valor */
  .exito {
    color: var(--exito);
  }
  .aviso {
    color: var(--aviso);
  }
  .error {
    color: var(--error);
  }
  .primario {
    color: var(--primario-texto);
  }
  .tarjeta__cargando {
    height: 12px;
    width: 70%;
    margin-top: 10px;
    border-radius: 6px;
    background: var(--superficie-elevada);
  }
  .tarjeta__cargando.corto {
    width: 45%;
  }

  /* ---------- Dispositivos y actividad: en dos columnas si hay espacio ---------- */
  .columnas {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 28px;
    margin-top: 28px;
  }
  @container (min-width: 860px) {
    .columnas {
      grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
      gap: 24px;
    }
  }
  @container (max-width: 620px) {
    .resumen {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  .seccion {
    min-width: 0;
  }
  .seccion__cabeza {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 28px;
    margin-bottom: 12px;
  }
  h2 {
    margin: 0;
    font-size: 17px;
    line-height: 24px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .enlace {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
    gap: 2px;
    height: 28px;
    padding: 0 6px 0 10px;
    border: 0;
    border-radius: var(--radio-pildora);
    background: transparent;
    color: var(--primario-texto);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .enlace:hover {
    background: var(--superficie-hover);
  }

  ul,
  ol {
    display: grid;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .tranquilo {
    margin: 0;
    padding: 14px 2px;
    font-size: 13px;
    color: var(--texto-2);
  }

  /* Actividad: una lista calma dentro de una sola superficie; el texto baja de línea si hace falta. */
  .actividad {
    gap: 0;
    padding: 6px 16px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-md);
    background: var(--superficie);
  }
  .evento {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 48px;
    padding: 9px 0;
    font-size: 13px;
    line-height: 19px;
  }
  .evento + .evento {
    border-top: 1px solid var(--borde);
  }
  .evento__icono {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--superficie-elevada);
    color: var(--texto-3);
  }
  .evento__icono.conecto {
    color: var(--exito);
    background: color-mix(in srgb, var(--exito) 14%, transparent);
  }
  .evento__texto {
    flex: 1;
    min-width: 0;
    overflow-wrap: break-word;
    color: var(--texto-2);
  }
  .evento__texto b {
    font-weight: 600;
    color: var(--texto);
  }
  .evento__momento {
    flex-shrink: 0;
    font-size: 12.5px;
    color: var(--texto-3);
    font-variant-numeric: tabular-nums;
  }

  /* ---------- Cargando ---------- */
  .fila-cargando {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 76px;
    padding: 10px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-md);
    background: var(--superficie);
    animation: respirar 1.4s ease-in-out infinite;
  }
  .fila-cargando .foto {
    width: 56px;
    height: 56px;
    border-radius: 14px;
    background: var(--superficie-elevada);
  }
  .lineas {
    display: grid;
    gap: 8px;
    flex: 1;
  }
  .lineas span {
    height: 10px;
    width: 46%;
    border-radius: 5px;
    background: var(--superficie-elevada);
  }
  .lineas span + span {
    width: 68%;
  }
  .tarjeta[aria-busy='true'] {
    animation: respirar 1.4s ease-in-out infinite;
  }
  @keyframes respirar {
    50% {
      opacity: 0.55;
    }
  }
</style>
