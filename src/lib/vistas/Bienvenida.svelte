<!--
  Bienvenida de la primera vez. Ocupa toda la ventana y reemplaza al panel hasta terminar.
  Pasos: Bienvenida, Tu nombre, Permisos y Todo listo. En Mac todos los permisos son obligatorios y "Continuar"
  no se activa hasta tenerlos todos. En Windows no hay permisos que pedir, así que ese paso se salta.
  Con `soloPermisos` muestra solo el paso de permisos, con el título "Falta un permiso": es lo que se ve cuando
  la bienvenida ya se hizo pero el sistema quitó un permiso. Nexo no sigue hasta que vuelva.
  Mientras se ven los permisos, se releen cada 1,5 s y al volver a la ventana, porque se activan en Ajustes.
  Superficies sólidas; la aurora son degradados quietos. Ningún texto se corta: todo baja de línea.
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { fade } from 'svelte/transition';
  import { MediaQuery } from 'svelte/reactivity';
  import { leerAjustes, usarTeclasBrillo } from '$lib/api';
  import {
    completarBienvenida,
    esMac,
    guardarNombre,
    leerPermisos,
    pedirPermiso,
    permisoDeSiONo,
    permisosFaltantes,
    permisosRequeridos,
    usarArranque,
  } from '$lib/api-sistema';
  import type { EstadoPermiso, Permisos, SeccionPrivacidad } from '$lib/tipos-sistema';
  import Icono from '$lib/componentes/Icono.svelte';
  import IconoBienvenida, { type NombreBienvenida } from '$lib/componentes/IconoBienvenida.svelte';
  import Interruptor from '$lib/componentes/Interruptor.svelte';
  import NexoBot from '$lib/componentes/NexoBot.svelte';
  import LogoNexo from '$lib/componentes/LogoNexo.svelte';

  let {
    nombreInicial = '',
    soloPermisos = false,
    permisosIniciales = null,
    onlisto,
  }: {
    /** Lo que trae el campo del nombre: el guardado o el de la cuenta del sistema. */
    nombreInicial?: string;
    /** Solo el paso de permisos ("Falta un permiso"): la bienvenida ya se hizo, pero falta uno. */
    soloPermisos?: boolean;
    /** Los permisos ya leídos, para no mostrar la lista vacía al entrar. */
    permisosIniciales?: Permisos | null;
    /** Terminó: se muestra el panel. Trae el nombre con que saludar. */
    onlisto: (nombre: string) => void;
  } = $props();

  type Paso = 'bienvenida' | 'nombre' | 'permisos' | 'listo';
  const pasos: Paso[] = untrack(() => soloPermisos)
    ? ['permisos']
    : permisosRequeridos.length
      ? ['bienvenida', 'nombre', 'permisos', 'listo']
      : ['bienvenida', 'nombre', 'listo'];
  let indice = $state(0);
  const paso = $derived(pasos[indice]);

  const sinMovimiento =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const dur = (ms: number) => (sinMovimiento ? 0 : ms);
  // Con la ventana baja, la mascota se achica para que cada paso quepa sin desplazar.
  const baja = new MediaQuery('max-height: 680px');

  let contenido = $state<HTMLElement | null>(null);

  // Al cambiar de paso, el foco va al título (el lector de pantalla lo anuncia) o, en Tu nombre, al campo.
  async function irA(i: number) {
    indice = i;
    await tick();
    if (pasos[i] === 'nombre') {
      const campo = contenido?.querySelector('input');
      campo?.focus();
      // El cursor al final del nombre que ya trae, listo para corregirlo o seguir con Enter.
      campo?.setSelectionRange(campo.value.length, campo.value.length);
    } else contenido?.querySelector<HTMLElement>('h1')?.focus();
  }
  const siguiente = () => irA(Math.min(indice + 1, pasos.length - 1));
  const atras = () => irA(Math.max(indice - 1, 0));

  // --- Tu nombre ---
  let nombre = $state(untrack(() => nombreInicial.trim().slice(0, 30)));
  let errorNombre = $state<string | null>(null);
  let guardandoNombre = $state(false);
  const hayNombre = $derived(nombre.trim().length > 0);

  async function enviarNombre(e: SubmitEvent) {
    e.preventDefault();
    if (!hayNombre || guardandoNombre) return;
    guardandoNombre = true;
    errorNombre = null;
    try {
      nombre = await guardarNombre(nombre.trim());
      siguiente();
    } catch (err) {
      errorNombre = String(err);
    }
    guardandoNombre = false;
  }

  // --- Permisos ---
  const DATOS: Record<SeccionPrivacidad, { titulo: string; para: string; icono: NombreBienvenida | null }> = {
    bluetooth: { titulo: 'Bluetooth', para: 'Para ver, conectar y desconectar tus aparatos.', icono: null },
    accesibilidad: {
      titulo: 'Accesibilidad',
      para: 'Para que las teclas de brillo cambien el monitor bajo el mouse.',
      icono: 'accesibilidad',
    },
    notificaciones: {
      titulo: 'Notificaciones',
      para: 'Para avisarte cuando una batería esté baja.',
      icono: 'notificaciones',
    },
    pantalla: {
      titulo: 'Grabación de pantalla',
      para: 'Para las capturas con Imprimir Pantalla desde tu PC.',
      icono: null,
    },
  };
  // Qué hacer en Ajustes del Sistema cuando el permiso se activa a mano.
  const EN_AJUSTES: Record<SeccionPrivacidad, string> = {
    bluetooth: 'En Ajustes, activa Nexo en la lista y vuelve aquí.',
    accesibilidad: 'En Ajustes, activa Nexo en la lista y vuelve aquí.',
    notificaciones: 'En Ajustes, elige Nexo, activa «Permitir notificaciones» y vuelve aquí.',
    pantalla: 'En Ajustes, activa Nexo en la lista. Si macOS pide salir y volver a abrir Nexo, acepta.',
  };

  let permisos = $state<Permisos | null>(untrack(() => permisosIniciales));
  // Los que ya se mandaron a Ajustes: muestran cómo activarlos. La Accesibilidad siempre se activa allá.
  let enAjustes = $state<SeccionPrivacidad[]>([]);
  let pidiendo = $state<SeccionPrivacidad | null>(null);
  let errorPermiso = $state<{ clave: SeccionPrivacidad; texto: string } | null>(null);

  const faltan = $derived(permisos ? permisosFaltantes(permisos).length : permisosRequeridos.length);

  interface Fila {
    clave: SeccionPrivacidad;
    titulo: string;
    para: string;
    icono: NombreBienvenida | null;
    concedido: boolean;
    chip: string;
    /** "Permitir" si aún no se preguntó, "Abrir Ajustes" si se negó; sin botón si ya está o lo bloquea un perfil. */
    accion: string | null;
    ayuda: string | null;
  }

  const filas = $derived.by((): Fila[] => {
    const p = permisos;
    if (!p) return [];
    return permisosRequeridos.map((clave) => {
      // La Accesibilidad y la Grabación de pantalla solo dicen sí o no: la primera vez se piden y, después, se
      // activan en Ajustes.
      const estado: EstadoPermiso = permisoDeSiONo(clave)
        ? p[clave]
          ? 'concedido'
          : enAjustes.includes(clave)
            ? 'denegado'
            : 'pendiente'
        : p[clave];
      const datos = DATOS[clave];
      const concedido = estado === 'concedido';
      const restringido = estado === 'restringido';
      return {
        clave,
        titulo: datos.titulo,
        icono: datos.icono,
        para: restringido
          ? 'Lo bloquea un perfil de administración de este equipo. Pide a quien administra tu Mac que lo permita.'
          : datos.para,
        concedido,
        chip: concedido ? 'Concedido' : restringido ? 'Restringido' : 'Falta',
        accion: concedido || restringido ? null : estado === 'pendiente' ? 'Permitir' : 'Abrir Ajustes',
        ayuda: !concedido && !restringido && enAjustes.includes(clave) ? EN_AJUSTES[clave] : null,
      };
    });
  });

  const tituloPermisos = $derived(
    !soloPermisos
      ? 'Permisos que Nexo necesita'
      : faltan === 0
        ? 'Permisos listos'
        : faltan === 1
          ? 'Falta un permiso'
          : 'Faltan permisos',
  );
  const mensajePermisos = $derived(
    !permisos
      ? 'Revisando permisos…'
      : faltan === 0
        ? 'Todo concedido. Ya puedes continuar.'
        : faltan === 1
          ? 'Falta 1 permiso para continuar.'
          : `Faltan ${faltan} permisos para continuar.`,
  );

  async function pedir(fila: Fila) {
    if (pidiendo) return;
    pidiendo = fila.clave;
    errorPermiso = null;
    // Con "Abrir Ajustes", y siempre con la Accesibilidad y la Grabación de pantalla, la persona tiene que activarlo allá.
    if ((fila.accion === 'Abrir Ajustes' || permisoDeSiONo(fila.clave)) && !enAjustes.includes(fila.clave))
      enAjustes = [...enAjustes, fila.clave];
    try {
      permisos = await pedirPermiso(fila.clave);
    } catch (e) {
      errorPermiso = { clave: fila.clave, texto: String(e) };
    }
    pidiendo = null;
  }

  const iguales = (a: Permisos | null, b: Permisos) =>
    !!a &&
    a.accesibilidad === b.accesibilidad &&
    a.bluetooth === b.bluetooth &&
    a.notificaciones === b.notificaciones &&
    a.pantalla === b.pantalla;

  let leyendo = false;
  async function leer() {
    if (leyendo) return;
    leyendo = true;
    try {
      const p = await leerPermisos();
      if (!iguales(permisos, p)) permisos = p;
    } catch {
      // Si falla una lectura, la siguiente lo corrige.
    }
    leyendo = false;
  }

  // Mientras se ve este paso: cada 1,5 s (si falta alguno) y al volver a la ventana desde Ajustes.
  $effect(() => {
    if (paso !== 'permisos') return;
    untrack(leer);
    const intervalo = setInterval(() => {
      if (document.visibilityState === 'visible' && faltan > 0) leer();
    }, 1500);
    window.addEventListener('focus', leer);
    return () => {
      clearInterval(intervalo);
      window.removeEventListener('focus', leer);
    };
  });

  // --- Todo listo ---
  let arranque = $state(true);
  let terminando = $state(false);
  let errorFinal = $state<string | null>(null);

  async function terminar() {
    if (terminando) return;
    terminando = true;
    errorFinal = null;
    try {
      // El arranque y las teclas de brillo no detienen la bienvenida: se cambian luego en Ajustes y Pantallas.
      await usarArranque(arranque).catch(() => {});
      if (esMac) {
        const p = await leerPermisos().catch(() => permisos);
        if (p?.accesibilidad) await usarTeclasBrillo(true).catch(() => {});
      }
      await completarBienvenida();
      onlisto(nombre.trim());
    } catch (e) {
      errorFinal = String(e);
      terminando = false;
    }
  }

  // Si faltaba la Accesibilidad al abrir, las teclas de brillo no pudieron arrancar: se encienden si estaban elegidas.
  const faltabaAccesibilidad = untrack(() => !permisosIniciales?.accesibilidad);
  async function seguir() {
    if (terminando || faltan > 0) return;
    terminando = true;
    if (esMac && faltabaAccesibilidad) {
      try {
        if ((await leerAjustes()).teclasBrillo) await usarTeclasBrillo(true);
      } catch {
        // Se puede encender después en Pantallas.
      }
    }
    onlisto(nombre.trim());
  }
