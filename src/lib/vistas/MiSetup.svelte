<!--
  Mi setup: tus escenas de un toque y tus dispositivos donde están, en la casa y en la oficina.
  - Arriba, las escenas como tarjetas grandes ("Modo estudio", "Modo remoto"…). Un toque la activa: se ve el
    paso en curso, luego "Listo" o qué paso falló y por qué. La última activada queda con el estilo de lo elegido.
    El lápiz abre el editor (una hoja a la derecha).
  - Debajo, el mapa: un escritorio por lugar, uno al lado del otro si hay espacio o con un selector si es angosto.
    Este equipo se ve en vivo; el otro llega por Nexo Drop (`setup_estado`). Si está apagado, lo último que se vio
    queda tenue. "Editar mi setup" deja arrastrar, agrandar, quitar y sumar cosas (lo detectado por Nexo y
    decoraciones); "Listo" lo guarda.
  - Al final, la nota de las luces.
  Liviano: se relee cada 15 s solo con la ventana a la vista (y al volver a ella); sin bucles ni animaciones.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    activarEscena,
    alAvanceEscena,
    alFinEscena,
    borrarEscena,
    enApp,
    guardarEscena,
    guardarMapa,
    leerEscenas,
    leerMapa,
    leerSetup,
    pruebaSetup,
  } from '$lib/api-setup';
  import type { Dispositivo, Estado, Pantalla } from '$lib/tipos';
  import { nivelPrincipal } from '$lib/tipos';
  import type {
    Aparato,
    Aparatos,
    AvanceEscena,
    Deco,
    Escena,
    EstadoEscenas,
    EstadoSetup,
    ItemMapa,
    LugarId,
    MapaSetup,
    ResultadoAccion,
    TipoItem,
  } from '$lib/tipos-setup';
  import {
    DECOS,
    LUGARES,
    NOMBRE_CAPA,
    PARED,
    PROPORCION,
    acotarPared,
    acotarX,
    anchoDe,
    capaCercana,
    claveItem,
    comoDispositivo,
    encajar,
    formaDeEquipo,
    fotoMonitor,
    haciendo,
    iconoEscena,
    lugarDe,
    nombreAparato,
    nuevoItem,
    paso,
    porDefecto,
    resolver,
    resumen,
    ubicar,
    type Forma,
    type Fuente,
    type Ubicada,
  } from '$lib/setup';
  import { modeloDe } from '$lib/catalogo';
  import { cuando } from '$lib/tiempo';
  import { tooltip } from '$lib/tooltip';
  import Icono from '$lib/componentes/Icono.svelte';
  import Ilustracion from '$lib/componentes/Ilustracion.svelte';
  import Escritorio from '$lib/componentes/Escritorio.svelte';
  import EditorEscena from '$lib/componentes/EditorEscena.svelte';
  import IlustracionEscena, { ilustracionDe } from '$lib/componentes/IlustracionEscena.svelte';

  let {
    estado,
    pantallas,
    onir,
    onerror,
  }: {
    estado: Estado | null;
    pantallas: Pantalla[];
    onir: (seccion: string, id?: string) => void;
    onerror: (texto: string) => void;
  } = $props();

  // ---------------------------------------------------------------- Datos

  let setup = $state<EstadoSetup | null>(null);
  let mapa = $state<MapaSetup | null>(null);
  let escenas = $state<EstadoEscenas | null>(null);

  let leyendo = false;
  async function leer() {
    if (leyendo) return;
    leyendo = true;
    try {
      setup = await leerSetup();
    } catch (e) {
      onerror(String(e));
    } finally {
      leyendo = false;
    }
  }

  const aAparato = (d: Dispositivo): Aparato => ({
    id: d.id,
    nombre: d.nombre,
    alias: d.alias,
    tipo: d.tipo,
    conectado: d.conectado,
    bateria: nivelPrincipal(d),
    fabricante: d.fabricante,
    vendorId: d.vendorId,
    productId: d.productId,
  });

  // Este equipo: en la app, con lo que el panel ya lee cada 5 s (más al día); fuera de ella, el ejemplo.
  const local = $derived.by((): Aparatos | null => {
    if (!setup) return null;
    if (!enApp || !estado) return setup.local;
    return {
      ...setup.local,
      dispositivos: estado.dispositivos.map(aAparato),
      // La pantalla virtual de RustDesk no va en el mapa: no está en el escritorio.
      pantallas: pantallas.length
        ? pantallas
            .filter((p) => !p.virtual)
            .map((p) => ({ id: p.id, nombre: p.nombre, interna: p.interna, brillo: p.brillo }))
        : setup.local.pantallas,
    };
  });
  const lugarLocal = $derived<LugarId>(local ? lugarDe(local.plataforma) : 'casa');
  const fuenteLocal = $derived<Fuente | null>(local ? { aparatos: local, local: true, vivo: true, visto: null } : null);
  // Los otros equipos, con el nombre con que Tailscale los ve desde aquí (el mismo que se guarda en el mapa).
  const remotos = $derived(
    (setup?.remotos ?? []).map((r) => (r.aparatos ? { ...r, aparatos: { ...r.aparatos, equipo: r.nombre } } : r)),
  );
  const remotosConocidos = $derived(new Set(remotos.map((r) => r.nombre)));

  /** De dónde sale cada cosa del mapa, por el nombre de su equipo. */
  const fuentes = $derived.by(() => {
    const m = new Map<string, Fuente>();
    for (const r of remotos) {
      if (!r.aparatos) continue;
      m.set(r.nombre, { aparatos: r.aparatos, local: false, vivo: r.enLinea && r.alDia, visto: r.alDia ? null : r.visto });
    }
    if (fuenteLocal) m.set(fuenteLocal.aparatos.equipo, fuenteLocal);
    return m;
  });

  type EstadoLugar = 'local' | 'en-linea' | 'apagada' | 'error' | 'desconocido';
  interface InfoLugar {
    id: LugarId;
    nombre: string;
    equipo: string | null;
    aparatos: Aparatos | null;
    estado: EstadoLugar;
    visto: number | null;
    error: string | null;
  }

  /** Cada lugar con su equipo: la Mac en la casa y el PC en la oficina. */
  const lugares = $derived(
    LUGARES.map((l): InfoLugar => {
      if (local && lugarLocal === l.id)
        return { ...l, equipo: local.equipo, aparatos: local, estado: 'local', visto: null, error: null };
      const candidatos = remotos.filter((r) => lugarDe(r.plataforma) === l.id);
      const r = candidatos.find((x) => x.enLinea && x.alDia) ?? candidatos.find((x) => x.aparatos) ?? candidatos[0];
      if (!r) return { ...l, equipo: null, aparatos: null, estado: 'desconocido', visto: null, error: null };
      const estadoR: EstadoLugar = r.enLinea && r.alDia ? 'en-linea' : r.enLinea && r.error ? 'error' : 'apagada';
      return { ...l, equipo: r.nombre, aparatos: r.aparatos, estado: estadoR, visto: r.visto, error: r.error };
    }),
  );

  // ---------------------------------------------------------------- Mapa

  let editando = $state(false);
  /** El lugar a la vista cuando es angosto, y el que se edita. */
  let lugarSel = $state<LugarId>('casa');
  let borrador = $state<Record<LugarId, ItemMapa[]>>({ casa: [], oficina: [] });
  let tocados = $state<LugarId[]>([]);
  let seleccion = $state<string | null>(null);
  let guardando = $state(false);
  let ficha = $state<{ lugar: LugarId; id: string } | null>(null);
  let escenasEl = $state<Partial<Record<LugarId, HTMLDivElement>>>({});

  /** Lo que hay en un lugar: el borrador al editar, lo guardado, o el de la primera vez con lo que haya. */
  function itemsDe(l: LugarId): ItemMapa[] {
    if (editando) return borrador[l];
    const guardado = mapa?.[l];
    if (guardado) return guardado.items;
    return porDefecto(l, lugares.find((x) => x.id === l)?.aparatos ?? null).items;
  }

  const vistas = $derived.by(() => {
    const r = {} as Record<LugarId, Ubicada[]>;
    for (const l of LUGARES) {
      const piezas = itemsDe(l.id)
        .map((i) => {
          const usarLocal = l.id === lugarLocal && !(i.tipo === 'equipo' && remotosConocidos.has(i.ref));
          return resolver(i, fuentes, usarLocal ? fuenteLocal : null);
        })
        .filter((p) => p !== null);
      r[l.id] = ubicar(piezas);
    }
    return r;
  });

  function entrar() {
    borrador = { casa: $state.snapshot(itemsDe('casa')), oficina: $state.snapshot(itemsDe('oficina')) };
    tocados = [];
    seleccion = null;
    ficha = null;
    editando = true;
  }

  function cancelar() {
    editando = false;
    seleccion = null;
  }

  async function listo() {
    if (guardando) return;
    guardando = true;
    const nuevo: MapaSetup = $state.snapshot({
      casa: tocados.includes('casa') ? { items: borrador.casa } : (mapa?.casa ?? null),
      oficina: tocados.includes('oficina') ? { items: borrador.oficina } : (mapa?.oficina ?? null),
    });
    try {
      mapa = await guardarMapa(nuevo);
      editando = false;
      seleccion = null;
    } catch (e) {
      onerror(String(e));
    } finally {
      guardando = false;
    }
  }

  function tocar() {
    if (!tocados.includes(lugarSel)) tocados.push(lugarSel);
  }

  const redondear = (n: number, d = 10) => Math.round(n * d) / d;
  const piezaDe = (id: string) => vistas[lugarSel].find((p) => p.item.id === id) ?? null;
  const itemDe = (id: string) => borrador[lugarSel].find((i) => i.id === id) ?? null;

  function mover(id: string, c: { x?: number; y?: number; capa?: number; escala?: number }) {
    const it = itemDe(id);
    const p = piezaDe(id);
    if (!it || !p) return;
    if (c.x !== undefined) it.x = redondear(acotarX(c.x));
    if (c.escala !== undefined) it.escala = redondear(Math.min(2, Math.max(0.5, c.escala)), 20);
    if (p.forma.pared) {
      const y = c.y ?? it.y ?? 30;
      it.y = redondear(acotarPared(y, anchoDe(it, p.forma), p.forma));
    } else if (c.capa !== undefined) {
      it.capa = Math.min(2, Math.max(0, c.capa));
    }
    tocar();
  }

  /** Al soltar cerca del soporte de audífonos o de la base del monitor, queda justo encima. */
  function soltar(id: string) {
    const it = itemDe(id);
    if (!it) return;
    const ajuste = encajar(it, vistas[lugarSel]);
    if (ajuste) Object.assign(it, ajuste);
  }

  function quitar(id: string) {
    borrador[lugarSel] = borrador[lugarSel].filter((i) => i.id !== id);
    if (seleccion === id) seleccion = null;
    tocar();
  }

  function restablecer() {
    const info = lugares.find((l) => l.id === lugarSel);
    borrador[lugarSel] = porDefecto(lugarSel, info?.aparatos ?? null).items;
    seleccion = null;
    tocar();
  }

  /** Suma algo al lugar que se edita: donde se soltó o, con un clic, cerca del centro. */
  function agregar(tipo: TipoItem, ref: string, equipo: string | null, forma: Forma, punto?: { x: number; y: number }) {
    const id = tipo === 'deco' ? null : claveItem(tipo, equipo, ref);
    if (id && itemDe(id)) {
      seleccion = id;
      return;
    }
    const n = borrador[lugarSel].length;
    const x = punto ? acotarX(punto.x) : 50 + ((n % 5) - 2) * 7;
    let item: ItemMapa;
    if (forma.pared) {
      const alto = (forma.caja[3] - forma.caja[1]) * forma.tam * PROPORCION;
      const y = acotarPared(punto ? punto.y + alto / 2 : 32, forma.tam, forma);
      item = nuevoItem(tipo, ref, equipo, { x, y: redondear(y), capa: 0 });
    } else {
      const capa = punto ? (punto.y < PARED ? 0 : capaCercana(punto.y)) : 1;
      item = nuevoItem(tipo, ref, equipo, { x: redondear(x), capa });
    }
    borrador[lugarSel].push(item);
    tocar();
    seleccion = item.id;
    // Si quedó cerca del soporte o de la base, encaja (con las piezas ya ubicadas).
    queueMicrotask(() => soltar(item.id));
  }

  // --- Lo que se puede sumar al lugar que se edita ---
  interface Detectado {
    tipo: TipoItem;
    ref: string;
    equipo: string;
    nombre: string;
    detalle: string;
    forma: Forma;
    marca: string | null;
  }

  const detectados = $derived.by(() => {
    if (!editando) return [];
    const puestos = new Set(borrador[lugarSel].map((i) => i.id));
    const grupos: { equipo: string; titulo: string; lista: Detectado[] }[] = [];
    const orden = [...fuentes.entries()].sort(([, a], [, b]) => Number(b.local) - Number(a.local));
    for (const [nombre, f] of orden) {
      const lista: Detectado[] = [];
      if (!puestos.has(claveItem('equipo', nombre, nombre)))
        lista.push({
          tipo: 'equipo',
          ref: nombre,
          equipo: nombre,
          nombre,
          detalle: f.aparatos.plataforma === 'mac' ? 'Esta Mac' : 'El equipo',
          forma: formaDeEquipo(f.aparatos),
          marca: null,
        });
      for (const p of f.aparatos.pantallas.filter((x) => !x.interna)) {
        if (puestos.has(claveItem('pantalla', nombre, p.nombre))) continue;
        const foto = fotoMonitor(p.nombre);
        lista.push({
          tipo: 'pantalla',
          ref: p.nombre,
          equipo: nombre,
          nombre: p.nombre,
          detalle: 'Monitor',
          forma: { foto, ilustracion: foto ? null : 'monitor', caja: [0, 0, 1, 1], tam: 38 },
          marca: null,
        });
      }
      for (const a of f.aparatos.dispositivos) {
        if (puestos.has(claveItem('aparato', nombre, a.id))) continue;
        const foto = modeloDe(comoDispositivo(a))?.foto ?? null;
        lista.push({
          tipo: 'aparato',
          ref: a.id,
          equipo: nombre,
          nombre: nombreAparato(a),
          detalle: a.conectado ? 'Conectado' : 'Sin conectar',
          forma: { foto, ilustracion: foto ? null : a.tipo, caja: [0, 0, 1, 1], tam: 10 },
          marca: a.fabricante,
        });
      }
      grupos.push({ equipo: nombre, titulo: f.local ? `En ${nombre}, este equipo` : `En ${nombre}`, lista });
    }
    return grupos;
  });

  /** La forma con la que se ubica en la escena (la del catálogo, no la de la miniatura). */
  function formaReal(d: Detectado): Forma | null {
    const probe = resolver(nuevoItem(d.tipo, d.ref, d.equipo, { x: 50, capa: 1 }), fuentes, null);
    return probe?.forma ?? null;
  }

  // --- Arrastrar desde la paleta (con el puntero; el arrastre nativo lo toma Nexo Drop) ---
  let origen: { cx: number; cy: number; nombre: string; poner: (punto?: { x: number; y: number }) => void } | null = null;
  let fantasma = $state<{ x: number; y: number; nombre: string } | null>(null);

  function presionarPaleta(e: PointerEvent, nombre: string, poner: (punto?: { x: number; y: number }) => void) {
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    origen = { cx: e.clientX, cy: e.clientY, nombre, poner };
  }
  function moverPaleta(e: PointerEvent) {
    if (!origen) return;
    if (!fantasma && Math.hypot(e.clientX - origen.cx, e.clientY - origen.cy) < 6) return;
    fantasma = { x: e.clientX, y: e.clientY, nombre: origen.nombre };
  }
  function soltarPaleta(e: PointerEvent) {
    const o = origen;
    origen = null;
    if (!o) return;
    if (!fantasma) {
      o.poner();
      return;
    }
    fantasma = null;
    const r = escenasEl[lugarSel]?.getBoundingClientRect();
    if (r && e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom)
      o.poner({ x: ((e.clientX - r.left) / r.width) * 100, y: ((e.clientY - r.top) / r.height) * 100 });
  }
  function cancelarPaleta() {
    origen = null;
    fantasma = null;
  }

  const ponerDetectado = (d: Detectado) => (punto?: { x: number; y: number }) => {
    const f = formaReal(d);
    if (f) agregar(d.tipo, d.ref, d.equipo, f, punto);
  };
  const ponerDeco = (ref: Deco) => (punto?: { x: number; y: number }) => agregar('deco', ref, null, DECOS[ref].forma, punto);

  const elegida = $derived(editando && seleccion ? piezaDe(seleccion) : null);

  // ---------------------------------------------------------------- Escenas

  let corriendo = $state<AvanceEscena | null>(null);
  let fin = $state<{ id: string; resultados: ResultadoAccion[]; ok: boolean } | null>(null);
  /** La escena que se activó desde aquí (su resultado llega por la respuesta, no por el aviso). */
  let propia: string | null = null;
  let temporizadorFin: ReturnType<typeof setTimeout> | undefined;

  /** El nombre de un aparato de un paso: el de ahora en este equipo o el que se guardó con el paso. */
  const nombrePor = (id: string, guardado?: string | null) => {
    const a = local?.dispositivos.find((d) => d.id.toUpperCase() === id.toUpperCase());
    return a ? nombreAparato(a) : guardado || 'un aparato';
  };

  async function activar(e: Escena) {
    if (corriendo) return;
    propia = e.id;
    fin = null;
    corriendo = { id: e.id, indice: 0, total: e.acciones.length, accion: e.acciones[0] ?? { tipo: 'luces' } };
    try {
      terminar(e.id, await activarEscena(e.id));
    } catch (err) {
      corriendo = null;
      onerror(String(err));
    } finally {
      propia = null;
    }
  }

  function terminar(id: string, resultados: ResultadoAccion[]) {
    corriendo = null;
    const ok = resultados.every((r) => r.ok);
    fin = { id, resultados, ok };
    clearTimeout(temporizadorFin);
    // "Listo" se va solo; si algo falló, queda a la vista hasta la próxima.
    if (ok) temporizadorFin = setTimeout(() => fin?.id === id && fin.ok && (fin = null), 5000);
    leerEscenas()
      .then((x) => (escenas = x))
      .catch(() => {});
  }

  let editor = $state<{ escena: Escena | null } | null>(null);

  async function guardar(e: Escena): Promise<string | null> {
    try {
      escenas = await guardarEscena(e);
      editor = null;
      return null;
    } catch (err) {
      return String(err);
    }
  }

  async function borrar(id: string): Promise<string | null> {
    try {
      escenas = await borrarEscena(id);
      if (fin?.id === id) fin = null;
      editor = null;
      return null;
    } catch (err) {
      return String(err);
    }
  }

  // ---------------------------------------------------------------- Fila de escenas
  // Una sola fila: si sobra ancho, las tarjetas se estiran; si no, la fila se desliza (con imán por tarjeta)
  // y aparecen las flechas. Sin barra de desplazamiento, así la fila no salta de alto.

  let tira = $state<HTMLDivElement | null>(null);
  let tiraDesborda = $state(false);
  let tiraAtras = $state(false);
  let tiraAdelante = $state(false);

  function medirTira() {
    if (!tira) return;
    const { scrollLeft, scrollWidth, clientWidth } = tira;
    tiraDesborda = scrollWidth > clientWidth + 1;
    tiraAtras = scrollLeft > 2;
    tiraAdelante = scrollLeft + clientWidth < scrollWidth - 2;
  }

  function moverTira(sentido: 1 | -1) {
    if (!tira) return;
    const paso = (tira.firstElementChild as HTMLElement | null)?.offsetWidth ?? 240;
    const quieto = matchMedia('(prefers-reduced-motion: reduce)').matches;
    tira.scrollBy({ left: sentido * (paso + 12), behavior: quieto ? 'auto' : 'smooth' });
  }

  // Se vuelve a medir al cambiar el ancho o la cantidad de escenas.
  $effect(() => {
    const el = tira;
    if (!el) return;
    void escenas?.escenas.length;
    const obs = new ResizeObserver(medirTira);
    obs.observe(el);
    medirTira();
    return () => obs.disconnect();
  });

  // ---------------------------------------------------------------- Ciclo

  onMount(() => {
    leer();
    leerMapa()
      .then((m) => (mapa = m))
      .catch((e) => {
        mapa = { casa: null, oficina: null };
        onerror(String(e));
      });
    leerEscenas()
      .then((x) => {
        escenas = x;
        // Para revisar el editor fuera de la app (api-setup.ts).
        if ((pruebaSetup === 'escena' || pruebaSetup === 'aparatos') && x.escenas[0]) editor = { escena: x.escenas[0] };
      })
      .catch((e) => onerror(String(e)));

    // Cada 15 s solo con la ventana a la vista; al volver a ella, enseguida.
    let intervalo: ReturnType<typeof setInterval> | undefined;
    const arrancar = () => {
      clearInterval(intervalo);
      intervalo = setInterval(leer, 15_000);
    };
    if (document.visibilityState === 'visible') arrancar();
    const alCambiarVista = () => {
      if (document.visibilityState === 'visible') {
        leer();
        arrancar();
      } else {
        clearInterval(intervalo);
      }
    };
    const alEnfocar = () => leer();
    document.addEventListener('visibilitychange', alCambiarVista);
    window.addEventListener('focus', alEnfocar);

    // Las escenas también se activan desde el ícono de la barra de menús o la bandeja.
    const quitarAvance = alAvanceEscena((a) => (corriendo = a));
    const quitarFin = alFinEscena((f) => {
      if (f.id !== propia) terminar(f.id, f.resultados);
    });

    return () => {
      clearInterval(intervalo);
      clearTimeout(temporizadorFin);
      document.removeEventListener('visibilitychange', alCambiarVista);
      window.removeEventListener('focus', alEnfocar);
      quitarAvance();
      quitarFin();
    };
  });

  // Angosto: se ve primero el lugar de este equipo (hasta que se elija otro).
  let lugarPuesto = false;
  $effect(() => {
    if (!local || lugarPuesto) return;
    lugarPuesto = true;
    lugarSel = lugarLocal;
  });

  // `?setup=editar`: abre el mapa para acomodarlo apenas llegan los datos.
  let pedirEdicion = pruebaSetup === 'editar';
  $effect(() => {
    if (pedirEdicion && setup && mapa) {
      pedirEdicion = false;
      entrar();
    }
  });

  // ---------------------------------------------------------------- Textos

  function lineaLugar(l: InfoLugar): { texto: string; tono: 'ok' | 'tenue' | 'aviso' } {
    switch (l.estado) {
      case 'local':
        return { texto: `${l.equipo} · Este equipo`, tono: 'ok' };
      case 'en-linea':
        return { texto: `${l.equipo} · En línea`, tono: 'ok' };
      case 'apagada':
        return { texto: `${l.equipo} · Apagada o sin Nexo`, tono: 'tenue' };
      case 'error':
        return { texto: `${l.equipo} · Sin datos de ahora`, tono: 'aviso' };
      default:
        return { texto: setup ? 'Todavía sin equipo con Nexo' : 'Buscando…', tono: 'tenue' };
    }
  }

  function avisoLugar(l: InfoLugar): string | null {
    switch (l.estado) {
      case 'apagada':
        return `${l.equipo} está apagada o sin Nexo.${l.visto && l.aparatos ? ` Lo último que se vio: ${cuando(l.visto)}.` : ''}`;
      case 'error':
        return `${l.equipo} responde, pero no mostró sus aparatos: ${l.error ?? 'sin motivo'}`;
      case 'desconocido':
        if (!setup) return null;
        return l.id === 'casa'
          ? 'Cuando abras Nexo en tu Mac de la casa, sus aparatos aparecerán aquí.'
          : 'Cuando abras Nexo en tu PC de la oficina, sus aparatos aparecerán aquí.';
      default:
        return null;
    }
  }

  function abrir(p: Ubicada) {
    if (p.item.tipo === 'aparato' && p.aparatoId) onir('dispositivos', p.aparatoId);
    else onir('pantallas');
  }

  const fallas = (r: ResultadoAccion[]) => r.filter((x) => !x.ok);
  const plural = (n: number, uno: string, varios: string) => `${n} ${n === 1 ? uno : varios}`;
