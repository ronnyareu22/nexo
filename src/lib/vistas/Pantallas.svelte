<!-- Pantallas: cada pantalla con brillo, resolución, frecuencia y cuál es la principal; abajo, el atajo de brillo.
     Las pantallas y su brillo los lee quien la usa (cada 5 s y cada 250 ms) y llegan por `pantallas`.
     Al cambiar un modo se pregunta si mantenerlo; sin respuesta en 15 s, vuelve al anterior (como macOS). -->
<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { cambiarModoPantalla, esMac, hacerPrincipal, modosPantalla } from '$lib/api-sistema';
  import type { Pantalla } from '$lib/tipos';
  import type { ModoPantalla, PantallaSistema } from '$lib/tipos-sistema';
  import TarjetaPantalla from '$lib/componentes/TarjetaPantalla.svelte';
  import PanelAtajos from '$lib/componentes/PanelAtajos.svelte';
  import Dialogo from '$lib/componentes/Dialogo.svelte';

  let { pantallas, onerror }: { pantallas: Pantalla[]; onerror: (texto: string) => void } = $props();

  /** Segundos para confirmar un modo nuevo antes de volver al anterior. */
  const ESPERA = 15;

  // Modos por pantalla: null mientras cargan; vacío si no se pudieron leer (y entonces no hay menús).
  let modos = $state<Record<number, ModoPantalla[] | null>>({});
  let ocupada = $state(false);
  let prueba = $state<{ pantalla: number; previo: number; que: 'resolucion' | 'frecuencia'; hasta: number } | null>(
    null,
  );
  let segundos = $state(ESPERA);
  let reloj: ReturnType<typeof setInterval> | undefined;

  // --- Pantalla principal: la que dice el sistema; justo después de cambiarla, la elegida (hasta la próxima lectura). ---
  let principalElegida = $state<{ id: number; hasta: number } | null>(null);
  const varias = $derived(pantallas.length > 1);
  const principalId = $derived.by(() => {
    const delSistema = pantallas.find((p) => (p as Partial<PantallaSistema>).principal === true)?.id;
    if (principalElegida && (delSistema === undefined || Date.now() < principalElegida.hasta)) return principalElegida.id;
    return delSistema ?? pantallas.find((p) => p.interna)?.id ?? pantallas[0]?.id;
  });

  async function leerModos(id: number) {
    if (!(id in modos)) modos[id] = null;
    try {
      modos[id] = await modosPantalla(id);
    } catch {
      modos[id] = [];
    }
  }

  // Lee los modos de cada pantalla nueva (no se repite con cada lectura de brillo).
  const ids = $derived(pantallas.map((p) => p.id).join(','));
  $effect(() => {
    const lista = ids ? ids.split(',').map(Number) : [];
    untrack(() => {
      for (const id of lista) if (!(id in modos)) leerModos(id);
    });
  });

  function marcarActual(id: number, modo: number) {
    for (const m of modos[id] ?? []) m.actual = m.id === modo;
  }

  async function probar(p: Pantalla, nuevo: ModoPantalla, que: 'resolucion' | 'frecuencia') {
    const previo = modos[p.id]?.find((m) => m.actual);
    if (!previo || previo.id === nuevo.id || ocupada || prueba) return;
    ocupada = true;
    marcarActual(p.id, nuevo.id);
    try {
      await cambiarModoPantalla(p.id, nuevo.id);
    } catch (e) {
      marcarActual(p.id, previo.id);
      onerror(String(e));
      ocupada = false;
      return;
    }
    ocupada = false;
    prueba = { pantalla: p.id, previo: previo.id, que, hasta: Date.now() + ESPERA * 1000 };
    segundos = ESPERA;
    clearInterval(reloj);
    reloj = setInterval(contar, 250);
  }

  // Cuenta contra la hora límite (no por vueltas), así no se atrasa si el sistema demora el reloj.
  function contar() {
    if (!prueba) return clearInterval(reloj);
    segundos = Math.max(0, Math.ceil((prueba.hasta - Date.now()) / 1000));
    if (segundos === 0) volver();
  }

  function mantener() {
    clearInterval(reloj);
    const pr = prueba;
    prueba = null;
    if (pr) leerModos(pr.pantalla);
  }

  async function volver() {
    clearInterval(reloj);
    const pr = prueba;
    prueba = null;
    if (!pr) return;
    marcarActual(pr.pantalla, pr.previo);
    try {
      await cambiarModoPantalla(pr.pantalla, pr.previo);
    } catch (e) {
      onerror(String(e));
    }
    leerModos(pr.pantalla);
  }

  async function hacerla(p: Pantalla) {
    if (ocupada || prueba) return;
    ocupada = true;
    try {
      await hacerPrincipal(p.id);
      principalElegida = { id: p.id, hasta: Date.now() + 8000 };
    } catch (e) {
      onerror(String(e));
    }
    ocupada = false;
  }

  onMount(() => {
    // Al volver al panel, los modos se leen de nuevo por si se cambiaron desde el sistema.
    const alVolver = () => {
      if (document.visibilityState !== 'visible' || prueba || ocupada) return;
      for (const id of Object.keys(modos)) leerModos(Number(id));
    };
    document.addEventListener('visibilitychange', alVolver);
    return () => document.removeEventListener('visibilitychange', alVolver);
  });

  // Si la vista se cierra sin confirmar, el modo nuevo no se queda.
  onDestroy(() => {
    if (prueba) volver();
    else clearInterval(reloj);
  });
</script>

<div class="vista">
  <header class="cabecera">
    <h1>Pantallas</h1>
    <p>Brillo, resolución y cuál es la principal.</p>
  </header>

  {#if pantallas.length}
    <div class="rejilla">
      {#each pantallas as p (p.id)}
        <TarjetaPantalla
          pantalla={p}
          {onerror}
          {varias}
          principal={p.id === principalId}
          modos={modos[p.id] ?? null}
          ocupada={ocupada || !!prueba}
          onmodo={(m, que) => probar(p, m, que)}
          onprincipal={() => hacerla(p)}
        />
      {/each}
    </div>
  {:else}
    <div class="rejilla" aria-busy="true">
      <span class="solo-lector">Buscando pantallas…</span>
      <div class="silueta vidrio"></div>
    </div>
  {/if}

  {#if esMac}
    <PanelAtajos {onerror} />
  {/if}
</div>

<Dialogo
  abierto={!!prueba}
  titulo={prueba?.que === 'frecuencia' ? '¿Mantener esta frecuencia?' : '¿Mantener esta resolución?'}
  descripcion="Si no eliges nada, vuelve a la anterior en {segundos} s."
  icono="pantallas"
  textoConfirmar="Mantener"
  textoCancelar="Volver a la anterior"
  onconfirmar={mantener}
  oncancelar={volver}
/>

<style>
  .vista {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
  }
  .cabecera h1 {
    margin: 0;
    font-size: 26px;
    line-height: 32px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .cabecera p {
    margin: 4px 0 0;
    color: var(--texto-2);
  }

  .rejilla {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 380px), 1fr));
    gap: 16px;
    align-items: start;
  }

  /* Silueta de la tarjeta mientras llegan las pantallas: quieta, sin brillos que cruzan. */
  .silueta {
    height: 380px;
    border-radius: var(--radio-lg);
  }
</style>
