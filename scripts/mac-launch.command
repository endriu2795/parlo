#!/bin/bash
set -euo pipefail
PROJECT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$PROJECT/src-tauri/target/debug/bundle/macos/PARLO.app"
if [ ! -d "$APP" ]; then
  echo 'Apri prima Prepara PARLO.command e attendi il completamento.'
  read -r _
  exit 1
fi
/usr/bin/open "$APP"
