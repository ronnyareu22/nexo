<!--
  Nombrar una habitación (nueva o existente) y, si ya existe, borrarla con confirmación: lo que tenía queda en
  "Sin cuarto". Los dos diálogos van uno tras otro, nunca uno encima del otro.
-->
<script lang="ts">
  import type { Habitacion } from '$lib/tipos-hogar';
  import Dialogo from './Dialogo.svelte';

  let {
    abierto,
    habitacion,
    onguardar,
    onborrar,
    oncerrar,
  }: {
    abierto: boolean;
    /** La habitación a cambiar; null para una nueva. */
    habitacion: Habitacion | null;
    /** Devuelven el motivo si no se pudo. */
    onguardar: (h: Habitacion) => Promise<string | null>;
    onborrar: (id: string) => Promise<string | null>;
    oncerrar: () => void;
  } = $props();

  let nombre = $state('');
  let error = $state<string | null>(null);
  let ocupado = $state(false);
  let paso = $state<'nombre' | 'borrar'>('nombre');

  let estaba = false;
  $effect(() => {
    if (abierto && !estaba) {
      nombre = habitacion?.nombre ?? '';
      error = null;
      paso = 'nombre';
    }
    estaba = abierto;
  });

  async function guardar() {
    if (ocupado) return;
    if (!nombre.trim()) {
      error = 'Ponle un nombre a la habitación.';
      return;
    }
    ocupado = true;
    error = await onguardar({ id: habitacion?.id ?? '', nombre: nombre.trim() });
    ocupado = false;
    if (!error) oncerrar();
  }

  async function borrar() {
    if (!habitacion || ocupado) return;
    ocupado = true;
    error = await onborrar(habitacion.id);
    ocupado = false;
    if (error) paso = 'nombre';
    else oncerrar();
  }
</script>

<Dialogo
  abierto={abierto && paso === 'nombre'}
  titulo={habitacion ? 'Habitación' : 'Nueva habitación'}
  descripcion="Agrupa tus aparatos como están en tu casa."
  icono="casa"
  textoConfirmar={ocupado ? 'Guardando…' : 'Guardar'}
  onconfirmar={guardar}
  oncancelar={oncerrar}
>
  <div class="campos">
    <label class="campo">
      <span class="rotulo">Nombre</span>
      <input type="text" maxlength="40" placeholder="Por ejemplo, Sala" bind:value={nombre} data-autofocus />
    </label>
    {#if habitacion}
      <div class="borrar">
        <button type="button" class="peligro" disabled={ocupado} onclick={() => (paso = 'borrar')}>Borrar habitación…</button>
      </div>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </div>
</Dialogo>

<Dialogo
  abierto={abierto && paso === 'borrar'}
  titulo={`¿Borrar «${habitacion?.nombre ?? ''}»?`}
  descripcion="Lo que tenía queda en «Sin cuarto». Tus aparatos y grupos siguen igual."
  icono="papelera"
  tono="peligro"
  textoConfirmar="Borrar"
  onconfirmar={borrar}
  oncancelar={() => (paso = 'nombre')}
/>

<style>
  .campos {
    display: grid;
    gap: 12px;
    text-align: left;
  }
  .campo {
    display: grid;
    gap: 6px;
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
  .borrar {
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
