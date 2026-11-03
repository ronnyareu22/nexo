# Nexo

Centro de control de dispositivos para macOS y Windows (Tauri 2 + Svelte 5 + Rust).

- **Antes de tocar la interfaz o sumar funciones, lee `DISENO.md`**: identidad, tokens, pantallas, qué permite cada sistema (no simular controles) y las fases.
- Cómo correrlo y la estructura de carpetas: `README.md`.
- Rust está en `/opt/homebrew/opt/rustup/bin` (agrégalo al PATH).
- Para revisar el diseño sin la app: `pnpm build && pnpm preview` (datos de ejemplo; `?ver=<dirección>` abre un detalle). Usa un solo Chrome sin ventana, a escala 1, y ciérralo enseguida; apaga el servidor por su PID.
- Al terminar una fase, actualiza el estado en `DISENO.md` (secciones 2, 8 y 15).
