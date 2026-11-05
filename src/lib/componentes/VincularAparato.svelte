<!--
  Vincular algo encontrado (en "Buscar aparatos" y en "Tu red"), en un diálogo corto con una sola instrucción.
  - Con botón (Hue, Nanoleaf): apenas se abre, Nexo pregunta cada 2 s, hasta 40 s, si ya se presionó. Una línea que
    se llena una sola vez y el tiempo que queda. Si no llega: "No vi el botón presionado." y Reintentar.
  - Con contraseña (Shelly, Tasmota): el campo y Agregar; si no es, el motivo ahí mismo para probar de nuevo.
  - Listo: lo que dice Nexo ("Agregué 3 luces del puente Hue.").
  Cancelar, Escape o un clic fuera cortan todo al instante: no queda nada preguntando.
-->
<script lang="ts">
  import { onDestroy, tick, untrack } from 'svelte';
  import { vincularAparato } from '$lib/api-hogar';
  import type { EstadoCasa, Hallazgo } from '$lib/tipos-hogar';
  import Dialogo from './Dialogo.svelte';

  let {
    hallazgo,
    onestado,
    onvinculado,
    oncerrar,
  }: {
    /** Lo que se vincula; null cierra el diálogo. */
    hallazgo: Hallazgo | null;
    /** La casa cambió (cada respuesta trae cómo quedó). */
    onestado: (e: EstadoCasa) => void;
    /** Ya quedó en Nexo (para marcarlo en la lista). */
    onvinculado?: (id: string) => void;
    oncerrar: () => void;
  } = $props();

  const ESPERA = 40_000;
  const CADA = 2_000;

  type Fase = 'esperando' | 'agotado' | 'error' | 'clave' | 'enviando' | 'listo';

  // Lo que se muestra queda aunque `hallazgo` pase a null: así el fundido de salida no se vacía.
  let actual = $state<Hallazgo | null>(null);
  let fase = $state<Fase>('clave');
  let clave = $state('');
  let error = $state<string | null>(null);
  let mensaje = $state('');
  let quedan = $state(ESPERA / 1000);
  let lleno = $state(false);
  let campo = $state<HTMLInputElement | null>(null);

  const paso = $derived(actual?.manejo.tipo === 'vincular' ? actual.manejo.paso : '');
  const uid = $props.id();

  // Cada espera lleva su vuelta: al cancelar o reintentar, lo que llegue tarde se ignora.
  let vuelta = 0;
  let siguiente: ReturnType<typeof setTimeout> | undefined;
  let reloj: ReturnType<typeof setInterval> | undefined;

  function parar() {
    vuelta++;
    clearTimeout(siguiente);
    clearInterval(reloj);
    siguiente = undefined;
    reloj = undefined;
  }

  /** Con botón: pregunta enseguida y luego cada 2 s, hasta 40 s. */
  function esperarBoton() {
    parar();
    const mia = vuelta;
    const inicio = Date.now();
    fase = 'esperando';
    error = null;
    quedan = ESPERA / 1000;
    lleno = false;
    // Un solo llenado de 40 s: se pone en marcha en el cuadro siguiente.
    requestAnimationFrame(() => requestAnimationFrame(() => mia === vuelta && (lleno = true)));
    reloj = setInterval(() => {
      quedan = Math.max(0, Math.ceil((ESPERA - (Date.now() - inicio)) / 1000));
    }, 1000);
    preguntar(mia, inicio);
  }

  async function preguntar(mia: number, inicio: number) {
    const h = actual;
    if (!h || mia !== vuelta) return;
    try {
      const r = await vincularAparato(h.id);
      if (mia !== vuelta) return;
      onestado(r.estado);
      if (r.listo) {
        terminar(r.mensaje, h.id);
        return;
      }
    } catch (e) {
      if (mia !== vuelta) return;
      parar();
      error = String(e);
      fase = 'error';
      return;
    }
    if (Date.now() - inicio + CADA >= ESPERA) {
      parar();
      fase = 'agotado';
      return;
    }
    siguiente = setTimeout(() => preguntar(mia, inicio), CADA);
  }

  /** Con contraseña: un solo intento por toque. */
  async function enviarClave() {
    const h = actual;
    if (!h || fase === 'enviando') return;
    if (!clave) {
      error = 'Escribe la contraseña.';
      campo?.focus();
      return;
    }
    parar();
    const mia = vuelta;
    fase = 'enviando';
    error = null;
    try {
      const r = await vincularAparato(h.id, clave);
      if (mia !== vuelta) return;
      onestado(r.estado);
      if (r.listo) {
        terminar(r.mensaje, h.id);
        return;
      }
      error = r.mensaje;
    } catch (e) {
      if (mia !== vuelta) return;
      error = String(e);
    }
    fase = 'clave';
    await tick();
    campo?.focus();
    campo?.select();
  }

  function terminar(texto: string, id: string) {
    parar();
    clave = '';
    // "Listo: agregué…" ya va en el título.
    const limpio = texto.replace(/^listo[:.,]?\s*/i, '').trim();
    mensaje = limpio ? limpio[0].toUpperCase() + limpio.slice(1) : 'Ya está en Nexo.';
    fase = 'listo';
    onvinculado?.(id);
  }

  function cerrar() {
    parar();
    oncerrar();
  }

  function confirmar() {
    if (fase === 'listo') cerrar();
    else if (fase === 'agotado' || fase === 'error') esperarBoton();
    else if (fase === 'clave') enviarClave();
  }

  // Al abrirse con algo nuevo: parte de cero y, si es con botón, empieza a esperar. Al cerrarse, para.
  let abiertoCon: string | null = null;
  $effect(() => {
    const h = hallazgo;
    untrack(() => {
      if (!h) {
        if (abiertoCon !== null) parar();
        abiertoCon = null;
        return;
      }
      if (abiertoCon === h.id) return;
      abiertoCon = h.id;
      actual = h;
      clave = '';
      error = null;
      mensaje = '';
      if (h.manejo.tipo === 'vincular' && h.manejo.forma === 'boton') esperarBoton();
      else {
        parar();
        fase = 'clave';
      }
    });
  });

  onDestroy(parar);

  const descripcion = $derived(
    fase === 'listo' ? mensaje : fase === 'agotado' ? 'No vi el botón presionado.' : fase === 'error' ? (error ?? '') : paso,
  );
  const textoConfirmar = $derived(
    {
      esperando: 'Esperando…',
      agotado: 'Reintentar',
      error: 'Reintentar',
      clave: 'Agregar',
      enviando: 'Agregando…',
      listo: 'Cerrar',
    }[fase],
  );
