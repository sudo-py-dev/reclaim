import copy
from datetime import date
import unittest

from check_advisory_policy import validate


class AdvisoryPolicyTests(unittest.TestCase):
    def setUp(self):
        self.config = {"advisories": {"ignore": [{
            "id": "RUSTSEC-2026-0370",
            "reason": "Expires 2026-12-15. Reviewed upstream maintenance notice.",
        }]}}
        self.packages = [{"name": "zvariant", "version": "5.15.1"}]

    def test_reviewed_version_before_expiry(self):
        validate(self.config, self.packages, date(2026, 12, 14))

    def test_exception_expires_at_deadline(self):
        with self.assertRaises(ValueError):
            validate(self.config, self.packages, date(2026, 12, 15))

    def test_different_dependency_version_requires_review(self):
        with self.assertRaises(ValueError):
            validate(self.config, [{"name": "zvariant", "version": "5.15.2"}], date(2026, 11, 1))

    def test_unknown_advisories_and_missing_dates_fail(self):
        for field, value in [("id", "RUSTSEC-2099-0001"), ("reason", "No deadline")]:
            config = copy.deepcopy(self.config)
            config["advisories"]["ignore"][0][field] = value
            with self.assertRaises(ValueError):
                validate(config, self.packages, date(2026, 11, 1))


if __name__ == "__main__":
    unittest.main()
