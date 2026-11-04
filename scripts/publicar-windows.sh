#!/bin/zsh
# Publica Nexo para Windows desde esta Mac. Lo corres tú: ./scripts/publicar-windows.sh
#
# 1. Compila el instalador con cargo-xwin y un sello nuevo (NEXO_COMPILADO, segundos UTC): así el PC sabe que es
#    más nuevo aunque la versión siga en 0.1.0.
# 2. Lo deja en la carpeta de actualizaciones de Nexo en esta Mac, con su info.json (compilado, tamano, sha256).
#    El Nexo del PC lo encuentra solo por XDrop (a los 20 s de abrirse y cada 30 min) y ofrece "Actualizar":
#    lo baja, revisa la huella, lo instala en silencio y se vuelve a abrir.
# 3. Copia el instalador y el .exe portable a Google Drive › Nexo.
#
# Hace falta: brew install llvm lld nsis · rustup target add x86_64-pc-windows-msvc · cargo install --locked cargo-xwin.
# Para que el PC lo vea: esta Mac con Tailscale y Nexo abiertos (XDrop › Recibir encendido), y con un Nexo que ya
# sepa ofrecer actualizaciones (si no, primero ./scripts/instalar-mac.sh).
set -euo pipefail
cd "${0:A:h}/.."
export PATH=/opt/homebrew/opt/rustup/bin:/opt/homebrew/opt/llvm/bin:$HOME/.cargo/bin:$PATH

COMPILADO=$(date -u +%s)
export NEXO_COMPILADO=$COMPILADO

echo "Compilando Nexo para Windows (sello $COMPILADO)…"
pnpm tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis

VERSION=$(sed -n 's/^ *"version": *"\([^"]*\)".*/\1/p' src-tauri/tauri.conf.json | head -1)
SALIDA=src-tauri/target/x86_64-pc-windows-msvc/release
INSTALADOR="$SALIDA/bundle/nsis/Nexo_${VERSION}_x64-setup.exe"
PORTABLE="$SALIDA/nexo.exe"
[ -f "$INSTALADOR" ] || { echo "No encontré el instalador en $INSTALADOR" >&2; exit 1; }
[ -f "$PORTABLE" ] || { echo "No encontré el .exe en $PORTABLE" >&2; exit 1; }
# Tiene que ser el de esta compilación, no uno viejo que quedó ahí.
if [ "$(stat -f %m "$INSTALADOR")" -lt "$COMPILADO" ]; then
  echo "El instalador es anterior a esta compilación: algo falló al empaquetar." >&2
  exit 1
fi

TAMANO=$(stat -f %z "$INSTALADOR")
HUELLA=$(shasum -a 256 "$INSTALADOR" | awk '{print $1}')

# 2. La carpeta que sirve el receptor de XDrop (la carpeta de ajustes de Nexo en la Mac).
CARPETA="$HOME/Library/Application Support/com.ronny.nexo/actualizaciones/windows"
mkdir -p "$CARPETA"
# Primero con nombres temporales y después a su lugar: el PC nunca ve un archivo a medias.
cp "$INSTALADOR" "$CARPETA/nexo-setup.exe.nuevo"
printf '{\n  "compilado": %s,\n  "tamano": %s,\n  "sha256": "%s",\n  "version": "%s"\n}\n' \
  "$COMPILADO" "$TAMANO" "$HUELLA" "$VERSION" > "$CARPETA/info.json.nuevo"
mv -f "$CARPETA/nexo-setup.exe.nuevo" "$CARPETA/nexo-setup.exe"
mv -f "$CARPETA/info.json.nuevo" "$CARPETA/info.json"
echo "Listo para el PC: $CARPETA"

# 3. Google Drive › Nexo (para instalar a mano o en otro PC).
DRIVE="$HOME/Library/CloudStorage/GoogleDrive-ronnyareu22@gmail.com/Mi unidad/Nexo"
if [ -d "${DRIVE:h}" ]; then
  mkdir -p "$DRIVE"
  cp "$INSTALADOR" "$DRIVE/Instalar Nexo (Windows).exe"
  cp "$PORTABLE" "$DRIVE/Nexo portable (Windows).exe"
  echo "Copiado a Google Drive › Nexo."
else
  echo "No encontré Google Drive; me salto esa copia." >&2
fi

# 4. ¿Esta Mac ya lo ofrece? Se le pregunta a su propio Nexo por su dirección de Tailscale.
TS=""
for c in /Applications/Tailscale.app/Contents/MacOS/Tailscale /usr/local/bin/tailscale /opt/homebrew/bin/tailscale; do
  [ -x "$c" ] && TS=$c && break
done
IP=""
[ -n "$TS" ] && IP=$("$TS" ip -4 2>/dev/null | head -1 || true)
if [ -n "$IP" ] && OFRECE=$(curl -fsS --max-time 4 "http://$IP:47821/nexo/actualizacion?plataforma=windows" 2>/dev/null); then
  echo "Tu Mac ya la ofrece: $OFRECE"
else
  echo "Ojo: el Nexo de esta Mac no respondió. Ábrelo (XDrop › Recibir encendido) o instala la versión nueva con ./scripts/instalar-mac.sh."
fi

echo "Publicado: Nexo $VERSION compilado el $(date -r "$COMPILADO" '+%d/%m/%Y %H:%M') · $TAMANO bytes · sha256 ${HUELLA[1,12]}…"
