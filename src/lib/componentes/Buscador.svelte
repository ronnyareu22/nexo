<!-- Buscador de la barra de arriba. ⌘K (Ctrl K fuera de Mac) lo enfoca desde cualquier parte y Escape
     lo borra y lo suelta. Lo escrito sube con `bind:valor`; la sección Dispositivos es la que filtra. -->
<script lang="ts">
  import Icono from './Icono.svelte';

  let {
    valor = $bindable(''),
    onescribir,
  }: {
    valor?: string;
    /** Avisa que se escribió algo (para llevar a Dispositivos). */
    onescribir?: () => void;
  } = $props();

  // En Mac el atajo va con ⌘; en Windows y Linux, con Ctrl.
  const esMac = /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent);
  let campo: HTMLInputElement | undefined;

  function teclaGlobal(e: KeyboardEvent) {
    if (e.key?.toLowerCase() !== 'k' || e.altKey || e.shiftKey) return;
    const conAtajo = esMac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
    if (!conAtajo) return;
    e.preventDefault();
    campo?.focus();
    campo?.select();
  }

  function teclaCampo(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    // Que no le llegue también a la vista (Escape ahí cierra el detalle).
    e.preventDefault();
    e.stopPropagation();
    valor = '';
    campo?.blur();
  }

  function borrar() {
    valor = '';
    campo?.focus();
  }
</script>

<svelte:window onkeydown={teclaGlobal} />

<div class="buscador" role="search">
  <Icono nombre="buscar" tamano={17} />
  <input
    bind:this={campo}
    bind:value={valor}
    type="search"
    placeholder="Buscar dispositivos…"
    aria-label="Buscar dispositivos"
    aria-keyshortcuts={esMac ? 'Meta+K' : 'Control+K'}
    autocomplete="off"
    spellcheck="false"
    enterkeyhint="search"
    oninput={() => valor && onescribir?.()}
    onkeydown={teclaCampo}
  />
  {#if valor}
    <button class="limpiar" onclick={borrar} aria-label="Borrar búsqueda">
      <Icono nombre="cerrar" tamano={14} />
    </button>
  {:else}
    <kbd class="atajo" aria-hidden="true">{esMac ? '⌘' : 'Ctrl'} K</kbd>
  {/if}
</div>

<style>
  .buscador {
    display: flex;
    align-items: center;
    gap: 9px;
    flex: 0 1 380px;
    min-width: 240px;
    height: 38px;
    padding: 0 7px 0 13px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-sm);
    background: var(--superficie);
    color: var(--texto-3);
    transition:
      border-color var(--dur-color) ease,
      box-shadow var(--dur-color) ease;
  }
  .buscador > :global(svg) {
    flex-shrink: 0;
  }
  /* Con el foco: borde turquesa y un anillo suave, como el foco de macOS. */
  .buscador:focus-within {
    border-color: var(--primario);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--primario) 20%, transparent);
  }

  input {
    -webkit-appearance: none;
    appearance: none;
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--texto);
    font: inherit;
    font-size: 13.5px;
    user-select: text;
    -webkit-user-select: text;
  }
  input::placeholder {
    color: var(--texto-3);
  }
  input::-webkit-search-cancel-button,
  input::-webkit-search-decoration {
    display: none;
  }

  .atajo {
    flex-shrink: 0;
    padding: 2px 7px;
    border: 1px solid var(--borde);
    border-radius: 7px;
    background: var(--superficie-elevada);
    color: var(--texto-2);
    font-family: inherit;
    font-size: 11.5px;
    line-height: 16px;
    font-weight: 500;
    white-space: nowrap;
  }

  .limpiar {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--texto-3);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .limpiar:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
</style>
