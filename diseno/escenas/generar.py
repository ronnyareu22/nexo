# -*- coding: utf-8 -*-
"""Ilustraciones de las escenas de Nexo (Mi setup) y del estado de Remoto.

Calco del generador de SIAU (~/ronny/SIAU-UAE/.claude/scripts/ilustraciones_svg.py), que sale del de SIGE: plano y
amable, paleta fija, mancha translúcida detrás y sombra en elipse en el piso, personas armadas siempre con las mismas
piezas (pies en y=386, cabeza r=21 a 246 px de los pies) y tarjetitas flotantes con íconos de trazo.
Aquí, con la marca de Nexo: turquesa, azul y celeste; marino en contornos y pelo; ámbar y amarillo en la luz.
Sin texto dentro de los dibujos: las palabras van en la app.

Temas: cada color de la paleta que cambia en oscuro lleva una clase (nxe-f<clave> para el relleno, nxe-s<clave>
para el trazo) que lee una variable --ilus-<clave>, con el color claro de respaldo.
- Como archivo suelto (o en <img>), se ve con la paleta clara.
- Pegado en línea en Nexo (IlustracionEscena.svelte), las reglas y las variables de los dos temas salen de
  src/lib/estilos/ilustraciones.css, que también escribe este script. Así la paleta vive en un solo lugar.

    python3 diseno/escenas/generar.py

Escribe static/escenas/<nombre>.svg (una por cada ícono de escena, «nueva» y las tres de Remoto)
y src/lib/estilos/ilustraciones.css.
"""
import math
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent.parent
SALIDA = RAIZ / 'static' / 'escenas'
CSS = RAIZ / 'src' / 'lib' / 'estilos' / 'ilustraciones.css'

# ---------------------------------------------------------------- paleta (marca de Nexo)

TURQ = '#00E6B8'; TURQ2 = '#00B894'; TURQ3 = '#7EEBD6'
AZUL = '#3B82F6'; AZUL2 = '#7AA9F7'; CIELO = '#38BDF8'
MARINO = '#1B2A4A'; MARINO2 = '#111B30'; PELO = '#1C2B4B'; OJOS = '#16223D'
AMBAR = '#F59E0B'; AMARILLO = '#FBBF24'
VERDE = '#22C55E'; ROJO = '#EF4444'
# Superficies (pantallas, tarjetas): en oscuro pasan a un azul pizarra. El blanco de los trazos sobre color sigue '#fff'.
SUPERFICIE = '#FFFFFF'
PALIDO = '#CDEFE7'      # borde de las tarjetitas y barras
MANCHA = '#ECFAF6'      # bloques dentro de las pantallas
CONTORNO = '#B3E3D7'    # marcos de ventanas y objetos grandes
MESA = '#BCD3F5'        # tablero de la mesa
AZUL_P = '#DCE9FD'      # rellenos azul pálido
AMARILLO_P = '#FEF3C7'  # rellenos amarillo pálido
GRIS = '#C6CFDB'; GRIS2 = '#97A6BA'  # la versión apagada (Remoto sin sesión)
NOCHE = '#22335C'       # el cielo de la ventana de noche y la pantalla tenue (igual en los dos temas)
# La mancha y la sombra son translúcidas: turquesa y azul en claro, un tinte suave en oscuro.
MANCHA_T = 'rgba(0,184,148,0.10)'; SOMBRA_T = 'rgba(59,130,246,0.14)'
LUZ = 'rgba(251,191,36,0.22)'; LUZ_TIBIA = 'rgba(245,158,11,0.16)'; HALO = 'rgba(251,191,36,0.30)'

PIELES = {'clara': ('#F3C7A0', '#E2AE86'), 'media': ('#E0A47A', '#C98B62'), 'morena': ('#C98B62', '#A9714D')}

# Color fijo (el de claro) -> (clave, color en oscuro). Los demás colores no cambian con el tema.
TEMA = {
    SUPERFICIE: ('w', '#26334A'),
    MARINO: ('n', '#5A6C8E'),
    MARINO2: ('z', '#3B4A66'),
    PELO: ('e', '#3A4A6E'),
    PALIDO: ('p', '#36495F'),
    CONTORNO: ('c', '#40586C'),
    MANCHA: ('k', '#2D3C53'),
    MESA: ('d', '#3D5478'),
    AZUL_P: ('a', '#2F4262'),
    AMARILLO_P: ('y', '#4D432A'),
    GRIS: ('g', '#3F4B5D'),
    GRIS2: ('h', '#5B697E'),
    MANCHA_T: ('m', 'rgba(0,230,184,0.07)'),
    SOMBRA_T: ('o', 'rgba(0,0,0,0.30)'),
}


def n(v):
    """Número corto: sin ceros de más."""
    return f'{v:.2f}'.rstrip('0').rstrip('.') if isinstance(v, float) else str(v)


# ---------------------------------------------------------------- piezas básicas

def trazo(d, color=AZUL, ancho=2.4, relleno='none'):
    return (f'<path d="{d}" fill="{relleno}" stroke="{color}" stroke-width="{n(ancho)}" '
            'stroke-linecap="round" stroke-linejoin="round"/>')


def barra(x, y, w, h=9, color=PALIDO):
    return f'<rect x="{n(x)}" y="{n(y)}" width="{n(w)}" height="{n(h)}" rx="{n(h / 2)}" fill="{color}"/>'


def punteada(d, color=CONTORNO, ancho=2):
    return (f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{ancho}" stroke-linecap="round" '
            'stroke-dasharray="5 8"/>')


def destello(x, y, color=AMARILLO, tam=12):
    m = tam // 2
    return trazo(f'M{x - m} {y}h{tam}M{x} {y - m}v{tam}', color, 3)


def rayos(cx, cy, r1, r2, cuantos=8, giro=0.0):
    d = ''
    for i in range(cuantos):
        a = math.radians(giro + i * 360 / cuantos)
        d += f'M{n(cx + r1 * math.cos(a))} {n(cy + r1 * math.sin(a))}L{n(cx + r2 * math.cos(a))} {n(cy + r2 * math.sin(a))}'
    return d


def insignia(x, y, tipo='visto', r=20):
    """Círculo de estado con borde de superficie: visto verde o más turquesa."""
    color = {'visto': VERDE, 'mas': TURQ2}[tipo]
    out = f'<circle cx="{x}" cy="{y}" r="{r}" fill="{color}" stroke="{SUPERFICIE}" stroke-width="4"/>'
    k = r / 20
    if tipo == 'visto':
        out += trazo(f'M{n(x - 9 * k)} {n(y)}l{n(6 * k)} {n(6 * k)} {n(12 * k)}-{n(12 * k)}', '#fff', 4)
    else:
        out += trazo(f'M{x} {n(y - 9 * k)}v{n(18 * k)}M{n(x - 9 * k)} {y}h{n(18 * k)}', '#fff', 4)
    return out


# ---------------------------------------------------------------- tarjetitas flotantes

def tarjeta(x, y, icono, ancho=46):
    escala = '' if ancho == 46 else f' scale({n(ancho / 46)})'
    return (f'<g transform="translate({x} {y}){escala}"><rect width="46" height="46" rx="12" fill="{SUPERFICIE}" '
            f'stroke="{PALIDO}" stroke-width="2"/>{icono}</g>')


COMANDO = ('M18 18v-3.5a3.5 3.5 0 1 0-3.5 3.5H31.5a3.5 3.5 0 1 0-3.5-3.5V31.5a3.5 3.5 0 1 0 3.5-3.5'
           'H14.5a3.5 3.5 0 1 0 3.5 3.5z')

