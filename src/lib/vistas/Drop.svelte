<!--
  Drop: archivos, carpetas, textos y enlaces entre tus equipos por Tailscale (Nexo Drop).
  Arriba, a quién se manda: tu otro equipo con Nexo, si está en línea y su dirección (con un menú si hay más de uno).
  Debajo, la zona para soltar (se ilumina mientras arrastras sobre la ventana), las acciones rápidas, lo que está
  en curso con su avance y, al lado, el historial y los ajustes de este equipo.
  Soltar en cualquier parte de la ventana también envía (CapaDrop.svelte).
  Los destinos se releen cada 10 s mientras se ve y al llegar algo; el avance llega por "drop-progreso".
  Superficies sólidas, el hover solo cambia el color y ningún texto se corta: todo baja de línea.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    abrirCarpetaDrop,
    abrirDrop,
    ajustarDrop,
    alCambiarHistorialDrop,
    alProgresoDrop,
    borrarHistorialDrop,
    cancelarDrop,
    copiarDrop,
    enviarCopiadoDrop,
    enviarEnlaceDrop,
    esMac,
    leerAjustesDrop,
    leerEnviosDrop,
    leerHistorialDrop,
    mostrarDrop,
  } from '$lib/api-sistema';
  import type { AjustesDrop, EntradaDrop, EstadoDrop, PlataformaEquipo, ProgresoDrop } from '$lib/tipos-sistema';
  import { destinoActual, drop, elegirDestino, leerDestinos } from '$lib/drop.svelte';
  import { cuando, duracion } from '$lib/tiempo';
  import { tooltip } from '$lib/tooltip';
  import Icono, { type Nombre } from '$lib/componentes/Icono.svelte';
  import Interruptor from '$lib/componentes/Interruptor.svelte';
  import MenuDesplegable from '$lib/componentes/MenuDesplegable.svelte';
  import Dialogo from '$lib/componentes/Dialogo.svelte';

  let { onerror }: { onerror: (texto: string) => void } = $props();

  let estado = $state<EstadoDrop | null>(null);
  let historial = $state<EntradaDrop[] | null>(null);
  let transferencias = $state<ProgresoDrop[]>([]);

  const destino = $derived(destinoActual());
  const plataformaPropia = $derived<PlataformaEquipo>(estado?.plataforma ?? (esMac ? 'mac' : 'windows'));
  const iconoDe = (p: PlataformaEquipo): Nombre => (p === 'mac' ? 'portatil' : 'pantallas');
  const corto = (p: PlataformaEquipo) => (p === 'mac' ? 'Mac' : p === 'windows' ? 'PC' : 'Equipo');
  const sistema = (p: PlataformaEquipo) => (p === 'mac' ? 'Mac' : p === 'windows' ? 'Windows' : 'Otro sistema');

  // --- Números en palabras ---
  const miles = (n: number) => n.toLocaleString('es');

  /** "412 KB", "2,8 MB", "1,2 GB" (como Finder: de a 1000). */
  function peso(bytes: number): string {
    if (bytes < 1000) return `${bytes} B`;
    const unidades = ['KB', 'MB', 'GB', 'TB'];
    let valor = bytes / 1000;
    let i = 0;
    while (valor >= 1000 && i < unidades.length - 1) {
      valor /= 1000;
      i++;
    }
    return `${valor.toLocaleString('es', { maximumFractionDigits: valor < 10 ? 1 : 0 })} ${unidades[i]}`;
  }

  const archivos = (n: number) => (n === 1 ? '1 archivo' : `${miles(n)} archivos`);
  const quedan = (s: number) => (s < 60 ? `${Math.max(1, Math.round(s))} s` : duracion(s));

  // --- Lectura ---
  async function leerEstado() {
    try {
      estado = await leerAjustesDrop();
    } catch (e) {
      if (!estado) onerror(String(e));
    }
  }

  async function leerHistorial() {
    try {
      historial = await leerHistorialDrop();
    } catch {
      historial ??= [];
    }
  }

  async function leerEnCurso() {
    try {
      for (const p of await leerEnviosDrop()) alAvanzar(p);
    } catch {
      // El avance llega igual por los avisos.
    }
  }

  // --- En curso ---
  const claveDe = (p: ProgresoDrop) => `${p.direccion}:${p.lote}`;
  const terminado = (p: ProgresoDrop) => p.estado === 'listo' || p.estado === 'cancelado' || p.estado === 'error';
  const quitarDespues = new Map<string, ReturnType<typeof setTimeout>>();
  /** Cuándo llegó el último aviso de cada transferencia (lo que llega y se queda quieto, se quita). */
  const vistoEn = new Map<string, number>();

  function quitar(clave: string) {
    transferencias = transferencias.filter((t) => claveDe(t) !== clave);
    quitarDespues.delete(clave);
    vistoEn.delete(clave);
  }

  function alAvanzar(p: ProgresoDrop) {
    const clave = claveDe(p);
    vistoEn.set(clave, Date.now());
    const i = transferencias.findIndex((t) => claveDe(t) === clave);
    if (i >= 0) transferencias[i] = p;
    else transferencias.push(p);
    clearTimeout(quitarDespues.get(clave));
    quitarDespues.delete(clave);
    if (terminado(p)) {
      // "Listo" se ve un momento; un error, un rato más para poder leerlo.
      quitarDespues.set(clave, setTimeout(() => quitar(clave), p.estado === 'error' ? 9000 : 2500));
      leerHistorial();
      if (p.estado === 'error') leerDestinos(true);
    }
  }

  /** Lo que llega y lleva 15 s sin avisos: el otro lado dejó de mandar. */
  function barrer() {
    const ahora = Date.now();
    for (const t of transferencias) {
      const clave = claveDe(t);
      if (t.direccion === 'recibido' && !terminado(t) && ahora - (vistoEn.get(clave) ?? ahora) > 15000) quitar(clave);
    }
  }

  function fraccion(t: ProgresoDrop): number | null {
    if (t.direccion === 'enviado') return t.tamanoLote > 0 ? Math.min(1, t.enviadosLote / t.tamanoLote) : null;
    return t.tamano > 0 ? Math.min(1, t.enviados / t.tamano) : null;
  }

  function cifras(t: ProgresoDrop): string {
    const [hecho, total] = t.direccion === 'enviado' ? [t.enviadosLote, t.tamanoLote] : [t.enviados, t.tamano];
    const partes = [`${peso(hecho)} de ${peso(total)}`];
    if (t.velocidad > 0 && hecho < total) {
      partes.push(`${peso(Math.round(t.velocidad))}/s`);
      partes.push(`quedan ${quedan((total - hecho) / t.velocidad)}`);
    }
    return partes.join(' · ');
  }

  function tituloTransferencia(t: ProgresoDrop): string {
    if (t.estado === 'esperando') return 'En espera';
    if (!t.nombre) return 'Preparando el envío…';
    return t.nombre;
  }

  function metaTransferencia(t: ProgresoDrop): string {
    const quien = t.direccion === 'enviado' ? `A ${t.equipo}` : `De ${t.equipo}`;
    if (t.estado === 'esperando') return `${quien} · sale cuando termine el envío anterior`;
    if (t.total > 1) return `${quien} · archivo ${miles(t.indice + 1)} de ${miles(t.total)}`;
    return quien;
  }

  let cancelando = $state<string | null>(null);
  async function cancelar(t: ProgresoDrop) {
    cancelando = t.lote;
    try {
      await cancelarDrop(t.lote);
    } catch (e) {
      onerror(String(e));
    }
    cancelando = null;
  }

  // --- Acciones rápidas ---
  type Tono = 'ok' | 'aviso';
  let resultado = $state<{ texto: string; tono: Tono } | null>(null);
  let temporizadorResultado: ReturnType<typeof setTimeout> | undefined;
  function mostrarResultado(texto: string, tono: Tono) {
    resultado = { texto, tono };
    clearTimeout(temporizadorResultado);
    temporizadorResultado = setTimeout(() => (resultado = null), 8000);
  }

  let enviandoCopiado = $state(false);
  async function enviarCopiado() {
    const d = destino;
    if (!d || enviandoCopiado) return;
    enviandoCopiado = true;
    try {
      const r = await enviarCopiadoDrop(d.ip);
      if (r.tipo === 'texto') mostrarResultado(`Texto enviado: ya está en el portapapeles de ${d.nombre}.`, 'ok');
      else if (r.tipo === 'imagen') mostrarResultado(`Enviando la imagen a ${d.nombre}.`, 'ok');
      else mostrarResultado(`Enviando ${r.cantidad === 1 ? '1 elemento' : `${miles(r.cantidad)} elementos`} a ${d.nombre}.`, 'ok');
      leerHistorial();
    } catch (e) {
      mostrarResultado(String(e), 'aviso');
    }
    enviandoCopiado = false;
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
      mostrarResultado(`Listo: se abrió en ${d.nombre}.`, 'ok');
      leerHistorial();
    } catch (e) {
      mostrarResultado(String(e), 'aviso');
    }
    abriendoEnlace = false;
  }

  async function abrirCarpeta() {
    try {
      await abrirCarpetaDrop();
    } catch (e) {
      onerror(String(e));
    }
  }

  // --- Historial ---
  const LIMITE_VISTA = 120;

  /** Las dos primeras líneas con texto, hasta unos 120 caracteres (cortando en un espacio). Sin "…": se dice cuánto falta. */
  function vistaTexto(completo: string): { texto: string; faltan: number } {
    const lineas = completo
      .replace(/\r/g, '')
      .split('\n')
      .filter((l) => l.trim() !== '');
    let texto = lineas.slice(0, 2).join('\n');
    if (texto.length > LIMITE_VISTA) {
      const espacio = texto.lastIndexOf(' ', LIMITE_VISTA);
      texto = texto.slice(0, espacio > LIMITE_VISTA * 0.6 ? espacio : LIMITE_VISTA).trimEnd();
    }
    return { texto, faltan: Math.max(0, [...completo].length - [...texto].length) };
  }

  function tituloEntrada(e: EntradaDrop): string {
    if (e.tipo === 'texto') return e.texto ? vistaTexto(e.texto).texto : 'Texto oculto';
    if (e.tipo === 'enlace') return e.texto ?? 'Enlace';
    if (e.tipo === 'varios' && e.cantidad > 1) return `${e.nombre} y ${miles(e.cantidad - 1)} más`;
    return e.nombre;
  }

  function detalleEntrada(e: EntradaDrop): string {
    const quien = e.direccion === 'enviado' ? `A ${e.equipo}` : `De ${e.equipo}`;
    const partes = [quien];
    if (e.tipo === 'texto') {
      if (!e.texto) partes.push('parecía una contraseña: no se guardó');
    } else if (e.tipo !== 'enlace') {
      if (e.estado === 'en curso' || (e.estado !== 'completo' && e.total > 1))
        partes.push(`${miles(e.cantidad)} de ${archivos(e.total)}`);
      else if (e.tipo !== 'archivo') partes.push(archivos(e.cantidad));
      if (e.tamano > 0) partes.push(peso(e.tamano));
    }
    if (e.estado === 'cancelado') partes.push('cancelado');
    partes.push(cuando(e.momento));
    return partes.join(' · ');
  }

  const iconoEntrada = (e: EntradaDrop): Nombre =>
    e.tipo === 'texto' ? 'portapapeles' : e.tipo === 'enlace' ? 'conectar' : e.tipo === 'carpeta' ? 'carpeta' : e.tipo === 'varios' ? 'copiar' : 'archivo';

  const esArchivo = (e: EntradaDrop) => e.tipo === 'archivo' || e.tipo === 'carpeta' || e.tipo === 'varios';

  let ocupadoId = $state<number | null>(null);
  let copiadoId = $state<number | null>(null);
  let temporizadorCopia: ReturnType<typeof setTimeout> | undefined;

  async function accion(e: EntradaDrop, que: 'abrir' | 'mostrar' | 'copiar') {
    if (ocupadoId !== null) return;
    ocupadoId = e.id;
    try {
      if (que === 'abrir') await abrirDrop(e.id);
      else if (que === 'mostrar') await mostrarDrop(e.id);
      else {
        await copiarDrop(e.id);
        copiadoId = e.id;
        clearTimeout(temporizadorCopia);
        temporizadorCopia = setTimeout(() => (copiadoId = null), 1800);
      }
    } catch (err) {
      onerror(String(err));
      leerHistorial();
    }
    ocupadoId = null;
  }

  let preguntandoBorrar = $state(false);
  async function borrarHistorial() {
    preguntandoBorrar = false;
    try {
      await borrarHistorialDrop();
      historial = [];
    } catch (e) {
      onerror(String(e));
    }
  }

  // --- Ajustes de este equipo ---
  let ocupado = $state<keyof AjustesDrop | null>(null);
  async function ajustar(clave: keyof AjustesDrop, valor: boolean) {
    if (!estado || ocupado) return;
    ocupado = clave;
    try {
      estado = await ajustarDrop({ [clave]: valor });
    } catch (e) {
      onerror(String(e));
    }
    ocupado = null;
  }

  const ayudaRecibir = $derived.by(() => {
    const e = estado;
    if (!e) return '';
    if (!e.ajustes.recibir) return 'Apagado: tus otros equipos no pueden mandarte nada.';
    if (e.escuchando) return `Tus equipos pueden mandarte a ${e.nombre} (${e.escuchando}). Solo tus equipos en Tailscale.`;
    if (e.tailscale === 'no instalado') return 'Instala Tailscale para recibir.';
    return 'Se activa solo en cuanto Tailscale se conecte.';
  });

  // --- Destino ---
  const opcionesDestino = $derived(
    drop.destinos.map((d) => ({
      valor: d.ip,
      nombre: d.nombre,
      detalle: `${sistema(d.plataforma)} · ${d.enLinea ? 'En línea' : 'Apagada o sin Nexo'}`,
      icono: iconoDe(d.plataforma),
    })),
  );

  const sinDestino = $derived.by((): { titulo: string; detalle: string } => {
    const ts = estado?.tailscale;
    if (ts === 'no instalado')
      return { titulo: 'Falta Tailscale', detalle: 'Nexo Drop viaja por Tailscale: instálalo en tus dos equipos con la misma cuenta.' };
    if (ts === 'detenido')
      return { titulo: 'Tailscale está desconectado', detalle: 'Conéctalo en este equipo para ver tus otros equipos.' };
    return {
      titulo: 'No encontré otro equipo tuyo',
      detalle: 'Abre Nexo en tu otro equipo (Mac o Windows), con Tailscale conectado y tu misma cuenta.',
    };
  });

  onMount(() => {
    leerDestinos(true);
    leerEstado();
    leerHistorial();
    leerEnCurso();
    const quitarAvisos = [
      alProgresoDrop(alAvanzar),
      alCambiarHistorialDrop(() => {
        leerHistorial();
        leerDestinos();
      }),
    ];
    // Cada 5 s se barre lo que llega y quedó quieto; cada 10 s, mientras se ve, se releen los destinos.
    let vuelta = 0;
    const intervalo = setInterval(() => {
      barrer();
      vuelta++;
      if (vuelta % 2 === 0 && document.visibilityState === 'visible') {
        leerDestinos(true);
        leerEstado();
      }
    }, 5000);
    const alVolver = () => {
      if (document.visibilityState !== 'visible') return;
      leerDestinos(true);
      leerEstado();
      leerHistorial();
    };
    document.addEventListener('visibilitychange', alVolver);
    window.addEventListener('focus', alVolver);
    return () => {
      for (const q of quitarAvisos) q();
      clearInterval(intervalo);
      clearTimeout(temporizadorResultado);
      clearTimeout(temporizadorCopia);
      for (const t of quitarDespues.values()) clearTimeout(t);
      document.removeEventListener('visibilitychange', alVolver);
      window.removeEventListener('focus', alVolver);
    };
  });
