"""Checks of the buy-gold builder without requiring the proprietary game assets."""
import hashlib
from pathlib import Path
import re
import tempfile
import unittest
from unittest import mock

import buy_gold_026g as buy_gold

HERE = Path(__file__).resolve().parent


class BuyGoldTests(unittest.TestCase):
    def test_unknown_movie_is_rejected_before_invoking_compiler(self):
        with mock.patch.object(buy_gold.subprocess, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'Not a known live lobby movie'):
                buy_gold.build_movie(b'unknown movie', None, None, None, None)
            run.assert_not_called()

    def test_retail_movie_is_named_as_such(self):
        retail = hashlib.sha256(b'retail movie').hexdigest()
        with mock.patch.object(buy_gold, 'RETAIL_SHA256', retail), \
                mock.patch.object(buy_gold.subprocess, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'retail lobby movie'):
                buy_gold.build_movie(b'retail movie', None, None, None, None, known=False)
            run.assert_not_called()

    def test_existing_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'lobby_menu.swf'
            output.write_bytes(b'existing movie')
            with mock.patch('sys.argv', ['buy_gold_026g.py', 'unused', str(output), '--ffdec=unused', '--step-only']):
                with self.assertRaises(SystemExit):
                    buy_gold.main()
            self.assertEqual(output.read_bytes(), b'existing movie')

    def test_every_class_changed_may_change_its_block(self):
        java = (HERE / 'buy_gold' / 'PatchBuyGold.java').read_text(encoding='utf-8')
        store = re.search(r'STORE = \{(.*?)\};', java, re.S).group(1)
        # Each class's block, and the exchange window's for the new class.
        self.assertEqual(len(re.findall(r'"survarium\.[\w.]+"', store)) + 1, buy_gold.HOOKED)

    def test_the_exchange_opens_over_the_dialog_on_the_buy_side(self):
        source = (HERE / 'buy_gold' / 'SrpBuyGold.as').read_text(encoding='utf-8')
        self.assertIn('Message.showMessage("exchange", {"label": "", "srpBuyGold": true}, true, 0, true);',
                      source)


if __name__ == '__main__':
    unittest.main()