</script>

<!-- Miniatura de la paleta: la foto real o la ilustración del aparato. -->
{#snippet miniatura(forma: Forma, marca: string | null)}
  <span class="mini" aria-hidden="true">
    {#if forma.foto}
      <img src={forma.foto} alt="" draggable="false" decoding="async" />
    {:else if forma.ilustracion && forma.ilustracion !== 'torre'}
      <Ilustracion tipo={forma.ilustracion} {marca} />
    {:else}
      <Icono nombre="dispositivos" tamano={22} />
    {/if}
  </span>
{/snippet}

{#snippet selector()}
  <div class="segmentado vidrio" role="group" aria-label="Lugar">
    {#each LUGARES as l (l.id)}
      <button
        type="button"
        class:elegido={lugarSel === l.id}
        aria-pressed={lugarSel === l.id}
        onclick={() => {
          lugarSel = l.id;
          seleccion = null;
          ficha = null;
        }}
      >
        {l.nombre}
      </button>
    {/each}
  </div>
{/snippet}

<div class="setup">
  <header class="cabecera">
    <h1>Mi setup</h1>
    <p>Tus escenas de un toque y tus dispositivos donde están, en la casa y en la oficina.</p>
  </header>

  <!-- ---------- Escenas ---------- -->
  <section class="escenas" aria-labelledby="setup-escenas">
    <div class="escenas__cabeza">
      <h2 class="rotulo" id="setup-escenas">Escenas</h2>
      <!-- Si no caben todas, la fila se desliza: estas flechas la mueven de a una tarjeta. -->
      {#if tiraDesborda}
        <div class="escenas__flechas">
          <button
            type="button"
            class="flecha-tira"
            aria-label="Escenas anteriores"
            use:tooltip={'Anteriores'}
            disabled={!tiraAtras}
            onclick={() => moverTira(-1)}
          >
            <Icono nombre="atras" tamano={16} />
          </button>
          <button
            type="button"
            class="flecha-tira"
            aria-label="Más escenas"
            use:tooltip={'Más escenas'}
            disabled={!tiraAdelante}
            onclick={() => moverTira(1)}
          >
            <Icono nombre="chevron" tamano={16} />
          </button>
        </div>
      {/if}
    </div>
    {#if escenas}
      <div class="tarjetas" bind:this={tira} onscroll={medirTira}>
        {#each escenas.escenas as e (e.id)}
          {@const activa = escenas.ultima?.id === e.id}
          {@const enCurso = corriendo?.id === e.id ? corriendo : null}
          {@const resultado = fin?.id === e.id ? fin : null}
          <article class="tarjeta vidrio" class:activa class:corriendo={!!enCurso} class:ocupada={!!corriendo && !enCurso}>
            <button
              type="button"
              class="tarjeta__toque"
              aria-busy={!!enCurso}
              aria-describedby="escena-{e.id}-estado"
              onclick={() => activar(e)}
            >
              <span class="tarjeta__escenario">
                <IlustracionEscena nombre={ilustracionDe(e.icono)} icono={iconoEscena(e.icono)} />
                {#if enCurso}
                  <!-- Avance calmo: crece paso a paso, sin moverse en bucle. -->
                  <span class="tarjeta__avance" aria-hidden="true">
                    <span style:transform="scaleX({(enCurso.indice + 1) / Math.max(1, enCurso.total)})"></span>
                  </span>
                {/if}
              </span>
              <span class="tarjeta__nombre">{e.nombre}</span>
              <span class="tarjeta__resumen">{resumen(e, nombrePor)}</span>
              <span class="tarjeta__estado" id="escena-{e.id}-estado" role="status" aria-live="polite">
                {#if enCurso}
                  <span class="girador" aria-hidden="true"></span>
                  <span>Paso {enCurso.indice + 1} de {enCurso.total} · {haciendo(enCurso.accion, nombrePor)}</span>
                {:else if resultado?.ok}
                  <Icono nombre="check-circulo" tamano={16} />
                  <span>Listo</span>
                {:else if resultado}
                  <Icono nombre="alerta" tamano={16} />
                  <span>Listo, salvo {plural(fallas(resultado.resultados).length, 'paso', 'pasos')}</span>
                {:else if activa && escenas.ultima}
                  <span class="punto-activa" aria-hidden="true"></span>
                  <span>Última que activaste · {cuando(escenas.ultima.momento)}</span>
                {:else}
                  <span class="tenue">{plural(e.acciones.length, 'paso', 'pasos')} · toca para activarla</span>
                {/if}
              </span>
            </button>
            {#if resultado && !resultado.ok}
              <ul class="tarjeta__fallas">
                {#each fallas(resultado.resultados) as r, i (i)}
                  <li><b>{paso(r.accion, nombrePor)}:</b> {r.error ?? 'no salió'}</li>
                {/each}
              </ul>
            {/if}
            <button
              type="button"
              class="tarjeta__editar"
              aria-label="Editar {e.nombre}"
              use:tooltip={'Editar'}
              onclick={() => (editor = { escena: e })}
            >
              <Icono nombre="editar" tamano={15} />
            </button>
          </article>
        {/each}
        <button type="button" class="tarjeta nueva vidrio" onclick={() => (editor = { escena: null })}>
          <span class="tarjeta__escenario"><IlustracionEscena nombre="nueva" icono="mas" /></span>
          <span class="tarjeta__nombre">Nueva escena</span>
          <span class="tarjeta__resumen">Junta aparatos, brillo, sonido y apps en un solo toque.</span>
        </button>
      </div>
    {:else}
      <div class="tarjetas" aria-busy="true" aria-label="Cargando escenas">
        {#each [0, 1, 2, 3] as i (i)}<div class="tarjeta cargando vidrio"><span class="tarjeta__escenario"></span></div>{/each}
      </div>
    {/if}
  </section>

  <!-- ---------- Mapa ---------- -->
  <section class="mapa" class:editando aria-labelledby="setup-mapa">
    <header class="mapa__cabeza">
      <div class="mapa__textos">
        <h2 class="t-titulo" id="setup-mapa">{editando ? 'Acomoda tu setup' : 'Tu espacio'}</h2>
        <p class="t-sub">
          {editando
            ? 'Arrastra cada cosa: a lo ancho, y hacia atrás o adelante. Suma lo que detecta Nexo y decoración.'
            : 'Verde, encendido o conectado; tenue, apagado. Toca un aparato para ver más.'}
        </p>
      </div>
      <div class="mapa__acciones">
        <span class="mapa__selector" class:siempre={editando}>{@render selector()}</span>
        {#if editando}
          <button type="button" class="boton-vidrio boton" onclick={restablecer}>Volver a como venía</button>
          <button type="button" class="boton-vidrio boton" onclick={cancelar}>Cancelar</button>
          <button type="button" class="boton-primario" disabled={guardando} aria-busy={guardando} onclick={listo}>
            {guardando ? 'Guardando…' : 'Listo'}
          </button>
        {:else}
          <button type="button" class="boton-vidrio boton" disabled={!setup || !mapa} onclick={entrar}>
            <Icono nombre="editar" tamano={16} />
            Editar mi setup
          </button>
        {/if}
      </div>
    </header>

    {#if editando}
      {@const info = lugares.find((l) => l.id === lugarSel)}
      <div class="edicion">
        <div class="edicion__escena">
          <Escritorio
            piezas={vistas[lugarSel]}
            etiqueta="Escritorio de {lugarSel === 'casa' ? 'la casa' : 'la oficina'}"
            editando
            {seleccion}
            tenue={info?.estado === 'apagada'}
            bind:escena={escenasEl[lugarSel]}
            onelegir={(id) => (seleccion = id)}
            onmover={mover}
            onsoltar={soltar}
            onquitar={quitar}
          />
          {#if elegida}
            <div class="inspector vidrio" role="toolbar" aria-label="Ajustar {elegida.nombre}">
              <span class="inspector__nombre">
                <b>{elegida.nombre}</b>
                <span>{elegida.forma.pared ? 'En la pared' : NOMBRE_CAPA[elegida.item.capa]}</span>
              </span>
              <span class="inspector__botones">
                <button
                  type="button"
                  disabled={!elegida.forma.pared && elegida.item.capa === 0}
                  onclick={() =>
                    mover(elegida.item.id, elegida.forma.pared ? { y: elegida.base - 4 } : { capa: elegida.item.capa - 1 })}
                >
                  <Icono nombre="chevron-arriba" tamano={16} />
                  {elegida.forma.pared ? 'Más arriba' : 'Más atrás'}
                </button>
                <button
                  type="button"
                  disabled={!elegida.forma.pared && elegida.item.capa === 2}
                  onclick={() =>
                    mover(elegida.item.id, elegida.forma.pared ? { y: elegida.base + 4 } : { capa: elegida.item.capa + 1 })}
                >
                  <Icono nombre="chevron-abajo" tamano={16} />
                  {elegida.forma.pared ? 'Más abajo' : 'Más adelante'}
                </button>
                <button
                  type="button"
                  disabled={elegida.item.escala <= 0.5}
                  onclick={() => mover(elegida.item.id, { escala: elegida.item.escala - 0.1 })}
                >
                  Más chico
                </button>
                <button
                  type="button"
                  disabled={elegida.item.escala >= 2}
                  onclick={() => mover(elegida.item.id, { escala: elegida.item.escala + 0.1 })}
                >
                  Más grande
                </button>
                <button type="button" class="peligro" onclick={() => quitar(elegida.item.id)}>
                  <Icono nombre="papelera" tamano={16} />
                  Quitar
                </button>
              </span>
            </div>
          {:else}
            <p class="pista">
              Elige algo para cambiar su tamaño o quitarlo. Con el teclado: flechas para mover, + y − para el tamaño
              y Suprimir para quitar.
            </p>
          {/if}
        </div>

        <aside class="paleta vidrio" aria-label="Para sumar">
          <h3 class="rotulo">Detectados por Nexo</h3>
          {#each detectados as g (g.equipo)}
            <p class="paleta__grupo">{g.titulo}</p>
            {#if g.lista.length}
              <ul class="paleta__lista">
                {#each g.lista as d (claveItem(d.tipo, d.equipo, d.ref))}
                  <li>
                    <button
                      type="button"
                      class="paleta__op"
                      onpointerdown={(e) => presionarPaleta(e, d.nombre, ponerDetectado(d))}
                      onpointermove={moverPaleta}
                      onpointerup={soltarPaleta}
                      onpointercancel={cancelarPaleta}
                      onclick={(e) => e.detail === 0 && ponerDetectado(d)()}
                    >
                      {@render miniatura(d.forma, d.marca)}
                      <span class="paleta__textos">
                        <span class="paleta__nombre">{d.nombre}</span>
                        <span class="paleta__detalle">{d.detalle}</span>
                      </span>
                      <Icono nombre="mas" tamano={16} />
                    </button>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="paleta__vacio">Todo lo de este equipo ya está en el mapa.</p>
            {/if}
          {:else}
            <p class="paleta__vacio">Nexo todavía no detecta aparatos.</p>
          {/each}

          <h3 class="rotulo paleta__deco">Decoración</h3>
          <ul class="paleta__lista">
            {#each Object.entries(DECOS) as [ref, d] (ref)}
              <li>
                <button
                  type="button"
                  class="paleta__op"
                  onpointerdown={(e) => presionarPaleta(e, d.nombre, ponerDeco(ref as Deco))}
                  onpointermove={moverPaleta}
                  onpointerup={soltarPaleta}
                  onpointercancel={cancelarPaleta}
                  onclick={(e) => e.detail === 0 && ponerDeco(ref as Deco)()}
                >
                  {@render miniatura(d.forma, null)}
                  <span class="paleta__textos">
                    <span class="paleta__nombre">{d.nombre}</span>
                    {#if d.forma.pared}<span class="paleta__detalle">En la pared</span>{/if}
                  </span>
                  <Icono nombre="mas" tamano={16} />
                </button>
              </li>
            {/each}
          </ul>
        </aside>
      </div>
    {:else}
      <div class="lugares">
        {#each lugares as l (l.id)}
          {@const linea = lineaLugar(l)}
          {@const piezas = vistas[l.id]}
          {@const conEstado = piezas.filter((p) => p.estado !== null).sort((a, b) => a.item.x - b.item.x)}
          <article class="lugar vidrio" class:activo={lugarSel === l.id} aria-labelledby="lugar-{l.id}">
            <header class="lugar__cabeza">
              <h3 id="lugar-{l.id}">{l.nombre}</h3>
              <span class="lugar__equipo {linea.tono}"><span class="punto" aria-hidden="true"></span>{linea.texto}</span>
            </header>
            <Escritorio
              {piezas}
              etiqueta="Escritorio de {l.id === 'casa' ? 'la casa' : 'la oficina'}"
              tenue={l.estado === 'apagada' || l.estado === 'error'}
              aviso={avisoLugar(l)}
              ficha={ficha?.lugar === l.id ? ficha.id : null}
              onabrir={abrir}
              onficha={(id) => (ficha = id ? { lugar: l.id, id } : null)}
            />
            {#if conEstado.length}
              <ul class="leyenda" aria-label="Estado en {l.nombre}">
                {#each conEstado as p (p.item.id)}
                  <li>
                    <button
                      type="button"
                      class="chip"
                      class:encendido={p.estado === 'encendido'}
                      onclick={() =>
                        p.local ? abrir(p) : (ficha = ficha?.id === p.item.id ? null : { lugar: l.id, id: p.item.id })}
                    >
                      <span class="punto" aria-hidden="true"></span>
                      <span>{p.nombre}</span>
                      <span class="solo-lector">: {p.estadoTexto}</span>
                      {#if p.bateria !== null}<span class="chip__nivel">{p.bateria} %</span>{/if}
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
  </section>

  <!-- ---------- Luces ---------- -->
  <aside class="luces vidrio" aria-labelledby="setup-luces">
    <span class="ficha-icono" style:--tono="var(--amarillo)"><Icono nombre="bombilla" tamano={19} /></span>
    <div class="t-textos">
      <h2 class="t-titulo" id="setup-luces">Luces</h2>
      <p class="t-sub">Cuando compres tus luces, aparecerán aquí y en las escenas.</p>
    </div>
  </aside>
</div>

{#if fantasma}
  <div class="fantasma flotante" style:left="{fantasma.x}px" style:top="{fantasma.y}px" aria-hidden="true">
    {fantasma.nombre}
  </div>
{/if}

<EditorEscena
  abierto={!!editor}
  escena={editor?.escena ?? null}
  aparatos={local?.dispositivos ?? []}
  pantallas={local?.pantallas ?? []}
  equipo={local?.equipo ?? ''}
  {remotos}
  mac={(local?.plataforma ?? 'mac') === 'mac'}
  onguardar={guardar}
  onborrar={borrar}
  oncerrar={() => (editor = null)}
/>

<style>
  .setup {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 24px;
    max-width: 1320px;
    margin: 0 auto;
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

  /* ---------- Escenas ---------- */
  .escenas {
    display: grid;
    gap: 10px;
    min-width: 0;
  }
  .escenas__cabeza {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 28px;
  }
  .escenas h2 {
    margin: 0;
  }
  .escenas__flechas {
    display: flex;
    gap: 4px;
  }
  .flecha-tira {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-2);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .flecha-tira:hover:not(:disabled) {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .flecha-tira:disabled {
    color: var(--texto-3);
    opacity: 0.5;
    cursor: default;
  }

  /* Una fila pareja: las tarjetas se estiran si sobra ancho y se deslizan (con imán) si falta.
     El relleno deja ver el canto, la sombra y el foco de cada tarjeta; la barra no se ve. */
  .tarjetas {
    display: flex;
    gap: 12px;
    margin: -4px -4px -26px;
    padding: 4px 4px 26px;
    overflow-x: auto;
    overscroll-behavior-x: contain;
    scroll-snap-type: x mandatory;
    scroll-padding-inline: 4px;
    scrollbar-width: none;
  }
  .tarjetas::-webkit-scrollbar {
    display: none;
  }
  .tarjeta {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1 0 236px;
    min-width: 0;
    border-radius: 20px;
    scroll-snap-align: start;
    transition: background-color var(--dur-color) ease;
  }
  .tarjeta.cargando {
    min-height: 236px;
    padding: 8px;
  }
  .tarjeta__toque,
  .tarjeta.nueva {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 4px;
    flex: 1;
    width: 100%;
    padding: 8px 8px 14px;
    border: 0;
    border-radius: 20px;
    background: transparent;
    color: var(--texto);
    text-align: left;
    overflow-wrap: break-word;
  }
  /* "Nueva escena" es más angosta: así la fila termina pareja, sin un hueco grande. */
  .tarjeta.nueva {
    flex: 0 0 184px;
    background: var(--relleno-tarjeta);
  }
  .tarjeta:hover:not(.activa),
  .tarjeta.nueva:hover {
    background: var(--superficie-hover);
  }
  .tarjeta__toque:active,
  .tarjeta.nueva:active {
    transform: scale(0.98);
  }
  .tarjeta.ocupada .tarjeta__toque {
    cursor: progress;
  }

  /* El escenario: la ilustración sobre un fondo apenas teñido de turquesa, con esquinas suaves. */
  .tarjeta__escenario {
    position: relative;
    display: block;
    height: 124px;
    margin-bottom: 8px;
    padding: 6px 4px 0;
    border-radius: 14px;
    background: color-mix(in srgb, var(--primario) 6%, var(--superficie-inserto));
    overflow: hidden;
  }
  .tarjeta.activa .tarjeta__escenario {
    background: color-mix(in srgb, var(--primario) 12%, var(--superficie));
  }
  .tarjeta.nueva .tarjeta__escenario {
    background: var(--superficie-inserto);
  }
  .tarjeta.cargando .tarjeta__escenario {
    background: var(--superficie-inserto);
  }
  /* Mientras corre: una línea fina al pie del escenario que crece con cada paso. */
  .tarjeta__avance {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 8px;
    height: 3px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--primario) 22%, transparent);
    overflow: hidden;
  }
  .tarjeta__avance span {
    display: block;
    height: 100%;
    background: var(--primario);
    transform-origin: left center;
    transition: transform var(--dur-vista) ease;
  }

  .tarjeta__nombre,
  .tarjeta__resumen,
  .tarjeta__estado {
    padding: 0 6px;
  }
  .tarjeta__nombre {
    font-size: 16px;
    line-height: 22px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .tarjeta__resumen {
    font: var(--letra-pequena);
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .tarjeta__estado {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin-top: auto;
    padding-top: 6px;
    font-size: 12.5px;
    line-height: 18px;
    font-weight: 600;
    color: var(--texto-2);
  }
  .tarjeta__estado .tenue {
    font-weight: 500;
    color: var(--texto-3);
  }
  .tarjeta__estado :global(svg),
  .tarjeta__estado .girador,
  .punto-activa {
    flex-shrink: 0;
    margin-top: 1px;
  }
  .tarjeta__estado .girador {
    width: 15px;
    height: 15px;
    color: var(--primario-texto);
  }
  .tarjeta__estado :global(svg) {
    color: var(--exito);
  }
  .punto-activa {
    width: 7px;
    height: 7px;
    margin-top: 6px;
    border-radius: 50%;
    background: var(--primario);
  }
  .tarjeta.activa .tarjeta__estado,
  .tarjeta.corriendo .tarjeta__estado {
    color: var(--elegido-fg);
  }
  .tarjeta:has(.tarjeta__fallas) .tarjeta__estado,
  .tarjeta:has(.tarjeta__fallas) .tarjeta__estado :global(svg) {
    color: var(--aviso);
  }
  .tarjeta__fallas {
    display: grid;
    gap: 4px;
    margin: -6px 0 0;
    padding: 0 14px 14px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
    overflow-wrap: break-word;
  }
  .tarjeta__fallas b {
    color: var(--texto);
    font-weight: 600;
  }
  /* El lápiz: un botón redondo chico sobre la esquina del escenario. */
  .tarjeta__editar {
    position: absolute;
    top: 14px;
    right: 14px;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--superficie);
    box-shadow:
      0 0 0 1px var(--borde),
      var(--sombra-panel);
    color: var(--texto-2);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .tarjeta__editar:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .tarjeta__editar:active {
    transform: scale(0.96);
  }
  /* La última activada: lo elegido (turquesa suave, borde fino, nombre en turquesa). */
  .tarjeta.activa {
    background: var(--elegido-bg);
    box-shadow:
      0 0 0 1px var(--elegido-borde),
      var(--sombra-tarjeta);
  }
  .tarjeta.activa .tarjeta__nombre {
    color: var(--elegido-fg);
  }

  /* ---------- Mapa ---------- */
  .mapa {
    display: grid;
    gap: 14px;
  }
  .mapa__cabeza {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px 16px;
  }
  .mapa__textos {
    flex: 1 1 280px;
    min-width: 0;
  }
  .mapa__acciones {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .boton {
    background: var(--relleno-tarjeta);
    box-shadow:
      inset 0 1px 0 var(--luz-tope),
      0 0 0 1px var(--borde-tarjeta);
    color: var(--texto);
  }
  .boton:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .boton:disabled {
    color: var(--texto-3);
    cursor: default;
  }

  .segmentado {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--radio-pildora);
  }
  .segmentado button {
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

  /* Los dos lugares lado a lado si hay espacio; si no, uno a la vez con el selector. */
  .lugares {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  .lugar:not(.activo) {
    display: none;
  }
  @container (min-width: 900px) {
    .lugares {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .lugar:not(.activo) {
      display: flex;
    }
    .mapa__selector:not(.siempre) {
      display: none;
    }
  }
  .lugar {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
    padding: 14px 14px 16px;
    border-radius: var(--radio-lg);
  }
  .lugar__cabeza {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 12px;
    padding: 2px 4px 0;
  }
  .lugar__cabeza h3 {
    margin: 0;
    font: var(--letra-panel);
    font-size: 16px;
  }
  .lugar__equipo {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: var(--letra-pequena);
    color: var(--texto-2);
  }
  .punto {
    width: 8px;
    height: 8px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--texto-3);
  }
  .lugar__equipo.ok .punto,
  .chip.encendido .punto {
    background: var(--exito);
    box-shadow: 0 0 0 2.5px color-mix(in srgb, var(--exito) 20%, transparent);
  }
  .lugar__equipo.aviso .punto {
    background: var(--aviso);
  }

  .leyenda {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    padding: 0 2px;
    list-style: none;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 3px 10px;
    border: 0;
    border-radius: var(--radio-pildora);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-2);
    font-size: 12.5px;
    line-height: 17px;
    font-weight: 550;
    text-align: left;
    transition: background-color var(--dur-color) ease;
  }
  .chip:hover {
    background: var(--superficie-hover);
  }
  .chip.encendido {
    color: var(--texto);
  }
  .chip__nivel {
    font-variant-numeric: tabular-nums;
    color: var(--texto-2);
  }

  /* ---------- Edición ---------- */
  .edicion {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }
  @container (min-width: 820px) {
    .edicion {
      grid-template-columns: minmax(0, 1fr) 290px;
    }
  }
  .edicion__escena {
    display: grid;
    gap: 12px;
    min-width: 0;
  }
  .inspector {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 16px;
    padding: 10px 12px 10px 16px;
    border-radius: var(--radio-md);
  }
  .inspector__nombre {
    display: grid;
    flex: 1 1 160px;
    min-width: 0;
    font-size: 13px;
    line-height: 18px;
    overflow-wrap: break-word;
  }
  .inspector__nombre span {
    color: var(--texto-2);
    font-size: 12.5px;
  }
  .inspector__botones {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .inspector__botones button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--radio-sm);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font-size: 12.5px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .inspector__botones button:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .inspector__botones button:disabled {
    color: var(--texto-3);
    cursor: default;
  }
  .inspector__botones .peligro {
    color: var(--error);
  }
  .pista {
    margin: 0;
    padding: 0 4px;
    font: var(--letra-pequena);
    color: var(--texto-2);
  }

  .paleta {
    display: grid;
    gap: 8px;
    padding: 14px 12px;
    border-radius: var(--radio-lg);
  }
  .paleta h3 {
    margin: 0 4px;
  }
  .paleta__deco {
    margin-top: 10px !important;
  }
  .paleta__grupo {
    margin: 4px 4px 0;
    font: var(--letra-pequena);
    font-weight: 600;
    color: var(--texto-2);
  }
  .paleta__vacio {
    margin: 0 4px;
    font: var(--letra-pequena);
    color: var(--texto-3);
  }
  .paleta__lista {
    display: grid;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .paleta__op {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 44px;
    padding: 4px 10px 4px 6px;
    border: 0;
    border-radius: var(--radio-sm);
    background: transparent;
    color: var(--texto);
    text-align: left;
    cursor: grab;
    touch-action: none;
    transition: background-color var(--dur-color) ease;
  }
  .paleta__op:hover {
    background: var(--superficie-hover);
  }
  .paleta__op > :global(svg) {
    flex-shrink: 0;
    color: var(--primario-texto);
  }
  .mini {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 38px;
    height: 38px;
    padding: 3px;
    border-radius: 10px;
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-2);
  }
  .mini img,
  .mini :global(svg) {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .paleta__textos {
    display: grid;
    flex: 1;
    min-width: 0;
  }
  .paleta__nombre {
    font-size: 13px;
    line-height: 18px;
    font-weight: 600;
    overflow-wrap: break-word;
  }
  .paleta__detalle {
    font-size: 12px;
    line-height: 16px;
    color: var(--texto-2);
  }

  .fantasma {
    position: fixed;
    z-index: 70;
    padding: 6px 10px;
    border-radius: var(--radio-sm);
    font-size: 12.5px;
    font-weight: 600;
    pointer-events: none;
    translate: 12px 12px;
  }

  /* ---------- Luces ---------- */
  .luces {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 18px;
    border-radius: var(--radio-lg);
  }
</style>
