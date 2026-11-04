<!-- Panel de una pantalla: dibujo, nombre, brillo, resolución, frecuencia y "Hacer principal".
     El cambio de modo lo confirma quien la usa (Pantallas.svelte), que vuelve al anterior si no se confirma. -->
<script lang="ts">
  import { untrack } from 'svelte';
  import { ponerBrillo } from '$lib/api';
  import { esMac } from '$lib/api-sistema';
  import { enviadorDelUltimo } from '$lib/enviador';
  import type { Pantalla } from '$lib/tipos';
  import type { ModoPantalla } from '$lib/tipos-sistema';
  import { fotoDePantalla } from '$lib/catalogo';
  import Ilustracion from './Ilustracion.svelte';
  import Deslizador from './Deslizador.svelte';
  import Icono from './Icono.svelte';
  import MenuDesplegable from './MenuDesplegable.svelte';

  let {
    pantalla: p,
    onerror,
    principal = false,
    varias = false,
    modos = null,
    ocupada = false,
    onmodo,
    onprincipal,
  }: {
    pantalla: Pantalla;
    onerror: (texto: string) => void;
    /** Es la pantalla principal (se marca solo si hay más de una). */
    principal?: boolean;
    /** Hay más de una pantalla: se puede elegir la principal. */
    varias?: boolean;
    /** Modos de la pantalla; null mientras cargan o si no hay. */
    modos?: ModoPantalla[] | null;
    /** Hay un cambio en curso: no se acepta otro. */
    ocupada?: boolean;
    onmodo?: (modo: ModoPantalla, que: 'resolucion' | 'frecuencia') => void;
    onprincipal?: () => void;
  } = $props();

  // --- Brillo: valor propio mientras arrastras; si no, sigue al que llega del sistema (por ejemplo, con las teclas). ---
  let tocando = $state(false);
  let brillo = $state(untrack(() => p.brillo ?? 0));
  $effect(() => {
    const b = p.brillo;
    if (!tocando && b !== null) brillo = b;
  });

  // Siempre el último valor, sin fila: la pantalla sigue al dedo.
  const enviar = enviadorDelUltimo((v: number) => ponerBrillo(p.id, v), (e) => onerror(String(e)));
  function cambiarBrillo(valor: number) {
    brillo = valor;
    enviar(valor);
  }

  // --- Modos: agrupados por tamaño; cada tamaño con sus frecuencias. ---
  interface Tamano {
    clave: string;
    ancho: number;
    alto: number;
    retina: boolean;
    modos: ModoPantalla[];
  }
  const claveDe = (m: ModoPantalla) => `${m.ancho}x${m.alto}${m.retina ? 'r' : ''}`;
  const hzDe = (m: ModoPantalla) => Math.round(m.hz * 100);
  const numero = new Intl.NumberFormat('es', { maximumFractionDigits: 2 });

  const tamanos = $derived.by(() => {
    const mapa = new Map<string, Tamano>();
    for (const m of modos ?? []) {
      const clave = claveDe(m);
      let t = mapa.get(clave);
      if (!t) mapa.set(clave, (t = { clave, ancho: m.ancho, alto: m.alto, retina: m.retina, modos: [] }));
      t.modos.push(m);
    }
    return [...mapa.values()].sort((a, b) => b.ancho - a.ancho || b.alto - a.alto || Number(b.retina) - Number(a.retina));
  });
  const actual = $derived(modos?.find((m) => m.actual) ?? null);
  const tamanoActual = $derived(actual ? tamanos.find((t) => t.clave === claveDe(actual)) : undefined);

  // Una opción por frecuencia (la del modo actual si se repite), de mayor a menor. Sin las que el sistema no informa.
  const frecuencias = $derived.by(() => {
    const porHz = new Map<number, ModoPantalla>();
    for (const m of tamanoActual?.modos ?? []) {
      if (m.hz <= 0) continue;
      const ya = porHz.get(hzDe(m));
      if (!ya || m.actual) porHz.set(hzDe(m), m);
    }
    return [...porHz.values()].sort((a, b) => b.hz - a.hz);
  });

  const opcionesResolucion = $derived(
    tamanos.map((t) => ({ valor: t.clave, nombre: `${t.ancho} × ${t.alto}${t.retina ? ' (Retina)' : ''}` })),
  );
  const opcionesFrecuencia = $derived(frecuencias.map((m) => ({ valor: String(m.id), nombre: `${numero.format(m.hz)} Hz` })));
  const valorFrecuencia = $derived(
    actual ? String(frecuencias.find((m) => hzDe(m) === hzDe(actual))?.id ?? actual.id) : '',
  );

  function elegirResolucion(clave: string) {
    const t = tamanos.find((x) => x.clave === clave);
    if (!t || !onmodo || ocupada) return;
    // La misma frecuencia si ese tamaño la tiene; si no, la más alta.
    const igual = actual ? t.modos.find((m) => hzDe(m) === hzDe(actual)) : undefined;
    const modo = igual ?? [...t.modos].sort((a, b) => b.hz - a.hz)[0];
    if (modo) onmodo(modo, 'resolucion');
  }

  function elegirFrecuencia(valor: string) {
    const modo = modos?.find((m) => String(m.id) === valor);
    if (modo && onmodo && !ocupada) onmodo(modo, 'frecuencia');
  }