ICONOS = {
    'audifonos': (trazo('M13 28v-5a10 10 0 0 1 20 0v5', TURQ2, 2.6)
                  + f'<rect x="9.5" y="25" width="7.5" height="11" rx="3.7" fill="{TURQ2}"/>'
                  + f'<rect x="29" y="25" width="7.5" height="11" rx="3.7" fill="{TURQ2}"/>'),
    # El sol con el brillo al 70 %: la barra de abajo va llena hasta el 70 %.
    'sol70': (f'<circle cx="23" cy="17" r="5" fill="{AMARILLO}"/>' + trazo(rayos(23, 17, 8, 10.5), AMBAR, 2)
              + barra(10, 31, 26, 5, PALIDO) + barra(10, 31, 18.2, 5, AMARILLO)),
    'sol': f'<circle cx="23" cy="23" r="6" fill="{AMARILLO}"/>' + trazo(rayos(23, 23, 9.5, 13), AMBAR, 2.2),
    'teclado': (f'<rect x="8" y="15" width="30" height="17" rx="4" fill="none" stroke="{AZUL}" stroke-width="2.4"/>'
                + trazo('M13.5 20.5h1.5M18.5 20.5h1.5M23.5 20.5h1.5M28.5 20.5h1.5M13.5 26.5h1.5M30 26.5h1.5M19 26.5h7.5',
                        AZUL, 2.3)),
    'comando': trazo(COMANDO, AZUL, 2.4),
    'luna': trazo('M30.5 27A10.5 10.5 0 1 1 20 12.5a8.5 8.5 0 0 0 10.5 14.5z', AMBAR, 2.2, AMARILLO_P),
    'microfono': (f'<rect x="18.5" y="9" width="9" height="16" rx="4.5" fill="{AZUL_P}" stroke="{AZUL}" stroke-width="2.3"/>'
                  + trazo('M14 22a9 9 0 0 0 18 0M23 31v5M18.5 36h9', AZUL, 2.3)),
    'camara': (trazo('M10 16h15a3 3 0 0 1 3 3v9a3 3 0 0 1-3 3H10a3 3 0 0 1-3-3v-9a3 3 0 0 1 3-3z', AZUL, 2.3, AZUL_P)
               + trazo('M28 21.5l9-4.5v13l-9-4.5z', AZUL, 2.3)),
    'bombilla': (trazo('M18.5 28c-3-2-5.5-5.2-5.5-9a10 10 0 0 1 20 0c0 3.8-2.5 7-5.5 9v3h-9z', AMBAR, 2.2, AMARILLO_P)
                 + trazo('M19.5 35h7', AMBAR, 2.4)),
    'reloj': f'<circle cx="23" cy="23" r="12" fill="none" stroke="{AZUL}" stroke-width="2.4"/>' + trazo('M23 16.5V23l4.5 3', AZUL),
    'campana': (trazo('M15 29v-7a8 8 0 0 1 16 0v7l2 3H13z', AMBAR, 2.2, AMARILLO_P) + trazo('M20 35a3 3 0 0 0 6 0', AMBAR, 2.2)),
    'casa': trazo('M10.5 23.5L23 13l12.5 10.5M14 21v14h18V21', TURQ2, 2.4) + trazo('M20 35v-7h6v7', TURQ2, 2.2),
    'rayo': trazo('M25.5 9L14 25.5h8.5L20 37l12-17h-8.5z', AMBAR, 2.2, AMARILLO),
    'visto': trazo('M14 23l6 6 12-12', VERDE, 2.8),
    'escudo': (trazo('M23 10l11 4v8c0 7-5 11.5-11 14-6-2.5-11-7-11-14v-8z', TURQ2, 2.3, PALIDO)
               + trazo('M18 23l4 4 6.5-7', TURQ2, 2.4)),
    'pantalla': (f'<rect x="9" y="11" width="28" height="19" rx="3.5" fill="none" stroke="{AZUL}" stroke-width="2.4"/>'
                 + trazo('M23 30v5M17 35.5h12', AZUL, 2.4)),
    'bluetooth': trazo('M16.5 17.5l12 10.5-5.5 5V13l5.5 5-12 10.5', AZUL, 2.4),
    'wifi': (trazo('M12 20a16 16 0 0 1 22 0M15.5 24.5a11 11 0 0 1 15 0M19 29a5.5 5.5 0 0 1 8 0', TURQ2, 2.5)
             + f'<circle cx="23" cy="33" r="2.2" fill="{TURQ2}"/>'),
    'volumen': (trazo('M11 19.5h5l6-5v17l-6-5h-5z', CIELO, 2.3, AZUL_P) + trazo('M26.5 18.5a6.5 6.5 0 0 1 0 9M30 15a11.5 11.5 0 0 1 0 16', CIELO, 2.3)),
    'onda': trazo('M11 23v0M16 18v10M21 13v20M26 17v12M31 20v6M36 23v0', CIELO, 2.8),
    'bateria': (f'<rect x="9" y="16" width="25" height="14" rx="4" fill="none" stroke="{VERDE}" stroke-width="2.3"/>'
                + f'<rect x="12" y="19" width="15" height="8" rx="2" fill="{VERDE}"/>' + trazo('M37 21v4', VERDE, 2.6)),
    'calendario': (f'<rect x="12" y="14" width="22" height="20" rx="4" fill="none" stroke="{AZUL}" stroke-width="2.4"/>'
                   + trazo('M12 20h22M18 10v6M28 10v6')),
    'nota': trazo('M20 31V14l12-3v16', AZUL, 2.4) + f'<circle cx="17" cy="31" r="3.5" fill="{AZUL}"/><circle cx="29" cy="27" r="3.5" fill="{AZUL}"/>',
    'mas': trazo('M23 14v18M14 23h18', TURQ2, 3),
}


# ---------------------------------------------------------------- personas (las mismas piezas de SIGE y SIAU)

def piernas(color=MARINO, alto=100, separacion=11):
    return (f'<rect x="{-separacion - 13}" y="{-alto - 3}" width="16" height="{alto}" rx="8" fill="{color}"/>'
            f'<rect x="{separacion - 3}" y="{-alto - 3}" width="16" height="{alto}" rx="8" fill="{color}"/>'
            f'<path d="M-25 -7h24a4 4 0 0 1 4 4v3h-31a6 6 0 0 1 3-7z" fill="{MARINO2}"/>'
            f'<path d="M1 -7h24a6 6 0 0 1 3 7h-31v-3a4 4 0 0 1 4-4z" fill="{MARINO2}"/>')


def brazo(desde, curva, color, ancho=15):
    return (f'<path d="M{desde[0]} {desde[1]}{curva}" fill="none" stroke="{color}" stroke-width="{ancho}" '
            'stroke-linecap="round" stroke-linejoin="round"/>')


def mano(x, y, piel):
    return f'<circle cx="{x}" cy="{y}" r="8" fill="{piel}"/>'


# Poses de brazos: (curva relativa desde el hombro, posición de la mano relativa al hombro)
POSES_IZQ = {
    'abajo': ('c-6 34-8 62-6 92', (-6, 96)),
    'senalar': ('c-20 10-30 22-38 32', (-42, 36)),
    'saludar': ('c-14-10-20-30-18-54', (-18, -58)),
}
POSES_DER = {
    'abajo': ('c6 34 8 62 6 92', (6, 96)),
    'saludar': ('c14-10 20-30 18-54', (18, -58)),
    'telefono': ('c12 26 6 46-12 52', (-14, 54)),
    'taza': ('c12 26 6 46-12 52', (-14, 54)),
    'pensar': ('l12 44-26-58', (-15, -18)),
}


