# Pubblicare una release ufficiale di PARLO

Questa guida riguarda le release del repository
[endriu2795/parlo](https://github.com/endriu2795/parlo). Per distribuire un
fork, segui la [guida agli installer](DEVELOPMENT.md#building-an-installer-for-your-fork).

## Preparare la versione

1. Lavora su un clone del repository ufficiale e accedi con un account GitHub
   abilitato a pubblicare release.
2. Conserva la chiave di firma dell’updater in `.tools/updater/parlo.key`
   oppure imposta `TAURI_SIGNING_PRIVATE_KEY` sul computer di build. La
   configurazione del repository contiene la chiave pubblica per verificare
   gli aggiornamenti.
3. Esegui `npm run release:version -- VERSIONE`, poi committa e pubblica i
   sorgenti su `main`.

## Windows x64

Installa i [prerequisiti](DEVELOPMENT.md#windows-x64), accedi con
`gh auth login` e prepara il [runtime Vulkan](DEVELOPMENT.md#windows-x64).
Dalla cartella del progetto esegui:

```sh
npm run release
```

L’installer attuale non ha una firma Authenticode. Indicalo nelle note della
release, insieme alla possibilità che Windows mostri un avviso o ne blocchi
l’avvio secondo le impostazioni di sicurezza.

## macOS

Per ora le release rimandano alla
[compilazione dai sorgenti](DEVELOPMENT.md#macos-apple-silicon-or-intel),
che non richiede un abbonamento Apple.

Quando il pacchetto ufficiale sarà pronto, configuralo con
[Developer ID e notarizzazione](https://developer.apple.com/developer-id/)
e compilalo dallo stesso commit della versione Windows:

```sh
bash scripts/pubblica-mac.command
```

## Pubblicazione e aggiornamenti

Gli script pubblicano i pacchetti in
[Releases](https://github.com/endriu2795/parlo/releases) e aggiornano il
manifest usato dall’app. Il tag identifica il commit compilato; i pacchetti
per le diverse piattaforme possono essere aggiunti alla stessa release in
momenti diversi.

Prima di distribuire, completa la raccolta delle
[licenze dei componenti inclusi](THIRD_PARTY_NOTICES.md#when-distributing-an-installer).
