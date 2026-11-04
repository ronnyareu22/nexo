# Nexo: brief de diseño y producto

> Para quien diseña y programa Nexo (personas o agentes de IA). Léelo completo antes de tocar la interfaz.
> Define la identidad, las pantallas, lo que el sistema permite de verdad y el orden de trabajo.
> Versión 1 · 3 de noviembre de 2026 · Dueño: Ronny Arellano

---

## 0. Cómo usar este documento

- **Primero el estado real (sección 2).** No borres lo que ya funciona y no muestres controles que el sistema no permite (sección 8).
- **Una fase a la vez (sección 15).** Al cerrar cada fase, revísala con la "Definición de terminado" (sección 16).
- **Las imágenes de referencia marcan una dirección artística, no son pantallas para copiar.** Toma su lenguaje: un objeto protagonista, poco texto, paneles calmos y una navegación lateral compacta.

## 1. Qué es Nexo

Un centro de control nativo para **macOS y Windows** con el que se administran audífonos, monitores, teclados, ratones y luces desde un solo panel. Corre en segundo plano con un ícono en la barra de menús (Mac) o en la bandeja (Windows), y el panel se abre con un clic.

**Principios, en orden de prioridad:**
1. **Honesto.** Solo se muestra lo que funciona en ese sistema y con ese aparato. Si falta un permiso, se dice cuál y dónde se activa.
2. **Ligero.** Pesa menos de 5 MB, usa unos 30 MB en segundo plano y menos de 100 MB con el panel abierto. No hay animaciones en bucle.
3. **Calmo.** Poca cosa en pantalla: el aparato es el protagonista y los controles van agrupados.
4. **Nativo.** Respeta las convenciones de cada sistema: atajos, permisos, notificaciones, barra de menús o bandeja.

## 2. Estado actual del proyecto

**Stack:** Tauri 2, con la interfaz en Svelte 5 y TypeScript y la parte nativa en Rust. Carpeta: `~/Desktop/nexo`.

```
src/                      Interfaz (no llama APIs del sistema; solo `lib/api.ts`)
  routes/+page.svelte     Pantalla principal · routes/aviso/ aviso de brillo · routes/brillo/ widget de brillo
  lib/api.ts              Puente a Rust (y datos de ejemplo fuera de la app)
  lib/tipos.ts            Tipos compartidos con Rust
  lib/estilos/global.css  Tokens y temas
  lib/componentes/        Tarjetas, paneles, interruptor, deslizador, anillo…
src-tauri/src/
  lib.rs                  Arranque, ventana, vigía
  dispositivos/           Adaptadores: bluetooth, audio, pantallas, historial
  sistema/                Teclas de brillo, aviso en pantalla, ajustes, menú del ícono (bandeja.rs)
```

**Qué hay hoy** (✅ funciona y está probado · ⚠️ está hecho pero sin probar con el aparato real · ⏳ pendiente):

| Función | Mac | Windows |
|---|---|---|
| Lista Bluetooth, conectar y desconectar, batería | ✅ IOBluetooth + system_profiler | ⏳ WinRT Bluetooth |
| Sonido: volumen, silencio, salida, micrófono | ✅ CoreAudio | ⏳ Core Audio (IAudioEndpointVolume) |
| Brillo de la pantalla integrada | ✅ DisplayServices (0 ms) | ⏳ WMI / DDC |
| Brillo de monitores externos (DDC/CI) | ⚠️ ddc-macos | ⏳ ddc-winapi |
| Teclas de brillo en el monitor bajo el mouse, con aviso propio | ⚠️ CGEventTap + Accesibilidad | ⚠️ Atajos a elección (de fábrica Ctrl+Alt+F1 / F2) en el gancho WH_KEYBOARD_LL del remoto; las teclas del portátil, por WmiMonitorBrightnessEvent (el portátil vuelve a su nivel; el aviso de Windows sale igual). Por monitor: si responde y su paso |
| Historial (última vez usado, uso, batería) y automático | ✅ vigía cada 20 s | ⏳ |
| Notificación de batería baja | ✅ tauri-plugin-notification | ⏳ |
| Tema claro, oscuro o del sistema (persistente) | ✅ | ✅ (misma interfaz) |
| Fotos reales de los aparatos + herramienta para prepararlas | ✅ `diseno/herramientas` | ✅ |
| Salida y entrada de sonido del sistema (todas las del equipo) | ✅ CoreAudio, lectura comprobada | ⏳ |
| Alias ("Renombrar") y "Olvidar" | ⚠️ alias guardado por Nexo; olvidar con la API privada `remove`, sin probar | ⏳ |
| Resolución, frecuencia y pantalla principal | ⚠️ lectura comprobada (132 modos); cambiar sin probar, con vuelta atrás a los 15 s | ⏳ |
| Permisos (Accesibilidad, Bluetooth) y arranque al iniciar sesión | ✅ lectura · ⚠️ activar el arranque sin probar · las notificaciones no informan su estado real | ⏳ |
| Interfaz nueva: barra lateral, Inicio, Dispositivos, Audio, Pantallas, Automatizaciones, Ajustes | ✅ revisada en claro y oscuro con datos reales | ✅ (misma interfaz) |
| XDrop (antes Nexo Drop): archivos, carpetas, textos, enlaces y capturas entre tus equipos por Tailscale (puerto 47821) | ⚠️ escrito, sin probar entre los dos equipos | ⚠️ escrito y compilado con cargo-xwin, sin probar (sin "Enviar captura") |
| Audio como mezclador: tiras verticales, silencio del micrófono, "Probar sonido" y volumen de avisos | ⚠️ escrito; el silencio del micrófono depende del aparato · avisos con `osascript` | ⚠️ escrito y compilado con cargo-xwin (sin volumen de avisos) |
| Menú del ícono con accesos rápidos: captura de zona (F13), enviar por XDrop, escenas, salida y micrófono, silencio, brillo, conectar audífonos, remoto | ⚠️ escrito; el armado del menú tiene pruebas (`cargo test --lib bandeja`), sin probar en la app | ⚠️ escrito y compilado con cargo-xwin, sin probar (sin captura de zona) |
| Mi setup: escenas de un toque (también en el menú del ícono) y el mapa editable de la casa y la oficina (`GET /nexo/aparatos` por XDrop) | ⚠️ escrito; lógica pura con pruebas (`cargo test --lib setup escenas`), sin probar con los aparatos ni entre los dos equipos | ⚠️ escrito y compilado con cargo-xwin, sin probar ("Modo remoto" abre RustDesk) |
| Brillo mínimo por monitor externo (10 % de fábrica, 0–40 %): el 0 % de Nexo nunca baja de ahí, así no parpadea | ⚠️ ddc-macos, sin probar con el aparato | ⚠️ dxva2, compilado con cargo-xwin, sin probar con el Steren |
| Ajustes del monitor por DDC/CI (contraste, modo de color, luz cálida, rojo/verde/azul, nitidez, modo de imagen, entrada, volumen, silencio, valores de fábrica), solo lo que el monitor dice que tiene | ⚠️ capacidades con el crate `ddc`, sin probar | ⚠️ `CapabilitiesRequestAndCapabilitiesReply`, sin probar |
| Widget de brillo de una sola pantalla (`brillo-<id>`), desde la tarjeta de la pantalla o el submenú Brillo del ícono | ⚠️ escrito, sin probar | ⚠️ escrito y compilado con cargo-xwin, sin probar |

