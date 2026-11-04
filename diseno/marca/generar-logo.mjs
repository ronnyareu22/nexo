// Genera el logo de Nexo (la "N" de cinta de la guía de marca) en SVG, limpio y simétrico.
//
// La cinta es UNA sola línea central suave (punta izquierda → curva izquierda → diagonal → curva derecha →
// punta derecha) con grosor variable: filo en las puntas y cuerpo parejo. Se corta en tres piezas que encajan
// sin costuras; la diagonal se dibuja encima y las curvas llevan una sombra suave junto al pliegue, como en la guía.
// Todo con degradados (sin filtros), para que el borde quede nítido en cualquier tamaño.
// Uso: node diseno/marca/generar-logo.mjs   → escribe los SVG en diseno/marca/
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const aqui = dirname(fileURLToPath(import.meta.url));

// Caja de 100×66.6 medida sobre el logo de la guía, simétrica al girarla 180° alrededor de (50.8, 33.3).
// [x, y, grosor]: las curvas miden ~18.5 y la diagonal ~22 (en la guía la diagonal es más ancha).
const ALTO = 66.6;
const mitad = [
  [39.4, 63.7, 14], // punta de abajo (curva izquierda), mirando a la derecha
  [24.5, 61.2, 16],
  [12.6, 52, 18],
  [9.6, 38, 18.5],
  [11.2, 24.4, 18.8],
  [18.2, 13.4, 19],
  [28.6, 7.4, 19.5],
  [37, 11.2, 20.5], // esquina de arriba: aquí empieza la diagonal
];
const espejo = ([x, y, g]) => [101.6 - x, ALTO - y, g];
const centro = [50.8, ALTO / 2, 22];
const linea = [...mitad, centro, ...mitad.map(espejo).reverse()];
const esquina1 = mitad.length - 1; // índice del punto de control de cada esquina
const esquina2 = mitad.length + 1;

// Catmull-Rom centrípeta: devuelve muestras y en qué muestra cae cada punto de control.
function spline(puntos, porTramo = 90) {
  const p = [puntos[0], ...puntos, puntos[puntos.length - 1]];
  const muestras = [];
  const marcas = [];
  for (let i = 1; i < p.length - 2; i++) {
    marcas.push(muestras.length);
    const [p0, p1, p2, p3] = [p[i - 1], p[i], p[i + 1], p[i + 2]];
    const d = (a, b) => Math.pow(Math.hypot(b[0] - a[0], b[1] - a[1]) || 1e-6, 0.5);
    const t0 = 0, t1 = d(p0, p1), t2 = t1 + d(p1, p2), t3 = t2 + d(p2, p3);
    for (let k = 0; k < porTramo; k++) {
      const u = t1 + ((t2 - t1) * k) / porTramo;
      const m = (a, b, ta, tb) => a.map((v, j) => ((tb - u) / (tb - ta)) * v + ((u - ta) / (tb - ta)) * b[j]);
      const a1 = m(p0, p1, t0, t1), a2 = m(p1, p2, t1, t2), a3 = m(p2, p3, t2, t3);
      muestras.push(m(m(a1, a2, t0, t2), m(a2, a3, t1, t3), t1, t2));
    }
  }
  marcas.push(muestras.length);
  muestras.push(puntos[puntos.length - 1]);
  return { muestras, marcas };
}

