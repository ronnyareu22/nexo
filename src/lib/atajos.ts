// Atajos de brillo de Windows: cómo se leen de una tecla, cómo se muestran y cuáles no se pueden usar.
// Rust revisa lo mismo al guardar (`teclas::Atajo::validar`); aquí es para avisar al instante.
import type { Atajo } from './tipos';

const F1 = 0x70;
const F24 = 0x87;

/** Ctrl, Alt, Mayús y Win (izquierdo y derecho): solos no hacen un atajo. */
const esModificador = (vk: number) => (vk >= 0x10 && vk <= 0x12) || vk === 0x5b || vk === 0x5c || (vk >= 0xa0 && vk <= 0xa5);
const esTeclaF = (vk: number) => vk >= F1 && vk <= F24;

const NOMBRES: Record<number, string> = {
  0x08: 'Retroceso',
  0x09: 'Tab',
  0x0d: 'Enter',
  0x1b: 'Esc',
  0x20: 'Espacio',
  0x21: 'RePág',
  0x22: 'AvPág',
  0x23: 'Fin',
  0x24: 'Inicio',
  0x25: '←',
  0x26: '↑',
  0x27: '→',
  0x28: '↓',
  0x2d: 'Insert',
  0x2e: 'Supr',
  0x6a: 'Num *',
  0x6b: 'Num +',
  0x6d: 'Num -',
  0x6f: 'Num /',
  0xbb: '+',
  0xbc: ',',
  0xbd: '-',
  0xbe: '.',
};

/** El nombre corto de una tecla virtual (el mismo que usa el menú del ícono). */
export function nombreTecla(vk: number): string {
  if (esTeclaF(vk)) return `F${vk - F1 + 1}`;
  if ((vk >= 0x30 && vk <= 0x39) || (vk >= 0x41 && vk <= 0x5a)) return String.fromCharCode(vk);
  if (vk >= 0x60 && vk <= 0x69) return `Num ${vk - 0x60}`;
  return NOMBRES[vk] ?? `Tecla ${vk}`;
}

/** Las teclas del atajo, en orden, para dibujarlas: ["Ctrl", "Alt", "F1"]. */
export function teclasDe(a: Atajo): string[] {
  const t: string[] = [];
  if (a.ctrl) t.push('Ctrl');
  if (a.alt) t.push('Alt');
  if (a.shift) t.push('Mayús');
  if (a.win) t.push('Win');
  t.push(nombreTecla(a.vk));
  return t;
}

export const textoAtajo = (a: Atajo) => teclasDe(a).join(' + ');

export const mismoAtajo = (a: Atajo, b: Atajo) =>
  a.vk === b.vk && a.ctrl === b.ctrl && a.alt === b.alt && a.shift === b.shift && a.win === b.win;

/**
 * El atajo de una tecla presionada. null si es solo un modificador (se espera la tecla que sigue).
 * En Windows (WebView2), `keyCode` es la tecla virtual de Windows; si no llega, las teclas F salen de `code`.
 */
export function atajoDe(e: KeyboardEvent): Atajo | null {
  let vk = e.keyCode;
  if (!vk || vk > 0xfe) {
    const f = /^F(\d{1,2})$/.exec(e.code);
    vk = f ? F1 + Number(f[1]) - 1 : 0;
  }
  if (!vk || esModificador(vk)) return null;
  return { vk, ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, win: e.metaKey };
}

/** Para qué usa Windows (o casi toda app) esa combinación; null si está libre. */
function reservado(a: Atajo): string | null {
  const solo = (ctrl: boolean, alt: boolean, shift: boolean, win: boolean) =>
    a.ctrl === ctrl && a.alt === alt && a.shift === shift && a.win === win;
  const vk = a.vk;
  if (solo(false, false, false, true)) {
    if (vk === 0x4c) return 'bloquear la computadora';
    if (vk === 0x44) return 'mostrar el escritorio';
    if (vk === 0x09) return 'la vista de tareas';
    if (vk >= 0x25 && vk <= 0x28) return 'acomodar las ventanas';
    if ((vk >= 0x30 && vk <= 0x39) || (vk >= 0x41 && vk <= 0x5a) || vk === 0xbe || vk === 0x2c) return 'sus atajos';
    return null;
  }
  if (solo(false, false, true, true) && vk === 0x53) return 'recortar la pantalla';
  if (solo(false, true, false, false)) {
    if (vk === 0x73) return 'cerrar la ventana';
    if (vk === 0x09) return 'cambiar de ventana';
    if (vk === 0x1b || vk === 0x20) return 'el menú de la ventana';
    return null;
  }
  if (solo(true, false, false, false)) {
    if (vk === 0x1b) return 'abrir Inicio';
    if (vk >= 0x41 && vk <= 0x5a) return 'los atajos de las apps (copiar, pegar, deshacer…)';
    return null;
  }
  if (solo(true, false, true, false) && vk === 0x1b) return 'abrir el Administrador de tareas';
  if (solo(true, true, false, false) && vk === 0x2e) return 'la pantalla de seguridad';
  return null;
}

/** Por qué no sirve un atajo (texto para la persona), o null si sirve. */
export function problemaAtajo(a: Atajo): string | null {
  if (!a.ctrl && !a.alt && !a.win && !esTeclaF(a.vk)) return 'Usa Ctrl, Alt o Win con otra tecla, o una tecla F sola.';
  const uso = reservado(a);
  return uso ? `Windows usa ${textoAtajo(a)} para ${uso}. Elige otra.` : null;
}