**Medidas actuales:** la app pesa 4,5 MB y el proceso principal usa unos 27 MB. Con el panel abierto se suman unos 70 MB del motor web, que se liberan al cerrar el panel.

## 3. Identidad visual

**Inspiración:** macOS Tahoe, Apple Home, la hoja de ajustes de los AirPods y apps profesionales minimalistas. **Superficies sólidas**: sin vidrio, sin transparencias y sin desenfoque (el dueño rechazó el liquid glass el 3 nov).

**Marca (guía del dueño, 3 nov 2026):** `~/Downloads/Guía de Marca Nexo con Robot Mascota.png`.
- **Logo:** la "N" de cinta, con degradado turquesa #3EF3D3 → cian #22C6F0 → azul #2F6CF4.
  - Se genera en vector con `diseno/marca/generar-logo.mjs`, a partir de la silueta medida de la guía, simétrico y con puntas finas.
  - Salidas: `logo-n.svg`, `logo-n-mono.svg` (barra de menús), `icono-app.svg` / `icono-app-1024.png` (de ahí salen los íconos con `pnpm tauri icon`) y el componente `src/lib/componentes/LogoNexo.svelte`.
  - Al lado de la N, la palabra "Nexo" en Inter 600–700.
- **Lema:** "Conecta. Controla. Trabaja mejor."
- **Mascota:** "Tu asistente inteligente para conectar y controlar tus dispositivos."
- **Colores de la guía:** Primary #00E6B8, Primary Hover #00C9A0, Accent Blue #3B82F6, Success #22C55E, Dark BG #0B0F14, Surface #141A26, Border #1B2431, Text #E5EAF0.
- **Qué evitar:** deformar, rotar o cambiar los colores del logo, y ponerlo sobre fondos de bajo contraste.

**Debe transmitir:** elegancia, tecnología, simplicidad, precisión y personalización.

**Prohibido:**
- Que parezca un panel administrativo genérico, o lleno de tarjetas iguales.
- Degradados decorativos sin función, bordes luminosos, neón y sombras pesadas. **Excepción pedida por el dueño (3 nov): la aurora**, un resplandor suave turquesa y azul detrás de la mascota en Inicio, al pie de la barra lateral y detrás de la foto del aparato. Va solo con `radial-gradient` fijos y los tokens `--aurora-1/2/3`: sin desenfoque, sin animación y sin paneles transparentes.
- **Textos cortados con "…"** (sin `text-overflow: ellipsis` ni `line-clamp`). El dueño lo pidió: "no cortes legibilidad". Si un texto no cabe, pasa a otra línea, y el diseño se arma para que quepa.
- **Animaciones de hover con movimiento** (levantar, agrandar, reflejos que cruzan, objetos flotando). El dueño las rechazó. El hover es solo un cambio de color de fondo (`--superficie-hover`).
- **Liquid glass** en cualquier forma: lente que sigue al mouse, desenfoques, transparencias, cantos irisados o brillos en los bordes. El dueño lo rechazó.
- Emojis en lugar de íconos, íconos mezclados de varios estilos y textos en inglés.
- Cualquier cosa que parezca hecha por IA: rellenos sin función, "próximamente", datos inventados.

## 4. Tokens de diseño

Todo color, radio, sombra y duración sale de `lib/estilos/global.css`. Los componentes no usan colores sueltos.

### 4.1 Colores

