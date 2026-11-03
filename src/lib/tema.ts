// Tema de la app: el del sistema, claro u oscuro. Se guarda en este equipo (localStorage).
// app.html lo aplica antes de pintar para que no parpadee; aquí se cambia desde el selector.

export type Tema = 'sistema' | 'claro' | 'oscuro';
const CLAVE = 'nexo-tema';

export function leerTema(): Tema {
  try {
    const t = localStorage.getItem(CLAVE);
    return t === 'claro' || t === 'oscuro' ? t : 'sistema';
  } catch {
    return 'sistema';
  }
}

export function aplicarTema(tema: Tema) {
  const html = document.documentElement;
  if (tema === 'sistema') delete html.dataset.theme;
  else html.dataset.theme = tema === 'claro' ? 'light' : 'dark';
  // Los selectores de tema que haya abiertos (arriba y en Ajustes) se ponen al día.
  window.dispatchEvent(new CustomEvent<Tema>('nexo-tema', { detail: tema }));
  try {
    localStorage.setItem(CLAVE, tema);
  } catch {
    // Sin almacenamiento: el tema dura hasta cerrar el panel.
  }
  // La ventana nativa (botones de arriba y barra de título) en el mismo tema.
  if ('__TAURI_INTERNALS__' in window) {
    import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) =>
        getCurrentWindow().setTheme(tema === 'sistema' ? null : tema === 'claro' ? 'light' : 'dark'),
      )
      .catch(() => {});
  }
}
