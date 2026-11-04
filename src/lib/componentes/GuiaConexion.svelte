<!--
  "Conecta tus equipos": la guía para unir esta computadora con la otra por Tailscale (y RustDesk para el remoto).
  Cinco pasos, cada uno con su estado (hecho, el que sigue o pendiente), una frase y un botón:
  1. Instala Tailscale · 2. Conéctate a Tailscale · 3. Agrega tu otro equipo · 4. Abre Nexo en el otro equipo ·
  5. RustDesk (en Windows, instalarlo; en la Mac, además, el acceso directo por IP). Con `rustdesk={false}` (XDrop),
  sin el quinto.
  `para="remoto"` busca el equipo del perfil (en Windows tu Mac, en la Mac tu PC); `para="equipos"`, cualquiera tuyo.
  Cada paso se revisa solo: al abrirla, al volver a la ventana y cada 10 s mientras se ve (perfil.svelte.ts).
  Los botones solo están donde se puede hacer algo ya: el paso que sigue y RustDesk, que no depende de los demás.
  Al terminar, una línea que nombra el equipo encontrado. Con `oncerrar`, una equis para cerrarla.
  `compacta` (arriba de Remoto y en XDrop): menos aire y lo hecho en una sola línea.
  Superficies sólidas, el hover solo cambia el color y ningún texto se corta: todo baja de línea.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { abrirDescarga, abrirTailscale } from '$lib/api-equipos';
  import { abrirRustDesk, esMac } from '$lib/api-sistema';
  import { nombreTipo, perfil, ponerPerfil, recargar } from '$lib/perfil.svelte';
  import type { Descarga } from '$lib/tipos-sistema';
  import Icono, { type Nombre } from './Icono.svelte';

  let {
    para = 'equipos',
    rustdesk = true,
    compacta = false,
    titulo = 'Conecta tus equipos',
    onerror,
    oncerrar,
  }: {
    para?: 'remoto' | 'equipos';
    rustdesk?: boolean;
    compacta?: boolean;
    titulo?: string;
    onerror?: (texto: string) => void;
    /** Con todo listo, muestra "Cerrar". */
    oncerrar?: () => void;
  } = $props();

  const CADA = 10000;
  const id = $props.id();

  // Lo que busca este equipo para el remoto: en Windows una Mac; en la Mac una PC con Windows.
  const buscada = esMac ? 'windows' : 'mac';
  const otroTexto = esMac ? 'tu PC' : 'tu Mac';
  const otraComputadora = $derived(para === 'remoto' ? otroTexto : 'tu otra computadora');

  type Accion =
    | { tipo: 'descarga'; cual: Descarga; texto: string }
    | { tipo: 'tailscale'; texto: string }
    | { tipo: 'buscar'; texto: string }
    | { tipo: 'rustdesk'; texto: string };

  interface Paso {
    id: string;
    titulo: string;
    frase: string;
    hecho: boolean;
    /** Se puede hacer sin esperar a los pasos de antes. */
    suelto?: boolean;
    accion: Accion;
  }

  const s = $derived(perfil.estado);
  // El otro equipo que cuenta para esta guía.
  const otro = $derived.by(() => {
    if (para === 'remoto') return perfil.otro?.plataforma === buscada ? perfil.otro : null;
    return perfil.otro;
  });
  const computadoras = $derived(
    perfil.equipos.filter((e) => (para === 'remoto' ? e.plataforma === buscada : e.plataforma !== 'otro')),
  );
  // El equipo para nombrar: el del perfil o el primero con Nexo (o, si no, el primero que haya).
  const encontrado = $derived.by(() => {
    if (otro) return { nombre: otro.nombre, plataforma: otro.plataforma };
    const e = computadoras.find((x) => x.nexo) ?? computadoras[0];
    return e ? { nombre: e.nombre, plataforma: e.plataforma } : null;
  });

  const pasos = $derived.by((): Paso[] => {
    if (!s) return [];
    const instalado = s.tailscale !== 'no instalado';
    const conectado = s.tailscale === 'activo';
    const hayOtro = !!otro || computadoras.length > 0;
    const conNexo = para === 'remoto' ? !!otro && (otro.nexo || otro.conNexo) : perfil.conNexo || !!otro?.conNexo;
    const nombreOtro = encontrado?.nombre ?? '';

    const lista: Paso[] = [
      {
        id: 'instalar',
        titulo: 'Instala Tailscale',
        frase: instalado
          ? 'Instalado en este equipo.'
          : 'Une tus equipos en una red privada, estén donde estén.',
        hecho: instalado,
        accion: { tipo: 'descarga', cual: 'tailscale', texto: 'Descargar Tailscale' },
      },
      {
        id: 'conectar',
        titulo: 'Conéctate a Tailscale',
        frase: conectado
          ? `Conectado como ${s.propio.nombre}.`
          : s.tailscale === 'detenido'
            ? 'Está desconectado: ábrelo y conéctalo.'
            : 'Ábrelo y entra con tu cuenta. Usa la misma en tus dos equipos.',
        hecho: conectado,
        accion: { tipo: 'tailscale', texto: 'Abrir Tailscale' },
      },
      {
        id: 'otro',
        titulo: 'Agrega tu otro equipo',
        frase: hayOtro
          ? `Encontré ${nombreOtro}.`
          : `Instala Tailscale en ${otraComputadora} y entra con la misma cuenta.`,
        hecho: hayOtro,
        accion: { tipo: 'buscar', texto: 'Buscar de nuevo' },
      },
      {
        id: 'nexo',
        titulo: 'Abre Nexo en el otro equipo',
        frase: conNexo
          ? `Nexo responde en ${nombreOtro}.`
          : `Instálalo en ${hayOtro && nombreOtro ? nombreOtro : otraComputadora} y déjalo abierto: así se encuentran solos.`,
        hecho: conNexo,
        accion: { tipo: 'descarga', cual: 'nexo', texto: 'Descargar Nexo' },
      },
    ];

    if (rustdesk) {
      const r = s.rustdesk;
      if (esMac) {
        const listo = r.instalado && r.accesoDirecto === true;
        lista.push({
          id: 'rustdesk',
          titulo: r.instalado ? 'Activa el acceso directo en RustDesk' : 'Instala RustDesk',
          frase: listo
            ? 'Listo para que entres desde tu PC.'
            : r.instalado
              ? 'En RustDesk, ve a Ajustes › Seguridad y activa el acceso directo por IP.'
              : 'Después, en sus Ajustes › Seguridad, activa el acceso directo por IP.',
          hecho: listo,
          suelto: true,
          accion: r.instalado
            ? { tipo: 'rustdesk', texto: 'Abrir RustDesk' }
            : { tipo: 'descarga', cual: 'rustdesk', texto: 'Descargar RustDesk' },
        });
      } else {
        lista.push({
          id: 'rustdesk',
          titulo: 'Instala RustDesk',
          frase: r.instalado ? 'Instalado en esta PC.' : 'Con él ves y manejas tu Mac desde esta PC.',
          hecho: r.instalado,
          suelto: true,
          accion: { tipo: 'descarga', cual: 'rustdesk', texto: 'Descargar RustDesk' },
        });
      }
    }
    return lista;
  });

  const actual = $derived(pasos.find((p) => !p.hecho)?.id ?? null);
  const hechos = $derived(pasos.filter((p) => p.hecho).length);
  const listo = $derived(pasos.length > 0 && actual === null);

  const exito = $derived(
    encontrado
      ? `Listo: encontré tu ${nombreTipo(encontrado.plataforma)} ${encontrado.nombre}.`
      : 'Listo: tus equipos ya están conectados.',
  );

  // --- Acciones ---
  let ocupado = $state<string | null>(null);

  async function hacer(p: Paso) {
    if (ocupado) return;
    ocupado = p.id;
    try {
      const a = p.accion;
      if (a.tipo === 'descarga') await abrirDescarga(a.cual);
      else if (a.tipo === 'tailscale') ponerPerfil(await abrirTailscale());
      else if (a.tipo === 'rustdesk') await abrirRustDesk();
      else await recargar(true);
    } catch (e) {
      onerror?.(String(e));
    }
    ocupado = null;
  }

  const ocupadoTexto = (a: Accion) =>
    a.tipo === 'buscar' ? 'Buscando…' : a.tipo === 'tailscale' ? 'Abriendo…' : a.texto;

  const iconoDe = (a: Accion): Nombre =>
    a.tipo === 'descarga' ? 'externo' : a.tipo === 'buscar' ? 'actualizar' : 'chevron';

  onMount(() => {
    recargar(true);
    const aLaVista = () => document.visibilityState === 'visible';
    // Solo mientras la guía se ve: cada 10 s y al volver a la ventana (por ejemplo, después de instalar algo).
    const intervalo = setInterval(() => {
      if (aLaVista()) recargar(true);
    }, CADA);
    const alVolver = () => {
      if (aLaVista()) recargar(true);
    };
    document.addEventListener('visibilitychange', alVolver);
    window.addEventListener('focus', alVolver);
    return () => {
      clearInterval(intervalo);
      document.removeEventListener('visibilitychange', alVolver);
      window.removeEventListener('focus', alVolver);
    };
  });
