<!-- Panel "Sonido" de la pestaña Audio del aparato: usarlo para el sonido, volumen, silencio y micrófono.
     Se lee cada 2 s mientras se ve, sin pisar un deslizador que se mueve. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { ajustarSonido, leerSonido } from '$lib/api';
  import { enviadorDelUltimo } from '$lib/enviador';
  import { tooltip } from '$lib/tooltip';
  import type { CambioSonido, Sonido } from '$lib/tipos';
  import Icono from './Icono.svelte';
  import Deslizador from './Deslizador.svelte';

  let { id, onerror }: { id: string; onerror: (texto: string) => void } = $props();

  let sonido = $state<Sonido | null>(null);
  // Mientras arrastras un deslizador no se pisa su valor con la lectura periódica.
  let tocando = $state(false);

  async function leer() {
    if (tocando) return;
    try {
      sonido = await leerSonido(id);
    } catch (e) {
      onerror(String(e));
    }
  }

  async function aplicar(cambio: CambioSonido) {
    try {
      const nuevo = await ajustarSonido(id, cambio);
      if (!tocando) sonido = nuevo;
    } catch (e) {
      onerror(String(e));
    }
  }

  // Siempre el último valor, sin fila: el sonido sigue al dedo.
  const fallo = (e: unknown) => onerror(String(e));
  const enviarVolumen = enviadorDelUltimo((v: number) => ajustarSonido(id, { accion: 'volumen', valor: v }), fallo);
  const enviarMicrofono = enviadorDelUltimo(
    (v: number) => ajustarSonido(id, { accion: 'volumenMicrofono', valor: v }),
    fallo,
  );
  function volumenEnVivo(accion: 'volumen' | 'volumenMicrofono', valor: number) {
    (accion === 'volumen' ? enviarVolumen : enviarMicrofono)(valor);
  }

  onMount(() => {
    leer();
    const intervalo = setInterval(() => document.visibilityState === 'visible' && leer(), 2000);
    return () => clearInterval(intervalo);
  });
</script>

{#if sonido && (sonido.tieneSalida || sonido.tieneMicrofono)}
  <section class="panel vidrio">
    <h2>Sonido</h2>

    {#if sonido.tieneSalida}
      <div class="fila">
        <div class="textos">
          <div class="titulo">Salida de sonido</div>
          <div class="ayuda">
            {sonido.salidaEnUso ? 'El sonido de la Mac sale por aquí.' : 'El sonido de la Mac sale por otro aparato.'}
          </div>
        </div>
        {#if sonido.salidaEnUso}
          <span class="en-uso"><Icono nombre="check" tamano={14} /> En uso</span>
        {:else}
          <button class="boton-primario principal" onclick={() => aplicar({ accion: 'usarSalida' })}>Usar para el sonido</button>
        {/if}
      </div>

      {#if sonido.volumen !== null}
        <div class="control">
          <button
            class="redondo"
            class:silenciado={sonido.silenciado}
            aria-pressed={sonido.silenciado ?? false}
            onclick={() => aplicar({ accion: 'silenciar', valor: !sonido?.silenciado })}
            aria-label={sonido.silenciado ? 'Quitar silencio' : 'Silenciar'}
            use:tooltip={sonido.silenciado ? 'Quitar silencio' : 'Silenciar'}
          >
            <Icono nombre={sonido.silenciado || sonido.volumen === 0 ? 'silencio' : 'volumen'} tamano={18} />
          </button>
          <Deslizador
            valor={sonido.volumen}
            etiqueta="Volumen"
            apagado={sonido.silenciado ?? false}
            oncambio={(v) => {
              if (sonido) sonido.volumen = v;
              volumenEnVivo('volumen', v);
            }}
            ontocando={(t) => (tocando = t)}
          />
          <span class="valor">{sonido.volumen}%</span>
        </div>
      {/if}
    {/if}

    {#if sonido.tieneMicrofono}
      <div class="fila separada">
        <div class="textos">
          <div class="titulo">Micrófono</div>
          <div class="ayuda">
            {sonido.microfonoEnUso
              ? 'Las llamadas usan este micrófono.'
              : 'Al usarlo, el Bluetooth baja la calidad del sonido mientras esté activo.'}
          </div>
        </div>
        {#if sonido.microfonoEnUso}
          <span class="en-uso"><Icono nombre="check" tamano={14} /> En uso</span>
        {:else}
          <button class="boton-vidrio secundario" onclick={() => aplicar({ accion: 'usarMicrofono' })}>
            Usar su micrófono
          </button>
        {/if}
      </div>

      {#if sonido.microfonoEnUso && sonido.volumenMicrofono !== null}
        <div class="control">
          <span class="redondo"><Icono nombre="microfono" tamano={18} /></span>
          <Deslizador
            valor={sonido.volumenMicrofono}
            etiqueta="Volumen del micrófono"
            oncambio={(v) => {
              if (sonido) sonido.volumenMicrofono = v;
              volumenEnVivo('volumenMicrofono', v);
            }}
            ontocando={(t) => (tocando = t)}
          />
          <span class="valor">{sonido.volumenMicrofono}%</span>
        </div>
      {/if}
    {/if}
  </section>
{:else if sonido}
  <section class="panel vidrio">
    <h2>Sonido</h2>
    <p class="ayuda">macOS todavía no lo muestra como salida de sonido ni como micrófono. Si recién se conectó, espera unos segundos.</p>
  </section>
{/if}

<style>
  .panel {
    border-radius: var(--radio-lg);
    padding: 18px 20px;
  }
  h2 {
    margin: 0 0 14px;
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
  }
  .fila {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px 16px;
  }
  .separada {
    margin-top: 20px;
    padding-top: 18px;
    border-top: 1px solid var(--borde);
  }
  .textos {
    flex: 1 1 220px;
    min-width: 0;
  }
  .titulo {
    font-weight: 650;
  }
  .ayuda {
    margin-top: 2px;
    font-size: 12.5px;
    color: var(--texto-2);
    line-height: 1.4;
  }

  .principal {
    flex-shrink: 0;
  }
  .secundario {
    flex-shrink: 0;
    background: var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .secundario:hover {
    background: var(--superficie-hover);
  }
  .en-uso {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 11px 0 8px;
    border-radius: 99px;
    background: color-mix(in srgb, var(--verde) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--verde) 30%, transparent);
    color: var(--verde);
    font-size: 12.5px;
    font-weight: 600;
  }

  .control {
    display: grid;
    grid-template-columns: 40px 1fr 44px;
    align-items: center;
    gap: 12px;
    margin-top: 16px;
  }
  .redondo {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--celeste);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  button.redondo:hover {
    background: var(--superficie-hover);
  }
  .redondo.silenciado {
    color: var(--peligro);
  }
  .valor {
    text-align: right;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
</style>
