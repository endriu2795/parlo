"""Local document/media helpers. No shell, downloads or model dependencies."""
from __future__ import annotations

import argparse
from contextlib import closing
import importlib.metadata
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory


MEDIA_INPUT_FORMATS = 'mov,matroska,avi,mpeg,mpegts,wav,mp3,aac,flac,ogg,aiff,asf'
MEDIA_OUTPUT_EXTENSIONS = {'.mp4', '.mov', '.mkv', '.avi', '.webm', '.wav', '.mp3',
                           '.m4a', '.aac', '.ogg', '.opus', '.flac'}


class MediaScope:
    def __init__(self, directory, sources):
        self.directory = directory.resolve(strict=True)
        self.files = {Path(path) for path in sources['files']}
        self.excluded = {Path(path) for path in sources['excluded']}
        self.folders = [(Path(folder['path']), {Path(path) for path in folder['excluded']})
                        for folder in sources['folders']]

    @classmethod
    def current(cls):
        workspace = os.environ.get('PARLO_MEDIA_WORKSPACE')
        if workspace:
            directory = Path(workspace).resolve(strict=True)
            path = directory.parent / '.parlo-media-scope.json'
            if path.is_file() and path.resolve() == path:
                return cls(directory, json.loads(path.read_text(encoding='utf-8')))
        raise ValueError('Avvia lo strumento multimediale dalla chat di PARLO.')

    def input(self, path):
        path = path.resolve(strict=True)
        if path.is_file() and (
            path in self.files
            or (path.is_relative_to(self.directory) and path not in self.excluded)
            or any(path.is_relative_to(folder) and path not in excluded for folder, excluded in self.folders)
        ):
            return path
        raise ValueError('Usa un file della call o una fonte condivisa e abilitata.')

    def output(self, path):
        if path.is_symlink() or not path.resolve().is_relative_to(self.directory):
            raise ValueError('Salva il risultato nella cartella della call.')
        path = path.resolve()
        if path.exists():
            raise ValueError('Il file di output esiste già: scegli un nome nuovo.')
        return path


def media_input(path):
    # The helper handles local media, not playlists or other indirect inputs.
    return ['-protocol_whitelist', 'file', '-format_whitelist', MEDIA_INPUT_FORMATS, '-i', str(path)]


def run_ffmpeg(args, **options):
    return subprocess.run([str(ffmpeg_binary()), '-hide_banner', '-nostdin', *args],
                          **({'creationflags': 0x08000000} if sys.platform == 'win32' else {}), **options)


def publish_media(staged, output):
    try:
        os.link(staged, output)
    except FileExistsError:
        raise ValueError('Il file di output esiste già: scegli un nome nuovo.') from None
    except OSError:
        # Some external disks do not implement hard links.
        target = output.open('xb')
        try:
            with target, staged.open('rb') as source:
                shutil.copyfileobj(source, target)
        except OSError:
            output.unlink(missing_ok=True)
            raise


def ffmpeg_binary():
    import imageio_ffmpeg
    # Use the binary shipped in the wheel, independent of PATH and user overrides.
    binaries = Path(imageio_ffmpeg.__file__).parent / 'binaries'
    candidates = sorted(p for p in binaries.glob('ffmpeg-*') if p.is_file() and p.suffix != '.txt')
    if len(candidates) != 1:
        raise RuntimeError('FFmpeg incluso non disponibile per questa piattaforma.')
    return candidates[0].resolve()


def info():
    ffmpeg_binary()
    packages = ['python-docx', 'python-pptx', 'openpyxl', 'reportlab', 'pypdf',
                'pypdfium2', 'Pillow', 'imageio-ffmpeg', 'edge-tts']
    return {'python': sys.executable, 'helper': str(Path(__file__).resolve()),
            'packages': {name: importlib.metadata.version(name) for name in packages}}


def extract(path):
    suffix = path.suffix.lower()
    if suffix == '.docx':
        from docx import Document
        doc = Document(path)
        return {'paragraphs': [p.text for p in doc.paragraphs],
                'tables': [[[c.text for c in row.cells] for row in t.rows] for t in doc.tables]}
    if suffix in ('.pptx', '.potx'):
        from pptx import Presentation
        # python-pptx expects a presentation content type even for a template.
        from io import BytesIO
        from zipfile import ZipFile, ZIP_DEFLATED
        data = path
        if suffix == '.potx':
            data = BytesIO()
            with ZipFile(path) as source, ZipFile(data, 'w', ZIP_DEFLATED) as target:
                for entry in source.infolist():
                    content = source.read(entry)
                    if entry.filename == '[Content_Types].xml':
                        content = content.replace(b'application/vnd.openxmlformats-officedocument.presentationml.template.main+xml',
                                                  b'application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml')
                    target.writestr(entry, content)
            data.seek(0)
        return {'slides': [{'number': i + 1, 'text': [s.text for s in slide.shapes if s.has_text_frame],
                            'tables': [[[c.text for c in row.cells] for row in s.table.rows]
                                       for s in slide.shapes if s.has_table]}
                           for i, slide in enumerate(Presentation(data).slides)]}
    if suffix == '.xlsx':
        from openpyxl import load_workbook
        book = load_workbook(path, read_only=True, data_only=False)
        try:
            # Bound console output and parsing work on large sheets.
            return {'sheets': {s.title: list(s.iter_rows(max_row=min(s.max_row or 0, 200),
                                max_col=min(s.max_column or 0, 50), values_only=True)) for s in book},
                    'limits': {'rows_per_sheet': 200, 'columns_per_sheet': 50},
                    'formulas_recalculated': False}
        finally:
            book.close()
    if suffix == '.pdf':
        from pypdf import PdfReader
        return {'pages': [{'number': i + 1, 'text': page.extract_text()} for i, page in enumerate(PdfReader(path).pages)]}
    raise ValueError('Formati supportati: DOCX, PPTX, POTX, XLSX, PDF.')