</script>

<div class="bienvenida">
  <div class="arrastre" data-tauri-drag-region></div>

  <div class="desplazable">
    <main class="tarjeta" aria-labelledby="bienvenida-titulo">
      <header class="cabeza">
        {#if indice > 0}
          <button class="atras" type="button" onclick={atras}>
            <Icono nombre="atras" tamano={16} />
            Atrás
          </button>
        {:else}
          <span class="marca"><span class="logo" aria-hidden="true"><LogoNexo tamano={30} etiqueta="" /></span>Nexo</span>
        {/if}
        {#if pasos.length > 1}
          <div class="progreso">
            <span class="puntos" aria-hidden="true">
              {#each pasos as p, i (p)}
                <span class:hecho={i < indice} class:actual={i === indice}></span>
              {/each}
            </span>
            <span>Paso {indice + 1} de {pasos.length}</span>
          </div>
        {/if}
      </header>

      <div class="contenido" bind:this={contenido}>
        {#key paso}
          <section class="paso" in:fade={{ duration: dur(220) }}>
            {#if paso === 'bienvenida'}
              <div class="bot" style:--bot="{baja.current ? 128 : 170}px">
                <NexoBot expresion="saludo" tamano={baja.current ? 128 : 170} />
              </div>
              <h1 id="bienvenida-titulo" class="portada" tabindex="-1">Te damos la bienvenida a Nexo</h1>
              <p class="sub">Tus audífonos, pantallas y aparatos, en un solo lugar.</p>
              <div class="pie">
                <button class="boton-primario grande" type="button" onclick={siguiente}>Empezar</button>
              </div>
            {:else if paso === 'nombre'}
              <span class="insignia" aria-hidden="true"><IconoBienvenida nombre="persona" tamano={24} /></span>
              <h1 id="bienvenida-titulo" tabindex="-1">¿Cómo te llamas?</h1>
              <form class="formulario" onsubmit={enviarNombre} novalidate>
                <input
                  class="campo"
                  type="text"
                  bind:value={nombre}
                  maxlength="30"
                  placeholder="Tu nombre"
                  autocomplete="given-name"
                  spellcheck="false"
                  aria-labelledby="bienvenida-titulo"
                  aria-describedby="bienvenida-ayuda-nombre"
                  aria-invalid={!!errorNombre}
                  oninput={() => (errorNombre = null)}
                />
                <p class="ayuda" id="bienvenida-ayuda-nombre">Así te saludo en Inicio.</p>
                {#if errorNombre}
                  <p class="error" role="alert"><Icono nombre="alerta" tamano={15} />{errorNombre}</p>
                {/if}
                <div class="pie">
                  <button
                    class="boton-primario grande"
                    type="submit"
                    disabled={!hayNombre || guardandoNombre}
                    aria-busy={guardandoNombre}
                  >
                    Continuar
                  </button>
                </div>
              </form>
            {:else if paso === 'permisos'}
              <span class="insignia" class:aviso={soloPermisos && faltan > 0} aria-hidden="true">
                <IconoBienvenida nombre="escudo" tamano={24} />
              </span>
              <h1 id="bienvenida-titulo" tabindex="-1">{tituloPermisos}</h1>
              <p class="sub">Son obligatorios: sin ellos Nexo no puede funcionar.</p>

              {#if permisos}
                <ul class="lista">
                  {#each filas as f (f.clave)}
                    <li class="permiso">
                      <span class="icono-fila" aria-hidden="true">
                        {#if f.icono}<IconoBienvenida nombre={f.icono} tamano={21} />{:else}<Icono
                            nombre={f.clave === 'pantalla' ? 'captura' : 'bluetooth'}
                            tamano={21}
                          />{/if}
                      </span>
                      <div class="permiso__textos">
                        <div class="permiso__cabeza">
                          <span class="permiso__titulo">{f.titulo}</span>
                          <span class="chip" class:si={f.concedido}><span class="punto"></span>{f.chip}</span>
                        </div>
                        <p class="permiso__para">{f.para}</p>
                        {#if f.ayuda}
                          <p class="permiso__ayuda"><Icono nombre="info" tamano={15} />{f.ayuda}</p>
                        {/if}
                        {#if errorPermiso?.clave === f.clave}
                          <p class="error" role="alert"><Icono nombre="alerta" tamano={15} />{errorPermiso.texto}</p>
                        {/if}
                      </div>
                      {#if f.accion}
                        <button
                          class="permiso__accion"
                          type="button"
                          disabled={pidiendo !== null}
                          aria-busy={pidiendo === f.clave}
                          aria-label="{f.accion}: {f.titulo}"
                          onclick={() => pedir(f)}
                        >
                          {f.accion}
                        </button>
                      {/if}
                    </li>
                  {/each}
                </ul>
              {:else}
                <ul class="lista" aria-busy="true">
                  {#each permisosRequeridos as clave (clave)}
                    <li class="permiso cargando"><span class="icono-fila"></span><span class="lineas"><span></span><span></span></span></li>
                  {/each}
                </ul>
              {/if}

              <div class="pie">
                <button
                  class="boton-primario grande"
                  type="button"
                  disabled={!permisos || faltan > 0 || terminando}
                  aria-describedby="bienvenida-faltan"
                  onclick={soloPermisos ? seguir : siguiente}
                >
                  {soloPermisos ? 'Abrir Nexo' : 'Continuar'}
                </button>
                <p
                  class="estado-permisos"
                  class:listo={!!permisos && faltan === 0}
                  id="bienvenida-faltan"
                  role="status"
                  aria-live="polite"
                >
                  <Icono nombre={permisos && faltan === 0 ? 'check-circulo' : 'info'} tamano={15} />
                  {mensajePermisos}
                </p>
              </div>
            {:else if paso === 'listo'}
              <div class="bot" style:--bot="{baja.current ? 84 : 116}px">
                <NexoBot expresion="feliz" tamano={baja.current ? 84 : 116} />
              </div>
              <h1 id="bienvenida-titulo" tabindex="-1">{nombre.trim() ? `Listo, ${nombre.trim()}` : 'Todo listo'}</h1>

              <ul class="lista">
                <li class="consejo">
                  <span class="icono-fila" aria-hidden="true"><Icono nombre="widgets" tamano={21} /></span>
                  <span>
                    {esMac
                      ? 'Para poner los widgets: clic derecho en el escritorio › Editar widgets › Nexo.'
                      : 'El widget de Nexo ya está en tu escritorio.'}
                  </span>
                </li>
                <li class="consejo">
                  <span class="icono-fila" aria-hidden="true">
                    <IconoBienvenida nombre={esMac ? 'barra-menus' : 'bandeja'} tamano={21} />
                  </span>
                  <span>
                    {esMac
                      ? 'Nexo vive en la barra de menús. Ábrelo desde su ícono cuando quieras.'
                      : 'Nexo vive en la bandeja, junto al reloj. Ábrelo desde su ícono cuando quieras.'}
                  </span>
                </li>
                <li class="consejo">
                  <span class="icono-fila" aria-hidden="true"><Icono nombre="encendido" tamano={21} /></span>
                  <span class="consejo__textos">
                    <span class="consejo__titulo">Abrir Nexo al iniciar sesión</span>
                    <span class="consejo__ayuda">Así está listo cada vez que enciendes el equipo.</span>
                  </span>
                  <Interruptor
                    encendido={arranque}
                    etiqueta="Abrir Nexo al iniciar sesión"
                    oncambio={() => (arranque = !arranque)}
                  />
                </li>
              </ul>

              {#if errorFinal}
                <p class="error" role="alert"><Icono nombre="alerta" tamano={15} />{errorFinal}</p>
              {/if}
              <div class="pie">
                <button
                  class="boton-primario grande"
                  type="button"
                  disabled={terminando}
                  aria-busy={terminando}
                  onclick={terminar}
                >
                  Abrir Nexo
                </button>
              </div>
            {/if}
          </section>
        {/key}
      </div>
    </main>
  </div>
</div>

<style>
  /* Toda la ventana, con su propio contexto: la aurora (z-index -1) queda sobre el fondo y bajo la tarjeta. */
  .bienvenida {
    position: fixed;
    inset: 0;
    isolation: isolate;
    display: flex;
    flex-direction: column;
    background: var(--fondo);
  }
  /* Aurora del fondo: lomas de luz que suben del borde de abajo, como al pie de la barra lateral. Quieta. */
  .bienvenida::before {
    content: '';
    position: absolute;
    z-index: -1;
    left: 0;
    right: 0;
    bottom: 0;
    height: 60%;
    pointer-events: none;
    background:
      radial-gradient(ellipse 40% 70% at 18% 100%, var(--aurora-2), transparent),
      radial-gradient(ellipse 36% 62% at 82% 100%, var(--aurora-1), transparent),
      radial-gradient(
        ellipse 30% 90% at 50% 100%,
        color-mix(in srgb, var(--aurora-3) 70%, transparent),
        transparent
      );
  }

  /* Arriba: libre para arrastrar la ventana y para los botones de macOS. En Windows hay barra de título. */
  .arrastre {
    flex-shrink: 0;
    height: 52px;
  }
  :global([data-plataforma='windows']) .arrastre {
    height: 16px;
  }

  /* La tarjeta va al centro; si no cabe, se desplaza desde arriba sin cortarse. */
  .desplazable {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 0 24px 28px;
  }
  .tarjeta {
    position: relative;
    isolation: isolate;
    width: min(600px, 100%);
    margin: auto;
    padding: 22px 40px 32px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-xl);
    background: var(--superficie);
    box-shadow: var(--sombra-panel);
  }

  /* ---------- Cabeza: la marca o Atrás, y el progreso ---------- */
  .cabeza {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 36px;
    margin: 0 -10px 14px;
  }
  .marca {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px;
    font-size: 17px;
    line-height: 22px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .logo {
    display: flex;
    flex-shrink: 0;
  }
  .atras {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 36px;
    padding: 0 12px 0 8px;
    border: 0;
    border-radius: var(--radio-sm);
    background: transparent;
    color: var(--texto-2);
    font-size: 14px;
    font-weight: 600;
    transition:
      background-color var(--dur-color) ease,
      color var(--dur-color) ease;
  }
  .atras:hover {
    background: var(--superficie-hover);
    color: var(--texto);
  }
  .atras:active {
    transform: scale(0.98);
  }
  .progreso {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px;
    font-size: 12.5px;
    line-height: 18px;
    font-weight: 500;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .puntos {
    display: flex;
    gap: 5px;
  }
  .puntos span {
    width: 7px;
    height: 7px;
    border-radius: var(--radio-pildora);
    background: var(--borde-fuerte);
    transition: background-color var(--dur-color) ease;
  }
  .puntos .hecho {
    background: color-mix(in srgb, var(--primario) 50%, var(--superficie));
  }
  .puntos .actual {
    width: 18px;
    background: var(--primario);
  }

  /* ---------- Cada paso ---------- */
  .paso {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
  }
  h1 {
    margin: 0;
    font-size: 26px;
    line-height: 32px;
    font-weight: 700;
    letter-spacing: -0.015em;
    overflow-wrap: anywhere;
    outline: none;
  }
  .portada {
    font-size: 32px;
    line-height: 38px;
    letter-spacing: -0.025em;
  }
  .sub {
    max-width: 440px;
    margin: 8px 0 0;
    font-size: 15px;
    line-height: 22px;
    color: var(--texto-2);
  }

  /* Nexo Bot con la aurora detrás: un centro turquesa, un arco azul y un toque celeste. Solo degradados quietos. */
  .bot {
    position: relative;
    display: flex;
    margin: 2px 0 18px;
  }
  .bot::before {
    content: '';
    position: absolute;
    z-index: -1;
    left: 50%;
    top: 50%;
    width: calc(var(--bot) * 2.3);
    height: calc(var(--bot) * 1.8);
    translate: -50% -46%;
    pointer-events: none;
    background:
      radial-gradient(
        ellipse 30% 34% at 52% 42%,
        var(--aurora-1),
        color-mix(in srgb, var(--aurora-1) 45%, transparent) 55%,
        transparent
      ),
      radial-gradient(ellipse 42% 48% at 50% 54%, transparent 48%, var(--aurora-2) 72%, transparent),
      radial-gradient(ellipse 26% 24% at 30% 50%, var(--aurora-3), transparent);
  }

  .insignia {
    display: grid;
    place-items: center;
    width: 52px;
    height: 52px;
    margin: 2px 0 14px;
    border-radius: var(--radio-md);
    color: var(--primario-texto);
    background: color-mix(in srgb, var(--primario) 14%, var(--superficie));
  }
  .insignia.aviso {
    color: var(--aviso);
    background: color-mix(in srgb, var(--aviso) 14%, var(--superficie));
  }

  /* Abajo de cada paso: un botón grande a lo ancho y, en Permisos, el aviso de cuántos faltan. */
  .pie {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    width: 100%;
    margin-top: 28px;
  }
  .grande {
    width: 100%;
    min-height: 48px;
    padding: 0 20px;
    font-size: 15px;
  }
  .grande:active:not(:disabled) {
    transform: scale(0.98);
  }
  .grande:disabled {
    background: var(--superficie-hundida);
    color: var(--texto-2);
    box-shadow: inset 0 0 0 1px var(--borde);
    cursor: default;
  }

  /* ---------- Tu nombre ---------- */
  .formulario {
    display: flex;
    flex-direction: column;
    width: 100%;
    margin: 20px 0 0;
    text-align: left;
  }
  .campo {
    width: 100%;
    height: 50px;
    padding: 0 16px;
    border: 1px solid var(--borde-fuerte);
    border-radius: var(--radio-sm);
    outline: none;
    background: var(--superficie-elevada);
    color: var(--texto);
    font: inherit;
    font-size: 17px;
    font-weight: 500;
    user-select: text;
    -webkit-user-select: text;
    transition:
      border-color var(--dur-color) ease,
      box-shadow var(--dur-color) ease;
  }
  .campo::placeholder {
    color: var(--texto-3);
    font-weight: 400;
  }
  .campo:focus {
    border-color: var(--primario);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--primario) 24%, transparent);
  }
  .campo[aria-invalid='true'] {
    border-color: var(--error);
  }
  .ayuda {
    margin: 8px 2px 0;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto-2);
  }
  .error {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 8px 2px 0;
    font-size: 13px;
    line-height: 19px;
    color: var(--error);
    text-align: left;
  }
  .error :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
  }

  /* ---------- Listas: permisos y consejos (paneles dentro del panel) ---------- */
  .lista {
    display: grid;
    gap: 10px;
    width: 100%;
    margin: 22px 0 0;
    padding: 0;
    list-style: none;
    text-align: left;
  }
  .permiso,
  .consejo {
    display: flex;
    gap: 14px;
    padding: 14px 16px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-md);
    background: var(--superficie-elevada);
  }
  .permiso {
    align-items: flex-start;
  }
  .consejo {
    align-items: center;
    font-size: 14px;
    line-height: 20px;
  }
  .icono-fila {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 40px;
    height: 40px;
    border-radius: var(--radio-sm);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
    color: var(--celeste);
  }

  .permiso__textos {
    flex: 1;
    min-width: 0;
  }
  .permiso__cabeza {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
  }
  .permiso__titulo {
    font-size: 15px;
    line-height: 22px;
    font-weight: 650;
  }
  .permiso__para {
    margin: 2px 0 0;
    font-size: 13px;
    line-height: 19px;
    color: var(--texto-2);
  }
  .permiso__ayuda {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 8px 0 0;
    font-size: 13px;
    line-height: 19px;
    font-weight: 500;
    color: var(--texto);
  }
  .permiso__ayuda :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
    color: var(--info);
  }
  /* El botón de cada permiso: turquesa suave, para que se note sin competir con Continuar. */
  .permiso__accion {
    align-self: center;
    flex-shrink: 0;
    min-height: 36px;
    padding: 0 16px;
    border: 1px solid var(--elegido-borde);
    border-radius: var(--radio-sm);
    background: color-mix(in srgb, var(--primario) 12%, var(--superficie-elevada));
    color: var(--primario-texto);
    font-size: 13.5px;
    font-weight: 650;
    white-space: nowrap;
    transition: background-color var(--dur-color) ease;
  }
  .permiso__accion:hover:not(:disabled) {
    background: color-mix(in srgb, var(--primario) 22%, var(--superficie-elevada));
  }
  .permiso__accion:active:not(:disabled) {
    transform: scale(0.98);
  }
  .permiso__accion:disabled {
    cursor: default;
    opacity: 0.6;
  }

  /* Chip: "Concedido" en verde; "Falta" o "Restringido" en ámbar. */
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 22px;
    padding: 0 9px 0 8px;
    border-radius: var(--radio-pildora);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    color: var(--aviso);
    background: color-mix(in srgb, var(--aviso) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--aviso) 30%, transparent);
  }
  .chip.si {
    color: var(--exito);
    background: color-mix(in srgb, var(--exito) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--exito) 30%, transparent);
  }
  .punto {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }

  .estado-permisos {
    display: flex;
    align-items: flex-start;
    justify-content: center;
    gap: 6px;
    margin: 0;
    font-size: 13px;
    line-height: 19px;
    font-weight: 600;
    color: var(--aviso);
  }
  .estado-permisos :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
  }
  .estado-permisos.listo {
    color: var(--exito);
  }

  /* Mientras llegan los permisos: siluetas quietas de las filas. */
  .cargando .icono-fila {
    box-shadow: none;
  }
  .lineas {
    flex: 1;
    display: grid;
    gap: 8px;
    padding-top: 6px;
  }
  .lineas span {
    height: 11px;
    border-radius: 6px;
    background: var(--superficie-hundida);
  }
  .lineas span:first-child {
    width: 35%;
  }
  .lineas span:last-child {
    width: 75%;
  }

  .consejo__textos {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .consejo__titulo {
    font-weight: 600;
  }
  .consejo__ayuda {
    font-size: 12.5px;
    line-height: 18px;
    color: var(--texto-2);
  }

  /* Ventana baja (desde 540 px de alto): menos aire para que cada paso quepa entero. */
  @media (max-height: 680px) {
    .arrastre {
      height: 40px;
    }
    .tarjeta {
      padding: 14px 32px 22px;
    }
    .cabeza {
      margin-bottom: 6px;
    }
    .insignia {
      display: none;
    }
    .bot {
      margin-bottom: 12px;
    }
    .lista {
      gap: 8px;
      margin-top: 16px;
    }
    .permiso,
    .consejo {
      padding: 10px 14px;
    }
    .pie {
      margin-top: 20px;
    }
  }
</style>
