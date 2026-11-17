# PARLO su Mac

[English](MACOS.en.md)

Serve macOS 13 o successivo, su Apple Silicon o Intel. PARLO registra il
microfono tramite i dispositivi audio del Mac e l’audio del computer tramite
ScreenCaptureKit. Per ora non è disponibile un installer ufficiale: l’app si
compila dai sorgenti.

## Compilare

Segui i passaggi nella [guida per compilare su Mac](DEVELOPMENT.md#macos-apple-silicon-or-intel)
(in inglese). La guida spiega come aprire la cartella nel Terminale e trovare l’app nel Finder.

## Primo avvio

1. Apri **PARLO.app** e prepara il modello in **Impostazioni → Registrazione**.
2. Alla prima registrazione concedi **Microfono** e **Registrazione schermo**
   in **Privacy e sicurezza**. Il secondo permesso serve anche per acquisire
   il solo audio del computer.
3. Riapri PARLO se macOS lo richiede. Se i permessi restano associati a una
   vecchia build, usa **Ripristina permessi e riavvia** in
   **Impostazioni → Registrazione**.

## Kit di prova

Se hai ricevuto lo ZIP di prova, estrailo e apri **Prepara PARLO.command**.
Se Finder blocca l’avvio, apri **PRIMO AVVIO MAC.html** e segui le istruzioni
del kit. Se la preparazione fallisce dopo l’avvio, controlla
`Diagnostica-PARLO-Mac.zip`. Per riaprire l’app usa **Avvia PARLO.command**.

## Stato del collaudo

La versione Mac non è ancora stata collaudata a fondo su hardware fisico.
Restano da verificare avvio, permessi, registrazione di microfono e audio del
computer, cambio di dispositivo, trascrizione locale e firma dell’installer.
Segnalazioni e contributi sono benvenuti. Per distribuire un DMG servono
Developer ID e notarizzazione.
[Firma macOS con Tauri](https://v2.tauri.app/distribute/sign/macos/)
