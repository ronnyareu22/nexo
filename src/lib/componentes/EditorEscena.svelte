<!--
  Editor de una escena, en una hoja a la derecha: nombre, ícono y los pasos en orden (subir, bajar, quitar),
  cada uno con su control simple: el aparato en un menú propio, el valor con un deslizador y el nombre de la app
  en un campo. Abajo se agregan pasos. Solo ofrece lo que Nexo controla en este equipo (el modo trabajo, solo en
  la Mac). Superficie sólida, sin desplazamiento de entrada: solo un fundido. Esc o un clic afuera cierran.
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { fade } from 'svelte/transition';
  import { listarDispositivosAudio } from '$lib/api';
  import type { DispositivoAudio } from '$lib/tipos';
  import type { Accion, Aparato, Escena } from '$lib/tipos-setup';
  import { ICONOS_ESCENA, PASOS, accionNueva, iconoEscena, nombreAparato, pasoDe, type IconoEscena } from '$lib/setup';
  import { tooltip } from '$lib/tooltip';
  import Icono, { type Nombre } from './Icono.svelte';
  import Deslizador from './Deslizador.svelte';
  import MenuDesplegable from './MenuDesplegable.svelte';
  import Dialogo from './Dialogo.svelte';

  let {
    abierto,
    escena,
    aparatos,
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
      listarDispositivosAudio()
        .then((l) => (salidas = l.filter((d) => d.salida)))
        .catch(() => (salidas = []));
    });
    tick().then(() => hoja?.querySelector<HTMLInputElement>('[data-autofocus]')?.focus());
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

  // --- Opciones de los menús ---
  const esDeSonido = (a: Aparato) => ['audifonos', 'diadema', 'parlante'].includes(a.tipo);
  const normal = (t: string) => t.toUpperCase().replaceAll('-', ':');

  const opcionesBluetooth = $derived(
    aparatos.map((a) => ({
      valor: a.id,
      nombre: nombreAparato(a),
      detalle: a.conectado ? 'Conectado' : 'Sin conectar',
    })),
  );

  // Para el sonido: los aparatos Bluetooth de sonido (aunque no estén conectados) y las demás salidas del sistema.
  const opcionesSalida = $derived.by(() => {
    const bt = aparatos.filter(esDeSonido);
    const lista = bt.map((a) => ({ valor: a.id, nombre: nombreAparato(a), detalle: 'Bluetooth' }));
    for (const s of salidas) {
      if (bt.some((a) => normal(s.uid).startsWith(normal(a.id)))) continue;
      lista.push({ valor: s.uid, nombre: s.nombre, detalle: s.salidaEnUso ? 'En uso' : 'Salida del sistema' });
    }
    return lista;
  });

  type Opcion = { valor: string; nombre: string; detalle?: string };
  /** Si el aparato guardado ya no está en este equipo, se muestra igual (con su nombre guardado). */
  function conGuardado(lista: Opcion[], a: Accion): Opcion[] {
    if (!('dispositivo' in a) || !a.dispositivo || lista.some((o) => o.valor === a.dispositivo)) return lista;
    return [...lista, { valor: a.dispositivo, nombre: a.nombre || 'Aparato guardado', detalle: 'No está aquí ahora' }];
  }

  // --- Pasos ---
  function agregar(tipo: (typeof PASOS)[number]['tipo']) {
    const primero =
      tipo === 'salida'
        ? opcionesSalida[0]
        : tipo === 'conectar' || tipo === 'desconectar'
          ? opcionesBluetooth[0]
          : undefined;
    acciones.push(accionNueva(tipo, primero ? { id: primero.valor, nombre: primero.nombre } : undefined));
    error = null;
  }

  function mover(i: number, paso: number) {
    const j = i + paso;
    if (j < 0 || j >= acciones.length) return;
    const [a] = acciones.splice(i, 1);
    acciones.splice(j, 0, a);
  }

  function elegirAparato(a: Accion, valor: string, lista: Opcion[]) {
    if (!('dispositivo' in a)) return;
    a.dispositivo = valor;
    a.nombre = lista.find((o) => o.valor === valor)?.nombre ?? null;
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

  const pasosDisponibles = $derived(PASOS.filter((p) => mac || !p.soloMac));
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
          <legend class="rotulo">Ícono</legend>
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
                <Icono nombre={i} tamano={18} />
              </button>
            {/each}
          </div>
        </fieldset>

        <section class="bloque" aria-labelledby="{id}-pasos">
          <h3 class="rotulo" id="{id}-pasos">Pasos, en orden</h3>
          {#if acciones.length}
            <ol class="pasos">
              {#each acciones as a, i (i)}
                {@const info = pasoDe(a.tipo)}
                <li class="paso inserto">
                  <div class="paso__cabeza">
                    <span class="ficha-icono chica" aria-hidden="true">
                      <Icono nombre={(info?.icono ?? 'rayo') as Nombre} tamano={16} />
                    </span>
                    <span class="paso__nombre">{i + 1}. {info?.nombre ?? 'Paso de una versión más nueva'}</span>
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

                  <div class="paso__control">
                    {#if a.tipo === 'brillo'}
                      {@render dosOpciones(
                        'Qué pantallas',
                        { valor: 'todas', nombre: 'Todas las pantallas' },
                        { valor: 'principal', nombre: 'La principal' },
                        a.pantalla,
                        (v) => (a.pantalla = v === 'principal' ? 'principal' : 'todas'),
                      )}
                      <div class="valor">
                        <Deslizador valor={a.valor} etiqueta="Brillo" color="amarillo" oncambio={(v) => (a.valor = v)} />
                        <span class="valor__numero">{a.valor} %</span>
                      </div>
                    {:else if a.tipo === 'conectar' || a.tipo === 'desconectar'}
                      {@const lista = conGuardado(opcionesBluetooth, a)}
                      {#if lista.length}
                        <MenuDesplegable
                          opciones={lista}
                          valor={a.dispositivo}
                          etiqueta={a.tipo === 'conectar' ? 'Qué aparato conectar' : 'Qué aparato desconectar'}
                          oncambio={(v) => elegirAparato(a, v, lista)}
                        />
                      {:else}
                        <p class="nota">No hay aparatos Bluetooth emparejados en este equipo.</p>
                      {/if}
                    {:else if a.tipo === 'salida'}
                      {@const lista = conGuardado(opcionesSalida, a)}
                      {#if lista.length}
                        <MenuDesplegable
                          opciones={lista}
                          valor={a.dispositivo}
                          etiqueta="Por dónde sale el sonido"
                          oncambio={(v) => elegirAparato(a, v, lista)}
                        />
                      {:else}
                        <p class="nota">No hay salidas de sonido en este equipo.</p>
                      {/if}
                    {:else if a.tipo === 'volumen' || a.tipo === 'volumenMicrofono'}
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
                    {:else if a.tipo === 'modoTrabajo'}
                      {@render dosOpciones(
                        'Modo trabajo',
                        { valor: 'si', nombre: 'Encender' },
                        { valor: 'no', nombre: 'Apagar' },
                        a.activo ? 'si' : 'no',
                        (v) => (a.activo = v === 'si'),
                      )}
                      <p class="nota">Con el modo trabajo, la Mac no se duerme.</p>
                    {:else if a.tipo === 'abrirApp'}
                      <input
                        class="texto"
                        type="text"
                        maxlength="200"
                        placeholder={mac ? 'Por ejemplo, Spotify' : 'Como aparece en el menú Inicio'}
                        aria-label="Nombre de la app"
                        bind:value={a.nombre}
                      />
                      <p class="nota">Solo abre apps; nada más.</p>
                    {:else if a.tipo === 'luces'}
                      <p class="nota">Disponible cuando conectes tus luces. Mientras tanto, este paso se salta.</p>
                    {:else}
                      <p class="nota">Este paso es de una versión más nueva de Nexo y se salta.</p>
                    {/if}
                  </div>
                </li>
              {/each}
            </ol>
          {:else}
            <p class="vacio">Todavía no hay pasos. Agrega el primero aquí abajo.</p>
          {/if}
        </section>

        <section class="bloque" aria-labelledby="{id}-agregar">
          <h3 class="rotulo" id="{id}-agregar">Agregar un paso</h3>
          <div class="agregar">
            {#each pasosDisponibles as p (p.tipo)}
              <button type="button" class="agregar__op" onclick={() => agregar(p.tipo)}>
                <Icono nombre={p.icono as Nombre} tamano={16} />
                {p.nombre}
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
    width: min(440px, 100%);
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
  .iconos__lista {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .icono-op {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radio-sm);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-2);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .icono-op:hover:not(.elegido) {
    background: var(--superficie-hover);
    color: var(--texto);
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
    overflow-wrap: break-word;
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
