<!--
  Casa: todo lo de la casa en un solo lugar, por habitación (Sala, Cuarto, Escritorio… y "Sin cuarto" al final).
  - Arriba: el resumen ("14 aparatos · 6 encendidos"), encender o apagar todas las luces, "Nuevo grupo", "Tu red"
    (todo lo que hay en tu Wi-Fi, ≈4 s; ver TuRed.svelte) y "Buscar aparatos" (≈6 s en la red y por Bluetooth; ver
    BuscarAparatos.svelte). Un solo panel abierto a la vez.
  - Los grupos ("Escritorio") con su interruptor; cada habitación con el de sus luces y sus tarjetas.
  - Las habitaciones se acomodan en una rejilla pareja: las que tienen pocos aparatos se ponen una al lado de la
    otra, así no quedan huecos grandes.
  - Sin aparatos: un estado vacío calmo que empieza por lo de tu red y Bluetooth, sin cuentas ni nube ("Buscar
    aparatos" y "Ver tu red"); Home Assistant, opcional, en Ajustes.
  Se lee al entrar y cada 15 s solo con la ventana a la vista y enfocada (y al volver a ella o cuando algo cambia
  desde el menú del ícono o una escena). Nada más corre.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    alCambiarCasa,
    borrarGrupo,
    borrarHabitacion,
    editarAparato,
    ejecutarEnCasa,
    guardarGrupo,
    guardarHabitacion,
    leerCasa,
    mostrarTodo,
    pruebaCasa,
    quitarAparato,
  } from '$lib/api-hogar';
  import { hayCuentas } from '$lib/hogar';
  import type { Accion, Aparato, CambioAparato, Destino, EstadoCasa, Grupo, Habitacion } from '$lib/tipos-hogar';
  import { tooltip } from '$lib/tooltip';
  import Icono from '$lib/componentes/Icono.svelte';
  import Interruptor from '$lib/componentes/Interruptor.svelte';
  import TarjetaAparato from '$lib/componentes/TarjetaAparato.svelte';
  import IlustracionAparato from '$lib/componentes/IlustracionAparato.svelte';
  import BuscarAparatos from '$lib/componentes/BuscarAparatos.svelte';
  import TuRed from '$lib/componentes/TuRed.svelte';
  import EditarAparato from '$lib/componentes/EditarAparato.svelte';
  import EditarGrupo from '$lib/componentes/EditarGrupo.svelte';
  import EditarHabitacion from '$lib/componentes/EditarHabitacion.svelte';

  let {
    buscarAlEntrar = false,
    abrirRedAlEntrar = false,
    onir,
    onerror,
  }: {
    /** Empieza a buscar aparatos apenas se abre (desde Ajustes o Mi setup). */
    buscarAlEntrar?: boolean;
    /** Abre "Tu red" apenas se abre (desde Ajustes o Mi setup). */
    abrirRedAlEntrar?: boolean;
    onir: (seccion: string, id?: string) => void;
    onerror: (texto: string) => void;
  } = $props();

  // ---------------------------------------------------------------- Datos

  let estado = $state<EstadoCasa | null>(null);
  let leyendo = false;

  async function leer(fresco = true) {
    if (leyendo) return;
    leyendo = true;
    try {
      estado = await leerCasa(fresco);
    } catch (e) {
      onerror(String(e));
    } finally {
      leyendo = false;
    }
  }

  /** Una orden; la casa queda como la devuelve Rust. */
  async function mandar(destino: Destino, accion: Accion, confirmado = false) {
    const r = await ejecutarEnCasa({ destino, accion, confirmado });
    estado = r.estado;
    if (!r.ok && !r.requiereConfirmacion && r.mensaje) onerror(r.mensaje);
  }

  /** Las luces de una habitación: una orden por luz (así no se prende también la tele). */
  async function lucesDe(lista: Aparato[], encender: boolean) {
    const accion: Accion = { tipo: encender ? 'encender' : 'apagar' };
    const fallas: string[] = [];
    const resultados = await Promise.allSettled(
      lista.map((a) => ejecutarEnCasa({ destino: { tipo: 'aparato', id: a.id }, accion })),
    );
    for (const r of resultados) {
      if (r.status === 'fulfilled') {
        estado = r.value.estado;
        if (!r.value.ok && r.value.mensaje) fallas.push(r.value.mensaje);
      } else fallas.push(String(r.reason));
    }
    if (fallas.length) onerror(fallas.join(' '));
  }

  const ordenDe = (id: string) => (accion: Accion, confirmado?: boolean) =>
    mandar({ tipo: 'aparato', id }, accion, confirmado);

  // ---------------------------------------------------------------- Buscar y Tu red (uno a la vez)

  let panel = $state<'buscar' | 'red' | null>(null);
  let vuelta = $state(0);

  /** Abre la búsqueda o, si ya está abierta, busca de nuevo. */
  function buscar() {
    panel = 'buscar';
    vuelta++;
  }

  /** Abre Tu red o, si ya está abierta, la analiza de nuevo. */
  function verRed() {
    panel = 'red';
    vuelta++;
  }

  // ---------------------------------------------------------------- Lo que se ve

  const visibles = $derived(estado?.aparatos ?? []);
  const hayLuces = $derived(visibles.some((a) => a.clase === 'luz' && a.controlable));
  const manejables = $derived(visibles.filter((a) => a.controlable && !a.oculto));

  const resumen = $derived.by(() => {
    if (!estado) return 'Leyendo tu casa…';
    if (!hayCuentas(estado)) return 'Luces, enchufes, la tele y la puerta, en un solo lugar.';
    const lista = visibles.filter((a) => !a.oculto);
    const n = lista.length;
    const prendidos = lista.filter(
      (a) => a.enLinea && a.estado.encendido === true && ['luz', 'enchufe', 'tv', 'clima'].includes(a.clase),
    ).length;
    const abiertas = lista.filter((a) => a.clase === 'sensorPuerta' && a.estado.abierto === true).length;
    const partes = [plural(n, 'aparato', 'aparatos'), plural(prendidos, 'encendido', 'encendidos')];
    if (abiertas) partes.push(plural(abiertas, 'puerta abierta', 'puertas abiertas'));
    return partes.join(' · ');
  });

  /** Lo que falló al leer cada cuenta, en palabras. */
  const avisosCuenta = $derived(
    [
      estado?.errorTuya ? { cuenta: 'Tuya / Smart Life', texto: estado.errorTuya } : null,
      estado?.errorHa ? { cuenta: 'Home Assistant', texto: estado.errorHa } : null,
    ].filter((x) => x !== null),
  );

  interface Seccion {
    id: string;
    nombre: string;
    habitacion: Habitacion | null;
    aparatos: Aparato[];
    luces: Aparato[];
  }

  /** Cada habitación con lo suyo, en el orden de la persona; las vacías no se muestran; "Sin cuarto" al final. */
  const secciones = $derived.by((): Seccion[] => {
    if (!estado) return [];
    const ids = new Set(estado.habitaciones.map((h) => h.id));
    const por = new Map<string, Aparato[]>();
    for (const a of visibles) {
      const k = a.habitacion && ids.has(a.habitacion) ? a.habitacion : '';
      por.set(k, [...(por.get(k) ?? []), a]);
    }
    const luces = (l: Aparato[]) => l.filter((a) => a.clase === 'luz' && a.controlable && a.enLinea);
    const lista: Seccion[] = estado.habitaciones
      .filter((h) => por.get(h.id)?.length)
      .map((h) => {
        const suyos = por.get(h.id) ?? [];
        return { id: h.id, nombre: h.nombre, habitacion: h, aparatos: suyos, luces: luces(suyos) };
      });
    const sin = por.get('') ?? [];
    if (sin.length) lista.push({ id: '', nombre: 'Sin cuarto', habitacion: null, aparatos: sin, luces: luces(sin) });
    return lista;
  });

  const vacias = $derived(estado ? estado.habitaciones.filter((h) => !visibles.some((a) => a.habitacion === h.id)) : []);
  const prendido = (l: Aparato[]) => l.some((a) => a.estado.encendido === true);
  function plural(n: number, uno: string, varios: string) {
    return `${n} ${n === 1 ? uno : varios}`;
  }

  // Rejilla pareja, en el orden de las habitaciones: las chicas comparten fila y la última de cada fila toma el ancho
  // que sobra (sus tarjetas se ensanchan); las grandes van a lo ancho, con sus tarjetas repartidas en filas parejas
  // (4 aparatos en 3 columnas van de a 2). Así no quedan huecos grandes.
  const MINIMO = 232;
  const HUECO = 14;
  let rejilla = $state<HTMLDivElement | null>(null);
  let columnas = $state(3);
  const disposicion = $derived.by(() => {
    const m = new Map<string, { ancho: number; columnas: number }>();
    let fila: string[] = [];
    let ocupado = 0;
    const cerrar = () => {
      const ultima = fila.length ? m.get(fila[fila.length - 1]) : undefined;
      if (ultima && ocupado < columnas) ultima.ancho += columnas - ocupado;
      fila = [];
      ocupado = 0;
    };
    for (const s of secciones) {
      const n = Math.max(1, s.aparatos.length);
      if (n >= columnas) {
        cerrar();
        m.set(s.id, { ancho: columnas, columnas: Math.ceil(n / Math.ceil(n / columnas)) });
        continue;
      }
      if (ocupado + n > columnas) cerrar();
      m.set(s.id, { ancho: n, columnas: n });
      fila.push(s.id);
      ocupado += n;
    }
    cerrar();
    return m;
  });
  $effect(() => {
    const el = rejilla;
    if (!el) return;
    const obs = new ResizeObserver(([e]) => {
      columnas = Math.max(1, Math.floor((e.contentRect.width + HUECO) / (MINIMO + HUECO)));
    });
    obs.observe(el);
    return () => obs.disconnect();
  });

  // ---------------------------------------------------------------- Diálogos

  let editado = $state<Aparato | null>(null);
  let grupoAbierto = $state(false);
  let grupoEditado = $state<Grupo | null>(null);
  let habitacionAbierta = $state(false);
  let habitacionEditada = $state<Habitacion | null>(null);

  /** Hace un cambio y deja la casa como vuelve; devuelve el motivo si no se pudo. */
  async function intentar(f: () => Promise<EstadoCasa>): Promise<string | null> {
    try {
      estado = await f();
      return null;
    } catch (e) {
      return String(e);
    }
  }

  const guardarAparato = (id: string, c: CambioAparato) => intentar(() => editarAparato(id, c));
  const quitar = (id: string) => intentar(() => quitarAparato(id));

  function abrirGrupo(g: Grupo | null) {
    grupoEditado = g;
    grupoAbierto = true;
  }

  function abrirHabitacion(h: Habitacion | null) {
    habitacionEditada = h;
    habitacionAbierta = true;
  }

  async function alternarTodo() {
    const e = await intentar(() => mostrarTodo(!estado?.mostrarTodo));
    if (e) onerror(e);
  }

  // ---------------------------------------------------------------- Ciclo

  onMount(() => {
    leer(true);
    if (abrirRedAlEntrar || pruebaCasa === 'red') verRed();
    else if (buscarAlEntrar || pruebaCasa === 'buscar') buscar();
    // Cada 15 s, solo con la ventana a la vista y enfocada; al volver a ella, enseguida.
    const intervalo = setInterval(() => {
      if (document.visibilityState === 'visible' && document.hasFocus()) leer(true);
    }, 15_000);
    const alEnfocar = () => leer(true);
    window.addEventListener('focus', alEnfocar);
    const dejar = alCambiarCasa(() => leer(false));
    return () => {
      clearInterval(intervalo);
      window.removeEventListener('focus', alEnfocar);
      dejar();
    };
  });
