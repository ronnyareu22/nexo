// Nexo Bot: fuente única de la mascota en vector.
// Genera las 8 expresiones en diseno/mascota/expresiones/*.svg y el bloque marcado de
// src/lib/componentes/NexoBot.svelte (entre «generado:inicio» y «generado:fin»).
// Uso: node diseno/mascota/generar.mjs
//
// Capas: sombra · cuerpo (piernas, caderas, pies, cuello, torso) · pecho · brazos (según la pose)
// · cabeza (diadema, audífonos, casco, pantalla) con cara (ojos y boca) · extra (?, z, ondas).
// La cabeza va en coordenadas rectas y se inclina 8° como en la referencia. Luz desde arriba a la izquierda.

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const aqui = dirname(fileURLToPath(import.meta.url));
const raiz = join(aqui, '..', '..');

const CIAN = '#39D5FF';
const AMBAR = '#F59E0B';
const BORDE = '#C9D1DD'; // contorno muy suave para que el blanco se lea sobre fondo claro
const CENTRO = 120; // eje del torso para reflejar los brazos

export const EXPRESIONES = {
  saludo: 'Nexo Bot saluda',
  feliz: 'Nexo Bot contento',
  buscando: 'Nexo Bot buscando dispositivos',
  desconectado: 'Nexo Bot triste: el dispositivo se desconectó',
  error: 'Nexo Bot preocupado: error de conexión',
  'bluetooth-apagado': 'Nexo Bot dormido: el Bluetooth está apagado',
  cargando: 'Nexo Bot cargando',
  'sin-dispositivos': 'Nexo Bot curioso: no hay dispositivos',
};

// ---------- utilidades ----------
const f = (n) => String(Math.round(n * 10) / 10).replace(/^(-?)0\./, '$1.');
const at = (o) =>
  Object.entries(o)
    .filter(([, v]) => v !== undefined && v !== null && v !== '')
    .map(([k, v]) => ` ${k}="${v}"`)
    .join('');