</script>

<article class="tarjeta vidrio" aria-label={p.nombre}>
  <div class="escenario">
    <div class="figura">
      {#if fotoDePantalla(p)}
        <img src={fotoDePantalla(p)} alt="" draggable="false" />
      {:else}
        <Ilustracion tipo={p.interna ? 'portatil' : 'monitor'} encendido={p.controlable} />
      {/if}
    </div>
  </div>

  <div class="identidad">
    <div class="nombre-linea">
      <h2 class="nombre">{p.nombre}</h2>
      {#if varias && principal}<span class="chip-principal">Principal</span>{/if}
    </div>
    <p class="tipo">{p.interna ? 'Pantalla integrada' : 'Monitor externo'}</p>
  </div>

  <div class="filas">
    {#if p.controlable && p.brillo !== null}
      <div class="fila">
        <span class="fila__rotulo">Brillo</span>
        <div class="brillo">
          <span class="sol"><Icono nombre="sol-bajo" tamano={18} /></span>
          <Deslizador
            valor={brillo}
            color="amarillo"
            etiqueta="Brillo de {p.nombre}"
            oncambio={cambiarBrillo}
            ontocando={(t) => (tocando = t)}
          />
          <span class="sol"><Icono nombre="sol" tamano={18} /></span>
          <span class="valor">{brillo} %</span>
        </div>
      </div>
    {:else if !p.interna}
      <div class="fila nota">
        <span class="nota__icono"><Icono nombre="info" tamano={17} /></span>
        <p>
          Este monitor no responde al brillo. Activa <b>DDC/CI</b> en el menú del monitor (con sus botones) y vuelve a
          abrir Nexo.
        </p>
      </div>
    {/if}

    {#if tamanos.length > 0}
      <div class="fila">
        <span class="fila__rotulo">Resolución</span>
        <div class="fila__menu">
          <MenuDesplegable
            opciones={opcionesResolucion}
            valor={tamanoActual?.clave ?? ''}
            etiqueta="Resolución de {p.nombre}"
            oncambio={elegirResolucion}
          />
        </div>
      </div>
    {/if}

    {#if frecuencias.length > 1}
      <div class="fila">
        <span class="fila__rotulo">Frecuencia</span>
        <div class="fila__menu">
          <MenuDesplegable
            opciones={opcionesFrecuencia}
            valor={valorFrecuencia}
            etiqueta="Frecuencia de {p.nombre}"
            oncambio={elegirFrecuencia}
          />
        </div>
      </div>
    {:else if frecuencias.length === 1}
      <div class="fila">
        <span class="fila__rotulo">Frecuencia</span>
        <span class="fila__valor">{opcionesFrecuencia[0].nombre}</span>
      </div>
    {/if}

    {#if varias && !principal && onprincipal}
      <div class="fila">
        <div class="textos">
          <span class="fila__rotulo">Pantalla principal</span>
          <span class="ayuda">
            {esMac ? 'La que tiene la barra de menús y el Dock.' : 'La que tiene la barra de tareas y el menú Inicio.'}
          </span>
        </div>
        <button class="boton-vidrio boton" type="button" disabled={ocupada} onclick={onprincipal}>Hacer principal</button>
      </div>
    {/if}
  </div>
</article>

<style>
  .tarjeta {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
    padding: 16px 20px 6px;
    border-radius: var(--radio-lg);
  }

  /* El aparato, protagonista, sobre una superficie elevada. */
  .escenario {
    display: grid;
    place-items: center;
    height: 150px;
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
  }
  .figura {
    width: 132px;
    height: 132px;
  }
  .figura img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .identidad {
    min-width: 0;
  }
  .nombre-linea {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 10px;
  }
  .nombre {
    margin: 0;
    min-width: 0;
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
    overflow-wrap: anywhere;
  }
  .chip-principal {
    display: inline-flex;
    align-items: center;
    height: 22px;
    padding: 0 9px;
    border-radius: var(--radio-pildora);
    font-size: 11.5px;
    font-weight: 600;
    color: var(--elegido-fg);
    background: var(--elegido-bg);
    box-shadow: inset 0 0 0 1px var(--elegido-borde);
  }
  .tipo {
    margin: 2px 0 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }

  .filas {
    display: flex;
    flex-direction: column;
  }
  .fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 56px;
    padding: 10px 0;
    border-top: 1px solid var(--borde);
  }
  .fila__rotulo {
    flex-shrink: 0;
    font-weight: 500;
  }
  .fila__menu {
    width: min(260px, 62%);
  }
  .fila__valor {
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .textos {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .ayuda {
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }

  .brillo {
    flex: 1;
    max-width: 420px;
    display: grid;
    grid-template-columns: 18px 1fr 18px 44px;
    align-items: center;
    gap: 10px;
  }
  .sol {
    display: grid;
    color: var(--amarillo);
  }
  .valor {
    text-align: right;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .nota {
    justify-content: flex-start;
    align-items: flex-start;
    gap: 10px;
  }
  .nota__icono {
    display: grid;
    margin-top: 1px;
    color: var(--texto-3);
  }
  .nota p {
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }
  .nota b {
    color: var(--texto);
    font-weight: 600;
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
  .boton:disabled {
    cursor: default;
    opacity: 0.6;
  }
</style>
