<!--
  Cambiar un aparato de la casa: su nombre en Nexo (vacío vuelve al suyo), su habitación y, si es una luz, su forma
  (tira, foco, lámpara de monitor, lámpara). Abajo, en tenue, por dónde se maneja, su modelo y su dirección.
  Lo agregado desde tu red o por Bluetooth (Hue, LIFX, Nanoleaf, Elgato, WLED, Shelly, Tasmota, WiZ, Yeelight, Govee,
  tiras) también se puede quitar de Nexo.
-->
<script lang="ts">
  import { FORMAS_LUZ, tipoDe, viaDe } from '$lib/hogar';
  import type { Aparato, CambioAparato, FormaLuz, Habitacion } from '$lib/tipos-hogar';
  import Dialogo from './Dialogo.svelte';
  import MenuDesplegable from './MenuDesplegable.svelte';

  let {
    aparato,
    habitaciones,
    onguardar,
    onquitar,
    oncerrar,
  }: {
    /** El aparato a cambiar; null cierra el diálogo. */
    aparato: Aparato | null;
    habitaciones: Habitacion[];
    /** Devuelve el motivo si no se pudo. */
    onguardar: (id: string, cambio: CambioAparato) => Promise<string | null>;
    onquitar: (id: string) => Promise<string | null>;
    oncerrar: () => void;
  } = $props();

  const LOCALES = [
    'wiz',
    'yeelight',
    'govee',
    'hue',
    'lifx',
    'shelly',
    'nanoleaf',
    'elgato',
    'wled',
    'tasmota',
    'bleElk',
    'bleTriones',
  ];

  let alias = $state('');
  let habitacion = $state('');
  let forma = $state<FormaLuz | null>(null);
  let error = $state<string | null>(null);
  let ocupado = $state(false);

  // Cada vez que se abre con un aparato, parte de cómo está.
  let abiertoCon: string | null = null;
  $effect(() => {
    const a = aparato;
    if (!a || abiertoCon === a.id) {
      if (!a) abiertoCon = null;
      return;
    }
    abiertoCon = a.id;
    alias = a.alias ?? '';
    habitacion = a.habitacion ?? '';
    forma = a.forma;
    error = null;
  });

  const opcionesHabitacion = $derived([
    ...habitaciones.map((h) => ({ valor: h.id, nombre: h.nombre })),
    { valor: '', nombre: 'Sin cuarto' },
  ]);

  async function guardar() {
    const a = aparato;
    if (!a || ocupado) return;
    const cambio: CambioAparato = {};
    if (alias.trim() !== (a.alias ?? '')) cambio.alias = alias.trim();
    if (habitacion !== (a.habitacion ?? '')) cambio.habitacion = habitacion;
    if (a.clase === 'luz' && forma && forma !== a.forma) cambio.forma = forma;
    if (!Object.keys(cambio).length) {
      oncerrar();
      return;
    }
    ocupado = true;
    error = await onguardar(a.id, cambio);
    ocupado = false;
    if (!error) oncerrar();
  }

  async function quitar() {
    const a = aparato;
    if (!a || ocupado) return;
    ocupado = true;
    error = await onquitar(a.id);
    ocupado = false;
    if (!error) oncerrar();
  }

  const datos = $derived(
    aparato
      ? [viaDe(aparato), aparato.modelo, aparato.via === 'wifi' ? aparato.direccion : null].filter(Boolean).join(' · ')
      : '',
  );
</script>

<Dialogo
  abierto={!!aparato}
  titulo={aparato?.nombre ?? ''}
  descripcion={aparato ? tipoDe(aparato) : ''}
  icono="editar"
  textoConfirmar={ocupado ? 'Guardando…' : 'Guardar'}
  onconfirmar={guardar}
  oncancelar={oncerrar}
>
  {#if aparato}
    <div class="campos">
      <label class="campo">
        <span class="rotulo">Nombre en Nexo</span>
        <input type="text" maxlength="40" placeholder={aparato.nombreOriginal} bind:value={alias} data-autofocus />
        <span class="ayuda">Vacío vuelve a «{aparato.nombreOriginal}».</span>
      </label>
      <div class="campo">
        <span class="rotulo">Habitación</span>
        <MenuDesplegable
          opciones={opcionesHabitacion}
          valor={habitacion}
          etiqueta="Habitación de {aparato.nombre}"
          oncambio={(v) => (habitacion = v)}
        />
      </div>
      {#if aparato.clase === 'luz'}
        <div class="campo">
          <span class="rotulo">Qué es</span>
          <MenuDesplegable
            opciones={FORMAS_LUZ}
            valor={forma ?? 'foco'}
            etiqueta="Qué clase de luz es {aparato.nombre}"
            oncambio={(v) => (forma = v as FormaLuz)}
          />
        </div>
      {/if}
      {#if datos}<p class="datos">{datos}</p>{/if}
      {#if LOCALES.includes(aparato.proveedor)}
        <div class="quitar">
          <button type="button" class="peligro" disabled={ocupado} onclick={quitar}>Quitar de Nexo</button>
          <span class="ayuda">Puedes volver a agregarla con «Buscar aparatos».</span>
        </div>
      {/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>
  {/if}
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
  .ayuda,
  .datos {
    margin: 0;
    font-size: 12px;
    line-height: 17px;
    color: var(--texto-3);
    overflow-wrap: break-word;
  }
  .quitar {
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
