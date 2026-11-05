#!/usr/bin/env python3
"""Genera las tablas de "Tu red" (Casa) desde datos públicos, sin red: se le pasan los archivos ya bajados.

  python3 -I scripts/tablas-red.py <oui> <lifx-products.json>

- <oui>: el registro MA-L de la IEEE, como `oui.csv` (https://standards-oui.ieee.org/oui/oui.csv) o como el JSON
  del paquete `oui-data` (https://cdn.jsdelivr.net/npm/oui-data@1/index.json, el mismo registro).
- <lifx-products.json>: https://github.com/LIFX/products (qué luz LIFX tiene color, su rango de blancos y cuáles
  son interruptores).

Lo bajado es dato de afuera: se baja a una carpeta vacía propia, se lee solo con csv/json y se corre con `-I`.
Escribe `src-tauri/src/sistema/hogar/red/oui.rs` y `src-tauri/src/sistema/hogar/lan/lifx_productos.rs`.
Solo se guardan las marcas que ayudan a reconocer algo en una casa (no todo el registro).
"""

import csv
import json
import os
import re
import sys

# (patrón sobre el nombre de la organización en mayúsculas, marca corta, pista de tipo)
# Pistas: "" (de todo), iot, luz, enchufe, parlante, tele, camara, impresora, consola, router, computadora, telefono.
MARCAS = [
    (r"^APPLE,? INC", "Apple", ""),
    (r"^SAMSUNG ELECTRONICS", "Samsung", ""),
    (r"^GOOGLE,? INC|^GOOGLE LLC", "Google", ""),
    (r"^NEST LABS", "Google Nest", "iot"),
    (r"^AMAZON TECHNOLOGIES|^AMAZON\.COM", "Amazon", ""),
    (r"^ESPRESSIF", "Espressif", "iot"),
    (r"^TUYA SMART", "Tuya", "iot"),
    (r"^BEKEN CORPORATION", "Beken", "iot"),
    (r"^BOUFFALO LAB", "Bouffalo Lab", "iot"),
    (r"^WINNER MICRO", "Winner Micro", "iot"),
    (r"^SHENZHEN BILIAN", "Bilian", "iot"),
    (r"^HUI ZHOU GAOSHENGDA", "Gaoshengda", "iot"),
    (r"^SIGNIFY|^PHILIPS LIGHTING", "Philips Hue", "luz"),
    (r"^LIFI LABS", "LIFX", "luz"),
    (r"^NANOLEAF", "Nanoleaf", "luz"),
    (r"^QINGDAO YEELINK", "Yeelight", "luz"),
    (r"^SHENZHEN INTELLIROCKS", "Govee", "luz"),
    (r"^WIZ CONNECTED", "WiZ", "luz"),
    (r"^ALLTERCO ROBOTICS|^SHELLY", "Shelly", "iot"),
    (r"^ELGATO", "Elgato", "luz"),
    (r"^CORSAIR", "Corsair", ""),
    (r"^LUMI UNITED", "Aqara", "iot"),
    (r"^BROADLINK|^HANGZHOU BROADLINK", "Broadlink", "iot"),
    (r"^CHENGDU MEROSS", "Meross", "enchufe"),
    (r"^BELKIN", "Belkin", "iot"),
    (r"^ECOBEE", "ecobee", "iot"),
    (r"^IROBOT", "iRobot", "iot"),
    (r"^BEIJING ROBOROCK", "Roborock", "iot"),
    (r"^ECOVACS", "Ecovacs", "iot"),
    (r"^XIAOMI COMMUNICATIONS|^BEIJING XIAOMI", "Xiaomi", ""),
    (r"^SONOS", "Sonos", "parlante"),
    (r"^BOSE CORPORATION", "Bose", "parlante"),
    (r"^ROKU", "Roku", "tele"),
    (r"^LG ELECTRONICS", "LG", ""),
    (r"^SONY INTERACTIVE", "PlayStation", "consola"),
    (r"^SONY CORPORATION|^SONY GROUP|^SONY VISUAL", "Sony", ""),
    (r"^NINTENDO", "Nintendo", "consola"),
    (r"^VALVE CORPORATION", "Valve", "consola"),
    (r"^MICROSOFT", "Microsoft", ""),
    (r"^TCL KING|^TCL TECHNOLOGY|^SHENZHEN TCL", "TCL", "tele"),
    (r"^HISENSE", "Hisense", "tele"),
    (r"^VIZIO", "Vizio", "tele"),
    (r"^PANASONIC", "Panasonic", ""),
    (r"^SHARP CORPORATION", "Sharp", ""),
    (r"^HEWLETT PACKARD|^HP INC", "HP", "impresora"),
    (r"^CANON INC", "Canon", "impresora"),
    (r"^SEIKO EPSON", "Epson", "impresora"),
    (r"^BROTHER INDUSTRIES", "Brother", "impresora"),
    (r"^XEROX", "Xerox", "impresora"),
    (r"^LEXMARK", "Lexmark", "impresora"),
    (r"^KYOCERA", "Kyocera", "impresora"),
    (r"^HANGZHOU HIKVISION", "Hikvision", "camara"),
    (r"^ZHEJIANG DAHUA", "Dahua", "camara"),
    (r"^REOLINK", "Reolink", "camara"),
    (r"^WYZE", "Wyze", "camara"),
    (r"^RING LLC", "Ring", "camara"),
    (r"^ARLO", "Arlo", "camara"),
    (r"^HANGZHOU EZVIZ", "EZVIZ", "camara"),
    (r"^TP-LINK|^TP LINK", "TP-Link", "router"),
    (r"^NETGEAR", "Netgear", "router"),
    (r"^UBIQUITI", "Ubiquiti", "router"),
    (r"^LINKSYS|^BELKIN INTERNATIONAL", "Linksys", "router"),
    (r"^ASUSTEK", "ASUS", ""),
    (r"^D-LINK", "D-Link", "router"),
    (r"^ARRIS|^COMMSCOPE", "Arris", "router"),
    (r"^TECHNICOLOR", "Technicolor", "router"),
    (r"^SAGEMCOM", "Sagemcom", "router"),
    (r"^ZTE CORPORATION", "ZTE", "router"),
    (r"^HUAWEI TECHNOLOGIES", "Huawei", ""),
    (r"^HUAWEI DEVICE", "Huawei", "telefono"),
    (r"^HONOR DEVICE", "Honor", "telefono"),
    (r"^NOKIA", "Nokia", "router"),
    (r"^ZYXEL", "Zyxel", "router"),
    (r"^MITRASTAR", "MitraStar", "router"),
    (r"^ASKEY", "Askey", "router"),
    (r"^SERCOMM", "Sercomm", "router"),
    (r"^EERO", "eero", "router"),
    (r"^ROUTERBOARD|^MIKROTIK", "MikroTik", "router"),
    (r"^SHENZHEN TENDA", "Tenda", "router"),
    (r"^MERCUSYS", "Mercusys", "router"),
    (r"^HITRON", "Hitron", "router"),
    (r"^COMPAL BROADBAND", "Compal", "router"),
    (r"^ACTIONTEC", "Actiontec", "router"),
    (r"^CALIX", "Calix", "router"),
    (r"^AVM ", "AVM", "router"),
    (r"^INTEL CORPORATE", "Intel", "computadora"),
    (r"^REALTEK", "Realtek", "computadora"),
    (r"^LITEON|^LITE-ON", "Lite-On", "computadora"),
    (r"^AZUREWAVE", "AzureWave", "computadora"),
    (r"^HON HAI", "Foxconn", "computadora"),
    (r"^DELL", "Dell", "computadora"),
    (r"^LENOVO", "Lenovo", ""),
    (r"^MICRO-STAR", "MSI", "computadora"),
    (r"^GIGA-BYTE", "Gigabyte", "computadora"),
    (r"^RASPBERRY PI", "Raspberry Pi", "computadora"),
    (r"^MOTOROLA MOBILITY", "Motorola", "telefono"),
    (r"^GUANGDONG OPPO", "OPPO", "telefono"),
    (r"^VIVO MOBILE", "vivo", "telefono"),
    (r"^ONEPLUS", "OnePlus", "telefono"),
    (r"^REALME", "realme", "telefono"),
]