| Token | Oscuro | Claro | Uso |
|---|---|---|---|
| `--primario` | `#00E6B8` | `#00B894` | Identidad de Nexo: acción principal, estado activo, foco. **No en todo.** |
| `--primario-hover` | `#00C9A0` | `#009E80` | Hover de la acción principal |
| `--exito` | `#22C55E` | `#16A34A` | Conectado, batería alta |
| `--info` | `#3B82F6` | `#2563EB` | Información neutra |
| `--aviso` | `#F59E0B` | `#B45309` | Batería media, permiso faltante |
| `--error` | `#EF4444` | `#DC2626` | Error, batería muy baja, "Olvidar" |
| `--fondo` | `#0B0F14` | `#F5F7FB` | Fondo de la ventana |
| `--superficie` | `#141A26` | `#FFFFFF` | Paneles |
| `--superficie-elevada` | `#1B2431` | `#F0F3F8` | Paneles dentro de paneles, menús |
| `--superficie-hover` | `#222C3D` | `#E9EEF5` | Fila o botón al pasar el mouse (solo color) |
| `--borde` | `rgba(255,255,255,.08)` | `#E2E8F0` | Bordes |
| `--texto` | `#E5EAF0` | `#172033` | Texto principal |
| `--texto-2` | `#9AA6B5` | `#64748B` | Texto secundario |
| `--texto-3` | `#64748B` | `#94A3B8` | Rótulos, ayudas |

**Acentos por familia** (solo en íconos, rellenos de deslizador y pequeños indicadores; nunca en fondos grandes). Mantienen lo que el dueño pidió (verde, celeste y amarillo):
- Conexión y batería: `--exito`
- Sonido: celeste `#38BDF8` (claro `#0284C7`)
- Brillo y pantallas: amarillo `#FBBF24` (claro `#D97706`)

**Lo elegido** (pestaña, opción o vista seleccionada) va en un fondo turquesa suave con el texto en turquesa y un borde fino del mismo tono. En la barra lateral lleva además una rayita turquesa a la izquierda. Sin cantos irisados ni brillos.

### 4.2 Letra
- **Inter** variable, servida desde la app (`static/fuentes`, latín y latín extendido, licencia OFL). Respaldo: `-apple-system`, `Segoe UI Variable`.
- Escala (tamaño/interlineado, peso): portada 32/38 700 · título 26/32 700 · sección 20/26 700 · panel 15/22 650 · cuerpo 14/21 400 · pequeño 12.5/18 · rótulo 11/16 600 en versalitas con +0.06em.
- Números con `tabular-nums` (porcentajes, horas).

### 4.3 Forma, espacio y profundidad
- **Radios:** 26 ventana interna · 22 panel · 16 tarjeta pequeña · 12 botón · 99 píldora.
- **Espacios:** 4 · 8 · 12 · 16 · 20 · 24 · 32 · 40.
- **Sombras:** una sola, muy suave, para lo que flota (menús, aviso). Los paneles se separan por superficie y borde, no por sombra.
- **Sin vidrio.** Lo que flota (menús, tooltips, aviso de brillo, panel de la barra de menús) es una superficie elevada sólida (`.flotante`) con borde fino y una sombra suave. Los paneles de contenido son `--superficie` con borde.

### 4.4 Movimiento
- **Duraciones:** 120 ms (color), 220 ms (cambiar de vista), 420 ms con resorte (perilla del interruptor), 900 ms (llenado del anillo de batería al aparecer).
- **Solo se mueven `transform` y `opacity`.** Con `prefers-reduced-motion`, nada se desliza.
- **No hay animaciones en bucle,** salvo los indicadores de carga mientras algo carga.

## 5. Componentes base (reutilizables)

| Componente | Reglas |
|---|---|
| **Barra lateral** | Compacta: 220 px, o 68 px plegada (ventana de menos de 900 px). Ícono y texto. La sección elegida va con el estilo de "lo elegido". Abajo, una tarjeta con el estado general ("Bluetooth activo · Todo en perfecto estado", o el problema) y la aurora detrás. |
| **Fila de dispositivo** | Foto o ilustración, nombre, estado y batería en una línea, y un chevron. La elegida, con "lo elegido". |
| **Panel** | Superficie con título en versalitas y una sola idea por panel. |
| **Interruptor** | Píldora con perilla blanca de resorte. Encendido en `--primario`. Gira mientras conecta. |
| **Deslizador** | Pista hundida, relleno con el color de su familia y perilla blanca. Manda siempre **el último valor** (`lib/enviador.ts`), nunca una fila de órdenes. |
| **Anillo de batería** | Verde, ámbar o rojo según el nivel, con el % al centro y el estado en palabras al lado. Si hay datos, la estimación ("Le quedan unas 5 h") y la gráfica de las últimas horas. |
| **Chip de estado** | Conectado (verde), Conectando… (primario, parpadea), Sin conectar (gris), Falta permiso (ámbar). |
| **Selector segmentado** | Para pestañas cortas (macOS · Windows) y el tema (sistema · claro · oscuro). |
| **Menú desplegable propio** | Lista en superficie elevada con borde fino. Nunca el `<select>` nativo. |
| **Tooltip propio** | En superficie elevada, nunca el `title` gris del navegador. |
| **Diálogo** | Mínimo: ícono, título, qué pasa y dos botones a lo ancho. Rojo si borra ("Olvidar"). |
| **Skeleton** | Siluetas de las filas o paneles que van a llegar. |
| **Íconos** | Un solo juego de línea, 24×24 con trazo 1.8, al estilo de SF Symbols. Las SF Symbols reales no se pueden usar dentro de la vista web ni en Windows, así que se dibujan propios, coherentes con ellas. |

## 6. Mascota: Nexo Bot

**Referencia oficial:** `diseno/mascota/referencia-saludo.png` (1254×1254, fondo transparente). Saluda con la mano derecha y hace "ok" con la izquierda.

