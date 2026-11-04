# Nexo

Lee primero **DISENO.md** (identidad, pantallas y qué permite cada sistema).

Tus dispositivos en un solo panel: audífonos y aparatos Bluetooth, monitores y luces. App nativa para **Mac y Windows** hecha con [Tauri](https://tauri.app) (Rust y Svelte).

- Vive en la barra de menús (en Windows, en la bandeja) con su ícono. Un clic abre el panel.
- Al cerrar el panel se libera su memoria: queda en segundo plano con unos 30 MB.
- Pesa unos 4 MB. Con el panel abierto usa unos 100 MB.

## Qué hace hoy

| Aparatos | Qué puedes hacer | Mac | Windows |
|---|---|---|---|
| Bluetooth (audífonos, teléfono, teclado…) | Ver si está conectado y su batería; conectar y desconectar | ✅ | escrito, sin probar |
| Historial y automático | Desde cuándo está conectado, última vez usado, uso de hoy y de 7 días, estimación y gráfica de batería; usar para el sonido al conectarse; aviso de batería baja | ✅ | escrito, sin probar |
| Sonido de audífonos y parlantes | Volumen, silencio, usarlos como salida; usar su micrófono y su volumen | ✅ | escrito, sin probar (WASAPI) |
| Pantallas | Brillo de la MacBook (DisplayServices) y de monitores externos (DDC/CI); resolución y principal | ✅ (externo sin probar) | escrito, sin probar (DDC/CI y WMI) |
| Widget de escritorio | Audífonos, pantallas, Bluetooth y estado; abre Nexo en su sección | ✅ | ✅ (sin probar) |
| Teclas de brillo | Con el mouse sobre el monitor externo, las teclas de la Mac cambian ese monitor y aparece un aviso de vidrio | ✅ (sin probar) | — |
| Luces (Wi-Fi o Bluetooth) | Encender, brillo, color | pendiente | pendiente |

## Carpetas

```
nexo/
├── diseno/                  Íconos en SVG y herramienta para preparar fotos
├── static/dispositivos/     Fotos de los modelos conocidos
├── src/                     Interfaz (Svelte)
│   ├── routes/+page.svelte  Estructura: barra lateral y la sección elegida (?seccion=, ?ver=)
│   ├── lib/vistas/          Inicio, Dispositivos, Audio, Pantallas, Automatizaciones, Ajustes
│   └── lib/
│       ├── api.ts           Llamadas a Rust (y datos de ejemplo fuera de la app)
│       ├── tipos.ts         Tipos compartidos con Rust
│       ├── estilos/         Colores y letra (oscuro y claro)
│       └── componentes/     Tarjeta, detalle, batería, ilustraciones, íconos
└── src-tauri/               Parte nativa (Rust)
    ├── src/lib.rs           Arranque, ícono de la barra y ventana
    ├── src/dispositivos/    Un módulo por familia de aparatos
    │   ├── mod.rs           Tipos y comandos comunes
    │   ├── bluetooth.rs     Mac: system_profiler e IOBluetooth
    │   ├── audio.rs         Mac: volumen y micrófono con CoreAudio
    │   ├── historial.rs     Sesiones de uso, muestras de batería y ajustes automáticos (historial.json)
    │   └── pantallas.rs     Brillo: DisplayServices (MacBook) y DDC/CI (externos), en un hilo propio
    ├── src/sistema/         Integración con macOS
    │   ├── teclas.rs        Escucha las teclas de brillo (permiso de Accesibilidad)
    │   ├── aviso.rs         Ventanita del aviso de brillo (se libera tras 60 s sin uso)
    │   └── ajustes.rs       Ajustes guardados (ajustes.json)
    ├── Info.plist           Texto del permiso de Bluetooth
    └── .cargo/config.toml   Compila con 6 núcleos para no calentar la Mac
```

## Fotos de los aparatos

Los modelos conocidos se muestran con su foto real, y los demás con una ilustración.

1. Prepara la foto: `swift diseno/herramientas/preparar-imagen.swift foto.jpg static/dispositivos/marca-modelo.png --quitar-blanco`. Con `--quitar-blanco` el fondo blanco y las marcas de agua claras quedan transparentes.
2. Agrégala a `src/lib/catalogo.ts` con su clave `"marca:modelo"` (por ejemplo `0x054C:0x0EAD`). Son `device_vendorID` y `device_productID` de `system_profiler SPBluetoothDataType`.

## Cómo trabajar

Necesitas Node y pnpm, y Rust (en la Mac, Rust está en `/opt/homebrew/opt/rustup/bin`).

```sh
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
pnpm install
pnpm tauri dev                     # abre la app y se recarga al guardar
pnpm tauri build --bundles app     # app final en src-tauri/target/release/bundle/macos/Nexo.app
pnpm check                         # revisa la interfaz
```

Para ver solo el diseño en el navegador (con datos de ejemplo): `pnpm build && pnpm preview`. Con `?ver=<dirección>` se abre directo el detalle de un aparato.

**Windows, desde la Mac** (instalador NSIS `.exe`): hace falta `brew install llvm lld nsis`, `rustup target add x86_64-pc-windows-msvc` y `cargo install --locked cargo-xwin`. Después:

```
export PATH=/opt/homebrew/opt/rustup/bin:/opt/homebrew/opt/llvm/bin:$HOME/.cargo/bin:$PATH
pnpm tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
```

El instalador queda en `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`. Para revisar sin compilar: `cargo xwin check --target x86_64-pc-windows-msvc`.

## Permisos en la Mac

- **Bluetooth:** se pide al conectar un aparato por primera vez.
- **Accesibilidad:** solo si activas "Teclas de brillo en el monitor bajo el mouse". Como la app se firma localmente, cada vez que se vuelve a compilar macOS la toma como nueva y hay que activar el permiso otra vez en Ajustes del Sistema › Privacidad y seguridad › Accesibilidad.

## Siguientes pasos

1. Probar la versión Windows en el PC del trabajo: Bluetooth, sonido, y el Steren de 27" por DDC/CI (activado en su menú).
3. Luces: según la barra que se compre (Yeelight por LAN, Tuya, Govee o Bluetooth).
4. Escenas ("Trabajo", "Noche"), automatizaciones y control hablado con Claude.