function construir(modo) {
  const url = (k) => (modo === 'svelte' ? `url(#{p}${k})` : `url(#nb${k})`);
  const id = (k) => (modo === 'svelte' ? `{p}${k}` : `nb${k}`);
  const B = url('b'); // blanco
  const O = url('o'); // articulaciones

  // Cápsula (extremidades y dedos) como trayecto relleno.
  const cap = (x1, y1, x2, y2, r, a = {}) => {
    const L = Math.hypot(x2 - x1, y2 - y1);
    const nx = (-(y2 - y1) / L) * r;
    const ny = ((x2 - x1) / L) * r;
    const R = `${f(r)} ${f(r)} 0 0 0`;
    return `<path d="M${f(x1 + nx)} ${f(y1 + ny)}L${f(x2 + nx)} ${f(y2 + ny)}A${R} ${f(x2 - nx)} ${f(y2 - ny)}L${f(x1 - nx)} ${f(y1 - ny)}A${R} ${f(x1 + nx)} ${f(y1 + ny)}Z"${at(a)}/>`;
  };
  const elip = (cx, cy, rx, ry, rot = 0, a = {}) =>
    `<ellipse cx="${f(cx)}" cy="${f(cy)}" rx="${f(rx)}" ry="${f(ry)}"${rot ? ` transform="rotate(${f(rot)} ${f(cx)} ${f(cy)})"` : ''}${at(a)}/>`;
  const rect = (x, y, w, h, r, rot = 0, a = {}) =>
    `<rect x="${f(x)}" y="${f(y)}" width="${f(w)}" height="${f(h)}" rx="${f(r)}"${rot ? ` transform="rotate(${f(rot)} ${f(x + w / 2)} ${f(y + h / 2)})"` : ''}${at(a)}/>`;
  const path = (d, a = {}) => `<path d="${d}"${at(a)}/>`;

  // Luz con halo: dos trazos anchos y transparentes debajo del trazo principal (sin filtros).
  const luz = (d, w, color = CIAN) =>
    `<g fill="none" stroke="${color}" stroke-linecap="round" stroke-linejoin="round">` +
    `<path d="${d}" stroke-width="${f(w * 2.6)}" opacity=".1"/>` +
    `<path d="${d}" stroke-width="${f(w * 1.7)}" opacity=".2"/>` +
    `<path d="${d}" stroke-width="${f(w)}"/></g>`;
  // Forma rellena con halo.
  const luzLlena = (d, color = CIAN) =>
    `<path d="${d}" fill="none" stroke="${color}" stroke-width="4" opacity=".2" stroke-linejoin="round"/><path d="${d}" fill="${color}"/>`;
  const circ = (cx, cy, r) => `M${f(cx - r)} ${f(cy)}a${f(r)} ${f(r)} 0 1 0 ${f(2 * r)} 0a${f(r)} ${f(r)} 0 1 0 ${f(-2 * r)} 0Z`;

  // Grupo de piezas blancas: degradado y contorno suave heredados.
  const blancas = (s) => `<g fill="${B}" stroke="${BORDE}" stroke-width=".9">${s}</g>`;
  const oscuras = (s) => `<g fill="${O}">${s}</g>`;

  // ---------- definiciones ----------
  const defs = (ambar) =>
    '<defs>' +
    `<linearGradient id="${id('b')}" x1="0" y1="0" x2=".75" y2="1"><stop offset="0" stop-color="#fff"/><stop offset=".45" stop-color="#F4F6FA"/><stop offset="1" stop-color="#DDE3EC"/></linearGradient>` +
    `<linearGradient id="${id('o')}" x1="0" y1="0" x2=".6" y2="1"><stop offset="0" stop-color="#4A525F"/><stop offset=".5" stop-color="#2B313B"/><stop offset="1" stop-color="#1A1E25"/></linearGradient>` +
    `<radialGradient id="${id('s')}" cx=".32" cy=".22" r=".85"><stop offset="0" stop-color="#1A2230"/><stop offset=".6" stop-color="#05070B"/></radialGradient>` +
    `<radialGradient id="${id('g')}"><stop offset="0" stop-color="${CIAN}" stop-opacity=".55"/><stop offset="1" stop-color="${CIAN}" stop-opacity="0"/></radialGradient>` +
    (ambar
      ? `<radialGradient id="${id('a')}"><stop offset="0" stop-color="${AMBAR}" stop-opacity=".5"/><stop offset="1" stop-color="${AMBAR}" stop-opacity="0"/></radialGradient>`
      : '') +
    `<radialGradient id="${id('h')}"><stop offset="0" stop-color="#0B1220" stop-opacity=".2"/><stop offset="1" stop-color="#0B1220" stop-opacity="0"/></radialGradient>` +
    '</defs>';

  // ---------- cuerpo (compartido) ----------
  const cuerpo =
    '<g class="cuerpo">' +
    elip(124, 229, 70, 7, 0, { fill: url('h') }) +
    blancas(
      path('M89 166C100 162 114 168 118 178L115 208H65C69 190 77 174 89 166Z') +
        path('M134 176C142 170 160 170 167 177C171 186 174 197 175 208H129C129 196 130 185 134 176Z'),
    ) +
    luz('M83.5 178.5L74 198.5', 2.4) +
    luz('M168.5 183L171.8 196.5', 2.4) +
    blancas(path('M58 223C58 208 72 200 88 200C105 200 117 208 117 221Z') + path('M123 220C123 206 138 198 157 198C177 198 192 206 192 219Z')) +
    oscuras(rect(56, 218, 63, 13, 6.5) + rect(121, 215, 73, 13, 6.5)) +
    luz('M106.5 213.5L108.3 220.5', 3) +
    luz('M148 212.5L150.3 219.5', 3) +
    oscuras(elip(101, 175, 16, 9, 32) + elip(143, 180, 13, 8.5, -38) + elip(128, 118, 19, 7)) +
    blancas(path('M120 116C142 116 156 132 156 153C156 174 141 188 120 188C99 188 85 174 85 152C85 131 99 116 120 116Z')) +
    '</g>';

  // ---------- pecho ----------
  const N = 'M124 148V141.5C124 137 128.5 136.5 130.2 140L132 144C133.7 147.6 138 147 138 142.5V138';
  const pecho = (estado) => {
    const disco = elip(131, 143, 12.5, 12.5, 0, { fill: '#0B1017', stroke: '#3A424E', 'stroke-width': '1' });
    if (estado === 'luz' || estado === 'ambar') {
      const c = estado === 'luz' ? CIAN : AMBAR;
      return (
        `<g class="pecho">` +
        elip(131, 143, 21, 21, 0, { fill: url(estado === 'luz' ? 'g' : 'a') }) +
        disco +
        `<circle cx="131" cy="143" r="10.6" fill="none" stroke="${c}" stroke-width="1.8"/>` +
        luz(N, 2.2, c) +
        '</g>'
      );
    }
    const c = estado === 'tenue' ? CIAN : '#6B7686';
    const op = estado === 'tenue' ? ' opacity=".38"' : '';
    return (
      `<g class="pecho">${disco}<g fill="none" stroke="${c}" stroke-linecap="round" stroke-linejoin="round"${op}>` +
      `<circle cx="131" cy="143" r="10.6" stroke-width="1.8"/><path d="${N}" stroke-width="2.4"/></g></g>`
    );
  };

  // ---------- brazos ----------
  // Lado derecho de la pantalla en coordenadas absolutas; el izquierdo se refleja sobre el torso.
  const lado = (izq) => ({
    X: (x) => (izq ? 2 * CENTRO - x : x),
    Y: (y) => (izq ? y - 4 : y),
    A: (a) => (izq ? -a : a),
  });

  function pulgar(izq) {
    const { X, Y, A } = lado(izq);
    const cx = X(195), cy = Y(158), r = 9.5;
    const p1 = [X(195 + r * Math.cos(-0.96)), Y(158 + r * Math.sin(-0.96))];
    const ctl = [X(195 + 16.6), cy];
    const p2 = [X(195 + r * Math.cos(0.96)), Y(158 + r * Math.sin(0.96))];
    return (
      oscuras(elip(X(158), Y(137), 5.5, 14, A(-12))) +
      blancas(cap(X(160), Y(141), cx, cy, 12)) +
      luz(`M${f(p1[0])} ${f(p1[1])}Q${f(ctl[0])} ${f(ctl[1])} ${f(p2[0])} ${f(p2[1])}`, 2.2) +
      oscuras(elip(X(189), Y(156), 6, 15.5, A(-8)) + cap(X(179), Y(141), X(179), Y(135), 6.8)) +
      blancas(cap(X(179), Y(136), X(179), Y(127), 6.2) + rect(izq ? X(191) : 162, Y(138), 29, 33, 12, A(-6))) +
      `<path d="M${f(X(165))} ${f(Y(148))}H${f(X(175))}M${f(X(165))} ${f(Y(155.5))}H${f(X(176))}M${f(X(165))} ${f(Y(163))}H${f(X(175))}" fill="none" stroke="#C3CBD8" stroke-width="1.3" stroke-linecap="round"/>`
    );
  }

  function relajado(izq) {
    const { X, Y, A } = lado(izq);
    return (
      oscuras(elip(X(157), Y(137), 5.5, 13, A(-25))) +
      blancas(cap(X(158), Y(140), X(167), Y(160), 10.5)) +
      oscuras(elip(X(168), Y(163), 9.5, 4, A(-24))) +
      blancas(elip(X(169.5), Y(171.5), 8, 9.5, A(-22)))
    );
  }

  function encogido(izq) {
    const { X, Y, A } = lado(izq);
    return (
      oscuras(elip(X(157), Y(137), 5.5, 13, A(-25))) +
      blancas(cap(X(159), Y(141), X(180), Y(153), 10) + cap(X(180), Y(153), X(190), Y(136), 9)) +
      oscuras(elip(X(180), Y(153), 5, 9.5, A(-30)) + elip(X(190.5), Y(134), 7.5, 3.2, A(32))) +
      blancas(
        cap(X(189), Y(121), X(186.5), Y(112), 3.1) + cap(X(194), Y(120), X(194.5), Y(110), 3.1) + cap(X(199), Y(121), X(201.5), Y(112.5), 3.1) +
          cap(X(187), Y(128), X(181.5), Y(123), 3.2) + rect(izq ? X(201) : 186, Y(118), 15, 14, 6, A(12)),
      )
    );
  }

  const saludar =
    blancas(cap(50, 121, 79, 130, 13)) +
    luz('M41.8 121.5Q43 130 52.5 131.5', 2.4) +
    oscuras(elip(81, 130, 6, 15, 15) + cap(29, 96, 20, 84, 5.3) + cap(33, 92, 30, 74, 5.6) + cap(46, 90, 46, 72, 5.6) + cap(55, 99, 61, 86, 5.6)) +
    blancas(cap(26.5, 92.5, 20, 84, 5.3) + cap(31.7, 84.5, 30, 74, 5.6) + cap(46, 83, 46, 72, 5.6) + cap(57.8, 93.5, 61, 86, 5.6) + rect(27, 87, 33, 24, 11, -8)) +
    oscuras(elip(47.5, 110.5, 12.5, 4.6, -6));

  // ---------- cabeza ----------
  const DIADEMA = 'M79 46C80 3 197 1 200 48';
  // Tramo central de la diadema (t de .3 a .72) para la luz superior.
  const tramo = (() => {
    const P = [[79, 46], [80, 3], [197, 1], [200, 48]];
    const at = (t) => {
      const u = 1 - t;
      return [0, 1].map((k) => u * u * u * P[0][k] + 3 * u * u * t * P[1][k] + 3 * u * t * t * P[2][k] + t * t * t * P[3][k]);
    };
    const d = (t) => {
      const u = 1 - t;
      return [0, 1].map((k) => 3 * u * u * (P[1][k] - P[0][k]) + 6 * u * t * (P[2][k] - P[1][k]) + 3 * t * t * (P[3][k] - P[2][k]));
    };
    const [a, b] = [0.3, 0.72];
    const h = (b - a) / 3;
    const A = at(a), Bp = at(b), dA = d(a), dB = d(b);
    const c1 = [A[0] + dA[0] * h, A[1] + dA[1] * h];
    const c2 = [Bp[0] - dB[0] * h, Bp[1] - dB[1] * h];
    return `M${f(A[0])} ${f(A[1])}C${f(c1[0])} ${f(c1[1])} ${f(c2[0])} ${f(c2[1])} ${f(Bp[0])} ${f(Bp[1])}`;
  })();

  const cabezaAtras =
    `<path d="${DIADEMA}" fill="none" stroke="${B}" stroke-width="19"/>` +
    `<path d="${DIADEMA}" fill="none" stroke="${O}" stroke-width="12.5"/>` +
    luz(tramo, 3.6) +
    blancas(elip(67, 64, 11, 23) + rect(72, 34, 13, 15, 4, -24)) +
    luz('M64.5 44A8 20 0 0 0 64.5 84', 2.8) +
    oscuras(elip(77.5, 64, 5.5, 22.5)) +
    blancas(rect(75, 28, 112, 90, 38)) +
    `<rect x="83" y="40" width="96" height="68" rx="26" fill="${url('s')}" stroke="#1E242D" stroke-width="2"/>` +
    '<path d="M98 50.5Q131 41.5 164 50.5" fill="none" stroke="#fff" opacity=".09" stroke-width="5" stroke-linecap="round"/>';

  const cabezaFrente =
    blancas(rect(193, 37, 13, 16, 4, 24)) +
    oscuras(elip(191, 76, 8, 28)) +
    blancas(elip(204, 76, 14, 30) + elip(209, 76, 9, 26)) +
    `<g fill="none" stroke="${CIAN}"><ellipse cx="210" cy="76" rx="6.6" ry="21.5" stroke-width="7" opacity=".2"/><ellipse cx="210" cy="76" rx="6.6" ry="21.5" stroke-width="3.2"/></g>` +
    blancas(elip(211, 76, 4, 17.5));

  // ---------- caras (coordenadas de la cabeza recta) ----------
  const OI = 108, OD = 155, OY = 71, BX = 131.5; // ojo izquierdo, derecho, altura, centro de la boca
  const arcoFeliz = (x) => `M${f(x - 11)} ${OY + 4}A11 11 0 0 1 ${f(x + 11)} ${OY + 4}`;
  const rubor = (x, y) => `M${f(x)} ${f(y + 3)}l2.4-3.4M${f(x + 3.6)} ${f(y + 4)}l2.4-3.4`;
  const sonrisa = luzLlena(`M${f(BX - 8)} 82H${f(BX + 8)}A8 9 0 0 1 ${f(BX - 8)} 82Z`);
  const ojosFelices =
    luz(`${arcoFeliz(OI)}${arcoFeliz(OD)}`, 5) +
    `<path d="${rubor(88, 78)}${rubor(166, 78)}" fill="none" stroke="${CIAN}" stroke-opacity=".75" stroke-width="1.8" stroke-linecap="round"/>`;

  const bocaLinea = (d) => luz(d, 2.6);
  const ojoTriste = (x, izq) => {
    const r = 7.6, cy = OY + 2;
    const s = izq ? 1 : -1;
    const ang = (g) => [x + s * r * Math.cos((g * Math.PI) / 180), cy + r * Math.sin((g * Math.PI) / 180)];
    const [ix, iy] = ang(-42); // lado interior, más alto
    const [ex, ey] = ang(200); // lado exterior, más bajo
    return `M${f(ex)} ${f(ey)}L${f(ix)} ${f(iy)}A${r} ${r} 0 1 ${izq ? 1 : 0} ${f(ex)} ${f(ey)}Z`;
  };

  const caras = {
    feliz: ojosFelices + sonrisa,
    buscando:
      `<g class="buscar">` + luzLlena(circ(OI + 5, OY + 1, 6) + circ(OD + 5, OY + 1, 6)) + '</g>' + bocaLinea(`M${f(BX - 4)} 88Q${f(BX + 0.5)} 90 ${f(BX + 5)} 88`),
    desconectado: luzLlena(ojoTriste(OI, true) + ojoTriste(OD, false)) + bocaLinea(`M${f(BX - 5)} 90Q${f(BX)} 88 ${f(BX + 5)} 90`),
    error:
      luzLlena(circ(OI, OY + 2, 6) + circ(OD, OY + 2, 6)) +
      luz(`M${OI - 8} ${OY - 6}L${OI + 6} ${OY - 10.5}M${OD + 8} ${OY - 6}L${OD - 6} ${OY - 10.5}`, 2.6) +
      luz(circ(BX, 89, 3.6), 2.4),
    'bluetooth-apagado': luz(`M${OI - 8} ${OY + 3}H${OI + 8}M${OD - 8} ${OY + 3}H${OD + 8}`, 3.6) + bocaLinea(`M${f(BX - 3.5)} 88Q${f(BX)} 89.6 ${f(BX + 3.5)} 88`),
    cargando: [-15, 0, 15]
      .map((dx, i) => `<g class="punto"${i ? ` opacity="${['', '.6', '.3'][i]}"` : ''}>${luzLlena(circ(BX + dx, OY + 3, 4.6))}</g>`)
      .join(''),
    'sin-dispositivos': luzLlena(circ(OI, OY + 1, 8) + circ(OD, OY + 3, 4.8)) + bocaLinea(`M${f(BX - 5)} 89.5Q${f(BX)} 90.5 ${f(BX + 5)} 87.5`),
  };
  caras.saludo = caras.feliz;

  // ---------- extras alrededor de la cabeza (coordenadas del cuerpo) ----------
  const onda = (r, op) => {
    const cx = 207, cy = 45;
    const p = (g) => [cx + r * Math.cos((g * Math.PI) / 180), cy + r * Math.sin((g * Math.PI) / 180)];
    const [a, b] = [p(-78), p(-12)];
    return `<path d="M${f(a[0])} ${f(a[1])}A${r} ${r} 0 0 1 ${f(b[0])} ${f(b[1])}" opacity="${op}"/>`;
  };
  const extras = {
    buscando: `<g fill="none" stroke="${CIAN}" stroke-width="2.6" stroke-linecap="round">${onda(9, 0.9)}${onda(16, 0.55)}${onda(23, 0.3)}</g>`,
    'bluetooth-apagado':
      '<g fill="none" stroke="#8C98AA" stroke-linecap="round" stroke-linejoin="round"><path d="M209 30h6l-6 7h6" stroke-width="2"/><path d="M219 11h9l-9 10.5h9" stroke-width="2.6"/></g>',
    'sin-dispositivos':
      `<g fill="none" stroke="${CIAN}" stroke-linecap="round" stroke-width="3"><path d="M206 25.5q0-7.5 7.5-7.5t7.5 7q0 4.5-7 7v3.5"/></g><circle cx="214" cy="42.5" r="2" fill="${CIAN}"/>`,
  };

  const brazos = {
    saludo: saludar + pulgar(false),
    feliz: pulgar(true) + pulgar(false),
    'sin-dispositivos': encogido(true) + encogido(false),
    otro: relajado(true) + relajado(false),
  };
  const pechos = { desconectado: 'tenue', error: 'ambar', 'bluetooth-apagado': 'apagada' };

  return { defs, cuerpo, pecho, pechos, brazos, cabezaAtras, cabezaFrente, caras, extras };
}

