<!-- Automatizaciones: lo que Nexo hace por su cuenta, agrupado por aparato, y el atajo de brillo de las pantallas.
     Cada aparato muestra solo las reglas que le sirven (el sonido, solo en los de audio; la batería, en los que la informan). -->
<script lang="ts">
  import { untrack } from 'svelte';
  import { abrirAjustesBluetooth, guardarAutomatico, leerAutomatico } from '$lib/api';
  import { esMac } from '$lib/api-sistema';
  import type { Automatico, Dispositivo, Tipo } from '$lib/tipos';
  import { baterias, esDeSonido, nombreTipo } from '$lib/tipos';
  import Figura from '$lib/componentes/Figura.svelte';
  import Ilustracion from '$lib/componentes/Ilustracion.svelte';
  import Interruptor from '$lib/componentes/Interruptor.svelte';
  import PanelAtajos from '$lib/componentes/PanelAtajos.svelte';
  import NexoBot from '$lib/componentes/NexoBot.svelte';

  let { dispositivos, onerror }: { dispositivos: Dispositivo[]; onerror: (texto: string) => void } = $props();

  // Tipos que suelen informar su batería por Bluetooth; los demás, solo si ya la informaron.
  const conBateria: Tipo[] = ['audifonos', 'diadema', 'teclado', 'mouse', 'parlante'];
  const avisaBateria = (d: Dispositivo) => conBateria.includes(d.tipo) || baterias(d).length > 0;
  const tieneReglas = (d: Dispositivo) => esDeSonido(d) || avisaBateria(d);

  const conReglas = $derived(dispositivos.filter(tieneReglas));

  let reglas = $state<Record<string, Automatico>>({});
  // Aparatos cuyas reglas no se pudieron leer: no se muestran interruptores inventados.
  let sinLeer = $state<Record<string, boolean>>({});

  // Lee las reglas de cada aparato nuevo (no en cada actualización de la lista).
  const ids = $derived(conReglas.map((d) => d.id).join('\n'));
  $effect(() => {
    const lista = ids ? ids.split('\n') : [];
    untrack(() => cargar(lista.filter((id) => !(id in reglas))));
  });

  async function cargar(lista: string[]) {
    let avisado = false;
    await Promise.all(
      lista.map((id) =>
        leerAutomatico(id)
          .then((a) => {
            reglas[id] = a;
            delete sinLeer[id];
          })
          .catch((e) => {
            sinLeer[id] = true;
            if (!avisado) onerror(String(e));
            avisado = true;
          }),
      ),
    );
  }

  async function alternar(id: string, campo: keyof Automatico) {
    const antes = reglas[id];
    if (!antes) return;
    const nuevo = { ...antes, [campo]: !antes[campo] };
    reglas[id] = nuevo;
    try {
      reglas[id] = await guardarAutomatico(id, nuevo);
    } catch (e) {
      reglas[id] = antes;
      onerror(String(e));
    }
  }

  function abrirBluetooth() {
    abrirAjustesBluetooth().catch((e) => onerror(String(e)));
  }
</script>

