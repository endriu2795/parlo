"""Synthetic media only: no user files, network or paid requests."""
import base64
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import wave
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))
import speech_tools as speech
from document_tools import ffmpeg_binary


class SpeechTests(unittest.TestCase):
    def test_voice_catalog_exposes_readable_names_languages_and_gender(self):
        voices = [{'ShortName': 'it-IT-IsabellaNeural', 'FriendlyName': 'Microsoft technical name', 'Locale': 'it-IT', 'Gender': 'Female'},
                  {'ShortName': 'en-US-JennyNeural', 'Locale': 'en-US', 'Gender': 'Female'}]
        # Test the catalog mapping without opening the Windows event loop's
        # internal loopback sockets in the restricted test environment.
        with patch.dict(sys.modules, {'edge_tts': SimpleNamespace(list_voices=lambda: None)}), patch.object(speech.asyncio, 'run', return_value=voices):
            result = speech.run({'operation': 'voices', 'language': 'it'})
            self.assertEqual(result, [{'id': 'it-IT-IsabellaNeural', 'name': 'Isabella', 'locale': 'it-IT', 'gender': 'Female'}])

    def test_piper_reuses_one_cpu_model_and_preserves_real_audio_duration(self):
        with tempfile.TemporaryDirectory(prefix='parlo-piper-unit-') as tmp:
            root = Path(tmp)
            model, config = root / 'voice.onnx', root / 'voice.onnx.json'
            model.write_bytes(b'synthetic model')
            config.write_text('{}', encoding='utf-8')
            class Voice:
                def __init__(self, **kwargs): pass
                def synthesize_wav(self, text, output):
                    output.setnchannels(1); output.setsampwidth(2); output.setframerate(24000)
                    output.writeframes(b'\0\0' * 24000)
            speech._piper_model, speech._piper_path = None, None
            try:
                with patch('piper.PiperVoice', Voice), patch('piper.config.PiperConfig.from_dict', return_value=SimpleNamespace()), patch('onnxruntime.InferenceSession', return_value=object()) as sessions:
                    request = {'provider': 'piper', 'voice': 'it_IT-serena-medium', 'model': 'piper',
                               'text': 'Prova sintetica.', 'piper_model': str(model), 'piper_config': str(config), 'request_hash': 'synthetic'}
                    for index in range(2):
                        request['output'] = str(root / f'scene-{index}.mp3')
                        result = speech.generate(request)
                        self.assertAlmostEqual(result['duration_seconds'], 1., delta=.1)
                        self.assertFalse(result['word_timings_available'])
                        self.assertIsNone(result['subtitles'])
                        self.assertTrue(result['warnings'])
                    self.assertEqual(sessions.call_count, 1)
                    self.assertEqual(sessions.call_args.kwargs['providers'], ['CPUExecutionProvider'])
                    self.assertEqual(sessions.call_args.kwargs['sess_options'].intra_op_num_threads, 2)
                self.assertEqual(list(root.glob('.piper-*')), [])
                self.assertEqual(list(root.glob('.speech-*')), [])
            finally:
                speech._piper_model, speech._piper_path = None, None

    def test_failed_piper_does_not_replace_previous_files(self):
        with tempfile.TemporaryDirectory(prefix='parlo-piper-error-') as tmp:
            output = Path(tmp) / 'voice.mp3'
            output.write_bytes(b'previous audio')
            with patch.object(speech, 'piper', side_effect=ValueError('synthetic failure')):
                with self.assertRaises(ValueError):
                    speech.generate({'output': str(output), 'provider': 'piper'})
            self.assertEqual(output.read_bytes(), b'previous audio')
            self.assertEqual(list(Path(tmp).glob('.speech-*')), [])

    def test_alignment_preserves_words_and_real_provider_times(self):
        words = speech.aligned_words({'characters': list('Ciao mondo.'),
                                     'character_start_times_seconds': [i / 10 for i in range(11)],
                                     'character_end_times_seconds': [(i + 1) / 10 for i in range(11)]})
        self.assertEqual(words, [{'text': 'Ciao', 'start': 0.0, 'end': 0.4},
                                 {'text': 'mondo.', 'start': 0.5, 'end': 1.1}])
        self.assertIn('00:00:00,000 --> 00:00:01,100', speech.subtitles(words))
        self.assertIn('Ciao mondo.', speech.subtitles(words))
        with self.assertRaises(ValueError):
            speech.aligned_words({'characters': ['a'], 'character_start_times_seconds': [], 'character_end_times_seconds': []})

    def test_audio_is_decoded_and_timing_sidecars_match(self):
        with tempfile.TemporaryDirectory(prefix='parlo-speech-test-') as tmp:
            root = Path(tmp)
            wav, mp3, output = root / 'tone.wav', root / 'tone.mp3', root / 'voce à.mp3'
            with wave.open(str(wav), 'wb') as audio:
                audio.setnchannels(1)
                audio.setsampwidth(2)
                audio.setframerate(24000)
                audio.writeframes(struct.pack('<h', 0) * 24000)
            subprocess.run([str(ffmpeg_binary()), '-v', 'error', '-i', str(wav), str(mp3)], check=True)
            response = {'audio_base64': base64.b64encode(mp3.read_bytes()).decode(),
                        'alignment': {'characters': list('Ciao.'),
                                      'character_start_times_seconds': [0, .1, .2, .3, .4],
                                      'character_end_times_seconds': [.1, .2, .3, .4, .5]}}
            result = speech.generate({'output': str(output), 'provider': 'elevenlabs',
                                      'voice': 'synthetic', 'model': 'test', 'request_hash': 'test', 'response': response})
            self.assertAlmostEqual(result['duration_seconds'], 1.0, delta=.1)
            metadata = json.loads(Path(result['timings']).read_text(encoding='utf-8'))
            self.assertEqual(metadata['audio_sha256'], hashlib.sha256(output.read_bytes()).hexdigest())
            self.assertEqual(metadata['words'][0]['end'], .5)
            self.assertIn('Ciao.', Path(result['subtitles']).read_text(encoding='utf-8'))
            self.assertEqual(list(root.glob('.speech-*')), [])
            # Alignment is nullable: preserve audio without fabricating subtitles.
            response['alignment'] = None
            output_without_timing = root / 'without-timing.mp3'
            result = speech.generate({'output': str(output_without_timing), 'provider': 'elevenlabs',
                                      'voice': 'synthetic', 'model': 'test', 'request_hash': 'test', 'response': response})
            self.assertTrue(output_without_timing.is_file())
            self.assertFalse(result['word_timings_available'])
            self.assertIsNone(result['subtitles'])
            self.assertTrue(result['warnings'])
            self.assertFalse(output_without_timing.with_suffix('.srt').exists())

    def test_failed_synthesis_keeps_previous_audio(self):
        with tempfile.TemporaryDirectory(prefix='parlo-speech-test-') as tmp:
            output = Path(tmp) / 'voice.mp3'
            output.write_bytes(b'previous audio')
            with patch.object(speech, 'audio_duration', side_effect=ValueError('invalid audio')):
                with self.assertRaises(ValueError):
                    speech.generate({'output': str(output), 'provider': 'elevenlabs',
                                     'response': {'audio_base64': base64.b64encode(b'bad').decode()}})
            self.assertEqual(output.read_bytes(), b'previous audio')
            self.assertFalse(output.with_suffix('.speech.json').exists())
            self.assertEqual(list(Path(tmp).glob('.speech-*')), [])


if __name__ == '__main__':
    unittest.main()
