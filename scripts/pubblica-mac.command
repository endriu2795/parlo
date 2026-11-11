#!/bin/bash
# One command on macOS; the full Xcode app is not required.
set -euo pipefail
PROJECT="$(cd "$(dirname "$0")/.." && pwd)"
if [ "$(uname -s)" != Darwin ]; then echo 'Questo comando va eseguito sul Mac.'; exit 1; fi
if [ "$(/usr/bin/sw_vers -productVersion | cut -d. -f1)" -lt 13 ]; then echo 'Serve macOS 13 o successivo.'; exit 1; fi
if [ "$(/usr/sbin/sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ] && [ "$(uname -m)" != arm64 ]; then
  exec /usr/bin/arch -arm64 /bin/bash "$0"
fi
if ! /usr/bin/xcrun --sdk macosx --show-sdk-path >/dev/null 2>&1; then
  echo 'Serve installare soltanto i Command Line Tools gratuiti di Apple, non tutta l’app Xcode.'
  /usr/bin/xcode-select --install || true
  echo 'Conferma la finestra Apple. Quando ha finito, riesegui: bash scripts/pubblica-mac.command'
  exit 1
fi
mkdir -p "$PROJECT/.tools/downloads"
case "$(uname -m)" in
  arm64) NODE_ARCH=arm64; NODE_SHA=23b25245dcfb9af7262f8ff142e9e2e0af025368117329e7a7458a51e5922f53 ;;
  x86_64) NODE_ARCH=x64; NODE_SHA=8a677b0219178efd6eb0e475457c4afb452b521a92f6e67845a73bd85727f2a8 ;;
  *) echo 'Architettura Mac non supportata.'; exit 1 ;;
esac
NODE_FOLDER="$PROJECT/.tools/node-v22.23.3-darwin-$NODE_ARCH"
NODE_ARCHIVE="$PROJECT/.tools/downloads/node-v22.23.3-darwin-$NODE_ARCH.tar.gz"
if [ ! -x "$NODE_FOLDER/bin/node" ]; then
  echo 'Preparazione locale di Node.js…'
  /usr/bin/curl --fail --location --retry 3 --proto '=https' --proto-redir '=https' "https://nodejs.org/dist/v22.23.3/node-v22.23.3-darwin-$NODE_ARCH.tar.gz" --output "$NODE_ARCHIVE.part"
  ACTUAL="$(/usr/bin/shasum -a 256 "$NODE_ARCHIVE.part" | cut -d' ' -f1)"
  if [ "$ACTUAL" != "$NODE_SHA" ]; then echo 'Verifica del download Node.js fallita.'; exit 1; fi
  mv "$NODE_ARCHIVE.part" "$NODE_ARCHIVE"
  /usr/bin/tar -xzf "$NODE_ARCHIVE" -C "$PROJECT/.tools"
fi
export PATH="$NODE_FOLDER/bin:$PATH"
unset PYTHONHOME PYTHONPATH
cd "$PROJECT"
"$NODE_FOLDER/bin/node" scripts/pubblica-mac.mjs