def render_pdf(path, output, pages, scale):
    import pypdfium2 as pdfium
    if not math.isfinite(scale) or not 0.25 <= scale <= 4:
        raise ValueError('Scala PDF compresa tra 0.25 e 4.')
    output.mkdir(parents=True, exist_ok=True)
    saved = []
    with pdfium.PdfDocument(path) as document:
        indices = range(len(document)) if pages == 'all' else [int(p) - 1 for p in pages.split(',')]
        for i in indices:
            if not 0 <= i < len(document):
                raise ValueError(f'Pagina PDF non disponibile: {i + 1}')
            with closing(document[i]) as page:
                bitmap = page.render(scale=scale)
                try:
                    target = output / f'page-{i + 1:03}.png'
                    bitmap.to_pil().save(target)
                    saved.append(str(target.resolve()))
                finally:
                    bitmap.close()
    return {'images': saved}


def validate_mp4(path):
    try:
        result = run_ffmpeg(
            ['-xerror', *media_input(path.resolve()),
             '-map', '0:v:0', '-frames:v', '1', '-progress', 'pipe:1', '-nostats',
             '-f', 'null', '-'],
            capture_output=True, encoding='utf-8', errors='replace', timeout=30,
        )
    except subprocess.TimeoutExpired as error:
        raise ValueError('Tempo limite nella verifica del video.') from error
    if result.returncode or not re.search(r'^frame=\s*[1-9]\d*\r?$', result.stdout, re.MULTILINE):
        raise ValueError('Video senza fotogrammi validi.')
    input_info, _, output_info = result.stderr.partition('Stream mapping:')
    clock = re.search(r'(?m)^\s*Duration: (\d+):(\d+):(\d+(?:\.\d+)?)', input_info)
    video = re.search(r'(?m)^\s*Stream #0:\d+[^\r\n]*?: Video:[^\r\n]*', output_info)
    size = re.search(r',\s*([1-9]\d*)x([1-9]\d*)(?=[,\s]|$)', video[0]) if video else None
    if not clock or not size:
        raise ValueError('Video senza fotogrammi validi.')
    hours, minutes, seconds = map(float, clock.groups())
    duration = hours * 3600 + minutes * 60 + seconds
    if duration <= 0:
        raise ValueError('Video senza fotogrammi validi.')
    audio = re.search(r'(?m)^\s*Stream #0:\d+[^\r\n]*?: Audio:', input_info) is not None
    return {'valid': True, 'duration': duration, 'size': list(map(int, size.groups())), 'audio': audio}


def validate(path):
    if not path.is_file() or path.stat().st_size == 0:
        raise ValueError('File mancante o vuoto.')
    if path.suffix.lower() == '.mp4':
        return validate_mp4(path)
    data = extract(path)
    return {'valid': True, 'format': path.suffix.lower(), 'structural_check': True,
            'layout_checked': False, 'formulas_recalculated': False,
            'sections': len(next(iter(data.values())))}


