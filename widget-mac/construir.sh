#!/bin/zsh
# Compila los widgets nativos de Nexo (WidgetKit) sin Xcode, solo con las herramientas de línea de comandos.
# Deja widget-mac/build/NexoWidget.appex firmado con $FIRMA (o localmente si no hay); tauri.conf.json lo copia a Nexo.app/Contents/PlugIns.
set -euo pipefail
cd "${0:A:h}"
SDK=$(xcrun --show-sdk-path)
APPEX=build/NexoWidget.appex
rm -rf build && mkdir -p "$APPEX/Contents/MacOS" "$APPEX/Contents/Resources"
cp Info.plist "$APPEX/Contents/Info.plist"
# Fotos de los modelos conocidos (las mismas de la app).
cp ../static/dispositivos/beats-solo3.png ../static/dispositivos/sony-wh-ch520.png "$APPEX/Contents/Resources/"
swiftc -O -target arm64-apple-macos14.0 -sdk "$SDK" -parse-as-library -application-extension \
  -module-name NexoWidget NexoWidget.swift -o "$APPEX/Contents/MacOS/NexoWidget" \
  -Xlinker -e -Xlinker _NSExtensionMain  # como Xcode: arranca por NSExtensionMain, no por main
# Datos de los botones (lo que Xcode generaría con appintentsmetadataprocessor).
python3 metadatos.py "$APPEX/Contents/Resources"
codesign --force --sign "${FIRMA:--}" --entitlements NexoWidget.entitlements --timestamp=none "$APPEX"
# Ayudante que pide redibujar los widgets (va en Nexo.app/Contents/MacOS).
swiftc -O -target arm64-apple-macos14.0 -sdk "$SDK" recargar.swift -o build/nexo-recargar-widgets
codesign --force --sign "${FIRMA:--}" --identifier com.ronny.nexo --timestamp=none build/nexo-recargar-widgets
echo "Listo: $APPEX"
