<!--
  Tarjeta de un aparato de la casa: arriba su dibujo (las luces brillan con su color), su nombre, qué es y cómo está,
  y el interruptor grande; debajo, solo lo que ese aparato deja hacer:
  - Luz: brillo; de cálido a frío si lo tiene; y, si es de color, muestras de un toque y el tono.
  - Tele: volumen, silencio y entrada. Cortina: abrir, cerrar y cuánto. Cerradura: cerrar, o abrir con confirmación.
  - Sensor de puerta: abierta o cerrada y su batería. Lo detectado sin control: "pronto podrás manejarlo".
  `compacta` (Mi setup): dibujo chico al lado del nombre, interruptor y brillo.
  Los deslizadores se ven al instante y mandan el último valor cuando se dejan quietos (≈0,3 s): nunca una fila de
  órdenes. Sin animaciones; el escenario solo cambia de color.
-->
<script lang="ts">
  import type { Accion, Aparato } from '$lib/tipos-hogar';
  import { MUESTRAS, PISTA_BLANCO, PISTA_TONO, colorDeLuz, cssDeHsv, estadoEnPalabras, tipoDe, viaDe } from '$lib/hogar';
  import { enviadorDelUltimo } from '$lib/enviador';
  import { tooltip } from '$lib/tooltip';
  import Icono from './Icono.svelte';
  import Interruptor from './Interruptor.svelte';
  import Deslizador from './Deslizador.svelte';
  import MenuDesplegable from './MenuDesplegable.svelte';
  import Dialogo from './Dialogo.svelte';
  import IlustracionAparato from './IlustracionAparato.svelte';

  let {
    aparato: a,
    compacta = false,
    onorden,
    oneditar,
    onerror,
  }: {
    aparato: Aparato;
    compacta?: boolean;
    /** Manda una acción a este aparato (`confirmado` solo para abrir una cerradura). */
    onorden: (accion: Accion, confirmado?: boolean) => Promise<void>;
    /** Nombre, habitación y forma. */
    oneditar?: () => void;
    onerror?: (texto: string) => void;
  } = $props();

  const c = $derived(a.capacidades);
  const e = $derived(a.estado);
  const sinConexion = $derived(!a.enLinea);
  const deshabilitado = $derived(sinConexion || !a.controlable);

  // --- Lo que se ve mientras se manda (y un rato después, hasta que llega lo de Rust) ---
  type Clave = 'encendido' | 'brillo' | 'temperatura' | 'tono' | 'volumen' | 'posicion';
  let local = $state<Partial<Record<Clave, number | boolean>>>({});

  const encendido = $derived((local.encendido as boolean | undefined) ?? e.encendido === true);
  const brillo = $derived((local.brillo as number | undefined) ?? e.brillo ?? 100);
  const temperatura = $derived((local.temperatura as number | undefined) ?? e.temperatura ?? 50);
  const tono = $derived((local.tono as number | undefined) ?? e.color?.h ?? 0);
  const volumen = $derived((local.volumen as number | undefined) ?? e.volumen ?? 0);
  const posicion = $derived((local.posicion as number | undefined) ?? e.posicion ?? (e.abierto ? 100 : 0));
  const enColor = $derived(e.modo === 'color' && !!e.color);
  const luz = $derived(
    a.clase === 'luz'
      ? enColor
        ? cssDeHsv({ h: tono, s: e.color?.s ?? 100, v: 100 })
        : local.temperatura !== undefined
          ? colorDeLuz({ ...a, estado: { ...e, modo: 'blanco', temperatura } })
          : colorDeLuz(a)
      : null,
  );

  function soltar(clave: Clave, valor: number | boolean) {
    setTimeout(() => {
      if (local[clave] === valor) delete local[clave];
    }, 1200);
  }

  async function mandar(accion: Accion, confirmado = false) {
    try {
      await onorden(accion, confirmado);
    } catch (err) {
      onerror?.(String(err));
    }
  }

  /** Un deslizador: se ve enseguida y manda el último valor al dejarlo quieto. */
  function deslizador(clave: Clave, accion: (v: number) => Accion) {
    let t: ReturnType<typeof setTimeout> | undefined;
    const enviar = enviadorDelUltimo((v: number) => mandar(accion(v)));
    return (v: number) => {
      local[clave] = v;
      clearTimeout(t);
      t = setTimeout(() => {
        enviar(v).then(() => soltar(clave, v));
      }, 280);
    };
  }

  const cambiarBrillo = deslizador('brillo', (v) => ({ tipo: 'brillo', valor: Math.max(1, v) }));
  const cambiarTemperatura = deslizador('temperatura', (v) => ({ tipo: 'temperatura', valor: v }));
  const cambiarTono = deslizador('tono', (v) => ({ tipo: 'color', h: v, s: e.color?.s ?? 100, v: Math.max(1, brillo) }));
  const cambiarVolumen = deslizador('volumen', (v) => ({ tipo: 'volumen', valor: v }));
  const cambiarPosicion = deslizador('posicion', (v) => ({ tipo: 'posicion', valor: v }));

  function alternar() {
    const nuevo = !encendido;
    local.encendido = nuevo;
    mandar({ tipo: nuevo ? 'encender' : 'apagar' }).then(() => soltar('encendido', nuevo));
  }

  function muestra(h: number, s: number) {
    local.tono = h;
    mandar({ tipo: 'color', h, s, v: Math.max(1, brillo) }).then(() => soltar('tono', h));
  }

  let confirmando = $state(false);
  let ocupado = $state(false);
  async function puerta(abrir: boolean, confirmado = false) {
    confirmando = false;
    ocupado = true;
    await mandar({ tipo: abrir ? 'abrir' : 'cerrar' }, confirmado);
    ocupado = false;
  }

  /** Prendido para el dibujo y el texto (una puerta abierta también se marca). */
  const estaActivo = $derived(
    a.enLinea && (a.clase === 'cerradura' || a.clase === 'sensorPuerta' ? e.abierto === true : encendido),
  );
  const etiquetaDe = $derived(`${a.nombre}: ${estadoEnPalabras(a) || 'detectado'}`);
  const tieneControles = $derived(
    a.controlable && (c.brillo || c.temperatura || c.color || c.volumen || c.abrir || c.posicion || c.entradas),
  );
  const muestraElegida = (h: number) => enColor && Math.abs(tono - h) < 8;