PISTAS = {"", "iot", "luz", "enchufe", "parlante", "tele", "camara", "impresora", "consola", "router", "computadora", "telefono"}


def leer_oui(ruta):
    """(prefijo de 6 hex en mayúsculas, organización) del registro MA-L."""
    with open(ruta, encoding="utf-8", errors="replace") as f:
        if ruta.endswith(".json"):
            datos = json.load(f)
            for prefijo, texto in datos.items():
                if isinstance(prefijo, str) and isinstance(texto, str):
                    yield prefijo, texto.split("\n", 1)[0]
        else:
            for fila in csv.DictReader(f):
                if fila.get("Registry") == "MA-L":
                    yield fila.get("Assignment", ""), fila.get("Organization Name", "")


def tabla_oui(ruta):
    patrones = [(re.compile(p), marca, pista) for p, marca, pista in MARCAS]
    marcas = []  # (marca, pista) en orden de aparición
    prefijos = []
    for prefijo, org in leer_oui(ruta):
        if not re.fullmatch(r"[0-9A-Fa-f]{6}", prefijo or ""):
            continue
        nombre = (org or "").strip().upper()
        for patron, marca, pista in patrones:
            if patron.search(nombre):
                clave = (marca, pista)
                if clave not in marcas:
                    marcas.append(clave)
                prefijos.append((int(prefijo, 16), marcas.index(clave)))
                break
    if len(marcas) > 255:
        sys.exit("Demasiadas marcas para un byte.")
    prefijos.sort()
    return marcas, prefijos