const { muestras: crudas, marcas } = spline(linea);
// El grosor se suaviza a lo largo de la cinta (promedio móvil), así las puntas afinan sin escalones.
const muestras = crudas.map((m, i) => {
  let suma = 0, n = 0;
  for (let k = Math.max(0, i - 25); k <= Math.min(crudas.length - 1, i + 25); k++) { suma += crudas[k][2]; n++; }
  // Puntas en filo: el grosor sube suave (curva seno) en el primer y último 16 % de la cinta.
  const borde = Math.min(i, crudas.length - 1 - i) / (crudas.length * 0.25);
  const afinado = borde >= 1 ? 1 : Math.sin((borde * Math.PI) / 2);
  return [m[0], m[1], (suma / n) * (0.06 + 0.94 * afinado)];
});
// Bordes de la cinta en cada muestra (perpendiculares a la línea central).
const bordes = muestras.map((c, i) => {
  // Tangente promediada en una ventana corta: bordes sin ondas.
  const a = muestras[Math.max(0, i - 4)], b = muestras[Math.min(muestras.length - 1, i + 4)];
  let dx = b[0] - a[0], dy = b[1] - a[1];
  const l = Math.hypot(dx, dy) || 1;
  dx /= l; dy /= l;
  const g = Math.max(0, c[2]) / 2;
  return [[c[0] - dy * g, c[1] + dx * g], [c[0] + dy * g, c[1] - dx * g]];
});

// Bordes suavizados (promedio móvil de cada borde): curvas limpias, sin quiebres en las partes muy curvas.
for (const lado of [0, 1]) {
  const original = bordes.map((b) => b[lado]);
  for (let i = 1; i < original.length - 1; i++) {
    const v = Math.min(32, i, original.length - 1 - i);
    let sx = 0, sy = 0;
    for (let k = i - v; k <= i + v; k++) { sx += original[k][0]; sy += original[k][1]; }
    bordes[i][lado] = [sx / (2 * v + 1), sy / (2 * v + 1)];
  }
}

const f = (n) => n.toFixed(2);
function pieza(desde, hasta) {
  const tramo = bordes.slice(desde, hasta + 1);
  const ida = tramo.map((b) => b[0]);
  const vuelta = tramo.map((b) => b[1]).reverse();
  return 'M' + [...ida, ...vuelta].map(([x, y]) => `${f(x)} ${f(y)}`).join('L') + 'Z';
}

const corte1 = marcas[esquina1];
const corte2 = marcas[esquina2];
const piezas = {
  izquierda: pieza(0, corte1),
  diagonal: pieza(corte1, corte2),
  derecha: pieza(corte2, muestras.length - 1),
};
const todo = pieza(0, muestras.length - 1);

// Colores de la guía de marca: turquesa (#00E6B8) hacia el azul de acento (#3B82F6), un poco más vivos.
const defs = (id) => `
  <defs>
    <linearGradient id="${id}-g" x1="4" y1="2" x2="98" y2="64" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#3EF3D3"/>
      <stop offset="0.48" stop-color="#22C6F0"/>
      <stop offset="1" stop-color="#2F6CF4"/>
    </linearGradient>
    <linearGradient id="${id}-d" x1="36" y1="10" x2="66" y2="58" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#4BF5DA"/>
      <stop offset="0.55" stop-color="#28B9F2"/>
      <stop offset="1" stop-color="#2E66F2"/>
    </linearGradient>
    <linearGradient id="${id}-luz" x1="0" y1="0" x2="0" y2="66.6" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.30"/>
      <stop offset="0.5" stop-color="#FFFFFF" stop-opacity="0.04"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </linearGradient>
    <radialGradient id="${id}-pliegue1" cx="29" cy="22" r="14" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#031426" stop-opacity="0.55"/>
      <stop offset="1" stop-color="#031426" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="${id}-pliegue2" cx="72.6" cy="44.6" r="14" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#031426" stop-opacity="0.55"/>
      <stop offset="1" stop-color="#031426" stop-opacity="0"/>
    </radialGradient>
    <clipPath id="${id}-ci"><path d="${piezas.izquierda}"/></clipPath>
    <clipPath id="${id}-cd"><path d="${piezas.derecha}"/></clipPath>
  </defs>`;