<div class="vista">
  <header class="cabecera">
    <h1>Automatizaciones</h1>
    <p>Lo que Nexo hace por su cuenta con tus aparatos.</p>
  </header>

  {#if dispositivos.length === 0}
    <section class="panel vidrio vacio">
      <NexoBot expresion="sin-dispositivos" tamano={120} />
      <div class="vacio__textos">
        <h2>Todavía no hay dispositivos</h2>
        <p>Empareja tus audífonos, teclado o mouse en los ajustes de Bluetooth y aquí eliges qué hace Nexo con ellos.</p>
        <button class="boton-vidrio boton" type="button" onclick={abrirBluetooth}>Abrir ajustes de Bluetooth</button>
      </div>
    </section>
  {:else if conReglas.length === 0}
    <section class="panel vidrio sin-reglas">
      <p>Las reglas aparecen para audífonos, parlantes, teclados y mouse.</p>
    </section>
  {/if}

  <div class="grupos">
    {#each conReglas as d (d.id)}
      {@const r = reglas[d.id]}
      <section class="panel vidrio" aria-label={d.nombre}>
        <div class="cabeza">
          <span class="miniatura"><Figura dispositivo={d} /></span>
          <div class="cabeza__textos">
            <h2>{d.nombre}</h2>
            <p>
              {nombreTipo[d.tipo]}
              <span class="estado" class:conectado={d.conectado}>· {d.conectado ? 'Conectado' : 'Sin conectar'}</span>
            </p>
          </div>
        </div>

        {#if r}
          {#if esDeSonido(d)}
            <div class="fila">
              <div class="textos">
                <div class="titulo">Usar para el sonido al conectarse</div>
                <div class="ayuda">Cuando se conecta, el sonido sale por {d.nombre}.</div>
              </div>
              <Interruptor
                encendido={r.usarSalidaAlConectar}
                etiqueta="Usar {d.nombre} para el sonido al conectarse"
                oncambio={() => alternar(d.id, 'usarSalidaAlConectar')}
              />
            </div>
          {/if}
          {#if avisaBateria(d)}
            <div class="fila">
              <div class="textos">
                <div class="titulo">Avisarme con batería baja (20 %)</div>
                <div class="ayuda">Te llega una notificación {esMac ? 'de macOS' : 'de Windows'}.</div>
              </div>
              <Interruptor
                encendido={r.avisarBateriaBaja}
                etiqueta="Avisarme cuando {d.nombre} tenga batería baja"
                oncambio={() => alternar(d.id, 'avisarBateriaBaja')}
              />
            </div>
          {/if}
        {:else if sinLeer[d.id]}
          <p class="fila ayuda">No se pudieron leer sus reglas.</p>
        {:else}
          <div class="fila cargando" aria-busy="true"><span class="linea"></span><span class="solo-lector">Cargando reglas…</span></div>
        {/if}
      </section>
    {/each}

    {#if esMac}
      <section class="panel vidrio" aria-label="Pantallas">
        <div class="cabeza">
          <span class="miniatura"><Ilustracion tipo="monitor" encendido /></span>
          <div class="cabeza__textos">
            <h2>Pantallas</h2>
            <p>Atajo de brillo</p>
          </div>
        </div>
        <PanelAtajos {onerror} dentro />
      </section>
    {/if}
  </div>
</div>

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

  .grupos {
    display: grid;
    gap: 16px;
    align-items: start;
  }
  /* Con la ventana ancha, dos columnas para no dejar espacio muerto. */
  @media (min-width: 1360px) {
    .grupos {
      grid-template-columns: 1fr 1fr;
    }
  }

  .panel {
    padding: 14px 20px 4px;
    border-radius: var(--radio-lg);
    min-width: 0;
  }

  .cabeza {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-bottom: 12px;
  }
  .miniatura {
    display: grid;
    flex-shrink: 0;
    width: 48px;
    height: 48px;
    padding: 4px;
    border-radius: 12px;
    background: var(--superficie-elevada);
  }
  .cabeza__textos {
    min-width: 0;
  }
  .cabeza h2 {
    margin: 0;
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
    overflow-wrap: anywhere;
  }
  .cabeza p {
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .estado {
    color: var(--texto-3);
  }
  .estado.conectado {
    color: var(--exito);
  }

  .fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
    border-top: 1px solid var(--borde);
  }
  .textos {
    min-width: 0;
  }
  p.fila {
    margin: 0;
  }
  .titulo {
    font-weight: 600;
  }
  .ayuda {
    margin-top: 2px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }

  /* Mientras llegan las reglas: una línea quieta del alto de una fila. */
  .cargando {
    min-height: 62px;
  }
  .linea {
    width: 55%;
    height: 12px;
    border-radius: 6px;
    background: var(--superficie-elevada);
  }

  .vacio {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 20px 24px;
  }
  .vacio__textos {
    min-width: 0;
  }
  .vacio h2 {
    margin: 0;
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
  }
  .vacio p {
    margin: 4px 0 14px;
    max-width: 46ch;
    color: var(--texto-2);
  }
  .sin-reglas {
    padding: 16px 20px;
  }
  .sin-reglas p {
    margin: 0;
    color: var(--texto-2);
  }

  .boton {
    min-height: 34px;
    padding: 0 14px;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .boton:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
</style>
