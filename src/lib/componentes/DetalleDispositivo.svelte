<!--
  Panel lateral de Dispositivos (DISENO.md, sección 7.2). De arriba abajo, en este orden:
  1. foto grande con un resplandor quieto detrás y el botón "⋯" (Ver todos los ajustes · Ajustes de Bluetooth);
  2. nombre; 3. estado; 4. una línea; 5. batería (si se sabe); 6–7. volumen y micrófono (sonido conectado);
  8. salida y entrada de audio (aparatos de sonido); 9. acciones: Conectar o Desconectar · Renombrar · Olvidar.
  Las acciones quedan fijas abajo y lo de arriba se desplaza. Conexión, Automático e Información están en la página
  del aparato (`onentrar`). Los diálogos de Renombrar y Olvidar los abre la vista.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import type { Dispositivo } from '$lib/tipos';
  import { baterias, esDeSonido, estadoDe, nombreDe } from '$lib/tipos';
  import { abrirAjustesBluetooth } from '$lib/api';
  import { tooltip } from '$lib/tooltip';
  import Figura from './Figura.svelte';
  import EstadoChip from './EstadoChip.svelte';
  import Anillo from './Anillo.svelte';
  import ControlesAudio from './ControlesAudio.svelte';
  import { colorBateria, iconoBateria, textoBateria } from './PanelBateria.svelte';
  import Icono from './Icono.svelte';

  let {
    dispositivo: d,
    pendiente,
    avisoOlvidar = null,
    onalternar,
    onrenombrar,
    onolvidar,
    onentrar,
    onerror,
  }: {
    dispositivo: Dispositivo;
    pendiente: boolean;
    /** Si no se pudo olvidar desde Nexo: el motivo, con la salida a Ajustes › Bluetooth. */
    avisoOlvidar?: string | null;
    onalternar: () => void;
    onrenombrar: () => void;
    onolvidar: () => void;
    /** Abre la página con todos los ajustes del aparato. */
    onentrar: () => void;
    onerror: (texto: string) => void;
  } = $props();

  const id = $props.id();
  const nombre = $derived(nombreDe(d));
  const listaBaterias = $derived(baterias(d));
  const deSonido = $derived(esDeSonido(d));

  const ajustesBluetooth = () => abrirAjustesBluetooth().catch((e) => onerror(String(e)));

  // --- Menú "⋯": se cierra con Escape, con un clic afuera o al salir con Tab ---
  let menuAbierto = $state(false);
  let botonMenu = $state<HTMLButtonElement>();
  let menu = $state<HTMLDivElement>();

  const opcionesMenu = () => Array.from(menu?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []);

  async function abrirMenu() {
    menuAbierto = true;
    await tick();
    opcionesMenu()[0]?.focus();
  }

  function cerrarMenu(volverAlBoton = false) {
    if (!menuAbierto) return;
    menuAbierto = false;
    if (volverAlBoton) botonMenu?.focus();
  }

  function teclasMenu(e: KeyboardEvent) {
    const items = opcionesMenu();
    const i = items.indexOf(document.activeElement as HTMLElement);
    let destino = -1;
    switch (e.key) {
      case 'Escape':
        e.preventDefault();
        e.stopPropagation();
        cerrarMenu(true);
        return;
      case 'ArrowDown':
        destino = (i + 1) % items.length;
        break;
      case 'ArrowUp':
        destino = (i - 1 + items.length) % items.length;
        break;
      case 'Home':
        destino = 0;
        break;
      case 'End':
        destino = items.length - 1;
        break;
      case 'Tab':
        cerrarMenu();
        return;
      default:
        return;
    }
    e.preventDefault();
    items[destino]?.focus();
  }

  $effect(() => {
    if (!menuAbierto) return;
    const clicAfuera = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!menu?.contains(t) && !botonMenu?.contains(t)) cerrarMenu();
    };
    document.addEventListener('pointerdown', clicAfuera, true);
    return () => document.removeEventListener('pointerdown', clicAfuera, true);
  });

  function elegirDelMenu(accion: () => void) {
    cerrarMenu(true);
    accion();
  }
</script>

