"""The parts of the real-client driver that need no game: finding a cursor in a picture, and log waits."""
import tempfile
import unittest
from pathlib import Path

import numpy as np

import driver


def picture(height=120, width=200, colour=(20, 20, 20)):
    return np.full((height, width, 3), colour, dtype=np.uint8)


def stamp(image, sprite, x, y):
    for px, py, r, g, b in sprite.body.tolist():
        image[y + py, x + px] = (r, g, b)


class FindingTheCursor(unittest.TestCase):
    def test_it_is_found_where_it_is(self):
        image = picture()
        stamp(image, driver.ARROW, 57, 31)
        self.assertEqual(driver.ARROW.find(image), (57, 31))

    def test_nothing_on_an_empty_picture(self):
        self.assertIsNone(driver.ARROW.find(picture()))

    def test_a_bright_patch_is_not_the_cursor(self):
        anchor = driver.ARROW.body[0].tolist()
        image = picture()
        image[10:60, 10:60] = (0, 0, 0)
        image[30, 30] = anchor[2:]  # one pixel of the right colour, nothing else of its shape
        self.assertIsNone(driver.ARROW.find(image))

    def test_it_is_found_on_a_dimmed_scene(self):
        image = picture(colour=(5, 8, 6))
        stamp(image, driver.ARROW, 120, 70)
        self.assertEqual(driver.ARROW.find(image), (120, 70))

    def test_a_look_alike_outside_the_window_is_not_taken(self):
        image = picture()
        stamp(image, driver.ARROW, 120, 70)
        self.assertEqual(driver.ARROW.find(image, near=(125, 75, 20)), (120, 70))
        self.assertIsNone(driver.ARROW.find(image, near=(30, 30, 20)))

    def test_a_flat_patch_of_the_same_grey_is_not_the_cursor(self):
        grey = [int(c) for c in driver.ARROW.body[:, 2:].mean(axis=0)]
        self.assertIsNone(driver.ARROW.find(picture(colour=grey)))

    def test_a_match_in_the_corner_can_be_skipped(self):
        image = picture()
        stamp(image, driver.ARROW, 2, 2)
        self.assertEqual(driver.ARROW.find(image), (2, 2))
        self.assertIsNone(driver.ARROW.find(image, skip_corner=True))


class Waiting(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.path = Path(folder.name) / "server.log"
        self.path.write_text("old: the event\n", encoding="utf-8")
        self.run_ = driver.Run(folder.name, str(self.path), "127.0.0.1", folder.name)
        self.log = driver.Log(self.path)

    def test_old_lines_do_not_count(self):
        with self.assertRaises(driver.Stop):
            self.run_.wait(self.log, "the event", 0.1)

    def test_a_line_after_the_mark_counts(self):
        mark = self.log.mark()
        with self.path.open("a", encoding="utf-8") as out:
            out.write("new: the event\n")
        self.assertEqual(self.run_.wait(self.log, "new: the event", 0.5, since=mark), "new: the event")


if __name__ == "__main__":
    unittest.main()
