#!/usr/bin/env python3
"""Escribe Metadata.appintents para los botones de brillo del widget.

Xcode lo genera solo (appintentsmetadataprocessor), pero aquí no hay Xcode: se escribe a mano con el mismo
formato que usan las apps de Apple. Sin esto, macOS no sabe qué acción correr al tocar un botón del widget.
Uso: python3 metadatos.py <carpeta Resources del .appex>
"""
import json
import os
import sys

MODULO = "NexoWidget"
ACCIONES = {
    "BajarBrillo0": "Bajar brillo",
    "SubirBrillo0": "Subir brillo",
    "BajarBrillo1": "Bajar brillo de la segunda pantalla",
    "SubirBrillo1": "Subir brillo de la segunda pantalla",
}


def mangled(nombre: str) -> str:
    # Nombre de tipo de Swift para un struct: <largo><módulo><largo><nombre>V
    return f"{len(MODULO)}{MODULO}{len(nombre)}{nombre}V"


def accion(nombre: str, titulo: str) -> dict:
    return {
        "assistantDefinedSchemas": [],
        "assistantDefinedSchemaTraits": [],
        "authenticationPolicy": 0,
        "availabilityAnnotations": {"LNPlatformNameWildcard": {"introducedVersion": "*"}},
        "descriptionMetadata": {"descriptionText": {"alternatives": [], "key": titulo}, "searchKeywords": []},
        "effectiveBundleIdentifiers": [],
        "fullyQualifiedTypeName": f"{MODULO}.{nombre}",
        "identifier": nombre,
        "isAuthPolExplicit": False,
        "isDiscoverable": False,
        "mangledTypeName": mangled(nombre),
        "mangledTypeNameByBundleIdentifier": {},
        "mangledTypeNameByBundleIdentifierV2": {},
        "mangledTypeNameV2": mangled(nombre),
        "openAppWhenRun": False,
        "outputFlags": 0,
        "parameters": [],
        "presentationStyle": 0,
        "requiredCapabilities": [],
        "supportedModes": 1,
        "systemProtocolMetadata": [],
        "systemProtocolMetadataV2": [],
        "systemProtocols": [],
        "title": {"alternatives": [], "key": titulo},
        "typeSpecificMetadata": [],
        "visibilityMetadata": {"assistantOnly": False, "isDiscoverable": False},
    }


def main() -> None:
    destino = os.path.join(sys.argv[1], "Metadata.appintents")
    os.makedirs(destino, exist_ok=True)
    datos = {
        "actions": {n: accion(n, t) for n, t in ACCIONES.items()},
        "assistantEntities": [],
        "assistantIntentNegativePhrases": [],
        "assistantIntents": [],
        "autoShortcuts": [],
        "entities": {},
        "enums": [],
        "generator": {"name": "xcode-tools", "version": "17E6107"},
        "negativePhrases": [],
        "queries": {},
        "shortcutTileColor": 14,
        "version": 1,
    }
    with open(os.path.join(destino, "extract.actionsdata"), "w") as f:
        json.dump(datos, f, indent=2, sort_keys=True)
    with open(os.path.join(destino, "version.json"), "w") as f:
        json.dump({"version": "3.0", "toolsVersion": "17E6107"}, f, indent=2)


if __name__ == "__main__":
    main()