// La N dentro de un cuadro de lado `lado`, centrada.
function logo(id, lado = 100, x = 0, y = 0) {
  // La forma mide ~104 de ancho con el grosor de las curvas: se reduce un poco y se centra.
  const s = (lado / 100) * 0.9;
  x += (lado - 101.6 * s) / 2 + 0.8 * s;
  return `<g transform="translate(${x - 0.8 * s} ${y + (lado - ALTO * s) / 2}) scale(${s})">
    <path d="${piezas.izquierda}" fill="url(#${id}-g)"/>
    <path d="${piezas.derecha}" fill="url(#${id}-g)"/>
    <rect x="0" y="0" width="102" height="${ALTO}" fill="url(#${id}-pliegue1)" clip-path="url(#${id}-ci)"/>
    <rect x="0" y="0" width="102" height="${ALTO}" fill="url(#${id}-pliegue2)" clip-path="url(#${id}-cd)"/>
    <path d="${piezas.diagonal}" fill="url(#${id}-d)"/>
    <path d="${todo}" fill="url(#${id}-luz)"/>
  </g>`;
}

// 1. Logo a color, fondo transparente (cuadrado, la N centrada).
writeFileSync(
  join(aqui, 'logo-n.svg'),
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="512" height="512">${defs('n')}${logo('n')}</svg>\n`,
);

// 2. Un solo color, sin luces ni sombras: barra de menús de macOS (la pinta según el tema) y usos monocromos.
writeFileSync(
  join(aqui, 'logo-n-mono.svg'),
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="512" height="512">` +
    `<g transform="translate(${(100 - 101.6 * 0.9) / 2} ${(100 - ALTO * 0.9) / 2}) scale(0.9)"><path d="${todo}" fill="#000"/></g></svg>\n`,
);

// 3. Ícono de la app (1024): el cuadrado redondeado oscuro de la guía, un borde fino de luz,
//    una aurora suave detrás y la N grande. Margen de 100 px como piden macOS y Windows.
writeFileSync(
  join(aqui, 'icono-app.svg'),
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  ${defs('i')}
  <defs>
    <linearGradient id="fondo" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#1A2333"/>
      <stop offset="1" stop-color="#070A10"/>
    </linearGradient>
    <linearGradient id="borde" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#3EF3D3" stop-opacity="0.6"/>
      <stop offset="0.5" stop-color="#3EF3D3" stop-opacity="0.06"/>
      <stop offset="1" stop-color="#2F6CF4" stop-opacity="0.45"/>
    </linearGradient>
    <radialGradient id="aurora" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="#22C6F0" stop-opacity="0.26"/>
      <stop offset="1" stop-color="#22C6F0" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <rect x="100" y="100" width="824" height="824" rx="186" fill="url(#fondo)"/>
  <rect x="100" y="100" width="824" height="824" rx="186" fill="url(#aurora)"/>
  <rect x="103" y="103" width="818" height="818" rx="183" fill="none" stroke="url(#borde)" stroke-width="6"/>
  ${logo('i', 700, 162, 162)}
</svg>\n`,
);

// 4. Componente para la app: la N a color, con ids únicos para poder usarla varias veces en la misma vista.
const componente = `<!-- Logo de Nexo: la N de cinta de la guía de marca. Generado por diseno/marca/generar-logo.mjs; no editar a mano. -->
<script lang="ts">
  let { tamano = 28, etiqueta = 'Nexo' }: { tamano?: number; etiqueta?: string } = $props();
  const id = 'nexo-' + Math.random().toString(36).slice(2, 8);
</script>

<svg width={tamano} height={tamano} viewBox="0 0 100 100" role="img" aria-label={etiqueta}>
  {@html \`${defs('ID').replaceAll('ID-', '${id}-').replace(/\n\s*/g, '')}${logo('ID').replaceAll('ID-', '${id}-').replace(/\n\s*/g, '')}\`}
</svg>
`;
writeFileSync(join(aqui, '../../src/lib/componentes/LogoNexo.svelte'), componente);

// 5. Logo horizontal (N + "Nexo") para la barra lateral y la bienvenida: solo la N; la palabra la pone la app con Inter.
console.log('Listo: logo-n.svg, logo-n-mono.svg, icono-app.svg');
