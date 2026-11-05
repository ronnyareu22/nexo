<!--
  Ajustes › Casa, en filas como el resto de Ajustes. Primero lo de tu red, sin cuentas:
  - "Buscar aparatos" y "Tu red" (llevan a Casa con la búsqueda o con Tu red abiertas).
  - Home Assistant (opcional): los pasos para crear un token de larga duración, la dirección (la que trae la
    búsqueda, si la hubo) y el token (al llavero del sistema). Gratis y en tu red: nunca por Nabu Casa.
  - "Otras cuentas (opcional)", cerrada de entrada (se abre sola si Tuya tiene un aviso o si se llega desde "Ver
    opciones" de un aparato Tuya): Tuya / Smart Life, con los pasos para crear el proyecto en iot.tuya.com y vincular
    la app, Access ID, Access Secret (oculto, nunca se vuelve a mostrar: va al llavero) y el centro de datos, con
    "Detectar". Cerrada, su línea dice si Tuya está conectada.
  - Conectadas: lo que se ve de cada una, el error en palabras si lo hay y "Desconectar" con confirmación.
  Lleva el id "ajustes-casa": Ajustes baja hasta aquí cuando se llega desde Casa o Mi setup.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { conectarHa, conectarTuya, desconectarHa, desconectarTuya, leerCasa } from '$lib/api-hogar';
  import type { EstadoCasa } from '$lib/tipos-hogar';
  import Icono from './Icono.svelte';
  import MenuDesplegable from './MenuDesplegable.svelte';
  import Dialogo from './Dialogo.svelte';

  let { onerror, onir }: { onerror: (texto: string) => void; onir?: (seccion: string, id?: string) => void } = $props();

  let estado = $state<EstadoCasa | null>(null);

  // --- Otras cuentas (Tuya): cerradas de entrada ---
  let otras = $state(false);

  // --- Tuya ---
  let formTuya = $state(false);
  let accessId = $state('');
  let secreto = $state('');
  let region = $state('auto');
  let conectandoTuya = $state(false);
  let errorTuya = $state<string | null>(null);

  // --- Home Assistant ---
  let formHa = $state(false);
  let url = $state('');
  let token = $state('');
  let conectandoHa = $state(false);
  let errorHa = $state<string | null>(null);

  let preguntando = $state<'tuya' | 'ha' | null>(null);

  const tuya = $derived(estado?.tuya ?? null);
  const ha = $derived(estado?.homeAssistant ?? null);
  const cuantos = (p: string) => estado?.aparatos.filter((a) => a.proveedor === p).length ?? 0;
  const aparatos = (n: number) => `${n} ${n === 1 ? 'aparato' : 'aparatos'}`;

  /** La línea de "Otras cuentas": si Tuya está conectada, se dice aunque esté cerrada. */
  const resumenOtras = $derived(
    tuya
      ? estado?.errorTuya
        ? 'Tuya / Smart Life conectada · tiene un aviso'
        : `Tuya / Smart Life conectada · ${aparatos(cuantos('tuya'))}`
      : 'Tuya / Smart Life, para lo que solo funciona con su nube. Lo de tu red no la necesita.',
  );

  const regiones = $derived([
    { valor: 'auto', nombre: 'Detectar', detalle: 'Prueba cada centro de datos' },
    ...(estado?.regiones ?? []).map(([valor, nombre]) => ({ valor, nombre })),
  ]);

  async function conectarCuentaTuya(e: SubmitEvent) {
    e.preventDefault();
    if (conectandoTuya) return;
    if (!accessId.trim() || !secreto.trim()) {
      errorTuya = 'Pega el Access ID y el Access Secret de tu proyecto.';
      return;
    }
    conectandoTuya = true;
    errorTuya = null;
    try {
      estado = await conectarTuya(accessId.trim(), secreto.trim(), region);
      secreto = '';
      accessId = '';
      formTuya = false;
    } catch (err) {
      errorTuya = String(err);
    }
    conectandoTuya = false;
  }

  async function conectarCuentaHa(e: SubmitEvent) {
    e.preventDefault();
    if (conectandoHa) return;
    if (!url.trim() || !token.trim()) {
      errorHa = 'Escribe la dirección y pega el token.';
      return;
    }
    conectandoHa = true;
    errorHa = null;
    try {
      estado = await conectarHa(url.trim(), token.trim());
      token = '';
      formHa = false;
      try {
        sessionStorage.removeItem('nexo-ha-url');
      } catch {
        // Sin almacenamiento: no había nada guardado.
      }
    } catch (err) {
      errorHa = String(err);
    }
    conectandoHa = false;
  }

  async function desconectar() {
    const cual = preguntando;
    preguntando = null;
    try {
      estado = cual === 'tuya' ? await desconectarTuya() : await desconectarHa();
      if (cual === 'tuya') formTuya = false;
    } catch (err) {
      onerror(String(err));
    }
  }

  onMount(() => {
    // Desde "Ver opciones" de un aparato Tuya: "Otras cuentas" abierta y Tuya lista para conectar.
    let pidioTuya = false;
    try {
      pidioTuya = sessionStorage.getItem('nexo-abrir-tuya') === '1';
      sessionStorage.removeItem('nexo-abrir-tuya');
    } catch {
      // Sin almacenamiento: se abre a mano.
    }
    leerCasa(false)
      .then((e) => {
        estado = e;
        otras = pidioTuya || !!e.errorTuya;
        formTuya = pidioTuya && !e.tuya;
      })
      .catch((e) => onerror(String(e)));
    // La dirección de un Home Assistant encontrado al buscar.
    try {
      const encontrada = sessionStorage.getItem('nexo-ha-url');
      if (encontrada) {
        url = encontrada;
        formHa = true;
      }
    } catch {
      // Sin almacenamiento: se escribe a mano.
    }
  });
</script>

<section class="panel vidrio" id="ajustes-casa" aria-labelledby="ajustes-casa-titulo">
  <h2 class="rotulo" id="ajustes-casa-titulo">Casa</h2>

  <div class="fila">
    <div class="textos">
      <div class="titulo">Buscar aparatos</div>
      <div class="ayuda">
        Luces y enchufes de tu Wi-Fi (Philips Hue, LIFX, Nanoleaf, Elgato, WLED, Shelly, WiZ, Yeelight, Govee, Tasmota),
        tiras Bluetooth y Home Assistant. Sin cuentas; tarda unos segundos.
      </div>
    </div>
    <button class="boton-vidrio boton" type="button" onclick={() => onir?.('casa', 'buscar')}>
      <Icono nombre="buscar" tamano={15} />
      Buscar
    </button>
  </div>

  <div class="fila separada">
    <div class="textos">
      <div class="titulo">Tu red</div>
      <div class="ayuda">Todo lo que está conectado a tu Wi-Fi: qué es, su IP y si Nexo lo puede manejar.</div>
    </div>
    <button class="boton-vidrio boton" type="button" onclick={() => onir?.('casa', 'red')}>
      <Icono nombre="red" tamano={15} />
      Ver tu red
    </button>
  </div>

  {#if !estado}
    <div class="cuenta cargando" aria-busy="true"><span class="linea"></span><span class="solo-lector">Leyendo tus cuentas…</span></div>
  {:else}
  <!-- ---------- Home Assistant ---------- -->
  <div class="cuenta">
    <div class="fila">
      <div class="textos">
        <div class="titulo con-punto">
          <span class="punto-estado" class:ok={!!ha && !estado?.errorHa} class:aviso={!!ha && !!estado?.errorHa} aria-hidden="true"></span>
          <span>Home Assistant <span class="opcional">(opcional)</span></span>
        </div>
        <div class="ayuda">
          {ha
            ? `Conectado · ${aparatos(cuantos('homeAssistant'))} · ${ha.url}`
            : 'La tele, la puerta, cortinas y todo lo que ya tengas en Home Assistant.'}
        </div>
      </div>
      {#if ha}
        <button class="boton-vidrio boton peligro" type="button" onclick={() => (preguntando = 'ha')}>Desconectar</button>
      {:else if !formHa}
        <button class="boton-vidrio boton" type="button" onclick={() => (formHa = true)}>Conectar</button>
      {/if}
    </div>

    {#if ha}
      {#if estado?.errorHa}<p class="error-cuenta" role="status"><Icono nombre="alerta" tamano={15} /><span>{estado.errorHa}</span></p>{/if}
    {:else if formHa}
      <ol class="pasos">
        <li>En Home Assistant, abre tu <b>perfil › Seguridad</b>.</li>
        <li>En <b>Tokens de acceso de larga duración</b>, crea uno llamado Nexo y cópialo: se muestra una sola vez.</li>
        <li>
          Escribe su dirección: <b>http://homeassistant.local:8123</b>, o su IP de Tailscale para usarlo desde la oficina.
        </li>
      </ol>
      <p class="nota">Home Assistant es gratis y vive en tu red. Nexo se conecta directo, nunca por Nabu Casa.</p>
      <form class="formulario" onsubmit={conectarCuentaHa}>
        <label class="campo">
          <span class="campo__rotulo">Dirección</span>
          <input type="text" bind:value={url} autocomplete="off" spellcheck="false" autocapitalize="off" placeholder="http://homeassistant.local:8123" />
        </label>
        <label class="campo">
          <span class="campo__rotulo">Token</span>
          <input type="password" bind:value={token} autocomplete="off" spellcheck="false" placeholder="Se guarda en el llavero" />
        </label>
        {#if errorHa}<p class="error-campo" role="alert">{errorHa}</p>{/if}
        <div class="enviar">
          <button class="boton-primario" type="submit" disabled={conectandoHa} aria-busy={conectandoHa}>
            {conectandoHa ? 'Conectando…' : 'Conectar'}
          </button>
        </div>
      </form>
    {/if}
  </div>

  <!-- ---------- Otras cuentas (opcional): Tuya / Smart Life ---------- -->
  <div class="cuenta">
    <div class="fila">
      <div class="textos">
        <div class="titulo">Otras cuentas <span class="opcional">(opcional)</span></div>
        <div class="ayuda">{resumenOtras}</div>
      </div>
      <button
        class="boton-vidrio boton"
        type="button"
        aria-expanded={otras}
        aria-controls="ajustes-otras-cuentas"
        onclick={() => (otras = !otras)}
      >
        {otras ? 'Ocultar' : 'Mostrar'}
        <span class="giro" class:abierta={otras}><Icono nombre="chevron-abajo" tamano={15} /></span>
      </button>
    </div>
    {#if otras}
      <div class="cuenta sub" id="ajustes-otras-cuentas">
        <div class="fila">
          <div class="textos">
            <div class="titulo con-punto">
              <span class="punto-estado" class:ok={!!tuya && !estado?.errorTuya} class:aviso={!!tuya && !!estado?.errorTuya} aria-hidden="true"></span>
              <span>Tuya / Smart Life</span>
            </div>
            <div class="ayuda">
              {tuya
                ? `Conectada · ${aparatos(cuantos('tuya'))} · ${tuya.regionNombre || tuya.region}`
                : 'Los aparatos de la app Smart Life o Tuya, por su nube y con tu cuenta.'}
            </div>
          </div>
          {#if tuya}
            <button class="boton-vidrio boton peligro" type="button" onclick={() => (preguntando = 'tuya')}>Desconectar</button>
          {:else if !formTuya}
            <button class="boton-vidrio boton" type="button" onclick={() => (formTuya = true)}>Conectar</button>
          {/if}
        </div>

        {#if tuya}
          <dl class="datos">
            <div><dt>Access ID</dt><dd class="clave">{tuya.accessId}</dd></div>
            <div><dt>Access Secret</dt><dd>Guardado en el llavero de este equipo</dd></div>
          </dl>
          {#if estado?.errorTuya}<p class="error-cuenta" role="status"><Icono nombre="alerta" tamano={15} /><span>{estado.errorTuya}</span></p>{/if}
        {:else if formTuya}
          <ol class="pasos">
            <li>Entra a <b>iot.tuya.com</b> con una cuenta (es aparte de la de la app).</li>
            <li>
              En <b>Cloud › Development</b>, crea un proyecto <b>Smart Home</b> con el centro de datos de tu cuenta. En Smart
              Life lo ves en Yo › Configuración › Cuenta y seguridad › Región.
            </li>
            <li>
              En el proyecto, abre <b>Devices › Link Tuya App Account › Add App Account</b> y escanea el código con Smart
              Life (Yo › ícono de escanear).
            </li>
            <li>En <b>Overview</b>, copia el Access ID y el Access Secret.</li>
            <li>Pégalos aquí y toca Conectar.</li>
          </ol>
          <p class="nota">
            La prueba gratis de Tuya dura cerca de un mes y se renueva gratis en iot.tuya.com. Da unas 26 000 consultas al mes:
            Nexo pregunta solo mientras miras tu casa.
          </p>
          <form class="formulario" onsubmit={conectarCuentaTuya}>
            <label class="campo">
              <span class="campo__rotulo">Access ID</span>
              <input type="text" bind:value={accessId} autocomplete="off" spellcheck="false" autocapitalize="off" placeholder="Client ID del proyecto" />
            </label>
            <label class="campo">
              <span class="campo__rotulo">Access Secret</span>
              <input type="password" bind:value={secreto} autocomplete="off" spellcheck="false" placeholder="Se guarda en el llavero" />
            </label>
            <div class="campo">
              <span class="campo__rotulo">Centro de datos</span>
              <MenuDesplegable opciones={regiones} valor={region} etiqueta="Centro de datos de tu cuenta" oncambio={(v) => (region = v)} />
            </div>
            {#if errorTuya}<p class="error-campo" role="alert">{errorTuya}</p>{/if}
            <div class="enviar">
              <button class="boton-primario" type="submit" disabled={conectandoTuya} aria-busy={conectandoTuya}>
                {conectandoTuya ? 'Conectando…' : 'Conectar'}
              </button>
              {#if region === 'auto'}<span class="ayuda">Detectar prueba cada centro de datos: puede tardar unos 20 segundos.</span>{/if}
            </div>
          </form>
        {/if}
      </div>
    {/if}
  </div>
  {/if}
</section>

<Dialogo
  abierto={preguntando !== null}
  titulo={preguntando === 'tuya' ? '¿Desconectar Tuya / Smart Life?' : '¿Desconectar Home Assistant?'}
  descripcion={preguntando === 'tuya'
    ? 'Nexo borra la clave del llavero y deja de mostrar esos aparatos. En la app siguen igual.'
    : 'Nexo borra el token del llavero y deja de mostrar esos aparatos. En Home Assistant siguen igual.'}
  icono="desconectar"
  tono="peligro"
  textoConfirmar="Desconectar"
  onconfirmar={desconectar}
  oncancelar={() => (preguntando = null)}
/>

<style>
  .panel {
    padding: 14px 20px 8px;
    border-radius: var(--radio-lg);
    min-width: 0;
  }
  .rotulo {
    margin: 0 0 4px;
  }
  .cuenta {
    display: grid;
    gap: 10px;
    padding-bottom: 12px;
    border-top: 1px solid var(--borde);
  }
  /* Tuya, dentro de "Otras cuentas": su raya va un poco más adentro. */
  .cuenta.sub {
    margin-left: 12px;
    padding-bottom: 0;
  }
  .separada {
    border-top: 1px solid var(--borde);
  }
  .opcional {
    font-weight: 500;
    color: var(--texto-3);
  }
  .giro {
    display: inline-grid;
    transition: transform var(--dur-color) ease;
  }
  .giro.abierta {
    transform: rotate(180deg);
  }
  .fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 60px;
    padding: 12px 0;
  }
  .textos {
    flex: 1;
    min-width: 0;
  }
  .titulo {
    font-weight: 600;
  }
  .ayuda {
    margin-top: 2px;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
    overflow-wrap: anywhere;
  }
  .con-punto {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .punto-estado {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--texto-3);
  }
  .punto-estado.ok {
    background: var(--exito);
  }
  .punto-estado.aviso {
    background: var(--aviso);
  }
  .boton {
    flex-shrink: 0;
    min-height: 32px;
    padding: 0 14px;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .boton:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .boton.peligro {
    color: var(--error);
  }

  /* Mientras llegan las cuentas: una línea quieta del alto de una fila. */
  .cargando {
    min-height: 60px;
    align-content: center;
  }
  .linea {
    width: 55%;
    height: 12px;
    border-radius: 6px;
    background: var(--superficie-elevada);
  }
  .datos {
    display: grid;
    gap: 6px;
    margin: -4px 0 0;
    padding: 10px 12px;
    border-radius: var(--radio-md);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .datos div {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 12px;
    font-size: 12.5px;
    line-height: 18px;
  }
  .datos dt {
    flex: 0 0 110px;
    color: var(--texto-3);
  }
  .datos dd {
    flex: 1 1 160px;
    min-width: 0;
    margin: 0;
    color: var(--texto-2);
    overflow-wrap: anywhere;
  }
  .clave {
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.01em;
  }
  .error-cuenta {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--aviso);
  }
  .error-cuenta :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
  }

  /* Los pasos: números en círculos suaves, texto que pasa de línea. */
  .pasos {
    display: grid;
    gap: 8px;
    margin: -2px 0 0;
    padding: 0;
    list-style: none;
    counter-reset: paso;
  }
  .pasos li {
    position: relative;
    padding-left: 30px;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto-2);
    counter-increment: paso;
    overflow-wrap: break-word;
  }
  .pasos li::before {
    content: counter(paso);
    position: absolute;
    left: 0;
    top: 0;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--elegido-bg);
    color: var(--elegido-fg);
    font-size: 11px;
    font-weight: 700;
  }
  .pasos b {
    color: var(--texto);
    font-weight: 600;
  }
  .nota {
    margin: 0;
    font-size: 12px;
    line-height: 17px;
    color: var(--texto-3);
  }

  .formulario {
    display: grid;
    gap: 10px;
    padding: 12px;
    border-radius: var(--radio-md);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .campo {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .campo__rotulo {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--texto-2);
  }
  .campo input {
    width: 100%;
    height: 38px;
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
  .error-campo {
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--error);
    overflow-wrap: break-word;
  }
  .enviar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }
  .enviar .ayuda {
    flex: 1 1 200px;
    margin: 0;
  }
</style>
