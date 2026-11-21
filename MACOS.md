# PARLO su Mac

[English](MACOS.en.md)

PARLO funziona su macOS 13 o successivo, con Apple Silicon o Intel. La
versione Mac si compila dai sorgenti; l’installer ufficiale non è ancora
disponibile.

## Registrazione audio e schermo

Puoi registrare il microfono e l’audio del computer. Il microfono segue il
dispositivo di ingresso predefinito di macOS, anche se lo cambi durante la
call. L’audio del computer viene acquisito tramite ScreenCaptureKit, senza
selezionare un’uscita audio da PARLO.

Puoi aggiungere screenshot e annotazioni. La registrazione video dello
schermo o di un’area è attualmente disponibile solo su Windows.

## Compilare

La [guida alla compilazione per Mac](DEVELOPMENT.md#macos-apple-silicon-or-intel)
(in inglese) contiene i comandi per preparare le dipendenze, compilare e
aprire **PARLO.app**.

## Primo avvio

1. Apri **PARLO.app**. Se usi la trascrizione locale, prepara il modello in
   **Impostazioni → Registrazione**.
2. Alla prima registrazione concedi **Microfono** e **Registrazione schermo**
   in **Privacy e sicurezza**. PARLO acquisisce l’audio del computer tramite
   ScreenCaptureKit, che richiede il permesso di registrazione schermo anche
   quando registri solo l’audio.
3. Riapri PARLO se macOS lo richiede. Se i permessi restano associati a una
   vecchia build, usa **Ripristina permessi e riavvia** in
   **Impostazioni → Registrazione**.

## Kit di prova

Se usi il kit ZIP di prova:

1. Estrai lo ZIP e apri **Prepara PARLO.command**.
2. Se Finder blocca l’avvio, segui le istruzioni in **PRIMO AVVIO MAC.html**.
3. Se la preparazione fallisce, consulta `Diagnostica-PARLO-Mac.zip`.

Per gli avvii successivi usa **Avvia PARLO.command**.

## Stato del collaudo

Il collaudo su Mac fisici è ancora da completare. Le verifiche previste
coprono avvio, permessi, registrazione di microfono e audio del computer,
cambio di dispositivo, trascrizione locale e firma dell’installer.

Per il pacchetto ufficiale sono previsti Developer ID e notarizzazione.
I passaggi sono nella [guida alla firma macOS di Tauri](https://v2.tauri.app/distribute/sign/macos/).
