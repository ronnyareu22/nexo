<!-- Tarjeta "Automático" de un aparato: usarlo para el sonido al conectarse y avisar con batería baja.
     La tarjeta está desde el principio (con siluetas mientras lee) para que la página no salte al cargar.
     Mide lo justo: en la página no se estira (el alto que sobra se lo llevan las tarjetas con gráficas). -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { guardarAutomatico, leerAutomatico } from '$lib/api';
  import { esMac } from '$lib/api-sistema';
  import type { Automatico, Dispositivo } from '$lib/tipos';
  import { esDeSonido } from '$lib/tipos';
  import Interruptor from './Interruptor.svelte';
  import Icono, { type Nombre } from './Icono.svelte';

  let { dispositivo: d, onerror }: { dispositivo: Dispositivo; onerror: (texto: string) => void } = $props();

  const id = $props.id();

  let ajustes = $state<Automatico | null>(null);
  onMount(() => {
    leerAutomatico(d.id)
      .then((a) => (ajustes = a))
      .catch((e) => onerror(String(e)));
  });

  async function cambiar(campo: keyof Automatico) {
    if (!ajustes) return;
    const nuevo = { ...ajustes, [campo]: !ajustes[campo] };
    ajustes = nuevo;
    try {
      ajustes = await guardarAutomatico(d.id, nuevo);
    } catch (e) {
      onerror(String(e));
    }
  }

  const sistema = esMac ? 'macOS' : 'Windows';
  const equipo = esMac ? 'la Mac' : 'la computadora';

  const filas = $derived<{ campo: keyof Automatico; icono: Nombre; tono: string; titulo: string; ayuda: string }[]>([
    ...(esDeSonido(d)
      ? [
          {
            campo: 'usarSalidaAlConectar' as const,
            icono: 'volumen' as const,
            tono: 'var(--celeste)',
            titulo: 'Usar para el sonido al conectarse',
            ayuda: `Al conectarse, el sonido de ${equipo} pasa a este aparato.`,
          },
        ]
      : []),
    {
      campo: 'avisarBateriaBaja',
      icono: 'bateria',
      tono: 'var(--exito)',
      titulo: 'Avisarme con batería baja',
      ayuda: `Una notificación de ${sistema} cuando baje del 20 %.`,
    },
  ]);
</script>

<section class="panel vidrio" aria-labelledby="{id}-titulo">
  <header class="t-cabeza">
    <span class="ficha-icono"><Icono nombre="automatizaciones" tamano={19} /></span>
    <div class="t-textos">
      <h2 class="t-titulo" id="{id}-titulo">Automático</h2>
      <p class="t-sub">Nexo lo hace solo, sin que abras nada</p>
    </div>
  </header>

  <ul class="filas inserto" aria-busy={!ajustes}>
    {#each filas as f (f.campo)}
      <li class="fila">
        <span class="ficha-icono chica" style:--tono={f.tono}><Icono nombre={f.icono} tamano={16} /></span>
        <div class="textos">
          <div class="titulo">{f.titulo}</div>
          <div class="ayuda">{f.ayuda}</div>
        </div>
        {#if ajustes}
          <Interruptor encendido={ajustes[f.campo]} etiqueta={f.titulo} oncambio={() => cambiar(f.campo)} />
        {:else}
          <span class="silueta" aria-hidden="true"></span>
        {/if}
      </li>
    {/each}
  </ul>
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
    border-radius: var(--radio-lg);
  }
  .filas {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0 14px;
    list-style: none;
  }
  .fila {
    flex: 1 1 auto;
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 64px;
    padding: 12px 0;
  }
  .fila + .fila {
    border-top: 1px solid var(--borde);
  }
  .textos {
    flex: 1;
    min-width: 0;
  }
  .titulo {
    font-size: 13.5px;
    line-height: 19px;
    font-weight: 600;
  }
  .ayuda {
    margin-top: 2px;
    font-size: 12.5px;
    line-height: 17px;
    color: var(--texto-2);
  }
  .silueta {
    flex-shrink: 0;
    width: 50px;
    height: 30px;
    border-radius: 99px;
    background: var(--superficie-hundida);
    animation: respirar 1.4s ease-in-out infinite;
  }
  @keyframes respirar {
    50% {
      opacity: 0.55;
    }
  }
</style>
