<!--
  Un grupo de aparatos con un nombre ("Escritorio" = la tira y la barra del monitor): así se encienden juntos desde
  Casa, el menú del ícono, las escenas y, más adelante, la voz ("prende la luz del escritorio").
  Nombre, los aparatos (fichas que se marcan) y, si se quiere, su habitación. Borrar no cambia los aparatos.
-->
<script lang="ts">
  import { tipoDe } from '$lib/hogar';
  import type { Aparato, Grupo, Habitacion } from '$lib/tipos-hogar';
  import Dialogo from './Dialogo.svelte';
  import Icono from './Icono.svelte';
  import MenuDesplegable from './MenuDesplegable.svelte';

  let {
    abierto,
    grupo,
    aparatos,
    habitaciones,
    onguardar,
    onborrar,
    oncerrar,
  }: {
    abierto: boolean;
    /** El grupo a cambiar; null para uno nuevo. */
    grupo: Grupo | null;
    /** Los aparatos que Nexo maneja (los que se pueden juntar). */
    aparatos: Aparato[];
    habitaciones: Habitacion[];
    /** Devuelven el motivo si no se pudo. */
    onguardar: (g: Grupo) => Promise<string | null>;
    onborrar: (id: string) => Promise<string | null>;
    oncerrar: () => void;
  } = $props();

  let nombre = $state('');
  let elegidos = $state<string[]>([]);
  let habitacion = $state('');
  let error = $state<string | null>(null);
  let ocupado = $state(false);

  // Al abrir, parte del grupo (o de uno vacío).
  let estaba = false;
  $effect(() => {
    if (abierto && !estaba) {
      nombre = grupo?.nombre ?? '';
      elegidos = [...(grupo?.aparatos ?? [])];
      habitacion = grupo?.habitacion ?? '';
      error = null;
    }
    estaba = abierto;
  });

  function alternar(id: string) {
    elegidos = elegidos.includes(id) ? elegidos.filter((x) => x !== id) : [...elegidos, id];
  }

  async function guardar() {
    if (ocupado) return;
    if (!nombre.trim()) {
      error = 'Ponle un nombre al grupo.';
      return;
    }
    if (!elegidos.length) {
      error = 'Elige al menos un aparato.';
      return;
    }
    ocupado = true;
    error = await onguardar({ id: grupo?.id ?? '', nombre: nombre.trim(), aparatos: elegidos, habitacion: habitacion || null });
    ocupado = false;
    if (!error) oncerrar();
  }

  async function borrar() {
    if (!grupo || ocupado) return;
    ocupado = true;
    error = await onborrar(grupo.id);
    ocupado = false;
    if (!error) oncerrar();
  }

  const opcionesHabitacion = $derived([
    { valor: '', nombre: 'En ninguna' },
    ...habitaciones.map((h) => ({ valor: h.id, nombre: h.nombre })),
  ]);
</script>

<Dialogo
  {abierto}
  titulo={grupo ? 'Editar grupo' : 'Nuevo grupo'}
  descripcion="Se encienden y se apagan juntos, también desde el menú del ícono y las escenas."
  icono="bombilla"
  textoConfirmar={ocupado ? 'Guardando…' : 'Guardar'}
  onconfirmar={guardar}
  oncancelar={oncerrar}
>
  <div class="campos">
    <label class="campo">
      <span class="rotulo">Nombre</span>
      <input type="text" maxlength="40" placeholder="Por ejemplo, Escritorio" bind:value={nombre} data-autofocus />
    </label>
    <fieldset class="campo">
      <legend class="rotulo">Aparatos</legend>
      {#if aparatos.length}
        <div class="fichas">
          {#each aparatos as a (a.id)}
            {@const marcado = elegidos.includes(a.id)}
            <button type="button" class="ficha" class:marcado aria-pressed={marcado} onclick={() => alternar(a.id)}>
              {#if marcado}<Icono nombre="check" tamano={14} />{/if}
              <span class="ficha__textos">
                <span>{a.nombre}</span>
                <span class="ficha__tipo">{tipoDe(a)}</span>
              </span>
            </button>
          {/each}
        </div>
      {:else}
        <p class="ayuda">Todavía no hay aparatos que Nexo maneje.</p>
      {/if}
    </fieldset>
    <div class="campo">
      <span class="rotulo">Habitación</span>
      <MenuDesplegable opciones={opcionesHabitacion} valor={habitacion} etiqueta="Habitación del grupo" oncambio={(v) => (habitacion = v)} />
    </div>
    {#if grupo}
      <div class="borrar">
        <button type="button" class="peligro" disabled={ocupado} onclick={borrar}>Borrar grupo</button>
        <span class="ayuda">Los aparatos no cambian.</span>
      </div>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </div>
</Dialogo>

<style>
  .campos {
    display: grid;
    gap: 12px;
    text-align: left;
  }
  .campo {
    display: grid;
    gap: 6px;
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }
  .campo legend {
    padding: 0;
    margin-bottom: 6px;
  }
  .campo input {
    width: 100%;
    height: 40px;
    padding: 0 12px;
    border: 1px solid var(--borde-fuerte);
    border-radius: var(--radio-sm);
    outline: none;
    background: var(--superficie);
    color: var(--texto);
    font: inherit;
    font-size: 14px;
    user-select: text;
    -webkit-user-select: text;
    transition: border-color var(--dur-color) ease;
  }
  .campo input:focus {
    border-color: var(--primario);
  }
  .campo input::placeholder {
    color: var(--texto-3);
  }
  /* Las fichas de los aparatos: si son muchas, se desplazan dentro del diálogo. */
  .fichas {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    max-height: 220px;
    overflow-y: auto;
    padding: 2px;
  }
  .ficha {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
    padding: 4px 12px;
    border: 1px solid transparent;
    border-radius: 12px;
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font-size: 12.5px;
    font-weight: 600;
    text-align: left;
    transition: background-color var(--dur-color) ease;
  }
  .ficha:hover:not(.marcado) {
    background: var(--superficie-hover);
  }
  .ficha.marcado {
    background: var(--elegido-bg);
    border-color: var(--elegido-borde);
    box-shadow: none;
    color: var(--elegido-fg);
  }
  .ficha__textos {
    display: grid;
    line-height: 16px;
  }
  .ficha__tipo {
    font-size: 11.5px;
    font-weight: 500;
    color: var(--texto-3);
  }
  .ayuda {
    margin: 0;
    font-size: 12px;
    line-height: 17px;
    color: var(--texto-3);
  }
  .borrar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    padding-top: 10px;
    border-top: 1px solid var(--borde);
  }
  .peligro {
    min-height: 32px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--radio-sm);
    background: transparent;
    color: var(--error);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .peligro:hover:not(:disabled) {
    background: color-mix(in srgb, var(--error) 10%, transparent);
  }
  .error {
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--error);
  }
</style>
