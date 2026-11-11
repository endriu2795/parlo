"""Validate the private document/media/voice runtime without downloading anything."""
import importlib
import json
import subprocess

MODULES = ('docx', 'pptx', 'openpyxl', 'reportlab', 'pypdf', 'pypdfium2',
           'PIL', 'moviepy', 'imageio_ffmpeg', 'edge_tts', 'piper', 'onnxruntime', 'kokoro_onnx')


def check():
    for name in MODULES:
        try:
            importlib.import_module(name)
        except Exception as error:
            print(json.dumps({'ready': False, 'module': name, 'error_type': type(error).__name__}))
            return 1
    try:
        import imageio_ffmpeg
        subprocess.run([imageio_ffmpeg.get_ffmpeg_exe(), '-version'], check=True,
                       capture_output=True, timeout=30)
    except Exception as error:
        print(json.dumps({'ready': False, 'module': 'ffmpeg', 'error_type': type(error).__name__}))
        return 1
    print(json.dumps({'ready': True}))
    return 0


if __name__ == '__main__':
    raise SystemExit(check())
