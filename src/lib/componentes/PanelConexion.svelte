<!--
  Tarjeta de conexión de la página del aparato, todo sacado del historial real:
  - tres datos: desde cuándo está conectado (o la última vez que se usó), el uso de hoy y el de los últimos 7 días;
  - el uso por día de la última semana en barras, con guías finas (la de hoy en turquesa);
  - las dos últimas sesiones con su horario y duración.
  Con `onalternar` suma el interruptor para conectar. Las barras ocupan el alto que sobre en la tarjeta.
  Ningún rótulo se corta: si no cabe, baja de línea.
-->
<script lang="ts">
  import type { Dispositivo, Registro } from '$lib/tipos';
  import { cuando, duracion, hora, inicioDeHoy, usoDesde, ahora, ultimaVez as ultimaDe } from '$lib/tiempo';
  import { tooltip } from '$lib/tooltip';
  import Interruptor from './Interruptor.svelte';
  import Icono from './Icono.svelte';

  let {
    dispositivo: d,
    pendiente,
    registro,
    onalternar,
  }: { dispositivo: Dispositivo; pendiente: boolean; registro: Registro | null; onalternar?: () => void } = $props();

  const id = $props.id();

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

  const ayuda = $derived(
    pendiente
      ? d.conectado
        ? 'Desconectando…'
        : 'Conectando…'
      : d.conectado
        ? 'Conectado a esta computadora'
        : 'Emparejado, listo para conectar',
  );

  // --- Uso por día: los últimos 7 días, de más viejo a hoy ---
  const nombreDia = (t: number, forma: 'narrow' | 'long') => new Date(t * 1000).toLocaleDateString('es', { weekday: forma });
  const dias = $derived.by(() => {
    void reloj;
    const hoyInicio = inicioDeHoy();
    const lista = [];
    for (let i = 6; i >= 0; i--) {
      const desde = hoyInicio - i * 86400;
      const uso = usoDesde(sesiones, desde) - usoDesde(sesiones, desde + 86400);
      lista.push({ desde, uso, hoy: i === 0, letra: nombreDia(desde, 'narrow').toUpperCase(), nombre: nombreDia(desde, 'long') });
    }
    return lista;
  });
  // La escala va al día más usado, redondeada a horas enteras (al menos una): poco uso no parece mucho.
  const escala = $derived(Math.max(1, Math.ceil(Math.max(...dias.map((x) => x.uso)) / 3600)) * 3600);
  const resumenDias = $derived(
    dias.map((x) => `${x.hoy ? 'hoy' : x.nombre}: ${x.uso ? duracion(x.uso) : 'sin uso'}`).join(', '),
  );

  // --- Últimas sesiones (las 2 más nuevas) ---
  const capital = (t: string) => t.charAt(0).toUpperCase() + t.slice(1);
  function dia(t: number) {
    const h = inicioDeHoy();
    if (t >= h) return 'Hoy';
    if (t >= h - 86400) return 'Ayer';
    return capital(nombreDia(t, 'long'));
  }
  const recientes = $derived.by(() => {
    void reloj;
    return sesiones
      .slice(-2)
      .reverse()
      .map(([inicio, fin]) => ({
        clave: `${inicio}`,
        dia: dia(inicio),
        horario: `${hora(inicio)} – ${fin === null ? 'ahora' : hora(fin)}`,
        dura: duracion((fin ?? ahora()) - inicio),
        abierta: fin === null,
      }));
  });
</script>

