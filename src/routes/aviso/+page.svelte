<!-- Aviso de brillo: píldora sólida (superficie elevada) con 16 rayitas, como el de macOS. Lo abre Rust (sistema/aviso.rs). -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { nivelAviso } from '$lib/api';
  import Icono from '$lib/componentes/Icono.svelte';

  let nivel = $state(0);
  let pulso = $state(0); // cambia en cada toque para reiniciar la animación

  const rayitas = $derived(Math.round((nivel / 100) * 16));

  onMount(() => {
    nivelAviso().then((n) => (nivel = n));
    const quitar = listen<number>('aviso-brillo', (e) => {
      nivel = e.payload;
      pulso++;
    });
    return () => {
      quitar.then((f) => f());
    };
  });
</script>

<div class="lienzo">
  {#key pulso}
    <div class="pildora flotante" role="status" aria-label="Brillo {nivel}%">
      <span class="sol"><Icono nombre="sol" tamano={22} /></span>
      <div class="rayitas">
        {#each Array.from({ length: 16 }, (_, i) => i) as i (i)}
          <span class:llena={i < rayitas}></span>
        {/each}
      </div>
      <span class="valor">{nivel}%</span>
    </div>
  {/key}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
  }
  .lienzo {
    height: 100vh;
    display: grid;
    place-items: center;
    padding: 8px;
  }
  .pildora {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    height: 64px;
    padding: 0 20px;
    border-radius: 99px;
    animation: entrar 260ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  @keyframes entrar {
    from {
      transform: scale(0.94);
      opacity: 0.6;
    }
  }
  .sol {
    color: var(--amarillo);
    display: grid;
  }
  .rayitas {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(16, 1fr);
    gap: 3px;
  }
  .rayitas span {
    height: 8px;
    border-radius: 3px;
    background: var(--superficie-hundida);
    transition: background 160ms ease;
  }
  .rayitas span.llena {
    background: var(--amarillo);
  }
  .valor {
    width: 42px;
    text-align: right;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
</style>
