<!--
  Inicio (DISENO.md, sección 7.1):
  - saludo con la fecha y Nexo Bot (la ilustración oficial) sobre un arco de aurora turquesa y azul; sus pies quedan
    detrás de las tarjetas del resumen;
  - resumen en 4 tarjetas que llevan a su sección, cada una con su detalle chico: la pila de los audífonos, el brillo
    de la pantalla, un punto por aparato en Bluetooth y las fotos de los aparatos;
  - "Tus dispositivos": aparatos y pantallas en una sola tarjeta, con atajos propios (silenciar los audífonos,
    subir o bajar el brillo) además del interruptor para conectar;
  - "Actividad reciente" del historial real, agrupada por día.
  La aurora son solo degradados quietos (sin desenfoque ni animación). Ningún texto se corta: todo baja de línea.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { Dispositivo, Estado, Pantalla, Registro, Sonido } from '$lib/tipos';
  import { esDeSonido, nivelPrincipal, nombreDe } from '$lib/tipos';
  import { ajustarSonido, leerHistoriales, leerSonido, ponerBrillo } from '$lib/api';
  import { fotoDePantalla } from '$lib/catalogo';
  import { enviadorDelUltimo } from '$lib/enviador';
  import { ahora, cuando, hora, inicioDeHoy, ultimaVez } from '$lib/tiempo';
  import { tooltip } from '$lib/tooltip';
  import FilaDispositivo from '$lib/componentes/FilaDispositivo.svelte';
  import Figura from '$lib/componentes/Figura.svelte';
  import Ilustracion from '$lib/componentes/Ilustracion.svelte';
  import NexoBot from '$lib/componentes/NexoBot.svelte';
  import Icono, { type Nombre } from '$lib/componentes/Icono.svelte';
  import { colorBateria } from '$lib/componentes/PanelBateria.svelte';

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

  const MAXIMO_FILAS = 3;
  const MAXIMO_PANTALLAS = 2;
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
  const filasPantalla = $derived(pantallas.slice(0, MAXIMO_PANTALLAS));

  const plural = (n: number, uno: string, varios: string) => (n === 1 ? uno : varios);
  const capital = (t: string) => t.charAt(0).toUpperCase() + t.slice(1);
  // La fecha de hoy sobre el saludo: "Lunes, 3 de noviembre".
  const fecha = capital(new Date().toLocaleDateString('es', { weekday: 'long', day: 'numeric', month: 'long' }));

  type Tono = 'exito' | 'aviso' | 'error' | 'primario' | 'tenue';
  // Los mismos cortes que el anillo de batería: rojo, ámbar o verde.
  const tonoBateria = (n: number): Tono => (n <= 15 ? 'error' : n < 40 ? 'aviso' : 'exito');

  /** El detalle chico al pie de cada tarjeta. */
  type Micro =
    | { tipo: 'barra'; valor: number; color: string }
    | { tipo: 'puntos'; encendidos: boolean[] }
    | { tipo: 'fotos'; aparatos: Dispositivo[] };

  // Cada tarjeta: ícono, rótulo, un valor (número grande con su unidad y/o texto), una línea de detalle y su micro.
  interface Tarjeta {
    id: string;
    rotulo: string;
    icono: Nombre;
    /** Estado como un punto de color sobre la ficha del ícono ("Conectado", "Apagado"). */
    previo?: { texto: string; tono: Tono };
    numero?: { texto: string; tono: Tono };
    unidad?: string;
    /** Texto del valor ("en total", "Activo"); con tono va en negrita y en color. */
    valor?: { texto: string; tono?: Tono };
    detalle: string;
    micro: Micro;
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
      micro: { tipo: 'barra', valor: nivel ?? 0, color: nivel !== null ? colorBateria(nivel) : 'var(--texto-3)' },
      ir: () => onir('dispositivos', (audifonos ?? deSonido[0])?.id),
    };
    if (audifonos && nivel !== null) {
      sonido.previo = { texto: 'Conectado', tono: 'exito' };
      sonido.numero = { texto: String(nivel), tono: tonoBateria(nivel) };
      sonido.unidad = '%';
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
        micro: { tipo: 'barra', valor: principal?.brillo ?? 0, color: 'var(--amarillo)' },
        ir: () => onir('pantallas'),
      },
      {
        id: 'bluetooth',
        rotulo: 'Bluetooth',
        icono: 'bluetooth',
        previo: bt ? { texto: 'Encendido', tono: 'exito' } : { texto: 'Apagado', tono: 'error' },
        valor: bt ? { texto: 'Activo', tono: 'exito' } : { texto: 'Apagado', tono: 'error' },
        detalle: bt ? `${n} ${plural(n, 'dispositivo', 'dispositivos')}` : 'Enciéndelo para conectar',
        micro: { tipo: 'puntos', encendidos: bt ? dispositivos.map((d) => d.conectado) : [] },
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
        micro: { tipo: 'fotos', aparatos: dispositivos.slice(0, 5) },
        ir: () => onir('dispositivos'),
      },
    ];
  });

  // --- Sonido de los audífonos conectados: se lee una vez al conectarse, para el atajo de silenciar ---
  let sonidos = $state<Record<string, Sonido>>({});
  const firmaSonido = $derived(conectados.filter(esDeSonido).map((d) => d.id).join('|'));
  $effect(() => {
    const ids = firmaSonido ? firmaSonido.split('|') : [];
    untrack(() => leerSonidos(ids));
  });
  async function leerSonidos(ids: string[]) {
    const leidos = await Promise.allSettled(ids.map((id) => leerSonido(id)));
    const nuevos: Record<string, Sonido> = {};
    leidos.forEach((r, i) => {
      if (r.status === 'fulfilled') nuevos[ids[i]] = r.value;
    });
    sonidos = nuevos;
  }
  const conSilencio = (d: Dispositivo) =>
    d.conectado && esDeSonido(d) && !!sonidos[d.id]?.tieneSalida && sonidos[d.id].silenciado !== null;

  async function silenciar(d: Dispositivo) {
    const antes = sonidos[d.id];
    if (!antes || antes.silenciado === null) return;
    const valor = !antes.silenciado;
    sonidos[d.id] = { ...antes, silenciado: valor };
    try {
      sonidos[d.id] = await ajustarSonido(d.id, { accion: 'silenciar', valor });
    } catch {
      sonidos[d.id] = antes;
    }
  }

  // --- Brillo con − y +: se ve el valor nuevo al instante y se manda siempre el último ---
  let brillos = $state<Record<number, number>>({});
  const enviadores = new Map<number, (v: number) => void>();
  const soltar = new Map<number, ReturnType<typeof setTimeout>>();
  const brilloDe = (p: Pantalla) => brillos[p.id] ?? p.brillo;

  function pasoBrillo(p: Pantalla, paso: number) {
    const actual = brilloDe(p) ?? 0;
    const valor = Math.min(100, Math.max(0, Math.round((actual + paso) / 10) * 10));
    brillos[p.id] = valor;
    let enviar = enviadores.get(p.id);
    if (!enviar) {
      enviar = enviadorDelUltimo((v: number) => ponerBrillo(p.id, v));
      enviadores.set(p.id, enviar);
    }
    enviar(valor);
    // Al rato vuelve a mandar el valor que lee la ventana (que sigue a las teclas de brillo).
    clearTimeout(soltar.get(p.id));
    soltar.set(
      p.id,
      setTimeout(() => delete brillos[p.id], 1500),
    );
  }
  $effect(() => () => soltar.forEach((t) => clearTimeout(t)));

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

  // Conexiones y desconexiones de las últimas 48 h, de la más nueva a la más vieja, agrupadas por día.
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
  function dia(t: number) {
    const h = inicioDeHoy();
    if (t >= h) return 'Hoy';
    if (t >= h - 86400) return 'Ayer';
    return capital(new Date(t * 1000).toLocaleDateString('es', { weekday: 'long' }));
  }
  const grupos = $derived.by(() => {
    void reloj;
    const lista: { dia: string; eventos: Evento[] }[] = [];
    for (const e of eventos) {
      const d = dia(e.momento);
      const ultimo = lista[lista.length - 1];
      if (ultimo?.dia === d) ultimo.eventos.push(e);
      else lista.push({ dia: d, eventos: [e] });
    }
    return lista;
  });
  // Dentro de la hora, "hace 12 min"; después, solo la hora (el día ya va arriba).
  const textoMomento = (t: number) => (reloj - t < 3600 ? cuando(t) : hora(t));
