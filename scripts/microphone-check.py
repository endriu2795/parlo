"""Check weak-voice inference on public audio; never capture the user's microphone."""
from pathlib import Path
from collections import Counter
import contextlib
import importlib.util
import io
import json
import re
import tempfile
import unittest
import urllib.request
import wave

import av
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("parlo_worker", ROOT / "engine/worker.py")
worker = importlib.util.module_from_spec(spec)
import sys
sys.modules[spec.name] = worker
spec.loader.exec_module(worker)


class AudioBoundaries(unittest.TestCase):
    def test_silence_and_inaudible_input_are_not_amplified(self):
        for value in (0., .00001):
            samples = np.full(16000, value, dtype=np.float32)
            np.testing.assert_array_equal(worker.prepare_microphone_audio(samples.copy(), "microphone", {}), samples)

    def test_gain_is_bounded_and_preserves_peak_headroom(self):
        samples = np.sin(np.arange(16000, dtype=np.float32) * .1) * .001
        prepared = worker.prepare_microphone_audio(samples.copy(), "microphone", {})
        self.assertLessEqual(float(np.max(np.abs(prepared))), .012001)
        self.assertGreater(float(np.max(np.abs(prepared))), .001)
        samples[80] = .8
        self.assertLessEqual(float(np.max(np.abs(worker.prepare_microphone_audio(samples, "microphone", {})))), .950001)

    def test_disabling_and_system_source_preserve_original_samples(self):
        original = np.linspace(-.01, .01, 5000, dtype=np.float32)
        for source, settings in (("system", {}), ("microphone", {"microphone_voice_boost": False})):
            np.testing.assert_array_equal(worker.prepare_microphone_audio(original.copy(), source, settings), original)


def check_public_speech():
    from faster_whisper.vad import get_speech_timestamps, VadOptions
    source_url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/1-two-speakers-en.wav"
    settings = {"whisper_model": str(ROOT / ".tools/engine/models/whisper-turbo"),
                "live_whisper_model": str(ROOT / ".tools/engine/models/whisper-tiny"),
                "language": "en", "diarization": False}
    with tempfile.TemporaryDirectory(prefix="parlo-microphone-") as temporary:
        directory = Path(temporary)
        fixture = directory / "fixture.wav"
        urllib.request.urlretrieve(source_url, fixture)
        pcm = []
        with av.open(str(fixture)) as container:
            resampler = av.AudioResampler(format="fltp", layout="mono", rate=16000)
            for frame in container.decode(audio=0):
                pcm.extend(converted.to_ndarray().reshape(-1) for converted in resampler.resample(frame))
            pcm.extend(converted.to_ndarray().reshape(-1) for converted in resampler.resample(None))
        samples = np.concatenate(pcm).astype(np.float32)[:12 * 16000]
        records = []
        def speech_seconds(audio, source, selected):
            return round(sum(part["end"] - part["start"] for part in get_speech_timestamps(audio, VadOptions(**worker.speech_filter(source, selected)))) / 16000, 3)
        for attenuation in (0, -20, -30, -40, -50):
            quiet = samples * (10 ** (attenuation / 20))
            # Reproduce the actual PCM16 capture path, including its quantization floor.
            quiet = (np.clip(quiet, -1., 1.) * 32767).astype(np.int16).astype(np.float32) / 32768
            prepared = worker.prepare_microphone_audio(quiet.copy(), "microphone", settings)
            records.append({"attenuation_db": attenuation,
                            "before_speech_seconds": speech_seconds(quiet, "microphone", {"microphone_voice_boost": False}),
                            "after_speech_seconds": speech_seconds(prepared, "microphone", settings),
                            "gain_db": round(float(20 * np.log10(np.max(np.abs(prepared)) / np.max(np.abs(quiet)))), 2)})
        weak = records[-1]
        assert weak["after_speech_seconds"] > weak["before_speech_seconds"] + 4., records
        # Raising the sensitivity must still reject silence and ordinary white noise.
        rng = np.random.default_rng(2026)
        for noise in (np.zeros(8 * 16000, dtype=np.float32), rng.normal(0., .001, 8 * 16000).astype(np.float32)):
            prepared = worker.prepare_microphone_audio(noise, "microphone", settings)
            assert speech_seconds(prepared, "microphone", settings) == 0., "Noise was classified as speech"
        weak_samples = samples * 10 ** (-50 / 20)
        def write_microphone(audio):
            with wave.open(str(directory / "microphone.wav"), "wb") as f:
                f.setnchannels(1); f.setsampwidth(2); f.setframerate(16000)
                f.writeframes((np.clip(audio, -1., 1.) * 32767).astype("<i2").tobytes())
        def words(segments):
            return re.findall(r"\w+", " ".join(s["text"] for s in segments).lower())
        results = {"disabled": {}, "enabled": {}}
        # Real recognition via both worker paths, with and without the new setting.
        with contextlib.redirect_stdout(io.StringIO()):
            reference_settings = {**settings, "microphone_voice_boost": False}
            for phase in ("final", "preview"):
                def recognize(selected):
                    return worker.run(selected, directory) if phase == "final" else worker.live(selected, directory, {})[0]
                write_microphone(samples)
                reference = words(recognize(reference_settings))
                write_microphone(weak_samples)
                for enabled in (False, True):
                    selected = {**settings, "microphone_voice_boost": enabled}
                    recognized = words(recognize(selected))
                    entry = results["enabled" if enabled else "disabled"]
                    entry[f"{phase}_words"] = len(recognized)
                    entry[f"{phase}_reference_word_recall"] = round(sum((Counter(recognized) & Counter(reference)).values()) / max(1, len(reference)), 3)
        assert results["enabled"]["final_words"] > results["disabled"]["final_words"], results
        assert results["enabled"]["preview_words"] > results["disabled"]["preview_words"], results
        assert results["enabled"]["final_reference_word_recall"] >= .7, results
        assert results["enabled"]["preview_reference_word_recall"] >= .7, results
        # Reproduce an enabled but silent PC source whose header isn't ready yet.
        # It must not abort the microphone preview, and must recover when data arrives.
        (directory / "system.wav").write_bytes(b"")
        with contextlib.redirect_stdout(io.StringIO()):
            microphone_only, cursors = worker.live(settings, directory, {})
        assert microphone_only and all(s["source"] == "microphone" for s in microphone_only)
        assert "microphone" in cursors and "system" not in cursors
        with wave.open(str(directory / "system.wav"), "wb") as f:
            f.setnchannels(1); f.setsampwidth(2); f.setframerate(16000)
            f.writeframes((np.clip(samples, -1., 1.) * 32767).astype("<i2").tobytes())
        with contextlib.redirect_stdout(io.StringIO()):
            system_ready, recovered = worker.live(settings, directory, cursors)
        assert any(s["source"] == "system" for s in system_ready)
        assert recovered["system"] > 0.
        report = {"source": source_url, "public_audio_seconds": len(samples) / 16000,
                  "models": {"final": "whisper-turbo", "preview": "whisper-tiny"},
                  "vad_comparison": records, "recognition_at_minus_50_db": results,
                  "silence_and_noise_rejected": True,
                  "empty_pc_track_does_not_block_microphone": True,
                  "pc_preview_recovers_when_audio_arrives": True}
        (ROOT / ".tools/qa/microphone-report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(json.dumps(report, indent=2), flush=True)


if __name__ == "__main__":
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(AudioBoundaries)
    if not unittest.TextTestRunner().run(suite).wasSuccessful():
        raise SystemExit(1)
    check_public_speech()
