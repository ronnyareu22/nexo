<!-- Panel de conexión: desde cuándo está conectado o la última vez que se usó, y el uso de hoy y de la semana.
     Con `onalternar` suma el interruptor para conectar (en el detalle ya está el botón de Desconectar).
     Los datos se reparten en columnas de al menos 150 px y bajan de línea si no caben: ningún rótulo se corta. -->
<script lang="ts">
  import type { Dispositivo, Registro } from '$lib/tipos';
  import { cuando, duracion, hora, inicioDeHoy, usoDesde, ahora, ultimaVez as ultimaDe } from '$lib/tiempo';
  import Interruptor from './Interruptor.svelte';
  import Icono from './Icono.svelte';

  let {
    dispositivo: d,
    pendiente,
    registro,
    onalternar,
  }: { dispositivo: Dispositivo; pendiente: boolean; registro: Registro | null; onalternar?: () => void } = $props();

  // Se recalcula cada 30 s para que "hace 12 min" avance solo.
  let reloj = $state(ahora());
  $effect(() => {
    const t = setInterval(() => (reloj = ahora()), 30_000);
    return () => clearInterval(t);
  });

  const sesiones = $derived(registro?.sesiones ?? []);
  const abierta = $derived(sesiones.length && sesiones[sesiones.length - 1][1] === null ? sesiones[sesiones.length - 1] : null);
  const ultimaVez = $derived(ultimaDe(sesiones));
  // Leen `reloj` para recalcularse cada 30 s.
  const hoy = $derived.by(() => (void reloj, usoDesde(sesiones, inicioDeHoy())));
  const semana = $derived.by(() => (void reloj, usoDesde(sesiones, ahora() - 7 * 86400)));
  const conectadoHace = $derived.by(() => (void reloj, abierta ? duracion(ahora() - abierta[0]) : ''));
  const ultimaVezTexto = $derived.by(() => (void reloj, ultimaVez ? cuando(ultimaVez) : ''));
</script>

<section class="panel vidrio">
  <div class="cabeza">
    <div class="icono" class:activo={d.conectado}><Icono nombre="bluetooth" tamano={20} /></div>
    <div class="textos">
      <h2 class="titulo">Conexión</h2>
      <div class="ayuda">
        {pendiente ? (d.conectado ? 'Desconectando…' : 'Conectando…') : d.conectado ? 'Conectado a esta computadora' : 'Emparejado, listo para conectar'}
      </div>
    </div>
    {#if onalternar}
      <Interruptor grande encendido={d.conectado} cargando={pendiente} etiqueta={d.conectado ? 'Desconectar' : 'Conectar'} oncambio={onalternar} />
    {/if}
  </div>

  <dl class="datos">
    {#if d.conectado && abierta}
      <div class="dato">
        <dt>Conectado desde</dt>
        <dd class="valor">{hora(abierta[0])}</dd>
        <dd class="sub">hace {conectadoHace}</dd>
      </div>
    {:else}
      <div class="dato">
        <dt>Última vez usado</dt>
        {#if ultimaVez}
          <dd class="valor">{ultimaVezTexto}</dd>
        {:else}
          <dd class="valor tenue">Sin registro aún</dd>
        {/if}
      </div>
    {/if}
    <div class="dato">
      <dt>Hoy</dt>
      <dd class="valor">{hoy ? duracion(hoy) : '—'}</dd>
    </div>
    <div class="dato">
      <dt>Últimos 7 días</dt>
      <dd class="valor">{semana ? duracion(semana) : '—'}</dd>
    </div>
  </dl>
</section>

<style>
  .panel {
    border-radius: var(--radio-lg);
    padding: 18px 20px;
  }
  .cabeza {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .icono {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    flex-shrink: 0;
    border-radius: 13px;
    background: var(--superficie-hundida);
    color: var(--texto-3);
  }
  .icono.activo {
    color: var(--sobre-primario);
    background: var(--primario);
  }
  .textos {
    flex: 1;
    min-width: 0;
  }
  .titulo {
    margin: 0;
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
  }
  .ayuda {
    margin-top: 2px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .datos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 10px;
    margin: 16px 0 0;
  }
  .dato {
    display: grid;
    align-content: start;
    gap: 2px;
    min-width: 0;
    padding: 10px 12px;
    border-radius: 14px;
    background: var(--superficie-hundida);
  }
  dt {
    font-size: 12px;
    line-height: 17px;
    font-weight: 600;
    color: var(--texto-2);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .valor {
    font-size: 14px;
    line-height: 20px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .valor.tenue {
    color: var(--texto-2);
    font-weight: 500;
  }
  .sub {
    font-size: 12px;
    line-height: 17px;
    color: var(--texto-2);
  }
</style>