</script>

<!-- Atajo de los audífonos conectados: silenciar o quitar el silencio, con el volumen a la vista. -->
{#snippet atajoSonido(d: Dispositivo)}
  {@const s = sonidos[d.id]}
  {#if s}
    <button
      class="atajo"
      class:apagado={s.silenciado}
      type="button"
      aria-pressed={s.silenciado ?? false}
      aria-label={s.silenciado ? `Quitar silencio a ${nombreDe(d)}` : `Silenciar ${nombreDe(d)}`}
      use:tooltip={s.silenciado ? 'Quitar silencio' : 'Silenciar'}
      onclick={() => silenciar(d)}
    >
      <Icono nombre={s.silenciado || s.volumen === 0 ? 'silencio' : 'volumen'} tamano={16} />
      <span>{s.silenciado ? 'Silencio' : s.volumen !== null ? `${s.volumen} %` : 'Sonido'}</span>
    </button>
  {/if}
{/snippet}

<div class="inicio">
  <!-- ---------- Saludo ---------- -->
  <header class="saludo">
    <div class="saludo__texto">
      <p class="saludo__fecha">{fecha}</p>
      <h1>{nombre ? `Hola, ${nombre}` : 'Hola'}</h1>
      <p class="saludo__sub">Gestiona tus dispositivos de forma simple y rápida.</p>
    </div>
    <div class="saludo__bot" aria-hidden="true">
      <NexoBot expresion="saludo" foto tamano={224} />
    </div>
  </header>

  <!-- ---------- Resumen ---------- -->
  <div class="resumen">
    {#each tarjetas as t (t.id)}
      <button class="kpi vidrio" onclick={t.ir} aria-busy={!estado}>
        <span class="kpi__cabeza">
          <span class="ficha-icono kpi__ficha" style:--tono="var(--celeste)">
            <Icono nombre={t.icono} tamano={20} />
            {#if estado && t.previo}<span class="kpi__punto {t.previo.tono}" aria-hidden="true"></span>{/if}
          </span>
          <span class="kpi__rotulo">{t.rotulo}</span>
          {#if estado && t.previo}<span class="solo-lector">{t.previo.texto}</span>{/if}
        </span>
        {#if estado}
          <span class="kpi__valor">
            {#if t.numero}
              <span class="kpi__numero {t.numero.tono}">{t.numero.texto}{#if t.unidad}<small>{t.unidad}</small>{/if}</span>
            {/if}
            {#if t.valor}
              <span class="kpi__texto {t.valor.tono ?? ''}" class:fuerte={!!t.valor.tono}>{t.valor.texto}</span>
            {/if}
          </span>
          <span class="kpi__detalle">{t.detalle}</span>
          <span class="kpi__micro" aria-hidden="true">
            {#if t.micro.tipo === 'barra'}
              <span class="micro-barra"><span style:width="{t.micro.valor}%" style:background={t.micro.color}></span></span>
            {:else if t.micro.tipo === 'puntos'}
              <span class="micro-puntos">
                {#each t.micro.encendidos as encendido, i (i)}<span class:encendido></span>{/each}
              </span>
            {:else}
              <span class="micro-fotos">
                {#each t.micro.aparatos as d (d.id)}
                  <span class="micro-foto" class:apagado={!d.conectado}><Figura dispositivo={d} sombra={false} /></span>
                {/each}
              </span>
            {/if}
          </span>
        {:else}
          <span class="kpi__cargando"></span>
          <span class="kpi__cargando corto"></span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="columnas">
    <!-- ---------- Tus dispositivos ---------- -->
    <section class="bloque vidrio" aria-labelledby="inicio-dispositivos">
      <header class="t-cabeza bloque__cabeza">
        <span class="ficha-icono"><Icono nombre="dispositivos" tamano={19} /></span>
        <div class="t-textos">
          <h2 class="t-titulo" id="inicio-dispositivos">Tus dispositivos</h2>
          <p class="t-sub">
            {#if !estado}Cargando…{:else if dispositivos.length}{conectados.length} de {dispositivos.length}
              {plural(conectados.length, 'conectado', 'conectados')}{:else}Ninguno emparejado{/if}
          </p>
        </div>
        {#if dispositivos.length}
          <button class="enlace" onclick={() => onir('dispositivos')}>
            Ver todos <Icono nombre="chevron" tamano={14} />
          </button>
        {/if}
      </header>

      {#if !estado}
        <ul class="filas" aria-busy="true" aria-label="Cargando dispositivos">
          {#each [0, 1, 2] as i (i)}
            <li class="fila-cargando"><span class="foto"></span><span class="lineas"><span></span><span></span></span></li>
          {/each}
        </ul>
      {:else}
        {#if !estado.bluetoothEncendido}
          <p class="tranquilo">El Bluetooth está apagado. Enciéndelo para ver tus dispositivos.</p>
        {:else if !dispositivos.length}
          <p class="tranquilo">Todavía no hay dispositivos emparejados.</p>
        {:else}
          <ul class="filas">
            {#each filas as d (d.id)}
              <li>
                <FilaDispositivo
                  dispositivo={d}
                  agrupada
                  pendiente={pendienteId === d.id}
                  ultimaVez={historiales[d.id] ? ultimaVez(historiales[d.id].sesiones) : null}
                  acciones={conSilencio(d) ? atajoSonido : undefined}
                  onabrir={() => onir('dispositivos', d.id)}
                  onalternar={() => onalternar(d)}
                />
              </li>
            {/each}
          </ul>
        {/if}

        {#if filasPantalla.length}
          <ul class="filas pantallas" aria-label="Pantallas">
            {#each filasPantalla as p (p.id)}
              {@const brillo = brilloDe(p)}
              {@const foto = fotoDePantalla(p)}
              <li>
                <div class="pantalla">
                  <button class="pantalla__abrir" onclick={() => onir('pantallas')} aria-label="Ver {p.nombre}"></button>
                  <span class="pantalla__foto" aria-hidden="true">
                    {#if foto}
                      <img src={foto} alt="" draggable="false" decoding="async" />
                    {:else}
                      <Ilustracion tipo={p.interna ? 'portatil' : 'monitor'} encendido />
                    {/if}
                  </span>
                  <span class="pantalla__textos">
                    <span class="pantalla__nombre">{p.nombre}</span>
                    <span class="pantalla__linea">
                      <span class="punto" aria-hidden="true"></span>
                      {p.interna ? 'Pantalla integrada' : p.virtual ? 'Pantalla virtual' : 'Monitor externo'}
                    </span>
                  </span>
                  {#if p.controlable && brillo !== null}
                    <span class="paso" role="group" aria-label="Brillo de {p.nombre}">
                      <button type="button" aria-label="Bajar brillo" use:tooltip={'Bajar brillo'} disabled={brillo <= 0} onclick={() => pasoBrillo(p, -10)}>
                        <Icono nombre="sol-bajo" tamano={16} />
                      </button>
                      <span class="paso__valor" aria-live="polite">{brillo} %</span>
                      <button type="button" aria-label="Subir brillo" use:tooltip={'Subir brillo'} disabled={brillo >= 100} onclick={() => pasoBrillo(p, 10)}>
                        <Icono nombre="sol" tamano={16} />
                      </button>
                    </span>
                  {:else}
                    <span class="pantalla__nota">{p.virtual ? 'Para RustDesk' : 'Brillo no disponible'}</span>
                  {/if}
                  <span class="chevron" aria-hidden="true"><Icono nombre="chevron" tamano={16} /></span>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
    </section>

    <!-- ---------- Actividad reciente ---------- -->
    <section class="bloque vidrio" aria-labelledby="inicio-actividad">
      <header class="t-cabeza bloque__cabeza">
        <span class="ficha-icono" style:--tono="var(--info)"><Icono nombre="reloj" tamano={19} /></span>
        <div class="t-textos">
          <h2 class="t-titulo" id="inicio-actividad">Actividad reciente</h2>
          <p class="t-sub">Conexiones de las últimas 48 horas</p>
        </div>
      </header>
      {#if grupos.length}
        <div class="actividad">
          {#each grupos as g (g.dia)}
            <div class="dia">
              <span class="rotulo">{g.dia}</span>
              <ol>
                {#each g.eventos as e (e.clave)}
                  <li class="evento">
                    <span class="evento__icono" class:conecto={e.conecto} aria-hidden="true">
                      <Icono nombre={e.conecto ? 'conectar' : 'desconectar'} tamano={14} />
                    </span>
                    <span class="evento__texto"><b>{e.nombre}</b> {e.conecto ? 'se conectó' : 'se desconectó'}</span>
                    <span class="evento__momento">{textoMomento(e.momento)}</span>
                  </li>
                {/each}
              </ol>
            </div>
          {/each}
        </div>
      {:else if estado}
        <div class="vacio">
          <span class="ficha-icono tenue"><Icono nombre="reloj" tamano={19} /></span>
          <p>Sin actividad en las últimas 48 horas.</p>
        </div>
      {/if}
    </section>
  </div>
</div>

<style>
  /* Contexto propio: las auroras (z-index -1) quedan detrás del contenido y delante del fondo. */
  .inicio {
    container-type: inline-size;
    isolation: isolate;
    display: flex;
    flex-direction: column;
    gap: 24px;
    max-width: 1180px;
    margin: 0 auto;
    padding-bottom: 4px;
  }

  /* ---------- Saludo ---------- */
  .saludo {
    position: relative;
    display: flex;
    align-items: center;
    min-height: 156px;
    padding: 4px 300px 16px 4px;
  }
  /* Arco de aurora detrás de Nexo Bot: una loma turquesa que sube desde las tarjetas, el canto azul del arco
     y un toque celeste a la izquierda. Cada degradado termina en transparente dentro de su caja. */
  /* La caja queda dentro de la vista (arriba no se recorta) y cada degradado se apaga antes de sus bordes. */
  .saludo::before {
    content: '';
    position: absolute;
    z-index: -1;
    right: -32px;
    bottom: -40px;
    width: 540px;
    height: 200px;
    pointer-events: none;
    background:
      radial-gradient(150px 132px at 324px 100%, var(--aurora-1), var(--aurora-1) 30%, transparent),
      radial-gradient(150px 132px at 324px 100%, var(--aurora-1), transparent),
      radial-gradient(210px 192px at 324px 100%, transparent 66%, var(--aurora-2) 80%, transparent 99%),
      radial-gradient(200px 170px at 324px 100%, var(--aurora-3), transparent),
      radial-gradient(120px 80px at 110px 82%, var(--aurora-3), transparent);
  }
  .saludo__texto {
    min-width: 0;
  }
  .saludo__fecha {
    margin: 0 0 6px;
    font: var(--letra-rotulo);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--primario-texto);
  }
  h1 {
    margin: 0;
    font: var(--letra-portada);
    font-size: 34px;
    line-height: 40px;
    letter-spacing: -0.03em;
    overflow-wrap: break-word;
  }
  .saludo__sub {
    margin: 8px 0 0;
    font-size: 15px;
    line-height: 22px;
    color: var(--texto-2);
  }
  /* Nexo Bot: sus pies quedan detrás de las tarjetas del resumen (que son opacas). */
  .saludo__bot {
    position: absolute;
    z-index: 0;
    right: 72px;
    bottom: -58px;
    width: 224px;
    height: 224px;
    display: flex;
    pointer-events: none;
  }

  /* ---------- Resumen: 4 tarjetas (2 si es angosto) ---------- */
  .resumen {
    position: relative;
    z-index: 1;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 16px;
  }
  .kpi {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    min-width: 0;
    padding: 16px 18px 18px;
    border: 0;
    border-radius: 20px;
    text-align: left;
    overflow-wrap: break-word;
    transition: background-color var(--dur-color) ease;
  }
  .kpi:hover {
    background: var(--superficie-hover);
  }
  .kpi:active {
    transform: scale(0.98);
  }
  .kpi__cabeza {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 36px;
    margin-bottom: 14px;
  }
  .kpi__rotulo {
    flex: 1;
    min-width: 0;
    font-size: 14px;
    line-height: 19px;
    font-weight: 600;
    color: var(--texto);
  }
  /* El estado, como un punto sobre la esquina de la ficha (con un aro del color de la tarjeta). */
  .kpi__ficha {
    position: relative;
  }
  .kpi__punto {
    position: absolute;
    top: -3px;
    right: -3px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: currentColor;
    box-shadow: 0 0 0 2.5px var(--superficie-tope);
  }
  .kpi__valor {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    column-gap: 6px;
    min-height: 36px;
    font-size: 15px;
    line-height: 36px;
    font-weight: 500;
    color: var(--texto-2);
  }
  .kpi__numero {
    font-size: 32px;
    font-weight: 700;
    letter-spacing: -0.035em;
    font-variant-numeric: tabular-nums;
  }
  .kpi__numero small {
    margin-left: 2px;
    font-size: 17px;
    font-weight: 650;
    letter-spacing: 0;
    opacity: 0.8;
  }
  .kpi__texto.fuerte {
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .kpi__texto.tenue {
    font-size: 18px;
    color: var(--texto-2);
  }
  .kpi__detalle {
    margin-top: 2px;
    font: var(--letra-pequena);
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  /* El micro detalle va siempre al pie, a la misma altura en las cuatro tarjetas. */
  .kpi__micro {
    display: flex;
    align-items: center;
    min-height: 22px;
    margin-top: auto;
    padding-top: 14px;
  }
  .micro-barra {
    flex: 1;
    height: 5px;
    overflow: hidden;
    border-radius: 99px;
    background: var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .micro-barra > span {
    display: block;
    height: 100%;
    border-radius: inherit;
  }
  .micro-puntos {
    display: flex;
    gap: 6px;
  }
  .micro-puntos > span {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde-fuerte);
  }
  .micro-puntos > span.encendido {
    background: var(--exito);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--exito) 20%, transparent);
  }
  .micro-fotos {
    display: flex;
  }
  /* Fotos chicas encimadas, como avatares; las de aparatos desconectados, tenues. */
  .micro-foto {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    margin-right: -6px;
    padding: 3px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--texto) 9%, var(--superficie-inserto));
    box-shadow:
      0 0 0 2px var(--superficie-base),
      inset 0 0 0 1px var(--borde);
  }
  .micro-foto.apagado :global(img),
  .micro-foto.apagado :global(svg) {
    opacity: 0.55;
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
  .kpi__cargando {
    height: 12px;
    width: 70%;
    margin-top: 12px;
    border-radius: 6px;
    background: var(--superficie-elevada);
  }
  .kpi__cargando.corto {
    width: 45%;
  }

  /* ---------- Dispositivos y actividad: dos columnas de igual alto si hay espacio ---------- */
  .columnas {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  @container (min-width: 860px) {
    .columnas {
      grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr);
      align-items: stretch;
    }
  }
  @container (max-width: 720px) {
    .resumen {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .saludo {
      padding-right: 200px;
    }
    .saludo__bot {
      right: 8px;
      bottom: -40px;
      width: 180px;
      height: 180px;
    }
    .saludo__bot :global(.nexo-bot) {
      width: 180px;
      height: 180px;
    }
    /* El arco, más chico y centrado en el bot más chico. */
    .saludo::before {
      width: 400px;
      height: 170px;
      background:
        radial-gradient(110px 100px at 270px 100%, var(--aurora-1), transparent),
        radial-gradient(125px 150px at 270px 100%, transparent 66%, var(--aurora-2) 80%, transparent 99%),
        radial-gradient(130px 130px at 270px 100%, var(--aurora-3), transparent);
    }
  }

  .bloque {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 18px 12px 12px;
    border-radius: var(--radio-lg);
  }
  .bloque__cabeza {
    padding: 0 6px 4px;
  }
  .enlace {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
    gap: 2px;
    height: 30px;
    padding: 0 8px 0 12px;
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
    margin: 0;
    padding: 0;
    list-style: none;
  }
  /* Filas de la tarjeta: separadas por una línea fina que empieza después de la foto. */
  .filas {
    display: grid;
  }
  .filas > li {
    position: relative;
  }
  .filas > li + li::before {
    content: '';
    position: absolute;
    top: 0;
    left: 82px;
    right: 8px;
    border-top: 1px solid var(--borde);
  }
  .filas.pantallas {
    margin-top: -4px;
    padding-top: 4px;
    border-top: 1px solid var(--borde);
  }

  .tranquilo {
    margin: 0;
    padding: 14px 8px;
    font-size: 13px;
    color: var(--texto-2);
  }

  /* Atajo de sonido dentro de la fila: píldora con el ícono y el volumen. */
  .atajo {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 11px 0 9px;
    border: 0;
    border-radius: 99px;
    background: color-mix(in srgb, var(--celeste) 12%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--celeste) 20%, transparent);
    color: var(--celeste);
    font-size: 12.5px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .atajo:hover {
    background: color-mix(in srgb, var(--celeste) 20%, transparent);
  }
  .atajo:active {
    transform: scale(0.98);
  }
  .atajo.apagado {
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-2);
  }

  /* ---------- Fila de una pantalla (mismo ritmo que las de aparatos) ---------- */
  .pantalla {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 68px;
    padding: 8px 10px 8px 6px;
    border-radius: 14px;
    transition: background-color var(--dur-color) ease;
  }
  .pantalla:has(.pantalla__abrir:hover) {
    background: var(--superficie-hover);
  }
  .pantalla__abrir {
    position: absolute;
    inset: 0;
    z-index: 1;
    padding: 0;
    border: 0;
    border-radius: inherit;
    background: transparent;
  }
  .pantalla__foto {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 64px;
    height: 52px;
    pointer-events: none;
  }
  .pantalla__foto img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .pantalla__textos {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 2px;
    pointer-events: none;
  }
  .pantalla__nombre {
    font-size: 14px;
    line-height: 20px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .pantalla__linea {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .pantalla__linea .punto {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--amarillo);
    box-shadow: 0 0 0 2.5px color-mix(in srgb, var(--amarillo) 20%, transparent);
  }
  .pantalla__nota {
    flex-shrink: 0;
    font-size: 12.5px;
    color: var(--texto-3);
  }
  /* Brillo − y +: un solo bloque con el valor al medio. */
  .paso {
    position: relative;
    z-index: 2;
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
    height: 32px;
    padding: 2px;
    border-radius: 99px;
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .paso button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--amarillo-texto);
    transition: background-color var(--dur-color) ease;
  }
  .paso button:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .paso button:active:not(:disabled) {
    transform: scale(0.94);
  }
  .paso button:disabled {
    color: var(--texto-3);
    cursor: default;
  }
  .paso__valor {
    min-width: 44px;
    text-align: center;
    font-size: 12.5px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .chevron {
    display: grid;
    flex-shrink: 0;
    color: var(--texto-3);
  }

  /* ---------- Actividad: agrupada por día, con una línea de tiempo entre los íconos ---------- */
  .actividad {
    display: grid;
    gap: 6px;
    padding: 0 8px;
  }
  .dia {
    display: grid;
    gap: 2px;
  }
  .dia .rotulo {
    padding: 4px 0 2px;
  }
  .evento {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 44px;
    padding: 6px 0;
    font-size: 13px;
    line-height: 19px;
  }
  /* La línea de tiempo: une el ícono con el siguiente del mismo día. */
  .evento + .evento::before {
    content: '';
    position: absolute;
    left: 14px;
    top: -14px;
    height: 24px;
    border-left: 1.5px solid var(--borde-fuerte);
    opacity: 0.6;
  }
  .evento__icono {
    position: relative;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 29px;
    height: 29px;
    border-radius: 50%;
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-3);
  }
  .evento__icono.conecto {
    color: var(--exito);
    background: color-mix(in srgb, var(--exito) 13%, var(--superficie-base));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--exito) 24%, transparent);
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
    padding: 2px 8px;
    border-radius: 99px;
    background: var(--superficie-inserto);
    font-size: 12px;
    line-height: 16px;
    font-weight: 500;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .vacio {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 24px 12px;
    text-align: center;
  }
  .vacio p {
    margin: 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
  }

  /* ---------- Cargando ---------- */
  .fila-cargando {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 68px;
    padding: 8px;
    animation: respirar 1.4s ease-in-out infinite;
  }
  .fila-cargando .foto {
    width: 56px;
    height: 48px;
    border-radius: 12px;
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
  .kpi[aria-busy='true'] {
    animation: respirar 1.4s ease-in-out infinite;
  }
  @keyframes respirar {
    50% {
      opacity: 0.55;
    }
  }
</style>
