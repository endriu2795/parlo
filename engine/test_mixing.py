import tempfile
import unittest
import wave
from unittest.mock import patch
from pathlib import Path
import numpy as np
from worker import mixed_audio, remove_echo, frame_levels, SourceActivity


def write(path, samples):
    with wave.open(str(path), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(16000)
        output.writeframes((np.clip(samples, -1, 1) * 32767).astype("<i2").tobytes())

class MixingTests(unittest.TestCase):
    def test_single_microphone_skips_echo_and_channel_analysis(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            write(path / 'microphone.wav', np.full(16000, .05, dtype=np.float32))
            with patch('worker.remove_echo') as echo, patch('worker.frame_levels') as levels, patch('worker.speech_activity') as speech:
                samples, activity = mixed_audio(path, {})
            self.assertEqual(len(samples), 16000)
            self.assertEqual(activity.speaker(0, .5), 'Tu')
            echo.assert_not_called()
            levels.assert_not_called()
            speech.assert_not_called()

    def test_local_speech_is_not_relabelled_by_louder_pc_background(self):
        # Constant PC sound used to outweigh quiet local words. Even continuous
        # local speech used to be mistaken for noise by the percentile floor.
        microphone = np.full(32000, .02, dtype=np.float32)
        system = np.full(32000, .12, dtype=np.float32)
        with patch('faster_whisper.vad.get_speech_timestamps', side_effect=[[{'start':0,'end':32000}], []]):
            activity = SourceActivity(microphone, system, True, True)
        self.assertEqual(activity.speaker(.3, .55), 'Tu')

    def test_real_short_remote_turn_is_kept_between_local_turns(self):
        microphone = np.full(48000, .03, dtype=np.float32)
        system = np.full(48000, .02, dtype=np.float32)
        with patch('faster_whisper.vad.get_speech_timestamps', side_effect=[
            [{'start':0,'end':16000}, {'start':20800,'end':48000}],
            [{'start':16000,'end':20800}],
        ]):
            activity = SourceActivity(microphone, system, True, True)
        self.assertEqual(activity.speaker(.8, .98), 'Tu')
        self.assertEqual(activity.speaker(1.03, 1.25), 'Speaker')
        self.assertEqual(activity.speaker(1.35, 1.6), 'Tu')

    def test_delayed_echo_is_removed_and_independent_voice_is_preserved(self):
        rng = np.random.default_rng(42)
        remote = (rng.standard_normal(48000) * .06).astype(np.float32)
        delay = 1920
        echo = np.pad(remote * .3, (delay, 0))[:len(remote)]
        cleaned = remove_echo(echo.copy(), remote)
        self.assertLess(np.mean(cleaned**2), np.mean(echo**2) * .001)
        local = (rng.standard_normal(48000) * .04).astype(np.float32)
        np.testing.assert_allclose(remove_echo(local.copy(), remote), local, atol=1e-7)
        # Concurrent local voice must remain after subtracting a louder echo.
        combined = echo + local * .15
        residual = remove_echo(combined.copy(), remote)
        self.assertGreater(np.mean(residual**2), np.mean((local * .15)**2) * .8)

    def test_one_waveform_labels_remote_echo_as_speaker_and_local_turn_as_tu(self):
        rng = np.random.default_rng(123)
        remote = np.zeros(64000, dtype=np.float32)
        remote[32000:] = rng.standard_normal(32000) * .08
        local = np.zeros(64000, dtype=np.float32)
        local[:24000] = rng.standard_normal(24000) * .025
        echo = np.pad(remote * .25, (1600, 0))[:64000]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            write(path / "microphone.wav", local + echo)
            write(path / "system.wav", remote)
            mixed, activity = mixed_audio(path, {"microphone_voice_boost": True})
        self.assertEqual(len(mixed), 64000)
        self.assertEqual(activity.speaker(.3, 1.), "Tu")
        self.assertEqual(activity.speaker(2.3, 3.), "Speaker")
        self.assertLessEqual(float(np.max(np.abs(mixed))), .951)

    def test_single_source_and_unequal_track_lengths(self):
        for source, label in (("microphone", "Tu"), ("system", "Speaker")):
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory)
                write(path / f"{source}.wav", np.ones(12345, dtype=np.float32) * .05)
                mixed, activity = mixed_audio(path, {})
                self.assertEqual(len(mixed), 12345)
                self.assertEqual(activity.speaker(0, .5), label)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            write(path / "microphone.wav", np.ones(20000, dtype=np.float32) * .05)
            write(path / "system.wav", np.zeros(10000, dtype=np.float32))
            mixed, _ = mixed_audio(path, {})
            self.assertEqual(len(mixed), 20000)
        np.testing.assert_allclose(frame_levels(np.ones(333, dtype=np.float32) * .1), [.1, .1], rtol=1e-6)

if __name__ == "__main__":
    unittest.main()
