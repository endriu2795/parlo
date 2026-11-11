# PARLO

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="src/assets/parlo-lockup-white.svg">
  <img src="src/assets/parlo-lockup-indigo.svg" width="200" alt="PARLO">
</picture>

[English](README.en.md)

App desktop gratuita per registrare call, trascriverle e lavorare sui contenuti
con un assistente AI. Il codice del progetto è disponibile sotto **GPLv3**.

## Funzioni principali

- Registrazione di microfono e audio del computer; importazione di audio/video.
- Trascrizione locale, oppure Groq cloud con chiave personale.
- Archivio ricercabile, note, screenshot ed esportazione Markdown.
- Chat con Claude Code, Codex o Ollama, con file e cartelle come fonti.
- Creazione di documenti, presentazioni e slideshow tramite gli strumenti della chat.
- Voce online con Edge/ElevenLabs oppure locale con Piper, Kokoro e Qwen3-TTS.
  Qwen3-TTS è opzionale e inizialmente disponibile su Windows x64.

## Installare e iniziare

1. Scarica l’installer per il tuo sistema quando disponibile. [Releases ufficiali](https://github.com/endriu2795/parlo/releases)
2. Scegli le sorgenti audio e avvia una registrazione. Per trascrivere sul
   computer prepara il modello locale; in alternativa configura Groq.
3. Per usare la chat scegli l'assistente nelle impostazioni e configura
   il tuo account oppure Ollama.

Gli installer includono Python e gli strumenti per documenti e media.
I servizi esterni opzionali possono prevedere costi e quote propri.

Il codice si può compilare per **Windows x64 e macOS 13+** su Apple Silicon e Intel.
Per il collaudo e i limiti su Mac, vedi [lo stato della build macOS](MACOS.md).

## Guide

- [Uso dell'app, assistenti, voce e dati](USO.md).
- [Compilare il codice e creare il proprio installer su Windows o Mac](SVILUPPO.md).
- [Riutilizzare un template PowerPoint in tutte le chat](USO.md#slide-post-call-con-il-tuo-template).
- [Licenze e componenti di terze parti](THIRD_PARTY_NOTICES.md).

## Dati e servizi

L'archivio viene salvato sul computer dell'utente e non è cifrato dall'app.
La trascrizione locale elabora l'audio sul computer; Groq, gli assistenti cloud,
la voce online e le operazioni web inviano i contenuti necessari ai rispettivi
servizi. Con Ollama l'inferenza è locale; gli strumenti web e il servizio voce
scelto mantengono il proprio comportamento. Dettagli in [USO.md](USO.md).

## Licenza

Il codice originale di PARLO è distribuito sotto
**GNU General Public License versione 3 (GPL-3.0-only)**: puoi usarlo,
modificarlo e redistribuirlo rispettandone le condizioni. Il testo integrale
è in [LICENSE](LICENSE). Il software viene fornito senza garanzia.

I componenti, modelli e servizi di terzi conservano le proprie licenze e
condizioni; non vengono rilicenziati dalla GPL del progetto.
Attribuzioni e riferimenti sono in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
