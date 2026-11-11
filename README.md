# PARLO

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="src/assets/parlo-lockup-white.svg">
  <img src="src/assets/parlo-lockup-indigo.svg" width="200" alt="PARLO">
</picture>

[English](README.en.md)

PARLO è un’app desktop gratuita per **registrare le call e ritrovarle nel tempo**.
Avvia la registrazione del microfono e dell’audio del computer con una
scorciatoia da tastiera configurabile, anche mentre usi un’altra app.
A fine call, la trascrizione entra nel tuo archivio
locale. Puoi cercare le parole pronunciate, aggiungere tag e filtrare per tag
o data.

## Altre funzioni

- Importa file audio e video da trascrivere.
- Trascrivi sul computer o usa Groq con la tua chiave.
- Aggiungi note e screenshot alla call ed esporta la trascrizione in Markdown.
- Usa Claude Code, Codex o Ollama per fare domande sulla call e creare
  documenti, presentazioni e slideshow a partire dalle fonti che scegli.
- Genera voce online con Edge o ElevenLabs oppure in locale con Piper, Kokoro
  e Qwen3-TTS. Qwen3-TTS è opzionale e inizialmente disponibile su Windows x64.

## Installare e iniziare

- **Windows x64.** Scarica il file `.exe`. Per ora l’installer è privo di
  firma Authenticode. Windows può mostrare un avviso o bloccarlo in base alle
  impostazioni di sicurezza. [Releases ufficiali](https://github.com/endriu2795/parlo/releases)
- **macOS 13+.** Per ora puoi compilare l’app dai sorgenti su Apple Silicon o
  Intel. [Istruzioni per macOS](SVILUPPO.md#mac-apple-silicon-o-intel)

Scegli le sorgenti audio e avvia una registrazione. Prepara il modello locale
per trascrivere sul computer oppure configura Groq. Per usare la chat, scegli
l’assistente nelle impostazioni.

Gli installer includono Python e gli strumenti per documenti e media.
I servizi esterni opzionali possono prevedere costi e quote propri.

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
