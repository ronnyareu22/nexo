<!-- Panel de una pantalla: a la izquierda la foto sobre un halo que brilla según el nivel; a la derecha el brillo
     en grande con su barra (el tono va de ámbar tenue a amarillo intenso), − y + y accesos rápidos.
     Debajo: resolución, frecuencia y "Hacer principal".
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

  // --- Brillo: valor propio mientras lo cambias; si no, sigue al que llega del sistema (por ejemplo, con las teclas). ---
  // Después de soltar se espera un momento antes de volver a seguir al sistema: así la barra no rebota con una
  // lectura vieja. Las diferencias de 1 % (redondeo del sistema) tampoco mueven la barra.
  let tocando = $state(false);
  let ultimoToque = 0;
  let brillo = $state(untrack(() => p.brillo ?? 0));
  $effect(() => {
    const b = p.brillo;
    if (b === null || tocando || performance.now() - ultimoToque < 900) return;
    if (Math.abs(b - untrack(() => brillo)) > 1) brillo = b;
  });

  // Siempre el último valor, sin fila: la pantalla sigue al dedo.
  const enviar = enviadorDelUltimo((v: number) => ponerBrillo(p.id, v), (e) => onerror(String(e)));
  function cambiarBrillo(valor: number) {
    ultimoToque = performance.now();
    brillo = Math.max(0, Math.min(100, Math.round(valor)));
    enviar(brillo);
  }
  function alTocar(t: boolean) {
    tocando = t;
    ultimoToque = performance.now();
  }
  // − y +: al múltiplo de 10 anterior o siguiente (31 → 30 → 20…; 31 → 40).
  const paso = (subir: boolean) =>
    cambiarBrillo(subir ? (Math.floor(brillo / 10) + 1) * 10 : Math.floor((brillo - 1) / 10) * 10);
  const rapidos = [
    { valor: 5, nombre: 'Mínimo' },
    { valor: 25, nombre: '25 %' },
    { valor: 50, nombre: '50 %' },
    { valor: 75, nombre: '75 %' },
    { valor: 100, nombre: 'Máximo' },
  ];

  // El tono del brillo: ámbar tenue abajo, amarillo intenso arriba.
  const paradas: [number, [number, number, number]][] = [
    [0, [124, 74, 18]],
    [40, [217, 119, 6]],
    [75, [245, 158, 11]],
    [100, [250, 204, 21]],
  ];
  const tono = $derived.by(() => {
    const b = Math.max(0, Math.min(100, brillo));
    for (let i = 1; i < paradas.length; i++) {
      const [b0, c0] = paradas[i - 1];
      const [b1, c1] = paradas[i];
      if (b <= b1) {
        const t = (b - b0) / (b1 - b0);
        const c = c0.map((v, k) => Math.round(v + (c1[k] - v) * t));
        return `rgb(${c[0]} ${c[1]} ${c[2]})`;
      }
    }
    return 'rgb(250 204 21)';
  });
  const nivel = $derived(
    brillo <= 15 ? 'Muy tenue' : brillo <= 40 ? 'Tenue' : brillo <= 70 ? 'Medio' : brillo <= 90 ? 'Alto' : 'Al máximo',
  );
  const conBrillo = $derived(p.controlable && p.brillo !== null);

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

  const resumen = $derived(
    [
      p.interna ? 'Pantalla integrada' : 'Monitor externo',
      actual ? `${actual.ancho} × ${actual.alto}` : '',
      actual && actual.hz > 0 ? `${numero.format(actual.hz)} Hz` : '',
    ]
      .filter(Boolean)
      .join(' · '),
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

<article class="tarjeta vidrio" aria-label={p.nombre} style:--tono={tono} style:--luz={conBrillo ? brillo / 100 : 0.6}>
  <div class="cabecera">
    <div class="escenario" aria-hidden="true">
      <div class="halo"></div>
      <div class="figura">
        {#if fotoDePantalla(p)}
          <img src={fotoDePantalla(p)} alt="" draggable="false" />
        {:else}
          <Ilustracion tipo={p.interna ? 'portatil' : 'monitor'} encendido={p.controlable} />
        {/if}
      </div>
    </div>

    <div class="lado">
      <div class="identidad">
        <div class="nombre-linea">
          <h2 class="nombre">{p.nombre}</h2>
          {#if varias && principal}<span class="chip-principal">Principal</span>{/if}
        </div>
        <p class="tipo">{resumen}</p>
      </div>

      {#if conBrillo}
        <div class="brillo-panel">
          <div class="cifra-linea">
            <span class="cifra">{brillo}<small>%</small></span>
            <span class="nivel">{nivel}</span>
          </div>
          <div class="control">
            <button class="redondo" type="button" onclick={() => paso(false)} disabled={brillo <= 0} aria-label="Bajar brillo">
              <Icono nombre="sol-bajo" tamano={18} />
            </button>
            <Deslizador
              valor={brillo}
              color="amarillo"
              {tono}
              etiqueta="Brillo de {p.nombre}"
              oncambio={cambiarBrillo}
              ontocando={alTocar}
            />
            <button class="redondo" type="button" onclick={() => paso(true)} disabled={brillo >= 100} aria-label="Subir brillo">
              <Icono nombre="sol" tamano={18} />
            </button>
          </div>
          <div class="rapidos" role="group" aria-label="Brillo rápido">
            {#each rapidos as r (r.valor)}
              <button
                type="button"
                class="rapido"
                class:activo={Math.abs(brillo - r.valor) <= 1}
                onclick={() => cambiarBrillo(r.valor)}>{r.nombre}</button
              >
            {/each}
          </div>
        </div>
      {:else if !p.interna}
        <div class="nota">
          <span class="nota__icono"><Icono nombre="info" tamano={17} /></span>
          <p>
            Este monitor no responde al brillo. Activa <b>DDC/CI</b> en el menú del monitor (con sus botones) y vuelve a
            abrir Nexo.
          </p>
        </div>
      {/if}
    </div>
  </div>

  <div class="filas">
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
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    padding: 20px 22px 6px;
    border-radius: var(--radio-lg);
  }

  /* Arriba: la foto a la izquierda y el brillo a la derecha; en tarjetas angostas, uno debajo del otro. */
  .cabecera {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 18px;
    padding-bottom: 14px;
  }
  @container (min-width: 620px) {
    .cabecera {
      grid-template-columns: minmax(200px, 300px) minmax(0, 1fr);
      gap: 28px;
      align-items: center;
    }
  }

  /* El aparato sobre una superficie hundida, con un halo cálido que crece con el brillo. */
  .escenario {
    position: relative;
    isolation: isolate;
    display: grid;
    place-items: center;
    min-height: 190px;
    border-radius: var(--radio-md);
    background: var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
    overflow: hidden;
  }
  .halo {
    position: absolute;
    inset: -20%;
    z-index: -1;
    background: radial-gradient(closest-side, var(--tono), transparent 72%);
    opacity: calc(0.08 + 0.42 * var(--luz));
    transition: opacity 160ms ease;
  }
  .figura {
    width: min(200px, 80%);
    aspect-ratio: 1;
  }
  /* La foto se apaga o se enciende con el brillo, como la pantalla de verdad. */
  .figura img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    filter: brightness(calc(0.45 + 0.6 * var(--luz))) drop-shadow(0 10px 18px rgb(0 0 0 / 0.28));
    transition: filter 160ms ease;
  }

  .lado {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
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
    font-size: 19px;
    line-height: 26px;
    font-weight: 700;
    letter-spacing: -0.015em;
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

  /* Brillo: la cifra grande con su nivel, la barra con − y +, y los accesos rápidos. */
  .brillo-panel {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px 18px;
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .cifra-linea {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }
  .cifra {
    font-size: 40px;
    line-height: 44px;
    font-weight: 700;
    letter-spacing: -0.03em;
    font-variant-numeric: tabular-nums;
    color: var(--texto);
  }
  .cifra small {
    margin-left: 2px;
    font-size: 18px;
    font-weight: 600;
    color: var(--texto-2);
  }
  .nivel {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    font-weight: 600;
    color: var(--texto-2);
  }
  .nivel::before {
    content: '';
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--tono);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--tono) 22%, transparent);
  }
  .control {
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr) 36px;
    align-items: center;
    gap: 12px;
  }
  .redondo {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--tono);
    transition: background-color var(--dur-color) ease;
  }
  .redondo:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .redondo:active:not(:disabled) {
    transform: scale(0.96);
  }
  .redondo:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .rapidos {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .rapido {
    min-height: 30px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--radio-pildora);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto-2);
    font-size: 12.5px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .rapido:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .rapido.activo {
    background: color-mix(in srgb, var(--tono) 18%, var(--superficie));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--tono) 45%, transparent);
    color: var(--texto);
  }

  .nota {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 14px 16px;
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
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
