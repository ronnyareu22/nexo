<!--
  Remoto: la Mac desde el PC del trabajo, con RustDesk por Tailscale. De arriba abajo:
  1. El estado de la sesión: la ilustración (el PC de la oficina, la Mac de la casa y Tailscale al medio: conectada,
     lista o apagada), el estado, las acciones y los servicios en fichas.
  2. Windows: "Tu Mac", su pantalla virtual, las opciones de la sesión y el modo trabajo, manejados por XDrop
     (api-remoto-mac.ts), y el enlace al estado de la Mac (sección "analitica"); debajo, cómo ver la pantalla
     virtual en RustDesk. Mac: la pantalla virtual para RustDesk, con sus ajustes y, al lado, cómo verla.
  3. Los atajos de Windows que Nexo traduce en la Mac (con espacio, en dos mitades).
  4. En la Mac, una rejilla de dos columnas: la captura y la comodidad durante la sesión a la izquierda y el
     portapapeles a la derecha. En Windows queda solo la captura, a lo ancho.
  Todo se acomoda con consultas de contenedor: una columna bajo unos 760 px y dos por encima. Sin alturas fijas.
  Solo con el perfil de trabajo remoto encendido (perfil.svelte.ts: Nexo encontró tu Mac o tu PC, o "Siempre" en
  Ajustes). Si no, la guía "Conecta tus equipos" es lo principal, con lo que se activa al lado; con "Nunca", una
  nota que lleva a Ajustes. Encendido, si falta algo en este equipo (Tailscale o RustDesk), la guía va arriba, chica.
  En Windows solo se conecta: la traducción, la captura y el portapapeles los hace Nexo en la Mac.
  Se relee al llegar "remoto-cambio" y cada 5 s mientras se ve; la Mac desde Windows, cada 10 s; el historial,
  al llegar "portapapeles-cambio".
  Superficies sólidas, el hover solo cambia el color y ningún texto se corta: todo baja de línea.
-->
<script lang="ts" module>
  // Si se cerró la guía de arriba, no vuelve mientras el panel siga abierto.
  let guiaCerrada = false;
</script>

