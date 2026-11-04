<!-- "Ajustes del monitor" de un monitor externo, dentro de su tarjeta (Pantallas). Se despliega con un botón, sin
     pestañas. Arriba, siempre, el brillo mínimo (el 0 % de Nexo, para que el monitor no parpadee) y cómo responde
     a las teclas de brillo (si responde y de cuánto es el paso). Debajo, una rejilla con lo que el monitor dice que
     tiene por DDC/CI, cada control con su valor actual.
     Los valores se leen al abrir la sección (nunca en un bucle) y cada control manda siempre el último valor.
     Cambiar la entrada y restaurar los valores de fábrica piden confirmación. -->
<script lang="ts">
  import {
    ajustesMonitor,
    ponerMinimoBrillo,
    ponerTeclasPantalla,
    ponerVcp,
    pruebaMonitorAbierto,
    teclasPantalla,
  } from '$lib/api-monitor';
  import { esMac } from '$lib/api-sistema';
  import { enviadorDelUltimo } from '$lib/enviador';
  import {
    CON_SONIDO,
    KELVIN,
    MODOS_USUARIO,
    SILENCIADO,
    VCP,
    masCalido,
    nombreColor,
    nombreEntrada,
    nombreModoImagen,
    textoContinuo,
  } from '$lib/monitor';
  import type { Pantalla } from '$lib/tipos';
  import type { AjustesMonitor, CapacidadMonitor, TeclasPantalla } from '$lib/tipos-sistema';
  import Deslizador from './Deslizador.svelte';
  import Dialogo from './Dialogo.svelte';
  import Icono from './Icono.svelte';
  import Interruptor from './Interruptor.svelte';
  import MenuDesplegable from './MenuDesplegable.svelte';

  let { pantalla: p, onerror }: { pantalla: Pantalla; onerror: (texto: string) => void } = $props();

  const idCuerpo = $props.id();
  let abierto = $state(pruebaMonitorAbierto);
  let cargando = $state(false);
  let error = $state<string | null>(null);
  let datos = $state<AjustesMonitor | null>(null);
  /** Valores actuales por código (lo que muestra cada control). */
  let actuales = $state<Record<number, number>>({});
  let minimo = $state(10);
  let restaurando = $state(false);
  let confirmar = $state<{ tipo: 'entrada'; valor: number } | { tipo: 'fabrica' } | null>(null);

  // Cada lectura lleva un número: si llega una vieja (se cerró y se volvió a abrir), no pisa a la nueva.
  let lectura = 0;
  async function leer() {
    const esta = ++lectura;
    cargando = true;
    error = null;
    try {
      const a = await ajustesMonitor(p.id);
      if (esta !== lectura) return;
      datos = a;
      minimo = a.minimo;
      actuales = Object.fromEntries(Object.entries(a.valores).map(([k, v]) => [Number(k), v.actual]));
    } catch (e) {
      if (esta === lectura) error = String(e);
    }
    if (esta === lectura) cargando = false;
  }

  // --- Teclas de brillo: si este monitor responde y de cuánto es el paso (guardado en Nexo, no en el monitor). ---
  let teclas = $state<TeclasPantalla>({ responde: true, paso: 6 });
  const pasos = [
    { valor: '3', nombre: 'Fino', detalle: '3 % por toque' },
    { valor: '6', nombre: 'Normal', detalle: '6 % por toque' },
    { valor: '10', nombre: 'Grande', detalle: '10 % por toque' },
  ];
  const otraPantalla = esMac ? 'la pantalla de la Mac' : 'la pantalla del portátil';
  function leerTeclas() {
    teclasPantalla(p.id)
      .then((t) => (teclas = t))
      .catch(() => {});
  }
  async function cambiarTeclas(cambio: Partial<TeclasPantalla>) {
    const antes = teclas;
    teclas = { ...teclas, ...cambio };
    try {
      teclas = await ponerTeclasPantalla(p.id, teclas);
    } catch (e) {
      teclas = antes;
      onerror(String(e));
    }
  }

  function alternar() {
    abierto = !abierto;
    if (abierto) {
      leer();
      leerTeclas();
    }
  }
  if (pruebaMonitorAbierto) {
    leer();
    leerTeclas();
  }

  // --- Escribir: siempre el último valor de cada código, sin fila de órdenes. ---
  const enviadores = new Map<number, (v: number) => void>();
  function enviar(codigo: number, valor: number) {
    let f = enviadores.get(codigo);
    if (!f) {
      f = enviadorDelUltimo(
        (v: number) => ponerVcp(p.id, codigo, v),
        (e) => onerror(String(e)),
      );
      enviadores.set(codigo, f);
    }
    actuales[codigo] = valor;
    f(valor);
  }
  const enviarMinimo = enviadorDelUltimo(
    (v: number) => ponerMinimoBrillo(p.id, v),
    (e) => onerror(String(e)),
  );
  function cambiarMinimo(v: number) {
    minimo = Math.max(0, Math.min(40, Math.round(v)));
    enviarMinimo(minimo);
  }

  // --- Lo que el monitor tiene ---
  const cap = (codigo: number): CapacidadMonitor | undefined => datos?.capacidades.find((c) => c.codigo === codigo);
  const maximo = (codigo: number) => datos?.valores[codigo]?.maximo ?? 100;

  const capColor = $derived(cap(VCP.color));
  const colorActual = $derived(actuales[VCP.color]);
  // Si el monitor dice que usa un modo que no está en su lista, igual se muestra como el elegido.
  const opcionesColor = $derived.by(() => {
    if (!capColor) return [];
    const lista = colorActual !== undefined && !capColor.valores.includes(colorActual) ? [colorActual, ...capColor.valores] : capColor.valores;
    return lista.map((v) => ({ valor: String(v), nombre: nombreColor(v) }));
  });

  // Luz cálida: el preset más cálido de su lista; al apagarla vuelve al que había antes.
  const calido = $derived(capColor ? masCalido(capColor.valores) : null);
  const luzCalida = $derived(calido !== null && colorActual === calido);
  const clavePrevio = $derived(`nexo-luz-calida-${p.id}`);
  function alternarLuzCalida() {
    if (!capColor || calido === null) return;
    if (!luzCalida) {
      try {
        if (colorActual !== undefined) localStorage.setItem(clavePrevio, String(colorActual));
      } catch {
        // Sin almacenamiento: al apagarla vuelve a Natural.
      }
      enviar(VCP.color, calido);
      return;
    }
    let previo: number | null = null;
    try {
      const t = localStorage.getItem(clavePrevio);
      previo = t === null ? null : Number(t);
    } catch {
      previo = null;
    }
    const lista = capColor.valores;
    const destino =
      previo !== null && previo !== calido && lista.includes(previo)
        ? previo
        : lista.includes(0x05)
          ? 0x05
          : lista.find((v) => v !== calido);
    if (destino !== undefined) enviar(VCP.color, destino);
  }

  // Rojo, verde y azul: con un modo "Usuario" elegido, o siempre si el monitor no tiene modos de color.
  const conRgb = $derived(!!(cap(VCP.rojo) || cap(VCP.verde) || cap(VCP.azul)));
  const modoUsuario = $derived(!capColor || (colorActual !== undefined && MODOS_USUARIO.includes(colorActual)));
  const usuarioDisponible = $derived(capColor?.valores.find((v) => MODOS_USUARIO.includes(v)));
  const canales = [
    { codigo: VCP.rojo, nombre: 'Rojo', tono: 'var(--error)' },
    { codigo: VCP.verde, nombre: 'Verde', tono: 'var(--exito)' },
    { codigo: VCP.azul, nombre: 'Azul', tono: 'var(--info)' },
  ];

  const capEntrada = $derived(cap(VCP.entrada));
  const opcionesEntrada = $derived.by(() => {
    if (!capEntrada) return [];
    const actual = actuales[VCP.entrada];
    const lista = actual !== undefined && !capEntrada.valores.includes(actual) ? [actual, ...capEntrada.valores] : capEntrada.valores;
    return lista.map((v) => ({ valor: String(v), nombre: nombreEntrada(v) }));
  });

  const capModo = $derived(cap(VCP.modoImagen));
  const opcionesModo = $derived.by(() => {
    if (!capModo) return [];
    const actual = actuales[VCP.modoImagen];
    const lista = actual !== undefined && !capModo.valores.includes(actual) ? [actual, ...capModo.valores] : capModo.valores;
    return lista.map((v) => ({ valor: String(v), nombre: nombreModoImagen(v) }));
  });

  const continuos = [
    { codigo: VCP.contraste, nombre: 'Contraste', color: 'amarillo' as const },
    { codigo: VCP.nitidez, nombre: 'Nitidez', color: 'amarillo' as const },
  ];

  async function aceptar() {
    const c = confirmar;
    confirmar = null;
    if (!c) return;
    if (c.tipo === 'entrada') {
      enviar(VCP.entrada, c.valor);
      return;
    }
    restaurando = true;
    try {
      await ponerVcp(p.id, VCP.fabrica, 1);
      await leer();
    } catch (e) {
      onerror(String(e));
    }
    restaurando = false;
  }

  const textoMinimo = $derived(minimo === 0 ? 'Sin mínimo' : `${minimo} %`);