def escribir_oui(ruta, marcas, prefijos, origen):
    for _, pista in marcas:
        assert pista in PISTAS
    lineas = [
        "//! La marca de un equipo por los 3 primeros bytes de su MAC (registro MA-L de la IEEE), solo de las marcas",
        "//! que ayudan a reconocer algo en una casa. Generado por `scripts/tablas-red.py`: no editar a mano.",
        f"//! Origen: {origen}. {len(prefijos)} prefijos de {len(marcas)} marcas.",
        "",
        "/// (marca, pista de tipo: \"\", iot, luz, enchufe, parlante, tele, camara, impresora, consola, router,",
        "/// computadora o telefono).",
        f"pub const MARCAS: [(&str, &str); {len(marcas)}] = [",
    ]
    for marca, pista in marcas:
        lineas.append(f'    ("{marca}", "{pista}"),')
    lineas.append("];")
    lineas.append("")
    lineas.append("/// `prefijo << 8 | índice en MARCAS`, en orden (para buscar por mitades).")
    lineas.append(f"pub static PREFIJOS: [u32; {len(prefijos)}] = [")
    fila = []
    for p, i in prefijos:
        fila.append(f"0x{(p << 8) | i:08x}")
        if len(fila) == 8:
            lineas.append("    " + ", ".join(fila) + ",")
            fila = []
    if fila:
        lineas.append("    " + ", ".join(fila) + ",")
    lineas.append("];")
    with open(ruta, "w", encoding="utf-8") as f:
        f.write("\n".join(lineas) + "\n")


def tabla_lifx(ruta):
    """(pid, color, kelvin mínimo, kelvin máximo, relés) de los productos de LIFX (vid 1)."""
    with open(ruta, encoding="utf-8") as f:
        datos = json.load(f)
    filas = []
    for vendedor in datos:
        if not isinstance(vendedor, dict) or vendedor.get("vid") != 1:
            continue
        base = vendedor.get("defaults") or {}
        for p in vendedor.get("products") or []:
            if not isinstance(p, dict) or not isinstance(p.get("pid"), int):
                continue
            rasgos = dict(base)
            rasgos.update(p.get("features") or {})
            rango = rasgos.get("temperature_range")
            kmin, kmax = (0, 0)
            if isinstance(rango, list) and len(rango) == 2 and all(isinstance(x, int) for x in rango):
                kmin, kmax = rango
            filas.append((p["pid"], bool(rasgos.get("color")), kmin, kmax, bool(rasgos.get("relays")), str(p.get("name", ""))[:40]))
    filas.sort()
    return filas


def escribir_lifx(ruta, filas):
    lineas = [
        "//! Qué sabe hacer cada luz LIFX por su número de producto (github.com/LIFX/products, sin las mejoras por",
        "//! versión). Generado por `scripts/tablas-red.py`: no editar a mano.",
        "",
        "/// (producto, color, Kelvin mínimo, Kelvin máximo (0 si no cambia de blanco), interruptor de relés).",
        f"pub const PRODUCTOS: [(u16, bool, u16, u16, bool); {len(filas)}] = [",
    ]
    for pid, color, kmin, kmax, reles, nombre in filas:
        limpio = re.sub(r"[^A-Za-z0-9 +()./-]", "", nombre)
        lineas.append(f"    ({pid}, {str(color).lower()}, {kmin}, {kmax}, {str(reles).lower()}), // {limpio}")
    lineas.append("];")
    with open(ruta, "w", encoding="utf-8") as f:
        f.write("\n".join(lineas) + "\n")


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    raiz = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    hogar = os.path.join(raiz, "src-tauri", "src", "sistema", "hogar")
    marcas, prefijos = tabla_oui(sys.argv[1])
    origen = "oui.csv de la IEEE" if sys.argv[1].endswith(".csv") else "oui-data (registro MA-L de la IEEE en JSON)"
    os.makedirs(os.path.join(hogar, "red"), exist_ok=True)
    escribir_oui(os.path.join(hogar, "red", "oui.rs"), marcas, prefijos, origen)
    filas = tabla_lifx(sys.argv[2])
    escribir_lifx(os.path.join(hogar, "lan", "lifx_productos.rs"), filas)
    print(f"oui: {len(prefijos)} prefijos de {len(marcas)} marcas · lifx: {len(filas)} productos")


if __name__ == "__main__":
    main()