def sombrear(color):
    tabla = {AZUL: '#2F6FDB', TURQ2: '#009E80', CIELO: '#1EA3DE', AMBAR: '#DB8A06'}
    return tabla.get(color, color)


def cara(tipo, piel, sombra, pelo=PELO):
    out = f'<circle cx="0" cy="-246" r="21" fill="{piel}"/>'
    if tipo == 'mujer':
        out += (f'<path d="M-21 -250c0-18 16-26 28-22 10 3 16 12 16 22-10-8-24-10-36-2-3 2-6 4-8 2z" fill="{pelo}"/>'
                f'<circle cx="13" cy="-276" r="9" fill="{pelo}"/>')
    elif tipo == 'mujer_larga':
        out += f'<path d="M-22 -250c2-17 16-24 29-20 9 3 15 10 15 19-10-8-22-9-32-4-5 2-9 4-12 5z" fill="{pelo}"/>'
    else:
        out += (f'<circle cx="-21" cy="-243" r="4.5" fill="{sombra}"/>'
                f'<path d="M-21 -252c0-16 14-24 28-21 9 2 15 9 15 18-8-6-20-8-31-4-5 2-9 5-12 7z" fill="{pelo}"/>')
        if tipo == 'hombre_barba':
            out += f'<path d="M-17 -238c3 9 9 13 17 13s14-4 17-13c-5 3-11 4-17 4s-12-1-17-4z" fill="{pelo}"/>'
    out += f'<circle cx="-6" cy="-246" r="1.9" fill="{OJOS}"/><circle cx="8" cy="-246" r="1.9" fill="{OJOS}"/>'
    if tipo != 'hombre_barba':
        out += '<path d="M-3 -237q5 4 9 0" fill="none" stroke="#B9785A" stroke-width="1.8" stroke-linecap="round"/>'
    return out


def audifonos(color=TURQ2):
    """Audífonos grandes sobre la cabeza (como los de Nexo Bot): diadema marino y copas de color."""
    return (f'<path d="M-24 -244a24 27 0 0 1 48 0" fill="none" stroke="{MARINO}" stroke-width="5" stroke-linecap="round"/>'
            f'<rect x="-31" y="-259" width="12" height="24" rx="6" fill="{color}"/>'
            f'<rect x="19" y="-259" width="12" height="24" rx="6" fill="{color}"/>'
            f'<rect x="-28" y="-253" width="4" height="12" rx="2" fill="#fff" opacity=".45"/>')


def extremidades(izq, der, color, piel, hombro=27):
    out = ''
    ci, mi = POSES_IZQ[izq]
    out += brazo((-hombro, -191), ci, color) + mano(-hombro + mi[0], -191 + mi[1], piel)
    cd, md = POSES_DER[der]
    out += brazo((hombro, -191), cd, color) + mano(hombro + md[0], -191 + md[1], piel)
    hx, hy = hombro + md[0], -191 + md[1]
    if der == 'telefono':
        out += (f'<rect x="{hx - 9}" y="{hy - 30}" width="18" height="30" rx="4" fill="{MARINO}"/>'
                f'<rect x="{hx - 6.5}" y="{hy - 27}" width="13" height="22" rx="2" fill="{TURQ3}"/>' + mano(hx, hy, piel))
    if der == 'taza':
        # Una taza tibia en la mano, con su vapor.
        out += (f'<path d="M{hx - 14} {hy - 26}h22v18a6 6 0 0 1-6 6h-10a6 6 0 0 1-6-6z" fill="{SUPERFICIE}" stroke="{CONTORNO}" stroke-width="2"/>'
                + trazo(f'M{hx + 8} {hy - 21}h3a4 4 0 0 1 0 8h-3', CONTORNO, 2.4)
                + trazo(f'M{hx - 8} {hy - 32}c-3-4 3-6 0-10M{hx} {hy - 32}c-3-4 3-6 0-10', CONTORNO, 2.2)
                + mano(hx, hy, piel))
    return out


def rayas_torso(ancho, alto=118, arriba=-215, r=18):
    """Rayas de la camiseta sin recorte: cada una mide lo que deja la esquina redondeada."""
    d = ''
    for y in range(-195, -100, 18):
        dy = max(0.0, (arriba + r) - (y - 2.5), (y + 2.5) - (arriba + alto - r))
        dentro = r - math.sqrt(max(0.0, r * r - dy * dy)) if dy > 0 else 0
        x0 = -ancho / 2 + dentro + 1
        d += f'M{n(x0)} {y}H{n(-x0)}'
    return f'<path d="{d}" stroke="#fff" stroke-opacity=".3" stroke-width="5"/>'


