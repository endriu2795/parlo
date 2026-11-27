"""Synthetic documents and media only; never inspects the user's archive."""
from contextlib import closing
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import wave

import document_tools as tools


class DocumentToolsTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='parlo-docs-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'file con spazi è'
        self.root.mkdir()
        self.media_environment = {**os.environ, 'PARLO_MEDIA_WORKSPACE': str(self.root)}
        self.scope = self.media_scope()

    def media_scope(self, files=(), folders=(), excluded=()):
        sources = {'files': [str(path.resolve()) for path in files],
                   'excluded': [str(path.resolve()) for path in excluded],
                   'folders': [{'path': str(folder.resolve()),
                                'excluded': [str(path.resolve()) for path in disabled]}
                               for folder, disabled in folders]}
        scope_file = self.root.parent / '.parlo-media-scope.json'
        scope_file.write_text(json.dumps(sources), encoding='utf-8')
        return tools.MediaScope(self.root, sources)

    def test_office_roundtrip_preserves_template_and_reads_tables_and_formulas(self):
        from docx import Document
        from pptx import Presentation
        from openpyxl import Workbook

        template = self.root / 'modello.docx'
        doc = Document()
        doc.sections[0].header.paragraphs[0].text = 'Intestazione modello'
        doc.save(template)
        doc = Document(template)
        doc.add_paragraph('Decisione di prova è confermata')
        doc.add_table(rows=1, cols=1).cell(0, 0).text = 'Attività sintetica'
        target = self.root / 'verbale.docx'
        doc.save(target)
        self.assertTrue(tools.validate(target)['valid'])
        self.assertEqual(tools.extract(target)['tables'], [[['Attività sintetica']]])
        self.assertEqual(Document(target).sections[0].header.paragraphs[0].text, 'Intestazione modello')
        self.assertEqual(Document(template).paragraphs, [])
        run = subprocess.run([sys.executable, '-I', str(Path(tools.__file__).resolve()), 'extract', str(target)],
                             capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertIn('Decisione di prova è confermata', json.loads(run.stdout)['paragraphs'])

        deck = Presentation()
        slide = deck.slides.add_slide(deck.slide_layouts[0])
        slide.shapes.title.text = 'Titolo di prova'
        target = self.root / 'slide.pptx'
        deck.save(target)
        self.assertIn('Titolo di prova', tools.extract(target)['slides'][0]['text'])
        self.assertFalse(tools.validate(target)['layout_checked'])

        book = Workbook()
        book.active.append([2, 3, '=SUM(A1:B1)'])
        target = self.root / 'calcolo.xlsx'
        book.save(target)
        data = tools.extract(target)
        self.assertEqual(data['sheets']['Sheet'][0][2], '=SUM(A1:B1)')
        self.assertFalse(data['formulas_recalculated'])

    def test_pdf_extract_and_render_without_poppler_or_office(self):
        from reportlab.pdfgen.canvas import Canvas
        from PIL import Image
        target = self.root / 'prova.pdf'
        canvas = Canvas(str(target), pagesize=(200, 100))
        canvas.drawString(10, 50, 'Documento sintetico')
        canvas.save()
        self.assertIn('Documento sintetico', tools.extract(target)['pages'][0]['text'])
        result = tools.render_pdf(target, self.root / 'pagine', '1', 1.5)
        with Image.open(result['images'][0]) as image:
            self.assertEqual(image.size, (300, 150))
            self.assertLess(image.convert('L').getextrema()[0], 128)

    def test_slideshow_orders_frames_and_muxes_audio_with_unicode_paths(self):
        from PIL import Image
        from imageio_ffmpeg import read_frames
        for name, color in [('uno.png', 'red'), ('due.png', 'blue')]:
            Image.new('RGB', (160, 90), color).save(self.root / name)
        audio = self.root / 'voce.wav'
        with wave.open(str(audio), 'wb') as wav:
            wav.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
            wav.writeframes(bytes(16000 * 2))
        manifest = self.root / 'sequenza.json'
        manifest.write_text(json.dumps({'size': [160, 90], 'fps': 10, 'audio': audio.name,
            'slides': [{'image': 'uno.png', 'duration': .5}, {'image': 'due.png', 'duration': .5}]}), encoding='utf-8')
        output = self.root / 'video è pronto.mp4'
        result = tools.slideshow(manifest, output, self.scope)
        self.assertAlmostEqual(result['duration'], 1, delta=.1)
        self.assertTrue(result['audio'])
        with closing(read_frames(str(output))) as frames:
            self.assertEqual(next(frames)['size'], (160, 90))
            offset = (45 * 160 + 80) * 3
            pixels = [frame[offset:offset + 3] for frame in frames]
        self.assertGreater(pixels[1][0], 200)
        self.assertGreater(pixels[8][2], 200)
        run = subprocess.run([sys.executable, '-I', str(Path(tools.__file__).resolve()), 'validate', str(output)],
                             capture_output=True, text=True, encoding='utf-8',
                             cwd=self.root, env=self.media_environment)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertTrue(json.loads(run.stdout)['valid'])
        with self.assertRaisesRegex(ValueError, 'esiste già'):
            tools.slideshow(manifest, output, self.scope)

        excerpt = self.root / 'estratto.mp4'
        run = subprocess.run([sys.executable, '-I', str(Path(tools.__file__).resolve()), 'ffmpeg',
                              '-i', str(output), '--output', str(excerpt), '-t', '.5'],
                             capture_output=True, text=True, encoding='utf-8',
                             cwd=self.root, env=self.media_environment)
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        self.assertAlmostEqual(tools.validate(excerpt)['duration'], .5, delta=.15)

    def test_mp4_validation_handles_silent_video_and_rejects_invalid_data(self):
        from PIL import Image
        image = self.root / 'slide.png'
        Image.new('RGB', (64, 64)).save(image)
        manifest = self.root / 'sequenza.json'
        manifest.write_text(json.dumps({'size': [64, 64], 'fps': 10,
            'slides': [{'image': image.name, 'duration': .5}]}), encoding='utf-8')
        self.assertFalse(tools.slideshow(manifest, self.root / 'silenzioso.mp4', self.scope)['audio'])
        invalid = self.root / 'invalido.mp4'
        invalid.write_bytes(b'invalid media')
        with self.assertRaisesRegex(ValueError, 'senza fotogrammi'):
            tools.validate(invalid)

    def test_invalid_slideshow_does_not_start_encoding(self):
        manifest = self.root / 'invalid.json'
        manifest.write_text(json.dumps({'fps': float('nan'), 'slides': [{'image': 'missing.png'}]}), encoding='utf-8')
        output = self.root / 'invalid.mp4'
        with self.assertRaisesRegex(ValueError, 'FPS'):
            tools.slideshow(manifest, output, self.scope)
        self.assertFalse(output.exists())

    def test_media_scope_accepts_chat_files_and_shared_files_and_rejects_other_paths(self):
        sibling = Path(self.temp.name)
        private = sibling / 'private.mp4'
        attachment = sibling / 'attachment.mp4'
        shared = sibling / 'shared'
        shared.mkdir()
        approved = shared / 'approved.wav'
        excluded = shared / 'excluded.wav'
        generated = self.root / 'generated.mp4'
        for path in (private, attachment, approved, excluded, generated):
            path.write_bytes(b'synthetic input')
        scope = self.media_scope(files=[attachment], folders=[(shared, [excluded])])
        for path in (attachment, approved, generated):
            self.assertEqual(scope.input(path), path.resolve())
        for path in (private, excluded):
            with self.assertRaisesRegex(ValueError, 'fonte condivisa'):
                scope.input(path)
        with self.assertRaisesRegex(ValueError, 'cartella della call'):
            scope.output(self.root / '..' / 'output.mp4')
        run = subprocess.run([sys.executable, '-I', str(Path(tools.__file__).resolve()), 'ffmpeg',
                              '-i', str(private), '--output', str(self.root / 'unexpected.mp4')],
                             capture_output=True, text=True, encoding='utf-8',
                             cwd=sibling, env=self.media_environment)
        self.assertNotEqual(run.returncode, 0)
        self.assertIn('fonte condivisa', json.loads(run.stderr)['error'])


if __name__ == '__main__':
    unittest.main()
