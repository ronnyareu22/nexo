#!/usr/bin/env python3
"""Genera web/artifact.html a partir de web/index.html, para que las dos nunca se separen.

index.html es la página completa (con doctype, <html>, <head> y <body>) para alojarla donde sea.
artifact.html es la misma página en el formato de un Artifact de claude.ai: sin doctype, <html>, <head>
ni <body>. Empieza con <title> y la meta de los lados; luego las fuentes, el <style>, el contenido y el <script>.

Uso: python3 web/generar-artifact.py   (después de cada cambio en index.html)
"""
import pathlib
import re
import sys

AQUI = pathlib.Path(__file__).resolve().parent
html = (AQUI / 'index.html').read_text(encoding='utf-8')

cabeza = re.search(r'<head>(.*?)</head>', html, re.S)
cuerpo = re.search(r'<body>(.*?)</body>', html, re.S)
if not cabeza or not cuerpo:
    sys.exit('index.html debe tener <head> y <body> sin atributos.')
cabeza, cuerpo = cabeza.group(1), cuerpo.group(1)

titulo = re.search(r'<title>.*?</title>', cabeza, re.S).group(0)
descripcion = re.search(r'<meta name="description"[^>]*>', cabeza)
fuentes = [l for l in re.findall(r'<link [^>]*>', cabeza) if 'fonts.googleapis.com/css' in l]
estilos = re.findall(r'<style>.*?</style>', cabeza, re.S)

partes = [titulo, '<meta name="claude:viewport-fit-sides" content="cover">']
if descripcion:
    partes.append(descripcion.group(0))
partes += fuentes + estilos + [cuerpo.strip()]

salida = '\n'.join(partes) + '\n'
sobra = re.search(r'<(!doctype|html|head|body)[\s>]|</(html|head|body)>', salida, re.I)
assert not sobra, f'Sobra una etiqueta de documento: {sobra.group(0)}'
(AQUI / 'artifact.html').write_text(salida, encoding='utf-8')
print(f'artifact.html: {len(salida.encode("utf-8")):,} bytes')
