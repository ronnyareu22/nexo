<!-- Barra lateral compacta: la marca (la N de cinta de la guía de marca y "Nexo"), las secciones con ícono y nombre
     (la elegida con el estilo de "lo elegido" y la rayita con el degradado de marca) y, abajo, una tarjeta con el
     estado general sobre una aurora suave y el lema "Conecta. Controla. Trabaja mejor.". Con la ventana angosta (menos de 900 px)
     queda solo con íconos: los nombres y el estado pasan al tooltip y siguen ahí para los lectores de pantalla.
     Arriba deja 52 px libres para arrastrar la ventana y para los botones de macOS. En Windows la ventana
     tiene su barra de título, así que deja solo 8 px y la marca queda a la altura del buscador. -->
<script lang="ts" module>
  /** Estado general al pie: "Bluetooth activo" / "Todo en perfecto estado", o el problema. */
  export interface EstadoGeneral {
    titulo: string;
    detalle: string;
    tono: 'ok' | 'aviso' | 'error';
  }
</script>

<script lang="ts">
  import { MediaQuery } from 'svelte/reactivity';
  import { tooltip } from '$lib/tooltip';
  import Icono, { type Nombre } from './Icono.svelte';
  import LogoNexo from './LogoNexo.svelte';

  let {
    secciones,
    actual,
    onelegir,
    estado,
  }: {
    secciones: { id: string; nombre: string; icono: Nombre }[];
    actual: string;
    onelegir: (id: string) => void;
    estado?: EstadoGeneral;
  } = $props();

  // Debe coincidir con la media query de abajo.
  const plegada = new MediaQuery('max-width: 899.98px');
</script>

