"""Pure steering tests: no native child, algebra, license or build required."""
import argparse
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).with_name("match_shared_owner_domains.py")
SPEC = importlib.util.spec_from_file_location("match_domains", SOURCE)
MATCH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MATCH)


class MatchSteeringTests(unittest.TestCase):
    def arguments(self):
        return [str(SOURCE), "--executable", "native", "--manifest", "selection.json",
                "--queries", "queries.json", "--output", "result.json"]

    def test_positive_allowances_are_distinct_from_per_query_rank(self):
        for value in ("1", "10000", "1000000"):
            self.assertEqual(MATCH.positive(value), int(value))
        for value in ("0", "-1", "+1", "1.5", "NaN", "", " 1", "１"):
            with self.assertRaises(argparse.ArgumentTypeError):
                MATCH.positive(value)

    def test_defaults_do_not_invent_work_or_rank_overrides(self):
        with patch("sys.argv", self.arguments()), patch.object(MATCH.os, "execve") as execute:
            MATCH.main()
        command = execute.call_args.args[1]
        self.assertEqual(command[1], "owner-domain-match")
        self.assertEqual(command[command.index("--queries") + 1], "queries.json")
        for option in MATCH.ALLOWANCES:
            self.assertNotIn("--" + option, command)
        for option in ("--workers", "--timeout", "--max-numerator-rank", "--targets"):
            self.assertNotIn(option, command)

    def test_exec_replaces_process_with_all_explicit_allowances_and_inner_pools_one(self):
        arguments = self.arguments() + ["--owner-base", "owners", "--events", "events.jsonl",
                                       "--stop-file", "stop", "--no-progress"]
        for index, option in enumerate(MATCH.ALLOWANCES, 2):
            arguments.extend(["--" + option, str(index)])
        inherited = {"SYMBOLICA_LICENSE": "test-only-placeholder", "RAYON_NUM_THREADS": "99"}
        with patch.object(MATCH.os, "environ", inherited), patch("sys.argv", arguments), \
                patch.object(MATCH.os, "execve") as execute, \
                patch("sys.stdout", new_callable=io.StringIO) as output:
            MATCH.main()
        executable, command, environment = execute.call_args.args
        self.assertEqual(executable, str(Path("native").resolve()))
        self.assertEqual(command[0], executable)
        for index, option in enumerate(MATCH.ALLOWANCES, 2):
            self.assertEqual(command[command.index("--" + option) + 1], str(index))
        self.assertIn("--no-progress", command)
        self.assertEqual(environment["SYMBOLICA_LICENSE"], inherited["SYMBOLICA_LICENSE"])
        self.assertEqual(inherited["RAYON_NUM_THREADS"], "99")
        for name in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                     "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS"):
            self.assertEqual(environment[name], "1")
        self.assertEqual(output.getvalue(), "")

    def test_invalid_scope_or_work_flags_do_not_launch(self):
        for suffix in (["--max-queries", "0"], ["--max-numerator-rank", "10"],
                       ["--workers", "6"], ["--timeout", "60"]):
            with patch("sys.argv", self.arguments() + suffix), \
                    patch.object(MATCH.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()


if __name__ == "__main__":
    unittest.main()
