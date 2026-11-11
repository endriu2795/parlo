# Provare una pillola formativa in PARLO

In Impostazioni → Voce abilita TTS e scegli Edge, Piper oppure ElevenLabs. Edge
richiede Internet; per ElevenLabs salva la tua chiave e scegli una voce. Piper
funziona offline dopo aver scaricato la voce e restituisce durata reale della
scena, senza tempi delle singole parole o sottotitoli automatici.
Le impostazioni si applicano dalla richiesta successiva in chat.

Collega i materiali e l'eventuale PPTX del cliente alle fonti della chat.
Copia il blocco seguente in **Impostazioni → Assistente → Istruzioni di progetto**.
Queste istruzioni valgono per tutte le chat: se hai già istruzioni aziendali,
aggiungi il blocco alle tue senza cancellarle, rispettando il limite del campo.

```text
Quando ti chiedo una pillola formativa, accompagnami dal materiale al video.
Per le altre richieste continua a comportarti normalmente.

1. Parti dalle fonti abilitate e dal risultato richiesto. Chiarisci soltanto
   eventuali informazioni indispensabili: pubblico, obiettivo e durata indicativa.
   Non inventare fatti mancanti. Segnala i punti da verificare.

2. Crea copione.md nella cartella della chat. Dividilo in scene numerate, ognuna
   con: obiettivo, testo esatto della voce, testo breve a schermo, indicazioni
   visive ed eventuali animazioni. Separa ciò che va letto da titoli, note e
   istruzioni. La durata iniziale è una stima; non promettere secondi esatti.
   Fammi rivedere il copione prima della produzione completa, a meno che io
   abbia già autorizzato tutto. Posso correggerlo direttamente o in chat.

3. Se fornisco un PPTX/POTX come template, usalo come riferimento per palette,
   font, loghi, proporzioni e layout. Conserva l'originale. Per il PPTX finale
   riusa per quanto possibile i layout del file: puoi creare e modificare il
   PPTX senza un convertitore Office. Nel video riproduci lo stile con gli
   strumenti disponibili. Se ti chiedo di usare proprio le slide del PPTX nel
   video, verifica come convertirle in immagini e non sostituirle automaticamente
   con slide ridisegnate. Segnala un convertitore mancante solo quando serve
   quella conversione o una verifica visiva; non è un impedimento alla creazione
   del PPTX. Non dichiarare verifiche mai eseguite né animazioni conservate senza
   averle controllate nel video.

4. Prima del video completo, se non abbiamo già validato questo stile e questa
   voce, mostrami due o tre fotogrammi rappresentativi e un campione di 20–40
   secondi con una o due scene. Aspetta la mia approvazione dello stile prima
   di produrre tutte le scene, salvo mia autorizzazione esplicita a proseguire.
   Confronta font, composizione, spaziature e asset con il template e con gli
   esempi approvati; usare soltanto gli stessi colori non basta. Usa il TTS
   integrato parlo_tts, senza cercare chiavi nei file o installare altri TTS.
   Usa il fornitore e il modello scelti nelle impostazioni. Non cambiare piano,
   qualità, voce o fornitore autonomamente. Invia al TTS solo il testo parlato.
   Se il servizio rifiuta la lunghezza, dividi il testo senza omettere contenuti.

5. Genera un MP3 per scena e usa la durata effettiva restituita dal TTS per la
   timeline. Conserva JSON con tempi e sottotitoli SRT quando disponibili.
   Se il servizio non fornisce tempi delle parole, dichiaralo e usa la durata
   della scena: non inventare sincronizzazioni parola per parola.
   Fai finire la narrazione
   prima del cambio scena; per apparizioni sincronizzate usa i tempi delle parole.
   Mantieni la voce intera anche quando la durata finale differisce dalla stima.
   Se chiedo un tempo tassativo, proponi una revisione del testo: non tagliare
   l'audio e non accelerarlo di nascosto. Non aggiungere musica di sottofondo.

6. Scegli liberamente strumenti e tecnica per ottenere il video richiesto,
   usando il materiale fornito come riferimento. Python, MoviePy 2.x, Pillow
   e FFmpeg sono disponibili nell'app, ma non sono una scelta obbligatoria.
   Salva in .work gli script di produzione e un manifest con scene, immagini,
   audio, durate ed effetti, così possiamo modificare e rigenerare il risultato.
   Le animazioni possono essere apparizioni di testi, zoom lenti e dissolvenze:
   devono essere realmente presenti nel video, non soltanto descritte a parole.
   Salva gli output finali nella cartella della chat e collegali nella risposta.

7. Alle revisioni aggiorna il copione e le scene coinvolte. Riusa gli MP3
   invariati; rigenera la voce se cambia il parlato o se chiedo un'altra
   interpretazione. Per quest'ultima usa force_regenerate. Una modifica solo
   grafica non richiede nuova voce. Mantieni script e manifest coerenti con i file.

8. Prima della consegna controlla apertura dei file, dimensioni, durata video,
   presenza dell'audio e corrispondenza tra timeline e narrazione. Esamina
   fotogrammi rappresentativi con gli strumenti disponibili. Dichiara quali
   controlli hai eseguito e ciò che resta da verificare ascoltando o guardando.
   Quando sto facendo un test, scrivi verifica.md con tempi di produzione,
   durata finale, scene rigenerate e problemi. Distingui tempi misurati e stime;
   non inventare costi o crediti ElevenLabs se non sono disponibili.
```

## Prima richiesta di prova

> Dai materiali allegati prepara una pillola di circa 2 minuti per [pubblico]
> su [argomento], usando il PPTX [nome] come template. Prima fammi rivedere
> copione.md, poi realizziamo un campione con voce e una piccola animazione.
> Voglio verificare il flusso prima del video completo. Niente musica.

## Revisioni utili per verificare il flusso

- «Nella scena 2 riscrivi questa frase e rigenera soltanto la sua voce e il video.»
- «Ingrandisci il titolo della scena 1 mantenendo gli audio già approvati.»
- «Rifai la voce della scena 2 con lo stesso testo ma un tono più naturale.»
- «Ora genera il video completo e consegna anche il copione aggiornato e verifica.md.»

Il TTS fornisce i tempi; le animazioni e il montaggio vengono realizzati dallo
script prodotto dall'assistente. Questo flusso non richiede lo studio di Ludo.
La fedeltà del video a un PPTX complesso va provata sul template concreto:
la disponibilità del TTS non aggiunge un motore di rendering PowerPoint.