</script>

<Dialogo
  abierto={!!hallazgo}
  titulo={fase === 'listo' ? 'Listo' : `Vincular ${actual?.nombre ?? ''}`}
  {descripcion}
  icono={fase === 'listo' ? 'check' : 'conectar'}
  {textoConfirmar}
  ocupado={fase === 'esperando' || fase === 'enviando'}
  soloConfirmar={fase === 'listo'}
  onconfirmar={confirmar}
  oncancelar={cerrar}
>
  {#if fase === 'esperando'}
    <div class="espera" aria-busy="true">
      <div class="avance" aria-hidden="true"><span class:lleno></span></div>
      <p class="linea">
        <span role="status">Esperando el botón…</span>
        <span class="quedan" aria-hidden="true">Quedan {quedan}&nbsp;s</span>
      </p>
    </div>
  {:else if fase === 'agotado'}
    <p class="linea centro">Toca Reintentar y hazlo de nuevo.</p>
  {:else if fase === 'clave' || fase === 'enviando'}
    <div class="campos">
      <label class="campo">
        <span class="rotulo-campo">Contraseña</span>
        <input
          type="password"
          autocomplete="off"
          spellcheck="false"
          autocapitalize="off"
          bind:value={clave}
          bind:this={campo}
          readonly={fase === 'enviando'}
          aria-invalid={!!error}
          aria-describedby={error ? `${uid}-error` : undefined}
          data-autofocus
        />
      </label>
      {#if error}<p class="error" id="{uid}-error" role="alert">{error}</p>{/if}
      {#if fase === 'enviando'}<p class="linea" role="status">Probando la contraseña…</p>{/if}
    </div>
  {/if}
</Dialogo>

<style>
  .espera {
    display: grid;
    gap: 8px;
  }
  /* Se llena una sola vez en 40 s: es lo que Nexo espera el botón. */
  .avance {
    height: 4px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--primario) 18%, transparent);
    overflow: hidden;
  }
  .avance span {
    display: block;
    height: 100%;
    background: var(--primario);
    transform: scaleX(0.02);
    transform-origin: left center;
    transition: transform 40s linear;
  }
  .avance span.lleno {
    transform: scaleX(1);
  }
  @media (prefers-reduced-motion: reduce) {
    .avance span {
      transition: none;
      transform: scaleX(0.5);
    }
  }
  .linea {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 2px 10px;
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .linea.centro {
    justify-content: center;
    text-align: center;
  }
  .quedan {
    color: var(--texto-3);
    font-variant-numeric: tabular-nums;
  }

  .campos {
    display: grid;
    gap: 8px;
  }
  .campo {
    display: grid;
    gap: 6px;
  }
  .rotulo-campo {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--texto-2);
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
  .campo input[aria-invalid='true'] {
    border-color: var(--error);
  }
  .error {
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--error);
    overflow-wrap: break-word;
  }
</style>