</script>

{#snippet icono(nombre: 'sol' | 'volumen' | 'pantallas')}
  <span class="fila__icono" aria-hidden="true"><Icono {nombre} tamano={16} /></span>
{/snippet}

<article
  class="tarjeta lamina"
  class:compacta
  class:apagado={!estaActivo}
  class:sin-conexion={sinConexion}
  class:solo-ver={!a.controlable}
  style:--luz={luz}
  aria-label={etiquetaDe}
>
  <div class="escenario" class:brilla={a.clase === 'luz' && encendido && !sinConexion}>
    <span class="dibujo">
      <IlustracionAparato
        clase={a.clase}
        forma={a.forma}
        encendido={encendido && !sinConexion}
        color={luz}
        abierto={e.abierto}
        posicion={a.clase === 'cortina' ? posicion : null}
      />
    </span>
    {#if oneditar && !compacta}
      <button type="button" class="editar" aria-label="Cambiar nombre o habitación de {a.nombre}" use:tooltip={'Nombre y habitación'} onclick={oneditar}>
        <Icono nombre="editar" tamano={15} />
      </button>
    {/if}
  </div>

  <div class="cabeza">
    <div class="textos">
      <h3 class="nombre">{a.nombre}</h3>
      <p class="detalle">
        {tipoDe(a)}{#if estadoEnPalabras(a)}<span class="punto-sep" aria-hidden="true">{' · '}</span><span class="estado" class:ok={estaActivo}>{estadoEnPalabras(a)}</span>{/if}
      </p>
    </div>
    {#if a.controlable && c.encender}
      <Interruptor encendido={encendido} etiqueta={encendido ? `Apagar ${a.nombre}` : `Encender ${a.nombre}`} grande={!compacta} cargando={false} oncambio={alternar} />
    {/if}
  </div>

  {#if !a.controlable && a.nota}
    <p class="nota"><Icono nombre="info" tamano={14} /><span>{a.nota}</span></p>
  {:else if a.clase === 'sensorPuerta'}
    <p class="nota">
      <span class="chip-puerta" class:abierto={e.abierto === true}>{e.abierto ? 'Abierta' : e.abierto === false ? 'Cerrada' : 'Sin datos'}</span>
      {#if e.bateria !== null}<span>Batería {e.bateria} %</span>{/if}
    </p>
  {/if}

  {#if tieneControles && !deshabilitado}
    <div class="controles">
      {#if c.brillo}
        <div class="fila">
          {@render icono('sol')}
          <Deslizador valor={brillo} minimo={1} etiqueta="Brillo de {a.nombre}" tono={luz ?? undefined} oncambio={cambiarBrillo} />
          <span class="fila__numero" aria-hidden="true">{brillo}&nbsp;%</span>
        </div>
      {/if}
      {#if c.temperatura && !compacta}
        <div class="fila">
          <span class="fila__icono texto" aria-hidden="true">Cálida</span>
          <Deslizador valor={temperatura} etiqueta="Blanco de {a.nombre}, de cálido a frío" pista={PISTA_BLANCO} oncambio={cambiarTemperatura} />
          <span class="fila__numero texto" aria-hidden="true">Fría</span>
        </div>
      {/if}
      {#if c.color && !compacta}
        <div class="muestras" role="group" aria-label="Color de {a.nombre}">
          {#each MUESTRAS as m (m.nombre)}
            <button
              type="button"
              class="muestra"
              class:elegida={muestraElegida(m.h)}
              style:--m={cssDeHsv({ h: m.h, s: m.s, v: 100 })}
              aria-label={m.nombre}
              aria-pressed={muestraElegida(m.h)}
              use:tooltip={m.nombre}
              onclick={() => muestra(m.h, m.s)}
            ></button>
          {/each}
        </div>
        <div class="fila">
          <span class="fila__icono texto" aria-hidden="true">Tono</span>
          <Deslizador valor={tono} maximo={359} etiqueta="Tono de {a.nombre}" pista={PISTA_TONO} oncambio={cambiarTono} />
          <span class="fila__numero" aria-hidden="true"></span>
        </div>
      {/if}
      {#if c.volumen}
        <div class="fila">
          {@render icono('volumen')}
          <Deslizador valor={volumen} etiqueta="Volumen de {a.nombre}" color="celeste" apagado={!!e.silenciado} oncambio={cambiarVolumen} />
          <span class="fila__numero" aria-hidden="true">{e.silenciado ? 'Mudo' : volumen}</span>
        </div>
      {/if}
      {#if c.silencio || c.entradas}
        <div class="acciones">
          {#if c.silencio}
            <button type="button" class="boton-suave" aria-pressed={!!e.silenciado} onclick={() => mandar({ tipo: 'silenciar', activo: !e.silenciado })}>
              <Icono nombre={e.silenciado ? 'volumen' : 'silencio'} tamano={16} />
              {e.silenciado ? 'Con sonido' : 'Silenciar'}
            </button>
          {/if}
          {#if c.entradas && a.entradas.length}
            <span class="entrada">
              <MenuDesplegable
                opciones={a.entradas.map((x) => ({ valor: x, nombre: x }))}
                valor={a.entrada ?? ''}
                etiqueta="Entrada de {a.nombre}"
                oncambio={(v) => mandar({ tipo: 'entrada', nombre: v })}
              />
            </span>
          {/if}
        </div>
      {/if}
      {#if c.posicion}
        <div class="fila">
          {@render icono('pantallas')}
          <Deslizador valor={posicion} etiqueta="Cuánto se abre {a.nombre}" color="primario" oncambio={cambiarPosicion} />
          <span class="fila__numero" aria-hidden="true">{posicion}&nbsp;%</span>
        </div>
      {/if}
      {#if c.abrir}
        <div class="acciones">
          <button type="button" class="boton-suave" disabled={ocupado || e.abierto === false} onclick={() => puerta(false)}>
            {a.clase === 'cerradura' ? 'Cerrar con llave' : 'Cerrar'}
          </button>
          <button
            type="button"
            class="boton-suave"
            disabled={ocupado || (e.abierto === true && a.clase !== 'cortina')}
            onclick={() => (c.requiereConfirmacion ? (confirmando = true) : puerta(true))}
          >
            Abrir
          </button>
        </div>
      {/if}
    </div>
  {/if}

  {#if !compacta}
    <p class="pie">
      {#if sinConexion}
        <span class="aviso"><Icono nombre="alerta" tamano={13} />Sin conexión</span>
      {/if}
      <span>{viaDe(a)}{a.direccion && a.via === 'wifi' ? ` · ${a.direccion}` : ''}</span>
      {#if a.controlable && !c.informaEstado}<span>· Recuerda lo último que le mandaste</span>{/if}
    </p>
  {/if}
</article>

<Dialogo
  abierto={confirmando}
  titulo="¿Abrir {a.nombre}?"
  descripcion="La puerta queda sin llave. Hazlo solo si estás seguro de quién va a entrar."
  icono="alerta"
  tono="peligro"
  textoConfirmar="Abrir"
  onconfirmar={() => puerta(true, true)}
  oncancelar={() => (confirmando = false)}
/>

<style>
  .tarjeta {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
    padding: 8px 8px 12px;
    border-radius: 20px;
  }
  /* El escenario crece si la fila es más alta: así una tarjeta con menos controles no deja un hueco abajo. */
  .escenario {
    position: relative;
    display: grid;
    place-items: center;
    flex: 1 0 112px;
    min-height: 112px;
    border-radius: 14px;
    background: color-mix(in srgb, var(--primario) 4%, var(--superficie-inserto));
    overflow: hidden;
    transition: background-color var(--dur-color) ease;
  }
  .escenario.brilla {
    background:
      radial-gradient(65% 75% at 50% 55%, color-mix(in srgb, var(--luz) 30%, transparent), transparent),
      var(--superficie-inserto);
  }
  .dibujo {
    display: block;
    width: 112px;
    height: 112px;
  }
  .apagado .dibujo {
    opacity: 0.75;
  }
  .sin-conexion .dibujo {
    opacity: 0.45;
    filter: grayscale(0.6);
  }
  .editar {
    position: absolute;
    top: 8px;
    right: 8px;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--superficie);
    box-shadow: 0 0 0 1px var(--borde);
    color: var(--texto-2);
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .editar:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }

  .cabeza {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 6px;
  }
  .textos {
    flex: 1;
    min-width: 0;
  }
  .nombre {
    margin: 0;
    font-size: 15px;
    line-height: 20px;
    font-weight: 650;
    letter-spacing: -0.01em;
    overflow-wrap: break-word;
  }
  .detalle {
    margin: 2px 0 0;
    font: var(--letra-pequena);
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .punto-sep {
    color: var(--texto-3);
  }
  .estado.ok {
    color: var(--texto);
    font-weight: 550;
  }
  .sin-conexion .estado {
    color: var(--aviso);
  }

  .nota {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
    margin: 0;
    padding: 0 6px;
    font: var(--letra-pequena);
    color: var(--texto-2);
  }
  .nota :global(svg) {
    flex-shrink: 0;
    color: var(--info);
  }
  .nota span {
    flex: 1 1 160px;
    min-width: 0;
  }
  .chip-puerta {
    flex: 0 0 auto !important;
    padding: 2px 10px;
    border-radius: var(--radio-pildora);
    background: color-mix(in srgb, var(--exito) 14%, transparent);
    color: var(--exito);
    font-weight: 600;
  }
  .chip-puerta.abierto {
    background: color-mix(in srgb, var(--aviso) 14%, transparent);
    color: var(--aviso);
  }

  .controles {
    display: grid;
    gap: 8px;
    padding: 0 6px;
  }
  .fila {
    display: grid;
    grid-template-columns: 38px minmax(0, 1fr) 40px;
    align-items: center;
    gap: 8px;
  }
  .fila__icono {
    display: grid;
    place-items: center start;
    color: var(--texto-3);
  }
  .fila__icono.texto,
  .fila__numero.texto {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--texto-3);
  }
  .fila__numero {
    text-align: right;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .muestras {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding-left: 46px;
  }
  .muestra {
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--m);
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.15);
    transition: box-shadow var(--dur-color) ease;
  }
  .muestra:hover {
    box-shadow:
      inset 0 0 0 1px rgb(0 0 0 / 0.15),
      0 0 0 2px var(--superficie),
      0 0 0 3.5px var(--borde-fuerte);
  }
  .muestra.elegida {
    box-shadow:
      inset 0 0 0 1px rgb(0 0 0 / 0.15),
      0 0 0 2px var(--superficie),
      0 0 0 3.5px var(--primario);
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .boton-suave {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    flex: 1 1 auto;
    min-height: 32px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--radio-sm);
    background: var(--superficie-inserto);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font-size: 12.5px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .boton-suave:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .boton-suave:disabled {
    color: var(--texto-3);
    cursor: default;
  }
  .entrada {
    flex: 1 1 140px;
    min-width: 120px;
  }

  .pie {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 6px;
    margin: 0;
    padding: 0 6px;
    font-size: 11.5px;
    line-height: 16px;
    color: var(--texto-3);
  }
  .pie .aviso {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--aviso);
    font-weight: 600;
  }

  /* Compacta (Mi setup): el dibujo chico a la izquierda, sin escenario alto. */
  .compacta {
    display: grid;
    grid-template-columns: 56px minmax(0, 1fr);
    grid-template-areas: 'dibujo cabeza' 'controles controles';
    gap: 8px 10px;
    padding: 10px 12px 12px 10px;
    border-radius: 16px;
  }
  .compacta .escenario {
    grid-area: dibujo;
    flex: none;
    width: 56px;
    height: 56px;
    min-height: 0;
    border-radius: 12px;
  }
  .compacta .dibujo {
    width: 50px;
    height: 50px;
  }
  .compacta .cabeza {
    grid-area: cabeza;
    padding: 0;
  }
  .compacta .controles {
    grid-area: controles;
    padding: 0 2px;
  }
  .compacta .nota {
    grid-column: 1 / -1;
  }
</style>