**Fiel a la referencia, sin cambiarla por otro personaje:**
- Cuerpo blanco y futurista.
- Cabeza redondeada con la cara como una pantalla negra.
- Ojos y sonrisa **celestes y luminosos** (`#39D5FF`, como en la referencia), que se ven sonrientes cuando está feliz.
- Audífonos grandes.
- Un círculo en el pecho con la **"N" de Nexo** en luz celeste. Ese símbolo puede pasar a ser el ícono de la app.
- Articulaciones gris oscuro y líneas de luz celeste en los audífonos y los pies.
- Extremidades pequeñas y aire amable y premium.

**Formato:** mientras no esté el SVG, se usa la referencia optimizada en WebP de 512 px, que pesa menos de 120 KB. La meta es un SVG por capas (cuerpo, cara, ojos, boca, pecho), de menos de 12 KB por expresión. Las expresiones cambian solo los ojos, la boca y la luz del pecho, así se reutiliza el mismo cuerpo. Sigue los colores del tema.

**Expresiones:** saludo · feliz (todo conectado) · buscando · aparato desconectado · error de conexión · Bluetooth apagado · cargando · sin aparatos.

**Dónde aparece:** bienvenida e Inicio (saludo), estados vacíos, mensajes de éxito, ayuda contextual y presentación de una función nueva. **No aparece fijo en todas las pantallas.**

## 7. Navegación y pantallas

Barra lateral con: **Inicio · Dispositivos · Audio · Pantallas · Automatizaciones · Widgets · Ajustes.** "Teclado y ratón" se agrega cuando llegue su fase (decisión del 3 nov: todavía no). Una sección solo se muestra cuando tiene algo funcional en ese sistema.

**Barra de arriba (en todas las secciones):** a la izquierda, el buscador global "Buscar dispositivos…" con la tecla ⌘K (Ctrl K en Windows); escribir lleva a Dispositivos. A la derecha, en este orden: Actualizar y el botón de tema (sol en claro, luna en oscuro). El estado del Bluetooth va en una tarjeta al pie de la barra lateral ("Bluetooth activo · Todo en perfecto estado").

1. **Inicio**
   - "Hola, Ronny" (el nombre que la persona escribe en la bienvenida; **sin emojis en toda la app**, pedido del dueño el 3 nov), con Nexo Bot saludando y la aurora detrás.
   - Resumen en 4 bloques (KPIs): Audífonos, Pantallas, Bluetooth y Dispositivos. Cada uno lleva arriba el ícono de línea en celeste, el nombre y el valor en color (verde para estados, turquesa para cantidades). Teclado entra cuando llegue su fase.
   - Baterías y estado del Bluetooth.
   - Atajos frecuentes.
   - **Actividad reciente**, sacada del historial real ("WH-CH520 se conectó a las 12:40").
1b. **Mi setup** (`vistas/MiSetup.svelte`, escrito el 3 nov; pedido del dueño: "ver todos mis dispositivos como un mapa… decir modo estudio y que pum active eso"). Liviano: 2D, sin Three.js, sin bucles.
   - **Escenas** (`sistema/escenas.rs`, `escenas.json`): tarjetas de un toque con el paso en curso, "Listo" o qué paso falló. Cada una lleva arriba su ilustración plana (una por ícono, más "nueva"), generada con `python3 diseno/escenas/generar.py` (calco del de SIAU) en `static/escenas` y pegada en línea por `IlustracionEscena.svelte`, que sigue el tema con `lib/estilos/ilustraciones.css`. Van en una sola fila pareja: si no caben, se desliza con imán y flechas. El editor agrega pasos desde "Agregar dispositivo" (foto, nombre y equipo; lo del otro equipo se ve, pero no se elige) y el brillo puede ir a una pantalla por su nombre (`"nombre:<pantalla>"`). Pasos: brillo, conectar, desconectar, salida de sonido, volumen, micrófono, modo trabajo (solo Mac), abrir una app y luces (a la espera: nunca se ejecuta). La primera vez: Modo estudio, remoto, noche y reunión, con los aparatos que haya. También en el submenú "Escenas" del ícono.
   - **Mapa** (`sistema/setup.rs`, `setup.json`): un escritorio de frente por lugar (Casa = la Mac, Oficina = el PC) con pared, mesa y tres capas de profundidad, y las fotos reales de los aparatos y de la decoración (`static/setup`). Verde y resplandor si está encendido; tenue si no. El otro equipo llega por XDrop (`GET /nexo/aparatos`, misma seguridad); si está apagado, lo último que se vio queda tenue (`setup-visto.json`). "Editar mi setup": arrastrar, capas, tamaño, quitar y sumar lo detectado o decoración.
   - Para revisarlo fuera de la app: `?seccion=setup`, con `&setup=editar`, `apagada`, `sin`, `fallo`, `escena` (abre el editor) o `aparatos` (el editor con la lista de dispositivos).
2. **Dispositivos** (el orden importa, pedido del dueño)
   - Al centro, "Mis dispositivos", tarjetas grandes con foto, nombre, estado y chevron, y al final "+ Agregar dispositivo", que abre Ajustes › Bluetooth porque el emparejamiento lo hace el sistema.
   - **Panel a la derecha, de arriba abajo:**
     1. Foto grande con la aurora detrás y el botón ⋯.
     2. Nombre y chip de estado.
     3. Batería: anillo y "Carga completa".
     4. Volumen de salida y Micrófono.
     5. Salida de audio y Entrada de audio.
     6. Abajo, fijos: **Desconectar · Renombrar · Olvidar**, con el ícono encima del texto.
   - **Al entrar en un aparato** (chevron, doble clic, o desde Inicio) se abre su página completa:
     - A la izquierda: Atrás, foto, nombre, estado y Desconectar.
     - A la derecha, pestañas **General · Audio · Avanzado** (Teclado llega con su fase):
       - General: batería, controles de audio y conexión.
       - Audio: el panel de sonido completo.
       - Avanzado: automático, información, Renombrar y Olvidar.
