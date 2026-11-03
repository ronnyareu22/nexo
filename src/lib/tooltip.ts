// Tooltip propio (en lugar del `title` gris del navegador): use:tooltip={texto}. Sin texto, no hay tooltip.
// Hay uno solo para toda la app, dentro de <body>. Aparece tras 450 ms con el mouse encima, o al llegar con el teclado;
// se va al salir, al perder el foco, al hacer clic, al desplazar o con Escape. Solo se funde, no se mueve.
// En pantallas sin mouse (táctiles) no hace nada. Estilos: estilos/global.css («Tooltip propio») y la clase .flotante.

import type { Action } from 'svelte/action';

const ESPERA = 450;
const SEPARACION = 8;
const MARGEN = 6;
const ID = 'nexo-tooltip';

let globo: HTMLDivElement | null = null;
let dueno: HTMLElement | null = null;
let alSoltar: (() => void) | null = null;

function pieza(): HTMLDivElement {
  if (!globo || !globo.isConnected) {
    globo = document.createElement('div');
    globo.id = ID;
    globo.className = 'tooltip flotante';
    globo.setAttribute('role', 'tooltip');
    globo.setAttribute('aria-hidden', 'true');
    document.body.appendChild(globo);
  }
  return globo;
}

function mostrar(el: HTMLElement, texto: string) {
  const g = pieza();
  if (dueno && dueno !== el) ocultar(dueno);
  else alSoltar?.();
  dueno = el;
  g.textContent = texto;
  g.style.left = '0px'; // para medir su ancho natural
  g.setAttribute('aria-hidden', 'false');
  // Si el texto ya es el nombre accesible, no se repite al lector de pantalla.
  if (el.getAttribute('aria-label') !== texto && !el.hasAttribute('aria-describedby')) {
    el.setAttribute('aria-describedby', ID);
  }

  // Arriba del elemento; si no cabe, abajo. Centrado y sin salirse de la ventana.
  const r = el.getBoundingClientRect();
  const ancho = g.offsetWidth;
  const alto = g.offsetHeight;
  let top = r.top - alto - SEPARACION;
  if (top < MARGEN) top = r.bottom + SEPARACION;
  const left = Math.min(Math.max(r.left + r.width / 2 - ancho / 2, MARGEN), window.innerWidth - ancho - MARGEN);
  g.style.left = `${Math.round(left)}px`;
  g.style.top = `${Math.round(top)}px`;
  g.classList.add('visible');

  const ocultarYa = () => ocultar(el);
  const tecla = (e: KeyboardEvent) => {
    if (e.key === 'Escape') ocultarYa();
  };
  window.addEventListener('scroll', ocultarYa, { capture: true, passive: true });
  window.addEventListener('resize', ocultarYa, { passive: true });
  window.addEventListener('keydown', tecla);
  alSoltar = () => {
    window.removeEventListener('scroll', ocultarYa, { capture: true });
    window.removeEventListener('resize', ocultarYa);
    window.removeEventListener('keydown', tecla);
    alSoltar = null;
  };
}

function ocultar(el: HTMLElement) {
  if (dueno !== el) return;
  alSoltar?.();
  dueno = null;
  if (el.getAttribute('aria-describedby') === ID) el.removeAttribute('aria-describedby');
  globo?.classList.remove('visible');
  globo?.setAttribute('aria-hidden', 'true');
}

export const tooltip: Action<HTMLElement, string | undefined> = (nodo, inicial) => {
  let texto = inicial ?? '';
  // Sin mouse no hay "pasar por encima": no se arma nada.
  if (typeof window === 'undefined' || !matchMedia('(hover: hover) and (pointer: fine)').matches) {
    return { update: (t) => void (texto = t ?? ''), destroy: () => {} };
  }

  let espera: ReturnType<typeof setTimeout> | undefined;
  const cancelar = () => clearTimeout(espera);

  const entrar = () => {
    cancelar();
    if (texto) espera = setTimeout(() => texto && mostrar(nodo, texto), ESPERA);
  };
  const salir = () => {
    cancelar();
    ocultar(nodo);
  };
  const enfocar = () => {
    // Solo al llegar con el teclado; un clic con el mouse no lo muestra.
    if (texto && nodo.matches(':focus-visible')) {
      cancelar();
      mostrar(nodo, texto);
    }
  };

  nodo.addEventListener('pointerenter', entrar);
  nodo.addEventListener('pointerleave', salir);
  nodo.addEventListener('pointerdown', salir);
  nodo.addEventListener('focus', enfocar);
  nodo.addEventListener('blur', salir);

  return {
    update(nuevo) {
      texto = nuevo ?? '';
      if (dueno !== nodo) return;
      if (!texto) ocultar(nodo);
      else if (globo) globo.textContent = texto;
    },
    destroy() {
      cancelar();
      ocultar(nodo);
      nodo.removeEventListener('pointerenter', entrar);
      nodo.removeEventListener('pointerleave', salir);
      nodo.removeEventListener('pointerdown', salir);
      nodo.removeEventListener('focus', enfocar);
      nodo.removeEventListener('blur', salir);
    },
  };
};
