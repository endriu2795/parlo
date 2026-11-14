# PARLO su Mac

Serve macOS 13 o successivo, su Apple Silicon o Intel. La cattura usa i
dispositivi audio del Mac e ScreenCaptureKit per l’audio del computer.

## Compilare

Segui i passaggi nella [guida per compilare su Mac](SVILUPPO.md#mac-apple-silicon-o-intel).
La guida spiega come aprire la cartella nel Terminale e trovare l’app nel Finder.

## Primo avvio

1. Apri **PARLO.app** e prepara il modello in **Impostazioni → Registrazione**.
2. Alla prima registrazione concedi **Microfono** e **Registrazione schermo**
   in **Privacy e sicurezza**. Il secondo permesso serve anche per acquisire
   il solo audio del computer.
3. Riapri PARLO se macOS lo richiede. Se i permessi restano associati a una
   vecchia build, usa **Ripristina permessi e riavvia PARLO** nelle impostazioni
   di registrazione.

## Kit di prova

Se hai ricevuto lo ZIP di prova, estrailo e apri **Prepara PARLO.command**.
Se Finder blocca l’avvio, apri **PRIMO AVVIO MAC.html** e segui le istruzioni
del kit. Se la preparazione fallisce dopo l’avvio, controlla
`Diagnostica-PARLO-Mac.zip`. Per riaprire l’app usa **Avvia PARLO.command**.

## Collaudo sul Mac

Restano da verificare su Mac fisico avvio, permessi, registrazione delle due
sorgenti, cambio dispositivo, inferenza locale e firma dell’installer. Per
distribuire un DMG, configura Developer ID e notarizzazione.
[Firma macOS con Tauri](https://v2.tauri.app/distribute/sign/macos/)