3. **Audio** (un mezclador, rehecho el 3 nov: el dueño lo quería "en una barra vertical y más cositas")
   - **Mezclador:** tiras verticales para la salida en uso, el micrófono en uso y, en Mac, los avisos del sistema. Cada tira: foto o ícono (`fotoDeSonido` en `catalogo.ts`), nombre, cómo se conecta, el % en grande, una barra vertical (`DeslizadorVertical.svelte`, con teclado) y el silencio al pie. Las tiras comparten filas (subgrid), así las barras quedan alineadas.
   - Mientras se arrastra una barra, y 900 ms después, no se sigue lo que llega del sistema (`CanalAudio.svelte`). Se manda siempre el último valor.
   - **"Silenciar todo"** silencia la salida y el micrófono en uso.
   - **Aparatos:** salidas y entradas como tarjetas; tocar una la vuelve la del sistema. La que está en uso va con el estilo de "lo elegido". Abajo, "Probar sonido" (Mac: Tink con `afplay`; Windows: `MessageBeep`) y los ajustes de sonido del sistema.
   - **Volumen de avisos** (solo Mac): `osascript` con límite de tiempo, leído al entrar y al volver a la ventana. Es una parte del volumen general.
   - Avisos de calidad (el micrófono Bluetooth baja la calidad), con un botón para pasar al micrófono integrado. Nada pide el permiso de micrófono: no hay medidores en vivo.
4. **Pantallas**
   - Cada pantalla con brillo, resolución, frecuencia, cuál es la principal y su posición.
   - **Brillo mínimo** (pedido del dueño el 4 nov: "si pongo en 0, queda parpadeando"): cada monitor externo tiene un mínimo (10 % de fábrica, de 0 a 40 %, en `ajustes.json › minimosBrillo`). El 0 % de Nexo es ese mínimo y el 100 % el máximo. Se aplica en un solo lugar, el hilo de pantallas (`dispositivos/pantallas.rs`), así vale para el panel, las teclas, el menú, las escenas y los widgets. Si al conectarlo el monitor estaba por debajo del mínimo, sube al mínimo.
   - **Ajustes del monitor** (sección que se despliega en la tarjeta de cada monitor externo, `AjustesMonitor.svelte`; sin pestañas): el brillo mínimo y una rejilla con lo que el monitor dice que tiene en su lista de capacidades DDC/CI (`dispositivos/vcp.rs`, leída una vez por monitor): contraste (0x12), modo de color (0x14) con "Luz cálida" (el preset más cálido y vuelta al de antes), rojo, verde y azul (0x16/18/1A) en un modo "Usuario", nitidez (0x87), modo de imagen (0xDC), entrada (0x60, con confirmación), volumen (0x62), silencio (0x8D) y "Restaurar valores de fábrica" (0x04, con confirmación). Los valores se leen al abrir la sección, nunca en un bucle. Si el monitor no da su lista, solo se muestran los controles continuos que respondieron. Fuera de la app: `?seccion=pantallas&monitor=abierto`.
   - **Widget de brillo en esta pantalla**: interruptor por pantalla (ver la sección 10).
   - El atajo de teclas de brillo.
5. **Remoto** (hecho el 3 nov; el dueño controla la Mac desde la PC del trabajo con RustDesk por Tailscale)
   - **Estado:** quién está conectado, si va directo por Tailscale o por relé, y el estado de Tailscale y RustDesk en fichas. A la izquierda, la ilustración del PC y la Mac con Tailscale al medio (`remoto-conectado`, `remoto` o `remoto-sin`, del mismo generador).
     - Mac: "Preparar para el trabajo" y "Abrir RustDesk".
     - Windows: "Conectar a mi Mac" (`rustdesk --connect 100.88.188.99`).
   - **Atajos de Windows en la Mac** (`sistema/remoto/teclado.rs`): Ctrl → ⌘, Inicio/Fin, Alt+Tab, escritorios.
     - Solo traduce lo que llega de RustDesk (el PID de origen del evento); el teclado de la Mac no cambia.
     - En Windows, `sistema/remoto_windows.rs` atrapa lo que Windows se guarda (Win+Tab, Win+D, Ctrl+Win+←/→, Win+Shift+S, Alt+Tab, Win+E) y lo manda por su código de tecla. Requiere el modo de teclado "Mapa" de RustDesk.
     - En Logi Options+ se asignan pulsaciones de teclas, no acciones prediseñadas.
   - **Captura con Imprimir Pantalla** (F13 en la Mac): zona → PNG a 1x en el portapapeles y en Imágenes › Capturas remotas. Reemplaza a la app aparte "Captura remota".
   - **Portapapeles liviano:** durante la sesión, las imágenes grandes se reducen a 1600 px antes de que RustDesk las mande. Historial de 12 copias, sin contraseñas.
   - **Durante la sesión:** la Mac no se duerme (`caffeinate -s -w`) y el brillo baja al mínimo; al terminar, todo vuelve como estaba.
   - **Permiso nuevo y obligatorio:** Grabación de pantalla.
