# Licenze e componenti di terze parti

Il codice originale di PARLO usa **GPL-3.0-only**;
il testo integrale è in [LICENSE](LICENSE). Questa scelta non modifica le
licenze dei componenti esterni, dei modelli scaricabili o dei servizi collegati.
Copyright e attribuzioni originali devono essere conservati.

- **Inter** e **Bricolage Grotesque** usano la SIL Open Font License 1.1.
  [Licenza Inter](public/fonts/LICENSE-inter.txt) e
  [licenza Bricolage Grotesque](public/fonts/LICENSE-bricolage.txt).

## Voce locale

- **Piper (`piper-tts` 1.8.0)**: GPL-3.0-or-later, progetto
  [OHF-Voice/piper1-gpl](https://github.com/OHF-Voice/piper1-gpl).
  Il testo GPLv3 è anche in [LICENSE](LICENSE). Piper include eSpeak NG.
  Le voci Serena sono scaricate separatamente e dichiarate CC-BY-4.0:
  autori, model card e revisione in [PIPER-LICENSES.md](engine/PIPER-LICENSES.md).
- **Kokoro ONNX 0.6.1**: MIT, copyright 2025 github.com/thewh1teagle;
  [testo della licenza](licenses/kokoro-onnx.txt) e
  [sorgenti](https://github.com/thewh1teagle/kokoro-onnx).
  La sua licenza non copre automaticamente i modelli o le dipendenze.
- **Phonemizer 3.4.0**, usato da Kokoro: GPL-3.0-or-later,
  copyright degli autori del progetto, fra cui Mathieu Bernard e Hadrien Titeux.
  [Sorgenti e copyright](https://github.com/bootphon/phonemizer),
  [testo GPLv3](LICENSE).
- **eSpeak NG**, incluso attraverso Piper e `espeakng-loader`:
  [licenza GPLv3 e sorgenti](https://github.com/espeak-ng/espeak-ng).
  Il wrapper `espeakng-loader` 0.2.4 ha licenza MIT:
  [testo](licenses/espeakng-loader.txt),
  [sorgenti](https://github.com/thewh1teagle/espeakng-loader).
  La licenza del wrapper non sostituisce quella della libreria inclusa.
- **Qwen3-TTS**: ambiente opzionale scaricato separatamente. Codice, modello
  Apache-2.0 e revisione sono descritti in [QWEN-LICENSES.md](engine/QWEN-LICENSES.md);
  [testo Apache-2.0](engine/QWEN-MODEL-LICENSE.txt).

I metadati delle versioni Python di Piper, Phonemizer, Kokoro, espeakng-loader
e imageio-ffmpeg, con gli indirizzi e gli hash dei sorgenti pubblicati da PyPI,
sono raccolti in [licenses/python-sources.json](licenses/python-sources.json).
Per espeakng-loader 0.2.4 PyPI non pubblica un archivio sorgente: il collegamento
fra wheel, sorgenti del wrapper ed eSpeak incorporato va completato prima
della distribuzione binaria. Non sono stati scaricati modelli o dati degli utenti.

## Trascrizione, documenti e media

- **whisper.cpp 1.8.3**: MIT, copyright 2023-2024 The ggml authors;
  [licenza](licenses/whisper-cpp.txt),
  [sorgenti della versione usata](https://github.com/ggml-org/whisper.cpp/tree/v1.8.3).
  Gli script nativi conservano anche `LICENSE-whisper.txt` nel runtime.
- **faster-whisper 1.2.1**: MIT, copyright 2023 SYSTRAN;
  [licenza](licenses/faster-whisper.txt),
  [sorgenti](https://github.com/SYSTRAN/faster-whisper/tree/v1.2.1).
- **PyAV 16.0.1 (Windows) / 15.1.0 (Mac)**: BSD-3-Clause, copyright
  degli autori originali; [licenza](licenses/pyav.txt),
  [sorgenti](https://github.com/PyAV-Org/PyAV).
  Le librerie FFmpeg contenute nei wheel mantengono le proprie condizioni.
- **FFmpeg**, distribuito attraverso `imageio-ffmpeg` e le librerie di PyAV:
  LGPL o GPL a seconda della configurazione effettiva dei binari.
  [Condizioni ufficiali](https://ffmpeg.org/legal.html).
  Il wrapper `imageio-ffmpeg` 0.6.0 è BSD-2-Clause:
  [licenza](licenses/imageio-ffmpeg.txt),
  [sorgenti](https://github.com/imageio/imageio-ffmpeg).
  La licenza del wrapper non basta per descrivere il binario FFmpeg.
- **CPython**, versione e distribuzione fissate in
  [python-runtime.json](engine/python-runtime.json):
  [licenza Python](https://docs.python.org/3/license.html),
  [distribuzione python-build-standalone](https://github.com/astral-sh/python-build-standalone).
  Conservare anche le licenze delle librerie native del runtime.

L'elenco dei pacchetti Python e delle versioni è nei lockfile
[trascrizione](engine/requirements-lock.txt) e
[documenti/media](engine/documents-requirements-lock.txt).
Il runtime documenti/media include Pillow 11.3.0 con la correzione per
[CVE-2026-112399](https://github.com/python-pillow/Pillow/security/advisories/GHSA-5xm2-6676-qcp9),
applicata durante la [preparazione del runtime](scripts/prepare-runtime.py).
L'ambiente opzionale Qwen usa [qwen-tts-requirements.txt](engine/qwen-tts-requirements.txt).
I pacchetti conservano i propri file di licenza e metadati `dist-info`;
non rimuoverli quando si prepara un runtime.

## Interfaccia e backend

Le dipendenze e versioni complete, incluse quelle transitive, sono fissate in
[package-lock.json](package-lock.json) e [Cargo.lock](src-tauri/Cargo.lock).
Fra i componenti principali:

- React/React DOM, react-markdown e remark-gfm: MIT.
- Lucide: ISC, con eventuali attribuzioni delle icone derivate.
- Tauri e i plugin: MIT oppure Apache-2.0 secondo i rispettivi pacchetti.
- SQLite: pubblico dominio; il binding Rust e le altre crate mantengono
  le proprie licenze.

Questo documento è un indice dei componenti principali, non l'inventario
completo delle licenze di tutti i binari. Per una distribuzione vanno raccolti
anche i testi e gli eventuali NOTICE delle dipendenze transitive effettivamente
incluse; un lockfile da solo non li sostituisce.

## Programmi e servizi esterni

Claude Code resta proprietario e soggetto alle
[condizioni Anthropic](https://code.claude.com/docs/en/legal-and-compliance).
Il progetto usa la CLI ufficiale, installata separatamente o scaricata dal suo
distributore; non la rilicenzia sotto GPL. Anche Ollama la usa per gli strumenti.
Il pacchetto npm Agent SDK è una dipendenza di sviluppo con condizioni proprie.

Codex, Ollama e i rispettivi modelli mantengono le proprie licenze. Le revisioni
dei runtime preparati dall'app sono nei manifest `engine/*-runtime.json`.
Groq, Edge ed ElevenLabs sono servizi esterni: l'accesso segue le condizioni
e gli eventuali costi dell'account dell'utente.

## Quando si distribuisce un installer

La scelta del repository pubblico e del canale aggiornamenti è separata da
questo documento. Prima di distribuire un nuovo pacchetto:

1. Accompagnarlo con `LICENSE`, questo indice e i testi di licenza/NOTICE
   delle dipendenze incluse.
2. Rendere accessibili accanto al download i sorgenti esatti del codice GPL
   distribuito, le eventuali modifiche e gli script necessari a ricompilarlo.
   Un link generico al ramo principale di un progetto non identifica quei sorgenti.
3. Associare i componenti nativi incorporati alla loro versione e configurazione:
   in particolare eSpeak NG, FFmpeg, runtime Python e relative librerie.
   I sorgenti Python in `licenses/python-sources.json` non sostituiscono quelli
   delle librerie native già compilate nei wheel.
4. Conservare licenze e attribuzioni dei modelli effettivamente distribuiti;
   i modelli scaricati a parte mantengono le proprie condizioni.

Questa preparazione riguarda chi distribuisce i pacchetti. Chi usa l'app
non deve registrarsi per leggere le licenze o compiere altre operazioni.
La raccolta definitiva per gli installer è da completare nella fase di release;
non sono stati verificati o generati pacchetti binari con questa modifica.