<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import {
    abrirAjustesPrivacidad,
    abrirCapturas,
    abrirRustDesk,
    ajustarRemoto,
    alCambiarPortapapeles,
    alCambiarRemoto,
    alCapturar,
    borrarPortapapeles,
    capturarRemoto,
    conectarMac,
    copiarDelHistorial,
    esMac,
    leerPortapapeles,
    leerRemoto,
    pedirPermiso,
    prepararRemoto,
  } from '$lib/api-sistema';
  import type {
    AjustesRemoto,
    AtajoRemoto,
    CambioRemoto,
    CapturaRemota,
    DestinoDrop,
    EntradaPortapapeles,
    EstadoMac,
    EstadoRemoto,
  } from '$lib/tipos-sistema';
  import { cuando } from '$lib/tiempo';
  import { tooltip } from '$lib/tooltip';
  import {
    ajustarPantallaVirtual,
    alCambiarPantallaVirtual,
    claveResolucion,
    leerPantallaVirtual,
    opcionesResolucionVirtual,
    resolucionDe,
    usarPantallaVirtual,
  } from '$lib/api-pantalla-virtual';
  import { ajustarMac, leerMac, macDelPerfil, usarPantallaVirtualMac } from '$lib/api-remoto-mac';
  import { perfil, recargar } from '$lib/perfil.svelte';
  import GuiaConexion from '$lib/componentes/GuiaConexion.svelte';
  import type { EstadoPantallaVirtual } from '$lib/tipos-sistema';
  import Icono, { type Nombre } from '$lib/componentes/Icono.svelte';
  import Interruptor from '$lib/componentes/Interruptor.svelte';
  import MenuDesplegable from '$lib/componentes/MenuDesplegable.svelte';
  import Dialogo from '$lib/componentes/Dialogo.svelte';
  import IlustracionEscena from '$lib/componentes/IlustracionEscena.svelte';

  let {
    onerror,
    onir,
  }: {
    onerror: (texto: string) => void;
    /** Lleva a otra sección (la usa "Ver el estado de la Mac"). */
    onir?: (seccion: string) => void;
  } = $props();

  let remoto = $state<EstadoRemoto | null>(null);
  let historial = $state<EntradaPortapapeles[] | null>(null);

  // --- Perfil de trabajo remoto ---
  /** El nombre de tu Mac en Tailscale (en Windows), para los textos. */
  const nombreMac = $derived(perfil.otro?.plataforma === 'mac' ? perfil.otro.nombre : 'tu Mac');
  // Falta algo en este equipo (Tailscale o RustDesk; en la Mac, también el acceso directo por IP).
  const faltaAqui = $derived.by(() => {
    const e = perfil.estado;
    if (!e) return false;
    const rd = esMac ? e.rustdesk.instalado && e.rustdesk.accesoDirecto === true : e.rustdesk.instalado;
    return e.tailscale !== 'activo' || !rd;
  });
  // Con el perfil encendido, la guía va arriba si falta algo aquí o si se estaba viendo cuando apareció tu equipo.
  let vioGuia = false;
  let guiaArriba = $state(false);
  $effect(() => {
    if (perfil.leido && !perfil.activo) vioGuia = true;
    if (perfil.activo && (faltaAqui || vioGuia) && !guiaCerrada) guiaArriba = true;
  });
  function cerrarGuia() {
    guiaCerrada = true;
    guiaArriba = false;
  }
  // Al encenderse el perfil (o al llegar la primera lectura), lo del remoto se lee enseguida.
  $effect(() => {
    if (!perfil.activo) return;
    untrack(() => {
      leer();
      leerVirtual();
      leerTuMac();
    });
  });
  const desbloquea: { icono: Nombre; texto: string }[] = esMac
    ? [
        { icono: 'teclado', texto: 'Tus atajos de Windows, traducidos en esta Mac' },
        { icono: 'captura', texto: 'Capturas con Imprimir Pantalla, listas para pegar' },
        { icono: 'pantallas', texto: 'Una segunda pantalla para verla en RustDesk' },
      ]
    : [
        { icono: 'conectar', texto: 'Conectarte a tu Mac con un clic' },
        { icono: 'pantallas', texto: 'Encender su pantalla virtual desde aquí' },
        { icono: 'teclado', texto: 'Usar tus atajos de Windows en la Mac' },
      ];

  // --- Lectura ---
  // Cada cambio de ajuste sube la versión: una lectura que empezó antes no pisa lo recién elegido.
  let version = 0;
  let leyendo = false;
  async function leer() {
    if (leyendo || (perfil.leido && !perfil.activo)) return;
    leyendo = true;
    const desde = version;
    try {
      const r = await leerRemoto();
      if (desde === version) remoto = r;
    } catch (e) {
      if (!remoto) onerror(String(e));
    }
    leyendo = false;
  }

  async function leerHistorial() {
    if (!esMac) return;
    try {
      historial = await leerPortapapeles();
    } catch {
      historial ??= [];
    }
  }

  // --- Ajustes ---
  type Clave = keyof AjustesRemoto | 'modoTrabajo';
  let ocupado = $state<Clave | null>(null);

  async function ajustar(clave: Clave, valor: boolean) {
    if (!remoto || ocupado) return;
    ocupado = clave;
    version++;
    const cambio: CambioRemoto = {};
    cambio[clave] = valor;
    try {
      remoto = await ajustarRemoto(cambio);
    } catch (e) {
      onerror(String(e));
    }
    ocupado = null;
  }

  // --- Pantalla virtual para RustDesk (solo Mac) ---
  let virtual = $state<EstadoPantallaVirtual | null>(null);
  type ClaveVirtual = 'activa' | 'resolucion' | 'crearAlEmpezar' | 'quitarAlTerminar';
  let ocupadoVirtual = $state<ClaveVirtual | null>(null);

  async function leerVirtual() {
    if (!esMac || ocupadoVirtual) return;
    try {
      virtual = await leerPantallaVirtual();
    } catch {
      // La siguiente lectura lo corrige.
    }
  }

  // Encenderla tarda un par de segundos: macOS la agrega y la ordena junto a la de la MacBook.
  async function alternarVirtual() {
    const v = virtual;
    if (!v || ocupadoVirtual) return;
    ocupadoVirtual = 'activa';
    try {
      virtual = await usarPantallaVirtual(!v.activa, v.ancho, v.alto);
    } catch (e) {
      onerror(String(e));
    }
    ocupadoVirtual = null;
  }

  // Apagada, solo se guarda la que tendrá al encenderla.
  async function resolucionVirtual(clave: string) {
    const v = virtual;
    const r = resolucionDe(clave);
    if (!v || !r || ocupadoVirtual) return;
    ocupadoVirtual = 'resolucion';
    try {
      virtual = await usarPantallaVirtual(v.activa, r.ancho, r.alto);
    } catch (e) {
      onerror(String(e));
    }
    ocupadoVirtual = null;
  }

  async function opcionVirtual(clave: 'crearAlEmpezar' | 'quitarAlTerminar') {
    const v = virtual;
    if (!v || ocupadoVirtual) return;
    ocupadoVirtual = clave;
    try {
      virtual = await ajustarPantallaVirtual({ [clave]: !v[clave] });
    } catch (e) {
      onerror(String(e));
    }
    ocupadoVirtual = null;
  }

  // --- Tu Mac (solo Windows): su pantalla virtual, la sesión y el modo trabajo, por XDrop ---
  const NO_RESPONDE = 'Tu Mac no responde: revisa que Nexo esté abierto allá.';
  // undefined mientras se busca la primera vez; null si no está entre tus equipos de Tailscale.
  let destinoMac = $state<DestinoDrop | null | undefined>(undefined);
  let mac = $state<EstadoMac | null>(null);
  let errorMac = $state<string | null>(null);
  let buscandoMac = $state(false);
  type ClaveMac = ClaveVirtual | 'modoTrabajo';
  let ocupadoMac = $state<ClaveMac | null>(null);
  let versionMac = 0;

  // La Mac es la del perfil. Se vuelve a buscar la primera vez, si no apareció y cada vez que no respondió.
  async function leerTuMac() {
    if (esMac || buscandoMac || ocupadoMac || !perfil.activo) return;
    buscandoMac = true;
    const desde = versionMac;
    try {
      let d = destinoMac;
      if (!d || errorMac) {
        await recargar(true);
        d = macDelPerfil(perfil.otro);
        destinoMac = d;
      }
      if (!d) {
        mac = null;
        errorMac = null;
      } else if (!d.enLinea) {
        if (desde === versionMac) errorMac = NO_RESPONDE;
      } else {
        const e = await leerMac(d.ip);
        if (desde === versionMac) {
          mac = e;
          errorMac = null;
        }
      }
    } catch (e) {
      destinoMac ??= null;
      if (desde === versionMac) errorMac = String(e);
    }
    buscandoMac = false;
  }

  async function cambiarMac(clave: ClaveMac, hacer: (ip: string) => Promise<EstadoMac>) {
    const d = destinoMac;
    if (!d || !mac || ocupadoMac) return;
    ocupadoMac = clave;
    versionMac++;
    try {
      mac = await hacer(d.ip);
      errorMac = null;
    } catch (e) {
      onerror(String(e));
    }
    ocupadoMac = null;
  }

  function alternarVirtualMac() {
    const v = mac?.pantallaVirtual;
    if (v) cambiarMac('activa', (ip) => usarPantallaVirtualMac(ip, !v.activa, v.ancho, v.alto));
  }

  function resolucionMac(clave: string) {
    const v = mac?.pantallaVirtual;
    const r = resolucionDe(clave);
    if (v && r) cambiarMac('resolucion', (ip) => usarPantallaVirtualMac(ip, v.activa, r.ancho, r.alto));
  }

  function opcionMac(clave: 'crearAlEmpezar' | 'quitarAlTerminar') {
    const v = mac?.pantallaVirtual;
    if (v) cambiarMac(clave, (ip) => ajustarMac(ip, { [clave]: !v[clave] }));
  }

  function modoTrabajoMac() {
    const e = mac;
    if (e) cambiarMac('modoTrabajo', (ip) => ajustarMac(ip, { modoTrabajo: !e.modoTrabajo }));
  }

  // --- Estado de arriba ---
  type Tono = 'ok' | 'aviso' | 'apagado';

  const resumen = $derived.by((): { titulo: string; detalle: string; tono: Tono } | null => {
    const r = remoto;
    if (!r) return null;
    if (!esMac)
      return {
        titulo: 'Tu Mac, a un clic',
        detalle: !r.rustdesk
          ? 'Instala RustDesk desde rustdesk.com y vuelve aquí: el botón se activa solo.'
          : r.tailscale !== 'activo'
            ? 'Enciende Tailscale en esta PC para llegar directo a tu Mac.'
            : !r.ipMac
              ? 'Todavía no encontré tu Mac en Tailscale. Revisa que esté encendida y con tu misma cuenta.'
              : `Abre RustDesk conectado directo a ${nombreMac} por Tailscale (${r.ipMac}).`,
        tono: r.rustdesk && r.tailscale === 'activo' && r.ipMac ? 'ok' : 'apagado',
      };
    if (r.activa && r.rele)
      return {
        titulo: 'Conectado por el servidor de RustDesk',
        detalle: r.ipMac
          ? `Va más lento. Para ir directo, en RustDesk escribe ${r.ipMac} en lugar del ID.`
          : 'Va más lento. Para ir directo, conecta Tailscale en esta Mac y entra con su dirección en lugar del ID.',
        tono: 'aviso',
      };
    if (r.activa)
      return {
        titulo: `Conectado desde ${r.equipo ?? r.ipRemota ?? 'tu PC'}`,
        detalle: r.directa
          ? `Directo por Tailscale${r.equipo && r.ipRemota ? ` · ${r.ipRemota}` : ''}`
          : 'Directo por la red',
        tono: 'ok',
      };
    return {
      titulo: 'Sin sesión remota',
      detalle: r.modoTrabajo
        ? r.ipMac
          ? `Lista para el trabajo. Desde tu PC, en RustDesk escribe ${r.ipMac}.`
          : 'Lista para el trabajo. Conecta Tailscale para entrar directo desde tu PC.'
        : 'Cuando te conectes desde el trabajo, lo verás aquí.',
      tono: 'apagado',
    };
  });

  // La línea entre el PC y la Mac: viva (directa), por el relé o sin sesión. Tiñe el fondo del dibujo.
  const ruta = $derived.by((): 'viva' | 'rele' | 'apagada' => {
    const r = remoto;
    if (!r || !esMac || !r.activa) return 'apagada';
    return r.rele ? 'rele' : 'viva';
  });

  // La ilustración de arriba: conectada (línea firme y visto), lista (línea punteada) o apagada (sin sesión).
  const escenaRemoto = $derived(
    ruta === 'viva'
      ? 'remoto-conectado'
      : ruta === 'rele' || resumen?.tono === 'ok' || remoto?.modoTrabajo
        ? 'remoto'
        : 'remoto-sin',
  );

  const servicios = $derived.by((): { nombre: string; valor: string; tono: Tono }[] => {
    const r = remoto;
    if (!r) return [];
    const tailscale: Record<EstadoRemoto['tailscale'], [string, Tono]> = {
      activo: ['Conectado', 'ok'],
      detenido: ['Detenido', 'aviso'],
      'sin sesion': ['Sin sesión', 'aviso'],
      'no instalado': ['No instalado', 'apagado'],
    };
    const [valorTs, tonoTs] = tailscale[r.tailscale];
    const lista: { nombre: string; valor: string; tono: Tono }[] = [{ nombre: 'Tailscale', valor: valorTs, tono: tonoTs }];
    if (esMac) {
      lista.push({
        nombre: 'RustDesk',
        valor: r.rustdesk ? 'Abierto' : r.rustdeskInstalado ? 'Cerrado' : 'No instalado',
        tono: r.rustdesk ? 'ok' : r.rustdeskInstalado ? 'aviso' : 'apagado',
      });
      const despierta = r.ajustes.evitarReposo && (r.activa || r.modoTrabajo);
      lista.push({ nombre: 'Reposo', valor: despierta ? 'La Mac no se dormirá' : 'Normal', tono: despierta ? 'ok' : 'apagado' });
      const pc = perfil.otro;
      if (pc) lista.push({ nombre: 'Tu PC', valor: pc.nombre, tono: pc.nexo ? 'ok' : 'apagado' });
    } else {
      lista.push({ nombre: 'RustDesk', valor: r.rustdesk ? 'Instalado' : 'No instalado', tono: r.rustdesk ? 'ok' : 'aviso' });
    }
    return lista;
  });

  // "Tu Mac" (Windows): en línea, sin respuesta, sin encontrar o buscando.
  const estadoTuMac = $derived.by((): { texto: string; tono: Tono } => {
    if (destinoMac === undefined) return { texto: 'Buscando tu Mac en Tailscale…', tono: 'apagado' };
    if (destinoMac === null) return { texto: 'No aparece entre tus equipos de Tailscale', tono: 'aviso' };
    if (errorMac) return { texto: `${destinoMac.nombre} · Sin respuesta`, tono: 'aviso' };
    if (!mac) return { texto: `${destinoMac.nombre} · Revisando…`, tono: 'apagado' };
    return { texto: `${mac.equipo || destinoMac.nombre} · En línea`, tono: 'ok' };
  });

  const sesionMac = $derived.by((): { titulo: string; detalle: string; tono: Tono } | null => {
    const s = mac?.sesion;
    if (!s) return null;
    if (s.activa && s.rele)
      return {
        titulo: 'Conectada por el relé de RustDesk',
        detalle: 'Va más lento. Para ir directo, conéctate con «Conectar a mi Mac».',
        tono: 'aviso',
      };
    if (s.activa)
      return {
        titulo: `Conectada desde ${s.equipo ?? 'tu PC'}`,
        detalle: s.directa ? 'Directo por Tailscale.' : 'Directo por la red.',
        tono: 'ok',
      };
    return { titulo: 'Nadie conectado ahora', detalle: 'Entra con «Conectar a mi Mac», arriba.', tono: 'apagado' };
  });

  let preparando = $state(false);
  let avisoAccion = $state<{ texto: string; tono: Tono } | null>(null);
  let temporizadorAviso: ReturnType<typeof setTimeout> | undefined;
  function mostrarAviso(texto: string, tono: Tono) {
    avisoAccion = { texto, tono };
    clearTimeout(temporizadorAviso);
    temporizadorAviso = setTimeout(() => (avisoAccion = null), 9000);
  }

  async function preparar() {
    if (preparando) return;
    preparando = true;
    version++;
    try {
      const r = await prepararRemoto();
      remoto = r;
      if (r.tailscale !== 'activo')
        mostrarAviso(
          r.tailscale === 'no instalado'
            ? 'Instala Tailscale para entrar directo desde el trabajo.'
            : 'Tailscale no se conectó: haz clic en su ícono de la barra de menús y pulsa Connect.',
          'aviso',
        );
      else if (!r.rustdesk) mostrarAviso('RustDesk no se abrió. Ábrelo desde Aplicaciones.', 'aviso');
      else mostrarAviso('Listo: Tailscale conectado, RustDesk abierto y la Mac no se dormirá.', 'ok');
    } catch (e) {
      onerror(String(e));
    }
    preparando = false;
  }

  async function abrirRD() {
    try {
      await abrirRustDesk();
    } catch (e) {
      onerror(String(e));
    }
  }

  let conectando = $state(false);
  async function conectar() {
    if (conectando) return;
    conectando = true;
    try {
      await conectarMac();
      mostrarAviso('Abriendo RustDesk hacia tu Mac…', 'ok');
    } catch (e) {
      onerror(String(e));
    }
    setTimeout(() => (conectando = false), 2500);
  }

  // --- Atajos ---
  // Recogidos: lo de todos los días (los primeros de Edición y los escritorios). "Ver todos" muestra la tabla entera.
  let todos = $state(false);
  const grupos = $derived.by(() => {
    const mapa = new Map<string, AtajoRemoto[]>();
    for (const a of remoto?.atajos ?? []) {
      const lista = mapa.get(a.grupo) ?? [];
      lista.push(a);
      mapa.set(a.grupo, lista);
    }
    const lista = [...mapa].map(([nombre, atajos]) => ({ nombre, atajos }));
    if (todos) return lista;
    const breves = lista
      .filter((g) => g.nombre === 'Edición' || g.nombre === 'Escritorios y apps')
      .map((g) => (g.nombre === 'Edición' ? { ...g, atajos: g.atajos.slice(0, 5) } : g));
    return breves.length ? breves : lista.slice(0, 2);
  });
  const totalAtajos = $derived(remoto?.atajos.length ?? 0);

  /** Las teclas en palabras, para lectores de pantalla ("Comando más C"). */
  const NOMBRES_TECLAS: Record<string, string> = {
    '⌘': 'Comando',
    '⌥': 'Opción',
    '⌃': 'Control',
    '⇧': 'Mayúsculas',
    '⌫': 'Borrar',
    '⇥': 'Tab',
    '← →': 'flecha izquierda o derecha',
    '←': 'flecha izquierda',
    '→': 'flecha derecha',
    '↑': 'flecha arriba',
    '↓': 'flecha abajo',
  };
  const enPalabras = (teclas: string[]) => teclas.map((t) => NOMBRES_TECLAS[t] ?? t).join(' más ');
  const esSimbolo = (t: string) => /^[⌘⌥⌃⇧⌫⇥←→↑↓ ]+$/.test(t);

  // --- Captura ---
  let capturando = $state(false);
  let ultimaCaptura = $state<CapturaRemota | null>(null);
  let capturaCancelada = $state(false);

  async function probarCaptura() {
    if (capturando) return;
    capturando = true;
    capturaCancelada = false;
    try {
      const c = await capturarRemoto();
      if (c) ultimaCaptura = c;
      else capturaCancelada = true;
    } catch (e) {
      onerror(String(e));
    }
    capturando = false;
  }

  async function abrirCarpeta() {
    try {
      await abrirCapturas();
    } catch (e) {
      onerror(String(e));
    }
  }

  async function pedirPantalla() {
    try {
      await pedirPermiso('pantalla');
    } catch (e) {
      onerror(String(e));
    }
  }

  function abrirAccesibilidad() {
    abrirAjustesPrivacidad('accesibilidad').catch((e) => onerror(String(e)));
  }

  // --- Portapapeles ---
  const LIMITE_VISTA = 100;
  const LIMITE_TOOLTIP = 600;

  /** Las dos primeras líneas con texto, hasta unos 100 caracteres (cortando en un espacio). Sin "…": se dice cuánto falta. */
  function vista(e: EntradaPortapapeles): { texto: string; faltan: number } {
    const completo = e.texto ?? '';
    const lineas = completo
      .replace(/\r/g, '')
      .split('\n')
      .filter((l) => l.trim() !== '');
    let texto = lineas.slice(0, 2).join('\n');
    if (texto.length > LIMITE_VISTA) {
      const espacio = texto.lastIndexOf(' ', LIMITE_VISTA);
      texto = texto.slice(0, espacio > LIMITE_VISTA * 0.6 ? espacio : LIMITE_VISTA).trimEnd();
    }
    const total = e.largo ?? [...completo].length;
    return { texto, faltan: Math.max(0, total - [...texto].length) };
  }

  /** El texto completo en el tooltip, solo si no se ve entero. */
  function tooltipDe(e: EntradaPortapapeles, faltan: number): string {
    if (faltan <= 0) return '';
    const completo = e.texto ?? '';
    if ([...completo].length <= LIMITE_TOOLTIP) return completo;
    const corto = [...completo].slice(0, LIMITE_TOOLTIP).join('');
    return `${corto} (y ${miles((e.largo ?? 0) - LIMITE_TOOLTIP)} caracteres más)`;
  }

  const miles = (n: number) => n.toLocaleString('es');

  let copiadoId = $state<number | null>(null);
  let copiandoId = $state<number | null>(null);
  let temporizadorCopia: ReturnType<typeof setTimeout> | undefined;

  async function copiar(e: EntradaPortapapeles) {
    if (copiandoId !== null) return;
    copiandoId = e.id;
    try {
      await copiarDelHistorial(e.id);
      copiadoId = e.id;
      clearTimeout(temporizadorCopia);
      temporizadorCopia = setTimeout(() => (copiadoId = null), 1800);
      await leerHistorial();
    } catch (err) {
      onerror(String(err));
    }
    copiandoId = null;
  }

  let preguntandoBorrar = $state(false);
  async function borrarHistorial() {
    preguntandoBorrar = false;
    try {
      await borrarPortapapeles();
      historial = [];
    } catch (e) {
      onerror(String(e));
    }
  }

  onMount(() => {
    recargar();
    leer();
    leerHistorial();
    leerVirtual();
    leerTuMac();
    const quitar = [
      alCambiarRemoto(() => {
        leer();
        leerVirtual();
      }),
      alCambiarPantallaVirtual((e) => {
        if (!ocupadoVirtual) virtual = e;
      }),
      alCapturar((c) => {
        ultimaCaptura = c;
        capturaCancelada = false;
      }),
      alCambiarPortapapeles(leerHistorial),
    ];
    // Cada 5 s mientras se ve (la Mac desde Windows, cada 10 s); al volver a la ventana, enseguida.
    const intervalo = setInterval(() => {
      if (document.visibilityState === 'visible') leer();
    }, 5000);
    const intervaloMac = esMac
      ? undefined
      : setInterval(() => {
          if (document.visibilityState === 'visible') leerTuMac();
        }, 10000);
    const alVolver = () => {
      if (document.visibilityState !== 'visible') return;
      leer();
      leerHistorial();
      leerVirtual();
      leerTuMac();
    };
    document.addEventListener('visibilitychange', alVolver);
    window.addEventListener('focus', alVolver);
    return () => {
      for (const q of quitar) q();
      clearInterval(intervalo);
      clearInterval(intervaloMac);
      clearTimeout(temporizadorAviso);
      clearTimeout(temporizadorCopia);
      document.removeEventListener('visibilitychange', alVolver);
      window.removeEventListener('focus', alVolver);
    };
  });
