<!--
  Una ilustración de static/escenas (diseno/escenas/generar.py), pegada en línea para que siga el tema: sus clases
  leen los colores de lib/estilos/ilustraciones.css. Se lee una sola vez por archivo y se recuerda.
  Si el archivo no existe (o el nombre llega vacío), va la genérica (el rayo) con el ícono de línea encima.
  `recorte` la llena a lo ancho y a lo alto (recorta los lados): para las miniaturas cuadradas.
  Es decorativa: el texto accesible lo pone quien la usa.
-->
<script module lang="ts">
  import '$lib/estilos/ilustraciones.css';
  import { ICONOS_ESCENA } from '$lib/setup';

  const GENERICA = 'rayo';
  const leidas = new Map<string, Promise<string>>();

  /** Solo el dibujo: sin su <style> (las reglas vienen de ilustraciones.css) y oculto para los lectores. */
  function limpiar(svg: string): string {
    const t = svg.trim();
    if (!t.startsWith('<svg')) return '';
    return t.replace(/<style>[\s\S]*?<\/style>/, '').replace('<svg ', '<svg aria-hidden="true" focusable="false" ');
  }

  function leer(nombre: string): Promise<string> {
    if (!nombre) return Promise.resolve('');
    let p = leidas.get(nombre);
    if (!p) {
      p = fetch(`/escenas/${nombre}.svg`)
        .then((r) => (r.ok ? r.text() : ''))
        .then(limpiar)
        .catch(() => '');
      leidas.set(nombre, p);
    }
    return p;
  }

  /** El archivo de la ilustración de una escena por su ícono; vacío si el ícono no se conoce. */
  export const ilustracionDe = (icono: string): string =>
    (ICONOS_ESCENA as readonly string[]).includes(icono) ? icono : '';
</script>

<script lang="ts">
  import Icono, { type Nombre } from './Icono.svelte';

  let {
    nombre,
    icono = 'rayo',
    recorte = false,
  }: {
    /** El archivo, sin «.svg». */
    nombre: string;
    /** El ícono de línea que va encima si hay que usar la genérica. */
    icono?: Nombre;
    recorte?: boolean;
  } = $props();

  let svg = $state('');
  let generica = $state(false);

  $effect(() => {
    const pedido = nombre;
    let vigente = true;
    leer(pedido).then(async (t) => {
      const usar = t || (await leer(GENERICA));
      if (!vigente) return;
      generica = !t;
      svg = usar;
    });
    return () => {
      vigente = false;
    };
  });

  const dibujo = $derived(recorte ? svg.replace('<svg ', '<svg preserveAspectRatio="xMidYMid slice" ') : svg);
</script>

<span class="ilus" class:recorte>
  <!-- Contenido propio de static/escenas, sin datos de nadie. -->
  {@html dibujo}
  {#if generica && svg}
    <span class="ilus__icono"><Icono nombre={icono} tamano={recorte ? 13 : 16} /></span>
  {/if}
</span>

<style>
  .ilus {
    position: relative;
    display: block;
    width: 100%;
    height: 100%;
  }
  .ilus :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }
  /* El ícono de la escena sobre la genérica: una ficha chica abajo a la izquierda. */
  .ilus__icono {
    position: absolute;
    left: 8px;
    bottom: 8px;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 9px;
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--primario-texto);
  }
  .recorte .ilus__icono {
    left: 4px;
    bottom: 4px;
    width: 22px;
    height: 22px;
    border-radius: 7px;
  }
</style>
