# Pubblicare una release ufficiale di PARLO

Usa un clone di `https://github.com/endriu2795/parlo.git` e accedi con un
account GitHub che può pubblicare release. Conserva la chiave di firma
dell’updater in `.tools/updater/parlo.key` o imposta
`TAURI_SIGNING_PRIVATE_KEY` sul computer di build. La configurazione nel
repository contiene la chiave pubblica usata per verificare gli aggiornamenti.

Per ogni versione, usa `npm run release:version -- VERSIONE`, poi committa
e pubblica i sorgenti su `main`. Compila dallo stesso commit su Windows e Mac.

- **Windows x64**. Installa i [prerequisiti](SVILUPPO.md#windows-x64), accedi
  con `gh auth login` e prepara il
  [runtime Vulkan](SVILUPPO.md#windows-x64). Dalla repo esegui `npm run release`.
- **macOS**. Dalla repo esegui `bash scripts/pubblica-mac.command`.
  Lo script aggiorna il clone da `main`, prepara gli strumenti e compila
  per l’architettura del Mac in uso.

Entrambi pubblicano in [Releases](https://github.com/endriu2795/parlo/releases)
e aggiornano il manifest per l’app. Il tag della release punta al commit
compilato. Le piattaforme possono essere aggiunte alla stessa versione in
momenti diversi. Prima di distribuire gli installer, verifica le
[licenze dei componenti inclusi](THIRD_PARTY_NOTICES.md#quando-si-distribuisce-un-installer).
