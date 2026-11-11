#!/bin/bash
# Finder entry point: dependencies stay in this kit, except Apple's SDK.
set -euo pipefail
PROJECT="$(cd "$(dirname "$0")/.." && pwd)"
if [ "$(uname -s)" != Darwin ]; then echo 'Questo kit va aperto su un Mac.'; exit 1; fi
# Avoid building an Intel app from a Terminal running under Rosetta.
if [ "$(/usr/sbin/sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ] && [ "$(uname -m)" != arm64 ]; then
  exec /usr/bin/arch -arm64 /bin/bash "$0"
fi
mkdir -p "$PROJECT/test-results" "$PROJECT/.tools/downloads"
exec > >(tee -a "$PROJECT/test-results/preparazione.log") 2>&1
finish() {
  result=$?
  trap - EXIT
  /usr/bin/ditto -c -k "$PROJECT/test-results" "$PROJECT/Diagnostica-PARLO-Mac.zip" || true
  if [ "$result" -ne 0 ]; then
    echo ''
    echo 'Preparazione non completata. Invia Diagnostica-PARLO-Mac.zip a chi ti ha dato il kit.'
  fi
  echo 'Premi Invio per chiudere questa finestra.'
  read -r _ || true
  exit "$result"
}
trap finish EXIT
echo "PARLO — preparazione Mac ($(date))"
/usr/bin/sw_vers
echo 'Verifica dei file del kit…'
if [ ! -f "$PROJECT/VERIFICA-SHA256.txt" ] || ! (cd "$PROJECT" && /usr/bin/shasum -a 256 -c VERIFICA-SHA256.txt); then
  echo 'Kit incompleto o modificato. Estrai di nuovo lo ZIP aggiornato in una nuova cartella.'
  exit 1
fi
major="$(/usr/bin/sw_vers -productVersion | cut -d. -f1)"
if [ "$major" -lt 13 ]; then echo 'Serve macOS 13 o successivo.'; exit 1; fi
if ! /usr/bin/xcrun --sdk macosx --show-sdk-path >/dev/null 2>&1; then
  echo 'macOS deve installare gli strumenti Apple. Conferma la finestra di installazione.'
  /usr/bin/xcode-select --install || true
  echo 'Quando Apple ha completato l’installazione, riapri Prepara PARLO.command.'
  exit 1
fi
ARCH="$(uname -m)"
case "$ARCH" in
  arm64) NODE_ARCH=arm64; NODE_SHA=23b25245dcfb9af7262f8ff142e9e2e0af025368117329e7a7458a51e5922f53 ;;
  x86_64) NODE_ARCH=x64; NODE_SHA=8a677b0219178efd6eb0e475457c4afb452b521a92f6e67845a73bd85727f2a8 ;;
  *) echo 'Architettura Mac non supportata.'; exit 1 ;;
esac
NODE_FOLDER="$PROJECT/.tools/node-v22.23.3-darwin-$NODE_ARCH"
NODE_ARCHIVE="$PROJECT/.tools/downloads/node-v22.23.3-darwin-$NODE_ARCH.tar.gz"
if [ ! -x "$NODE_FOLDER/bin/node" ]; then
  echo 'Preparazione degli strumenti locali: Node.js…'
  /usr/bin/curl --fail --location --retry 3 --proto '=https' --proto-redir '=https' "https://nodejs.org/dist/v22.23.3/node-v22.23.3-darwin-$NODE_ARCH.tar.gz" --output "$NODE_ARCHIVE.part"
  ACTUAL="$(/usr/bin/shasum -a 256 "$NODE_ARCHIVE.part" | cut -d' ' -f1)"
  if [ "$ACTUAL" != "$NODE_SHA" ]; then echo 'Download Node.js non valido. Riapri il kit per riprovare.'; exit 1; fi
  mv "$NODE_ARCHIVE.part" "$NODE_ARCHIVE"
  /usr/bin/tar -xzf "$NODE_ARCHIVE" -C "$PROJECT/.tools"
fi
export PATH="$NODE_FOLDER/bin:$PATH"
unset PYTHONHOME PYTHONPATH
cd "$PROJECT"
"$NODE_FOLDER/bin/node" scripts/mac-test.mjs
