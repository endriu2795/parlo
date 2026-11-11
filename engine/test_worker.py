import unittest
from worker import group_words

class GroupingTests(unittest.TestCase):
    def test_split_sentence_at_channel_change(self):
        words=[{'start':0,'end':1,'speaker':'Tu','text':' Ciao'},
               {'start':1,'end':2,'speaker':'Tu','text':' Marco.'},
               {'start':2,'end':3,'speaker':'Speaker','text':' Ciao!'}]
        segments=group_words(words,'mixed')
        self.assertEqual(len(segments),2)
        self.assertEqual(segments[0]['text'],'Ciao Marco.')
        self.assertEqual(segments[1]['start_ms'],2000)
        self.assertEqual(segments[1]['speaker'],'Speaker')

if __name__ == '__main__': unittest.main()