6. **XDrop** (antes Nexo Drop; escrito el 3 nov y renombrado el mismo día): pasar archivos, carpetas, textos, enlaces y capturas entre la Mac y el PC por Tailscale. El id de la sección sigue siendo `drop`.
   - **Receptor** (`sistema/drop/servidor.rs`): escucha solo en la dirección de Tailscale del equipo, puerto 47821, y solo atiende a equipos del mismo dueño en Tailscale (y al mismo equipo, para probar). Lo recibido queda en Descargas › XDrop (`drop::CARPETA`, igual en los dos sistemas); el texto va al portapapeles y los enlaces se abren en el navegador. Las notificaciones llevan el título "XDrop".
   - **Envío** (`sistema/drop/envio.rs`): un lote a la vez, con avance, velocidad, tiempo restante y Cancelar. Se suelta sobre cualquier parte de la ventana (o sobre el widget) y lleva a XDrop.
   - **Pantalla:** la zona para soltar es la protagonista y a su lado va el **riel de acciones** (`RielXDrop.svelte`), una columna de botones parejos: "Enviar lo que copiaste", "Abrir enlace allá" (abre su campo bajo la fila), "Enviar captura" (solo Mac: `drop_enviar_captura` usa la captura de zona de Remoto, en 1x), "Escribir una nota" (llega al portapapeles del otro equipo) y "Abrir carpeta XDrop". Para sumar una acción se agrega una fila a `acciones`. Con la ventana ancha y alta, todo cabe en la ventana: el riel reparte el alto en mosaicos y el historial se desplaza dentro de su panel.
   - **Historial** de 40 entradas (`drop.json`), con miniaturas, Abrir, Mostrar y Copiar.
   - **Ajustes**, detrás del engranaje de arriba a la derecha (`AjustesXDrop.svelte`): "Recibir archivos" y "Copiar imágenes recibidas al portapapeles" (los dos encendidos), la carpeta con "Abrir carpeta" y, en Windows, la nota del cortafuegos. Si la recepción está apagada, el engranaje lleva un punto ámbar.
7. **Automatizaciones**
   - Reglas por aparato: "al conectar, usar para el sonido", "avisar con batería baja", "al conectar el monitor, poner brillo X".
   - Lista de reglas activas.
8. **Widgets:** ver la sección 10.
9. **Ajustes**
   - Tema.
   - Arranque al iniciar sesión.
   - Permisos: cuál tiene Nexo, cuál falta y un botón para abrirlo.
   - Privacidad del historial (borrarlo).
   - Acerca de.

**Ventana:** al menos 980×640. A partir de 1360 de ancho, el detalle se reparte en dos columnas para que no quede espacio muerto. Debe verse bien con la ventana a pantalla completa.

## 8. Lo que se puede y no se puede (no simular)

| Pedido | Mac | Windows | Qué se muestra |
|---|---|---|---|
| Apagar los audífonos | ❌ Bluetooth no lo permite | ❌ | Solo "Desconectar" |
| Ecualizador o gestos de Sony/Beats | ❌ protocolo cerrado (Sony: solo ingeniería inversa) | ❌ | Nada, hasta investigar un modelo concreto |
| Intensidad de la señal | ❌ IOBluetooth devuelve 0 | ⚠️ por investigar | Nada |
| Renombrar | ⚠️ sin API pública | ⚠️ | **Alias dentro de Nexo** (el nombre del aparato no cambia) |
| Olvidar (desemparejar) | ⚠️ API privada `remove` | ✅ WinRT `UnpairAsync` | Mac: probar la API; si falla, abrir Ajustes › Bluetooth |
| Salida y entrada de audio | ✅ CoreAudio | ✅ IMMDeviceEnumerator + IPolicyConfig | Menús propios |
| Resolución y frecuencia | ✅ CGDisplayMode | ✅ ChangeDisplaySettingsEx | Menús con los modos reales |
| Pantalla principal y posición | ✅ CGConfigureDisplayOrigin | ✅ | Distribución arrastrable |
| Rotación | ⚠️ privada | ✅ | Solo donde funcione |
| Atajos del sistema (Mission Control, escritorios, capturas) | ✅ `com.apple.symbolichotkeys` y aplicar sin cerrar sesión | ❌ son fijos | Mac: editables · Windows: de referencia |
| Reasignar teclas | ✅ `hidutil` (UserKeyMapping, por teclado) | ⚠️ con Nexo abierto (gancho de teclado) o permanente con permiso de administrador | Explicar el alcance en pantalla |
| Velocidad del cursor y del desplazamiento | ⚠️ preferencias del sistema; pueden pedir volver a abrir apps | ✅ SystemParametersInfo | Solo si se aplica de verdad |
| Perfiles por app (ratón) | ⚠️ con Nexo abierto | ⚠️ | Fase 5, si se valida |
| Widgets nativos | ✅ con una extensión WidgetKit (pide Xcode completo y firma con equipo de Apple) | ✅ con un proveedor del Windows App SDK (pide empaquetar en MSIX) | **Se hacen en los dos** (fase 5, sección 10) |

## 9. Teclado y ratón (más adelante)

> **Todavía no se hace** (decisión del dueño, 3 nov 2026). Queda especificado para cuando llegue su fase: no se empieza ni se muestra en la barra lateral antes de eso.

- **Selector "macOS | Windows"** para elegir el perfil, y una lista para elegir el **teclado concreto** (cada uno se reconoce por su ID de fabricante y producto).
- **Teclado dibujado:** las teclas de la combinación se iluminan al elegir un atajo, y la distribución cambia según el idioma.
- **Editor de atajos:**
  - Graba la combinación y avisa de **conflictos** con otro atajo del sistema o de Nexo.
  - Permite guardar perfiles, restaurar los valores originales y tener perfiles por aparato y por sistema.
- **Respeta las convenciones de cada sistema.**
  - En Mac: Mission Control, cambiar de escritorio, Launchpad/Apps, capturas, Spotlight y mostrar escritorio.
  - En Windows: Win+Tab, Alt+Tab, Win+D, Win+E, Win+Shift+S y Snap Layouts.
