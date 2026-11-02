"""Regression tests for the project's audit rules."""
import unittest

from check_project import check_catalogs, unique_object


class ProjectRulesTests(unittest.TestCase):
    def test_valid_translation_can_reorder_placeholders(self):
        check_catalogs({"en": {"{0} of {1}": "{0} of {1}"},
                        "he": {"{0} of {1}": "{1} מתוך {0}"}})

    def test_missing_translation_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "missing"):
            check_catalogs({"en": {"Name": "Name"}, "he": {}})

    def test_changed_placeholder_count_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "placeholders"):
            check_catalogs({"en": {"{0} {0}": "{0} {0}"},
                            "he": {"{0} {0}": "{0}"}})

    def test_empty_translation_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "empty"):
            check_catalogs({"en": {"Name": " "}})

    def test_duplicate_json_key_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "duplicate"):
            unique_object([("Name", "first"), ("Name", "second")])


if __name__ == "__main__":
    unittest.main()
