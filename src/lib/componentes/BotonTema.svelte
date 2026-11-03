<!-- Botón de tema de la barra de arriba: muestra un sol en claro y una luna en oscuro, y cambia al otro.
     Si el tema guardado es "sistema", se mira cuál se ve ahora y se pasa al contrario, ya fijo.
     Las tres opciones (Sistema, Claro, Oscuro) siguen en Ajustes con el SelectorTema. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { MediaQuery } from 'svelte/reactivity';
  import { aplicarTema, leerTema, type Tema } from '$lib/tema';
  import { tooltip } from '$lib/tooltip';
  import Icono from './Icono.svelte';

  let tema = $state<Tema>(leerTema());
  const sistemaOscuro = new MediaQuery('prefers-color-scheme: dark');

  // Se pone al día si el tema cambia desde Ajustes.
  onMount(() => {
    const alCambiar = (e: Event) => (tema = (e as CustomEvent<Tema>).detail);
    window.addEventListener('nexo-tema', alCambiar);
    return () => window.removeEventListener('nexo-tema', alCambiar);
  });

  const oscuro = $derived(tema === 'oscuro' || (tema === 'sistema' && sistemaOscuro.current));
  const etiqueta = $derived(oscuro ? 'Cambiar a modo claro' : 'Cambiar a modo oscuro');

  function alternar() {
    aplicarTema(oscuro ? 'claro' : 'oscuro');
  }
</script>

<button class="boton-redondo" onclick={alternar} aria-label={etiqueta} use:tooltip={etiqueta}>
  <Icono nombre={oscuro ? 'luna' : 'sol'} tamano={18} />
</button>
