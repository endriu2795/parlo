# Local Piper

The `piper-tts` 1.8.0 engine is distributed under GPL-3.0-or-later and
includes eSpeak NG. The packages keep their own licenses in the private runtime.

- Engine and code: https://github.com/OHF-Voice/piper1-gpl
- eSpeak NG: https://github.com/espeak-ng/espeak-ng

The two quality levels of the Serena voice were published by bacca87/committa,
trained from scratch on the `committa/serena-synthetic-it-27h` dataset, which
is declared CC-BY-4.0 in the respective model cards:

- https://huggingface.co/rhasspy/piper-voices/blob/5041a7a49ae5b3a0f30e6837769eb03fdbb231bd/it/it_IT/serena/medium/MODEL_CARD
- https://huggingface.co/rhasspy/piper-voices/blob/5041a7a49ae5b3a0f30e6837769eb03fdbb231bd/it/it_IT/serena/high/MODEL_CARD
- https://creativecommons.org/licenses/by/4.0/

The models are downloaded only on request, verified with SHA-256, and stored
with their attribution in the local voices folder.