- **Atajos propios de Nexo**, por ejemplo "captura liviana para trabajar en remoto" (hoy es la app aparte "Captura remota"): en Mac con Carbon RegisterEventHotKey, sin permiso especial; en Windows con RegisterHotKey.
- **Si algo pide un permiso** (Accesibilidad o administrador), se dice antes y no se simula.

### 9.1 Plan para los aparatos del dueño: Logitech Wave Keys y MX Master 4 (3 nov)
Se hace cuando los tenga, para probar con los aparatos reales. Nexo reemplaza a Logi Options+, que es pesado.
- **Protocolo HID++ 2.0 de Logitech** (el mismo que usa Solaar), por Bluetooth o por el receptor Logi Bolt. En Mac va con IOHIDManager y en Windows con HID.
  - Batería real de los dos (función 0x1004).
  - **Easy-Switch desde Nexo** (0x1814): un botón "Pasar teclado y mouse a la PC" y otro "a la Mac", que cambian los dos aparatos a la vez.
  - MX Master: DPI (0x2201) y SmartShift de la rueda (0x2111).
- **Botones del MX Master 4 en la Mac**, con el tap de eventos (no hace falta HID++):
  - Botón de gestos → Mission Control.
  - Botones de atrás y adelante → cambiar de escritorio.
  - Rueda del pulgar → desplazamiento horizontal.
  - **Dirección del scroll del mouse independiente del trackpad**, algo que macOS no permite.
- **Wave Keys:** perfil Mac/Windows y teclas multimedia. En la sesión remota se aplica la traducción del perfil Remoto.

## 10. Widgets y acceso rápido (obligatorio en Mac y en Windows)

El dueño los quiere **en los dos sistemas**. Hay tres niveles, que se hacen en este orden:

0. **Widget de escritorio de Nexo (hecho el 3 nov, Mac y Windows):** una tarjeta chica de 568×150 que vive en el escritorio. Tiene cuatro mosaicos: audífonos con su batería, pantallas en uso con su brillo, Bluetooth, y Nexo Bot con el estado general.
   - Se mueve arrastrando y recuerda dónde quedó.
   - Se prende y se apaga desde la bandeja ("Widget en el escritorio") y desde Ajustes; viene prendido.
   - Cada mosaico abre Nexo en su sección.
   - Se actualiza cada 30 s o cuando el vigía avisa un cambio.
   - Archivos: `sistema/widget.rs` y `routes/widget`. Cuesta un webview más mientras está prendido.

0b. **Widget de brillo de una sola pantalla (hecho el 4 nov, Mac y Windows; pedido del dueño para el Steren de la oficina):** una tarjeta de 260×96 por pantalla, con el sol, el nombre, el % en grande, la barra y − y +, en el tono del brillo.
   - Ventana `brillo-<id>` (`sistema/widget_brillo.rs`, página `routes/brillo`, `brillo?id=<id>`): sin bordes, transparente, debajo de las demás y fuera de la barra de tareas.
   - La primera vez va abajo a la derecha de la zona útil de su pantalla, a 24 px. Se arrastra y recuerda dónde quedó (`ajustes.json › widgetsBrillo`).
   - Se enciende en la tarjeta de la pantalla ("Widget de brillo en esta pantalla") o en el submenú Brillo del ícono ("Widget de brillo: …"). Clic derecho: abrir Pantallas o quitarlo.
   - Sigue el brillo con el aviso "brillo-cambio" del hilo de pantallas y lee la caché cada 2 s mientras se ve. Mientras se arrastra la barra, y 900 ms después, no sigue lo que llega.
   - Si el nombre ocupa más líneas, la ventana crece: nada se corta.
1. **Panel rápido** desde la barra de menús (Mac) o la bandeja (Windows): una ventanita sólida con baterías, salida de sonido, brillo y conectar o desconectar. Se crea al abrirla y se libera al cerrarla.
1b. **Widgets nativos de macOS: hechos el 3 nov, sin Xcode** (`widget-mac/`, se compilan con `widget-mac/construir.sh` antes de `tauri build`).
   - Cuatro widgets: Brillo (pequeño, con botones − y +), Brillo de tus pantallas (mediano), Audífonos (pequeño) y Resumen (mediano).
   - Leen `estado-widget.json`, que escribe `sistema/widgets_mac.rs`. Los botones avisan a Nexo con notificaciones del sistema, `com.ronny.nexo.brillo.<0|1>.<bajar|subir>`.
   - `Metadata.appintents` se escribe a mano (`metadatos.py`), porque no hay Xcode.
   - En la Mac, la tarjeta propia viene apagada.
