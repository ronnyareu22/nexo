<!--
  Editor de una escena, en una hoja a la derecha: nombre, ilustración y los pasos en orden (subir, bajar, quitar).
  Cada paso se ve con la foto del aparato y lo que hace en palabras ("Conectar Sony WH-CH520", "Brillo al 70 %"),
  y su control simple (un deslizador, el nombre de la app, encender o apagar).
  Abajo, "Agregar dispositivo": todos los aparatos que Nexo conoce (los de este equipo, sus pantallas y salidas de
  sonido, y los del otro equipo), y al elegir uno, lo que se puede hacer con él. Lo del otro equipo se ve, pero no
  se elige: sus pasos solo corren en ese equipo. Luego, los pasos sin aparato (apps, modo trabajo, luces).
  Solo ofrece lo que Nexo controla en este equipo (el modo trabajo, solo en la Mac).
  Superficie sólida, sin desplazamiento de entrada: solo un fundido. Esc o un clic afuera cierran.
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { fade } from 'svelte/transition';
  import { listarDispositivosAudio } from '$lib/api';
  import { pruebaSetup } from '$lib/api-setup';
  import type { DispositivoAudio, Tipo } from '$lib/tipos';
  import type { Accion, Aparato, EquipoRemoto, Escena, PantallaSetup } from '$lib/tipos-setup';
  import {
    ICONOS_ESCENA,
    PASOS,
    accionNueva,
    brilloDe,
    comoDispositivo,
    iconoEscena,
    nombreAparato,
    pantallaDe,
    paso as pasoEnPalabras,
    pasoDe,
    type IconoEscena,
    type NombrePor,
  } from '$lib/setup';
  import { fotoDePantalla, fotoDeSonido, modeloDe } from '$lib/catalogo';
  import { tooltip } from '$lib/tooltip';
  import Icono, { type Nombre } from './Icono.svelte';
  import Ilustracion from './Ilustracion.svelte';
  import IlustracionEscena from './IlustracionEscena.svelte';
  import Deslizador from './Deslizador.svelte';
  import Dialogo from './Dialogo.svelte';

  let {
    abierto,
    escena,
    aparatos,
    pantallas = [],
    equipo = '',
    remotos = [],
    mac,
    onguardar,
    onborrar,
    oncerrar,
  }: {
    abierto: boolean;
    /** La escena a editar; null para una nueva. */
    escena: Escena | null;
    /** Los aparatos Bluetooth de este equipo. */
    aparatos: Aparato[];
    /** Las pantallas de este equipo. */
    pantallas?: PantallaSetup[];
    /** El nombre de este equipo en Tailscale ("demo-sige"). */
    equipo?: string;
    /** Los otros equipos: sus aparatos se muestran, pero sus pasos no corren desde aquí. */
    remotos?: EquipoRemoto[];
    /** El modo trabajo solo existe en la Mac. */
    mac: boolean;
    /** Devuelve el motivo si no se pudo guardar. */
    onguardar: (e: Escena) => Promise<string | null>;
    onborrar: (id: string) => Promise<string | null>;
    oncerrar: () => void;
  } = $props();

  const id = $props.id();
  const sinMovimiento =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const dur = sinMovimiento ? 0 : 140;

  const NOMBRES_ICONO: Record<IconoEscena, string> = {
    audio: 'Audífonos',
    remoto: 'Remoto',
    luna: 'Luna',
    camara: 'Cámara',
    microfono: 'Micrófono',
    sol: 'Sol',
    portatil: 'Portátil',
    pantallas: 'Pantalla',
    teclado: 'Teclado',
    rayo: 'Rayo',
    bombilla: 'Bombilla',
    reloj: 'Reloj',
    inicio: 'Casa',
    setup: 'Escritorio',
  };

  let nombre = $state('');
  let icono = $state<IconoEscena>('rayo');
  let acciones = $state<Accion[]>([]);
  let error = $state<string | null>(null);
  let guardando = $state(false);
  let preguntandoBorrar = $state(false);
  let salidas = $state<DispositivoAudio[]>([]);
  let hoja = $state<HTMLDivElement | null>(null);

  // Al abrir: copia la escena (o empieza una nueva), lee las salidas de sonido y pone el foco en el nombre.
  $effect(() => {
    if (!abierto) return;
    const previo = document.activeElement as HTMLElement | null;
    untrack(() => {
      nombre = escena?.nombre ?? '';
      icono = iconoEscena(escena?.icono ?? 'rayo');
      acciones = escena ? $state.snapshot(escena.acciones) : [];
      error = null;
      // `?setup=aparatos` (fuera de la app) abre la lista de dispositivos para revisarla.
      eligiendo = pruebaSetup === 'aparatos';
      elegido = null;
      listarDispositivosAudio()
        .then((l) => (salidas = l.filter((d) => d.salida)))
        .catch(() => (salidas = []));
    });
    tick().then(() => {
      hoja?.querySelector<HTMLInputElement>('[data-autofocus]')?.focus();
      if (eligiendo) setTimeout(() => hoja?.querySelector('.selector')?.scrollIntoView({ block: 'start' }), 250);
    });
    return () => {
      if (previo?.isConnected) previo.focus();
    };
  });

  /** Lleva la hoja al final de <body>, como el diálogo: nada la recorta. */
  function alCuerpo(nodo: HTMLElement) {
    document.body.appendChild(nodo);
    return { destroy: () => nodo.remove() };
  }

  const enfocables = () =>
    Array.from(
      hoja?.querySelectorAll<HTMLElement>(
        'button:not(:disabled), input:not(:disabled), [tabindex]:not([tabindex="-1"])',
      ) ?? [],
    );

  function teclas(e: KeyboardEvent) {
    // Un menú abierto ya usó el Esc para cerrarse.
    if (e.key === 'Escape' && !e.defaultPrevented && !preguntandoBorrar) {
      e.preventDefault();
      oncerrar();
      return;
    }
    if (e.key !== 'Tab') return;
    const lista = enfocables();
    if (!lista.length) return;
    const primero = lista[0];
    const ultimo = lista[lista.length - 1];
    if (e.shiftKey && document.activeElement === primero) {
      e.preventDefault();
      ultimo.focus();
    } else if (!e.shiftKey && document.activeElement === ultimo) {
      e.preventDefault();
      primero.focus();
    }
  }

  // --- Aparatos que Nexo conoce ---
  const esDeSonido = (t: Tipo) => t === 'audifonos' || t === 'diadema' || t === 'parlante';
  const normal = (t: string) => t.toUpperCase().replaceAll('-', ':');

  /** Algo que se puede elegir: un aparato Bluetooth, una salida de sonido del sistema o una pantalla. */
  type Clase = 'sonido' | 'bluetooth' | 'pantalla' | 'salida';
  interface Elegible {
    clave: string;
    /** La dirección Bluetooth, el identificador de la salida o el nombre de la pantalla. */
    id: string;
    nombre: string;
    clase: Clase;
    /** Para dibujarlo si no hay foto. */
    tipo: Tipo;
    foto: string | null;
    equipo: string;
    /** De este equipo: solo esos se pueden elegir. */
    local: boolean;
  }

  const deAparato = (a: Aparato, eq: string, local: boolean): Elegible => ({
    clave: `${eq}|bt|${a.id}`,
    id: a.id,
    nombre: nombreAparato(a),
    clase: esDeSonido(a.tipo) ? 'sonido' : 'bluetooth',
    tipo: a.tipo,
    foto: modeloDe(comoDispositivo(a))?.foto ?? null,
    equipo: eq,
    local,
  });

  // La foto de la integrada es la MacBook del dueño: solo vale para la de este equipo.
  const dePantalla = (p: PantallaSetup, eq: string, local: boolean): Elegible => ({
    clave: `${eq}|pantalla|${p.nombre}`,
    id: p.nombre,
    nombre: p.nombre,
    clase: 'pantalla',
    tipo: p.interna ? 'portatil' : 'monitor',
    foto: !local && p.interna ? null : fotoDePantalla({ ...p, controlable: true }),
    equipo: eq,
    local,
  });

  // Las salidas del sistema que no son un aparato Bluetooth (las bocinas, el monitor por HDMI…).
  const salidasSistema = $derived(salidas.filter((s) => !aparatos.some((a) => normal(s.uid).startsWith(normal(a.id)))));
  const deSalida = (s: DispositivoAudio): Elegible => ({
    clave: `${equipo}|salida|${s.uid}`,
    id: s.uid,
    nombre: s.nombre,
    clase: 'salida',
    tipo: 'parlante',
    foto: fotoDeSonido(s.nombre, s.transporte),
    equipo,
    local: true,
  });

  const ORDEN: Record<Clase, number> = { sonido: 0, bluetooth: 1, pantalla: 2, salida: 3 };
  const ordenar = (l: Elegible[]) => l.sort((x, y) => ORDEN[x.clase] - ORDEN[y.clase]);

  /** Por equipo: primero este, luego los otros (que se ven, pero no se eligen). */
  const grupos = $derived.by(() => {
    const lista = [
      {
        equipo,
        local: true,
        aparatos: ordenar([
          ...aparatos.map((a) => deAparato(a, equipo, true)),
          ...pantallas.map((p) => dePantalla(p, equipo, true)),
          ...salidasSistema.map(deSalida),
        ]),
      },
    ];
    for (const r of remotos) {
      if (!r.aparatos) continue;
      const items = ordenar([
        ...r.aparatos.dispositivos.map((a) => deAparato(a, r.nombre, false)),
        ...r.aparatos.pantallas.map((p) => dePantalla(p, r.nombre, false)),
      ]);
      if (items.length) lista.push({ equipo: r.nombre, local: false, aparatos: items });
    }
    return lista;
  });

  type Hacer = 'conectar' | 'desconectar' | 'salida' | 'volumen' | 'brillo';
  /** Lo que se puede hacer con cada clase de aparato, en el orden en que se suele armar. */
  const QUE_HACER: Record<Clase, { tipo: Hacer; nombre: string; icono: Nombre }[]> = {
    sonido: [
      { tipo: 'conectar', nombre: 'Conectar', icono: 'conectar' },
      { tipo: 'salida', nombre: 'Usar para el sonido', icono: 'audio' },
      { tipo: 'volumen', nombre: 'Volumen', icono: 'volumen' },
      { tipo: 'desconectar', nombre: 'Desconectar', icono: 'desconectar' },
    ],
    bluetooth: [
      { tipo: 'conectar', nombre: 'Conectar', icono: 'conectar' },
      { tipo: 'desconectar', nombre: 'Desconectar', icono: 'desconectar' },
    ],
    pantalla: [{ tipo: 'brillo', nombre: 'Brillo', icono: 'sol' }],
    salida: [
      { tipo: 'salida', nombre: 'Usar para el sonido', icono: 'audio' },
      { tipo: 'volumen', nombre: 'Volumen', icono: 'volumen' },
    ],
  };

  let eligiendo = $state(false);
  let elegido = $state<Elegible | null>(null);
  /** Los volúmenes agregados desde este aparato (el volumen no lleva aparato: se marca aquí). */
  let volumenAgregado = $state(false);

  function elegir(e: Elegible) {
    elegido = e;
    volumenAgregado = false;
  }

  /** Ya está en la escena (el mismo paso con el mismo aparato o pantalla). */
  function yaEsta(e: Elegible, tipo: Hacer): boolean {
    if (tipo === 'volumen') return volumenAgregado;
    if (tipo === 'brillo') return acciones.some((a) => pantallaDe(a) === e.nombre);
    return acciones.some((a) => 'dispositivo' in a && a.tipo === tipo && normal(a.dispositivo) === normal(e.id));
  }

  function agregarCon(e: Elegible, tipo: Hacer) {
    if (tipo === 'brillo') acciones.push({ tipo, pantalla: brilloDe(e.nombre), valor: 70 });
    else if (tipo === 'volumen') {
      acciones.push({ tipo, valor: 50 });
      volumenAgregado = true;
    } else acciones.push({ tipo, dispositivo: e.id, nombre: e.nombre });
    error = null;
  }

  // --- Pasos ---
  /** Los pasos sin aparato: los de siempre, menos conectar, desconectar y el sonido (van con su aparato). */
  const NOMBRE_OTRO: Partial<Record<Accion['tipo'], string>> = {
    brillo: 'Brillo de todas las pantallas',
    volumen: 'Volumen',
    volumenMicrofono: 'Volumen del micrófono',
    abrirApp: 'Abrir una app',
    modoTrabajo: 'Modo trabajo',
    luces: 'Luces (cuando las tengas)',
  };
  const otrosPasos = $derived(PASOS.filter((p) => p.tipo in NOMBRE_OTRO && (mac || !p.soloMac)));

  function agregar(tipo: (typeof PASOS)[number]['tipo']) {
    acciones.push(accionNueva(tipo));
    error = null;
  }

  function mover(i: number, paso: number) {
    const j = i + paso;
    if (j < 0 || j >= acciones.length) return;
    const [a] = acciones.splice(i, 1);
    acciones.splice(j, 0, a);
  }

  const aparatoLocal = (id: string) => aparatos.find((a) => normal(a.id) === normal(id));
  const salidaLocal = (id: string) => salidas.find((x) => x.uid === id);

  /** El nombre del aparato de un paso: el de ahora en este equipo o el que se guardó con el paso. */
  const nombrePor: NombrePor = (id, guardado) => {
    const a = aparatoLocal(id);
    if (a) return nombreAparato(a);
    return salidaLocal(id)?.nombre ?? (guardado || 'un aparato');
  };

  /** Cómo se ve un paso en la lista: qué hace en palabras, su foto (o su ícono) y si su aparato está aquí. */
  function verPaso(a: Accion): { titulo: string; foto: string | null; tipo: Tipo | null; icono: Nombre; aqui: boolean } {
    const icono = (pasoDe(a.tipo)?.icono ?? 'rayo') as Nombre;
    const titulo =
      a.tipo === 'abrirApp'
        ? a.nombre.trim()
          ? `Abrir ${a.nombre.trim()}`
          : 'Abrir una app'
        : a.tipo === 'luces'
          ? 'Luces, cuando las tengas'
          : a.tipo === 'desconocida'
            ? 'Un paso de una versión más nueva'
            : pasoEnPalabras(a, nombrePor);
    if ('dispositivo' in a) {
      const ap = aparatoLocal(a.dispositivo);
      if (ap) return { titulo, foto: modeloDe(comoDispositivo(ap))?.foto ?? null, tipo: ap.tipo, icono, aqui: true };
      const sal = a.tipo === 'salida' ? salidaLocal(a.dispositivo) : undefined;
      if (sal) return { titulo, foto: fotoDeSonido(sal.nombre, sal.transporte), tipo: 'parlante', icono, aqui: true };
      return { titulo, foto: null, tipo: null, icono, aqui: false };
    }
    const nombrePantalla = pantallaDe(a);
    if (nombrePantalla !== null) {
      const p = pantallas.find((x) => x.nombre === nombrePantalla);
      if (p) return { titulo, foto: fotoDePantalla({ ...p, controlable: true }), tipo: p.interna ? 'portatil' : 'monitor', icono, aqui: true };
      return { titulo, foto: null, tipo: null, icono, aqui: false };
    }
    return { titulo, foto: null, tipo: null, icono, aqui: true };
  }

  /** Lo mismo que revisa Rust, para avisar antes de mandar. */
  function revisar(): string | null {
    if (!nombre.trim()) return 'Ponle un nombre a la escena.';
    if (!acciones.length) return 'Agrega al menos un paso.';
    if (acciones.some((a) => 'dispositivo' in a && !a.dispositivo.trim())) return 'Elige el aparato de cada paso.';
    if (acciones.some((a) => a.tipo === 'abrirApp' && !a.nombre.trim()))
      return 'Escribe el nombre de la app que quieres abrir.';
    return null;
  }

  async function guardar() {
    if (guardando) return;
    error = revisar();
    if (error) return;
    guardando = true;
    error = await onguardar({
      id: escena?.id ?? '',
      nombre: nombre.trim(),
      icono,
      acciones: $state.snapshot(acciones),
    });
    guardando = false;
  }

  async function borrar() {
    preguntandoBorrar = false;
    if (!escena) return;
    error = await onborrar(escena.id);
  }

