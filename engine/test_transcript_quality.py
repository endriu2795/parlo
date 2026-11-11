import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch
import numpy as np
import acceleration
import transcript_quality as quality
import worker


class TranscriptQualityTests(unittest.TestCase):
    def test_reported_loops_preserve_following_sentence_and_original(self):
        for text, count in [('dei ' * 328, 325), ('ci ' * 419, 416), ('si può costruire il timestamp ' * 31, 145)]:
            original = 'Prima. ' + text + 'Poi decidiamo la consegna.'
            clean, removed = quality.reduce_repetitions(original)
            self.assertEqual(removed, count)
            self.assertTrue(clean.startswith('Prima.'))
            self.assertTrue(clean.endswith('Poi decidiamo la consegna.'))
            segment = {'start_ms':10000,'end_ms':10520,'speaker':'Speaker','text':original,'source':'mixed'}
            reviewed = quality.review_segments([segment])[0]
            self.assertEqual(reviewed['original_text'], original)
            self.assertEqual(reviewed['text'], clean)
            self.assertIn('verifica', reviewed['quality'])
            self.assertEqual(segment['text'], original)
            self.assertEqual(quality.review_segments([reviewed]), [reviewed])

    def test_short_stutters_phrases_and_repeated_technical_terms_are_kept(self):
        for text in ['sì sì sì', 'dei dei dei dei', 'test test test test',
            'si può costruire il timestamp, si può costruire il timestamp',
            'Il timestamp identifica il dato. Il secondo timestamp identifica la risposta.']:
            self.assertEqual(quality.reduce_repetitions(text), (text, 0))

    def test_timing_density_is_checked_before_clamping(self):
        words = [SimpleNamespace(word=' parola'+str(i),start_ts=.2,end_ts=.3) for i in range(30)]
        self.assertTrue(quality.suspicious_decode(words,28))
        normal = [SimpleNamespace(word=' parola'+str(i),start_ts=i*.3,end_ts=(i+1)*.3) for i in range(30)]
        self.assertFalse(quality.suspicious_decode(normal,28))

    def test_openvino_retries_only_affected_window_once_and_marks_it(self):
        model = acceleration.OpenVINOModel.__new__(acceleration.OpenVINOModel)
        normal_words = [SimpleNamespace(word=' ' + word,start_ts=i*.2,end_ts=(i+1)*.2) for i,word in enumerate(['La','demo','sarà','pronta','venerdì.'])]
        loop = [SimpleNamespace(word=' dei',start_ts=.2,end_ts=.3) for _ in range(328)]
        model.pipeline = Mock()
        model.pipeline.generate.side_effect = [SimpleNamespace(words=loop,texts=['dei ' * 328]),
            SimpleNamespace(words=normal_words,texts=['La demo sarà pronta venerdì.']),
            SimpleNamespace(words=normal_words,texts=['La demo sarà pronta venerdì.'])]
        timeline=SimpleNamespace(get_original_time=lambda time, **_:time)
        windows=[(np.ones(32000,dtype=np.float32),0.),(np.ones(32000,dtype=np.float32),2.)]
        with patch.object(acceleration,'speech_windows',return_value=(iter(windows),timeline)):
            segments,_=model.transcribe(np.ones(64000,dtype=np.float32),language='it')
            segments=list(segments)
        self.assertEqual(model.pipeline.generate.call_count,3)
        self.assertEqual(model.pipeline.generate.call_args_list[0].kwargs['no_repeat_ngram_size'],12)
        self.assertEqual(model.pipeline.generate.call_args_list[1].kwargs['no_repeat_ngram_size'],8)
        self.assertEqual(model.pipeline.generate.call_args_list[2].kwargs['no_repeat_ngram_size'],12)
        self.assertTrue(all(word.quality for word in segments[0].words))
        self.assertTrue(all(word.quality is None for word in segments[1].words))

    def test_grouped_output_keeps_original_if_retry_did_not_remove_loop(self):
        words=[{'start':10.,'end':10.5,'speaker':'Speaker','text':' dei','quality':quality.RETRY_WARNING} for _ in range(328)]
        segment=worker.group_words(words,'mixed')[0]
        self.assertEqual(segment['text'],'dei dei dei')
        self.assertEqual(len(segment['original_text'].split()),328)
        self.assertEqual(segment['quality'],quality.REPETITION_WARNING)

if __name__=='__main__':unittest.main()