</script>

<section class="guia vidrio" class:compacta aria-labelledby="{id}-titulo">
  <header class="cabeza">
    <span class="ficha-icono" class:tenue={!listo} style:--tono={listo ? 'var(--exito)' : undefined}>
      <Icono nombre={listo ? 'check' : 'conectar'} tamano={compacta ? 18 : 20} />
    </span>
    <div class="cabeza__textos">
      <h2 class="cabeza__titulo" id="{id}-titulo">{titulo}</h2>
      <p class="cabeza__sub">
        {listo ? exito : 'Cada paso se revisa solo mientras ves esta guía.'}
      </p>
    </div>
    {#if pasos.length && !listo}
      <span class="avance" aria-label="{hechos} de {pasos.length} pasos listos">{hechos} de {pasos.length}</span>
    {/if}
    {#if oncerrar}
      <button class="cerrar" type="button" aria-label="Cerrar la guía" onclick={oncerrar}>
        <Icono nombre="cerrar" tamano={15} />
      </button>
    {/if}
  </header>

  {#if !s}
    <ul class="pasos" aria-busy="true">
      {#each [0, 1, 2] as i (i)}<li class="paso silueta"><span class="marca"></span><span class="linea"></span></li>{/each}
    </ul>
  {:else}
    <ol class="pasos">
      {#each pasos as p, i (p.id)}
        {@const estado = p.hecho ? 'hecho' : p.id === actual ? 'actual' : 'pendiente'}
        {@const conBoton = !p.hecho && (p.id === actual || p.suelto)}
        <li class="paso {estado}" aria-current={estado === 'actual' ? 'step' : undefined}>
          <span class="marca" aria-hidden="true">
            {#if p.hecho}<Icono nombre="check" tamano={14} />{:else}{i + 1}{/if}
          </span>
          <div class="paso__textos">
            <p class="paso__titulo">
              {p.titulo}
              <span class="solo-lector">
                · {estado === 'hecho' ? 'Hecho' : estado === 'actual' ? 'Paso siguiente' : 'Pendiente'}
              </span>
            </p>
            <!-- Chica: lo hecho va en una línea, sin su frase. -->
            {#if !(compacta && p.hecho)}<p class="paso__frase">{p.frase}</p>{/if}
          </div>
          {#if conBoton}
            <button
              class={p.id === actual ? 'boton-primario boton' : 'boton-vidrio boton secundario'}
              type="button"
              disabled={ocupado !== null}
              aria-busy={ocupado === p.id}
              onclick={() => hacer(p)}
            >
              {ocupado === p.id ? ocupadoTexto(p.accion) : p.accion.texto}
              <Icono nombre={iconoDe(p.accion)} tamano={15} />
            </button>
          {/if}
        </li>
      {/each}
    </ol>

  {/if}
</section>

<style>
  /* La guía es su propio contenedor: los pasos se acomodan a su ancho, no al de la ventana. */
  .guia {
    container-type: inline-size;
    min-width: 0;
    padding: 18px 20px 8px;
    border-radius: var(--radio-lg);
  }
  .guia.compacta {
    padding: 14px 16px 4px;
  }

  .cabeza {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-bottom: 10px;
  }
  .cabeza__textos {
    flex: 1;
    min-width: 0;
  }
  .cabeza__titulo {
    margin: 0;
    font: var(--letra-panel);
    letter-spacing: -0.01em;
  }
  .cabeza__sub {
    margin: 2px 0 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
  }
  .avance {
    flex-shrink: 0;
    padding: 3px 10px;
    border-radius: var(--radio-pildora);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    font-size: 12px;
    font-weight: 600;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .pasos {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .paso {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 14px;
    min-height: 64px;
    padding: 12px 0;
    border-top: 1px solid var(--borde);
  }
  .compacta .paso {
    min-height: 56px;
    padding: 10px 0;
  }
  .compacta .paso.hecho {
    min-height: 44px;
    padding: 8px 0;
  }
  .compacta .hecho .marca {
    width: 24px;
    height: 24px;
    margin: 0 2px;
  }

  /* El número del paso, o el visto cuando está hecho. */
  .marca {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    font-size: 12.5px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    color: var(--texto-3);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .actual .marca {
    color: var(--elegido-fg);
    background: var(--elegido-bg);
    box-shadow: inset 0 0 0 1px var(--elegido-borde);
  }
  .hecho .marca {
    color: var(--exito);
    background: color-mix(in srgb, var(--exito) 13%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--exito) 25%, transparent);
  }

  /* El texto toma el resto; si el botón no cabe al lado, baja debajo del texto (alineado con él). */
  .paso__textos {
    flex: 1 1 220px;
    min-width: 0;
  }
  .paso__titulo {
    margin: 0;
    font-weight: 600;
    color: var(--texto);
  }
  .pendiente .paso__titulo {
    color: var(--texto-2);
  }
  .paso__frase {
    margin: 2px 0 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
    overflow-wrap: break-word;
  }
  .hecho .paso__frase {
    color: var(--texto-3);
  }

  .boton {
    flex-shrink: 0;
    min-height: 32px;
    padding: 0 14px;
  }
  .boton.secundario {
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .boton.secundario:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .boton:disabled {
    cursor: default;
    opacity: 0.6;
  }

  .cerrar {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    margin-right: -4px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--texto-3);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .cerrar:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }

  /* Angosta: el botón baja a su propia línea, debajo del texto. */
  @container (max-width: 460px) {
    .paso .boton {
      margin-left: 42px;
    }
  }

  /* Mientras llega la primera lectura: siluetas quietas. */
  .silueta .marca {
    color: transparent;
  }
  .silueta .linea {
    width: 55%;
    height: 12px;
    border-radius: 6px;
    background: var(--superficie-elevada);
  }
</style>