</script>

<section class="ajustes">
  <button class="cabeza" type="button" aria-expanded={abierto} aria-controls={idCuerpo} onclick={alternar}>
    <span class="cabeza__icono" aria-hidden="true"><Icono nombre="ajustes" tamano={18} /></span>
    <span class="textos">
      <span class="titulo">Ajustes del monitor</span>
      <span class="ayuda">Brillo mínimo, teclas de brillo y lo que tu monitor deja cambiar por DDC/CI.</span>
    </span>
    <span class="cabeza__chevron" aria-hidden="true">
      <Icono nombre={abierto ? 'chevron-arriba' : 'chevron-abajo'} tamano={18} />
    </span>
  </button>

  {#if abierto}
    <div class="cuerpo" id={idCuerpo}>
      <div class="rejilla" aria-busy={cargando}>
        <div class="celda ancha">
          <div class="celda__cabeza">
            <span class="nombre">Brillo mínimo (evita el parpadeo)</span>
            <span class="valor">{textoMinimo}</span>
          </div>
          <Deslizador
            valor={minimo}
            maximo={40}
            color="amarillo"
            etiqueta="Brillo mínimo de {p.nombre}"
            oncambio={cambiarMinimo}
          />
          <p class="nota-celda">
            Tu 0 % de brillo usa este nivel del monitor. Si el monitor parpadea con poco brillo, súbelo hasta que deje
            de hacerlo.
          </p>
        </div>

        <div class="celda">
          <div class="celda__cabeza centrada">
            <span class="nombre">Responde a las teclas de brillo</span>
            <Interruptor
              encendido={teclas.responde}
              etiqueta="{p.nombre} responde a las teclas de brillo"
              oncambio={() => cambiarTeclas({ responde: !teclas.responde })}
            />
          </div>
          <span class="nota-celda">
            {teclas.responde
              ? 'Con el mouse aquí, las teclas de brillo cambian este monitor.'
              : `Las teclas cambian ${otraPantalla} aunque el mouse esté aquí.`}
          </span>
        </div>

        <div class="celda">
          <div class="celda__cabeza">
            <span class="nombre">Paso de las teclas</span>
          </div>
          <MenuDesplegable
            opciones={pasos}
            valor={String(teclas.paso)}
            etiqueta="Paso de las teclas de brillo en {p.nombre}"
            oncambio={(v) => cambiarTeclas({ paso: Number(v) })}
          />
        </div>

        {#if cargando && !datos}
          <span class="solo-lector">Leyendo el monitor…</span>
          {#each [0, 1, 2, 3] as i (i)}
            <div class="celda" aria-hidden="true">
              <span class="hueso"></span>
              <span class="hueso largo"></span>
            </div>
          {/each}
        {:else if error}
          <div class="celda ancha aviso" role="status">
            <span class="aviso__icono"><Icono nombre="info" tamano={17} /></span>
            <p>{error}</p>
            <button class="boton" type="button" onclick={leer}>Volver a leer</button>
          </div>
        {:else if datos}
          {#each continuos as c (c.codigo)}
            {#if cap(c.codigo) && actuales[c.codigo] !== undefined}
              <div class="celda">
                <div class="celda__cabeza">
                  <span class="nombre">{c.nombre}</span>
                  <span class="valor">{textoContinuo(actuales[c.codigo], maximo(c.codigo))}</span>
                </div>
                <Deslizador
                  valor={actuales[c.codigo]}
                  maximo={maximo(c.codigo)}
                  color={c.color}
                  etiqueta="{c.nombre} de {p.nombre}"
                  oncambio={(v) => enviar(c.codigo, Math.round(v))}
                />
              </div>
            {/if}
          {/each}

          {#if capColor && colorActual !== undefined}
            <div class="celda">
              <div class="celda__cabeza">
                <span class="nombre">Modo de color</span>
              </div>
              <MenuDesplegable
                opciones={opcionesColor}
                valor={String(colorActual)}
                etiqueta="Modo de color de {p.nombre}"
                oncambio={(v) => enviar(VCP.color, Number(v))}
              />
            </div>
          {/if}

          {#if calido !== null}
            <div class="celda">
              <div class="celda__cabeza centrada">
                <span class="nombre">Luz cálida</span>
                <Interruptor encendido={luzCalida} etiqueta="Luz cálida en {p.nombre}" oncambio={alternarLuzCalida} />
              </div>
              <span class="nota-celda">
                {luzCalida ? `Encendida: ${KELVIN[calido]} K.` : `Pasa a ${KELVIN[calido]} K y vuelve al modo de antes.`}
              </span>
            </div>
          {/if}

          {#if conRgb}
            <div class="ancha subtitulo">
              <span class="rotulo-seccion">Personalizado</span>
              {#if !modoUsuario && usuarioDisponible !== undefined}
                <span class="nota-celda">
                  Elige «{nombreColor(usuarioDisponible)}» en Modo de color para ajustar el rojo, el verde y el azul.
                </span>
              {/if}
            </div>
            {#if modoUsuario}
              {#each canales as c (c.codigo)}
                {#if cap(c.codigo) && actuales[c.codigo] !== undefined}
                  <div class="celda">
                    <div class="celda__cabeza">
                      <span class="nombre">{c.nombre}</span>
                      <span class="valor">{textoContinuo(actuales[c.codigo], maximo(c.codigo))}</span>
                    </div>
                    <Deslizador
                      valor={actuales[c.codigo]}
                      maximo={maximo(c.codigo)}
                      tono={c.tono}
                      etiqueta="{c.nombre} de {p.nombre}"
                      oncambio={(v) => enviar(c.codigo, Math.round(v))}
                    />
                  </div>
                {/if}
              {/each}
            {/if}
          {/if}

          {#if capModo && actuales[VCP.modoImagen] !== undefined}
            <div class="celda">
              <div class="celda__cabeza">
                <span class="nombre">Modo de imagen</span>
              </div>
              <MenuDesplegable
                opciones={opcionesModo}
                valor={String(actuales[VCP.modoImagen])}
                etiqueta="Modo de imagen de {p.nombre}"
                oncambio={(v) => enviar(VCP.modoImagen, Number(v))}
              />
            </div>
          {/if}

          {#if capEntrada && actuales[VCP.entrada] !== undefined}
            <div class="celda">
              <div class="celda__cabeza">
                <span class="nombre">Entrada</span>
              </div>
              <MenuDesplegable
                opciones={opcionesEntrada}
                valor={String(actuales[VCP.entrada])}
                etiqueta="Entrada de {p.nombre}"
                oncambio={(v) => (confirmar = { tipo: 'entrada', valor: Number(v) })}
              />
            </div>
          {/if}

          {#if cap(VCP.volumen) && actuales[VCP.volumen] !== undefined}
            <div class="celda">
              <div class="celda__cabeza">
                <span class="nombre">Volumen del monitor</span>
                <span class="valor">{textoContinuo(actuales[VCP.volumen], maximo(VCP.volumen))}</span>
              </div>
              <Deslizador
                valor={actuales[VCP.volumen]}
                maximo={maximo(VCP.volumen)}
                color="celeste"
                apagado={actuales[VCP.silencio] === SILENCIADO}
                etiqueta="Volumen de {p.nombre}"
                oncambio={(v) => enviar(VCP.volumen, Math.round(v))}
              />
            </div>
          {/if}

          {#if cap(VCP.silencio) && actuales[VCP.silencio] !== undefined}
            <div class="celda">
              <div class="celda__cabeza centrada">
                <span class="nombre">Silencio</span>
                <Interruptor
                  encendido={actuales[VCP.silencio] === SILENCIADO}
                  etiqueta="Silencio en {p.nombre}"
                  oncambio={() => enviar(VCP.silencio, actuales[VCP.silencio] === SILENCIADO ? CON_SONIDO : SILENCIADO)}
                />
              </div>
              <span class="nota-celda">
                {actuales[VCP.silencio] === SILENCIADO ? 'Las bocinas del monitor están en silencio.' : 'Las bocinas del monitor suenan.'}
              </span>
            </div>
          {/if}

          {#if !datos.capacidades.some((c) => c.codigo !== VCP.fabrica)}
            <p class="ancha nota-celda">Este monitor solo deja cambiar el brillo por DDC/CI.</p>
          {:else if !datos.conLista}
            <p class="ancha nota-celda">El monitor no dio su lista de ajustes: aparecen solo los que respondieron.</p>
          {/if}

          {#if cap(VCP.fabrica)}
            <div class="ancha pie">
              <button
                class="boton peligro"
                type="button"
                disabled={restaurando}
                onclick={() => (confirmar = { tipo: 'fabrica' })}
              >
                {restaurando ? 'Restaurando…' : 'Restaurar valores de fábrica'}
              </button>
            </div>
          {/if}
        {/if}
      </div>
    </div>
  {/if}
</section>

<Dialogo
  abierto={confirmar !== null}
  titulo={confirmar?.tipo === 'entrada' ? `¿Cambiar a ${nombreEntrada(confirmar.valor)}?` : '¿Restaurar los valores de fábrica?'}
  descripcion={confirmar?.tipo === 'entrada'
    ? 'Si no hay nada conectado en esa entrada, el monitor se queda sin imagen. Para volver, usa los botones del monitor.'
    : 'El monitor vuelve a su brillo, contraste y color de fábrica. Tu brillo mínimo en Nexo no cambia.'}
  icono="pantallas"
  tono={confirmar?.tipo === 'fabrica' ? 'peligro' : 'normal'}
  textoConfirmar={confirmar?.tipo === 'entrada' ? 'Cambiar entrada' : 'Restaurar'}
  onconfirmar={aceptar}
  oncancelar={() => (confirmar = null)}
/>

<style>
  .ajustes {
    border-top: 1px solid var(--borde);
  }

  .cabeza {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-height: 60px;
    padding: 10px 6px 10px 0;
    border: 0;
    border-radius: var(--radio-sm);
    background: transparent;
    color: var(--texto);
    text-align: left;
    transition: background-color var(--dur-color) ease;
  }
  .cabeza:hover {
    background: var(--superficie-hover);
  }
  .cabeza:focus-visible {
    outline: none;
    box-shadow: var(--foco);
  }
  .cabeza__icono {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 32px;
    height: 32px;
    margin-left: 4px;
    border-radius: 10px;
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--amarillo);
  }
  .cabeza__chevron {
    display: grid;
    flex-shrink: 0;
    color: var(--texto-3);
  }
  .textos {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .titulo {
    font-weight: 600;
  }
  .ayuda {
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }

  .cuerpo {
    padding: 4px 0 16px;
  }

  /* Rejilla compacta: una, dos o tres columnas según el ancho de la tarjeta. */
  .rejilla {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 160px), 1fr));
    gap: 8px;
  }
  .ancha {
    grid-column: 1 / -1;
  }

  .celda {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 8px;
    min-width: 0;
    padding: 12px 14px;
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  .celda__cabeza {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 2px 10px;
  }
  .nombre {
    font-size: 13px;
    line-height: 18px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .valor {
    font-size: 13px;
    line-height: 18px;
    font-weight: 650;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .nota-celda {
    margin: 0;
    font-size: 12px;
    line-height: 17px;
    color: var(--texto-2);
  }
  .celda__cabeza.centrada {
    align-items: center;
    flex-wrap: nowrap;
  }

  .subtitulo {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 2px 0;
  }
  .rotulo-seccion {
    font-size: 11px;
    line-height: 16px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--texto-3);
  }

  .aviso {
    flex-direction: row;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
  }
  .aviso__icono {
    display: grid;
    color: var(--texto-3);
  }
  .aviso p {
    flex: 1;
    min-width: 160px;
    margin: 0;
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }

  .pie {
    display: flex;
    justify-content: flex-end;
    padding-top: 4px;
  }
  .boton {
    flex-shrink: 0;
    min-height: 32px;
    padding: 0 14px;
    border: 0;
    border-radius: var(--radio-sm);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--texto);
    font-size: 13px;
    font-weight: 600;
    transition: background-color var(--dur-color) ease;
  }
  .boton:hover:not(:disabled) {
    background: var(--superficie-hover);
  }
  .boton:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .boton.peligro {
    color: var(--error);
  }

  /* Mientras se lee el monitor: siluetas quietas. */
  .hueso {
    width: 40%;
    height: 12px;
    border-radius: 6px;
    background: var(--superficie-hundida);
  }
  .hueso.largo {
    width: 100%;
    height: 8px;
  }
</style>