</script>

{#snippet ficha(icono: Nombre, activa: boolean)}
  <span class="ficha" class:activa aria-hidden="true"><Icono nombre={icono} tamano={18} /></span>
{/snippet}

<div class="vista">
  <header class="cabecera">
    <h1>Drop</h1>
    <p>Pasa archivos, textos y enlaces a tu otro equipo por Tailscale, directo y sin nube.</p>
  </header>

  <!-- A quién se manda -->
  {#if !drop.leidos}
    <div class="silueta-destino vidrio" aria-busy="true"><span class="solo-lector">Buscando tus equipos…</span></div>
  {:else}
    <section class="destino vidrio" aria-labelledby="drop-destino">
      <div class="ruta" class:viva={destino?.enLinea} class:apagada={!!destino && !destino.enLinea} aria-hidden="true">
        <span class="nodo">
          <span class="nodo__icono"><Icono nombre={iconoDe(plataformaPropia)} tamano={22} /></span>
          <span class="nodo__nombre">{corto(plataformaPropia)}</span>
        </span>
        <span class="tramo">
          <span class="tramo__linea"></span>
          {#if destino}<span class="tramo__via">Tailscale</span>{/if}
        </span>
        <span class="nodo otro">
          <span class="nodo__icono"><Icono nombre={destino ? iconoDe(destino.plataforma) : 'dispositivos'} tamano={22} /></span>
          <span class="nodo__nombre">{destino ? corto(destino.plataforma) : 'Otro'}</span>
        </span>
      </div>

      {#if destino}
        <div class="destino__textos" role="status" aria-live="polite">
          <p class="rotulo">Enviar a</p>
          <h2 class="destino__nombre" id="drop-destino">{destino.nombre}</h2>
          <p class="destino__estado">
            <span class="punto {destino.enLinea ? 'ok' : 'aviso'}" aria-hidden="true"></span>
            <span class={destino.enLinea ? 'ok' : 'aviso'}>{destino.enLinea ? 'En línea' : 'Apagada o sin Nexo'}</span>
            <span class="destino__ip">{sistema(destino.plataforma)} · {destino.ip}</span>
          </p>
        </div>
        {#if drop.destinos.length > 1}
          <div class="destino__menu">
            <MenuDesplegable
              opciones={opcionesDestino}
              valor={destino.ip}
              etiqueta="Elegir a qué equipo enviar"
              oncambio={elegirDestino}
            />
          </div>
        {/if}
      {:else}
        <div class="destino__textos" role="status">
          <h2 class="destino__nombre" id="drop-destino">{sinDestino.titulo}</h2>
          <p class="destino__detalle">{sinDestino.detalle}</p>
        </div>
      {/if}
    </section>
  {/if}

  <div class="columnas">
    <div class="columna">
      <!-- Zona para soltar: se ilumina mientras hay un arrastre sobre la ventana -->
      <section class="zona vidrio" class:encima={drop.arrastrando} aria-label="Zona para soltar archivos">
        <span class="zona__icono" aria-hidden="true"><Icono nombre="drop" tamano={30} /></span>
        <p class="zona__titulo">
          {drop.arrastrando && destino ? `Suelta para enviar a ${destino.nombre}` : 'Arrastra archivos o carpetas aquí'}
        </p>
        <p class="zona__detalle">
          {destino ? `Llegan a Descargas › Nexo Drop en ${destino.nombre}` : 'Cuando tu otro equipo tenga Nexo abierto, llegarán a su carpeta Descargas.'}
        </p>
      </section>

      <!-- Acciones rápidas -->
      <div class="acciones">
        <button class="accion vidrio" type="button" disabled={!destino || enviandoCopiado} aria-busy={enviandoCopiado} onclick={enviarCopiado}>
          {@render ficha('portapapeles', !!destino)}
          <span class="textos">
            <span class="titulo">{enviandoCopiado ? 'Enviando…' : 'Enviar lo que copiaste'}</span>
            <span class="ayuda">Texto, imagen o archivos copiados</span>
          </span>
        </button>
        <button class="accion vidrio" type="button" onclick={abrirCarpeta}>
          {@render ficha('carpeta', true)}
          <span class="textos">
            <span class="titulo">Abrir carpeta Nexo Drop</span>
            <span class="ayuda">Lo que recibes queda en Descargas</span>
          </span>
        </button>
        <form class="accion accion--enlace vidrio" onsubmit={abrirEnlace}>
          {@render ficha('conectar', !!destino)}
          <div class="textos">
            <label class="titulo" for="drop-enlace">Abrir un enlace allá</label>
            <span class="ayuda">Se abre en el navegador de {destino?.nombre ?? 'tu otro equipo'}</span>
            <div class="campo-fila">
              <input
                id="drop-enlace"
                class="campo"
                type="text"
                inputmode="url"
                autocomplete="off"
                spellcheck="false"
                placeholder="https://"
                bind:value={enlace}
                disabled={!destino}
              />
              <button class="boton-primario" type="submit" disabled={!destino || !enlace.trim() || abriendoEnlace} aria-busy={abriendoEnlace}>
                Abrir
              </button>
            </div>
          </div>
        </form>
      </div>

      {#if resultado}
        <p class="resultado {resultado.tono}" role="status">
          <Icono nombre={resultado.tono === 'ok' ? 'check-circulo' : 'alerta'} tamano={16} />
          {resultado.texto}
        </p>
      {/if}

      <!-- En curso -->
      {#if transferencias.length}
        <section class="panel vidrio" aria-labelledby="drop-en-curso">
          <h2 class="rotulo" id="drop-en-curso">En curso</h2>
          <ul class="lista">
            {#each transferencias as t (claveDe(t))}
              {@const f = fraccion(t)}
              <li class="transferencia" class:terminada={terminado(t)}>
                <span class="ficha direccion {t.direccion}" aria-hidden="true">
                  <Icono nombre={t.direccion === 'enviado' ? 'enviado' : 'recibido'} tamano={18} />
                </span>
                <div class="cuerpo">
                  <p class="nombre">{tituloTransferencia(t)}</p>
                  <p class="meta">{metaTransferencia(t)}</p>
                  {#if t.estado === 'transfiriendo' || t.estado === 'esperando'}
                    <div
                      class="barra"
                      role="progressbar"
                      aria-label="Avance"
                      aria-valuemin={0}
                      aria-valuemax={100}
                      aria-valuenow={f === null ? undefined : Math.round(f * 100)}
                    >
                      <span style:transform="scaleX({f ?? 0})"></span>
                    </div>
                    {#if t.estado === 'transfiriendo' && f !== null}<p class="cifras">{cifras(t)}</p>{/if}
                  {:else if t.estado === 'listo'}
                    <p class="final ok"><Icono nombre="check-circulo" tamano={15} /> Listo</p>
                  {:else if t.estado === 'cancelado'}
                    <p class="final">Cancelado</p>
                  {:else}
                    <p class="final aviso"><Icono nombre="alerta" tamano={15} /> {t.error ?? 'No se pudo terminar.'}</p>
                  {/if}
                </div>
                {#if t.direccion === 'enviado' && !terminado(t)}
                  <button class="boton-vidrio boton" type="button" disabled={cancelando === t.lote} onclick={() => cancelar(t)}>
                    Cancelar
                  </button>
                {/if}
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    </div>

    <div class="columna">
      <!-- Historial -->
      <section class="panel vidrio" aria-labelledby="drop-historial">
        <div class="historial-cabeza">
          <h2 class="rotulo" id="drop-historial">Historial</h2>
          {#if historial?.length}
            <button class="boton-texto peligro" type="button" onclick={() => (preguntandoBorrar = true)}>Borrar historial</button>
          {/if}
        </div>

        {#if historial === null}
          <ul class="lista" aria-busy="true">
            {#each [0, 1, 2] as i (i)}<li class="entrada silueta"><span class="miniatura"></span><span class="linea"></span></li>{/each}
          </ul>
        {:else if historial.length === 0}
          <p class="vacio">Todavía no hay envíos. Lo que mandes o te llegue aparece aquí.</p>
        {:else}
          <ul class="lista">
            {#each historial as e (e.id)}
              <li class="entrada">
                <span class="miniatura-caja">
                  {#if e.miniatura}
                    <img class="miniatura" src={e.miniatura} alt="Vista previa de {e.nombre}" width="44" height="44" draggable="false" />
                  {:else}
                    <span class="miniatura icono" aria-hidden="true"><Icono nombre={iconoEntrada(e)} tamano={19} /></span>
                  {/if}
                  <span class="insignia {e.direccion}" use:tooltip={e.direccion === 'enviado' ? 'Enviado' : 'Recibido'}>
                    <Icono nombre={e.direccion === 'enviado' ? 'enviado' : 'recibido'} tamano={11} />
                    <span class="solo-lector">{e.direccion === 'enviado' ? 'Enviado' : 'Recibido'}</span>
                  </span>
                </span>
                <div class="cuerpo">
                  {#if e.tipo === 'texto' && e.texto}
                    {@const v = vistaTexto(e.texto)}
                    <p class="nombre texto">
                      {v.texto}{#if v.faltan > 0}<span class="faltan"> y {miles(v.faltan)} caracteres más</span>{/if}
                    </p>
                  {:else}
                    <p class="nombre" class:enlace={e.tipo === 'enlace'}>{tituloEntrada(e)}</p>
                  {/if}
                  <p class="meta">{detalleEntrada(e)}</p>
                  {#if e.estado === 'error' && e.error}
                    <p class="final aviso"><Icono nombre="alerta" tamano={14} /> {e.error}</p>
                  {:else if esArchivo(e) && e.direccion === 'recibido' && !e.existe && e.estado === 'completo'}
                    <p class="meta">Ya no está en Nexo Drop: lo movieron o lo borraron.</p>
                  {/if}
                </div>
                <div class="botones">
                  {#if esArchivo(e) && e.existe}
                    {#if e.direccion === 'recibido'}
                      <button class="boton-vidrio boton" type="button" disabled={ocupadoId !== null} onclick={() => accion(e, 'abrir')}>Abrir</button>
                    {/if}
                    <button class="boton-vidrio boton" type="button" disabled={ocupadoId !== null} onclick={() => accion(e, 'mostrar')}>
                      Mostrar
                    </button>
                  {:else if e.tipo === 'enlace' && e.texto}
                    <button class="boton-vidrio boton" type="button" disabled={ocupadoId !== null} onclick={() => accion(e, 'abrir')}>Abrir</button>
                  {/if}
                  {#if (e.tipo === 'texto' || e.tipo === 'enlace') && e.texto}
                    <button
                      class="boton-vidrio boton"
                      class:hecho={copiadoId === e.id}
                      type="button"
                      disabled={ocupadoId !== null}
                      onclick={() => accion(e, 'copiar')}
                    >
                      <Icono nombre={copiadoId === e.id ? 'check' : 'copiar'} tamano={15} />
                      {copiadoId === e.id ? 'Copiado' : 'Copiar'}
                    </button>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      <!-- Este equipo -->
      <section class="panel vidrio" aria-labelledby="drop-ajustes">
        <h2 class="rotulo" id="drop-ajustes">En este equipo</h2>
        {#if estado}
          {@const a = estado.ajustes}
          <div class="fila">
            {@render ficha('recibido', a.recibir && !!estado.escuchando)}
            <div class="textos">
              <div class="titulo">Recibir archivos</div>
              <div class="ayuda">{ayudaRecibir}</div>
            </div>
            <Interruptor
              encendido={a.recibir}
              cargando={ocupado === 'recibir'}
              etiqueta="Recibir archivos"
              oncambio={() => ajustar('recibir', !a.recibir)}
            />
          </div>
          <div class="fila">
            {@render ficha('portapapeles', a.copiarImagenes)}
            <div class="textos">
              <div class="titulo">Copiar imágenes recibidas al portapapeles</div>
              <div class="ayuda">Si llega una sola imagen, queda lista para pegar.</div>
            </div>
            <Interruptor
              encendido={a.copiarImagenes}
              cargando={ocupado === 'copiarImagenes'}
              etiqueta="Copiar imágenes recibidas al portapapeles"
              oncambio={() => ajustar('copiarImagenes', !a.copiarImagenes)}
            />
          </div>
          {#if !esMac}
            <p class="nota">
              <Icono nombre="info" tamano={15} />
              La primera vez, Windows puede preguntar si Nexo puede usar la red: permite el acceso en redes privadas.
            </p>
          {/if}
        {:else}
          <div class="fila silueta-fila" aria-busy="true"><span class="linea"></span></div>
        {/if}
      </section>
    </div>
  </div>
</div>

<Dialogo
  abierto={preguntandoBorrar}
  titulo="¿Borrar el historial de Nexo Drop?"
  descripcion="Solo se borra la lista. Los archivos que recibiste siguen en Descargas › Nexo Drop."
  icono="papelera"
  tono="peligro"
  textoConfirmar="Borrar"
  textoCancelar="Cancelar"
  onconfirmar={borrarHistorial}
  oncancelar={() => (preguntandoBorrar = false)}
/>

<style>
  .vista {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
    container-type: inline-size;
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

  /* ---------- Destino ---------- */
  .destino {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 20px 28px;
    padding: 22px 24px;
    border-radius: var(--radio-lg);
  }
  .silueta-destino {
    height: 120px;
    border-radius: var(--radio-lg);
  }

  /* Este equipo → el otro: dos fichas y una línea quieta. Verde si Nexo responde, ámbar si no. */
  .ruta {
    --color-ruta: var(--borde-fuerte);
    display: flex;
    align-items: flex-start;
  }
  .ruta.viva {
    --color-ruta: var(--exito);
  }
  .ruta.apagada {
    --color-ruta: var(--aviso);
  }
  .nodo {
    display: grid;
    justify-items: center;
    gap: 6px;
  }
  .nodo__icono {
    display: grid;
    place-items: center;
    width: 52px;
    height: 52px;
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-3);
  }
  .viva .nodo__icono,
  .apagada .otro .nodo__icono {
    color: var(--color-ruta);
    background: color-mix(in srgb, var(--color-ruta) 12%, var(--superficie));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-ruta) 30%, transparent);
  }
  .nodo__nombre {
    font: var(--letra-rotulo);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--texto-3);
  }
  .tramo {
    position: relative;
    display: grid;
    place-items: center;
    width: 84px;
    height: 52px;
  }
  .tramo__linea {
    position: absolute;
    left: 6px;
    right: 6px;
    top: 50%;
    border-top: 2px dashed var(--color-ruta);
  }
  .viva .tramo__linea {
    border-top-style: solid;
  }
  .tramo__via {
    position: relative;
    padding: 1px 8px;
    border-radius: var(--radio-pildora);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-ruta) 45%, transparent);
    color: var(--color-ruta);
    font-size: 11px;
    line-height: 18px;
    font-weight: 600;
  }

  .destino__textos .rotulo {
    margin: 0 0 2px;
  }
  .destino__nombre {
    margin: 0;
    font: var(--letra-seccion);
    letter-spacing: -0.01em;
    overflow-wrap: anywhere;
  }
  .destino__estado {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    margin: 4px 0 0;
    font-size: 13px;
    line-height: 18px;
    font-weight: 600;
  }
  .destino__estado .ok {
    color: var(--exito);
  }
  .destino__estado .aviso {
    color: var(--aviso);
  }
  .destino__ip {
    color: var(--texto-2);
    font-weight: 400;
    font-variant-numeric: tabular-nums;
    user-select: text;
    -webkit-user-select: text;
    cursor: text;
  }
  .destino__detalle {
    margin: 4px 0 0;
    color: var(--texto-2);
  }
  .destino__menu {
    min-width: 220px;
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

  @container (max-width: 760px) {
    .destino {
      grid-template-columns: auto minmax(0, 1fr);
    }
    .destino__menu {
      grid-column: 1 / -1;
    }
  }
  @container (max-width: 520px) {
    .destino {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  /* ---------- Columnas ---------- */
  .columnas,
  .columna {
    display: grid;
    gap: 16px;
    align-content: start;
    min-width: 0;
  }
  @container (min-width: 1040px) {
    .columnas {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }
  }

  /* ---------- Zona para soltar ---------- */
  .zona {
    display: grid;
    justify-items: center;
    align-content: center;
    gap: 6px;
    min-height: 220px;
    padding: 32px 28px;
    border-radius: var(--radio-lg);
    text-align: center;
  }
  /* El borde punteado de adentro dice "aquí se suelta". */
  .zona::after {
    content: '';
    position: absolute;
    inset: 10px;
    border: 1.5px dashed var(--borde-fuerte);
    border-radius: 16px;
    pointer-events: none;
  }
  /* Mientras se arrastra: anillo turquesa y la aurora quieta detrás. Nada se mueve. */
  .zona.encima {
    background:
      radial-gradient(ellipse 260px 160px at 50% 62%, var(--aurora-1), transparent),
      radial-gradient(ellipse 200px 150px at 34% 78%, var(--aurora-2), transparent),
      radial-gradient(ellipse 200px 150px at 66% 78%, var(--aurora-3), transparent),
      var(--superficie);
    box-shadow:
      inset 0 0 0 2px var(--primario),
      0 0 0 1px var(--borde-tarjeta),
      var(--sombra-tarjeta);
  }
  .zona.encima::after {
    border-color: transparent;
  }
  .zona__icono {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    margin-bottom: 8px;
    border-radius: 18px;
    color: var(--primario-texto);
    background: var(--elegido-bg);
    box-shadow: inset 0 0 0 1px var(--elegido-borde);
  }
  .zona__titulo {
    margin: 0;
    font: var(--letra-panel);
    font-size: 17px;
    line-height: 24px;
    color: var(--texto);
    overflow-wrap: anywhere;
  }
  .zona__detalle {
    max-width: 420px;
    margin: 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
    overflow-wrap: anywhere;
  }

  /* ---------- Acciones rápidas ---------- */
  .acciones {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
    gap: 12px;
  }
  .accion {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin: 0;
    padding: 14px 16px;
    border: 0;
    border-radius: var(--radio-md);
    color: var(--texto);
    text-align: left;
    transition: background-color var(--dur-color) ease;
  }
  .accion:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .accion--enlace {
    grid-column: 1 / -1;
  }
  .accion .textos {
    display: grid;
    gap: 1px;
  }
  .campo-fila {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
  .campo {
    -webkit-appearance: none;
    appearance: none;
    flex: 1;
    min-width: 0;
    height: 36px;
    padding: 0 12px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-sm);
    outline: none;
    background: var(--superficie-inserto);
    color: var(--texto);
    font: inherit;
    font-size: 13.5px;
    user-select: text;
    -webkit-user-select: text;
    transition:
      border-color var(--dur-color) ease,
      box-shadow var(--dur-color) ease;
  }
  .campo::placeholder {
    color: var(--texto-3);
  }
  .campo:focus {
    border-color: var(--primario);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--primario) 20%, transparent);
  }
  .campo:disabled {
    opacity: 0.6;
  }

  .resultado {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: -4px 0 0;
    padding: 10px 12px;
    border-radius: var(--radio-sm);
    background: var(--superficie-elevada);
    color: var(--texto-2);
    font-size: 12.5px;
    line-height: 18px;
  }
  .resultado :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
  }
  .resultado.ok :global(svg) {
    color: var(--exito);
  }
  .resultado.aviso :global(svg) {
    color: var(--aviso);
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
    overflow-wrap: anywhere;
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
  .rotulo + .fila {
    border-top: 0;
  }
  .nota {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0 0 12px;
    padding: 10px 12px;
    border-radius: var(--radio-sm);
    background: var(--superficie-elevada);
    color: var(--texto-2);
    font-size: 12.5px;
    line-height: 18px;
  }
  .nota :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
    color: var(--texto-3);
  }

  .boton {
    flex-shrink: 0;
    min-height: 32px;
    padding: 0 12px;
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
  .boton.hecho {
    color: var(--exito);
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

  /* ---------- Listas: en curso e historial ---------- */
  .lista {
    margin: 0;
    padding: 0 0 6px;
    list-style: none;
  }
  .transferencia,
  .entrada {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 0;
  }
  .transferencia + .transferencia,
  .entrada + .entrada {
    border-top: 1px solid var(--borde);
  }
  .cuerpo {
    flex: 1;
    min-width: 0;
  }
  .nombre {
    margin: 0;
    font-size: 13.5px;
    line-height: 19px;
    font-weight: 600;
    color: var(--texto);
    overflow-wrap: anywhere;
  }
  .nombre.texto {
    font-weight: 400;
    white-space: pre-wrap;
  }
  .nombre.enlace {
    font-weight: 500;
    color: var(--primario-texto);
  }
  .faltan {
    color: var(--texto-3);
    white-space: normal;
  }
  .meta {
    margin: 2px 0 0;
    font-size: 12px;
    line-height: 17px;
    color: var(--texto-3);
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
  }
  .cifras {
    margin: 6px 0 0;
    font-size: 12px;
    line-height: 17px;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .final {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 6px 0 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .final :global(svg) {
    flex-shrink: 0;
    margin-top: 1.5px;
  }
  .final.ok {
    color: var(--exito);
  }
  .final.aviso :global(svg) {
    color: var(--aviso);
  }

  .direccion.enviado {
    color: var(--primario-texto);
    background: var(--elegido-bg);
  }
  .direccion.recibido {
    color: var(--info);
    background: color-mix(in srgb, var(--info) 13%, var(--superficie));
  }

  /* Barra de avance: pista hundida y relleno turquesa. Crece con transform, sin animación. */
  .barra {
    height: 6px;
    margin-top: 8px;
    border-radius: var(--radio-pildora);
    background: var(--superficie-hundida);
    overflow: hidden;
  }
  .barra > span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--primario);
    transform-origin: left center;
  }

  .historial-cabeza {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-bottom: 2px;
  }
  .historial-cabeza .rotulo {
    margin: 0;
  }
  .miniatura-caja {
    position: relative;
    flex-shrink: 0;
  }
  .miniatura {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 10px;
    object-fit: cover;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-3);
  }
  img.miniatura {
    border: 1px solid var(--borde);
  }
  /* La dirección: una insignia redonda en la esquina (turquesa lo enviado, azul lo recibido). */
  .insignia {
    position: absolute;
    right: -5px;
    bottom: -5px;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--superficie);
    box-shadow: 0 0 0 2px var(--superficie);
  }
  .insignia.enviado {
    color: var(--primario-texto);
    background: var(--elegido-bg);
  }
  .insignia.recibido {
    color: var(--info);
    background: color-mix(in srgb, var(--info) 16%, var(--superficie));
  }
  .botones {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
    max-width: 50%;
  }
  .botones .boton {
    gap: 6px;
  }
  .vacio {
    margin: 0;
    padding: 6px 0 14px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }

  /* ---------- Siluetas mientras carga (quietas) ---------- */
  .entrada.silueta .linea,
  .silueta-fila .linea {
    flex: 1;
    max-width: 60%;
    height: 12px;
    margin-top: 8px;
    border-radius: 6px;
    background: var(--superficie-elevada);
  }
</style>
