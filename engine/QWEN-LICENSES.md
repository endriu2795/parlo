# Optional Qwen3-TTS

PARLO offers Qwen3-TTS as an optional local speech engine. Qwen publishes the
`qwen-tts` 0.1.1 engine and Qwen3-TTS 12Hz 1.7B CustomVoice model under
Apache-2.0.

- [Source code](https://github.com/QwenLM/Qwen3-TTS)
- [Model and pinned revision](https://huggingface.co/Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice/tree/0c0e3051f131929182e2c023b9537f8b1c68adfe)
- [Official license text](QWEN-MODEL-LICENSE.txt)

## Download and dependencies

[qwen-tts-model.json](qwen-tts-model.json) records the model revision and
SHA-256 hashes. The preparation button in the settings starts the download.
PARLO saves a copy of the license and a revision reference next to the model.

Python dependencies are installed in a separate Qwen environment, with their
own license files. They are not added to the document or transcription
environments.

## Voices

The model provides nine built-in multilingual voices. Its Serena voice is
separate from Piper's Italian Serena voice.