</script>

{#snippet teclas(lista: string[], lado: 'windows' | 'mac')}
  <span class="teclas {lado}" aria-label="{lado === 'mac' ? 'En la Mac' : 'En Windows'}: {enPalabras(lista)}">
    {#each lista as t, i (i)}<kbd class:simbolo={esSimbolo(t)} aria-hidden="true">{t}</kbd>{/each}
  </span>
{/snippet}

{#snippet ficha(icono: Nombre, activa: boolean)}
  <span class="ficha" class:activa aria-hidden="true"><Icono nombre={icono} tamano={18} /></span>
{/snippet}

<!-- Cómo ver la segunda pantalla de la Mac en RustDesk. Cada paso va en un <span>: así el texto con negritas fluye
     junto, en la segunda columna (la primera es el número). `ancha`: con espacio, los pasos y el consejo lado a lado. -->
{#snippet guiaVirtual(ancha: boolean)}
  <div class="guia-pasos" class:ancha>
    <ol class="pasos">
      <li>
        <span>
          En la ventana de la sesión de RustDesk, abre el botón de monitores de la barra de arriba
          (<b>Seleccionar monitor</b>).
        </span>
      </li>
      <li>
        <span>
          Marca <b>Mostrar pantallas como ventanas individuales</b> y toca el <b>2</b>: la segunda pantalla de tu Mac se
          abre en otra ventana.
        </span>
      </li>
      <li><span>Arrastra esa ventana a tu otro monitor y pulsa <b>Pantalla completa</b> en su barra.</span></li>
    </ol>
    <p class="consejo">
      <Icono nombre="info" tamano={15} />
      <span>
        Para que se haga solo: en RustDesk, ve a <b>Ajustes › Pantalla</b> y, en «Otras opciones predeterminadas», marca
        <b>Usar todas mis pantallas para la sesión remota</b>. Al conectarte, cada pantalla de la Mac se abre a pantalla
        completa en un monitor. Windows debe estar en <b>Extender</b> (Win + P).
      </span>
    </p>
  </div>
{/snippet}

<div class="vista">
  <header class="cabecera">
    <h1>Remoto</h1>
    <p>
      {esMac
        ? 'Usa tu Mac desde el PC del trabajo con RustDesk y Tailscale.'
        : 'Conéctate a tu Mac con RustDesk por Tailscale y manéjala desde aquí.'}
    </p>
  </header>

  {#if perfil.leido && !perfil.activo}
    {#if perfil.estado?.modo === 'nunca'}
      <!-- Apagado a propósito en Ajustes: una nota calma, sin la guía. -->
      <section class="nota-nunca vidrio" aria-labelledby="remoto-apagado">
        <span class="ficha-icono tenue"><Icono nombre="remoto" tamano={20} /></span>
        <div class="textos">
          <h2 class="titulo-panel" id="remoto-apagado">El trabajo remoto está apagado</h2>
          <p class="ayuda">Lo apagaste en Ajustes. Enciéndelo ahí cuando quieras usarlo de nuevo.</p>
        </div>
        {#if onir}
          {@const ir = onir}
          <button class="boton-vidrio boton" type="button" onclick={() => ir('ajustes')}>Ir a Ajustes</button>
        {/if}
      </section>
    {:else}
      <!-- Sin tu otro equipo: la guía es lo principal y, al lado, lo que se activa al encontrarlo. -->
      <div class="sin-perfil">
        <GuiaConexion para="remoto" {onerror} />
        <aside class="desbloquea vidrio" aria-labelledby="remoto-desbloquea">
          <div class="desbloquea__escena" aria-hidden="true">
            <IlustracionEscena nombre="remoto-sin" icono="remoto" />
          </div>
          <div class="desbloquea__textos">
            <h2 class="titulo-panel" id="remoto-desbloquea">
              Cuando Nexo encuentre {esMac ? 'tu PC' : 'tu Mac'}, aquí podrás:
            </h2>
            <ul class="desbloquea__lista">
              {#each desbloquea as d (d.texto)}
                <li>
                  <span class="ficha-icono chica tenue"><Icono nombre={d.icono} tamano={16} /></span>
                  <span>{d.texto}</span>
                </li>
              {/each}
            </ul>
          </div>
        </aside>
      </div>
    {/if}
  {:else if remoto && resumen}
    {@const r = remoto}
    {#if guiaArriba}
      <GuiaConexion para="remoto" compacta {onerror} oncerrar={cerrarGuia} />
    {/if}
    <!-- 1. Estado de la sesión: la ilustración a la izquierda; el estado, las acciones y los servicios a la derecha -->
    <section class="estado vidrio" aria-labelledby="remoto-estado">
      <div class="estado__escena {ruta}" aria-hidden="true">
        <IlustracionEscena nombre={escenaRemoto} icono="remoto" />
      </div>

      <div class="estado__cuerpo">
        <div class="estado__textos" role="status" aria-live="polite">
          <h2 class="estado__titulo" id="remoto-estado">
            <span class="punto {resumen.tono}" aria-hidden="true"></span>{resumen.titulo}
          </h2>
          <p class="estado__detalle">{resumen.detalle}</p>
        </div>

        <div class="estado__acciones">
          {#if esMac}
            <button
              class={r.activa ? 'boton-vidrio boton' : 'boton-primario'}
              type="button"
              disabled={preparando}
              aria-busy={preparando}
              onclick={preparar}
            >
              {preparando ? 'Preparando…' : 'Preparar para el trabajo'}
            </button>
            <button class="boton-vidrio boton" type="button" onclick={abrirRD}>Abrir RustDesk</button>
          {:else}
            <button
              class="boton-primario"
              type="button"
              disabled={conectando || !r.rustdesk || !r.ipMac}
              aria-busy={conectando}
              onclick={conectar}
            >
              <Icono nombre="conectar" tamano={17} />
              Conectar a mi Mac
            </button>
            <button class="boton-vidrio boton" type="button" disabled={!r.rustdesk} onclick={abrirRD}>Abrir RustDesk</button>
          {/if}
        </div>

        <ul class="servicios" aria-label="Servicios">
          {#each servicios as s (s.nombre)}
            <li class="servicio {s.tono}">
              <span class="punto {s.tono}" aria-hidden="true"></span>
              <span class="servicio__nombre">{s.nombre}</span>
              <span class="servicio__valor">{s.valor}</span>
            </li>
          {/each}
          {#if r.ipMac}
            <li class="servicio">
              <span class="servicio__nombre">{esMac ? 'Esta Mac' : nombreMac === 'tu Mac' ? 'Tu Mac' : nombreMac}</span>
              <span class="servicio__valor direccion">{r.ipMac}</span>
            </li>
          {/if}
        </ul>

        {#if avisoAccion}
          <p class="aviso-accion {avisoAccion.tono}" role="status">
            <Icono nombre={avisoAccion.tono === 'ok' ? 'check-circulo' : 'info'} tamano={16} />
            <span>{avisoAccion.texto}</span>
          </p>
        {/if}
      </div>
    </section>

    {#if !esMac}
      <!-- 2. Windows: tu Mac, manejada por XDrop. A la izquierda su pantalla virtual; a la derecha la sesión. -->
      <section class="panel vidrio" aria-labelledby="remoto-mac">
        <div class="cabeza">
          {@render ficha('portatil', !!mac && !errorMac)}
          <div class="textos">
            <h2 class="titulo-panel" id="remoto-mac">Tu Mac</h2>
            <p class="ayuda con-punto" role="status">
              <span class="punto {estadoTuMac.tono}" aria-hidden="true"></span>
              <span>{estadoTuMac.texto}</span>
            </p>
          </div>
        </div>

        {#if destinoMac === null || errorMac}
          <div class="permiso">
            <Icono nombre="alerta" tamano={16} />
            <p>
              {destinoMac === null
                ? 'No encontré tu Mac entre tus equipos de Tailscale. Revisa que esté encendida, con Tailscale conectado y Nexo abierto.'
                : errorMac}
            </p>
            <button
              class="boton-vidrio boton"
              type="button"
              disabled={buscandoMac}
              aria-busy={buscandoMac}
              onclick={leerTuMac}
            >
              {buscandoMac ? 'Buscando…' : 'Reintentar'}
            </button>
          </div>
        {:else if !mac}
          <div class="silueta-filas" aria-busy="true"><span class="solo-lector">Revisando tu Mac…</span></div>
        {:else}
          {@const v = mac.pantallaVirtual}
          {@const s = sesionMac}
          <div class="dos-columnas">
            <div class="columna-filas">
              <h3 class="rotulo">Pantalla virtual</h3>
              <div class="fila">
                <div class="textos">
                  <div class="titulo">Pantalla virtual para RustDesk</div>
                  <div class="ayuda">
                    {ocupadoMac === 'activa'
                      ? v.activa
                        ? 'Quitándola…'
                        : 'Creándola en tu Mac…'
                      : v.activa
                        ? `Activa a ${v.ancho} × ${v.alto}: ábrela como la segunda pantalla en RustDesk.`
                        : 'Una segunda pantalla de tu Mac, para tu otro monitor.'}
                  </div>
                </div>
                {#if v.disponible}
                  <Interruptor
                    encendido={v.activa}
                    cargando={ocupadoMac === 'activa'}
                    etiqueta="Pantalla virtual para RustDesk"
                    oncambio={alternarVirtualMac}
                  />
                {/if}
              </div>
              {#if !v.disponible}
                <div class="permiso info">
                  <Icono nombre="info" tamano={16} />
                  <p>El macOS de tu Mac no deja crear pantallas virtuales, así que esta opción no está disponible.</p>
                </div>
              {:else}
                <div class="fila con-menu">
                  <div class="textos">
                    <div class="titulo">Resolución</div>
                    <div class="ayuda">
                      {ocupadoMac === 'resolucion' ? 'Cambiando…' : 'A 60 Hz y sin Retina: pesa poco en la conexión.'}
                    </div>
                  </div>
                  <div class="menu">
                    <MenuDesplegable
                      opciones={opcionesResolucionVirtual}
                      valor={claveResolucion(v.ancho, v.alto)}
                      etiqueta="Resolución de la pantalla virtual de tu Mac"
                      oncambio={resolucionMac}
                    />
                  </div>
                </div>
                <div class="fila">
                  <div class="textos">
                    <div class="titulo">Crear al empezar una sesión remota</div>
                    <div class="ayuda">Aparece sola cuando te conectas desde aquí.</div>
                  </div>
                  <Interruptor
                    encendido={v.crearAlEmpezar}
                    cargando={ocupadoMac === 'crearAlEmpezar'}
                    etiqueta="Crear al empezar una sesión remota"
                    oncambio={() => opcionMac('crearAlEmpezar')}
                  />
                </div>
                <div class="fila">
                  <div class="textos">
                    <div class="titulo">Quitar al terminar</div>
                    <div class="ayuda">Al desconectarte, tu Mac vuelve a tener una sola pantalla.</div>
                  </div>
                  <Interruptor
                    encendido={v.quitarAlTerminar}
                    cargando={ocupadoMac === 'quitarAlTerminar'}
                    etiqueta="Quitar al terminar la sesión remota"
                    oncambio={() => opcionMac('quitarAlTerminar')}
                  />
                </div>
              {/if}
            </div>

            <div class="columna-filas">
              <h3 class="rotulo">Sesión</h3>
              {#if s}
                <div class="fila">
                  <div class="textos">
                    <div class="titulo con-punto">
                      <span class="punto {s.tono}" aria-hidden="true"></span>
                      <span>{s.titulo}</span>
                    </div>
                    <div class="ayuda">{s.detalle}</div>
                  </div>
                </div>
              {/if}
              <div class="fila">
                <div class="textos">
                  <div class="titulo">Modo trabajo</div>
                  <div class="ayuda">
                    {mac.modoTrabajo
                      ? mac.evitarReposo
                        ? 'La Mac no se dormirá aunque todavía no te conectes.'
                        : 'Activo, pero «Evitar que la Mac se duerma» está apagado en la Mac.'
                      : 'Enciéndelo para que la Mac no se duerma entre una sesión y otra.'}
                  </div>
                </div>
                <Interruptor
                  encendido={mac.modoTrabajo}
                  cargando={ocupadoMac === 'modoTrabajo'}
                  etiqueta="Modo trabajo"
                  oncambio={modoTrabajoMac}
                />
              </div>
              {#if onir}
                {@const ir = onir}
                <div class="fila">
                  <button class="enlace" type="button" onclick={() => ir('analitica')}>
                    Ver el estado de la Mac
                    <Icono nombre="chevron" tamano={15} />
                  </button>
                </div>
              {/if}
            </div>
          </div>
        {/if}
      </section>

      <!-- Windows: cómo ver en RustDesk la pantalla virtual que se enciende en "Tu Mac" (pasos y consejo lado a lado) -->
      <section class="panel vidrio" aria-labelledby="remoto-virtual">
        <div class="cabeza">
          {@render ficha('pantallas', !!mac?.pantallaVirtual.activa)}
          <div class="textos">
            <h2 class="titulo-panel" id="remoto-virtual">Ver la pantalla virtual en RustDesk</h2>
            <p class="ayuda">
              Con la pantalla virtual de tu Mac encendida, cada pantalla se abre en su propia ventana, una en cada
              monitor.
            </p>
          </div>
        </div>
        <div class="guia-virtual">
          {@render guiaVirtual(true)}
        </div>
      </section>
    {:else}
      <!-- 2. Mac: la pantalla virtual para RustDesk. A la izquierda sus ajustes; a la derecha, cómo verla. -->
      <section class="panel vidrio" aria-labelledby="remoto-virtual">
        <div class="cabeza">
          {@render ficha('pantallas', !!virtual?.activa)}
          <div class="textos">
            <h2 class="titulo-panel" id="remoto-virtual">Pantalla virtual para RustDesk</h2>
            <p class="ayuda">
              Agrega una segunda pantalla a tu Mac para verla en otra ventana de RustDesk, en tu otro monitor de la
              oficina.
            </p>
          </div>
          {#if virtual?.disponible}
            <Interruptor
              encendido={virtual.activa}
              cargando={ocupadoVirtual === 'activa'}
              etiqueta="Pantalla virtual para RustDesk"
              oncambio={alternarVirtual}
            />
          {/if}
        </div>

        <div class="dos-columnas">
          <div class="columna-filas">
            {#if virtual && !virtual.disponible}
              <div class="permiso info">
                <Icono nombre="info" tamano={16} />
                <p>Este macOS no deja crear pantallas virtuales, así que esta opción no está disponible.</p>
              </div>
            {:else if virtual}
              {@const v = virtual}
              <div class="fila con-menu">
                <div class="textos">
                  <div class="titulo">Resolución</div>
                  <div class="ayuda">
                    {ocupadoVirtual === 'resolucion' ? 'Cambiando…' : 'A 60 Hz y sin Retina: pesa poco en la conexión.'}
                  </div>
                </div>
                <div class="menu">
                  <MenuDesplegable
                    opciones={opcionesResolucionVirtual}
                    valor={claveResolucion(v.ancho, v.alto)}
                    etiqueta="Resolución de la pantalla virtual"
                    oncambio={resolucionVirtual}
                  />
                </div>
              </div>
              <div class="fila">
                <div class="textos">
                  <div class="titulo">Crear al empezar una sesión remota</div>
                  <div class="ayuda">Aparece sola cuando te conectas desde el trabajo.</div>
                </div>
                <Interruptor
                  encendido={v.crearAlEmpezar}
                  cargando={ocupadoVirtual === 'crearAlEmpezar'}
                  etiqueta="Crear al empezar una sesión remota"
                  oncambio={() => opcionVirtual('crearAlEmpezar')}
                />
              </div>
              <div class="fila">
                <div class="textos">
                  <div class="titulo">Quitar al terminar la sesión remota</div>
                  <div class="ayuda">Al desconectarte, tu Mac vuelve a tener una sola pantalla.</div>
                </div>
                <Interruptor
                  encendido={v.quitarAlTerminar}
                  cargando={ocupadoVirtual === 'quitarAlTerminar'}
                  etiqueta="Quitar al terminar la sesión remota"
                  oncambio={() => opcionVirtual('quitarAlTerminar')}
                />
              </div>
              {#if v.activa}
                <p class="resultado" role="status">
                  <Icono nombre="check-circulo" tamano={16} />
                  <!-- En una sola expresión: un salto de línea dentro del texto con datos se ve como un espacio angosto. -->
                  <span>
                    {`Activa a ${v.ancho} × ${v.alto}. En tu Mac se llama «Nexo · Pantalla virtual»; puedes ordenarla en Ajustes del Sistema › Pantallas.`}
                  </span>
                </p>
              {/if}
            {:else}
              <div class="silueta-filas" aria-busy="true"><span class="solo-lector">Revisando la pantalla virtual…</span></div>
            {/if}
          </div>

          <div class="guia-virtual">
            <h3 class="rotulo">Cómo verla en la oficina</h3>
            {@render guiaVirtual(false)}
          </div>
        </div>
      </section>
    {/if}

    <!-- 3. Atajos: a lo ancho; con espacio, las filas van en dos mitades -->
    <section class="panel vidrio" aria-labelledby="remoto-atajos">
      <div class="cabeza">
        {@render ficha('teclado', esMac && r.ajustes.traducir && r.permisoTeclas)}
        <div class="textos">
          <h2 class="titulo-panel" id="remoto-atajos">Atajos de Windows en tu Mac</h2>
          <p class="ayuda">
            {esMac
              ? 'Usa los atajos de siempre desde el PC: Nexo los traduce solo en lo que llega por RustDesk. El teclado de tu Mac no cambia.'
              : 'La traducción la hace Nexo en tu Mac: desde aquí, conéctate y usa los atajos de siempre.'}
          </p>
        </div>
        {#if esMac}
          <Interruptor
            encendido={r.ajustes.traducir}
            cargando={ocupado === 'traducir'}
            etiqueta="Traducir atajos"
            oncambio={() => ajustar('traducir', !r.ajustes.traducir)}
          />
        {/if}
      </div>

      {#if esMac && !r.permisoTeclas}
        <div class="permiso">
          <Icono nombre="alerta" tamano={16} />
          <p>Falta el permiso de <b>Accesibilidad</b>: sin él, Nexo no puede traducir ni escuchar Imprimir Pantalla.</p>
          <button class="boton-vidrio boton" type="button" onclick={abrirAccesibilidad}>Abrir Accesibilidad</button>
        </div>
      {/if}

      <div class="grupos" class:apagado={esMac && !r.ajustes.traducir}>
        {#each grupos as g (g.nombre)}
          <div class="grupo">
            <h3 class="rotulo">{g.nombre}</h3>
            <ul class="lista-atajos">
              {#each g.atajos as a (a.windows.join('+') + a.accion)}
                <li class="atajo">
                  {@render teclas(a.windows, 'windows')}
                  <span class="flecha" aria-hidden="true"><Icono nombre="flecha" tamano={13} /></span>
                  {@render teclas(a.mac, 'mac')}
                  <span class="accion">{a.accion}</span>
                </li>
              {/each}
            </ul>
          </div>
        {/each}
      </div>
      {#if totalAtajos > 9}
        <button class="enlace mas-atajos" type="button" aria-expanded={todos} onclick={() => (todos = !todos)}>
          {todos ? 'Ver menos' : `Ver los ${totalAtajos} atajos`}
          <span class="giro" class:abierto={todos}><Icono nombre="chevron-abajo" tamano={15} /></span>
        </button>
      {/if}
    </section>

    <!-- 4. Rejilla: en la Mac, captura y la sesión a la izquierda y el portapapeles a la derecha;
         en Windows, solo la captura (a lo ancho). -->
    <div class="rejilla">
      <div class="columna">
        <!-- Captura -->
        <section class="panel vidrio" aria-labelledby="remoto-captura">
          <div class="cabeza">
            {@render ficha('captura', !esMac || (r.ajustes.captura && r.permisoPantalla && r.permisoTeclas))}
            <div class="textos">
              <h2 class="titulo-panel" id="remoto-captura">Captura con Imprimir Pantalla</h2>
              <p class="ayuda">
                {esMac
                  ? 'Selecciona una zona; se copia liviana para pegar en Windows y se guarda en Imágenes › Capturas remotas.'
                  : 'Con tu Mac en RustDesk, pulsa Imprimir Pantalla: eliges una zona y llega liviana a tu portapapeles.'}
              </p>
            </div>
            {#if esMac}
              <Interruptor
                encendido={r.ajustes.captura}
                cargando={ocupado === 'captura'}
                etiqueta="Captura con Imprimir Pantalla"
                oncambio={() => ajustar('captura', !r.ajustes.captura)}
              />
            {/if}
          </div>

          <div class="guia" aria-label="Imprimir Pantalla en el PC llega a la Mac como F13">
            <span class="guia__paso">
              <kbd aria-hidden="true">Impr Pant</kbd>
              <span>en tu PC</span>
            </span>
            <span class="flecha" aria-hidden="true"><Icono nombre="flecha" tamano={14} /></span>
            <span class="guia__paso">
              <kbd class="mac" aria-hidden="true">F13</kbd>
              <span>en tu Mac</span>
            </span>
            <span class="flecha" aria-hidden="true"><Icono nombre="flecha" tamano={14} /></span>
            <span class="guia__paso">
              <span class="guia__icono"><Icono nombre="portapapeles" tamano={16} /></span>
              <span>lista para pegar</span>
            </span>
          </div>

          {#if esMac}
            {#if !r.permisoPantalla}
              <div class="permiso">
                <Icono nombre="alerta" tamano={16} />
                <p>
                  Falta el permiso de <b>Grabación de pantalla</b>. Actívalo para Nexo; macOS te pedirá volver a abrirlo.
                </p>
                <button class="boton-vidrio boton" type="button" onclick={pedirPantalla}>Abrir Ajustes</button>
              </div>
            {/if}
            {#if r.capturaRemotaAbierta}
              <div class="permiso info">
                <Icono nombre="info" tamano={16} />
                <p>
                  La app <b>Captura remota</b> sigue abierta. Ciérrala desde su ícono de la barra de menús: Nexo ya hace lo
                  mismo y así Imprimir Pantalla no captura dos veces.
                </p>
              </div>
            {/if}

            <div class="botones">
              <button
                class="boton-vidrio boton"
                type="button"
                disabled={capturando || !r.permisoPantalla}
                aria-busy={capturando}
                onclick={probarCaptura}
              >
                <Icono nombre="captura" tamano={16} />
                {capturando ? 'Selecciona una zona…' : 'Probar captura'}
              </button>
              <button class="boton-vidrio boton" type="button" onclick={abrirCarpeta}>
                <Icono nombre="carpeta" tamano={16} />
                Abrir carpeta
              </button>
            </div>
            {#if ultimaCaptura}
              <p class="resultado" role="status">
                <Icono nombre="check-circulo" tamano={16} />
                <span>
                  {`${ultimaCaptura.copiada ? 'Copiada' : 'Guardada'} · ${ultimaCaptura.ancho} × ${ultimaCaptura.alto} px · ${miles(ultimaCaptura.kb)} KB`}
                </span>
              </p>
            {:else if capturaCancelada}
              <p class="resultado neutro" role="status">
                <span>Cancelaste la captura. Pulsa Esc cuando quieras salir sin capturar.</span>
              </p>
            {/if}
          {/if}
        </section>

        {#if esMac}
          <!-- Comodidad durante la sesión -->
          <section class="panel vidrio" aria-labelledby="remoto-sesion">
            <h2 class="rotulo" id="remoto-sesion">Durante la sesión</h2>
            <div class="fila">
              {@render ficha('luna', r.ajustes.evitarReposo)}
              <div class="textos">
                <div class="titulo">Evitar que la Mac se duerma</div>
                <div class="ayuda">Mientras estés conectado, con el cargador puesto. Al terminar, todo vuelve a lo normal.</div>
              </div>
              <Interruptor
                encendido={r.ajustes.evitarReposo}
                cargando={ocupado === 'evitarReposo'}
                etiqueta="Evitar que la Mac se duerma"
                oncambio={() => ajustar('evitarReposo', !r.ajustes.evitarReposo)}
              />
            </div>
            <div class="fila">
              {@render ficha('sol-bajo', r.ajustes.brilloMinimo)}
              <div class="textos">
                <div class="titulo">Bajar el brillo al mínimo</div>
                <div class="ayuda">La pantalla de la MacBook baja a 0 y vuelve a como estaba al terminar.</div>
              </div>
              <Interruptor
                encendido={r.ajustes.brilloMinimo}
                cargando={ocupado === 'brilloMinimo'}
                etiqueta="Bajar el brillo al mínimo"
                oncambio={() => ajustar('brilloMinimo', !r.ajustes.brilloMinimo)}
              />
            </div>
            {#if r.modoTrabajo}
              <div class="fila">
                {@render ficha('encendido', true)}
                <div class="textos">
                  <div class="titulo">Modo trabajo</div>
                  <div class="ayuda">
                    {r.ajustes.evitarReposo
                      ? 'La Mac no se dormirá aunque todavía no te hayas conectado.'
                      : 'Activo, pero «Evitar que la Mac se duerma» está apagado.'}
                  </div>
                </div>
                <button
                  class="boton-vidrio boton"
                  type="button"
                  disabled={ocupado === 'modoTrabajo'}
                  onclick={() => ajustar('modoTrabajo', false)}
                >
                  Terminar
                </button>
              </div>
            {/if}
          </section>
        {/if}
      </div>

      {#if esMac}
        <div class="columna">
          <!-- Portapapeles -->
          <section class="panel vidrio" aria-labelledby="remoto-portapapeles">
            <div class="cabeza">
              {@render ficha('portapapeles', r.ajustes.portapapelesLiviano)}
              <div class="textos">
                <h2 class="titulo-panel" id="remoto-portapapeles">Portapapeles</h2>
                <p class="ayuda">Tus últimas 12 copias, para volver a copiarlas. Las contraseñas no se guardan.</p>
              </div>
            </div>
            <div class="fila">
              <div class="textos">
                <div class="titulo">Reducir imágenes al copiar (evita que RustDesk se congele)</div>
                <div class="ayuda">Durante la sesión, las imágenes grandes pasan a 1600 px antes de llegar a Windows.</div>
              </div>
              <Interruptor
                encendido={r.ajustes.portapapelesLiviano}
                cargando={ocupado === 'portapapelesLiviano'}
                etiqueta="Reducir imágenes al copiar"
                oncambio={() => ajustar('portapapelesLiviano', !r.ajustes.portapapelesLiviano)}
              />
            </div>

            <div class="historial-cabeza">
              <h3 class="rotulo">Historial</h3>
              {#if historial?.length}
                <button class="boton-texto peligro" type="button" onclick={() => (preguntandoBorrar = true)}>
                  Borrar historial
                </button>
              {/if}
            </div>

            {#if historial === null}
              <ul class="copias" aria-busy="true">
                {#each [0, 1, 2] as i (i)}<li class="copia silueta"><span class="miniatura"></span><span class="linea"></span></li>{/each}
              </ul>
            {:else if historial.length === 0}
              <p class="vacio">Todavía no hay copias. Lo que copies en la Mac, o te llegue desde el PC, aparece aquí.</p>
            {:else}
              <ul class="copias">
                {#each historial as e (e.id)}
                  {@const v = vista(e)}
                  <li class="copia">
                    {#if e.tipo === 'imagen' && e.miniatura}
                      <img
                        class="miniatura"
                        src={e.miniatura}
                        alt="Imagen copiada de {e.ancho} × {e.alto} píxeles"
                        width="64"
                        height="44"
                        draggable="false"
                      />
                    {:else}
                      <span class="miniatura texto" aria-hidden="true"><Icono nombre="portapapeles" tamano={18} /></span>
                    {/if}
                    <div class="copia__cuerpo">
                      {#if e.tipo === 'imagen'}
                        <p class="copia__texto">Imagen · {e.ancho} × {e.alto} px</p>
                      {:else}
                        <p class="copia__texto" use:tooltip={tooltipDe(e, v.faltan)}>
                          {v.texto}{#if v.faltan > 0}<span class="faltan">{` y ${miles(v.faltan)} caracteres más`}</span>{/if}
                        </p>
                      {/if}
                      <p class="copia__meta">
                        <Icono nombre={e.origen === 'remoto' ? 'pantallas' : 'portatil'} tamano={13} />
                        <span>{e.origen === 'remoto' ? 'Desde el PC' : 'En la Mac'} · {cuando(e.momento)}</span>
                      </p>
                    </div>
                    <button
                      class="boton-vidrio boton copiar"
                      class:hecho={copiadoId === e.id}
                      type="button"
                      disabled={copiandoId !== null}
                      aria-label={e.tipo === 'imagen' ? 'Copiar esta imagen' : 'Copiar este texto'}
                      onclick={() => copiar(e)}
                    >
                      <Icono nombre={copiadoId === e.id ? 'check' : 'copiar'} tamano={15} />
                      {copiadoId === e.id ? 'Copiado' : 'Copiar'}
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          </section>
        </div>
      {/if}
    </div>
  {:else}
    <!-- Mientras responde Rust: siluetas quietas del estado y de los paneles. -->
    <div class="silueta-estado vidrio" aria-busy="true"><span class="solo-lector">Revisando el remoto…</span></div>
    <div class="silueta-panel vidrio"></div>
  {/if}
</div>

<Dialogo
  abierto={preguntandoBorrar}
  titulo="¿Borrar el historial del portapapeles?"
  descripcion="Se borran las copias que guarda Nexo. Lo que tienes copiado ahora no cambia."
  icono="papelera"
  tono="peligro"
  textoConfirmar="Borrar"
  textoCancelar="Cancelar"
  onconfirmar={borrarHistorial}
  oncancelar={() => (preguntandoBorrar = false)}
/>

<style>
  /* La vista es el contenedor: cada tarjeta se acomoda según el ancho real (no el de la ventana). */
  .vista {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
    container-type: inline-size;
  }
  .cabecera {
    margin-bottom: 4px;
  }
  .cabecera h1 {
    margin: 0;
    font-size: 26px;
    line-height: 32px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .cabecera p {
    margin: 4px 0 0;
    color: var(--texto-2);
  }

  /* ---------- Sin el perfil ---------- */
  /* La guía y, al lado, lo que se activa. Angosto: una debajo de la otra. */
  .sin-perfil {
    display: grid;
    gap: 16px;
    align-items: start;
    min-width: 0;
  }
  @container (min-width: 820px) {
    .sin-perfil {
      grid-template-columns: minmax(0, 1.55fr) minmax(260px, 1fr);
    }
  }
  .desbloquea {
    display: grid;
    gap: 12px;
    min-width: 0;
    padding: 12px 20px 18px 12px;
    border-radius: var(--radio-lg);
  }
  .desbloquea__escena {
    display: grid;
    width: 100%;
    aspect-ratio: 520 / 338;
    max-height: 210px;
    padding: 12px 14px;
    border-radius: 16px;
    background: var(--superficie-inserto);
  }
  .desbloquea__textos {
    display: grid;
    gap: 14px;
    min-width: 0;
    padding-left: 8px;
  }
  .desbloquea__lista {
    display: grid;
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .desbloquea__lista li {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto-2);
  }
  /* Angosto y con lugar a lo ancho: el dibujo a la izquierda y la lista a la derecha. */
  @container (min-width: 560px) and (max-width: 819px) {
    .desbloquea {
      grid-template-columns: minmax(200px, 0.9fr) minmax(0, 1fr);
      align-items: center;
      column-gap: 20px;
      padding-bottom: 12px;
    }
  }

  /* "Nunca" en Ajustes: una nota calma. */
  .nota-nunca {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px 14px;
    padding: 16px 20px;
    border-radius: var(--radio-lg);
  }
  .nota-nunca .textos {
    flex: 1 1 240px;
  }

  /* ---------- 1. Estado ---------- */
  /* La ilustración a la izquierda, en un escenario apenas teñido; el estado y las acciones a la derecha. */
  .estado {
    display: grid;
    grid-template-columns: minmax(220px, 300px) minmax(0, 1fr);
    align-items: center;
    gap: 16px 24px;
    padding: 12px 22px 12px 12px;
    border-radius: var(--radio-lg);
  }
  /* Mide lo que pide el dibujo (520 × 338) con aire alrededor: nunca se recorta ni queda nada encima. */
  .estado__escena {
    display: grid;
    width: 100%;
    min-width: 0;
    aspect-ratio: 520 / 338;
    max-height: 230px;
    padding: 12px 14px;
    border-radius: 16px;
    background: color-mix(in srgb, var(--primario) 6%, var(--superficie-inserto));
  }
  .estado__escena.viva {
    background: color-mix(in srgb, var(--exito) 8%, var(--superficie-inserto));
  }
  .estado__escena.rele {
    background: color-mix(in srgb, var(--aviso) 7%, var(--superficie-inserto));
  }
  .estado__escena.apagada {
    background: var(--superficie-inserto);
  }

  .estado__cuerpo {
    display: grid;
    gap: 14px;
    min-width: 0;
    padding: 8px 0;
  }
  .estado__titulo {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0;
    font-size: 20px;
    line-height: 26px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .estado__detalle {
    margin: 4px 0 0 18px;
    color: var(--texto-2);
  }
  .punto {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--texto-3);
  }
  .punto.ok {
    background: var(--exito);
  }
  .punto.aviso {
    background: var(--aviso);
  }

  .estado__acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  /* Los servicios como fichas chicas y parejas. */
  .servicios {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .servicio {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 3px 11px 3px 10px;
    border-radius: var(--radio-pildora);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    font-size: 12.5px;
    line-height: 18px;
  }
  .servicio .punto {
    width: 6px;
    height: 6px;
  }
  .servicio__nombre {
    color: var(--texto-3);
  }
  .servicio__valor {
    font-weight: 600;
    color: var(--texto-2);
  }
  .servicio.ok .servicio__valor {
    color: var(--exito);
  }
  .servicio.aviso .servicio__valor {
    color: var(--aviso);
  }
  .direccion {
    color: var(--texto);
    font-variant-numeric: tabular-nums;
    user-select: text;
    -webkit-user-select: text;
    cursor: text;
  }

  .aviso-accion {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radio-sm);
    font-size: 12.5px;
    line-height: 18px;
    background: var(--superficie-elevada);
    color: var(--texto-2);
  }
  .aviso-accion :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
  }
  .aviso-accion.ok :global(svg) {
    color: var(--exito);
  }
  .aviso-accion.aviso :global(svg) {
    color: var(--aviso);
  }

  /* Angosto: la ilustración arriba, a lo ancho (con su alto tope), y el estado debajo. */
  @container (max-width: 640px) {
    .estado {
      grid-template-columns: minmax(0, 1fr);
      padding: 12px 12px 16px;
    }
    .estado__escena {
      max-height: 200px;
    }
    .estado__cuerpo {
      padding: 0 8px;
    }
  }

  /* ---------- Rejillas ---------- */
  /* Una columna bajo 760 px de contenedor; dos parejas por encima. */
  .dos-columnas {
    display: grid;
    gap: 8px 32px;
    min-width: 0;
  }
  .rejilla,
  .columna {
    display: grid;
    gap: 16px;
    align-content: start;
    min-width: 0;
  }
  /* En una columna, el segundo grupo de filas se separa del primero con una línea. */
  .dos-columnas > .columna-filas + .columna-filas {
    margin-top: 4px;
    padding-top: 12px;
    border-top: 1px solid var(--borde);
  }
  @container (min-width: 760px) {
    .dos-columnas,
    .rejilla:has(> .columna + .columna) {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
      align-items: start;
    }
    .dos-columnas > .columna-filas + .columna-filas {
      margin-top: 0;
      padding-top: 0;
      border-top: 0;
    }
  }

  /* ---------- Paneles ---------- */
  .panel {
    padding: 16px 20px 8px;
    border-radius: var(--radio-lg);
    min-width: 0;
  }
  .rotulo {
    margin: 0 0 4px;
  }
  .cabeza {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding-bottom: 12px;
  }
  .cabeza :global(.interruptor) {
    margin-top: 2px;
  }
  .titulo-panel {
    margin: 0;
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
  }
  .textos {
    flex: 1;
    min-width: 0;
  }
  .titulo {
    font-weight: 600;
  }
  .ayuda {
    margin: 2px 0 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  /* Un punto de color delante del texto (el estado de la Mac y de la sesión). */
  .con-punto {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .con-punto > span:last-child {
    min-width: 0;
  }

  .ficha {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    background: var(--superficie-elevada);
    color: var(--texto-3);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .ficha.activa {
    color: var(--primario-texto);
    background: var(--elegido-bg);
  }

  .fila {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 60px;
    padding: 12px 0;
    border-top: 1px solid var(--borde);
  }
  .rotulo + .fila,
  .columna-filas > .fila:first-child {
    border-top: 0;
  }
  .columna-filas {
    min-width: 0;
  }
  .columna-filas > .rotulo {
    margin-bottom: 0;
  }
  /* La resolución: si no cabe al lado del texto, el menú baja a su propia línea. */
  .fila.con-menu {
    flex-wrap: wrap;
    row-gap: 10px;
  }
  .con-menu .textos {
    flex: 1 1 160px;
  }
  .menu {
    flex: 0 1 200px;
    min-width: 150px;
  }

  .boton {
    flex-shrink: 0;
    min-height: 32px;
    padding: 0 14px;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .boton:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .boton:disabled,
  .boton-primario:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .boton-texto {
    padding: 4px 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--texto-2);
    font-size: 12.5px;
    font-weight: 600;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .boton-texto:hover {
    background: var(--superficie-hover);
  }
  .boton-texto.peligro {
    color: var(--error);
  }
  /* Un enlace dentro de una tarjeta ("Ver los 27 atajos", "Ver el estado de la Mac"). */
  .enlace {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-left: -8px;
    padding: 6px 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--primario-texto);
    font-size: 13px;
    font-weight: 600;
    text-align: left;
    transition: background-color var(--dur-color) ease;
  }
  .enlace:hover {
    background: var(--superficie-hover);
  }
  .enlace :global(svg) {
    flex-shrink: 0;
  }

  .permiso {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin: 0 0 12px;
    padding: 10px 12px;
    border-radius: var(--radio-sm);
    background: color-mix(in srgb, var(--aviso) 9%, var(--superficie));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--aviso) 25%, transparent);
  }
  .permiso > :global(svg) {
    flex-shrink: 0;
    color: var(--aviso);
  }
  .permiso.info {
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .permiso.info > :global(svg) {
    color: var(--texto-3);
  }
  /* El texto toma el resto; si el botón no cabe al lado, baja. */
  .permiso p {
    flex: 1 1 220px;
    min-width: 0;
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .permiso b {
    color: var(--texto);
    font-weight: 600;
  }

  /* ---------- Teclas ---------- */
  .teclas {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
  kbd {
    display: inline-grid;
    place-items: center;
    min-width: 26px;
    height: 26px;
    padding: 0 7px;
    border-radius: 7px;
    background: var(--superficie);
    box-shadow:
      inset 0 0 0 1px var(--borde-fuerte),
      inset 0 -1.5px 0 var(--borde-fuerte);
    color: var(--texto);
    font-family: var(--fuente);
    font-size: 12px;
    line-height: 1;
    font-weight: 600;
    white-space: nowrap;
  }
  kbd.simbolo {
    font-size: 13.5px;
    font-weight: 500;
  }
  .mac kbd,
  kbd.mac {
    color: var(--primario-texto);
    background: var(--elegido-bg);
    box-shadow:
      inset 0 0 0 1px var(--elegido-borde),
      inset 0 -1.5px 0 var(--elegido-borde);
  }
  .flecha {
    display: grid;
    place-items: center;
    color: var(--texto-3);
  }

  /* Una tabla pareja para todos los grupos: cada fila es [teclas de Windows] → [teclas de la Mac] y la acción
     justo después. Las columnas de teclas son `auto`: miden lo que pide la combinación más larga si hay lugar y,
     si no, se encogen (las teclas bajan de línea). Así la tabla nunca se sale de la tarjeta. Con subgrid, las
     columnas quedan alineadas de un grupo a otro. */
  .grupos {
    display: grid;
    grid-template-columns: auto 14px auto minmax(0, 1fr);
    column-gap: 10px;
    row-gap: 14px;
    padding: 2px 0 8px;
  }
  .grupo,
  .lista-atajos {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
  }
  /* Con la traducción apagada, la tabla queda de referencia: el lado de la Mac pierde el color. */
  .grupos.apagado .mac kbd {
    color: var(--texto-2);
    background: var(--superficie-elevada);
    box-shadow:
      inset 0 0 0 1px var(--borde),
      inset 0 -1.5px 0 var(--borde);
  }
  .grupo .rotulo {
    grid-column: 1 / -1;
    margin: 0 0 2px;
  }
  .lista-atajos {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .atajo {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: center;
    padding: 6px 0;
    border-top: 1px solid var(--borde);
  }
  .atajo:first-child {
    border-top: 0;
  }
  .accion {
    min-width: 0;
    font-size: 13px;
    line-height: 18px;
    color: var(--texto);
    overflow-wrap: break-word;
  }
  /* Muy ancho: dos mitades por grupo, con un respiro al medio (cada acción conserva unos 280 px). */
  @container (min-width: 1000px) {
    .grupos {
      grid-template-columns:
        auto 14px auto minmax(0, 1fr)
        28px
        auto 14px auto minmax(0, 1fr);
    }
    .atajo:nth-child(odd) {
      grid-column: 1 / 5;
    }
    .atajo:nth-child(even) {
      grid-column: 6 / 10;
    }
    .atajo:nth-child(2) {
      border-top: 0;
    }
  }
  /* Angosto: la acción arriba, a lo ancho, y las teclas debajo. */
  @container (max-width: 560px) {
    .atajo {
      row-gap: 6px;
      padding: 8px 0;
    }
    .accion {
      grid-row: 1;
      grid-column: 1 / -1;
    }
    .atajo > .teclas.windows {
      grid-row: 2;
      grid-column: 1;
    }
    .atajo > .flecha {
      grid-row: 2;
      grid-column: 2;
    }
    .atajo > .teclas.mac {
      grid-row: 2;
      grid-column: 3;
    }
  }

  .mas-atajos {
    margin-bottom: 10px;
  }
  .giro {
    display: grid;
    transition: transform var(--dur-vista) ease;
  }
  .giro.abierto {
    transform: rotate(180deg);
  }

  /* ---------- 2. Pantalla virtual ---------- */
  .silueta-filas {
    height: 150px;
    margin-bottom: 12px;
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
  }
  .guia-virtual {
    min-width: 0;
    margin-bottom: 12px;
    padding: 14px 16px;
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
  }
  .guia-virtual .rotulo {
    margin: 0 0 10px;
  }
  .pasos {
    display: grid;
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;
    counter-reset: paso;
  }
  /* Dos columnas: el número y el texto (el texto va en un solo <span>, con sus negritas adentro). */
  .pasos li {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    align-items: start;
    gap: 10px;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto-2);
    counter-increment: paso;
  }
  .pasos li::before {
    content: counter(paso);
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--elegido-bg);
    box-shadow: inset 0 0 0 1px var(--elegido-borde);
    color: var(--primario-texto);
    font-size: 11.5px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .pasos li > span {
    min-width: 0;
    padding-top: 1px;
  }
  .pasos b,
  .consejo b {
    color: var(--texto);
    font-weight: 600;
  }
  .consejo {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 14px 0 0;
    padding-top: 12px;
    border-top: 1px solid var(--borde);
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .consejo :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
    color: var(--texto-3);
  }
  .consejo > span {
    min-width: 0;
  }
  /* En Windows, con espacio: los pasos a la izquierda y el consejo a la derecha, separados por una línea. */
  @container (min-width: 760px) {
    .guia-pasos.ancha {
      display: grid;
      grid-template-columns: minmax(0, 3fr) minmax(0, 2fr);
      gap: 0 24px;
      align-items: start;
    }
    .guia-pasos.ancha .consejo {
      margin: 0;
      padding: 0 0 0 20px;
      border-top: 0;
      border-left: 1px solid var(--borde);
    }
  }

  /* ---------- Captura ---------- */
  .guia {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 12px;
    margin: 0 0 14px;
    padding: 12px 14px;
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
  }
  .guia__paso {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .guia__icono {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde-fuerte);
    color: var(--texto-2);
  }
  .botones {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding-bottom: 12px;
  }
  /* El ícono y el texto en un <span>: el texto fluye entero y baja de línea a la par del ícono. */
  .resultado {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0 0 12px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .resultado > span {
    min-width: 0;
  }
  .resultado :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
    color: var(--exito);
  }

  /* ---------- Portapapeles ---------- */
  .historial-cabeza {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 0 4px;
    border-top: 1px solid var(--borde);
  }
  .historial-cabeza .rotulo {
    margin: 0;
  }
  .copias {
    margin: 0;
    padding: 0 0 6px;
    list-style: none;
  }
  .copia {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
  }
  .copia + .copia {
    border-top: 1px solid var(--borde);
  }
  .miniatura {
    flex-shrink: 0;
    width: 64px;
    height: 44px;
    border-radius: 8px;
    object-fit: cover;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  img.miniatura {
    border: 1px solid var(--borde);
  }
  .miniatura.texto {
    display: grid;
    place-items: center;
    color: var(--texto-3);
  }
  .copia__cuerpo {
    flex: 1;
    min-width: 0;
  }
  .copia__texto {
    margin: 0;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .faltan {
    color: var(--texto-3);
  }
  .copia__meta {
    display: flex;
    align-items: center;
    gap: 5px;
    margin: 3px 0 0;
    font-size: 12px;
    line-height: 16px;
    color: var(--texto-3);
  }
  .copia__meta :global(svg) {
    flex-shrink: 0;
  }
  .copiar {
    gap: 6px;
    padding: 0 12px;
  }
  .copiar.hecho {
    color: var(--exito);
  }
  .vacio {
    margin: 0;
    padding: 6px 0 14px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }

  /* ---------- Siluetas mientras carga (quietas) ---------- */
  .silueta-estado {
    height: 220px;
    border-radius: var(--radio-lg);
  }
  .silueta-panel {
    height: 320px;
    border-radius: var(--radio-lg);
  }
  .copia.silueta .linea {
    flex: 1;
    max-width: 60%;
    height: 12px;
    border-radius: 6px;
    background: var(--superficie-elevada);
  }
</style>
