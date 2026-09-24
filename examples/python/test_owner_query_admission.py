"""Input-admission forwarding only; no solver/native executable required."""
import argparse
import importlib.util
import io
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


MATCH = load("query_admission_match", "match_shared_owner_domains.py")
CAMPAIGN = load("query_admission_campaign", "shared_owner_campaign.py")


class QueryAdmissionTests(unittest.TestCase):
    def match_args(self):
        return ["match", "--executable", "native", "--manifest", "missing", "--queries", "queries.json", "--output", "result.json"]

    def test_representable_count_and_bytes_are_not_bounded_by_legacy_query_ceiling(self):
        for number in [1, 10_001, 70_000, 32 * 1024 * 1024, 2 * sys.maxsize + 1]:
            self.assertEqual(MATCH.query_allowance(str(number)), number)
        for text in ["0", "-1", "+1", "1.5", "１", str(2 * sys.maxsize + 2)]:
            with self.assertRaises(argparse.ArgumentTypeError):
                MATCH.query_allowance(text)

    def test_match_and_walk_forward_independent_allowances_once(self):
        for mode in [[], ["--follow-successors"]]:
            argv = self.match_args() + mode + ["--max-queries", "70000", "--max-query-bytes", "33554432"]
            with patch("sys.argv", argv), patch.object(MATCH.os, "execve") as execute:
                MATCH.main()
            command = execute.call_args.args[1]
            for option, value in [("--max-queries", "70000"), ("--max-query-bytes", "33554432")]:
                self.assertEqual(command.count(option), 1)
                self.assertEqual(command[command.index(option)+1], value)
            self.assertNotIn("--max-numerator-rank", command)
            self.assertNotIn("--max-domains", command)

    def test_invalid_or_duplicate_admission_options_never_exec_or_start_supervisor(self):
        for option in ["--max-queries", "--max-query-bytes"]:
            suffixes = [[option], [option, "0"], [option, str(2 * sys.maxsize+2)],
                        [option, "1", option, "2"]]
            for suffix in suffixes:
                with self.subTest(option=option, suffix=suffix), patch("sys.stderr", new_callable=io.StringIO):
                    with patch("sys.argv", self.match_args()+suffix), patch.object(MATCH.os, "execve") as execute:
                        with self.assertRaises(SystemExit): MATCH.main()
                        execute.assert_not_called()
                    base = ["campaign", "--executable", "missing", "--manifest", "missing", "--queries", "missing", "--workers", "1"]
                    with patch("sys.argv", base+suffix), patch.object(CAMPAIGN, "owned_process") as owned:
                        with self.assertRaises(SystemExit): CAMPAIGN.main()
                        owned.assert_not_called()

    def test_new_query_bytes_option_is_rejected_for_concrete_targets(self):
        base = ["campaign", "--executable", "missing", "--manifest", "missing", "--targets", "missing", "--workers", "1", "--max-query-bytes", "33554432"]
        with patch("sys.argv", base), patch.object(CAMPAIGN, "owned_process") as owned, patch("sys.stderr", new_callable=io.StringIO) as error:
            with self.assertRaises(SystemExit): CAMPAIGN.main()
        owned.assert_not_called()
        self.assertIn("require --queries", error.getvalue())


if __name__ == "__main__":
    unittest.main()
