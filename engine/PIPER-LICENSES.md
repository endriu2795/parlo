# Piper locale

Il motore `piper-tts` 1.8.0 è distribuito sotto GPL-3.0-or-later e include
eSpeak NG. I pacchetti mantengono le proprie licenze nel runtime privato.

- Motore e codice: https://github.com/OHF-Voice/piper1-gpl
- eSpeak NG: https://github.com/espeak-ng/espeak-ng

Le due qualità della voce Serena sono state pubblicate da bacca87/committa,
addestrate da zero sul dataset `committa/serena-synthetic-it-27h`, dichiarato
CC-BY-4.0 nelle rispettive model card:

- https://huggingface.co/rhasspy/piper-voices/blob/5041a7a49ae5b3a0f30e6837769eb03fdbb231bd/it/it_IT/serena/medium/MODEL_CARD
- https://huggingface.co/rhasspy/piper-voices/blob/5041a7a49ae5b3a0f30e6837769eb03fdbb231bd/it/it_IT/serena/high/MODEL_CARD
- https://creativecommons.org/licenses/by/4.0/

I modelli vengono scaricati soltanto su richiesta, verificati con SHA-256 e
accompagnati dall’attribuzione nel percorso locale delle voci.