def persona(x, tipo='hombre', piel='media', ropa=AZUL, rayas=True, izq='abajo', der='abajo', pelo=PELO,
            con_audifonos=False, color_audifonos=TURQ2, piso=386):
    p, s = PIELES[piel]
    ancho = 52 if tipo.startswith('mujer') else 58
    g = f'<g transform="translate({x} {piso})">'
    if tipo == 'mujer_larga':
        g += f'<path d="M-24 -246c-2-24 12-36 26-34 15 2 24 14 22 34v34c-4 6-11 4-13-3h-24c-3 7-9 9-12 3z" fill="{pelo}"/>'
    g += f'<rect x="-6" y="-233" width="12" height="24" fill="{s}"/>'
    g += piernas()
    g += f'<rect x="{-ancho // 2}" y="-215" width="{ancho}" height="118" rx="18" fill="{ropa}"/>'
    if rayas:
        g += rayas_torso(ancho)
    g += extremidades(izq, der, sombrear(ropa), p, hombro=ancho // 2 - 4)
    g += cara(tipo, p, s, pelo)
    if con_audifonos:
        g += audifonos(color_audifonos)
    return g + '</g>'


# ---------------------------------------------------------------- muebles y objetos

def escritorio(x0, x1, y=302):
    """Mesa de trabajo: tablero claro y dos patas que llegan al piso (y=386)."""
    return (f'<rect x="{x0}" y="{y}" width="{x1 - x0}" height="12" rx="6" fill="{MESA}"/>'
            f'<rect x="{x0 + 18}" y="{y + 12}" width="9" height="{386 - y - 12}" rx="4" fill="{MARINO}"/>'
            f'<rect x="{x1 - 27}" y="{y + 12}" width="9" height="{386 - y - 12}" rx="4" fill="{MARINO}"/>')


def monitor(x, y, w, h, pantalla, mesa=302):
    """PC de escritorio sobre la mesa: marco oscuro, pantalla, pie y base. pantalla(x, y, w, h)."""
    cx = x + w / 2
    return (f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="14" fill="{MARINO}"/>'
            f'<rect x="{x + 9}" y="{y + 9}" width="{w - 18}" height="{h - 26}" rx="7" fill="{SUPERFICIE}"/>'
            f'<circle cx="{n(cx)}" cy="{y + h - 8}" r="2.5" fill="{MESA}"/>'
            f'<rect x="{n(cx - 9)}" y="{y + h}" width="18" height="{mesa - y - h - 5}" fill="{MARINO}"/>'
            f'<rect x="{n(cx - 40)}" y="{mesa - 6}" width="80" height="7" rx="3.5" fill="{MARINO}"/>'
            + pantalla(x + 9, y + 9, w - 18, h - 26))


def laptop(x, y, w, h, pantalla):
    """Laptop abierta sobre la mesa: la base mide 10, así que y+h=292 la apoya en y=302."""
    return (f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="12" fill="{MARINO}"/>'
            f'<rect x="{x + 8}" y="{y + 8}" width="{w - 16}" height="{h - 14}" rx="6" fill="{SUPERFICIE}"/>'
            f'<path d="M{x - 16} {y + h}h{w + 32}l-8 10H{x - 8}z" fill="{CONTORNO}"/>'
            f'<rect x="{n(x + w / 2 - 18)}" y="{y + h + 2}" width="36" height="4" rx="2" fill="{MESA}"/>'
            + pantalla(x + 8, y + 8, w - 16, h - 14))


def lampara(x, lado=-1, luz='encendida', mesa=302):
    """Lámpara de escritorio: pie en la mesa, brazo y pantalla que mira hacia `lado` con su cono de luz.
    luz: 'encendida' (amarilla), 'tibia' (ámbar y baja) o 'apagada'."""
    jx, jy = x + 40 * lado, 196
    out = ''
    if luz != 'apagada':
        ancho, color = (64, LUZ) if luz == 'encendida' else (44, LUZ_TIBIA)
        out += f'<path d="M{jx - 17} {jy + 14}L{jx - ancho} {mesa}H{jx + ancho}L{jx + 17} {jy + 14}z" fill="{color}"/>'
        out += f'<circle cx="{jx}" cy="{jy + 15}" r="{14 if luz == "encendida" else 10}" fill="{HALO}"/>'
    out += (f'<ellipse cx="{x}" cy="{mesa - 3}" rx="18" ry="5" fill="{MARINO}"/>'
            + trazo(f'M{x} {mesa - 4}L{x + 10 * -lado} 240L{jx} {jy}', MARINO, 5)
            + f'<circle cx="{x + 10 * -lado}" cy="240" r="5" fill="{MARINO}"/>'
            + f'<path d="M{jx - 22} {jy + 14}a22 19 0 0 1 44 0z" fill="{TURQ2}"/>'
            + f'<circle cx="{jx}" cy="{jy + 14}" r="{6 if luz != "apagada" else 5}" fill="{AMARILLO if luz == "encendida" else AMBAR if luz == "tibia" else MESA}"/>')
    return out


def ventana(x, y, w, h, noche=False):
    """Ventana en la pared: de día cielo pálido con sol; de noche cielo marino con luna y estrellas."""
    out = f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="12" fill="{NOCHE if noche else AZUL_P}" stroke="{CONTORNO}" stroke-width="5"/>'
    if noche:
        lx, ly = x + w * 0.66, y + h * 0.36
        out += f'<path d="M{n(lx + 10)} {n(ly + 8)}A14 14 0 1 1 {n(lx - 4)} {n(ly - 13)}a11 11 0 0 0 {14} {21}z" fill="{AMARILLO}"/>'
        for sx, sy in ((0.2, 0.28), (0.34, 0.62), (0.82, 0.74)):
            out += f'<circle cx="{n(x + w * sx)}" cy="{n(y + h * sy)}" r="2.2" fill="#fff" opacity=".85"/>'
    else:
        sx, sy = x + w * 0.68, y + h * 0.38
        out += f'<circle cx="{n(sx)}" cy="{n(sy)}" r="13" fill="{AMARILLO}"/>' + trazo(rayos(sx, sy, 18, 23), AMARILLO, 3)
    out += trazo(f'M{n(x + w / 2)} {y + 3}V{y + h - 3}M{x + 3} {n(y + h / 2)}H{x + w - 3}', CONTORNO, 4)
    return out


def planta(x, base=302, alto=1.0):
    """Maceta con hojas turquesa, apoyada en `base`."""
    k = alto
    return (f'<path d="M{x} {base - 30 * k}c-26-6-34-30-30-50 18 4 30 22 30 50z" fill="{TURQ2}"/>'
            f'<path d="M{x} {base - 30 * k}c22-10 26-36 18-52-14 8-22 26-18 52z" fill="{TURQ}"/>'
            f'<path d="M{x} {base - 28 * k}c-4-24 2-44 10-56 6 18 2 38-10 56z" fill="#00A383"/>'
            f'<path d="M{x - 16} {base - 30}h32l-4 30h-24z" fill="{AMBAR}"/>'
            f'<rect x="{x - 18}" y="{base - 34}" width="36" height="8" rx="4" fill="#DB8A06"/>')


def taza(x, base=302):
    return (f'<path d="M{x - 12} {base - 26}h22v20a6 6 0 0 1-6 6h-10a6 6 0 0 1-6-6z" fill="{SUPERFICIE}" stroke="{CONTORNO}" stroke-width="2.5"/>'
            + trazo(f'M{x + 10} {base - 20}h3a4.5 4.5 0 0 1 0 9h-3', CONTORNO, 2.5)
            + trazo(f'M{x - 5} {base - 32}c-3-4 3-6 0-10M{x + 3} {base - 32}c-3-4 3-6 0-10', CONTORNO, 2.2))


def barra_superior(x, y, w, color=TURQ2):
    return (f'<path d="M{x} {y + 7}a7 7 0 0 1 7-7h{w - 14}a7 7 0 0 1 7 7v11H{x}z" fill="{color}"/>'
            + ''.join(f'<circle cx="{x + 12 + i * 9}" cy="{y + 9}" r="2.6" fill="#fff" opacity=".85"/>' for i in range(3)))


# ---------------------------------------------------------------- pantallas

def pantalla_estudio(x, y, w, h):
    """Notas de estudio a la izquierda y el anillo de concentración a la derecha."""
    out = barra_superior(x, y, w)
    for i, ancho in enumerate((0.5, 0.42, 0.48, 0.3)):
        out += barra(x + 12, y + 30 + i * 16, w * ancho, 7, AZUL_P if i else AZUL2)
    cx, cy, r = x + w - 34, y + h / 2 + 8, 19
    out += (f'<circle cx="{n(cx)}" cy="{n(cy)}" r="{r}" fill="none" stroke="{PALIDO}" stroke-width="7"/>'
            f'<path d="M{n(cx)} {n(cy - r)}a{r} {r} 0 1 1-{n(r * 0.95)} {n(r * 1.31)}" fill="none" stroke="{TURQ}" stroke-width="7" stroke-linecap="round"/>')
    return out


def cara_mini(cx, base, piel, pelo, ropa, mujer=False):
    """Una persona de la videollamada: hombros y cabeza, apoyados en `base`."""
    p = PIELES[piel][0]
    cy = base - 30
    out = (f'<path d="M{cx - 19} {base}a19 16 0 0 1 38 0z" fill="{ropa}"/>'
           f'<circle cx="{cx}" cy="{cy}" r="11" fill="{p}"/>'
           f'<path d="M{cx - 11} {cy - 1}a11 11 0 0 1 22 0c-6-5-15-6-22 0z" fill="{pelo}"/>')
    if mujer:
        out += f'<circle cx="{cx + 8}" cy="{cy - 11}" r="5" fill="{pelo}"/>'
    out += f'<circle cx="{cx - 3.5}" cy="{cy + 1}" r="1.3" fill="{OJOS}"/><circle cx="{cx + 4}" cy="{cy + 1}" r="1.3" fill="{OJOS}"/>'
    return out


def pantalla_llamada(x, y, w, h):
    """Videollamada: dos caras en sus cuadros y la barra de controles (micrófono, cámara y colgar)."""
    out = barra_superior(x, y, w, AZUL)
    tw = (w - 30) / 2
    th = h - 54
    for i, (piel, pelo, ropa, mujer) in enumerate((('clara', '#5A3B2E', TURQ2, True), ('morena', PELO, CIELO, False))):
        tx = x + 10 + i * (tw + 10)
        out += f'<rect x="{n(tx)}" y="{y + 26}" width="{n(tw)}" height="{th}" rx="8" fill="{AZUL_P if i else MANCHA}"/>'
        out += cara_mini(tx + tw / 2, y + 26 + th, piel, pelo, ropa, mujer)
    by = y + h - 13
    cx = x + w / 2
    out += (f'<circle cx="{n(cx - 20)}" cy="{by}" r="7" fill="{AZUL}"/><circle cx="{n(cx)}" cy="{by}" r="7" fill="{AZUL}"/>'
            f'<circle cx="{n(cx + 20)}" cy="{by}" r="7" fill="{ROJO}"/>'
            + trazo(f'M{n(cx - 20)} {by - 3}v4M{n(cx - 3)} {by - 2}h4v4h-4z', '#fff', 1.8)
            + trazo(f'M{n(cx + 16.5)} {by + 1}h7', '#fff', 2))
    return out


def pantalla_remota(x, y, w, h):
    """El escritorio del PC: dos ventanas y la barra de tareas."""
    out = f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="7" fill="{AZUL_P}"/>'
    out += (f'<rect x="{x + 12}" y="{y + 12}" width="{n(w * 0.55)}" height="{n(h * 0.52)}" rx="6" fill="{SUPERFICIE}"/>'
            + f'<path d="M{x + 12} {y + 18}a6 6 0 0 1 6-6h{n(w * 0.55 - 12)}a6 6 0 0 1 6 6v6H{x + 12}z" fill="{AZUL}"/>'
            + barra(x + 22, y + 34, w * 0.3, 6, PALIDO) + barra(x + 22, y + 46, w * 0.4, 6, PALIDO))
    out += (f'<rect x="{n(x + w * 0.42)}" y="{n(y + h * 0.34)}" width="{n(w * 0.5)}" height="{n(h * 0.42)}" rx="6" fill="{SUPERFICIE}" stroke="{PALIDO}" stroke-width="2"/>'
            + f'<path d="M{n(x + w * 0.42)} {n(y + h * 0.34 + 6)}a6 6 0 0 1 6-6h{n(w * 0.5 - 12)}a6 6 0 0 1 6 6v5H{n(x + w * 0.42)}z" fill="{TURQ2}"/>'
            + barra(x + w * 0.42 + 10, y + h * 0.34 + 20, w * 0.26, 6, MANCHA))
    out += (f'<rect x="{x}" y="{y + h - 14}" width="{w}" height="14" fill="{MARINO}" opacity=".18"/>'
            + ''.join(f'<rect x="{n(x + w / 2 - 24 + i * 17)}" y="{y + h - 11}" width="10" height="8" rx="2" fill="{AZUL if i == 1 else SUPERFICIE}"/>' for i in range(3)))
    return out


def pantalla_mac(x, y, w, h):
    """La Mac de la casa: barra de menús y la ventana de Nexo con sus mosaicos."""
    out = f'<rect x="{x}" y="{y}" width="{w}" height="9" rx="4" fill="{PALIDO}"/>'
    cw = (w - 30) / 2
    for i in range(2):
        for j in range(2):
            cx = x + 10 + i * (cw + 10)
            cy = y + 16 + j * ((h - 22) / 2)
            out += f'<rect x="{n(cx)}" y="{n(cy)}" width="{n(cw)}" height="{n((h - 22) / 2 - 6)}" rx="6" fill="{MANCHA}"/>'
            out += f'<circle cx="{n(cx + 10)}" cy="{n(cy + 10)}" r="4.5" fill="{(TURQ, CIELO, AMARILLO, TURQ2)[i * 2 + j]}"/>'
            out += barra(cx + 18, cy + 7, cw * 0.4, 6, PALIDO)
    return out


def pantalla_noche(x, y, w, h):
    """Pantalla tenue para la noche."""
    out = f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="7" fill="{NOCHE}"/>'
    out += barra(x + 14, y + 16, w * 0.4, 7, '#33497A') + barra(x + 14, y + 30, w * 0.55, 6, '#2D4170') + barra(x + 14, y + 42, w * 0.3, 6, '#2D4170')
    return out


def pantalla_brillo(x, y, w, h):
    """Ajuste de brillo: el sol grande y la barra al 70 %."""
    out = f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="7" fill="{AMARILLO_P}"/>'
    cx, cy = x + w / 2, y + h * 0.42
    out += f'<circle cx="{n(cx)}" cy="{n(cy)}" r="15" fill="{AMARILLO}"/>' + trazo(rayos(cx, cy, 21, 27), AMBAR, 3.2)
    out += barra(x + 16, y + h - 20, w - 32, 8, SUPERFICIE) + barra(x + 16, y + h - 20, (w - 32) * 0.7, 8, AMARILLO)
    out += f'<circle cx="{n(x + 16 + (w - 32) * 0.7)}" cy="{y + h - 16}" r="7" fill="{SUPERFICIE}" stroke="{AMBAR}" stroke-width="2.5"/>'
    return out


def pantalla_nexo(x, y, w, h):
    """Un panel como el de Nexo: aparatos con su punto de estado y el anillo de batería."""
    out = barra_superior(x, y, w)
    filas = 3
    for i in range(filas):
        fy = y + 28 + i * ((h - 36) / filas)
        out += (f'<rect x="{x + 10}" y="{n(fy)}" width="{n(w * 0.56)}" height="{n((h - 36) / filas - 7)}" rx="6" fill="{MANCHA}"/>'
                f'<circle cx="{x + 21}" cy="{n(fy + ((h - 36) / filas - 7) / 2)}" r="5" fill="{(TURQ, CIELO, VERDE)[i]}"/>'
                + barra(x + 31, fy + ((h - 36) / filas - 7) / 2 - 3, w * 0.3, 6, PALIDO))
    cx, cy, r = x + w * 0.8, y + h * 0.56, min(w * 0.13, h * 0.26)
    out += (f'<circle cx="{n(cx)}" cy="{n(cy)}" r="{n(r)}" fill="none" stroke="{PALIDO}" stroke-width="7"/>'
            f'<path d="M{n(cx)} {n(cy - r)}a{n(r)} {n(r)} 0 1 1-{n(r * 0.95)} {n(r * 1.31)}" fill="none" stroke="{VERDE}" stroke-width="7" stroke-linecap="round"/>')
    return out


def pantalla_doble(x, y, w, h):
    """Una ventana que sigue en la otra pantalla."""
    out = barra_superior(x, y, w, AZUL)
    out += f'<rect x="{x + 10}" y="{y + 28}" width="{w - 20}" height="{h - 40}" rx="6" fill="{MANCHA}"/>'
    for i, alto in enumerate((26, 40, 32, 50, 36)):
        bx = x + 18 + i * ((w - 36) / 5)
        out += f'<rect x="{n(bx)}" y="{y + h - 18 - alto}" width="{n((w - 36) / 5 - 8)}" height="{alto}" rx="4" fill="{(TURQ2, AZUL2, CIELO, TURQ, AZUL)[i]}"/>'
    return out


# ---------------------------------------------------------------- objetos grandes

def teclado_grande(x, y, w=250, h=104, giro=-6):
    """Un teclado visto desde arriba, con ⌘ y C en turquesa (copiar)."""
    out = f'<g transform="rotate({giro} {x + w / 2} {y + h / 2})">'
    out += f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="16" fill="{SUPERFICIE}" stroke="{CONTORNO}" stroke-width="3"/>'
    paso = (w - 28) / 10
    for fila in range(3):
        for col in range(10):
            kx, ky = x + 14 + col * paso, y + 14 + fila * 22
            color = TURQ if (fila, col) in ((2, 3),) else TURQ2 if (fila, col) == (2, 1) else MANCHA
            out += f'<rect x="{n(kx)}" y="{ky}" width="{n(paso - 5)}" height="17" rx="4" fill="{color}"/>'
    out += f'<rect x="{n(x + 14 + paso * 2.5)}" y="{y + 80}" width="{n(paso * 5 - 5)}" height="14" rx="4" fill="{PALIDO}"/>'
    out += f'<rect x="{x + 14}" y="{y + 80}" width="{n(paso * 2.2)}" height="14" rx="4" fill="{MANCHA}"/>'
    out += f'<rect x="{n(x + 14 + paso * 7.8)}" y="{y + 80}" width="{n(paso * 2.2 - 5)}" height="14" rx="4" fill="{MANCHA}"/>'
    return out + '</g>'


def raton(x, y):
    return (f'<rect x="{x}" y="{y}" width="40" height="62" rx="20" fill="{SUPERFICIE}" stroke="{CONTORNO}" stroke-width="3"/>'
            + trazo(f'M{x + 20} {y + 4}v16', CONTORNO, 3) + f'<rect x="{x + 17}" y="{y + 10}" width="6" height="10" rx="3" fill="{TURQ2}"/>')


def microfono_estudio(x, mesa=302):
    """Micrófono de estudio en su pie, con las ondas del sonido."""
    return (f'<ellipse cx="{x}" cy="{mesa - 3}" rx="26" ry="6" fill="{MARINO}"/>'
            f'<rect x="{x - 4}" y="{mesa - 92}" width="8" height="90" rx="4" fill="{MARINO}"/>'
            + trazo(f'M{x - 30} {mesa - 120}a30 30 0 0 0 60 0', MARINO, 6)
            + f'<rect x="{x - 24}" y="{mesa - 196}" width="48" height="96" rx="24" fill="{AZUL}"/>'
            + trazo(f'M{x - 12} {mesa - 176}h24M{x - 12} {mesa - 162}h24M{x - 12} {mesa - 148}h24', '#fff', 3)
            + f'<rect x="{x - 24}" y="{mesa - 134}" width="48" height="10" fill="{TURQ}"/>'
            + trazo(f'M{x + 42} {mesa - 168}a28 28 0 0 1 0 40M{x + 56} {mesa - 182}a46 46 0 0 1 0 68', CIELO, 4))


def reloj_despertador(cx, cy, r=72):
    """Reloj grande con campanas: la hora de la escena."""
    out = (f'<path d="M{cx - r * 0.55} {n(cy + r * 0.8)}l-14 22M{cx + r * 0.55} {n(cy + r * 0.8)}l14 22" stroke="{MARINO}" stroke-width="8" stroke-linecap="round"/>'
           f'<path d="M{n(cx - r * 0.95)} {n(cy - r * 0.62)}a30 30 0 0 1 42-36z" fill="{TURQ2}"/>'
           f'<path d="M{n(cx + r * 0.95)} {n(cy - r * 0.62)}a30 30 0 0 0-42-36z" fill="{TURQ2}"/>'
           f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{SUPERFICIE}" stroke="{MARINO}" stroke-width="9"/>')
    for i in range(12):
        a = math.radians(i * 30)
        largo = 10 if i % 3 == 0 else 5
        out += trazo(f'M{n(cx + (r - 14) * math.sin(a))} {n(cy - (r - 14) * math.cos(a))}L{n(cx + (r - 14 - largo) * math.sin(a))} {n(cy - (r - 14 - largo) * math.cos(a))}',
                     MESA if i % 3 else MARINO, 3.5)
    out += trazo(f'M{cx} {cy}L{cx} {cy - r * 0.55}M{cx} {cy}L{n(cx + r * 0.4)} {n(cy + r * 0.18)}', MARINO, 6)
    out += f'<circle cx="{cx}" cy="{cy}" r="7" fill="{TURQ}"/>'
    return out


def casa(x, y):
    """La casa: techo azul, ventanas con luz y puerta turquesa. Apoya en el piso."""
    return (f'<rect x="{x}" y="{y}" width="190" height="{386 - y}" rx="8" fill="{SUPERFICIE}" stroke="{CONTORNO}" stroke-width="3"/>'
            f'<path d="M{x - 22} {y + 12}L{x + 95} {y - 78}L{x + 212} {y + 12}" fill="none" stroke="{AZUL}" stroke-width="22" stroke-linecap="round" stroke-linejoin="round"/>'
            f'<rect x="{x + 132}" y="{y - 64}" width="22" height="40" rx="4" fill="{MARINO}"/>'
            + ''.join(f'<rect x="{x + dx}" y="{y + 30}" width="42" height="40" rx="8" fill="{AMARILLO_P}" stroke="{AMARILLO}" stroke-width="3"/>'
                      + trazo(f'M{x + dx + 21} {y + 32}v36M{x + dx + 2} {y + 50}h38', AMARILLO, 2.5) for dx in (22, 126))
            + f'<rect x="{x + 74}" y="{y + 70}" width="42" height="{386 - y - 70}" rx="8" fill="{TURQ2}"/>'
            + f'<circle cx="{x + 106}" cy="{y + 110}" r="3.5" fill="#fff"/>')


def boton_rayo(cx, cy, r=76):
    """Un botón grande con el rayo: todo en un toque."""
    return (f'<circle cx="{cx}" cy="{cy}" r="{r + 16}" fill="{PALIDO}"/>'
            f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{TURQ2}"/>'
            f'<circle cx="{cx}" cy="{cy - 6}" r="{r - 10}" fill="{TURQ}"/>'
            f'<path d="M{cx + 8} {cy - 50}L{cx - 30} {cy + 6}h28l-8 46 40-58h-28z" fill="#fff" stroke="#fff" stroke-width="4" stroke-linejoin="round"/>')


def bombilla_colgante(cx, techo=60, largo=70):
    """Lámpara colgante encendida, con su halo."""
    y = techo + largo
    return (f'<circle cx="{cx}" cy="{y + 30}" r="74" fill="{HALO}" opacity=".5"/>'
            f'<circle cx="{cx}" cy="{y + 30}" r="46" fill="{HALO}"/>'
            f'<rect x="{cx - 2}" y="{techo}" width="4" height="{largo}" fill="{MARINO}"/>'
            f'<circle cx="{cx}" cy="{y + 24}" r="14" fill="{AMARILLO}"/>'
            f'<path d="M{cx - 44} {y + 24}a44 36 0 0 1 88 0z" fill="{AZUL}"/>'
            f'<rect x="{cx - 9}" y="{y - 14}" width="18" height="14" rx="4" fill="{MARINO}"/>'
            + trazo(''.join(rayos(cx, y + 24, 24, 34, 1, a) for a in (45, 90, 135)), AMBAR, 3.5))


def parlante(x, mesa=302):
    return (f'<rect x="{x}" y="{mesa - 62}" width="40" height="62" rx="10" fill="{MARINO}"/>'
            f'<circle cx="{x + 20}" cy="{mesa - 22}" r="12" fill="{AZUL2}"/><circle cx="{x + 20}" cy="{mesa - 22}" r="5" fill="{MARINO}"/>'
            f'<circle cx="{x + 20}" cy="{mesa - 48}" r="5" fill="{AZUL2}"/>')


def libros(x, mesa=302):
    return (f'<rect x="{x}" y="{mesa - 14}" width="60" height="14" rx="3" fill="{AZUL}"/>'
            f'<rect x="{x + 6}" y="{mesa - 27}" width="52" height="13" rx="3" fill="{TURQ2}"/>'
            f'<rect x="{x + 2}" y="{mesa - 39}" width="48" height="12" rx="3" fill="{AMBAR}"/>')


# ---------------------------------------------------------------- tema y archivo

def tematizar(svg):
    """A cada elemento con un color de la paleta le agrega su clase nxe-f/nxe-s (relleno y trazo)."""
    usadas = set()

    def cambiar(m):
        etiqueta, atributos = m.group(1), m.group(2)
        clases = []
        f = re.search(r'\sfill="([^"]+)"', atributos)
        s = re.search(r'\sstroke="([^"]+)"', atributos)
        if f and f.group(1) in TEMA:
            clases.append('nxe-f' + TEMA[f.group(1)][0])
            usadas.add(('f', f.group(1)))
        if s and s.group(1) in TEMA:
            clases.append('nxe-s' + TEMA[s.group(1)][0])
            usadas.add(('s', s.group(1)))
        if not clases:
            return m.group(0)
        return f'<{etiqueta} class="{" ".join(clases)}"{atributos}>'

    return re.sub(r'<(path|rect|circle|ellipse|g)(\s[^>]*)>', cambiar, svg), usadas


def reglas(usadas):
    """Las reglas de cada clase usada, (tipo, color) con tipo 'f' o 's', con el color claro de respaldo."""
    out = ''
    for tipo, fijo in sorted(usadas, key=lambda u: (TEMA[u[1]][0], u[0])):
        k = TEMA[fijo][0]
        out += f'.nxe-{tipo}{k}{{{"fill" if tipo == "f" else "stroke"}:var(--ilus-{k},{fijo})}}'
    return out


APAGADO = {TURQ: GRIS2, TURQ2: GRIS2, TURQ3: GRIS, AZUL: GRIS2, AZUL2: GRIS, CIELO: GRIS, AMARILLO: GRIS,
           AMBAR: GRIS2, VERDE: GRIS2, '#00A383': GRIS2, '#DB8A06': GRIS2, MANCHA_T: 'rgba(100,116,139,0.08)'}
TEMA['rgba(100,116,139,0.08)'] = ('q', 'rgba(148,163,184,0.06)')

MANCHAS = {
    'a': 'M74 252C42 170 96 82 196 66c90-14 140-46 210-6 70 40 98 112 68 182-30 70 0 118-92 136-92 18-150-18-220-8-70 10-66-48-88-118z',
    'b': 'M60 240C40 160 110 76 210 70c90-6 128-40 204 0 70 38 92 120 62 190-28 66-6 110-96 120-96 10-150-26-220-14-80 14-82-50-100-126z',
}

ESCRITAS = []


def escena(nombre, cuerpo, mancha='a', apagada=False):
    dibujo = (f'<path d="{MANCHAS[mancha]}" fill="{MANCHA_T}"/>'
              f'<ellipse cx="262" cy="388" rx="214" ry="10" fill="{SOMBRA_T}"/>' + ''.join(cuerpo))
    if apagada:
        for de, a in APAGADO.items():
            dibujo = dibujo.replace(f'"{de}"', f'"{a}"')
    dibujo, usadas = tematizar(dibujo)
    # Recortado al dibujo (de y=60 al piso). Sin <title>: lo accesible lo pone la app.
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 60 520 338">'
           f'<style>{reglas(usadas)}</style>{dibujo}</svg>\n')
    SALIDA.mkdir(parents=True, exist_ok=True)
    (SALIDA / f'{nombre}.svg').write_text(svg, encoding='utf-8')
    ESCRITAS.append(nombre)


def escribir_css():
    """Las reglas de todas las clases y las variables de los dos temas, con los mismos selectores de global.css."""
    oscuro = ';'.join(f'--ilus-{k}:{d}' for k, d in TEMA.values())
    claro = ';'.join(f'--ilus-{k}:{f}' for f, (k, _) in TEMA.items())
    CSS.write_text(
        '/* Generado por diseno/escenas/generar.py: no lo edites a mano. Colores de las ilustraciones de las escenas\n'
        '   y de Remoto pegadas en línea (IlustracionEscena.svelte). Oscuro por defecto; claro como en global.css. */\n'
        f'{reglas([(t, c) for c in TEMA for t in "fs"])}\n'
        f':root{{{oscuro}}}\n'
        f"[data-theme='light']{{{claro}}}\n"
        f"@media (prefers-color-scheme: light){{:root:not([data-theme='dark']){{{claro}}}}}\n",
        encoding='utf-8')


# ---------------------------------------------------------------- escenas (una por ícono de escena)

def estudio():
    """Modo estudio (ícono «audio»): audífonos puestos, la lámpara encendida y la laptop con las notas."""
    escena('audio', [
        punteada('M300 160c10-40 40-70 80-76M40 140c14-40 46-62 90-64'),
        tarjeta(300, 92, ICONOS['audifonos']), tarjeta(452, 150, ICONOS['sol70']),
        destello(36, 210, AMARILLO, 10), destello(470, 236, TURQ2, 10),
        lampara(286, -1, 'encendida'),
        escritorio(30, 306), laptop(50, 180, 166, 112, pantalla_estudio), taza(240),
        persona(390, tipo='mujer', piel='media', ropa=TURQ2, rayas=False, izq='senalar', der='abajo', con_audifonos=True,
                color_audifonos=AZUL),
    ])


def remoto(variante=''):
    """Modo remoto y la cabecera de Remoto: el PC de la oficina a la izquierda, la MacBook de la casa a la derecha
    y la conexión por Tailscale al medio. variante: '' (lista), 'conectado' (línea firme y visto) o 'sin' (apagada)."""
    conectado = variante == 'conectado'
    linea = (trazo('M226 176C232 118 296 112 316 190', VERDE, 4) if conectado
             else punteada('M226 176C232 118 296 112 316 190', TURQ2 if not variante else CONTORNO, 3))
    cuerpo = [
        punteada('M40 150c10-40 30-60 64-68M440 220c30-14 46-40 46-76'),
        tarjeta(36, 74, ICONOS['teclado']), tarjeta(456, 96, ICONOS['comando'], 44),
        escritorio(18, 240), monitor(34, 146, 186, 136, pantalla_remota),
        f'<rect x="196" y="252" width="30" height="50" rx="6" fill="{MARINO}"/><circle cx="211" cy="264" r="3" fill="{TURQ}"/>',
        escritorio(290, 504), laptop(326, 196, 150, 96, pantalla_mac),
        linea, tarjeta(249, 98, ICONOS['escudo'], 46),
    ]
    if conectado:
        cuerpo.append(insignia(474, 202, 'visto', 18))
    else:
        cuerpo += [destello(150, 108, AMARILLO, 10)]
    escena('remoto' + (f'-{variante}' if variante else ''), cuerpo, mancha='b', apagada=variante == 'sin')


def noche():
    """Modo noche: la luna en la ventana, la lámpara tibia y baja, la pantalla tenue y una taza en la mano."""
    escena('luna', [
        ventana(46, 80, 124, 96, noche=True),
        tarjeta(452, 110, ICONOS['luna']), tarjeta(222, 76, ICONOS['bombilla'], 42),
        lampara(286, -1, 'tibia'),
        escritorio(30, 312), laptop(70, 206, 150, 86, pantalla_noche), taza(252),
        persona(400, tipo='hombre', piel='morena', ropa=AZUL, izq='abajo', der='taza'),
    ], mancha='b')


def reunion():
    """Modo reunión: audífonos puestos frente a la videollamada, con la tarjeta del micrófono."""
    escena('camara', [
        punteada('M300 70c50-10 100 10 120 50M30 230c-6-50 6-90 30-110'),
        tarjeta(300, 70, ICONOS['microfono']), tarjeta(454, 150, ICONOS['camara'], 44),
        destello(30, 110, AMARILLO, 10),
        escritorio(26, 304), monitor(40, 112, 240, 172, pantalla_llamada),
        persona(392, tipo='hombre', piel='clara', ropa=AZUL, izq='abajo', der='saludar', con_audifonos=True),
    ])


def microfono():
    escena('microfono', [
        punteada('M300 70c50-10 100 10 120 50M36 210c-4-50 10-90 40-110'),
        tarjeta(36, 74, ICONOS['onda'], 44), tarjeta(452, 150, ICONOS['volumen']),
        destello(300, 120, AMARILLO, 10),
        escritorio(30, 300), laptop(54, 214, 104, 78, pantalla_estudio), microfono_estudio(210),
        persona(392, tipo='mujer_larga', piel='morena', ropa=CIELO, rayas=False, izq='abajo', der='abajo', pelo='#3B2A25',
                con_audifonos=True),
    ], mancha='b')


def sol():
    escena('sol', [
        ventana(40, 86, 116, 100),
        tarjeta(302, 74, ICONOS['sol70']), tarjeta(456, 160, ICONOS['pantalla'], 42),
        destello(196, 96, AMARILLO), destello(30, 240, TURQ2, 10),
        escritorio(30, 300), monitor(150, 136, 140, 136, pantalla_brillo),
        persona(394, tipo='mujer', piel='clara', ropa=AMBAR, rayas=False, izq='senalar', der='abajo'),
    ])


def portatil():
    escena('portatil', [
        punteada('M300 66c50-10 100 10 120 50M40 120c20-50 60-60 110-58'),
        tarjeta(40, 76, ICONOS['bluetooth'], 44), tarjeta(298, 76, ICONOS['bateria'], 42), tarjeta(456, 150, ICONOS['audifonos'], 44),
        destello(250, 140, AMARILLO, 10),
        escritorio(26, 300), laptop(60, 166, 196, 126, pantalla_nexo), insignia(252, 172, 'visto', 16),
        persona(392, tipo='hombre_barba', piel='media', ropa=TURQ2, izq='abajo', der='saludar'),
    ], mancha='b')


def pantallas():
    escena('pantallas', [
        tarjeta(118, 68, ICONOS['pantalla'], 44), tarjeta(300, 72, ICONOS['sol'], 42),
        destello(40, 110, AMARILLO, 10), destello(360, 92, TURQ2, 10),
        escritorio(18, 376), monitor(28, 128, 170, 140, pantalla_nexo), monitor(204, 128, 170, 140, pantalla_doble),
        persona(440, tipo='mujer_larga', piel='media', ropa=AZUL, rayas=False, izq='senalar', der='abajo', pelo='#5A3B2E'),
    ])


def teclado():
    escena('teclado', [
        punteada('M300 70c50-10 100 10 120 50M40 330c-10-50 0-90 30-120'),
        tarjeta(40, 80, ICONOS['comando']), tarjeta(296, 90, ICONOS['teclado'], 44),
        destello(250, 170, AMARILLO, 10),
        teclado_grande(36, 196), raton(306, 270),
        persona(410, tipo='hombre', piel='morena', ropa=TURQ2, rayas=False, izq='senalar', der='abajo'),
    ], mancha='b')


def rayo():
    """Rayo: todo en un toque. También es la ilustración genérica para un ícono que no se conoce."""
    escena('rayo', [
        punteada('M40 330c-10-50 0-90 30-120M300 70c50-10 100 10 120 50'),
        tarjeta(40, 80, ICONOS['sol'], 44), tarjeta(286, 70, ICONOS['audifonos'], 42), tarjeta(458, 150, ICONOS['visto'], 42),
        destello(80, 300, AMARILLO), destello(270, 330, TURQ2, 10), destello(300, 170, AMARILLO, 10),
        boton_rayo(164, 236),
        persona(392, tipo='mujer', piel='morena', ropa=AZUL, rayas=False, izq='senalar', der='abajo'),
    ])


def bombilla():
    escena('bombilla', [
        tarjeta(300, 86, ICONOS['bombilla']), tarjeta(452, 160, ICONOS['wifi'], 42),
        destello(40, 120, AMARILLO, 10),
        bombilla_colgante(164, 60, 62),
        escritorio(30, 300), parlante(52), libros(196), planta(130, 302, 0.8),
        persona(392, tipo='hombre', piel='clara', ropa=CIELO, izq='abajo', der='saludar'),
    ], mancha='b')


def reloj():
    escena('reloj', [
        punteada('M300 70c50-10 100 10 120 50M40 330c-10-50 0-90 30-120'),
        tarjeta(36, 80, ICONOS['campana'], 44), tarjeta(296, 84, ICONOS['calendario'], 42), tarjeta(456, 160, ICONOS['luna'], 42),
        destello(270, 200, AMARILLO, 10),
        reloj_despertador(166, 268, 76),
        persona(394, tipo='mujer_larga', piel='clara', ropa=TURQ2, rayas=False, izq='abajo', der='telefono', pelo='#5A3B2E'),
    ])


def inicio():
    escena('inicio', [
        tarjeta(36, 120, ICONOS['wifi'], 42), tarjeta(296, 76, ICONOS['bombilla'], 44), tarjeta(458, 150, ICONOS['audifonos'], 42),
        destello(250, 168, AMARILLO, 10),
        casa(74, 236),
        persona(394, tipo='hombre', piel='media', ropa=AZUL, izq='abajo', der='saludar'),
    ], mancha='b')


def setup():
    """El escritorio completo, sin nadie: monitor, laptop, lámpara, planta y taza."""
    escena('setup', [
        tarjeta(36, 76, ICONOS['teclado'], 44), tarjeta(456, 90, ICONOS['audifonos'], 44),
        destello(300, 92, AMARILLO, 10),
        lampara(90, 1, 'encendida'),
        escritorio(20, 500), planta(46, 302, 0.9), monitor(160, 120, 196, 160, pantalla_nexo),
        laptop(376, 214, 104, 78, pantalla_mac), taza(146),
    ])


def nueva():
    """Nueva escena: un escritorio vacío y amable con la tarjeta punteada del «+»."""
    escena('nueva', [
        punteada('M300 70c50-10 100 10 120 50'),
        f'<rect x="132" y="108" width="104" height="104" rx="26" fill="{SUPERFICIE}" stroke="{TURQ2}" stroke-width="3.5" stroke-dasharray="9 9"/>',
        trazo('M184 136v48M160 160h48', TURQ2, 7),
        destello(270, 120, AMARILLO, 10), destello(100, 150, TURQ2, 10),
        escritorio(40, 320), planta(84, 302, 0.9), taza(290),
        persona(408, tipo='mujer', piel='media', ropa=CIELO, rayas=False, izq='abajo', der='pensar'),
    ], mancha='b')


ESCENAS = [estudio, remoto, noche, reunion, microfono, sol, portatil, pantallas, teclado, rayo, bombilla, reloj,
           inicio, setup, nueva, lambda: remoto('conectado'), lambda: remoto('sin')]


if __name__ == '__main__':
    for f in ESCENAS:
        f()
    escribir_css()
    grandes = []
    for nombre in ESCRITAS:
        kb = (SALIDA / f'{nombre}.svg').stat().st_size / 1024
        print(f'{nombre:20} {kb:5.1f} KB')
        if kb > 12:
            grandes.append(nombre)
    print(f'{len(ESCRITAS)} ilustraciones en {SALIDA.relative_to(RAIZ)} y {CSS.relative_to(RAIZ)}')
    if grandes:
        sys.exit('Pasan de 12 KB: ' + ', '.join(grandes))