// ---------- estilos de los bucles (solo en los archivos sueltos) ----------
const ESTILO = {
  buscando:
    '<style>.buscar{animation:m 2.4s ease-in-out infinite}@keyframes m{50%{transform:translateX(-9px)}}@media (prefers-reduced-motion:reduce){.buscar{animation:none}}</style>',
  cargando:
    '<style>.punto{animation:p 1.2s ease-in-out infinite;transform-box:fill-box;transform-origin:center}.punto+.punto{animation-delay:.2s}.punto+.punto+.punto{animation-delay:.4s}@keyframes p{0%,100%{opacity:.3;transform:scale(.8)}40%{opacity:1;transform:none}}@media (prefers-reduced-motion:reduce){.punto{animation:none}}</style>',
};

const MARCO = 'matrix(.92 0 0 .92 13 12)'; // deja margen alrededor del personaje
const GIRO = 'rotate(8 131 72)';

function svgSuelto(exp) {
  const g = construir('svg');
  const brazo = g.brazos[exp] ?? g.brazos.otro;
  const cuerpo =
    g.cuerpo + g.pecho(g.pechos[exp] ?? 'luz') + `<g class="brazos">${brazo}</g>` +
    `<g class="cabeza" transform="${GIRO}">${g.cabezaAtras}<g class="cara">${g.caras[exp]}</g>${g.cabezaFrente}</g>` +
    (g.extras[exp] ? `<g class="extra">${g.extras[exp]}</g>` : '');
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 240" width="240" height="240" role="img" aria-label="${EXPRESIONES[exp]}">` +
    `<title>${EXPRESIONES[exp]}</title>${ESTILO[exp] ?? ''}${g.defs(exp === 'error')}<g transform="${MARCO}">${cuerpo}</g></svg>\n`
  );
}

