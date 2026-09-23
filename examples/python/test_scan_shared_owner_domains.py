"""Steering tests only: no algebra, license, native child or build required."""
import argparse
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).with_name("scan_shared_owner_domains.py")
SPEC = importlib.util.spec_from_file_location("scan_domains", SOURCE)
SCAN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCAN)


class DomainSteeringTests(unittest.TestCase):
    def test_rank_and_work_allowances_have_distinct_bounds(self):
        for value in (0, 10, 2**32 - 1):
            self.assertEqual(SCAN.rank(str(value)), value)
        for value in (-1, 2**32):
            with self.assertRaises(argparse.ArgumentTypeError):
                SCAN.rank(str(value))
        self.assertEqual(SCAN.positive("500000"), 500000)
        for value in ("0", "-1"):
            with self.assertRaises(argparse.ArgumentTypeError):
                SCAN.positive(value)

    def test_exec_replaces_process_and_forwards_only_explicit_allowances(self):
        arguments = [str(SOURCE), "--executable", "native", "--manifest", "selection.json",
                     "--owner-base", "owners", "--max-numerator-rank", "0",
                     "--output", "result.json", "--events", "events.jsonl",
                     "--stop-file", "stop.json", "--max-summary-groups", "500000"]
        inherited = {"SYMBOLICA_LICENSE": "test-only-placeholder", "RAYON_NUM_THREADS": "99"}
        with patch.object(SCAN.os, "environ", inherited), patch("sys.argv", arguments), \
                patch.object(SCAN.os, "execve") as execute, patch("sys.stdout", new_callable=io.StringIO) as out:
            SCAN.main()
        executable, command, environment = execute.call_args.args
        self.assertEqual(executable, str(Path("native").resolve()))
        self.assertEqual(command[1], "owner-domain-scan")
        for option, value in (("--max-numerator-rank", "0"), ("--owner-base", "owners"),
                              ("--max-summary-groups", "500000"), ("--events", "events.jsonl"),
                              ("--stop-file", "stop.json")):
            self.assertEqual(command[command.index(option) + 1], value)
        self.assertNotIn("--max-total-regions", command)
        self.assertNotIn("--workers", command)
        self.assertNotIn("--timeout", command)
        self.assertEqual(environment["SYMBOLICA_LICENSE"], inherited["SYMBOLICA_LICENSE"])
        self.assertEqual(inherited["RAYON_NUM_THREADS"], "99")
        for name in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                     "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS"):
            self.assertEqual(environment[name], "1")
        self.assertEqual(out.getvalue(), "")

    def test_unbounded_rank_is_explicit_and_exclusive(self):
        arguments = [str(SOURCE), "--executable", "native", "--manifest", "selection.json",
                     "--output", "result.json"]
        with patch("sys.argv", [*arguments, "--unbounded-rank"]), \
                patch.object(SCAN.os, "execve") as execute:
            SCAN.main()
        command = execute.call_args.args[1]
        self.assertIn("--unbounded-rank", command)
        self.assertNotIn("--max-numerator-rank", command)
        for scope in ([], ["--unbounded-rank", "--max-numerator-rank", "0"]):
            with self.subTest(scope=scope), patch("sys.argv", [*arguments, *scope]), \
                    patch("sys.stderr", new_callable=io.StringIO), \
                    patch.object(SCAN.os, "execve") as execute, self.assertRaises(SystemExit):
                SCAN.main()
            execute.assert_not_called()


if __name__ == "__main__":
    unittest.main()