def slideshow(manifest, output, scope):
    os.environ['IMAGEIO_FFMPEG_EXE'] = str(ffmpeg_binary())
    from imageio_ffmpeg import write_frames
    from PIL import Image, ImageOps

    manifest = scope.input(manifest)
    output = scope.output(output)
    if output.suffix.lower() != '.mp4':
        raise ValueError('Lo slideshow deve essere un file MP4.')
    spec = json.loads(manifest.read_text(encoding='utf-8-sig'))
    width, height = spec.get('size', [1920, 1080])
    fps = spec.get('fps', 24)
    slides = spec.get('slides', [])
    if any(type(v) is not int or not 32 <= v <= 4096 or v % 2 for v in (width, height)):
        raise ValueError('Dimensioni video pari, tra 32 e 4096 pixel.')
    if not isinstance(fps, (float, int)) or not math.isfinite(fps) or not 1 <= fps <= 60:
        raise ValueError('FPS compresi tra 1 e 60.')
    if not slides or len(slides) > 1000:
        raise ValueError('Indica da 1 a 1000 slide.')
    counts = []
    for slide in slides:
        duration = float(slide.get('duration', 4))
        if not math.isfinite(duration) or not 0 < duration <= 3600:
            raise ValueError('Durata slide non valida.')
        counts.append(max(1, round(duration * fps)))
    total = sum(counts) / fps
    audio = scope.input(manifest.parent / spec['audio']) if spec.get('audio') else None
    images = [scope.input(manifest.parent / slide['image']) for slide in slides]
    output.parent.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(prefix='.slideshow-', dir=output.parent) as staging:
        video = Path(staging) / 'video.mp4'
        # Stream one image at a time: long slideshows do not retain every RGB frame.
        with closing(write_frames(str(video), (width, height), fps=fps, codec='libx264',
                                   macro_block_size=1, ffmpeg_log_level='error',
                                   output_params=['-threads', '2', '-t', str(total)])) as writer:
            writer.send(None)
            for path, count in zip(images, counts):
                with Image.open(path) as image:
                    image = ImageOps.exif_transpose(image).convert('RGBA')
                    fitted = ImageOps.contain(image, (width, height), Image.Resampling.LANCZOS)
                    frame = Image.new('RGB', (width, height), spec.get('background', '#000000'))
                    frame.paste(fitted, ((width - fitted.width) // 2, (height - fitted.height) // 2), fitted)
                    pixels = frame.tobytes()
                for _ in range(count):
                    writer.send(pixels)
        if audio:
            muxed = Path(staging) / 'muxed.mp4'
            run_ffmpeg(['-v', 'error', *media_input(video), *media_input(audio),
                        '-map', '0:v:0', '-map', '1:a:0', '-c:v', 'copy', '-c:a', 'aac',
                        '-t', str(total), str(muxed)], check=True)
            video = muxed
        result = validate(video)
        if abs(result['duration'] - total) > 1 / fps + 0.05:
            raise ValueError('La durata del video generato non corrisponde allo slideshow.')
        publish_media(video, output)
    return result


def convert_media(args, scope):
    input_path = scope.input(args.input)
    output = scope.output(args.output)
    if output.suffix.lower() not in MEDIA_OUTPUT_EXTENSIONS:
        raise ValueError('Scegli un formato di uscita audio o video supportato.')
    options = ['-v', 'error', '-n']
    if args.start is not None:
        options.extend(['-ss', str(args.start)])
    options.extend(media_input(input_path))
    if args.audio is not None:
        options.extend(media_input(scope.input(args.audio)))
        options.extend(['-map', '0:v:0', '-map', '1:a:0'])
    if args.duration is not None:
        options.extend(['-t', str(args.duration)])
    if args.size is not None:
        if not re.fullmatch(r'[1-9]\d{0,3}x[1-9]\d{0,3}', args.size):
            raise ValueError('Dimensioni video non valide. Usa larghezza x altezza senza spazi.')
        options.extend(['-s', args.size])
    if any(value is not None and (not math.isfinite(value) or value < 0)
           for value in (args.start, args.duration)) or args.duration == 0:
        raise ValueError('Tempo di inizio o durata non validi.')
    output.parent.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(prefix='.conversion-', dir=output.parent) as staging:
        result = Path(staging) / output.name
        run_ffmpeg([*options, str(result)], check=True)
        publish_media(result, output)
    return {'file': str(output)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('info')
    for name in ('extract', 'validate'):
        child = commands.add_parser(name)
        child.add_argument('file', type=Path)
    render = commands.add_parser('render-pdf')
    render.add_argument('file', type=Path)
    render.add_argument('output', type=Path)
    render.add_argument('--pages', default='all', help='1,2,3 oppure all')
    render.add_argument('--scale', type=float, default=1.5)
    video = commands.add_parser('slideshow')
    video.add_argument('manifest', type=Path)
    video.add_argument('output', type=Path)
    ffmpeg = commands.add_parser('ffmpeg')
    ffmpeg.add_argument('-i', '--input', required=True, type=Path)
    ffmpeg.add_argument('--output', required=True, type=Path)
    ffmpeg.add_argument('--audio', type=Path)
    ffmpeg.add_argument('-ss', '--start', type=float)
    ffmpeg.add_argument('-t', '--duration', type=float)
    ffmpeg.add_argument('--size')
    args = parser.parse_args()
    if args.command == 'info': result = info()
    elif args.command == 'extract': result = extract(args.file)
    elif args.command == 'validate':
        path = MediaScope.current().input(args.file) if args.file.suffix.lower() == '.mp4' else args.file
        result = validate(path)
    elif args.command == 'render-pdf': result = render_pdf(args.file, args.output, args.pages, args.scale)
    elif args.command == 'slideshow': result = slideshow(args.manifest, args.output, MediaScope.current())
    else: result = convert_media(args, MediaScope.current())
    print(json.dumps(result, ensure_ascii=False, default=str))


if __name__ == '__main__':
    # CLI output is UTF-8 even when a Windows shell pipes through a legacy locale.
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')
    try:
        main()
    except Exception as error:
        print(json.dumps({'error': str(error)}, ensure_ascii=False), file=sys.stderr)
        raise SystemExit(1)