<nav class="lateral" aria-label="Secciones">
  <div class="arrastre" data-tauri-drag-region></div>
  <div class="marca" data-tauri-drag-region>
    <span class="logo" aria-hidden="true"><LogoNexo tamano={30} etiqueta="" /></span>
    <span class="marca__nombre">Nexo</span>
  </div>

  <ul class="nav">
    {#each secciones as s (s.id)}
      <li>
        <button
          class="nav__op"
          class:elegido={s.id === actual}
          aria-current={s.id === actual ? 'page' : undefined}
          use:tooltip={plegada.current ? s.nombre : ''}
          onclick={() => onelegir(s.id)}
        >
          <Icono nombre={s.icono} tamano={20} />
          <span class="nav__nombre">{s.nombre}</span>
        </button>
      </li>
    {/each}
  </ul>

  {#if estado}
    <div class="pie">
      <div
        class="estado {estado.tono}"
        role="status"
        use:tooltip={plegada.current ? `${estado.titulo} · ${estado.detalle}` : ''}
      >
        <span class="punto" aria-hidden="true"></span>
        <span class="estado__titulo">{estado.titulo}</span>
        <span class="estado__onda" aria-hidden="true"><Icono nombre="onda" tamano={18} /></span>
        <span class="estado__detalle">{estado.detalle}</span>
      </div>
      <p class="lema">Conecta. Controla. Trabaja mejor.</p>
    </div>
  {/if}
</nav>

<style>
  .lateral {
    position: relative;
    isolation: isolate;
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    width: 220px;
    height: 100%;
    min-height: 0;
    /* Un poco más clara arriba, como las tarjetas. Sin línea al borde: la separa el cambio de fondo. */
    background: linear-gradient(180deg, var(--superficie-tope), var(--superficie) 40%, var(--superficie-base));
  }
  /* Aurora al pie, detrás de la tarjeta de estado: lomas de luz azul y turquesa que suben del borde.
     Solo degradados pintados una vez: sin desenfoque y sin animación. */
  .lateral::after {
    content: '';
    position: absolute;
    z-index: -1;
    left: 0;
    right: 0;
    bottom: 0;
    height: 260px;
    pointer-events: none;
    background:
      radial-gradient(ellipse 150px 150px at 22% 100%, var(--aurora-2), transparent),
      radial-gradient(ellipse 130px 120px at 82% 100%, var(--aurora-1), transparent),
      radial-gradient(
        ellipse 110px 210px at 50% 100%,
        color-mix(in srgb, var(--aurora-3) 70%, transparent),
        transparent
      );
  }

  .arrastre {
    height: 52px;
    flex-shrink: 0;
  }
  /* Windows: sin los botones de macOS encima. Con 8 px, el centro de la marca (8 + 18) coincide con el
     de la fila de arriba del panel (52 / 2). */
  :global([data-plataforma='windows']) .arrastre {
    height: 8px;
  }

  /* Marca: logo y "Nexo" centrados sobre el menú. */
  .marca {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    flex-shrink: 0;
    height: 36px;
    padding: 0 20px 0 12px;
    margin-bottom: 22px;
    font-size: 20px;
    line-height: 24px;
    font-weight: 650;
    letter-spacing: -0.025em;
  }
  .logo {
    display: flex;
    flex-shrink: 0;
  }

  /* Menú centrado: todas las opciones del mismo ancho, en una columna al medio de la barra.
     Dentro, ícono y nombre alineados a la izquierda para que los íconos queden en fila. */
  .nav {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 196px);
    justify-content: center;
    align-content: start;
    gap: 4px;
    margin: 0;
    padding: 0 12px;
    list-style: none;
    overflow-y: auto;
  }

  .nav__op {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-height: 42px;
    padding: 10px 16px;
    border: 0;
    border-radius: var(--radio-sm);
    font-size: 14px;
    line-height: 20px;
    font-weight: 500;
    letter-spacing: -0.005em;
    text-align: left;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .nav__op :global(svg) {
    flex-shrink: 0;
  }
  /* Lo no elegido: transparente; al pasar el mouse solo cambia el color. */
  .nav__op:not(.elegido) {
    border-color: transparent;
    background: transparent;
    color: var(--texto-2);
  }
  .nav__op:not(.elegido) :global(svg) {
    color: var(--texto-3);
    transition: color var(--dur-color) ease;
  }
  .nav__op:not(.elegido):hover :global(svg) {
    color: var(--texto-2);
  }
  .nav__op:not(.elegido):hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  /* Lo elegido: una pastilla turquesa suave, sin bordes ni rayas. */
  .nav__op.elegido {
    border: 0;
    background: color-mix(in srgb, var(--primario) 14%, transparent);
    color: var(--primario-texto);
    font-weight: 600;
  }
  /* Los nombres caben en 220 px; si algún día no, bajan de línea en vez de cortarse. */
  .nav__nombre {
    min-width: 0;
    overflow-wrap: break-word;
  }

  /* Tarjeta de estado: punto de color, título y detalle, y la onda a la derecha.
     El detalle pasa por debajo de la onda para que "Todo en perfecto estado" quepa en una línea. */
  /* El pie, del mismo ancho que el menú y centrado como él. */
  .pie {
    flex-shrink: 0;
    width: min(196px, calc(100% - 20px));
    margin: 0 auto;
    padding: 12px 0 16px;
  }
  .estado {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    column-gap: 10px;
    row-gap: 1px;
    padding: 12px 10px 12px 12px;
    border-radius: 16px;
    background: var(--relleno-tarjeta);
    box-shadow:
      inset 0 1px 0 var(--luz-tope),
      0 0 0 1px var(--borde-tarjeta),
      var(--sombra-tarjeta);
  }
  .punto {
    grid-row: 1 / 3;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    --tono: var(--exito);
    background: var(--tono);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--tono) 20%, transparent);
  }
  .aviso .punto {
    --tono: var(--aviso);
  }
  .error .punto {
    --tono: var(--error);
  }
  .estado__titulo {
    grid-column: 2;
    grid-row: 1;
    min-width: 0;
    font-size: 13px;
    line-height: 18px;
    font-weight: 650;
    color: var(--texto);
    overflow-wrap: break-word;
  }
  .estado__onda {
    grid-column: 3;
    grid-row: 1;
    display: flex;
    color: var(--primario-texto);
  }
  /* El lema de la guía de marca, quieto y tenue debajo del estado. */
  .lema {
    margin: 10px 0 0;
    text-align: center;
    font-size: 11px;
    line-height: 16px;
    font-weight: 500;
    letter-spacing: 0;
    color: var(--texto-3);
  }
  .estado__detalle {
    grid-column: 2 / 4;
    grid-row: 2;
    min-width: 0;
    font-size: 12px;
    line-height: 17px;
    color: var(--texto-2);
    overflow-wrap: break-word;
  }

  /* Plegada: solo íconos y el punto de estado. Los textos siguen ahí para los lectores de pantalla. */
  @media (max-width: 899.98px) {
    .lateral {
      width: 68px;
    }
    .marca {
      justify-content: center;
      padding: 0;
    }
    .marca__nombre {
      display: none;
    }
    .nav__op {
      justify-content: center;
      padding: 0;
    }
    .nav__nombre,
    .estado__titulo,
    .estado__detalle {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip: rect(0 0 0 0);
      white-space: nowrap;
    }
    .estado__onda,
    .lema {
      display: none;
    }
    .pie {
      padding: 8px 10px 14px;
    }
    .estado {
      display: flex;
      justify-content: center;
      padding: 14px 0;
      background: transparent;
      box-shadow: none;
    }
  }
</style>
