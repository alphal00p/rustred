"""Pure forwarding tests; no native child, license, algebra or builds."""
import argparse
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).with_name("apply_guarded_owner_rules.py")
SPEC = importlib.util.spec_from_file_location("guarded_driver", SOURCE)
DRIVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DRIVER)


class GuardedDriverTests(unittest.TestCase):
    def arguments(self):
        return [str(SOURCE), "--executable", "native", "--manifest", "owners.json",
                "--queries", "queries.json", "--output", "result.json"]

    def test_defaults_only_select_diagnostic_not_policy_or_work(self):
        with patch("sys.argv", self.arguments()), patch.object(DRIVER.os, "execve") as execute:
            DRIVER.main()
        command = execute.call_args.args[1]
        self.assertEqual(command[1], "owner-guarded-apply")
        for option in (*DRIVER.ALLOWANCES, "work-limits", "workers", "timeout", "follow-successors"):
            self.assertNotIn("--" + option, command)

    def test_explicit_paths_allowances_and_environment_are_forwarded(self):
        args = self.arguments() + ["--owner-base", "owner dir", "--work-limits", "native.json",
                                   "--events", "events.jsonl", "--stop-file", "stop", "--no-progress"]
        for i, name in enumerate(DRIVER.ALLOWANCES, 1):
            args.extend(["--" + name, str(i)])
        inherited = {"SYMBOLICA_LICENSE": "test-only-placeholder", "RAYON_NUM_THREADS": "99"}
        with patch("sys.argv", args), patch.object(DRIVER.os, "environ", inherited), \
                patch.object(DRIVER.os, "execve") as execute, patch("sys.stdout", new_callable=io.StringIO) as output:
            DRIVER.main()
        executable, command, environment = execute.call_args.args
        self.assertEqual(executable, str(Path("native").resolve()))
        self.assertEqual(command[command.index("--work-limits") + 1], "native.json")
        self.assertEqual(command[command.index("--owner-base") + 1], "owner dir")
        for i, name in enumerate(DRIVER.ALLOWANCES, 1):
            self.assertEqual(command[command.index("--" + name) + 1], str(i))
        self.assertEqual(environment["SYMBOLICA_LICENSE"], inherited["SYMBOLICA_LICENSE"])
        self.assertEqual(environment["RAYON_NUM_THREADS"], "1")
        self.assertEqual(inherited["RAYON_NUM_THREADS"], "99")
        self.assertEqual(output.getvalue(), "")

    def test_invalid_or_non_diagnostic_flags_never_execute(self):
        for suffix in (["--workers", "6"], ["--timeout", "60"], ["--follow-successors"],
                       ["--max-numerator-rank", "10"], ["--guards", "forged"], ["--max-report-events", "0"]):
            with patch("sys.argv", self.arguments() + suffix), patch.object(DRIVER.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO), self.assertRaises(SystemExit):
                DRIVER.main()
            execute.assert_not_called()

    def test_allowances_are_strict_positive_ascii_integers(self):
        self.assertEqual(DRIVER.positive("1000000"), 1000000)
        for value in ("0", "-1", "+1", "1.0", "", " 1", "１"):
            with self.assertRaises(argparse.ArgumentTypeError):
                DRIVER.positive(value)


if __name__ == "__main__":
    unittest.main()
