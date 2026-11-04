<!--
  Panel lateral de Dispositivos (DISENO.md, sección 7.2). De arriba abajo, en este orden (pedido del dueño):
  1. foto grande con la aurora detrás y el botón "⋯" (Ver todos los ajustes · Ajustes de Bluetooth);
  2. nombre y estado (con la marca y el tipo al lado del estado);
  3. batería, en su grupo (si se sabe);
  4–5. volumen y micrófono en un grupo, y debajo salida y entrada de audio en otro (aparatos de sonido);
  6. abajo, fijas, las acciones como una barra nativa: Conectar o Desconectar · Renombrar · Olvidar, con el ícono
     encima del texto. Mientras conecta o desconecta, el primer botón queda quieto y tenue con su indicador girando.
  Lo de arriba se desplaza y las acciones quedan fijas. Conexión, Automático e Información están en la página
  del aparato (`onentrar`). Los diálogos de Renombrar y Olvidar los abre la vista.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { textoAbrirBluetooth } from '$lib/api-sistema';
  import type { Dispositivo } from '$lib/tipos';
  import { baterias, esDeSonido, estadoDe, nombreDe, nombreTipo } from '$lib/tipos';
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
  const meta = $derived([d.fabricante, nombreTipo[d.tipo]].filter(Boolean).join(' · '));

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

<article class="detalle vidrio" aria-labelledby="{id}-nombre">
  <div class="cuerpo">
    <!-- 1. Foto con la aurora y el menú "⋯" -->
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
            <Icono nombre="bluetooth" tamano={17} /> {textoAbrirBluetooth}
          </button>
        </div>
      {/if}
    </div>

    <!-- 2. Nombre y estado -->
    <div class="quien">
      <h2 id="{id}-nombre">{nombre}</h2>
      <div class="estado">
        <EstadoChip estado={estadoDe(d, pendiente)} />
        {#if meta}<span class="meta">{meta}</span>{/if}
      </div>
    </div>

    <!-- 3. Batería -->
    {#if listaBaterias.length === 1}
      {@const nivel = listaBaterias[0].nivel}
      <div class="bateria inserto">
        <Anillo {nivel} tamano={64} grosor={6} />
        <div class="bateria__textos">
          <div class="bateria__titulo">Batería</div>
          <div class="bateria__estado">
            <span class="bateria__icono" style:color={colorBateria(nivel)}><Icono nombre={iconoBateria(nivel)} tamano={15} /></span>
            {textoBateria(nivel)}
          </div>
        </div>
      </div>
    {:else if listaBaterias.length > 1}
      <div class="bateria varias inserto">
        <div class="bateria__titulo">Batería</div>
        <div class="anillos">
          {#each listaBaterias as b (b.nombre)}
            <div class="anillo-chico">
              <Anillo nivel={b.nivel} tamano={56} grosor={5} etiqueta={b.nombre} />
              <span>{b.nombre}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- 4–5. Volumen y micrófono · salida y entrada de audio -->
    {#if deSonido}
      <ControlesAudio dispositivo={d} {onerror} />
    {/if}
  </div>

  <!-- 6. Acciones, fijas abajo -->
  <div class="pie">
    {#if avisoOlvidar}
      <div class="aviso" role="status">
        <Icono nombre="info" tamano={18} />
        <div>
          <p>{avisoOlvidar}</p>
          <button class="enlace" onclick={ajustesBluetooth}>{textoAbrirBluetooth}</button>
        </div>
      </div>
    {/if}

    <div class="barra-acciones" role="group" aria-label="Acciones de {nombre}">
      {#if d.conectado}
        <button type="button" onclick={onalternar} disabled={pendiente} aria-busy={pendiente}>
          {#if pendiente}<span class="girador" aria-hidden="true"></span>{:else}<Icono nombre="desconectar" tamano={20} />{/if}
          <span>{pendiente ? 'Desconectando…' : 'Desconectar'}</span>
        </button>
      {:else}
        <button type="button" class="primaria" onclick={onalternar} disabled={pendiente} aria-busy={pendiente}>
          {#if pendiente}<span class="girador" aria-hidden="true"></span>{:else}<Icono nombre="conectar" tamano={20} />{/if}
          <span>{pendiente ? 'Conectando…' : 'Conectar'}</span>
        </button>
      {/if}
      <button type="button" onclick={onrenombrar}>
        <Icono nombre="editar" tamano={20} />
        <span>Renombrar</span>
      </button>
      <button type="button" class="peligro" onclick={onolvidar}>
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
    border-radius: var(--radio-lg);
  }

  .cuerpo {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px 18px 18px;
  }

  /* 1. Escenario: la foto sobre la aurora (turquesa y azul) y una sombra de apoyo, pintadas una vez. */
  .escenario {
    position: relative;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    height: 204px;
    margin: 0 -4px;
    border-radius: 18px;
    background:
      radial-gradient(30% 6% at 50% 91%, var(--sombra-apoyo), transparent 100%),
      radial-gradient(48% 46% at 50% 54%, var(--aurora-1), transparent 72%),
      radial-gradient(40% 44% at 30% 40%, var(--aurora-2), transparent 70%),
      radial-gradient(40% 44% at 72% 34%, var(--aurora-3), transparent 70%);
  }
  .figura {
    width: 184px;
    height: 184px;
    transition: opacity var(--dur-vista) ease;
  }
  .apagado .figura {
    opacity: 0.6;
  }

  .mas {
    position: absolute;
    top: 0;
    right: 0;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--relleno-tarjeta);
    box-shadow:
      inset 0 1px 0 var(--luz-tope),
      0 0 0 1px var(--borde-tarjeta),
      var(--sombra-tarjeta);
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
    top: 42px;
    right: 0;
    display: grid;
    min-width: 236px;
    max-width: 100%;
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

  /* 2. Nombre y estado */
  .quien {
    display: grid;
    gap: 8px;
    padding: 0 2px 2px;
  }
  h2 {
    margin: 0;
    overflow-wrap: anywhere;
    font-size: 22px;
    line-height: 28px;
    font-weight: 700;
    letter-spacing: -0.022em;
  }
  .estado {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
  }
  .meta {
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-3);
  }

  /* 3. Batería */
  .bateria {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px 12px 12px;
  }
  .bateria__textos {
    min-width: 0;
    display: grid;
    gap: 3px;
  }
  .bateria__titulo {
    font-size: 14px;
    line-height: 20px;
    font-weight: 650;
  }
  .bateria__estado {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .bateria__icono {
    display: grid;
    flex-shrink: 0;
  }
  .bateria.varias {
    display: grid;
    gap: 10px;
    padding: 12px 14px 14px;
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
    font-size: 12px;
    color: var(--texto-2);
  }

  /* 6. Pie fijo: aviso de Olvidar y la barra de acciones */
  .pie {
    flex-shrink: 0;
    display: grid;
    gap: 12px;
    padding: 12px 18px 18px;
    border-top: 1px solid var(--borde);
    background: var(--superficie-base);
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
