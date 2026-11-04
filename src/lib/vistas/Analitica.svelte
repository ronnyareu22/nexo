<!--
  Analítica: cómo está cada equipo ahora mismo, sobre todo la Mac de la casa vista desde el PC de la oficina.
  Arriba, el selector del equipo ("Esta PC" / "Mi Mac · demo-sige"): de entrada, el otro equipo si responde.
  Solo están los equipos con Nexo abierto y el del perfil (perfil.svelte.ts), aunque duerma. Con este equipo solo,
  su nombre a secas y un enlace discreto a la guía de Remoto para conectar el otro.
  Luego seis tarjetas: primero las que tienen tendencia de 5 minutos (CPU, memoria y red, con bajada, subida y lo
  que pasa por Tailscale) y después las cortas (temperatura en palabras, batería y disco), así cada fila queda pareja.
  Debajo, las apps que más gastan (CPU y memoria) y la sesión remota (RustDesk, pantalla virtual, XDrop, reposo)
  con el tiempo encendida; con la ventana muy ancha, todo eso va en una sola fila.
  Se pide cada 2 s mientras la sección y la ventana están a la vista; Nexo mide solo mientras alguien pide.
  Si el otro equipo no responde, se dice con calma y queda lo último que se vio, tenue.
  Gráficas en SVG a mano (Tendencia.svelte). Superficies sólidas y ningún texto se corta: todo baja de línea.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { leerAnaliticaLocal, leerAnaliticaRemota, leerEquiposAnalitica } from '$lib/api-analitica';
  import type { Analitica, EquipoAnalitica, EstadoTermico, ProcesoApp } from '$lib/tipos-analitica';
  import Icono, { type Nombre } from '$lib/componentes/Icono.svelte';
  import Tendencia from '$lib/componentes/Tendencia.svelte';
  import { perfil, recargar } from '$lib/perfil.svelte';

  let { onir }: { /** Lleva a otra sección (el enlace a la guía, en Remoto). */ onir?: (seccion: string) => void } =
    $props();

  const CADA = 2000;
  const EQUIPOS_CADA = 15000;

  let equipos = $state<EquipoAnalitica[]>([]);
  /** 'local' o la dirección del otro equipo. */
  let clave = $state<string | null>(null);
  let elegidoAMano = false;
  let datos = $state<Record<string, Analitica>>({});
  let recibido = $state<Record<string, number>>({});
  let fallos = $state<Record<string, string>>({});
  const ocupados = new Set<string>();
  let vueltas = 0;

  const claveDe = (e: EquipoAnalitica) => (e.propio ? 'local' : (e.ip ?? e.nombre));
  // Este equipo, los que tienen Nexo abierto y el del perfil (tu Mac o tu PC, aunque duerma).
  const visibles = $derived(
    equipos.filter((e) => e.propio || e.enLinea || (!!perfil.otro && e.ip === perfil.otro.ip)),
  );
  const elegido = $derived(visibles.find((e) => claveDe(e) === clave) ?? null);
  const foto = $derived(clave ? (datos[clave] ?? null) : null);
  const fallo = $derived(clave ? (fallos[clave] ?? null) : null);
  const esMac = $derived((foto?.plataforma ?? elegido?.plataforma) === 'mac');

  // ------------------------------------------------------------ Equipos y datos

  /** "Esta Mac", "Esta PC", "Mi Mac · demo-sige", "Mi PC · innotech1017". */
  function etiqueta(e: EquipoAnalitica): string {
    const tipo = e.plataforma === 'mac' ? 'Mac' : e.plataforma === 'windows' ? 'PC' : 'computadora';
    return e.propio ? `Esta ${tipo}` : `Mi ${tipo} · ${e.nombre}`;
  }

  /** Nexo respondió allá en la última búsqueda y el último pedido no falló. */
  const responde = (e: EquipoAnalitica) => e.enLinea && !fallos[claveDe(e)];

  /** "la Mac" o "el PC". */
  const articulo = (e: EquipoAnalitica | null) => (e?.plataforma === 'mac' ? 'la Mac' : 'el PC');

  async function cargarEquipos() {
    try {
      const lista = await leerEquiposAnalitica();
      equipos = lista;
      // De entrada (y hasta que se elija a mano), el otro equipo si responde: para eso es esta sección.
      const otro = lista.find((e) => !e.propio && e.enLinea);
      const sigue = lista.some((e) => claveDe(e) === clave);
      if (!elegidoAMano || !sigue) {
        const nueva = otro ? claveDe(otro) : 'local';
        if (nueva !== clave) {
          clave = nueva;
          refrescar();
        }
      }
    } catch {
      if (!clave) {
        clave = 'local';
        refrescar();
      }
    }
  }

  async function refrescar() {
    const c = clave;
    if (!c || ocupados.has(c)) return;
    ocupados.add(c);
    try {
      const d = c === 'local' ? await leerAnaliticaLocal() : await leerAnaliticaRemota(c);
      datos[c] = d;
      recibido[c] = Date.now();
      delete fallos[c];
    } catch (e) {
      fallos[c] = String(e);
    } finally {
      ocupados.delete(c);
    }
  }

  function elegir(e: EquipoAnalitica) {
    elegidoAMano = true;
    clave = claveDe(e);
    refrescar();
  }

  // Si el equipo elegido ya no está en la lista (se fue sin ser el del perfil), vuelve a este.
  $effect(() => {
    if (clave && clave !== 'local' && equipos.length && !visibles.some((e) => claveDe(e) === clave)) {
      clave = 'local';
      refrescar();
    }
  });

  onMount(() => {
    recargar();
    cargarEquipos();
    const aLaVista = () => document.visibilityState === 'visible';
    // Cada 2 s mientras se ve. Si el otro equipo no responde, se prueba cada 6 s.
    const datosCada = setInterval(() => {
      vueltas++;
      if (!aLaVista()) return;
      if (clave && fallos[clave] && vueltas % 3) return;
      refrescar();
    }, CADA);
    const equiposCada = setInterval(() => {
      if (aLaVista()) cargarEquipos();
    }, EQUIPOS_CADA);
    const alVolver = () => {
      if (aLaVista()) refrescar();
    };
    document.addEventListener('visibilitychange', alVolver);
    return () => {
      clearInterval(datosCada);
      clearInterval(equiposCada);
      document.removeEventListener('visibilitychange', alVolver);
    };
  });

  // ------------------------------------------------------------ Números en palabras

  const NB = ' ';
  const num = (v: number, dec = 0) =>
    v.toLocaleString('es', { minimumFractionDigits: dec, maximumFractionDigits: dec });
  /** 6,1 · 14 */
  const pct = (v: number) => (v < 10 ? num(v, 1) : num(v));
  /** Memoria (base 1024, como el sistema): ["3,2", "GB"], ["812", "MB"], ["24", "GB"]. */
  function memoriaPartes(b: number): [string, string] {
    const gb = b / 1024 ** 3;
    if (gb >= 1) {
      const r = Math.round(gb * 10) / 10;
      return [num(r, Number.isInteger(r) || r >= 100 ? 0 : 1), 'GB'];
    }
    return [num(b / 1024 ** 2), 'MB'];
  }
  const memoriaTexto = (b: number) => memoriaPartes(b).join(NB);
  /** Disco (base 1000, como Finder y el Explorador): número y unidad. */
  function discoTexto(b: number): [string, string] {
    if (b >= 1e12) return [num(b / 1e12, 2), 'TB'];
    return [num(b / 1e9), 'GB'];
  }
  /** "1,6 MB/s", "92 KB/s", "0 KB/s". */
  function velocidad(bps: number): string {
    if (bps >= 1e6) return `${num(bps / 1e6, 1)}${NB}MB/s`;
    return `${num(bps / 1e3)}${NB}KB/s`;
  }
  /** "42 min", "9 h 14 min", "2 d 3 h". */
  function duracion(seg: number): string {
    const d = Math.floor(seg / 86400);
    const h = Math.floor((seg % 86400) / 3600);
    const m = Math.floor((seg % 3600) / 60);
    if (d) return h ? `${d}${NB}d ${h}${NB}h` : `${d}${NB}d`;
    if (h) return m ? `${h}${NB}h ${m}${NB}min` : `${h}${NB}h`;
    return `${Math.max(1, m)}${NB}min`;
  }
  const horaCorta = (ms: number) => new Date(ms).toLocaleTimeString('es', { hour: 'numeric', minute: '2-digit' });
  const horaLarga = (ms: number) =>
    new Date(ms).toLocaleTimeString('es', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  /** "hoy a las 6:56", "ayer a las 22:10", "el lunes a las 8:05", "el 28 de octubre a las 9:10". */
  function desdeCuando(ms: number): string {
    const d = new Date(ms);
    const hoy = new Date();
    hoy.setHours(0, 0, 0, 0);
    const dia = 86400000;
    if (ms >= hoy.getTime()) return `hoy a las ${horaCorta(ms)}`;
    if (ms >= hoy.getTime() - dia) return `ayer a las ${horaCorta(ms)}`;
    if (ms >= hoy.getTime() - 6 * dia)
      return `el ${d.toLocaleDateString('es', { weekday: 'long' })} a las ${horaCorta(ms)}`;
    return `el ${d.toLocaleDateString('es', { day: 'numeric', month: 'long' })} a las ${horaCorta(ms)}`;
  }

  // ------------------------------------------------------------ Lo que dice cada tarjeta

  const TERMICO: Record<EstadoTermico, { texto: string; tono: string; paso: number }> = {
    normal: { texto: 'Normal', tono: 'var(--exito)', paso: 0 },
    tibia: { texto: 'Tibia', tono: 'var(--amarillo)', paso: 1 },
    caliente: { texto: 'Caliente', tono: 'var(--aviso)', paso: 2 },
    'muy-caliente': { texto: 'Muy caliente', tono: 'var(--error)', paso: 3 },
  };
  const PASOS_TERMICOS = ['var(--exito)', 'var(--amarillo)', 'var(--aviso)', 'var(--error)'];

  const PRESION = {
    normal: { texto: 'Presión normal', clase: 'exito' },
    aviso: { texto: 'Presión alta', clase: 'aviso' },
    critica: { texto: 'Presión crítica', clase: 'error' },
  } as const;

  const termico = $derived(foto?.temperatura.estado ? TERMICO[foto.temperatura.estado] : null);

  const bateria = $derived.by(() => {
    const b = foto?.bateria;
    if (!b) return null;
    const estado = b.cargando
      ? 'Cargando'
      : b.completa
        ? 'Carga completa'
        : b.conectada
          ? 'Conectada, sin cargar'
          : 'Con batería';
    const partes: string[] = [];
    if (b.minutos) partes.push(b.cargando ? `Llena en ${duracion(b.minutos * 60)}` : `Quedan ${duracion(b.minutos * 60)}`);
    if (b.ciclos !== null) partes.push(`${num(b.ciclos)} ciclos`);
    if (b.salud !== null) partes.push(`capacidad máxima ${b.salud}${NB}%`);
    const tono = b.nivel <= 10 ? 'var(--error)' : b.nivel <= 20 ? 'var(--aviso)' : 'var(--exito)';
    return { b, estado, detalle: partes.join(' · '), tono };
  });

  const disco = $derived.by(() => {
    const d = foto?.disco;
    if (!d || !d.total) return null;
    const usado = Math.round(((d.total - d.libre) / d.total) * 100);
    const [libre, unidad] = discoTexto(d.libre);
    const [total, unidadTotal] = discoTexto(d.total);
    return { libre, unidad, detalle: `de ${total}${NB}${unidadTotal} · ${usado}${NB}% usado`, usado };
  });

  const cpuDetalle = $derived.by(() => {
    const c = foto?.cpu;
    if (!c) return '';
    const partes = [`${c.cantidad} núcleos`];
    if (c.nucleos.length) partes.push(`el más ocupado al ${pct(Math.max(...c.nucleos))}${NB}%`);
    return partes.join(' · ');
  });

  const temperaturaDetalle = $derived.by(() => {
    const g = foto?.temperatura.bateria;
    return g != null ? `Según macOS · batería a ${num(g, 1)}${NB}°C` : 'Según macOS';
  });

  const appsAyuda = $derived(
    'Agrupados por app. La CPU es la parte de todo el equipo.' +
      (foto?.procesos.soloTuyos ? ' Solo los de tu usuario: macOS no deja leer los del sistema.' : ''),
  );

  const memoriaDetalle = $derived.by(() => {
    const m = foto?.memoria;
    if (!m) return '';
    if (m.apps === null) return `${memoriaTexto(m.libre)} libres`;
    const partes = [`Apps ${memoriaTexto(m.apps)}`];
    if (m.comprimida !== null) partes.push(`comprimida ${memoriaTexto(m.comprimida)}`);
    if (m.swapUsada !== null) partes.push(m.swapUsada > 0 ? `swap ${memoriaTexto(m.swapUsada)}` : `sin${NB}swap`);
    return partes.join(' · ');
  });

  // Las barras de las apps: la mayor marca la escala, pero nunca menos de un 10 % (así lo quieto no se ve lleno).
  const escalaCpu = $derived(Math.max(10, ...(foto?.procesos.cpu.map((p) => p.cpu) ?? [0])));
  const escalaMemoria = $derived(
    Math.max((foto?.memoria?.total ?? 0) * 0.1, ...(foto?.procesos.memoria.map((p) => p.memoria) ?? [0]), 1),
  );

  const sesion = $derived.by(() => {
    if (!foto) return null;
    const s = foto.sesion;
    let rustdesk: { texto: string; detalle: string | null; activo: boolean };
    if (s.activa && s.rele)
      rustdesk = { texto: 'Sesión activa por el relé', detalle: 'Va más lenta que la directa por Tailscale', activo: true };
    else if (s.activa)
      rustdesk = {
        texto: 'Sesión activa, directa por Tailscale',
        detalle: s.equipo ? `Desde ${s.equipo}` : null,
        activo: true,
      };
    else if (s.rustdesk)
      rustdesk = { texto: foto.plataforma === 'mac' ? 'Abierto, sin sesión' : 'Abierto', detalle: null, activo: false };
    else rustdesk = { texto: 'Cerrado', detalle: null, activo: false };

    const t = s.xdrop[0];
    const xdrop = t
      ? `${t.direccion === 'enviado' ? 'Enviando' : 'Recibiendo'} ${t.nombre} ${t.direccion === 'enviado' ? 'a' : 'de'} ${t.equipo}` +
        (t.avance !== null ? ` · ${Math.round(t.avance * 100)}${NB}%` : '') +
        (s.xdrop.length > 1 ? ` · y ${s.xdrop.length - 1} más` : '')
      : 'Sin envíos ahora';
    return { s, rustdesk, xdrop, xdropActivo: !!t };
  });

  const arranque = $derived(foto?.encendida != null ? foto.momento - foto.encendida * 1000 : null);

  const estadoLinea = $derived.by(() => {
    if (!clave) return { tono: 'tenue', texto: 'Buscando tus equipos…' };
    const visto = recibido[clave];
    if (fallo) {
      return {
        tono: 'apagado',
        texto: visto ? `No responde · lo último que se vio, a las ${horaLarga(visto)}` : 'No responde',
      };
    }
    if (!visto) return { tono: 'tenue', texto: elegido && !elegido.propio ? `Conectando con ${elegido.nombre}…` : 'Midiendo…' };
    return {
      tono: clave === 'local' ? 'propio' : 'en-linea',
      texto: `${clave === 'local' ? 'Este equipo' : 'En línea'} · actualizado a las ${horaLarga(visto)}`,
    };
  });

  const falloDetalle = $derived(fallo && !/^No respondi/.test(fallo) ? fallo : null);
</script>

{#snippet cabezaTarjeta(icono: Nombre, rotulo: string)}
  <span class="kpi__cabeza">
    <span class="ficha-icono chica"><Icono nombre={icono} tamano={18} /></span>
    <span class="kpi__rotulo">{rotulo}</span>
  </span>
{/snippet}

{#snippet listaApps(lista: ProcesoApp[], tipo: 'cpu' | 'memoria')}
  {#if lista.length}
    <ol class="apps">
      {#each lista as p (p.nombre)}
        {@const valor = tipo === 'cpu' ? p.cpu : p.memoria}
        {@const escala = tipo === 'cpu' ? escalaCpu : escalaMemoria}
        <li class="app">
          <span class="app__nombre">
            {p.nombre}
            {#if p.procesos > 1}<small>{p.procesos} procesos</small>{/if}
          </span>
          <span class="app__valor">{tipo === 'cpu' ? `${pct(p.cpu)}${NB}%` : memoriaTexto(p.memoria)}</span>
          <span class="app__barra" aria-hidden="true">
            <span style:width="{Math.max(2, Math.min(100, (valor / escala) * 100))}%"></span>
          </span>
        </li>
      {/each}
    </ol>
  {:else}
    <p class="vacio">Midiendo…</p>
  {/if}
{/snippet}

<div class="analitica">
  <header class="cabecera">
    <div class="cabecera__textos">
      <h1>Analítica</h1>
      <p>Cómo está cada equipo ahora mismo. Nexo solo mide mientras miras.</p>
    </div>
    {#if visibles.length > 1}
      <div class="segmentado vidrio" role="group" aria-label="Equipo">
        {#each visibles as e (claveDe(e))}
          <button
            type="button"
            class:elegido={clave === claveDe(e)}
            aria-pressed={clave === claveDe(e)}
            onclick={() => elegir(e)}
          >
            {#if !e.propio}<span class="punto-equipo" class:apagado={!responde(e)} aria-hidden="true"></span>{/if}
            {etiqueta(e)}
            {#if !e.propio && !responde(e)}<span class="solo-lector">(no responde)</span>{/if}
          </button>
        {/each}
      </div>
    {:else if visibles.length === 1}
      <!-- Solo este equipo: su nombre, sin nada que elegir. -->
      <p class="solo-equipo vidrio"><span>{etiqueta(visibles[0])}</span></p>
    {/if}
  </header>

  <p class="estado {estadoLinea.tono}" aria-live="polite">
    <span class="estado__punto" aria-hidden="true"></span>
    {estadoLinea.texto}
  </p>

  {#if visibles.length === 1 && onir}
    {@const ir = onir}
    <button class="pista" type="button" onclick={() => ir('remoto')}>
      Conecta otro equipo para verlo aquí
      <Icono nombre="chevron" tamano={14} />
    </button>
  {/if}

  {#if fallo && elegido && !elegido.propio}
    <div class="sin-respuesta vidrio" role="status">
      <span class="ficha-icono tenue"><Icono nombre={elegido.plataforma === 'mac' ? 'portatil' : 'pantallas'} tamano={20} /></span>
      <div class="sin-respuesta__textos">
        <p class="sin-respuesta__titulo">{elegido.nombre} no responde · ¿Nexo está abierto en {articulo(elegido)}?</p>
        <p class="sin-respuesta__detalle">
          {falloDetalle ?? `Revisa que ${articulo(elegido)} esté encendida y con Tailscale conectado. Nexo vuelve a probar solo.`}
        </p>
      </div>
      <button type="button" class="boton-vidrio vidrio" onclick={refrescar}>Probar de nuevo</button>
    </div>
  {/if}

  {#if foto}
    <div class="contenido" class:tenue={!!fallo}>
      <!-- ---------- Tarjetas ---------- -->
      <div class="kpis">
        <!-- CPU -->
        <article class="kpi vidrio" aria-label="CPU">
          {@render cabezaTarjeta('cpu', 'CPU')}
          {#if foto.cpu}
            <p class="kpi__valor"><span class="kpi__numero">{pct(foto.cpu.total)}<small>%</small></span></p>
            <p class="kpi__detalle">{cpuDetalle}</p>
            <div class="kpi__pie">
              <Tendencia
                momentos={foto.historial.momentos}
                ahora={foto.momento}
                series={[{ nombre: 'CPU', valores: foto.historial.cpu, tipo: 'principal' }]}
                maximo={100}
                formato={(v) => `${pct(v)}${NB}%`}
                etiqueta="Uso de CPU"
              />
            </div>
          {:else}
            <p class="kpi__valor"><span class="kpi__texto tenue">Midiendo…</span></p>
          {/if}
        </article>

        <!-- Memoria -->
        <article class="kpi vidrio" aria-label="Memoria">
          {@render cabezaTarjeta('memoria', 'Memoria')}
          {#if foto.memoria}
            {@const m = foto.memoria}
            {@const [usada, unidad] = memoriaPartes(m.usada)}
            <p class="kpi__valor">
              <span class="kpi__numero">{usada}<small>{unidad}</small></span>
              <span class="kpi__de">de {memoriaTexto(m.total)}</span>
            </p>
            <p class="kpi__detalle">
              {#if m.presion}<span class="chip {PRESION[m.presion].clase}">{PRESION[m.presion].texto}</span>{/if}
              <span>{memoriaDetalle}</span>
            </p>
            <div class="kpi__pie">
              <Tendencia
                momentos={foto.historial.momentos}
                ahora={foto.momento}
                series={[{ nombre: 'Memoria', valores: foto.historial.memoria, tipo: 'principal' }]}
                maximo={100}
                formato={(v) => `${pct(v)}${NB}% en uso`}
                etiqueta="Memoria en uso"
              />
            </div>
          {:else}
            <p class="kpi__valor"><span class="kpi__texto tenue">No disponible</span></p>
          {/if}
        </article>

        <!-- Red -->
        <article class="kpi vidrio" aria-label="Red">
          {@render cabezaTarjeta('red', 'Red')}
          {#if foto.red}
            {@const r = foto.red}
            <p class="kpi__valor red">
              <span class="kpi__numero mediano"
                ><span class="flecha" aria-hidden="true">↓</span><span class="solo-lector">Bajada</span>{velocidad(r.bajada)}</span
              >
              <span class="kpi__numero mediano"
                ><span class="flecha" aria-hidden="true">↑</span><span class="solo-lector">Subida</span>{velocidad(r.subida)}</span
              >
            </p>
            <p class="kpi__detalle">
              {#if r.tailscale}Tailscale ↓{NB}{velocidad(r.tailscale.bajada)} · ↑{NB}{velocidad(r.tailscale.subida)}{:else}Tailscale no está conectado{/if}
            </p>
            <div class="kpi__pie">
              <span class="leyenda" aria-hidden="true">
                <span><span class="clave principal"></span>Bajada</span>
                <span><span class="clave secundaria"></span>Subida</span>
              </span>
              <Tendencia
                momentos={foto.historial.momentos}
                ahora={foto.momento}
                series={[
                  { nombre: 'Bajada', valores: foto.historial.bajada, tipo: 'principal' },
                  { nombre: 'Subida', valores: foto.historial.subida, tipo: 'secundaria' },
                ]}
                techoMinimo={64_000}
                formato={velocidad}
                etiqueta="Bajada de la red"
              />
            </div>
          {:else}
            <p class="kpi__valor"><span class="kpi__texto tenue">No disponible</span></p>
            <p class="kpi__detalle">
              {foto.plataforma === 'windows' ? 'Nexo todavía no mide la red en Windows.' : 'Este equipo no la informa.'}
            </p>
          {/if}
        </article>

        <!-- Temperatura -->
        <article class="kpi vidrio" aria-label="Temperatura">
          {@render cabezaTarjeta('termometro', 'Temperatura')}
          {#if termico}
            <p class="kpi__valor">
              <span class="punto-estado" style:background={termico.tono} aria-hidden="true"></span>
              <span class="kpi__texto fuerte">{termico.texto}</span>
            </p>
            <p class="kpi__detalle">{temperaturaDetalle}</p>
            <div class="kpi__pie">
              <span class="escala" aria-hidden="true">
                {#each PASOS_TERMICOS as tono, i (i)}
                  <span style:background={i <= termico.paso ? tono : null}></span>
                {/each}
              </span>
              <span class="escala__rotulos" aria-hidden="true"><span>Normal</span><span>Muy caliente</span></span>
            </div>
          {:else}
            <p class="kpi__valor"><span class="kpi__texto tenue">No disponible</span></p>
            <p class="kpi__detalle">
              {foto.plataforma === 'windows' ? 'Windows no la informa sin programas aparte.' : 'Este equipo no la informa.'}
            </p>
          {/if}
        </article>

        <!-- Batería -->
        <article class="kpi vidrio" aria-label="Batería">
          {@render cabezaTarjeta('bateria', 'Batería')}
          {#if bateria}
            <p class="kpi__valor">
              <span class="kpi__numero">{bateria.b.nivel}<small>%</small></span>
              <span class="kpi__de">
                {#if bateria.b.cargando}<Icono nombre="rayo" tamano={14} />{/if}{bateria.estado}
              </span>
            </p>
            {#if bateria.detalle}<p class="kpi__detalle">{bateria.detalle}</p>{/if}
            <div class="kpi__pie">
              <span class="barra" aria-hidden="true"><span style:width="{bateria.b.nivel}%" style:background={bateria.tono}></span></span>
            </div>
          {:else}
            <p class="kpi__valor"><span class="kpi__texto tenue">Sin batería</span></p>
            <p class="kpi__detalle">Este equipo va siempre enchufado.</p>
          {/if}
        </article>

        <!-- Disco -->
        <article class="kpi vidrio" aria-label="Disco">
          {@render cabezaTarjeta('disco', 'Disco')}
          {#if disco}
            <p class="kpi__valor">
              <span class="kpi__numero">{disco.libre}<small>{disco.unidad}</small></span>
              <span class="kpi__de">libres</span>
            </p>
            <p class="kpi__detalle">{disco.detalle}</p>
            <div class="kpi__pie">
              <span class="barra" aria-hidden="true">
                <span style:width="{disco.usado}%" style:background={disco.usado >= 90 ? 'var(--aviso)' : 'var(--serie-1)'}></span>
              </span>
            </div>
          {:else}
            <p class="kpi__valor"><span class="kpi__texto tenue">No disponible</span></p>
          {/if}
        </article>
      </div>

      <!-- ---------- Apps, sesión y tiempo encendida ---------- -->
      <div class="fila">
        <section class="panel vidrio" aria-labelledby="analitica-apps">
          <div class="t-cabeza">
            <span class="ficha-icono"><Icono nombre="analitica" tamano={20} /></span>
            <div class="t-textos">
              <h2 class="t-titulo" id="analitica-apps">Procesos que más gastan</h2>
              <p class="t-sub">{appsAyuda}</p>
            </div>
          </div>
          <div class="columnas-apps">
            <div class="columna-apps">
              <h3 class="rotulo">CPU</h3>
              {@render listaApps(foto.procesos.cpu, 'cpu')}
            </div>
            <div class="columna-apps">
              <h3 class="rotulo">Memoria</h3>
              {@render listaApps(foto.procesos.memoria, 'memoria')}
            </div>
          </div>
        </section>

        <div class="lateral">
          {#if sesion}
            <section class="panel vidrio" aria-labelledby="analitica-sesion">
              <div class="t-cabeza">
                <span class="ficha-icono"><Icono nombre="remoto" tamano={20} /></span>
                <div class="t-textos">
                  <h2 class="t-titulo" id="analitica-sesion">Sesión remota</h2>
                </div>
              </div>
              <ul class="filas inserto">
                <li class="fila-dato">
                  <span class="fila-dato__rotulo">RustDesk</span>
                  <span class="fila-dato__valor">
                    <span class="punto-equipo" class:apagado={!sesion.rustdesk.activo} aria-hidden="true"></span>
                    <span>
                      {sesion.rustdesk.texto}
                      {#if sesion.rustdesk.detalle}<small>{sesion.rustdesk.detalle}</small>{/if}
                    </span>
                  </span>
                </li>
                {#if sesion.s.pantallaVirtual}
                  {@const v = sesion.s.pantallaVirtual}
                  <li class="fila-dato">
                    <span class="fila-dato__rotulo">Pantalla virtual</span>
                    <span class="fila-dato__valor">
                      <span class="punto-equipo" class:apagado={!v.activa} aria-hidden="true"></span>
                      <span>{v.activa ? `Encendida · ${v.ancho}${NB}×${NB}${v.alto}` : 'Apagada'}</span>
                    </span>
                  </li>
                {/if}
                <li class="fila-dato">
                  <span class="fila-dato__rotulo">XDrop</span>
                  <span class="fila-dato__valor">
                    <span class="punto-equipo" class:apagado={!sesion.xdropActivo} aria-hidden="true"></span>
                    <span>{sesion.xdrop}</span>
                  </span>
                </li>
                {#if foto.plataforma === 'mac'}
                  <li class="fila-dato">
                    <span class="fila-dato__rotulo">Reposo</span>
                    <span class="fila-dato__valor">
                      <span class="punto-equipo" class:apagado={!sesion.s.sinReposo} aria-hidden="true"></span>
                      <span>{sesion.s.sinReposo ? 'La Mac no se dormirá' : 'Puede dormirse como siempre'}</span>
                    </span>
                  </li>
                {/if}
                {#if sesion.s.modoTrabajo !== null}
                  <li class="fila-dato">
                    <span class="fila-dato__rotulo">Modo trabajo</span>
                    <span class="fila-dato__valor">
                      <span class="punto-equipo" class:apagado={!sesion.s.modoTrabajo} aria-hidden="true"></span>
                      <span>{sesion.s.modoTrabajo ? 'Encendido' : 'Apagado'}</span>
                    </span>
                  </li>
                {/if}
              </ul>
            </section>
          {/if}

          <section class="panel vidrio encendida" aria-labelledby="analitica-encendida">
            <div class="t-cabeza">
              <span class="ficha-icono"><Icono nombre="reloj" tamano={20} /></span>
              <div class="t-textos">
                <h2 class="t-titulo" id="analitica-encendida">Tiempo encendida</h2>
              </div>
            </div>
            {#if foto.encendida !== null}
              <p class="encendida__valor">{duracion(foto.encendida)}</p>
              {#if arranque !== null}<p class="encendida__detalle">Desde {desdeCuando(arranque)}</p>{/if}
            {:else}
              <p class="encendida__detalle">No disponible</p>
            {/if}
          </section>
        </div>
      </div>
    </div>
  {:else if !fallo}
    <!-- Mientras llega la primera medición: las siluetas de las tarjetas. -->
    <div class="kpis" aria-busy="true">
      {#each Array(6) as _, i (i)}
        <div class="kpi vidrio esqueleto">
          <span class="esqueleto__linea corta"></span>
          <span class="esqueleto__linea grande"></span>
          <span class="esqueleto__linea"></span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .analitica {
    /* Colores de las gráficas: el turquesa de Nexo y el azul de información, ajustados para leerse sobre cada
       superficie (revisados para daltonismo y contraste). */
    --serie-1: #00ad8e;
    --serie-2: var(--info);
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1320px;
    margin: 0 auto;
    min-width: 0;
  }
  :global([data-theme='light']) .analitica {
    --serie-1: #009e80;
  }
  @media (prefers-color-scheme: light) {
    :global(:root:not([data-theme='dark'])) .analitica {
      --serie-1: #009e80;
    }
  }

  /* ---------- Cabecera ---------- */
  .cabecera {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 12px 24px;
  }
  .cabecera__textos {
    min-width: 0;
  }
  .cabecera h1 {
    margin: 0;
    font: var(--letra-titulo);
    letter-spacing: -0.01em;
  }
  .cabecera p {
    margin: 4px 0 0;
    color: var(--texto-2);
  }

  .segmentado {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 3px;
    border-radius: var(--radio-pildora);
  }
  .segmentado button {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 30px;
    padding: 0 14px;
    border: 1px solid transparent;
    border-radius: var(--radio-pildora);
    background: transparent;
    color: var(--texto-2);
    font-size: 13px;
    font-weight: 600;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .segmentado button:hover:not(.elegido) {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .segmentado button.elegido {
    background: var(--elegido-bg);
    color: var(--elegido-fg);
    border-color: var(--elegido-borde);
  }

  /* Punto de estado: verde si responde o está activo; gris si no. */
  .punto-equipo {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--exito);
  }
  .punto-equipo.apagado {
    background: var(--texto-3);
  }

  .estado {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: -6px 0 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .estado__punto {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--texto-3);
  }
  .estado.en-linea .estado__punto {
    background: var(--exito);
  }
  .estado.propio .estado__punto {
    background: var(--primario);
  }
  /* Un enlace discreto a la guía de Remoto. */
  .pista {
    display: inline-flex;
    align-items: center;
    align-self: flex-start;
    gap: 4px;
    margin: -10px 0 0 -8px;
    padding: 4px 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    font: var(--letra-pequena);
    font-weight: 600;
    color: var(--texto-2);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .pista:hover {
    background: var(--superficie-hover);
    color: var(--primario-texto);
  }

  /* Solo este equipo: como el selector, con su única opción elegida. */
  .solo-equipo {
    display: inline-flex;
    margin: 0;
    padding: 3px;
    border-radius: var(--radio-pildora);
  }
  .solo-equipo span {
    display: inline-flex;
    align-items: center;
    min-height: 30px;
    padding: 0 14px;
    border: 1px solid var(--elegido-borde);
    border-radius: var(--radio-pildora);
    background: var(--elegido-bg);
    color: var(--elegido-fg);
    font-size: 13px;
    font-weight: 600;
  }

  /* ---------- Sin respuesta ---------- */
  .sin-respuesta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px 14px;
    padding: 14px 16px;
    border-radius: var(--radio-lg);
  }
  .sin-respuesta__textos {
    flex: 1 1 260px;
    min-width: 0;
  }
  .sin-respuesta__titulo {
    margin: 0;
    font: var(--letra-panel);
    color: var(--texto);
  }
  .sin-respuesta__detalle {
    margin: 2px 0 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
  }
  .boton-vidrio {
    flex-shrink: 0;
    color: var(--texto);
  }

  .contenido {
    display: flex;
    flex-direction: column;
    gap: 16px;
    transition: opacity var(--dur-vista) ease;
  }
  /* Lo último que se vio de un equipo que no responde. */
  .contenido.tenue {
    opacity: 0.5;
  }

  /* ---------- Tarjetas: 3 por fila; 6 si sobra ancho; 2 si es angosto ---------- */
  .kpis {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
  }
  @container (min-width: 1240px) {
    .kpis {
      grid-template-columns: repeat(6, minmax(0, 1fr));
    }
  }
  @container (max-width: 620px) {
    .kpis {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  .kpi {
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 14px 16px 14px;
    border-radius: 20px;
    overflow-wrap: break-word;
  }
  .kpi__cabeza {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
  }
  .kpi__rotulo {
    flex: 1;
    min-width: 0;
    font-size: 14px;
    line-height: 19px;
    font-weight: 600;
    color: var(--texto);
  }
  .kpi__valor {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    column-gap: 8px;
    min-height: 38px;
    margin: 0;
    line-height: 38px;
    color: var(--texto-2);
  }
  .kpi__numero {
    font-size: 32px;
    font-weight: 700;
    letter-spacing: -0.035em;
    color: var(--texto);
  }
  .kpi__numero small {
    margin-left: 3px;
    font-size: 16px;
    font-weight: 650;
    letter-spacing: 0;
    color: var(--texto-2);
  }
  .kpi__numero.mediano {
    font-size: 22px;
    letter-spacing: -0.02em;
    white-space: nowrap;
  }
  .kpi__valor.red {
    column-gap: 14px;
  }
  .flecha {
    margin-right: 3px;
    font-weight: 600;
    color: var(--texto-3);
  }
  .kpi__de {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 13px;
    line-height: 18px;
    font-weight: 500;
  }
  .kpi__texto.fuerte {
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--texto);
  }
  .kpi__texto.tenue {
    font-size: 17px;
    font-weight: 600;
    color: var(--texto-3);
  }
  .punto-estado {
    flex-shrink: 0;
    align-self: center;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--texto) 6%, transparent);
  }
  .kpi__detalle {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    margin: 2px 0 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
  }
  /* La gráfica o la barra va siempre al pie: a la misma altura en las tarjetas de una fila. */
  .kpi__pie {
    display: grid;
    gap: 6px;
    margin-top: auto;
    padding-top: 12px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    padding: 1px 8px;
    border-radius: var(--radio-pildora);
    font-size: 11.5px;
    line-height: 17px;
    font-weight: 600;
  }
  .chip.exito {
    color: var(--exito);
    background: color-mix(in srgb, var(--exito) 13%, transparent);
  }
  .chip.aviso {
    color: var(--aviso);
    background: color-mix(in srgb, var(--aviso) 14%, transparent);
  }
  .chip.error {
    color: var(--error);
    background: color-mix(in srgb, var(--error) 13%, transparent);
  }

  .barra {
    display: block;
    height: 6px;
    overflow: hidden;
    border-radius: 99px;
    background: var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .barra > span {
    display: block;
    height: 100%;
    border-radius: inherit;
  }

  /* La escala térmica: cuatro tramos, encendidos hasta el estado de ahora. */
  .escala {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 3px;
  }
  .escala > span {
    height: 6px;
    border-radius: 99px;
    background: var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .escala__rotulos {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 11px;
    line-height: 14px;
    color: var(--texto-3);
  }

  .leyenda {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    font-size: 11.5px;
    line-height: 15px;
    color: var(--texto-2);
  }
  .leyenda > span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .clave {
    width: 12px;
    height: 2px;
    border-radius: 2px;
  }
  .clave.principal {
    background: var(--serie-1);
  }
  .clave.secundaria {
    background: var(--serie-2);
  }

  /* ---------- Segunda fila: apps a la izquierda; sesión y tiempo encendida a la derecha ---------- */
  .fila {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }
  .lateral {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
    align-items: start;
    min-width: 0;
  }
  @container (min-width: 640px) {
    .lateral {
      grid-template-columns: minmax(0, 1.7fr) minmax(0, 1fr);
    }
  }
  @container (min-width: 1240px) {
    .fila {
      grid-template-columns: minmax(0, 1.55fr) minmax(0, 1fr);
    }
    .lateral {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .panel {
    display: grid;
    gap: 14px;
    min-width: 0;
    padding: 16px 18px 18px;
    border-radius: var(--radio-lg);
  }

  .columnas-apps {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 20px;
  }
  @container (max-width: 520px) {
    .columnas-apps {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .columna-apps {
    display: grid;
    align-content: start;
    gap: 8px;
    min-width: 0;
  }
  .columna-apps h3 {
    margin: 0;
  }
  .apps {
    display: grid;
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .app {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: baseline;
    gap: 4px 10px;
  }
  .app__nombre {
    min-width: 0;
    font-size: 13.5px;
    line-height: 18px;
    font-weight: 550;
    color: var(--texto);
    overflow-wrap: anywhere;
  }
  .app__nombre small {
    margin-left: 6px;
    font-size: 11.5px;
    font-weight: 450;
    color: var(--texto-3);
    white-space: nowrap;
  }
  .app__valor {
    font-size: 13px;
    line-height: 18px;
    font-weight: 600;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .app__barra {
    grid-column: 1 / -1;
    height: 4px;
    overflow: hidden;
    border-radius: 99px;
    background: var(--superficie-hundida);
  }
  .app__barra > span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--serie-1);
  }
  .vacio {
    margin: 0;
    font: var(--letra-pequena);
    color: var(--texto-3);
  }

  .filas {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .fila-dato {
    display: grid;
    grid-template-columns: minmax(96px, auto) minmax(0, 1fr);
    gap: 4px 12px;
    padding: 10px 12px;
  }
  .fila-dato + .fila-dato {
    box-shadow: inset 0 1px 0 var(--borde);
  }
  .fila-dato__rotulo {
    font-size: 12.5px;
    line-height: 18px;
    font-weight: 600;
    color: var(--texto-2);
  }
  .fila-dato__valor {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    font-size: 13px;
    line-height: 18px;
    color: var(--texto);
    overflow-wrap: anywhere;
  }
  .fila-dato__valor > .punto-equipo {
    align-self: center;
  }
  .fila-dato__valor small {
    display: block;
    font-size: 12px;
    color: var(--texto-2);
  }
  @container (max-width: 380px) {
    .fila-dato {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .encendida {
    gap: 6px;
  }
  .encendida .t-cabeza {
    margin-bottom: 6px;
  }
  .encendida__valor {
    margin: 0;
    font-size: 28px;
    line-height: 34px;
    font-weight: 700;
    letter-spacing: -0.03em;
    color: var(--texto);
  }
  .encendida__detalle {
    margin: 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
  }

  /* ---------- Siluetas mientras carga ---------- */
  .esqueleto {
    gap: 12px;
    min-height: 150px;
  }
  .esqueleto__linea {
    display: block;
    width: 70%;
    height: 12px;
    border-radius: 6px;
    background: var(--superficie-elevada);
  }
  .esqueleto__linea.corta {
    width: 40%;
  }
  .esqueleto__linea.grande {
    width: 55%;
    height: 28px;
  }
</style>
