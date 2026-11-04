<!--
  Las luces del escritorio, en Mi setup: las de la habitación Escritorio (o del grupo con ese nombre), en tarjetas
  compactas con su interruptor y su brillo, y encender o apagar todas juntas. Todo lo demás está en Casa.
  - Sin cuentas ni luces: un estado calmo con "Conectar luces" (lleva a Ajustes › Casa) y "Buscar aparatos".
  - Con luces, pero ninguna en el escritorio: cómo ponerlas ahí (en Casa).
-->
<script lang="ts">
  import type { Accion, EstadoCasa } from '$lib/tipos-hogar';
  import { ESCRITORIO, delEscritorio, hayCuentas, normalizar } from '$lib/hogar';
  import Icono from './Icono.svelte';
  import IlustracionAparato from './IlustracionAparato.svelte';
  import TarjetaAparato from './TarjetaAparato.svelte';

  let {
    casa,
    onorden,
    ontodas,
    onir,
    onerror,
  }: {
    casa: EstadoCasa | null;
    /** Una acción para un aparato. */
    onorden: (id: string, accion: Accion, confirmado?: boolean) => Promise<void>;
    /** Encender o apagar todas las del escritorio (el grupo "Escritorio" o la habitación). */
    ontodas: (encender: boolean, grupo: string | null) => Promise<void>;
    onir: (seccion: string, id?: string) => void;
    onerror: (texto: string) => void;
  } = $props();

  const luces = $derived(casa ? delEscritorio(casa) : []);
  const grupo = $derived(casa?.grupos.find((g) => normalizar(g.nombre) === ESCRITORIO)?.id ?? null);
  const prendidas = $derived(luces.filter((a) => a.enLinea && a.estado.encendido).length);
  const manejables = $derived(luces.filter((a) => a.controlable && a.enLinea));
  let ocupado = $state(false);

  async function todas(encender: boolean) {
    ocupado = true;
    await ontodas(encender, grupo);
    ocupado = false;
  }
</script>

<section class="luces lamina" aria-labelledby="setup-luces">
  <header class="cabeza">
    <span class="ficha-icono" style:--tono="var(--amarillo)"><Icono nombre="bombilla" tamano={19} /></span>
    <div class="t-textos">
      <h2 class="t-titulo" id="setup-luces">Luces del escritorio</h2>
      <p class="t-sub">
        {#if !casa}
          Revisando tus luces…
        {:else if luces.length}
          {luces.length === 1 ? '1 luz' : `${luces.length} luces`} · {prendidas === 0
            ? 'todas apagadas'
            : prendidas === luces.length
              ? luces.length === 1
                ? 'encendida'
                : 'todas encendidas'
              : `${prendidas} encendida${prendidas === 1 ? '' : 's'}`}
        {:else if hayCuentas(casa)}
          Tus luces están en Casa, por habitación.
        {:else}
          La tira, la lámpara del monitor y tus focos, desde aquí.
        {/if}
      </p>
    </div>
    <div class="acciones">
      {#if manejables.length > 1}
        <div class="par" role="group" aria-label="Todas las luces del escritorio">
          <button type="button" class="boton-vidrio boton" disabled={ocupado} onclick={() => todas(true)}>Encender todas</button>
          <button type="button" class="boton-vidrio boton" disabled={ocupado} onclick={() => todas(false)}>Apagar todas</button>
        </div>
      {/if}
      {#if casa && hayCuentas(casa)}
        <button type="button" class="boton-vidrio boton" onclick={() => onir('casa')}>
          Ver toda la casa
          <Icono nombre="chevron" tamano={15} />
        </button>
      {/if}
    </div>
  </header>

  {#if !casa}
    <div class="cargando" aria-busy="true"><span></span><span></span></div>
  {:else if luces.length}
    <div class="rejilla">
      {#each luces as a (a.id)}
        <TarjetaAparato aparato={a} compacta {onerror} onorden={(accion, confirmado) => onorden(a.id, accion, confirmado)} />
      {/each}
    </div>
  {:else}
    <div class="vacio">
      <span class="vacio__dibujo" aria-hidden="true">
        <IlustracionAparato clase="luz" forma="barra" encendido color="rgb(255 214 160)" />
      </span>
      <div class="vacio__textos">
        {#if hayCuentas(casa)}
          <p class="vacio__titulo">Todavía no hay luces en tu escritorio</p>
          <p class="vacio__texto">En Casa, pon la tira, la lámpara del monitor o un foco en la habitación Escritorio y aparecerán aquí.</p>
          <div class="vacio__botones">
            <button type="button" class="boton-primario" onclick={() => onir('casa')}>Ir a Casa</button>
          </div>
        {:else}
          <p class="vacio__titulo">Conecta tus luces</p>
          <p class="vacio__texto">
            Con tu cuenta de Tuya / Smart Life o tu Home Assistant, o buscando en tu red las de WiZ, Yeelight, Govee y
            las tiras Bluetooth. Mi setup sigue funcionando igual sin luces.
          </p>
          <div class="vacio__botones">
            <button type="button" class="boton-primario" onclick={() => onir('ajustes', 'casa')}>Conectar luces</button>
            <button type="button" class="boton-vidrio boton" onclick={() => onir('casa', 'buscar')}>
              <Icono nombre="buscar" tamano={15} />
              Buscar aparatos
            </button>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</section>

<style>
  .luces {
    display: grid;
    gap: 14px;
    padding: 14px 16px 16px;
    border-radius: var(--radio-lg);
    min-width: 0;
  }
  .cabeza {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 12px;
  }
  .cabeza .t-textos {
    flex: 1 1 220px;
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .par {
    display: flex;
    gap: 6px;
  }
  .boton {
    min-height: 32px;
    padding: 0 12px;
    background: var(--relleno-tarjeta);
    box-shadow:
      inset 0 1px 0 var(--luz-tope),
      0 0 0 1px var(--borde-tarjeta);
    color: var(--texto);
  }
  .boton:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .boton:disabled {
    color: var(--texto-3);
    cursor: default;
  }

  .rejilla {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 10px;
  }

  .cargando {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 10px;
  }
  .cargando span {
    height: 112px;
    border-radius: 16px;
    background: var(--superficie-inserto);
  }

  .vacio {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 14px 20px;
    padding: 12px 14px;
    border-radius: var(--radio-md);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .vacio__dibujo {
    display: block;
    flex-shrink: 0;
    width: 96px;
    height: 96px;
  }
  .vacio__textos {
    flex: 1 1 260px;
    min-width: 0;
    display: grid;
    gap: 4px;
  }
  .vacio__titulo {
    margin: 0;
    font: var(--letra-panel);
  }
  .vacio__texto {
    margin: 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
    max-width: 62ch;
  }
  .vacio__botones {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
  }
</style>
