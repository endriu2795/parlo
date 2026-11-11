# Pubblicare una release ufficiale di PARLO

Usa un clone di `https://github.com/endriu2795/parlo.git` e accedi con un
account GitHub che può pubblicare release. Conserva la chiave di firma
dell’updater in `.tools/updater/parlo.key` o imposta
`TAURI_SIGNING_PRIVATE_KEY` sul computer di build. La configurazione nel
repository contiene la chiave pubblica usata per verificare gli aggiornamenti.

Per ogni versione, usa `npm run release:version -- VERSIONE`, poi committa
e pubblica i sorgenti su `main`.

- **Windows x64**. Installa i [prerequisiti](SVILUPPO.md#windows-x64), accedi
  con `gh auth login` e prepara il
  [runtime Vulkan](SVILUPPO.md#windows-x64). Dalla repo esegui `npm run release`.
  Per ora le note della release indicano che l’installer è privo di firma
  Authenticode e che Windows può mostrare un avviso o bloccarlo.
- **macOS**. Per ora rimanda alla [build da sorgente](SVILUPPO.md#mac-apple-silicon-o-intel),
  che si può fare senza un abbonamento Apple. Quando sarà pronto il pacchetto
  ufficiale, usa il [Developer ID e la notarizzazione](https://developer.apple.com/developer-id/)
  e compila dallo stesso commit della release Windows con
  `bash scripts/pubblica-mac.command`.

Gli script pubblicano in [Releases](https://github.com/endriu2795/parlo/releases)
e aggiornano il manifest per l’app. Il tag della release punta al commit
compilato. Le piattaforme possono essere aggiunte alla stessa versione in
momenti diversi. Prima di distribuire gli installer, verifica le
[licenze dei componenti inclusi](THIRD_PARTY_NOTICES.md#quando-si-distribuisce-un-installer).
