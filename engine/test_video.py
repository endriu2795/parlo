"""Synthetic frames and tones only: never acquire a screen or microphone."""
import tempfile
import unittest
import wave
import io
import json
import base64
import threading
import ctypes
import subprocess
import sys
import time
import textwrap
from pathlib import Path
from fractions import Fraction
from unittest.mock import patch
import av
import numpy as np
from video import video_stream, mux, capture, consume_control, DrawingLayer, Desktop as ScreenDesktop, RecoveringDesktop, ScreenUnavailable


class VideoTests(unittest.TestCase):
    def test_bad_annotation_does_not_stop_control_or_capture(self):
        drawing = DrawingLayer(dict(x=0, y=0, width=64, height=48), 64, 48)
        state = {'invalid_control_messages': 0, 'stop_reason': None}
        stopped = threading.Event()
        def messages():
            yield '{broken json'
            self.assertFalse(stopped.is_set())
            yield json.dumps({'at': 'bad', 'png': None})
            self.assertFalse(stopped.is_set())
            yield json.dumps({'at': 1, 'png': None})
            self.assertFalse(stopped.is_set())
            yield 'stop'
        consume_control(messages(), drawing, stopped, state)
        self.assertEqual(state['invalid_control_messages'], 2)
        self.assertEqual(state['stop_reason'], 'requested')
        self.assertTrue(stopped.is_set())
        self.assertEqual(len(drawing.pending), 1)

    def test_control_eof_and_encoder_failure_have_distinct_diagnostics(self):
        drawing = DrawingLayer(dict(x=0, y=0, width=64, height=48), 64, 48)
        state = {'invalid_control_messages': 0, 'stop_reason': None}
        consume_control([], drawing, threading.Event(), state)
        self.assertEqual(state['stop_reason'], 'control_eof')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch('av.open', side_effect=PermissionError('synthetic write failure')):
                with self.assertRaisesRegex(PermissionError, 'synthetic write failure'):
                    capture(root, dict(x=0, y=0, width=64, height=48))
            saved = json.loads((root / 'video-diagnostics.json').read_text(encoding='utf-8'))
            self.assertEqual(saved['failure']['stage'], 'open_video_file')
            self.assertEqual(saved['failure']['type'], 'PermissionError')
            self.assertEqual(saved['frames'], 0)

    def test_temporary_display_failure_reacquires_and_reports_gap(self):
        class FakeDesktop:
            def __init__(self, fail): self.fail, self.closed = fail, False
            def read(self):
                if self.fail: raise ScreenUnavailable('schermo non disponibile')
                return 'frame'
            def close(self): self.closed = True
        first, second = FakeDesktop(True), FakeDesktop(False)
        instances = iter([first, second])
        class Stop:
            def wait(self, delay): return False
        gaps = []
        desktop = RecoveringDesktop({}, Stop(), lambda start, end, failure: gaps.append((start, end, failure)), factory=lambda **_: next(instances))
        with patch('video.time.monotonic', side_effect=[0, 0, .5]):
            self.assertEqual(desktop.read(), 'frame')
        self.assertTrue(first.closed)
        self.assertEqual(gaps, [(0, .5, 'schermo non disponibile')])
        desktop.close()
        self.assertTrue(second.closed)

    def test_display_failure_gives_up_and_stop_interrupts_recovery(self):
        class BrokenDesktop:
            def read(self): raise ScreenUnavailable('schermo non disponibile')
            def close(self): pass
        class Stop:
            def __init__(self, stopped): self.stopped = stopped
            def wait(self, delay): return self.stopped
        with patch('video.time.monotonic', side_effect=[0, 0, 6]):
            desktop = RecoveringDesktop({}, Stop(False), lambda *_: None, factory=lambda **_: BrokenDesktop())
            with self.assertRaisesRegex(ScreenUnavailable, '5 secondi'):
                desktop.read()
        with patch('video.time.monotonic', side_effect=[0, 0]):
            desktop = RecoveringDesktop({}, Stop(True), lambda *_: self.fail('unexpected recovery'), factory=lambda **_: BrokenDesktop())
            self.assertIsNone(desktop.read())

    def test_desktop_reads_completed_copies_when_visible_window_changes(self):
        pixels = (ctypes.c_uint8 * (4 * 4 * 4))()
        calls = []
        class BatchedGdi:
            value = 0
            def BitBlt(self, *args):
                self.value += 50
                calls.append('copy')
                return True
            def GdiFlush(self):
                calls.append('flush')
                ctypes.memset(ctypes.addressof(pixels), self.value, ctypes.sizeof(pixels))
                return True
        # Build only a memory fixture; never create a real desktop DC.
        desktop = ScreenDesktop.__new__(ScreenDesktop)
        desktop.gdi = BatchedGdi()
        class FakeUser:
            def GetDC(self, _): calls.append('get'); return 123
            def ReleaseDC(self, _, dc): calls.append('release'); return 1
        desktop.user = FakeUser()
        desktop.width = desktop.height = 4
        desktop.memory = desktop.dc = 0
        desktop.x, desktop.y = -4, 20
        desktop.pixels = ctypes.c_void_p(ctypes.addressof(pixels))
        np.testing.assert_array_equal(desktop.read().copy(), 50)
        np.testing.assert_array_equal(desktop.read().copy(), 100)
        self.assertEqual(calls, ['get', 'copy', 'flush', 'release', 'get', 'copy', 'flush', 'release'])
        with patch.object(desktop.gdi, 'GdiFlush', return_value=False):
            with self.assertRaisesRegex(RuntimeError, 'aggiornamento dello schermo'):
                desktop.read()
        self.assertEqual(calls[-1], 'release')
        with patch.object(desktop.gdi, 'BitBlt', return_value=False):
            with self.assertRaisesRegex(ScreenUnavailable, 'copia dello schermo'):
                desktop.read()
        self.assertEqual(calls[-1], 'release')
        with patch.object(desktop.user, 'GetDC', return_value=0):
            with self.assertRaisesRegex(ScreenUnavailable, 'accesso allo schermo'):
                desktop.read()

    @staticmethod
    def annotation(at, end):
        image = np.zeros((48, 64, 4), dtype=np.uint8)
        image[12:36, 8:end] = [255, 0, 0, 255]
        buffer = io.BytesIO()
        with av.open(buffer, 'w', format='image2pipe') as output:
            stream = output.add_stream('png', rate=1)
            stream.width, stream.height, stream.pix_fmt = 64, 48, 'rgba'
            for packet in stream.encode(av.VideoFrame.from_ndarray(image, format='rgba')):
                output.mux(packet)
            for packet in stream.encode():
                output.mux(packet)
        return dict(at=at, x=0, y=0, width=64, height=48, window=42,
                    png=base64.b64encode(buffer.getvalue()).decode('ascii'))

    def test_encoded_video_shows_drawing_progress_and_then_clean_screen(self):
        region = dict(x=0, y=0, width=64, height=48)
        drawing = DrawingLayer(region, 64, 48, visible=lambda _: True)
        # Queue even future changes before encoding: the final drawing must not
        # appear on earlier frames and hiding must restore the clean desktop.
        for update in (self.annotation(.2, 24), self.annotation(.4, 48), dict(at=.6, png=None), self.annotation(.8, 24), self.annotation(1., 8)):
            drawing.update(update)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'synthetic.mp4'
            with av.open(str(path), 'w') as output:
                stream = video_stream(output, 64, 48)
                for ms in (0, 200, 400, 600, 800, 1000):
                    desktop = np.full((48, 64, 4), 90, dtype=np.uint8)
                    frame = av.VideoFrame.from_ndarray(drawing.apply(desktop, ms / 1000), format='bgra')
                    frame.pts, frame.time_base = ms, Fraction(1, 1000)
                    for packet in stream.encode(frame):
                        output.mux(packet)
                for packet in stream.encode():
                    output.mux(packet)
            with av.open(str(path)) as source:
                frames = [frame.to_ndarray(format='rgb24') for frame in source.decode(video=0)]
            self.assertEqual(len(frames), 6)
            for index in (0, 3, 5):
                self.assertLess(np.abs(frames[index][:, :, :3].astype(int) - 90).max(), 12)
            for index in (1, 2, 4):
                self.assertGreater(frames[index][24, 16, 0], 220)
            self.assertLess(frames[1][24, 40, 0], 110)
            self.assertGreater(frames[2][24, 40, 0], 220)
            self.assertLess(frames[4][24, 40, 0], 110)

    def test_annotations_are_composited_with_crop_dpi_and_visibility(self):
        image = np.zeros((8, 16, 4), dtype=np.uint8)
        image[2:4, 4:8] = [255, 0, 0, 255]
        buffer = io.BytesIO()
        with av.open(buffer, 'w', format='image2pipe') as output:
            stream = output.add_stream('png', rate=1)
            stream.width, stream.height, stream.pix_fmt = 16, 8, 'rgba'
            for packet in stream.encode(av.VideoFrame.from_ndarray(image, format='rgba')):
                output.mux(packet)
            for packet in stream.encode():
                output.mux(packet)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = {'x': -100, 'y': -50, 'width': 32, 'height': 16, 'at': 0, 'window': 42, 'png': base64.b64encode(buffer.getvalue()).decode('ascii')}
            visible = [True]
            drawing = DrawingLayer({'x': -92, 'y': -46, 'width': 16, 'height': 8}, 8, 4, visible=lambda window: visible[0] and window == 42)
            drawing.update(data)
            pixels = np.zeros((4, 8, 4), dtype=np.uint8)
            result = drawing.apply(pixels.copy(), 0)
            np.testing.assert_array_equal(result[:2, :4, 2], 255)
            np.testing.assert_array_equal(result[2:, :, :3], 0)
            visible[0] = False
            np.testing.assert_array_equal(drawing.apply(pixels.copy(), 0), pixels)
            drawing.update({'at': 0, 'png': None})
            np.testing.assert_array_equal(drawing.apply(pixels.copy(), 0), pixels)

    def test_capture_handshake_and_immediate_stop_without_accessing_screen(self):
        class SyntheticDesktop:
            def __init__(self, **region):
                self.closed = False
            def read(self):
                return np.full((240, 320, 4), 150, dtype=np.uint8)
            def close(self):
                self.closed = True
        desktop = SyntheticDesktop()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch('video.Desktop', return_value=desktop), patch('video.sys.stdin', io.StringIO('{"offset":0}\nstop\n')), patch('video.sys.stdout', new_callable=io.StringIO) as status:
                capture(root, {'x': 0, 'y': 0, 'width': 320, 'height': 240})
            self.assertEqual(status.getvalue().strip(), 'ready')
            self.assertTrue(desktop.closed)
            with av.open(str(root / 'capture.video.mp4')) as result:
                self.assertGreaterEqual(len(list(result.decode(video=0))), 1)

    def test_capture_receives_live_annotations_until_explicit_stop(self):
        ready = threading.Event()
        update = self.annotation(0, 24)
        class Control:
            def readline(self):
                return '{"offset":0}\n'
            def __iter__(self):
                yield json.dumps(update)
                # A later clear must not erase a drawing on an earlier frame.
                yield json.dumps(dict(at=60, png=None))
                ready.set()
                yield 'stop'
        class Desktop:
            def read(self):
                if not ready.wait(2):
                    raise RuntimeError('Control stream not consumed')
                return np.full((48, 64, 4), 90, dtype=np.uint8)
            def close(self):
                pass
        region = dict(x=0, y=0, width=64, height=48)
        drawing = DrawingLayer(region, 64, 48, visible=lambda _: True)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch('video.Desktop', return_value=Desktop()), patch('video.DrawingLayer', return_value=drawing), patch('video.sys.stdin', Control()), patch('video.sys.stdout', new_callable=io.StringIO):
                capture(root, region)
            with av.open(str(root / 'capture.video.mp4')) as result:
                first = next(result.decode(video=0)).to_ndarray(format='rgb24')
            self.assertGreater(first[24, 16, 0], 220)
            self.assertLess(first[24, 40, 0], 110)

    def test_real_control_pipe_keeps_recording_after_initial_clear(self):
        self.assert_pipe_capture(warmed=False)

    def test_warmed_process_waits_for_selection_then_records_on_the_same_pipe(self):
        self.assert_pipe_capture(warmed=True)

    def assert_pipe_capture(self, warmed):
        # Exercise the actual stdin pipe and fragmented MP4, including the
        # drawing-reset message sent at startup. Never access the real desktop.
        script = textwrap.dedent('''\
            import sys, time
            from pathlib import Path
            sys.path.insert(0, sys.argv[1])
            import video
            class Desktop:
                def __init__(self, **region): self.frames = 0
                def read(self):
                    # Keep the same cold-import order as the real Desktop.
                    # Let the control thread enter its blocking stdin read.
                    if self.frames == 0: time.sleep(.05)
                    import numpy as np
                    self.frames += 1
                    return np.full((1080, 1920, 4), (self.frames * 23) % 250, dtype=np.uint8)
                def close(self): pass
            video.Desktop = Desktop
            if sys.argv[3] == 'warm':
                sys.argv = ['video.py', '--warmup']
                video.main()
            else:
                video.capture(Path(sys.argv[2]), dict(x=0, y=0, width=1920, height=1080))
        ''')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            child = subprocess.Popen([sys.executable, '-I', '-X', 'utf8', '-u', '-c', script,
                                      str(Path(__file__).resolve().parent), str(root), 'warm' if warmed else 'cold'],
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                     creationflags=0x08000000 if sys.platform == 'win32' else 0)
            timeout = threading.Timer(15, child.kill)
            timeout.start()
            try:
                if warmed:
                    self.assertEqual(child.stdout.readline().strip(), b'warmed')
                    self.assertEqual(list(root.iterdir()), [], 'Warmup started a recording before selection')
                    request = dict(directory=str(root), region=dict(x=0, y=0, width=1920, height=1080))
                    child.stdin.write((json.dumps(request) + '\n').encode('utf-8'))
                    child.stdin.flush()
                self.assertEqual(child.stdout.readline().strip(), b'ready')
                child.stdin.write(b'{"offset":0}\n{"at":0,"png":null}\ninvalid annotation\n')
                child.stdin.flush()
                time.sleep(.4)
                self.assertIsNone(child.poll(), 'Video stopped before the explicit stop command')
                _, errors = child.communicate(b'stop\n', timeout=10)
                self.assertEqual(child.returncode, 0, errors.decode('utf-8', errors='replace'))
            finally:
                timeout.cancel()
                if child.poll() is None:
                    child.kill()
                    child.communicate()
            saved = json.loads((root / 'video-diagnostics.json').read_text(encoding='utf-8'))
            self.assertEqual(saved['stop_reason'], 'requested')
            self.assertGreater(saved['frames'], 2)
            with av.open(str(root / 'capture.video.mp4')) as source:
                frames = list(source.decode(video=0))
                self.assertGreater(len(frames), 2)
                self.assertGreater(len(set(round(frame.to_ndarray().mean()) for frame in frames)), 2)

    def test_warmup_cancel_and_eof_exit_without_recording_or_files(self):
        script = textwrap.dedent('''\
            import sys
            sys.path.insert(0, sys.argv[1])
            import video
            def forbidden_capture(*args):
                raise AssertionError('Warmup must not record before selection')
            video.capture = forbidden_capture
            sys.argv = ['video.py', '--warmup']
            video.main()
        ''')
        for command in (b'', b'stop\n'):
            with self.subTest(command=command), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                child = subprocess.Popen([sys.executable, '-I', '-X', 'utf8', '-u', '-c', script,
                                          str(Path(__file__).resolve().parent)], cwd=root,
                                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                         creationflags=0x08000000 if sys.platform == 'win32' else 0)
                timeout = threading.Timer(15, child.kill)
                timeout.start()
                try:
                    self.assertEqual(child.stdout.readline().strip(), b'warmed')
                    self.assertEqual(list(root.iterdir()), [])
                    _, errors = child.communicate(command, timeout=5)
                    self.assertEqual(child.returncode, 0, errors.decode('utf-8', errors='replace'))
                    self.assertEqual(list(root.iterdir()), [])
                finally:
                    timeout.cancel()
                    if child.poll() is None:
                        child.kill()
                        child.communicate()

    def test_control_eof_does_not_report_a_successful_recording(self):
        class Desktop:
            def read(self): return np.full((48, 64, 4), 90, dtype=np.uint8)
            def close(self): pass
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch('video.Desktop', return_value=Desktop()), patch('video.sys.stdin', io.StringIO('{"offset":0}\n')), patch('video.sys.stdout', io.StringIO()):
                with self.assertRaisesRegex(RuntimeError, 'control_eof'):
                    capture(root, dict(x=0, y=0, width=64, height=48))
            saved = json.loads((root / 'video-diagnostics.json').read_text(encoding='utf-8'))
            self.assertNotEqual(saved['stage'], 'completed')
            self.assertEqual(saved['stop_reason'], 'control_eof')
            self.assertTrue((root / 'capture.video.mp4').is_file())

    def test_diagnostics_keep_reporting_when_desktop_read_is_blocked(self):
        entered, release, reported = threading.Event(), threading.Event(), threading.Event()
        errors = []
        class Desktop:
            def read(self):
                entered.set()
                if not release.wait(5): raise RuntimeError('Synthetic capture timed out')
                return np.full((48, 64, 4), 90, dtype=np.uint8)
            def close(self): pass
        from video import save_diagnostics
        def save(root, state):
            save_diagnostics(root, state)
            if state['stage'] == 'desktop_copy': reported.set()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def run():
                try: capture(root, dict(x=0, y=0, width=64, height=48))
                except Exception as error: errors.append(error)
            with patch('video.Desktop', return_value=Desktop()), patch('video.save_diagnostics', side_effect=save), patch('video.sys.stdin', io.StringIO('{"offset":0}\nstop\n')), patch('video.sys.stdout', io.StringIO()):
                worker = threading.Thread(target=run)
                worker.start()
                try:
                    self.assertTrue(entered.wait(2))
                    self.assertTrue(reported.wait(3), 'No diagnostics during the blocked desktop copy')
                    saved = json.loads((root / 'video-diagnostics.json').read_text(encoding='utf-8'))
                    self.assertEqual(saved['stage'], 'desktop_copy')
                    self.assertEqual(saved['frames'], 0)
                finally:
                    release.set()
                    worker.join(timeout=5)
                self.assertFalse(worker.is_alive())
                self.assertEqual(errors, [])

    def test_mux_rejects_two_frames_with_long_audio_and_preserves_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'capture.video.mp4'
            with av.open(str(source), 'w', options={'movflags': 'frag_keyframe+empty_moov+default_base_moof'}) as output:
                stream = video_stream(output, 64, 48)
                for pts in (0, 100):
                    frame = av.VideoFrame.from_ndarray(np.full((48, 64, 3), 90, dtype=np.uint8), format='rgb24')
                    frame.pts, frame.time_base = pts, Fraction(1, 1000)
                    for packet in stream.encode(frame): output.mux(packet)
                for packet in stream.encode(): output.mux(packet)
            with wave.open(str(root / 'system.wav'), 'wb') as output:
                output.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
                output.writeframes(np.zeros(8 * 16000, dtype='<i2').tobytes())
            original = source.read_bytes()
            destination = root / 'registrazione.mp4'
            destination.write_bytes(b'previous complete recording')
            with self.assertRaisesRegex(RuntimeError, 'Video incompleto'):
                mux(root, destination)
            self.assertEqual(destination.read_bytes(), b'previous complete recording')
            self.assertEqual(source.read_bytes(), original)
            self.assertTrue((root / 'system.wav').is_file())
            self.assertFalse(destination.with_suffix('.part.mp4').exists())

    def test_mux_keeps_video_timestamps_and_both_audio_tracks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with av.open(str(root / 'capture.video.mp4'), 'w') as output:
                stream = video_stream(output, 320, 240)
                # Irregular timestamps model a slow capture; duration must not accelerate.
                for pts in (0, 83, 400, 800, 1000):
                    frame = av.VideoFrame.from_ndarray(np.full((240, 320, 3), 100, dtype=np.uint8), format='rgb24')
                    frame.pts, frame.time_base = pts, Fraction(1, 1000)
                    for packet in stream.encode(frame):
                        output.mux(packet)
                for packet in stream.encode():
                    output.mux(packet)
            for name, frequency, count in [('microphone', 300, 16000), ('system', 900, 12000)]:
                with wave.open(str(root / f'{name}.wav'), 'wb') as output:
                    output.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
                    tone = np.sin(np.arange(count) * 2 * np.pi * frequency / 16000) * 5000
                    output.writeframes(tone.astype('<i2').tobytes())
            destination = root / 'registrazione.mp4'
            mux(root, destination)
            with av.open(str(destination)) as result:
                self.assertEqual(len(result.streams.video), 1)
                self.assertEqual(len(result.streams.audio), 1)
                video = list(result.decode(video=0))
                times = [float(f.pts * f.time_base) for f in video]
                np.testing.assert_allclose(times, [0, .083, .4, .8, 1.], atol=.002)
            with av.open(str(destination)) as result:
                samples = np.concatenate([frame.to_ndarray().ravel() for frame in result.decode(audio=0)])
                spectrum = np.abs(np.fft.rfft(samples[:8000]))
                self.assertGreater(spectrum[150], 10)
                self.assertGreater(spectrum[450], 10)
                self.assertGreater(len(samples), 15000)
            self.assertFalse(destination.with_suffix('.part.mp4').exists())

    def test_failed_mux_does_not_publish_or_destroy_existing_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'capture.video.mp4'
            source.write_bytes(b'invalid synthetic input')
            destination = root / 'registrazione.mp4'
            destination.write_bytes(b'previous complete recording')
            with self.assertRaises(Exception):
                mux(root, destination)
            self.assertEqual(destination.read_bytes(), b'previous complete recording')
            self.assertEqual(source.read_bytes(), b'invalid synthetic input')
            self.assertFalse(destination.with_suffix('.part.mp4').exists())


if __name__ == '__main__':
    unittest.main()
