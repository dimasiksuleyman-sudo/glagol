import importlib.util
from pathlib import Path
import sys
import unittest
spec=importlib.util.spec_from_file_location("worker",Path(__file__).resolve().parents[2]/"src-tauri/src/tts/silero/worker.py")
worker=importlib.util.module_from_spec(spec)
original=sys.stdout
spec.loader.exec_module(worker)
sys.stdout=original

class TextTests(unittest.TestCase):
    def test_numbers_and_latin_do_not_disappear(self):
        text=worker.normalize("Windows USB 2026,56 14:30 12.09.2026")
        self.assertIn("виндоус ю эс би",text)
        self.assertIn("две тысячи двадцать шесть",text)
        self.assertIn("запятая пятьдесят шесть",text)
        self.assertIn("четырнадцать часов тридцать",text)
        self.assertFalse(any(c.isdigit() for c in text))
    def test_stress_and_questions_survive(self):
        self.assertEqual(worker.normalize("зам+ок? по+ёт! з+амок. замо́к"),"зам+ок? по+ёт! з+амок. зам+ок")
    def test_long_digits_preserve_every_digit(self):
        self.assertEqual(worker.normalize("001"),"ноль ноль один")
        self.assertEqual(len(worker.normalize("1"*20).split()),20)
    def test_split_is_bounded_and_lossless(self):
        text="Это предложение? Следующее предложение. "*100
        parts=list(worker.segments(text.strip()))
        self.assertTrue(all(0<len(p)<=480 for p in parts))
        self.assertEqual(" ".join(parts),text.strip())

if __name__=="__main__": unittest.main()
