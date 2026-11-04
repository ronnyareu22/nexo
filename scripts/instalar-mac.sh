#!/bin/zsh
# Compila Nexo para Mac, lo firma con el certificado propio "Nexo Local (Ronny)" y lo instala en ~/Applications.
# Con la misma firma en cada versión, macOS conserva los permisos (Bluetooth, Accesibilidad, Grabación de pantalla).
# El certificado vive solo en el llavero de esta Mac (ver ~/.config/nexo-firma/cert.pem). Si no está, firma localmente.
set -euo pipefail
cd "${0:A:h}/.."
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
FIRMA="Nexo Local (Ronny)"
security find-identity -p codesigning | grep -q "$FIRMA" || FIRMA="-"
export FIRMA

./widget-mac/construir.sh
pnpm tauri build --bundles app

APP=src-tauri/target/release/bundle/macos/Nexo.app
# Por dentro primero (widget y ayudante, con sus permisos), después la app entera, sin --deep.
codesign --force --sign "$FIRMA" --entitlements widget-mac/NexoWidget.entitlements --timestamp=none "$APP/Contents/PlugIns/NexoWidget.appex"
codesign --force --sign "$FIRMA" --identifier com.ronny.nexo --timestamp=none "$APP/Contents/MacOS/nexo-recargar-widgets"
codesign --force --sign "$FIRMA" --timestamp=none "$APP"
codesign --verify --deep --strict "$APP"

# Cierra el Nexo abierto (por su PID) e instala la versión nueva.
DESTINO=~/Applications/Nexo.app
PID=$(pgrep -f "$DESTINO/Contents/MacOS/nexo$" || true)
[ -n "$PID" ] && kill $PID && sleep 1.5
ditto "$APP" "$DESTINO.nuevo" && rm -rf "$DESTINO" && mv "$DESTINO.nuevo" "$DESTINO"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$DESTINO"
pluginkit -a "$DESTINO/Contents/PlugIns/NexoWidget.appex"
open "$DESTINO"
echo "Instalado y firmado con: $FIRMA"
