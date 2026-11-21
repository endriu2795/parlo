# Local Piper

PARLO uses `piper-tts` 1.8.0 for local speech synthesis. The engine is licensed
under GPL-3.0-or-later and includes eSpeak NG. License files for the packages
are kept in the app's runtime.

- [Piper engine and source code](https://github.com/OHF-Voice/piper1-gpl)
- [eSpeak NG source code](https://github.com/espeak-ng/espeak-ng)

## Serena voice

The medium- and high-quality Serena models were published by bacca87/committa
and trained from scratch on `committa/serena-synthetic-it-27h`. Their model
cards declare the dataset as CC-BY-4.0:

- [Medium model card](https://huggingface.co/rhasspy/piper-voices/blob/5041a7a49ae5b3a0f30e6837769eb03fdbb231bd/it/it_IT/serena/medium/MODEL_CARD)
- [High model card](https://huggingface.co/rhasspy/piper-voices/blob/5041a7a49ae5b3a0f30e6837769eb03fdbb231bd/it/it_IT/serena/high/MODEL_CARD)
- [CC-BY-4.0 license](https://creativecommons.org/licenses/by/4.0/)

PARLO downloads the models on request, verifies their SHA-256 hashes, and
stores them with their attribution in the local voices folder.