<section class="panel vidrio" aria-labelledby="{id}-titulo">
  <header class="t-cabeza">
    <span class="ficha-icono" class:tenue={!d.conectado}><Icono nombre="bluetooth" tamano={19} /></span>
    <div class="t-textos">
      <h2 class="t-titulo" id="{id}-titulo">Conexión</h2>
      <p class="t-sub">{ayuda}</p>
    </div>
    {#if onalternar}
      <Interruptor encendido={d.conectado} cargando={pendiente} etiqueta={d.conectado ? 'Desconectar' : 'Conectar'} oncambio={onalternar} />
    {/if}
  </header>

  <dl class="datos">
    {#if d.conectado && abierta}
      <div class="dato">
        <dt>Conectado</dt>
        <dd class="valor">{hora(abierta[0])}</dd>
        <dd class="sub">hace {conectadoHace}</dd>
      </div>
    {:else}
      <div class="dato">
        <dt>Última vez</dt>
        {#if ultimaVez}
          <dd class="valor chico">{ultimaVezTexto}</dd>
        {:else}
          <dd class="valor tenue">Sin registro</dd>
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

  <div class="detalle">
    <div class="semana">
      <div class="fila-rotulo">
        <span class="rotulo">Uso por día</span>
        {#if semana}<span class="nota">Promedio {duracion(semana / 7)}</span>{/if}
      </div>
      <div class="grafico" role="img" aria-label="Uso por día. {resumenDias}">
        <div class="area">
          <span class="guia" style:top="0%"><b>{duracion(escala)}</b></span>
          <span class="guia" style:top="50%"></span>
          <span class="guia base" style:top="100%"></span>
          <div class="barras">
            {#each dias as x (x.desde)}
              <div class="dia" class:hoy={x.hoy} use:tooltip={`${x.hoy ? 'Hoy' : capital(x.nombre)}: ${x.uso ? duracion(x.uso) : 'sin uso'}`}>
                <span class="barra" class:vacia={!x.uso} style:height={x.uso ? `${Math.max(3, (x.uso / escala) * 100)}%` : undefined}></span>
              </div>
            {/each}
          </div>
        </div>
        <div class="letras" aria-hidden="true">
          {#each dias as x (x.desde)}<span class:hoy={x.hoy}>{x.letra}</span>{/each}
        </div>
      </div>
    </div>

    {#if recientes.length}
      <div class="sesiones">
        <span class="rotulo">Sesiones recientes</span>
        <ul>
          {#each recientes as s (s.clave)}
            <li>
              <span class="punto" class:abierta={s.abierta} aria-hidden="true"></span>
              <span class="cuando"><b>{s.dia}</b> {s.horario}</span>
              <span class="dura">{s.dura}</span>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>
</section>

<style>
  .panel {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
    border-radius: var(--radio-lg);
  }

  /* Tres datos en fichas; si no caben, bajan a otra fila. */
  .datos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(104px, 1fr));
    gap: 8px;
    margin: 0;
  }
  .dato {
    display: grid;
    align-content: start;
    gap: 3px;
    min-width: 0;
    padding: 11px 13px 12px;
    border-radius: 14px;
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  dt {
    font-size: 12px;
    line-height: 16px;
    font-weight: 500;
    color: var(--texto-2);
  }
  dd {
    margin: 0;
    overflow-wrap: break-word;
  }
  .valor {
    font-size: 17px;
    line-height: 23px;
    font-weight: 650;
    letter-spacing: -0.015em;
    font-variant-numeric: tabular-nums;
  }
  .valor.chico {
    font-size: 14px;
    line-height: 20px;
    letter-spacing: 0;
  }
  .valor.tenue {
    font-size: 14px;
    line-height: 20px;
    font-weight: 500;
    color: var(--texto-2);
  }
  .sub {
    font-size: 12px;
    line-height: 16px;
    color: var(--texto-3);
    font-variant-numeric: tabular-nums;
  }

  .detalle {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .fila-rotulo {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }
  .nota {
    font-size: 12px;
    line-height: 16px;
    font-weight: 500;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }

  /* ---------- Barras de la semana: guías finas (arriba el máximo) y una barra por día ---------- */
  .semana {
    flex: 1 1 auto;
    min-height: 120px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .grafico {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .area {
    position: relative;
    flex: 1 1 auto;
    min-height: 72px;
    margin-top: 14px;
  }
  .guia {
    position: absolute;
    left: 0;
    right: 0;
    height: 0;
    border-top: 1px dashed color-mix(in srgb, var(--texto-3) 32%, transparent);
  }
  .guia.base {
    border-top-style: solid;
    border-top-color: var(--borde-fuerte);
  }
  .guia b {
    position: absolute;
    right: 0;
    bottom: 3px;
    font-size: 10.5px;
    line-height: 14px;
    font-weight: 600;
    color: var(--texto-3);
    font-variant-numeric: tabular-nums;
  }
  .barras,
  .letras {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 8px;
  }
  .barras {
    position: absolute;
    inset: 0;
    align-items: end;
  }
  .dia {
    display: flex;
    align-items: flex-end;
    justify-content: center;
    height: 100%;
  }
  .barra {
    width: 100%;
    max-width: 26px;
    height: 3px;
    border-radius: 7px 7px 3px 3px;
    background: color-mix(in srgb, var(--primario) 40%, var(--superficie-base));
  }
  .barra.vacia {
    border-radius: 2px;
    background: var(--borde-fuerte);
  }
  .hoy .barra:not(.vacia) {
    background: var(--primario);
  }
  .letras span {
    text-align: center;
    font-size: 11px;
    line-height: 14px;
    font-weight: 600;
    color: var(--texto-3);
  }
  .letras span.hoy {
    color: var(--primario-texto);
  }

  /* ---------- Sesiones recientes ---------- */
  .sesiones {
    display: grid;
    gap: 6px;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 34px;
    padding: 6px 0;
    font-size: 13px;
    line-height: 18px;
    font-variant-numeric: tabular-nums;
  }
  li + li {
    border-top: 1px solid var(--borde);
  }
  .punto {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--texto-3);
  }
  .punto.abierta {
    background: var(--exito);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--exito) 22%, transparent);
  }
  .cuando {
    flex: 1;
    min-width: 0;
    color: var(--texto-2);
    overflow-wrap: break-word;
  }
  .cuando b {
    font-weight: 600;
    color: var(--texto);
  }
  .dura {
    flex-shrink: 0;
    font-weight: 600;
    color: var(--texto);
  }

</style>