// Bloque de Svelte: cuerpo compartido y partes por expresión con {#if}.
function bloqueSvelte() {
  const g = construir('svelte');
  const si = (mapa, claves, otro) => {
    // Agrupa expresiones con el mismo marcado.
    const grupos = new Map();
    for (const k of claves) {
      const s = mapa(k);
      if (!grupos.has(s)) grupos.set(s, []);
      grupos.get(s).push(k);
    }
    const lista = [...grupos.entries()];
    if (lista.length === 1) return lista[0][0];
    let fuera = otro !== undefined ? lista.find(([s]) => s === otro) : lista[lista.length - 1];
    if (!fuera) fuera = lista[lista.length - 1];
    const resto = lista.filter((x) => x !== fuera);
    const cond = (ks) => ks.map((k) => `expresion === '${k}'`).join(' || ');
    let s = '';
    resto.forEach(([m, ks], i) => {
      s += `\n{${i ? ':else if' : '#if'} ${cond(ks)}}\n${m}`;
    });
    if (fuera[0]) s += `\n{:else}\n${fuera[0]}`;
    return `${s}\n{/if}\n`;
  };
  const claves = Object.keys(EXPRESIONES);
  return (
    `${g.defs(true)}\n<g transform="${MARCO}">\n${g.cuerpo}\n` +
    si((k) => g.pecho(g.pechos[k] ?? 'luz'), claves) +
    `<g class="brazos">` +
    si((k) => g.brazos[k] ?? g.brazos.otro, claves, g.brazos.otro) +
    `</g>\n<g class="cabeza" transform="${GIRO}">\n${g.cabezaAtras}\n<g class="cara">` +
    si((k) => g.caras[k], claves) +
    `</g>\n${g.cabezaFrente}\n</g>` +
    si((k) => (g.extras[k] ? `<g class="extra">${g.extras[k]}</g>` : ''), claves, '') +
    '</g>\n'
  );
}

// ---------- escribir ----------
const carpeta = join(aqui, 'expresiones');
mkdirSync(carpeta, { recursive: true });
for (const exp of Object.keys(EXPRESIONES)) {
  const s = svgSuelto(exp);
  writeFileSync(join(carpeta, `${exp}.svg`), s);
  console.log(`${exp}.svg ${(Buffer.byteLength(s) / 1024).toFixed(1)} KB`);
}

const componente = join(raiz, 'src', 'lib', 'componentes', 'NexoBot.svelte');
try {
  const fuente = readFileSync(componente, 'utf8');
  const ini = '<!-- generado:inicio -->';
  const fin = '<!-- generado:fin -->';
  const a = fuente.indexOf(ini);
  const b = fuente.indexOf(fin);
  if (a < 0 || b < 0) throw new Error('faltan las marcas');
  const nuevo = fuente.slice(0, a + ini.length) + '\n' + bloqueSvelte() + fuente.slice(b);
  writeFileSync(componente, nuevo);
  console.log('NexoBot.svelte actualizado');
} catch (e) {
  console.log(`NexoBot.svelte sin cambios (${e.message})`);
}
