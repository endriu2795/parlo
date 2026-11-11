"""Local document/media helpers. No shell, downloads or model dependencies."""
from __future__ import annotations

import argparse
from contextlib import closing
import importlib.metadata
import json
import math
from pathlib import Path
import subprocess
import sys


def ffmpeg_binary():
    import imageio_ffmpeg
    # Use the binary shipped in the wheel, independent of PATH and user overrides.
    binaries = Path(imageio_ffmpeg.__file__).parent / 'binaries'
    candidates = sorted(p for p in binaries.glob('ffmpeg-*') if p.is_file() and p.suffix != '.txt')
    if len(candidates) != 1:
        raise RuntimeError('FFmpeg incluso non disponibile per questa piattaforma.')
    return candidates[0].resolve()


def info():
    packages = ['python-docx', 'python-pptx', 'openpyxl', 'reportlab', 'pypdf',
                'pypdfium2', 'Pillow', 'moviepy', 'imageio-ffmpeg', 'edge-tts']
    return {'python': sys.executable, 'helper': str(Path(__file__).resolve()),
            'ffmpeg': str(ffmpeg_binary()),
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


def validate(path):
    if not path.is_file() or path.stat().st_size == 0:
        raise ValueError('File mancante o vuoto.')
    if path.suffix.lower() == '.mp4':
        import os
        os.environ['FFMPEG_BINARY'] = str(ffmpeg_binary())
        from moviepy import VideoFileClip
        with VideoFileClip(str(path), audio=False) as clip:
            if not clip.duration or clip.duration <= 0 or min(clip.size) <= 0:
                raise ValueError('Video senza fotogrammi validi.')
            clip.get_frame(0)
            return {'valid': True, 'duration': clip.duration, 'size': clip.size,
                    'audio': bool(clip.reader.infos.get('audio_found'))}
    data = extract(path)
    return {'valid': True, 'format': path.suffix.lower(), 'structural_check': True,
            'layout_checked': False, 'formulas_recalculated': False,
            'sections': len(next(iter(data.values())))}


def slideshow(manifest, output):
    import os
    os.environ['IMAGEIO_FFMPEG_EXE'] = str(ffmpeg_binary())
    from imageio_ffmpeg import write_frames
    from PIL import Image, ImageOps

    if output.suffix.lower() != '.mp4':
        raise ValueError('Lo slideshow deve essere un file MP4.')
    if output.exists():
        raise ValueError('Il file di output esiste già: scegli un nome nuovo.')
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
    audio = str(manifest.parent / spec['audio']) if spec.get('audio') else None
    output.parent.mkdir(parents=True, exist_ok=True)
    # Stream one image at a time: long slideshows do not retain every RGB frame.
    with closing(write_frames(str(output), (width, height), fps=fps, codec='libx264',
                              macro_block_size=1, ffmpeg_log_level='error', audio_path=audio,
                              audio_codec='aac', output_params=['-threads', '2', '-t', str(total)])) as writer:
        writer.send(None)
        for slide, count in zip(slides, counts):
            path = manifest.parent / slide['image']
            with Image.open(path) as image:
                image = ImageOps.exif_transpose(image).convert('RGBA')
                fitted = ImageOps.contain(image, (width, height), Image.Resampling.LANCZOS)
                frame = Image.new('RGB', (width, height), spec.get('background', '#000000'))
                frame.paste(fitted, ((width - fitted.width) // 2, (height - fitted.height) // 2), fitted)
                pixels = frame.tobytes()
            for _ in range(count):
                writer.send(pixels)
    result = validate(output)
    if abs(result['duration'] - total) > 1 / fps + 0.05:
        raise ValueError('La durata del video generato non corrisponde allo slideshow.')
    return result


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
    ffmpeg.add_argument('args', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command == 'info': result = info()
    elif args.command == 'extract': result = extract(args.file)
    elif args.command == 'validate': result = validate(args.file)
    elif args.command == 'render-pdf': result = render_pdf(args.file, args.output, args.pages, args.scale)
    elif args.command == 'slideshow': result = slideshow(args.manifest.resolve(), args.output)
    else:
        flags = args.args[1:] if args.args[:1] == ['--'] else args.args
        raise SystemExit(subprocess.run([str(ffmpeg_binary()), *flags],
                         **({'creationflags': 0x08000000} if sys.platform == 'win32' else {})).returncode)
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