<article class="detalle" aria-labelledby="{id}-nombre">
  <div class="cuerpo">
    <!-- 1. Foto con resplandor y el menú "⋯" -->
    <div class="escenario" class:apagado={!d.conectado}>
      <div class="figura"><Figura dispositivo={d} sombra={false} /></div>

      <button
        bind:this={botonMenu}
        class="mas"
        type="button"
        aria-label="Más opciones"
        aria-haspopup="menu"
        aria-expanded={menuAbierto}
        aria-controls={menuAbierto ? `${id}-menu` : undefined}
        use:tooltip={menuAbierto ? undefined : 'Más opciones'}
        onclick={() => (menuAbierto ? cerrarMenu() : abrirMenu())}
      >
        <Icono nombre="opciones" tamano={18} />
      </button>

      {#if menuAbierto}
        <div
          bind:this={menu}
          id="{id}-menu"
          class="menu flotante"
          role="menu"
          tabindex="-1"
          aria-label="Más opciones de {nombre}"
          onkeydown={teclasMenu}
          onfocusout={(e) => {
            const destino = e.relatedTarget as Node | null;
            if (destino && !menu?.contains(destino) && destino !== botonMenu) cerrarMenu();
          }}
        >
          <button class="menu__item" type="button" role="menuitem" tabindex="-1" onclick={() => elegirDelMenu(onentrar)}>
            <Icono nombre="ajustes" tamano={17} /> Ver todos los ajustes
          </button>
          <button class="menu__item" type="button" role="menuitem" tabindex="-1" onclick={() => elegirDelMenu(ajustesBluetooth)}>
            <Icono nombre="bluetooth" tamano={17} /> Abrir Ajustes de Bluetooth
          </button>
        </div>
      {/if}
    </div>

    <!-- 2. Nombre · 3. Estado -->
    <h2 id="{id}-nombre">{nombre}</h2>
    <div class="estado"><EstadoChip estado={estadoDe(d, pendiente)} /></div>

    <!-- 4. Línea (solo si hay algo debajo) -->
    {#if listaBaterias.length || deSonido}<hr />{/if}

    <!-- 5. Batería -->
    {#if listaBaterias.length === 1}
      {@const nivel = listaBaterias[0].nivel}
      <div class="bateria">
        <Anillo {nivel} tamano={84} grosor={7} />
        <div class="bateria__textos">
          <div class="bateria__titulo">Batería</div>
          <div class="bateria__estado">
            <span class="bateria__icono" style:color={colorBateria(nivel)}><Icono nombre={iconoBateria(nivel)} tamano={16} /></span>
            {textoBateria(nivel)}
          </div>
        </div>
      </div>
    {:else if listaBaterias.length > 1}
      <div class="bateria varias">
        <div class="bateria__titulo">Batería</div>
        <div class="anillos">
          {#each listaBaterias as b (b.nombre)}
            <div class="anillo-chico">
              <Anillo nivel={b.nivel} tamano={60} grosor={5} etiqueta={b.nombre} />
              <span>{b.nombre}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- 6–7. Volumen y micrófono · 8. Salida y entrada de audio -->
    {#if deSonido}
      <ControlesAudio dispositivo={d} {onerror} />
    {/if}
  </div>

  <!-- 9. Acciones, fijas abajo -->
  <div class="pie">
    {#if avisoOlvidar}
      <div class="aviso" role="status">
        <Icono nombre="info" tamano={18} />
        <div>
          <p>{avisoOlvidar}</p>
          <button class="enlace" onclick={ajustesBluetooth}>Abrir Ajustes de Bluetooth</button>
        </div>
      </div>
    {/if}

    <div class="acciones">
      {#if d.conectado}
        <button class="accion" onclick={onalternar} disabled={pendiente} aria-busy={pendiente}>
          <Icono nombre="desconectar" tamano={20} />
          <span>Desconectar</span>
        </button>
      {:else}
        <button class="accion primaria" onclick={onalternar} disabled={pendiente} aria-busy={pendiente}>
          <Icono nombre="conectar" tamano={20} />
          <span>Conectar</span>
        </button>
      {/if}
      <button class="accion" onclick={onrenombrar}>
        <Icono nombre="editar" tamano={20} />
        <span>Renombrar</span>
      </button>
      <button class="accion olvidar" onclick={onolvidar}>
        <Icono nombre="papelera" tamano={20} />
        <span>Olvidar</span>
      </button>
    </div>
  </div>
</article>

<style>
  .detalle {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    border: 1px solid var(--borde);
    border-radius: var(--radio-lg);
    background: var(--superficie);
  }

  .cuerpo {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 14px 20px 20px;
  }

  /* 1. Escenario: la foto sobre un resplandor turquesa y azul, pintado una vez (sin filtros ni animación). */
  .escenario {
    position: relative;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    height: 212px;
    background:
      radial-gradient(50% 48% at 50% 56%, var(--aurora-1), transparent 72%),
      radial-gradient(42% 46% at 32% 40%, var(--aurora-2), transparent 70%),
      radial-gradient(40% 44% at 70% 36%, var(--aurora-3), transparent 70%);
  }
  .figura {
    width: 196px;
    height: 196px;
    transition: opacity var(--dur-vista) ease;
  }
  .apagado .figura {
    opacity: 0.6;
  }

  .mas {
    position: absolute;
    top: 2px;
    right: -6px;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
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
  .mas:hover,
  .mas[aria-expanded='true'] {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .mas:active {
    transform: scale(0.98);
  }

  .menu {
    position: absolute;
    z-index: 5;
    top: 44px;
    right: -6px;
    display: grid;
    min-width: 236px;
    max-width: calc(100% + 6px);
    padding: 4px;
    border-radius: var(--radio-sm);
  }
  .menu:focus {
    outline: none;
  }
  .menu__item {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 36px;
    padding: 7px 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--texto);
    font-size: 13.5px;
    line-height: 19px;
    text-align: left;
    transition: background-color var(--dur-color) ease;
  }
  .menu__item :global(svg) {
    flex-shrink: 0;
    color: var(--texto-2);
  }
  .menu__item:hover,
  .menu__item:focus {
    background: var(--superficie-hover);
  }

  /* 2–4. Nombre, estado y línea */
  h2 {
    margin: 2px 0 -8px;
    overflow-wrap: anywhere;
    font-size: 22px;
    line-height: 28px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .estado {
    display: flex;
  }
  hr {
    width: 100%;
    margin: 0;
    border: 0;
    border-top: 1px solid var(--borde);
  }

  /* 5. Batería */
  .bateria {
    display: flex;
    align-items: center;
    gap: 18px;
  }
  .bateria__textos {
    min-width: 0;
    display: grid;
    gap: 4px;
  }
  .bateria__titulo {
    font-size: 15px;
    line-height: 22px;
    font-weight: 700;
  }
  .bateria__estado {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto-2);
  }
  .bateria__icono {
    display: grid;
    flex-shrink: 0;
  }
  .bateria.varias {
    display: grid;
    gap: 10px;
  }
  .anillos {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
  }
  .anillo-chico {
    display: grid;
    justify-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--texto-2);
  }

  /* 9. Pie fijo: aviso de Olvidar y las tres acciones (ícono sobre el texto) */
  .pie {
    flex-shrink: 0;
    display: grid;
    gap: 12px;
    padding: 14px 20px 18px;
    border-top: 1px solid var(--borde);
  }
  .acciones {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }
  .accion {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-width: 0;
    min-height: 66px;
    padding: 10px 6px;
    border: 0;
    border-radius: 14px;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font-size: 12.5px;
    line-height: 17px;
    font-weight: 600;
    text-align: center;
    overflow-wrap: anywhere;
    transition: background-color var(--dur-color) ease;
  }
  .accion :global(svg) {
    flex-shrink: 0;
  }
  .accion:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .accion:active:not(:disabled) {
    transform: scale(0.98);
  }
  .accion:disabled {
    opacity: 0.6;
    cursor: progress;
  }
  .accion.primaria {
    background: var(--primario);
    color: var(--sobre-primario);
    box-shadow: none;
  }
  .accion.primaria:hover:not(:disabled) {
    background: var(--primario-hover);
  }
  .accion.olvidar {
    color: var(--error);
  }

  .aviso {
    display: flex;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radio-md);
    background: color-mix(in srgb, var(--aviso) 10%, var(--superficie));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--aviso) 30%, transparent);
    color: var(--aviso);
  }
  .aviso :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
  }
  .aviso p {
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto);
  }
  .enlace {
    margin-top: 4px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--primario-texto);
    font-size: 12.5px;
    font-weight: 600;
  }
  .enlace:hover {
    text-decoration: underline;
  }
</style>
