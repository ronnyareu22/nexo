<!-- Selector de tema: Sistema, Claro u Oscuro. La opción elegida lleva el estilo de "lo elegido". -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { aplicarTema, leerTema, type Tema } from '$lib/tema';
  import { tooltip } from '$lib/tooltip';
  import Icono from './Icono.svelte';

  let tema = $state<Tema>('sistema');
  onMount(() => {
    tema = leerTema();
    const alCambiar = (e: Event) => (tema = (e as CustomEvent<Tema>).detail);
    window.addEventListener('nexo-tema', alCambiar);
    return () => window.removeEventListener('nexo-tema', alCambiar);
  });

  const opciones: { valor: Tema; nombre: string; icono: 'auto' | 'sol' | 'luna' }[] = [
    { valor: 'sistema', nombre: 'Como el sistema', icono: 'auto' },
    { valor: 'claro', nombre: 'Modo claro', icono: 'sol' },
    { valor: 'oscuro', nombre: 'Modo oscuro', icono: 'luna' },
  ];

  function elegir(t: Tema) {
    tema = t;
    aplicarTema(t);
  }
</script>

<div class="tema vidrio" role="group" aria-label="Tema">
  {#each opciones as o (o.valor)}
    <button
      class="tema__op"
      class:elegido={tema === o.valor}
      aria-pressed={tema === o.valor}
      aria-label={o.nombre}
      use:tooltip={o.nombre}
      onclick={() => elegir(o.valor)}
    >
      <Icono nombre={o.icono} tamano={16} />
    </button>
  {/each}
</div>

<style>
  .tema {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: 99px;
  }
  .tema__op {
    display: grid;
    place-items: center;
    width: 32px;
    height: 28px;
    padding: 0;
    border-width: 1px;
    border-style: solid;
    border-radius: 99px;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  /* Lo no elegido: transparente; al pasar el mouse solo cambia el color. */
  .tema__op:not(.elegido) {
    border-color: transparent;
    background: transparent;
    color: var(--texto-3);
  }
  .tema__op:not(.elegido):hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
</style>
