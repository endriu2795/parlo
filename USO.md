# Usare PARLO

## Registrare e ritrovare una call

1. Scegli microfono, audio del computer o entrambi in **Impostazioni → Registrazione**.
2. Premi **Nuova registrazione**. Puoi aggiungere note, screenshot e annotazioni.
3. Premi **Termina**. Apri la call dall’archivio per leggere il testo, cercare un passaggio o esportare in Markdown.

Puoi anche usare **Importa audio o video**. PARLO estrae l’audio e lascia il file
originale al suo posto. La ricerca dell’archivio copre titoli, tag, note e testo.
I risultati si possono restringere per data e tag.

In **Dove trascrivere**, scegli il modello locale, Groq con la tua chiave o un
endpoint di trascrizione compatibile. Il modello locale si scarica al primo
utilizzo e può mostrare un’anteprima durante la registrazione. Se
l’elaborazione si interrompe, apri la call e premi **Riprova**.

## Usare l’assistente

Scegli il fornitore in **Impostazioni → Assistente**.

- **Claude Code o Codex** — collega il tuo account.
- **Ollama** — avvia Ollama sul computer o indica un server Ollama. Scegli un
  modello già disponibile sul server. Il contesto è regolabile nelle impostazioni.
  Gli strumenti web si attivano separatamente.
- **Endpoint personalizzato** — inserisci URL base, ID del modello e chiave
  facoltativa per un server compatibile con l’API OpenAI per la chat.

Apri la chat di una call per fare domande sul testo. In **File e fonti**, scegli
quali materiali usare e digita `@` per citarli. Togli la spunta alla trascrizione
per escluderla dalle richieste successive. Se ne hai già discusso in chat, la
cronologia può ancora contenere quei passaggi. Per partire da una cronologia
vuota usa **Ricomincia chat**.

Con un assistente che supporta gli strumenti puoi chiedere documenti, fogli di
calcolo, presentazioni, PDF, quiz o slideshow. I file prodotti compaiono nella
call. L’endpoint personalizzato offre strumenti di lettura e salvataggio
testuale se li attivi.

## Slide post-call con il tuo template

1. In chat, apri **File e fonti → Aggiungi file condivisi** e scegli il template PPTX.
2. Accertati che sia spuntato, poi citalo con `@` nella richiesta.

> Crea una presentazione dalla trascrizione. Usa @Template.pptx come riferimento per lo stile e includi decisioni e prossimi passi.

## Ascoltare testo e trascrizioni

In **Impostazioni → Voce** puoi scegliere Edge o ElevenLabs per la voce online,
Piper, Kokoro o Qwen3-TTS per la voce locale, oppure un endpoint compatibile.
Per ElevenLabs usa la tua chiave. Per le voci locali scarica prima i modelli
richiesti. Qwen3-TTS è opzionale, disponibile su Windows x64 e richiede più
memoria. I formati e i tempi parola dipendono dal servizio scelto.

## Salvare e condividere

In **Impostazioni → Archivio** scegli cosa conservare e per quanto tempo.
In **Condivisione** puoi configurare una cartella o un canale Slack o Teams.
La scheda **Condivise** mostra le trascrizioni di altri autori nella cartella
e permette di cercarle per titolo o autore.

L’archivio è sul computer. Le funzioni online inviano al servizio scelto i dati
necessari, come audio per la trascrizione, fonti e messaggi per la chat, oppure
testo per la voce. Le chiavi restano nel portachiavi del sistema. I file
collegati come fonti restano nei percorsi originali.

[Compilazione e test](SVILUPPO.md) · [Licenze incluse](THIRD_PARTY_NOTICES.md)