</script>

<div class="casa">
  <header class="cabecera">
    <div class="cabecera__textos">
      <h1>Casa</h1>
      <p class="tabular">{resumen}</p>
    </div>
    <div class="cabecera__acciones">
      {#if hayLuces}
        <div class="par" role="group" aria-label="Todas las luces">
          <button type="button" onclick={() => mandar({ tipo: 'todos', clase: 'luz' }, { tipo: 'encender' })}>
            <Icono nombre="sol" tamano={16} />
            Encender luces
          </button>
          <button type="button" onclick={() => mandar({ tipo: 'todos', clase: 'luz' }, { tipo: 'apagar' })}>
            <Icono nombre="luna" tamano={16} />
            Apagar luces
          </button>
        </div>
      {/if}
      {#if hayCuentas(estado)}
        <button type="button" class="boton" onclick={() => abrirGrupo(null)}>
          <Icono nombre="mas" tamano={16} />
          Nuevo grupo
        </button>
      {/if}
      <button type="button" class="boton" onclick={verRed}>
        <Icono nombre="red" tamano={16} />
        Tu red
      </button>
      {#if hayCuentas(estado)}
        <button type="button" class="boton-primario" onclick={buscar}>
          <Icono nombre="buscar" tamano={16} />
          Buscar aparatos
        </button>
      {/if}
    </div>
  </header>

  {#each avisosCuenta as e (e.cuenta)}
    <div class="aviso-cuenta" role="status">
      <Icono nombre="alerta" tamano={18} />
      <p><b>{e.cuenta}:</b> {e.texto}</p>
      <button type="button" class="boton chico" onclick={() => onir('ajustes', 'casa')}>Revisar en Ajustes</button>
    </div>
  {/each}

  {#if panel === 'buscar'}
    {#key vuelta}
      <BuscarAparatos
        onestado={(e) => (estado = e)}
        {onir}
        {onerror}
        onbuscar={buscar}
        oncerrar={() => (panel = null)}
      />
    {/key}
  {:else if panel === 'red'}
    {#key vuelta}
      <TuRed onestado={(e) => (estado = e)} {onir} {onerror} onanalizar={verRed} oncerrar={() => (panel = null)} />
    {/key}
  {/if}

  {#if !estado}
    <div class="habitaciones-carga" aria-busy="true" aria-label="Leyendo tu casa">
      {#each [0, 1, 2] as i (i)}<div class="cargando lamina"></div>{/each}
    </div>
  {:else if !hayCuentas(estado)}
    <section class="vacio lamina" aria-labelledby="casa-vacia">
      <div class="vacio__escenario" aria-hidden="true">
        <span class="vacio__dibujo">
          <IlustracionAparato clase="luz" forma="foco" encendido color="rgb(255 214 160)" />
        </span>
      </div>
      <div class="vacio__textos">
        <h2 id="casa-vacia">Tu casa, desde Nexo</h2>
        <p>
          Nexo encuentra las luces y los enchufes que están en tu mismo Wi-Fi o cerca por Bluetooth, y los maneja
          directo: sin cuentas ni nube.
        </p>
        <ul class="formas">
          <li>
            <span class="ficha-icono chica" style:--tono="var(--celeste)"><Icono nombre="router" tamano={16} /></span>
            <span><b>En tu red Wi-Fi:</b> Philips Hue, LIFX, Nanoleaf, Elgato, WLED, Shelly, WiZ, Yeelight, Govee, Tasmota…</span>
          </li>
          <li>
            <span class="ficha-icono chica" style:--tono="var(--amarillo)"><Icono nombre="bluetooth" tamano={16} /></span>
            <span><b>Por Bluetooth:</b> tiras LED.</span>
          </li>
          <li>
            <span class="ficha-icono chica"><Icono nombre="casa" tamano={16} /></span>
            <span><b>Home Assistant (opcional):</b> lo que ya tienes en él.</span>
          </li>
        </ul>
        <div class="vacio__botones">
          <button type="button" class="boton-primario" onclick={buscar}>
            <Icono nombre="buscar" tamano={16} />
            Buscar aparatos
          </button>
          <button type="button" class="boton" onclick={verRed}>
            <Icono nombre="red" tamano={16} />
            Ver tu red
          </button>
        </div>
        <p class="vacio__pie">
          ¿Usas Home Assistant?
          <button type="button" class="enlace" onclick={() => onir('ajustes', 'casa')}>Conéctalo en Ajustes</button>
        </p>
      </div>
    </section>
  {:else}
    {#if estado.grupos.length}
      <section class="grupos" aria-labelledby="casa-grupos">
        <h2 class="rotulo" id="casa-grupos">Grupos</h2>
        <ul class="grupos__lista">
          {#each estado.grupos as g (g.id)}
            {@const suyos = visibles.filter((a) => g.aparatos.includes(a.id))}
            {@const conLuz = suyos.filter((a) => a.controlable && (a.clase === 'luz' || a.clase === 'enchufe'))}
            <li class="grupo lamina">
              <span class="grupo__textos">
                <span class="grupo__nombre">{g.nombre}</span>
                <span class="grupo__detalle">{plural(suyos.length, 'aparato', 'aparatos')}</span>
              </span>
              {#if conLuz.length}
                <Interruptor
                  encendido={prendido(conLuz)}
                  etiqueta={prendido(conLuz) ? `Apagar ${g.nombre}` : `Encender ${g.nombre}`}
                  oncambio={() => mandar({ tipo: 'grupo', id: g.id }, { tipo: prendido(conLuz) ? 'apagar' : 'encender' })}
                />
              {/if}
              <button
                type="button"
                class="lapiz"
                aria-label="Editar el grupo {g.nombre}"
                use:tooltip={'Editar'}
                onclick={() => abrirGrupo(g)}
              >
                <Icono nombre="editar" tamano={15} />
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <div class="habitaciones" bind:this={rejilla} style:--columnas={columnas}>
      {#each secciones as s (s.id)}
        {@const encendidas = s.luces.filter((a) => a.estado.encendido === true).length}
        <section class="habitacion" style:--ancho={disposicion.get(s.id)?.ancho ?? 1}
          style:--cols={disposicion.get(s.id)?.columnas ?? 1} aria-labelledby="hab-{s.id || 'sin'}">
          <header class="habitacion__cabeza">
            <div class="habitacion__textos">
              <h2 id="hab-{s.id || 'sin'}">{s.nombre}</h2>
              <span class="tabular">
                {plural(s.aparatos.length, 'aparato', 'aparatos')}{s.luces.length
                  ? ` · ${plural(encendidas, 'luz encendida', 'luces encendidas')}`
                  : ''}
              </span>
            </div>
            {#if s.luces.length > 1}
              <Interruptor
                encendido={encendidas > 0}
                etiqueta={encendidas > 0 ? `Apagar las luces de ${s.nombre}` : `Encender las luces de ${s.nombre}`}
                oncambio={() => lucesDe(s.luces, encendidas === 0)}
              />
            {/if}
            {#if s.habitacion}
              {@const h = s.habitacion}
              <button
                type="button"
                class="lapiz"
                aria-label="Editar la habitación {s.nombre}"
                use:tooltip={'Editar'}
                onclick={() => abrirHabitacion(h)}
              >
                <Icono nombre="editar" tamano={15} />
              </button>
            {/if}
          </header>
          <div class="tarjetas">
            {#each s.aparatos as a (a.id)}
              <TarjetaAparato aparato={a} onorden={ordenDe(a.id)} oneditar={() => (editado = a)} {onerror} />
            {/each}
          </div>
        </section>
      {/each}
    </div>

    <footer class="pie">
      <div class="pie__habitaciones">
        {#if vacias.length}
          <span class="pie__rotulo">Sin aparatos:</span>
          {#each vacias as h (h.id)}
            <button type="button" class="chip" onclick={() => abrirHabitacion(h)}>{h.nombre}</button>
          {/each}
        {/if}
        <button type="button" class="chip nueva" onclick={() => abrirHabitacion(null)}>
          <Icono nombre="mas" tamano={14} />
          Habitación
        </button>
      </div>
      {#if estado.ocultos > 0 || estado.mostrarTodo}
        <div class="pie__todo">
          <span>
            Mostrar todo
            {#if estado.ocultos > 0}<span class="tenue">· {estado.ocultos} más de Home Assistant</span>{/if}
          </span>
          <Interruptor encendido={estado.mostrarTodo} etiqueta="Mostrar todo" oncambio={alternarTodo} />
        </div>
      {/if}
    </footer>
  {/if}
</div>

<EditarAparato
  aparato={editado}
  habitaciones={estado?.habitaciones ?? []}
  onguardar={guardarAparato}
  onquitar={quitar}
  oncerrar={() => (editado = null)}
/>

<EditarGrupo
  abierto={grupoAbierto}
  grupo={grupoEditado}
  aparatos={manejables}
  habitaciones={estado?.habitaciones ?? []}
  onguardar={(g) => intentar(() => guardarGrupo(g))}
  onborrar={(id) => intentar(() => borrarGrupo(id))}
  oncerrar={() => (grupoAbierto = false)}
/>

<EditarHabitacion
  abierto={habitacionAbierta}
  habitacion={habitacionEditada}
  onguardar={(h) => intentar(() => guardarHabitacion(h))}
  onborrar={(id) => intentar(() => borrarHabitacion(id))}
  oncerrar={() => (habitacionAbierta = false)}
/>

<style>
  .casa {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 20px;
    max-width: 1320px;
    margin: 0 auto;
    min-width: 0;
  }
  .tabular {
    font-variant-numeric: tabular-nums;
  }

  /* ---------- Cabecera ---------- */
  .cabecera {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 12px 20px;
  }
  .cabecera__textos {
    flex: 1 1 220px;
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
  .cabecera__acciones {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .boton,
  .par button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-height: 36px;
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
  .boton:hover,
  .par button:hover {
    background: var(--superficie-hover);
  }
  .boton:active,
  .par button:active {
    transform: scale(0.98);
  }
  .chico {
    min-height: 30px;
    padding: 0 12px;
    font-size: 12.5px;
  }
  /* Encender y apagar todas las luces: un solo bloque partido en dos. */
  .par {
    display: inline-flex;
    border-radius: var(--radio-sm);
    box-shadow: 0 0 0 1px var(--borde-tarjeta);
  }
  .par button {
    border-radius: 0;
    box-shadow: inset 0 1px 0 var(--luz-tope);
  }
  .par button:first-child {
    border-radius: var(--radio-sm) 0 0 var(--radio-sm);
    box-shadow:
      inset 0 1px 0 var(--luz-tope),
      inset -1px 0 0 var(--borde);
  }
  .par button:last-child {
    border-radius: 0 var(--radio-sm) var(--radio-sm) 0;
  }
  .par button:first-child :global(svg) {
    color: var(--amarillo);
  }
  .par button:last-child :global(svg) {
    color: var(--texto-2);
  }

  /* ---------- Avisos de las cuentas ---------- */
  .aviso-cuenta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    padding: 12px 14px;
    border-radius: var(--radio-md);
    background: color-mix(in srgb, var(--aviso) 9%, var(--superficie));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--aviso) 28%, transparent);
  }
  .aviso-cuenta > :global(svg) {
    flex-shrink: 0;
    color: var(--aviso);
  }
  .aviso-cuenta p {
    flex: 1 1 260px;
    margin: 0;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto);
    overflow-wrap: break-word;
  }

  /* ---------- Grupos ---------- */
  .grupos {
    display: grid;
    gap: 8px;
  }
  .grupos h2 {
    margin: 0;
  }
  .grupos__lista {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .grupo {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    padding: 8px 8px 8px 16px;
    border-radius: var(--radio-md);
  }
  .grupo__textos {
    display: grid;
    min-width: 0;
  }
  .grupo__nombre {
    font-size: 14px;
    line-height: 19px;
    font-weight: 650;
    overflow-wrap: break-word;
  }
  .grupo__detalle {
    font-size: 12px;
    line-height: 16px;
    color: var(--texto-2);
  }
  .lapiz {
    display: grid;
    place-items: center;
    flex-shrink: 0;
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
  .lapiz:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }

  /* ---------- Habitaciones ---------- */
  .habitaciones {
    display: grid;
    grid-template-columns: repeat(var(--columnas), minmax(0, 1fr));
    gap: 22px 14px;
    align-items: start;
  }
  .habitacion {
    grid-column: span var(--ancho);
    display: grid;
    gap: 10px;
    min-width: 0;
  }
  .habitacion__cabeza {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    min-height: 36px;
    padding: 0 4px;
  }
  .habitacion__textos {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 10px;
    flex: 1 1 140px;
    min-width: 0;
  }
  .habitacion__textos h2 {
    margin: 0;
    font: var(--letra-panel);
    font-size: 16px;
    overflow-wrap: break-word;
  }
  .habitacion__textos span {
    font: var(--letra-pequena);
    color: var(--texto-2);
  }
  .tarjetas {
    display: grid;
    grid-template-columns: repeat(var(--cols), minmax(0, 1fr));
    gap: 14px;
  }

  /* ---------- Pie ---------- */
  .pie {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px 20px;
    padding-top: 4px;
  }
  .pie__habitaciones {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .pie__rotulo {
    margin-right: 2px;
    font: var(--letra-pequena);
    color: var(--texto-3);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 30px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--radio-pildora);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-2);
    font-size: 12.5px;
    font-weight: 600;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .chip:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .chip.nueva {
    color: var(--primario-texto);
  }
  .pie__todo {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    font-weight: 600;
  }
  .pie__todo .tenue {
    font-weight: 500;
    color: var(--texto-3);
  }

  /* ---------- Cargando ---------- */
  .habitaciones-carga {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(232px, 1fr));
    gap: 14px;
  }
  .cargando {
    min-height: 260px;
    border-radius: 20px;
  }

  /* ---------- Vacío ---------- */
  .vacio {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 20px;
    padding: 16px;
    border-radius: var(--radio-lg);
  }
  @container (min-width: 700px) {
    .vacio {
      grid-template-columns: minmax(220px, 0.8fr) minmax(0, 1.4fr);
      align-items: center;
      gap: 28px;
      padding: 16px 28px 16px 16px;
    }
  }
  .vacio__escenario {
    display: grid;
    place-items: center;
    min-height: 220px;
    height: 100%;
    border-radius: 18px;
    background:
      radial-gradient(55% 60% at 50% 48%, rgb(255 214 160 / 0.32), transparent),
      radial-gradient(70% 80% at 30% 100%, var(--aurora-1), transparent),
      var(--superficie-inserto);
  }
  .vacio__dibujo {
    display: block;
    width: 160px;
    height: 160px;
  }
  .vacio__textos {
    display: grid;
    gap: 12px;
    min-width: 0;
  }
  .vacio h2 {
    margin: 0;
    font: var(--letra-seccion);
    letter-spacing: -0.01em;
  }
  .vacio p {
    margin: 0;
    color: var(--texto-2);
  }
  .formas {
    display: grid;
    gap: 10px;
    margin: 4px 0;
    padding: 0;
    list-style: none;
  }
  .formas li {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 13.5px;
    line-height: 19px;
    color: var(--texto-2);
  }
  .formas b {
    color: var(--texto);
    font-weight: 600;
  }
  .vacio__botones {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 4px;
  }
  .vacio .vacio__pie {
    font: var(--letra-pequena);
    color: var(--texto-3);
  }
  .enlace {
    padding: 0;
    border: 0;
    background: none;
    color: var(--primario-texto);
    font: inherit;
    font-weight: 600;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .enlace:hover {
    text-decoration-thickness: 2px;
  }
</style>