</script>

{#snippet dosOpciones(etiqueta: string, a: { valor: string; nombre: string }, b: { valor: string; nombre: string }, actual: string, elegir: (v: string) => void)}
  <div class="opciones vidrio" role="group" aria-label={etiqueta}>
    {#each [a, b] as o (o.valor)}
      <button type="button" class:elegido={actual === o.valor} aria-pressed={actual === o.valor} onclick={() => elegir(o.valor)}>
        {o.nombre}
      </button>
    {/each}
  </div>
{/snippet}

{#snippet foto(src: string | null, tipo: Tipo)}
  {#if src}<img {src} alt="" draggable="false" />{:else}<Ilustracion {tipo} />{/if}
{/snippet}

{#if abierto}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="capa"
    use:alCuerpo
    transition:fade={{ duration: dur }}
    onkeydown={teclas}
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) oncerrar();
    }}
  >
    <div bind:this={hoja} class="hoja flotante" role="dialog" aria-modal="true" aria-labelledby="{id}-titulo">
      <header class="hoja__cabeza">
        <h2 id="{id}-titulo">{escena ? 'Editar escena' : 'Nueva escena'}</h2>
        <button type="button" class="cerrar" aria-label="Cerrar" onclick={oncerrar}>
          <Icono nombre="cerrar" tamano={16} />
        </button>
      </header>

      <div class="hoja__cuerpo">
        <label class="campo">
          <span class="rotulo">Nombre</span>
          <input
            data-autofocus
            type="text"
            maxlength="40"
            placeholder="Por ejemplo, Modo cine"
            bind:value={nombre}
            onkeydown={(e) => {
              if (e.key === 'Enter') guardar();
            }}
          />
        </label>

        <fieldset class="iconos">
          <legend class="rotulo">Ilustración</legend>
          <div class="iconos__lista">
            {#each ICONOS_ESCENA as i (i)}
              <button
                type="button"
                class="icono-op"
                class:elegido={icono === i}
                aria-pressed={icono === i}
                aria-label={NOMBRES_ICONO[i]}
                use:tooltip={NOMBRES_ICONO[i]}
                onclick={() => (icono = i)}
              >
                <IlustracionEscena nombre={i} icono={i} recorte />
              </button>
            {/each}
          </div>
        </fieldset>

        <section class="bloque" aria-labelledby="{id}-pasos">
          <h3 class="rotulo" id="{id}-pasos">Pasos, en orden</h3>
          {#if acciones.length}
            <ol class="pasos">
              {#each acciones as a, i (i)}
                {@const ver = verPaso(a)}
                <li class="paso inserto">
                  <div class="paso__cabeza">
                    {#if ver.foto || ver.tipo}
                      <span class="foto-aparato chica" aria-hidden="true">{@render foto(ver.foto, ver.tipo ?? 'otro')}</span>
                    {:else}
                      <span class="ficha-icono chica" aria-hidden="true"><Icono nombre={ver.icono} tamano={16} /></span>
                    {/if}
                    <span class="paso__nombre"><span class="paso__numero">{i + 1}.</span> {ver.titulo}</span>
                    <span class="paso__botones">
                      <button type="button" aria-label="Subir el paso {i + 1}" use:tooltip={'Subir'} disabled={i === 0} onclick={() => mover(i, -1)}>
                        <Icono nombre="chevron-arriba" tamano={16} />
                      </button>
                      <button
                        type="button"
                        aria-label="Bajar el paso {i + 1}"
                        use:tooltip={'Bajar'}
                        disabled={i === acciones.length - 1}
                        onclick={() => mover(i, 1)}
                      >
                        <Icono nombre="chevron-abajo" tamano={16} />
                      </button>
                      <button type="button" class="quitar" aria-label="Quitar el paso {i + 1}" use:tooltip={'Quitar'} onclick={() => acciones.splice(i, 1)}>
                        <Icono nombre="papelera" tamano={16} />
                      </button>
                    </span>
                  </div>

                  {#if !ver.aqui}
                    <p class="nota aviso">No está en este equipo ahora: este paso se salta hasta que vuelva.</p>
                  {/if}
                  {#if a.tipo === 'brillo'}
                    <div class="paso__control">
                      {#if pantallaDe(a) === null}
                        {@render dosOpciones(
                          'Qué pantallas',
                          { valor: 'todas', nombre: 'Todas las pantallas' },
                          { valor: 'principal', nombre: 'La principal' },
                          a.pantalla,
                          (v) => (a.pantalla = v === 'principal' ? 'principal' : 'todas'),
                        )}
                      {/if}
                      <div class="valor">
                        <Deslizador valor={a.valor} etiqueta="Brillo" color="amarillo" oncambio={(v) => (a.valor = v)} />
                        <span class="valor__numero">{a.valor} %</span>
                      </div>
                    </div>
                  {:else if a.tipo === 'volumen' || a.tipo === 'volumenMicrofono'}
                    <div class="paso__control">
                      <div class="valor">
                        <Deslizador
                          valor={a.valor}
                          etiqueta={a.tipo === 'volumen' ? 'Volumen' : 'Volumen del micrófono'}
                          color="celeste"
                          oncambio={(v) => (a.valor = v)}
                        />
                        <span class="valor__numero">{a.valor} %</span>
                      </div>
                      <p class="nota">
                        {a.tipo === 'volumen' ? 'Se aplica a la salida de sonido en uso.' : 'Se aplica al micrófono en uso.'}
                      </p>
                    </div>
                  {:else if a.tipo === 'modoTrabajo'}
                    <div class="paso__control">
                      {@render dosOpciones(
                        'Modo trabajo',
                        { valor: 'si', nombre: 'Encender' },
                        { valor: 'no', nombre: 'Apagar' },
                        a.activo ? 'si' : 'no',
                        (v) => (a.activo = v === 'si'),
                      )}
                      <p class="nota">Con el modo trabajo, la Mac no se duerme.</p>
                    </div>
                  {:else if a.tipo === 'abrirApp'}
                    <div class="paso__control">
                      <input
                        class="texto"
                        type="text"
                        maxlength="200"
                        placeholder={mac ? 'Por ejemplo, Spotify' : 'Como aparece en el menú Inicio'}
                        aria-label="Nombre de la app"
                        bind:value={a.nombre}
                      />
                      <p class="nota">Solo abre apps; nada más.</p>
                    </div>
                  {:else if a.tipo === 'luces'}
                    <p class="nota">Disponible cuando conectes tus luces. Mientras tanto, este paso se salta.</p>
                  {:else if a.tipo === 'desconocida'}
                    <p class="nota">Este paso es de una versión más nueva de Nexo y se salta.</p>
                  {/if}
                </li>
              {/each}
            </ol>
          {:else}
            <p class="vacio">Todavía no hay pasos. Agrega un dispositivo o un paso aquí abajo.</p>
          {/if}
        </section>

        <section class="bloque" aria-labelledby="{id}-agregar">
          <h3 class="rotulo" id="{id}-agregar">Agregar un paso</h3>
          {#if !eligiendo}
            <button
              type="button"
              class="agregar-aparato"
              onclick={() => {
                eligiendo = true;
                elegido = null;
              }}
            >
              <span class="agregar-aparato__icono" aria-hidden="true"><Icono nombre="mas" tamano={18} /></span>
              <span class="agregar-aparato__textos">
                <span class="agregar-aparato__titulo">Agregar dispositivo</span>
                <span class="agregar-aparato__ayuda">Conectarlo, usarlo para el sonido, su volumen o el brillo de una pantalla</span>
              </span>
            </button>
          {:else}
            <div class="selector inserto">
              <div class="selector__cabeza">
                {#if elegido}
                  <button type="button" class="selector__atras" aria-label="Elegir otro dispositivo" use:tooltip={'Otro dispositivo'} onclick={() => (elegido = null)}>
                    <Icono nombre="atras" tamano={16} />
                  </button>
                {/if}
                <span class="selector__titulo">{elegido ? 'Qué hacer con él' : 'Elige un dispositivo'}</span>
                <button
                  type="button"
                  class="selector__listo"
                  onclick={() => {
                    eligiendo = false;
                    elegido = null;
                  }}
                >
                  Listo
                </button>
              </div>

              {#if elegido}
                {@const e = elegido}
                <div class="elegido">
                  <span class="foto-aparato" aria-hidden="true">{@render foto(e.foto, e.tipo)}</span>
                  <span class="elegido__textos">
                    <span class="elegido__nombre">{e.nombre}</span>
                    <span class="elegido__equipo">En {e.equipo || 'este equipo'}</span>
                  </span>
                </div>
                <div class="chips" role="group" aria-label="Qué hacer con {e.nombre}">
                  {#each QUE_HACER[e.clase] as h (h.tipo)}
                    {@const listo = yaEsta(e, h.tipo)}
                    <button type="button" class="chip" class:listo disabled={listo} onclick={() => agregarCon(e, h.tipo)}>
                      <Icono nombre={listo ? 'check' : h.icono} tamano={16} />
                      {h.nombre}
                      {#if listo}<span class="solo-lector">: ya está en la escena</span>{/if}
                    </button>
                  {/each}
                </div>
                <p class="nota">
                  {e.clase === 'pantalla'
                    ? 'El brillo se ajusta en su paso, arriba.'
                    : 'Cada toque agrega un paso al final; luego puedes cambiarles el orden.'}
                </p>
              {:else}
                {#each grupos as g (g.equipo)}
                  <div class="grupo-aparatos">
                    <p class="rotulo">{g.local ? `Este equipo${g.equipo ? ` · ${g.equipo}` : ''}` : g.equipo}</p>
                    {#if g.aparatos.length}
                      <div class="aparatos">
                        {#each g.aparatos as e (e.clave)}
                          <button type="button" class="aparato-op" disabled={!e.local} onclick={() => elegir(e)}>
                            <span class="foto-aparato" aria-hidden="true">{@render foto(e.foto, e.tipo)}</span>
                            <span class="aparato-op__nombre">{e.nombre}</span>
                            <span class="aparato-op__equipo">{e.local ? `En ${e.equipo || 'este equipo'}` : `Está en ${e.equipo}`}</span>
                          </button>
                        {/each}
                      </div>
                    {:else}
                      <p class="nota">Nexo todavía no ve aparatos en este equipo.</p>
                    {/if}
                    {#if !g.local}
                      <p class="nota">Sus pasos solo corren en ese equipo: para usarlos, arma la escena allá.</p>
                    {/if}
                  </div>
                {/each}
              {/if}
            </div>
          {/if}

          <p class="rotulo otros">Otros pasos</p>
          <div class="agregar">
            {#each otrosPasos as p (p.tipo)}
              <button type="button" class="agregar__op" onclick={() => agregar(p.tipo)}>
                <Icono nombre={p.icono as Nombre} tamano={16} />
                {NOMBRE_OTRO[p.tipo] ?? p.nombre}
              </button>
            {/each}
          </div>
        </section>
      </div>

      {#if error}
        <p class="error" role="alert"><Icono nombre="alerta" tamano={16} />{error}</p>
      {/if}

      <footer class="hoja__pie">
        {#if escena}
          <button type="button" class="boton peligro" onclick={() => (preguntandoBorrar = true)}>
            <Icono nombre="papelera" tamano={16} /> Borrar
          </button>
        {/if}
        <span class="hueco"></span>
        <button type="button" class="boton secundario" onclick={oncerrar}>Cancelar</button>
        <button type="button" class="boton-primario" disabled={guardando} aria-busy={guardando} onclick={guardar}>
          {guardando ? 'Guardando…' : 'Guardar'}
        </button>
      </footer>
    </div>
  </div>
{/if}

<Dialogo
  abierto={preguntandoBorrar}
  titulo={`¿Borrar «${escena?.nombre ?? ''}»?`}
  descripcion="La escena se borra de este equipo. Tus aparatos y ajustes no cambian."
  icono="papelera"
  tono="peligro"
  textoConfirmar="Borrar"
  onconfirmar={borrar}
  oncancelar={() => (preguntandoBorrar = false)}
/>

<style>
  .capa {
    position: fixed;
    inset: 0;
    z-index: 45;
    display: flex;
    justify-content: flex-end;
    padding: 12px;
    background: rgb(4 8 14 / 0.4);
  }
  .hoja {
    display: flex;
    flex-direction: column;
    width: min(480px, 100%);
    max-height: 100%;
    border-radius: var(--radio-lg);
    overflow: hidden;
  }
  .hoja__cabeza {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 18px 18px 12px 22px;
  }
  .hoja__cabeza h2 {
    flex: 1;
    margin: 0;
    font: var(--letra-seccion);
    letter-spacing: -0.01em;
  }
  .cerrar {
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
  .cerrar:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }

  .hoja__cuerpo {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 4px 22px 18px;
  }

  .campo {
    display: grid;
    gap: 6px;
  }
  input[type='text'] {
    width: 100%;
    min-height: 38px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--radio-sm);
    background: var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font: inherit;
    font-size: 14px;
    user-select: text;
    -webkit-user-select: text;
  }
  input[type='text']:focus-visible {
    outline: none;
    box-shadow: var(--foco);
  }
  input::placeholder {
    color: var(--texto-3);
  }

  .iconos {
    margin: 0;
    padding: 0;
    border: 0;
    min-width: 0;
  }
  .iconos legend {
    padding: 0;
    margin-bottom: 8px;
  }
  /* Las 14 ilustraciones en dos filas parejas de 7 miniaturas cuadradas. */
  .iconos__lista {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 6px;
  }
  .icono-op {
    display: block;
    aspect-ratio: 1;
    padding: 2px;
    border: 1.5px solid transparent;
    border-radius: var(--radio-sm);
    background: color-mix(in srgb, var(--primario) 5%, var(--superficie-inserto));
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-2);
    overflow: hidden;
    transition:
      background-color var(--dur-color) ease,
      border-color var(--dur-color) ease;
  }
  .icono-op :global(.ilus) {
    border-radius: 9px;
    overflow: hidden;
  }
  .icono-op:hover:not(.elegido) {
    background: var(--superficie-hover);
  }
  .icono-op:active {
    transform: scale(0.98);
  }
  /* Lo elegido (DISENO.md 4.1): relleno turquesa suave, borde fino y el ícono en turquesa. */
  .icono-op.elegido,
  .opciones button.elegido {
    background: var(--elegido-bg);
    color: var(--elegido-fg);
    border-color: var(--elegido-borde);
    box-shadow: none;
  }

  .bloque {
    display: grid;
    gap: 8px;
  }
  .bloque h3 {
    margin: 0;
  }
  .pasos {
    display: grid;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .paso {
    display: grid;
    gap: 10px;
    padding: 10px 10px 12px 12px;
  }
  .paso__cabeza {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .paso__nombre {
    flex: 1;
    min-width: 0;
    font: var(--letra-panel);
    font-size: 14px;
    line-height: 19px;
    overflow-wrap: break-word;
  }
  .paso__numero {
    color: var(--texto-3);
    font-variant-numeric: tabular-nums;
  }

  /* La foto real del aparato (o su dibujo) en un cuadro claro. */
  .foto-aparato {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 52px;
    height: 52px;
    padding: 4px;
    border-radius: 12px;
    /* Un poco más claro que la superficie: las fotos de aparatos oscuros se leen también en oscuro. */
    background: color-mix(in srgb, #fff 9%, var(--superficie-elevada));
  }
  .foto-aparato.chica {
    width: 36px;
    height: 36px;
    padding: 3px;
    border-radius: 10px;
  }
  .foto-aparato img,
  .foto-aparato :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .paso__botones {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }
  .paso__botones button {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: var(--texto-2);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .paso__botones button:hover:not(:disabled) {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .paso__botones button:disabled {
    color: var(--texto-3);
    opacity: 0.5;
    cursor: default;
  }
  .paso__botones .quitar:hover:not(:disabled) {
    color: var(--error);
  }
  .paso__control {
    display: grid;
    gap: 8px;
  }
  .valor {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .valor :global(input[type='range']) {
    flex: 1;
  }
  .valor__numero {
    min-width: 44px;
    text-align: right;
    font-size: 13px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .nota,
  .vacio {
    margin: 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
    overflow-wrap: break-word;
  }
  .nota.aviso {
    color: var(--aviso);
  }

  /* "Agregar dispositivo": la entrada principal para sumar pasos. */
  .agregar-aparato {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 12px 14px;
    border: 1.5px dashed color-mix(in srgb, var(--primario) 45%, var(--borde));
    border-radius: var(--radio-md);
    background: color-mix(in srgb, var(--primario) 6%, var(--superficie));
    color: var(--texto);
    text-align: left;
    transition: background-color var(--dur-color) ease;
  }
  .agregar-aparato:hover {
    background: color-mix(in srgb, var(--primario) 12%, var(--superficie));
  }
  .agregar-aparato:active {
    transform: scale(0.98);
  }
  .agregar-aparato__icono {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 36px;
    height: 36px;
    border-radius: 11px;
    background: var(--elegido-bg);
    color: var(--primario-texto);
  }
  .agregar-aparato__textos {
    display: grid;
    min-width: 0;
  }
  .agregar-aparato__titulo {
    font-size: 14px;
    line-height: 20px;
    font-weight: 650;
  }
  .agregar-aparato__ayuda {
    font: var(--letra-pequena);
    color: var(--texto-2);
  }

  .selector {
    display: grid;
    gap: 12px;
    padding: 10px 12px 14px;
  }
  .selector__cabeza {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .selector__titulo {
    flex: 1;
    min-width: 0;
    font-size: 14px;
    font-weight: 650;
  }
  .selector__atras {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--texto-2);
    transition: background-color var(--dur-color) ease;
  }
  .selector__atras:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .selector__listo {
    min-height: 30px;
    padding: 0 12px;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: var(--primario-texto);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .selector__listo:hover {
    background: var(--superficie-hover);
  }
  .grupo-aparatos {
    display: grid;
    gap: 8px;
  }
  .grupo-aparatos .rotulo {
    margin: 0;
  }
  .aparatos {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
    gap: 8px;
  }
  .aparato-op {
    display: grid;
    justify-items: center;
    align-content: start;
    gap: 4px;
    min-width: 0;
    padding: 10px 8px 10px;
    border: 0;
    border-radius: var(--radio-sm);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    text-align: center;
    overflow-wrap: break-word;
    transition: background-color var(--dur-color) ease;
  }
  .aparato-op:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .aparato-op:active:not(:disabled) {
    transform: scale(0.98);
  }
  .aparato-op .foto-aparato {
    width: 64px;
    height: 64px;
    margin-bottom: 2px;
  }
  .aparato-op__nombre {
    max-width: 100%;
    font-size: 13px;
    line-height: 17px;
    font-weight: 600;
  }
  .aparato-op__equipo {
    max-width: 100%;
    font-size: 11.5px;
    line-height: 15px;
    color: var(--texto-3);
  }
  /* Lo del otro equipo: a la vista, pero no se elige. */
  .aparato-op:disabled {
    cursor: default;
    background: transparent;
  }
  .aparato-op:disabled .foto-aparato {
    opacity: 0.55;
  }
  .aparato-op:disabled .aparato-op__nombre {
    color: var(--texto-2);
  }

  .elegido {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .elegido__textos {
    display: grid;
    min-width: 0;
  }
  .elegido__nombre {
    font-size: 15px;
    line-height: 21px;
    font-weight: 650;
    overflow-wrap: break-word;
  }
  .elegido__equipo {
    font: var(--letra-pequena);
    color: var(--texto-3);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 34px;
    padding: 4px 14px 4px 11px;
    border: 1px solid transparent;
    border-radius: var(--radio-pildora);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde-fuerte);
    color: var(--texto);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .chip :global(svg) {
    color: var(--primario-texto);
  }
  .chip:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .chip:active:not(:disabled) {
    transform: scale(0.98);
  }
  /* Ya está en la escena: lo elegido, quieto. */
  .chip.listo {
    background: var(--elegido-bg);
    border-color: var(--elegido-borde);
    box-shadow: none;
    color: var(--elegido-fg);
    cursor: default;
  }
  .rotulo.otros {
    margin: 6px 0 0;
  }

  .opciones {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 3px;
    border-radius: var(--radio-sm);
  }
  .opciones button {
    flex: 1 1 auto;
    min-height: 30px;
    padding: 4px 12px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: transparent;
    color: var(--texto-2);
    font-size: 13px;
    font-weight: 600;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .opciones button:hover:not(.elegido) {
    background: var(--superficie-hover);
    color: var(--texto);
  }

  .agregar {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .agregar__op {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding: 4px 12px 4px 10px;
    border: 0;
    border-radius: var(--radio-pildora);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .agregar__op:hover {
    background: var(--superficie-hover);
  }
  .agregar__op :global(svg) {
    color: var(--primario-texto);
  }

  .error {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0 22px 4px;
    font-size: 13px;
    line-height: 19px;
    color: var(--error);
    overflow-wrap: break-word;
  }
  .error :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
  }

  .hoja__pie {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 14px 22px 18px;
    border-top: 1px solid var(--borde);
  }
  .hueco {
    flex: 1;
  }
  .boton {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
    padding: 0 14px;
    border: 0;
    border-radius: var(--radio-sm);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .boton:active {
    transform: scale(0.98);
  }
  .secundario {
    background: var(--superficie-hundida);
    color: var(--texto);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .secundario:hover {
    background: var(--superficie-hover);
  }
  .peligro {
    background: transparent;
    color: var(--error);
  }
  .peligro:hover {
    background: color-mix(in srgb, var(--error) 10%, transparent);
  }
</style>
