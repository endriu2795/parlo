"""Optional local screen recording. No inference imports and no network access."""
from __future__ import annotations
import argparse
import base64
import ctypes
import heapq
import io
import json
import math
import os
import sys
import threading
import time
import wave
from contextlib import ExitStack
from collections import deque
from fractions import Fraction
from pathlib import Path

FPS = 30


class ScreenUnavailable(RuntimeError):
    """A desktop capture operation failed and may be recoverable."""


def screen_error(operation):
    code = ctypes.get_last_error() if hasattr(ctypes, 'get_last_error') else 0
    detail = f'codice Windows {code}' if code else 'codice Windows non disponibile'
    return ScreenUnavailable(f'Cattura video: {operation} non riuscita ({detail}).')


def clear_screen_error():
    if hasattr(ctypes, 'set_last_error'):
        ctypes.set_last_error(0)


class DrawingLayer:
    """Apply live annotation updates only to frames at or after their timestamp."""
    def __init__(self, region, width, height, visible=None):
        self.region, self.width, self.height = region, width, height
        self.layer, self.window = None, 0
        self.pending, self.lock = deque(maxlen=16), threading.Lock()
        if visible is not None:
            self.visible = visible
        elif os.name == 'nt':
            from ctypes import wintypes
            function = ctypes.WinDLL('user32').IsWindowVisible
            function.argtypes, function.restype = [wintypes.HWND], wintypes.BOOL
            self.visible = function
        else:
            self.visible = lambda window: True

    def update(self, data):
        # Keep memory bounded if encoding temporarily falls behind. Older states
        # are irrelevant once the next desktop frame is acquired in real time.
        if not isinstance(data, dict) or not isinstance(data.get('at'), (int, float)) or not math.isfinite(data['at']) or data['at'] < 0:
            return False
        if data.get('png') is not None and not isinstance(data['png'], str):
            return False
        with self.lock:
            self.pending.append(data)
        return True

    def refresh(self, at):
        import av
        import numpy as np
        data = None
        with self.lock:
            while self.pending and self.pending[0]['at'] <= at:
                data = self.pending.popleft()
        if data is None:
            return
        if data.get('png') is None:
            self.layer, self.window = None, 0
            return
        try:
            with av.open(io.BytesIO(base64.b64decode(data['png'])), format='png_pipe') as source:
                image = next(source.decode(video=0)).to_ndarray(format='rgba')
            r = self.region
            left, top = max(r['x'], data['x']), max(r['y'], data['y'])
            right = min(r['x'] + r['width'], data['x'] + data['width'])
            bottom = min(r['y'] + r['height'], data['y'] + data['height'])
            layer = None
            if left < right and top < bottom:
                ih, iw = image.shape[:2]
                x0, x1 = [round((x - data['x']) * iw / data['width']) for x in (left, right)]
                y0, y1 = [round((y - data['y']) * ih / data['height']) for y in (top, bottom)]
                ox0, ox1 = [round((x - r['x']) * self.width / r['width']) for x in (left, right)]
                oy0, oy1 = [round((y - r['y']) * self.height / r['height']) for y in (top, bottom)]
                if x0 < x1 and y0 < y1 and ox0 < ox1 and oy0 < oy1:
                    crop = av.VideoFrame.from_ndarray(np.ascontiguousarray(image[y0:y1, x0:x1]), format='rgba')
                    rgba = crop.reformat(width=ox1-ox0, height=oy1-oy0).to_ndarray(format='rgba')
                    ys, xs = np.nonzero(rgba[:, :, 3])
                    if len(xs):
                        bx0, bx1, by0, by1 = xs.min(), xs.max()+1, ys.min(), ys.max()+1
                        rgba = rgba[by0:by1, bx0:bx1]
                        alpha = rgba[:, :, 3:4].astype(np.uint16)
                        layer = (ox0+bx0, oy0+by0, rgba[:, :, [2, 1, 0]].astype(np.uint16) * alpha, 255-alpha)
            self.layer, self.window = layer, data['window']
        except (OSError, ValueError, TypeError, OverflowError, KeyError, StopIteration, av.error.FFmpegError):
            # A bad annotation must not interrupt the screen/audio capture or
            # leave an old drawing stuck on subsequent frames.
            self.layer, self.window = None, 0

    def apply(self, image, at):
        import numpy as np
        self.refresh(at)
        if self.layer is not None and self.visible(self.window):
            x, y, color, inverse = self.layer
            height, width = color.shape[:2]
            destination = image[y:y+height, x:x+width, :3]
            destination[:] = ((destination.astype(np.uint16) * inverse + color + 127) // 255).astype(np.uint8)
        return image


class Desktop:
    """Reuse one top-down DIB, capturing only the selected physical rectangle."""
    def __init__(self, x, y, width, height):
        from ctypes import wintypes as wt
        if os.name != 'nt':
            raise RuntimeError('Registrazione video disponibile su Windows.')
        self.x, self.y, self.width, self.height = x, y, width, height
        self.user = ctypes.WinDLL('user32', use_last_error=True)
        self.gdi = ctypes.WinDLL('gdi32', use_last_error=True)
        self.user.SetProcessDpiAwarenessContext.argtypes = [ctypes.c_void_p]
        self.user.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
        self.user.GetDC.argtypes, self.user.GetDC.restype = [wt.HWND], wt.HDC
        self.user.ReleaseDC.argtypes = [wt.HWND, wt.HDC]
        self.gdi.CreateCompatibleDC.argtypes, self.gdi.CreateCompatibleDC.restype = [wt.HDC], wt.HDC
        self.gdi.SelectObject.argtypes, self.gdi.SelectObject.restype = [wt.HDC, wt.HANDLE], wt.HANDLE
        self.gdi.DeleteObject.argtypes = [wt.HANDLE]
        self.gdi.DeleteDC.argtypes = [wt.HDC]
        self.gdi.BitBlt.argtypes = [wt.HDC, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                   wt.HDC, ctypes.c_int, ctypes.c_int, wt.DWORD]
        self.gdi.GdiFlush.argtypes, self.gdi.GdiFlush.restype = [], wt.BOOL
        class Header(ctypes.Structure):
            _fields_ = [('size', wt.DWORD), ('width', wt.LONG), ('height', wt.LONG),
                        ('planes', wt.WORD), ('bits', wt.WORD), ('compression', wt.DWORD),
                        ('image_size', wt.DWORD), ('xppm', wt.LONG), ('yppm', wt.LONG),
                        ('colors', wt.DWORD), ('important', wt.DWORD)]
        self.gdi.CreateDIBSection.argtypes = [wt.HDC, ctypes.POINTER(Header), wt.UINT,
                                            ctypes.POINTER(ctypes.c_void_p), wt.HANDLE, wt.DWORD]
        self.gdi.CreateDIBSection.restype = wt.HANDLE
        self.dc = self.user.GetDC(None)
        self.memory = self.gdi.CreateCompatibleDC(self.dc)
        self.pixels = ctypes.c_void_p()
        header = Header(ctypes.sizeof(Header), width, -height, 1, 32, 0, 0, 0, 0, 0, 0)
        self.bitmap = self.gdi.CreateDIBSection(self.dc, ctypes.byref(header), 0,
                                               ctypes.byref(self.pixels), None, 0)
        self.old = None
        if not self.dc or not self.memory or not self.bitmap or not self.pixels.value:
            self.close()
            raise RuntimeError('Impossibile preparare la cattura video.')
        self.old = self.gdi.SelectObject(self.memory, self.bitmap)
        if not self.old or self.old == ctypes.c_void_p(-1).value:
            self.old = None
            self.close()
            raise RuntimeError('Impossibile selezionare la superficie video.')
        # The screen DC is a common DC: release it on the same capture thread
        # after preparing the DIB. Reacquire it for each desktop copy, so a
        # long recording does not keep using one potentially stale screen DC.
        self.user.ReleaseDC(None, self.dc)
        self.dc = None

    def read(self):
        import numpy as np
        clear_screen_error()
        dc = self.user.GetDC(None)
        if not dc:
            raise screen_error('accesso allo schermo')
        try:
            clear_screen_error()
            if not self.gdi.BitBlt(self.memory, 0, 0, self.width, self.height,
                                   dc, self.x, self.y, 0x40CC0020):
                raise screen_error('copia dello schermo')
            # Complete the desktop copy before releasing the DC and reading the DIB.
            clear_screen_error()
            if not self.gdi.GdiFlush():
                raise screen_error('aggiornamento dello schermo')
        finally:
            self.user.ReleaseDC(None, dc)
        buffer = (ctypes.c_uint8 * (self.width * self.height * 4)).from_address(self.pixels.value)
        return np.ctypeslib.as_array(buffer).reshape(self.height, self.width, 4)

    def close(self):
        if self.old:
            self.gdi.SelectObject(self.memory, self.old)
        if self.bitmap:
            self.gdi.DeleteObject(self.bitmap)
        if self.memory:
            self.gdi.DeleteDC(self.memory)
        if self.dc:
            self.user.ReleaseDC(None, self.dc)


class RecoveringDesktop:
    """Reacquire desktop resources for brief display transitions, with a limit."""
    def __init__(self, region, stopped, on_recovery, factory=None):
        self.region, self.stopped, self.on_recovery, self.factory = region, stopped, on_recovery, factory or Desktop
        self.current = self.factory(**region)

    def read(self):
        try:
            return self.current.read()
        except ScreenUnavailable as error:
            failure = str(error)
        self.current.close()
        self.current = None
        started = time.monotonic()
        while time.monotonic() - started < 5:
            if self.stopped.wait(.25):
                return None
            try:
                self.current = self.factory(**self.region)
                pixels = self.current.read()
            except (ScreenUnavailable, RuntimeError, OSError):
                if self.current is not None:
                    self.current.close()
                    self.current = None
                continue
            self.on_recovery(started, time.monotonic(), failure)
            return pixels
        raise ScreenUnavailable(f'{failure} Impossibile riprendere entro 5 secondi.')

    def close(self):
        if self.current is not None:
            self.current.close()


def video_stream(output, width, height):
    stream = output.add_stream('libx264', rate=FPS)
    scale = min(1., 1920 / width, 1080 / height)
    stream.width = max(2, int(width * scale) // 2 * 2)
    stream.height = max(2, int(height * scale) // 2 * 2)
    stream.pix_fmt = 'yuv420p'
    stream.codec_context.time_base = Fraction(1, 1000)
    stream.codec_context.thread_count = 2
    stream.codec_context.max_b_frames = 0
    stream.options = {'preset': 'ultrafast', 'crf': '25', 'tune': 'zerolatency'}
    return stream


def save_diagnostics(directory, state):
    """Only stages, counters and technical errors; no images or window titles."""
    try:
        partial = directory / 'video-diagnostics.json.part'
        partial.write_text(json.dumps(state, ensure_ascii=False), encoding='utf-8')
        os.replace(partial, directory / 'video-diagnostics.json')
    except OSError:
        # A full disk must not replace the original encoding/capture error.
        pass


def consume_control(lines, drawing, stopped, state):
    try:
        for line in lines:
            if line.strip() == 'stop':
                state['stop_reason'] = 'requested'
                return
            try:
                if not drawing.update(json.loads(line)):
                    state['invalid_control_messages'] += 1
            except (ValueError, TypeError):
                state['invalid_control_messages'] += 1
        state['stop_reason'] = 'control_eof'
    except OSError:
        state['stop_reason'] = 'control_read_error'
    finally:
        stopped.set()


def capture(directory, region):
    state = {'stage': 'initialization', 'frames': 0, 'recoveries': 0, 'invalid_control_messages': 0, 'stop_reason': None}
    save_diagnostics(directory, state)
    finished = threading.Event()
    def report():
        # The capture loop cannot report its stage while blocked in a driver or
        # encoder. A separate thread preserves the last progress in that case.
        while not finished.wait(1):
            save_diagnostics(directory, state.copy())
    reporter = threading.Thread(target=report, daemon=True)
    reporter.start()
    try:
        _capture(directory, region, state)
        if state['stop_reason'] != 'requested':
            reason = state['stop_reason'] or 'unknown'
            raise RuntimeError(f'Controllo video interrotto ({reason}). Audio e video acquisiti conservati.')
        state['stage'] = 'completed'
    except Exception as error:
        state['failure'] = {'stage': state['stage'], 'type': type(error).__name__, 'message': str(error)}
        raise
    finally:
        finished.set()
        reporter.join()
        save_diagnostics(directory, state)


def load_capture_dependencies():
    import av
    # On Windows, loading NumPy/OpenBLAS while another thread blocks on piped
    # stdin can deadlock native DLL initialization until the next input arrives.
    # Finish this import before "ready" and before starting the control reader.
    import numpy
    return av


def warmup():
    """Load native libraries, then wait for an explicit capture configuration."""
    load_capture_dependencies()
    print('warmed', flush=True)
    line = sys.stdin.readline()
    if not line or line.strip() == 'stop':
        return
    request = json.loads(line)
    capture(Path(request['directory']), request['region'])


def _capture(directory, region, state):
    state['stage'] = 'load_video_dependencies'
    av = load_capture_dependencies()
    stopped = threading.Event()
    state['stage'] = 'open_video_file'
    with av.open(str(directory / 'capture.video.mp4'), 'w',
                 options={'movflags': 'frag_keyframe+empty_moov+default_base_moof'}) as output:
        stream = video_stream(output, region['width'], region['height'])
        state['stage'] = 'initialize_encoder'
        output.start_encoding()  # Fail before starting the audio if the codec is unavailable.
        def recovered(start, end, failure):
            state['recoveries'] += 1
            # Preserve an explicit warning: during this gap the player holds the
            # previous frame even though audio continued to be recorded.
            warning = directory / 'video-recovery-warning.txt'
            with warning.open('a', encoding='utf-8') as output:
                output.write(f'Video temporaneamente interrotto: da {max(0, start-origin):.1f} a {max(0, end-origin):.1f} secondi; la riproduzione mantiene l’ultimo fotogramma. {failure}\n')
        state['stage'] = 'initialize_desktop'
        desktop = RecoveringDesktop(region, stopped, recovered)
        drawing = DrawingLayer(region, stream.width, stream.height)
        try:
            print('ready', flush=True)
            state['stage'] = 'start_handshake'
            request = json.loads(sys.stdin.readline())
            origin = time.monotonic() - request['offset']
            def control():
                # EOF also releases capture if the app exits unexpectedly.
                consume_control(sys.stdin, drawing, stopped, state)
            threading.Thread(target=control, daemon=True).start()
            deadline, last_pts = time.monotonic(), -1
            frame = None
            while not stopped.is_set() or frame is None:
                state['stage'] = 'desktop_copy'
                pixels = desktop.read()
                if pixels is None:
                    break
                frame = av.VideoFrame.from_ndarray(pixels, format='bgra')
                state['stage'] = 'annotations_and_resize'
                captured_at = time.monotonic() - origin
                drawing.refresh(captured_at)
                if drawing.layer is not None:
                    image = frame.reformat(width=stream.width, height=stream.height, format='bgra').to_ndarray()
                    frame = av.VideoFrame.from_ndarray(drawing.apply(image, captured_at), format='bgra')
                frame = frame.reformat(width=stream.width, height=stream.height, format='yuv420p')
                frame.pts = max(last_pts + 1, round(captured_at * 1000))
                frame.time_base = Fraction(1, 1000)
                last_pts = frame.pts
                state['stage'] = 'encode_frame'
                for packet in stream.encode(frame):
                    state['stage'] = 'write_video_packet'
                    output.mux(packet)
                state['frames'] += 1
                state['last_frame_seconds'] = round(captured_at, 3)
                deadline = max(deadline + 1 / FPS, time.monotonic())
                stopped.wait(max(0, deadline - time.monotonic()))
            # Preserve wall-clock duration even when frames were dropped under load.
            if frame is None:
                raise ScreenUnavailable('Nessun fotogramma video acquisito. Audio conservato.')
            frame.pts = max(last_pts + 1, round((time.monotonic() - origin) * 1000))
            state['stage'] = 'finalize_encoder'
            for packet in stream.encode(frame):
                output.mux(packet)
            for packet in stream.encode():
                output.mux(packet)
        finally:
            desktop.close()
        state['stage'] = 'close_video_file'


def audio_packets(directory, stream):
    import av
    import numpy as np
    with ExitStack() as stack:
        tracks = [stack.enter_context(wave.open(str(path), 'rb'))
                  for name in ('microphone.wav', 'system.wav')
                  if (path := directory / name).is_file()]
        if not tracks:
            raise RuntimeError('Audio della registrazione non disponibile.')
        if any((t.getnchannels(), t.getsampwidth(), t.getframerate()) != (1, 2, 16000) for t in tracks):
            raise ValueError('Formato audio non supportato.')
        pts = 0
        while True:
            blocks = [np.frombuffer(t.readframes(4096), dtype='<i2') for t in tracks]
            count = max(map(len, blocks))
            if not count:
                break
            samples = np.zeros(count, dtype=np.float32)
            for block in blocks:
                samples[:len(block)] += block.astype(np.float32) / (32768 * len(tracks))
            frame = av.AudioFrame.from_ndarray(samples.reshape(1, -1), format='fltp', layout='mono')
            frame.sample_rate, frame.time_base, frame.pts = 16000, Fraction(1, 16000), pts
            pts += count
            yield from stream.encode(frame)
        yield from stream.encode()


def mux(directory, destination):
    """Copy compressed video and add audio with bounded memory; commit atomically."""
    import av
    temporary = destination.with_suffix('.part.mp4')
    try:
        with av.open(str(directory / 'capture.video.mp4')) as source, av.open(str(temporary), 'w') as output:
            video = output.add_stream_from_template(source.streams.video[0])
            audio = output.add_stream('aac', rate=16000)
            audio.layout, audio.bit_rate = 'mono', 64000
            video_end, audio_end = 0., 0.
            def video_packets():
                for packet in source.demux(video=0):
                    if packet.dts is not None:
                        packet.stream = video
                        yield packet
            for packet in heapq.merge(video_packets(), audio_packets(directory, audio),
                                     key=lambda p: p.dts * p.time_base):
                # Read timestamps before mux() rescales them to the output stream.
                end = float(((packet.pts if packet.pts is not None else packet.dts)
                             + (packet.duration or 0)) * packet.time_base)
                if packet.stream == video:
                    video_end = max(video_end, end)
                else:
                    audio_end = max(audio_end, end)
                output.mux(packet)
            if video_end + max(2., audio_end * .001) < audio_end:
                raise RuntimeError(f'Video incompleto: {video_end:.1f} secondi di video su {audio_end:.1f} secondi di audio. Audio e file temporanei conservati.')
        with temporary.open('r+b') as file:
            os.fsync(file.fileno())
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--directory', type=Path)
    parser.add_argument('--region')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--warmup', action='store_true')
    args = parser.parse_args()
    if args.warmup:
        if args.directory is not None or args.region is not None or args.output is not None:
            parser.error('--warmup non accetta directory, area o output prima della selezione.')
        warmup()
    elif args.directory is None:
        parser.error('--directory è obbligatorio per registrare o salvare il video.')
    elif args.output:
        mux(args.directory, args.output)
    else:
        capture(args.directory, json.loads(args.region))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr, flush=True)
        sys.exit(1)