2. **Widgets nativos del sistema** (los que se fijan en el escritorio o en el panel de widgets). Los dibuja el sistema, así que no gastan memoria de Nexo:
   - **macOS:** una extensión **WidgetKit** en Swift/SwiftUI dentro de `Nexo.app/Contents/PlugIns`. Nexo (Rust) escribe el estado en un JSON del **App Group** y el widget lo lee. Tamaños pequeño y mediano, con un toque que abre Nexo en ese aparato. Los botones interactivos (App Intents, macOS 14+) quedan para una segunda vuelta.
     - **Requisitos:** **Xcode completo** (hoy solo están las herramientas de línea de comandos) y firmar con un equipo de Apple, aunque sea el gratuito, por el entitlement de App Group.
   - **Windows 11:** un **proveedor de widgets** del Windows App SDK (tarjetas adaptables en JSON) que se registra en el tablero de Widgets.
     - **Requisitos:** que Nexo se empaquete como **MSIX**, más un pequeño proceso proveedor (C# o C++) que lea el mismo JSON de estado.
3. **Widgets dentro de Inicio:** batería, Bluetooth, brillo, volumen, aparatos y atajos favoritos, con el mismo diseño que los nativos.

**Widgets previstos:** batería de un aparato, todos los aparatos, estado del Bluetooth, brillo, volumen y salida, pantallas activas y estado general de Nexo.

**Contrato común:** un solo `estado.json` que escribe Rust, con aparatos, baterías, brillo, salida de sonido y la hora de actualización. Lo leen los widgets de los dos sistemas. Se reescribe solo cuando algo cambia (el vigía ya revisa cada 20 s).

## 11. Temas

- Sistema, claro u oscuro, guardado en el equipo y aplicado antes de pintar. La ventana nativa también cambia (`setTheme`).
- **El modo claro no es "fondo blanco":** el fondo es `#F5F7FB`, las superficies blancas, las superficies elevadas `#F0F3F8`, con bordes discretos y estados activos visibles.
- Se respeta `prefers-contrast: more` (bordes firmes).

## 12. Microinteracciones permitidas

- **Al conectar,** la tarjeta pasa de "Conectando…" a "Conectado". La foto gana color y el anillo se llena.
- El interruptor con resorte, el anillo que se llena al aparecer y el skeleton mientras carga.
- Al hacer clic, una presión breve (escala 0,98 durante 120 ms).
- Tooltips, menús contextuales (clic derecho en un aparato) y notificaciones nativas.
- **Nada más se mueve.**

## 13. Rendimiento y cuidado de la máquina

- **Presupuestos:** menos de 5 MB de app, ~30 MB en segundo plano y menos de 100 MB con el panel abierto. Al cerrar el panel se destruye su ventana.
- **Sondeos:** el vigía de Bluetooth cada 20 s; el brillo 4 veces por segundo solo con el panel a la vista; el sonido cada 2 s solo en el detalle; cada widget de brillo lee la caché (sin tocar el monitor) cada 2 s mientras se ve. Los ajustes del monitor por DDC/CI se leen solo al abrir su sección.
- **Sin bucles de animación y sin desenfoques** (también ahorra GPU y batería).
- **Al desarrollar:** compilar con 6 núcleos (`.cargo/config.toml`). Revisar el diseño con `pnpm build && pnpm preview` y **un solo** Chrome sin ventana, a escala 1, que se cierra enseguida. Apagar los servidores por su PID.

## 14. Arquitectura

- **La interfaz nunca llama APIs del sistema:** solo `lib/api.ts` → comandos de Tauri.
- **En Rust hay un módulo por servicio** (`bluetooth`, `audio`, `pantallas`, `teclado`, `historial`) y, dentro, la implementación de cada sistema (`#[cfg(target_os = "macos")]` y `#[cfg(windows)]`) con la misma interfaz pública.
- **Los tipos se comparten** entre `tipos.ts` y los `struct` de Rust con `serde(rename_all = "camelCase")`.
- **Para sumar un aparato:** se prepara su foto con `diseno/herramientas/preparar-imagen.swift` y se registra en `lib/catalogo.ts` con su clave "marca:modelo".
- **Firma:** hoy es local (ad-hoc), así que cada compilación pierde los permisos de Accesibilidad. Antes de distribuir hay que firmar con un certificado estable.

## 15. Fases

| Fase | Contenido | Estado |
|---|---|---|
| 1 | Tokens de la sección 4, letra, componentes de la sección 5 y Nexo Bot con sus 8 expresiones | ✅ Hecha (3 nov): paleta turquesa, superficies sólidas, barra lateral, menú y tooltip propios, diálogo, Nexo Bot en SVG (`diseno/mascota/generar.mjs`) |
| 2 | Barra lateral, Inicio y Dispositivos (lista + panel de detalle) | ✅ Hecha (3 nov) |
| 3 | Detalle de audífonos y página Audio (salida y entrada del sistema, alias, olvidar) | ✅ Hecha · falta probar Olvidar con un aparato real |
| 4 | Pantallas: resolución, frecuencia, principal y distribución | Brillo, resolución, frecuencia y principal hechos · brillo mínimo y ajustes del monitor por DDC/CI escritos (4 nov) · falta la distribución arrastrable y probar con el Steren |
| 5 | Widgets: panel rápido, WidgetKit (Mac), proveedor de widgets (Windows), `estado.json` | Widget de escritorio propio hecho (Mac y Windows) · widget de brillo por pantalla escrito (4 nov), sin probar · faltan los nativos (Xcode; MSIX en Windows) |
| 6 | Automatizaciones y Ajustes completos; versión Windows de lo anterior | Mac hecho · Windows escrito el 3 nov (Bluetooth y sonido con WASAPI y KS, monitores por DDC/CI, WMI, modos, arranque) y compilado desde la Mac con cargo-xwin · falta probarlo en el PC del trabajo |
| 7 | Accesibilidad, optimización, pruebas y firma estable | Por empezar |
| Más adelante | Teclado y ratón: perfiles macOS y Windows, atajos, conflictos, reasignar | **Pospuesto por decisión del dueño** |

## 16. Definición de terminado (cada fase)

- Funciona **con el aparato real** en el sistema donde se promete. Lo que no se pudo probar se marca ⚠️ aquí.
- Revisado en claro y oscuro, a 1080 y a 1500 de ancho, y a pantalla completa.
- `pnpm check` y `cargo check` sin errores ni avisos.
- La RAM se mide de nuevo y queda dentro de los presupuestos de la sección 13.
- Este documento queda actualizado (estado, fases y la tabla de la sección 8).

**Regla final:** no es cambiar colores y bordes. Es un producto coherente, con identidad propia y componentes reutilizables, que solo muestra lo que de verdad hace.
