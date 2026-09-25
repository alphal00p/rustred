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
    def test_joint_support_is_opt_in_scoped_and_forwarded_once(self):
        flag = "--" + MATCH.JOINT_SUPPORT_PRUNING
        route = ["--follow-successors", "--route-domain-overcover"]
        for flags in (route, route + [flag], route + [flag, "--unbounded-work"]):
            with patch("sys.argv", self.arguments() + flags), patch.object(MATCH.os, "execve") as execute:
                MATCH.main()
            self.assertEqual(execute.call_args.args[1].count(flag), int(flag in flags))
        for flags in ([flag], ["--follow-successors", flag], route + [flag, flag]):
            with patch("sys.argv", self.arguments() + flags), patch.object(MATCH.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()

    def arguments(self):
        return [str(SOURCE), "--executable", "native", "--manifest", "selection.json",
                "--queries", "queries.json", "--output", "result.json"]

    def test_application_cell_refinement_is_positive_opt_in_not_work_cap(self):
        flag = "--" + MATCH.APPLICATION_REFINEMENT
        with patch("sys.argv", self.arguments() + ["--follow-successors", "--unbounded-work", flag, "2"]), \
                patch.object(MATCH.os, "execve") as execute:
            MATCH.main()
        command = execute.call_args.args[1]
        self.assertEqual(command[command.index(flag) + 1], "2")
        self.assertEqual(command.count(flag), 1)
        bad = [[flag, "2"], ["--follow-successors", flag, "2", flag, "3"]]
        bad += [["--follow-successors", flag, value] for value in
                ("0", "-1", "+1", "1.5", "True", "１", str(2 * MATCH.sys.maxsize + 2))]
        for flags in bad:
            with self.subTest(flags=flags), patch("sys.argv", self.arguments() + flags), \
                    patch.object(MATCH.os, "execve") as execute, patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()

    def test_checkpoint_resume_unbounded_and_subdivision_flags_are_forwarded_exactly(self):
        for mode in ("--checkpoint", "--resume"):
            flags = ["--follow-successors", mode, "checkpoint", "--checkpoint-interval-seconds", "3600",
                     "--unbounded-work", "--apply-subdivision-axis", "0", "--apply-subdivision-cut", "2"]
            with patch("sys.argv", self.arguments() + flags), patch.object(MATCH.os, "execve") as execute:
                MATCH.main()
            command = execute.call_args.args[1]
            for option in (mode, "--checkpoint-interval-seconds", "--apply-subdivision-axis", "--apply-subdivision-cut"):
                self.assertEqual(command[command.index(option) + 1], flags[flags.index(option) + 1])
            self.assertEqual(command.count("--unbounded-work"), 1)

    def test_checkpoint_and_subdivision_scope_errors_never_launch(self):
        for flags in (["--checkpoint", "saved"], ["--unbounded-work"],
                      ["--follow-successors", "--checkpoint", "saved", "--resume", "saved"],
                      ["--follow-successors", "--checkpoint-interval-seconds", "3600"],
                      ["--follow-successors", "--apply-subdivision-axis", "0"]):
            with patch("sys.argv", self.arguments() + flags), patch.object(MATCH.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()

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
        self.assertNotIn("--" + MATCH.REFINEMENT_AXES, command)
        self.assertNotIn("--" + MATCH.TRANSFER_LOOKAHEAD, command)
        self.assertNotIn("--" + MATCH.INITIAL_D_REUSE, command)
        self.assertNotIn("--" + MATCH.PUBLICATION_POLICY, command)
        self.assertNotIn("--" + MATCH.INSPECTION_WORKERS, command)
        self.assertNotIn("--" + MATCH.APPLICATION_REFINEMENT, command)
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

    def test_refinement_cells_are_opt_in_and_explicit_zero_is_forwarded(self):
        option = "--" + MATCH.REFINEMENT
        for value in ("0", "10", "64"):
            with patch("sys.argv", self.arguments() + [option, value]), \
                    patch.object(MATCH.os, "execve") as execute:
                MATCH.main()
            command = execute.call_args.args[1]
            self.assertEqual(command[command.index(option) + 1], value)
        for invalid in ("-1", "+1", "1.1", " 1", "１"):
            with self.assertRaises(argparse.ArgumentTypeError):
                MATCH.nonnegative(invalid)

    def test_refinement_axes_are_explicit_independent_local_policy(self):
        option = "--" + MATCH.REFINEMENT_AXES
        for value in MATCH.REFINEMENT_AXIS_CHOICES:
            for mode in ([], ["--follow-successors"]):
                flags = mode + [option, value, "--" + MATCH.REFINEMENT, "0"]
                with patch("sys.argv", self.arguments() + flags), \
                        patch.object(MATCH.os, "execve") as execute:
                    MATCH.main()
                command = execute.call_args.args[1]
                self.assertEqual(command[command.index(option) + 1], value)
                self.assertEqual(command[command.index("--" + MATCH.REFINEMENT) + 1], "0")
                self.assertNotIn("--route-domain-overcover", command)
        for value in ("all", "FiniteAxes", "0", ""):
            with patch("sys.argv", self.arguments() + [option, value]), \
                    patch.object(MATCH.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()

    def test_shared_successor_walk_is_opt_in(self):
        flags = ["--follow-successors"]
        for option in MATCH.WALK_ALLOWANCES:
            flags.extend(["--" + option, "17"])
        with patch("sys.argv", self.arguments() + flags), patch.object(MATCH.os, "execve") as execute:
            MATCH.main()
        command = execute.call_args.args[1]
        self.assertIn("--follow-successors", command)
        for option in MATCH.WALK_ALLOWANCES:
            self.assertEqual(command[command.index("--" + option) + 1], "17")
        with patch("sys.argv", self.arguments() + ["--max-domains", "17"]), \
                patch.object(MATCH.os, "execve") as execute, \
                patch("sys.stderr", new_callable=io.StringIO):
            with self.assertRaises(SystemExit):
                MATCH.main()
            execute.assert_not_called()

    def test_containment_unlimited_and_finite_are_explicit_but_omission_is_not_forwarded(self):
        option = "--max-containment-checks"
        for value in ("unlimited", "19"):
            with patch("sys.argv", self.arguments() + ["--follow-successors", option, value]), \
                    patch.object(MATCH.os, "execve") as execute:
                MATCH.main()
            command = execute.call_args.args[1]
            self.assertEqual(command[command.index(option) + 1], value)
        with patch("sys.argv", self.arguments() + ["--follow-successors"]), \
                patch.object(MATCH.os, "execve") as execute:
            MATCH.main()
        self.assertNotIn(option, execute.call_args.args[1])
        for suffix in ([option, "unlimited"],
                       ["--follow-successors", option, "0"],
                       ["--follow-successors", option, "Unlimited"]):
            with patch("sys.argv", self.arguments() + suffix), \
                    patch.object(MATCH.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()

    def test_route_overcover_requires_explicit_walk_and_forwards_mask_budget(self):
        flags = ["--follow-successors", "--route-domain-overcover", "--max-route-masks-per-query", "321"]
        with patch("sys.argv", self.arguments() + flags), patch.object(MATCH.os, "execve") as execute:
            MATCH.main()
        command = execute.call_args.args[1]
        self.assertIn("--route-domain-overcover", command)
        self.assertEqual(command[command.index("--max-route-masks-per-query") + 1], "321")
        for suffix in (["--route-domain-overcover"],
                       ["--follow-successors", "--max-route-masks-per-query", "321"]):
            with patch("sys.argv", self.arguments() + suffix), \
                    patch.object(MATCH.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()

    def test_transfer_policy_is_opt_in_and_requires_unlimited_walk(self):
        option = "--" + MATCH.TRANSFER_LOOKAHEAD
        for cap in ([], ["--max-containment-checks", "unlimited"]):
            with patch("sys.argv", self.arguments() + ["--follow-successors", option, "50"] + cap), \
                    patch.object(MATCH.os, "execve") as execute:
                MATCH.main()
            command = execute.call_args.args[1]
            self.assertEqual(command[command.index(option) + 1], "50")
        for suffix in ([option, "50"], ["--follow-successors", option, "0"],
                       ["--follow-successors", option, "+1"],
                       ["--follow-successors", option, "50", "--max-containment-checks", "99"]):
            with patch("sys.argv", self.arguments() + suffix), \
                    patch.object(MATCH.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()

    def test_initial_d_band_reuse_is_explicit_unique_and_requires_delegating_walk(self):
        flag = "--" + MATCH.INITIAL_D_REUSE
        transfer = "--" + MATCH.TRANSFER_LOOKAHEAD
        for cap in ([], ["--max-containment-checks", "unlimited"]):
            flags = ["--follow-successors", transfer, "50", flag] + cap
            with patch("sys.argv", self.arguments() + flags), \
                    patch.object(MATCH.os, "execve") as execute:
                MATCH.main()
            command = execute.call_args.args[1]
            self.assertEqual(command.count(flag), 1)
            self.assertEqual(command[command.index(transfer) + 1], "50")
        for suffix in ([flag], ["--follow-successors", flag],
                       [transfer, "50", flag],
                       ["--follow-successors", transfer, "50", flag, "--max-containment-checks", "99"],
                       ["--follow-successors", transfer, "50", flag, flag],
                       ["--follow-successors", transfer, "50", flag, "false"]):
            with patch("sys.argv", self.arguments() + suffix), \
                    patch.object(MATCH.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()

    def test_native_and_aggregate_events_and_workers_are_independent_controls(self):
        flags = ["--follow-successors", "--workers", "6", "--max-successor-events", "701",
                 "--max-rhs-events-per-query", "303", "--max-shift-groups-per-query", "202",
                 "--max-sign-splits-per-query", "101"]
        with patch("sys.argv", self.arguments() + flags), patch.object(MATCH.os, "execve") as execute:
            MATCH.main()
        command = execute.call_args.args[1]
        for option, value in (("workers", "6"), ("max-successor-events", "701"),
                              ("max-rhs-events-per-query", "303"), ("max-shift-groups-per-query", "202"),
                              ("max-sign-splits-per-query", "101")):
            self.assertEqual(command[command.index("--" + option) + 1], value)
        for suffix in (["--max-rhs-events-per-query", "10"],
                       ["--max-shift-groups-per-query", "10"],
                       ["--max-sign-splits-per-query", "10"],
                       ["--workers", "1"],
                       ["--follow-successors", "--workers", "0"],
                       ["--follow-successors", "--max-rhs-events-per-query", "0"],
                       ["--follow-successors", "--max-shift-groups-per-query", "0"],
                       ["--follow-successors", "--max-sign-splits-per-query", "0"]):
            with patch("sys.argv", self.arguments() + suffix), \
                    patch.object(MATCH.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()

    def test_inspector_partition_is_forwarded_without_changing_native_workload(self):
        for workers, inspectors in [(1, 1), (2, 1), (6, 1), (6, 5), (50, 40), (50, 48)]:
            flags = ["--follow-successors", "--workers", str(workers),
                     "--inspection-workers", str(inspectors)]
            with patch("sys.argv", self.arguments() + flags), \
                    patch.object(MATCH.os, "execve") as execute:
                MATCH.main()
            command = execute.call_args.args[1]
            self.assertEqual(command[command.index("--inspection-workers") + 1], str(inspectors))
            self.assertEqual(command[command.index("--workers") + 1], str(workers))
            self.assertNotIn("--publication-policy", command)
            self.assertNotIn("--max-numerator-rank", command)

    def test_inspector_partition_rejects_bad_scope_or_budget_before_launch(self):
        for suffix in (["--inspection-workers", "1"],
                       ["--follow-successors", "--inspection-workers", "2"],
                       ["--follow-successors", "--workers", "6", "--inspection-workers", "6"],
                       ["--follow-successors", "--inspection-workers", "0"],
                       ["--follow-successors", "--inspection-workers", "1", "--inspection-workers", "1"],
                       ["--follow-successors", "--workers", "50", "--inspection-workers", "40", "--max-containment-checks", "7"]):
            with patch("sys.argv", self.arguments() + suffix), \
                    patch.object(MATCH.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()
        for workers, inspectors in [(1, 1), (2, 1), (6, 5)]:
            with patch("sys.argv", self.arguments() + ["--follow-successors", "--workers", str(workers),
                       "--inspection-workers", str(inspectors), "--max-containment-checks", "7"]), \
                    patch.object(MATCH.os, "execve") as execute:
                MATCH.main()
                execute.assert_called_once()

    def test_publication_policy_is_explicit_and_only_changes_native_steering(self):
        option = "--" + MATCH.PUBLICATION_POLICY
        for policy in MATCH.PUBLICATION_POLICIES:
            transfer = ["--transfer-unreserved-lookahead", "50"] if policy == "ready" else []
            with patch("sys.argv", self.arguments() + ["--follow-successors", option, policy] + transfer), \
                    patch.object(MATCH.os, "execve") as execute:
                MATCH.main()
            command = execute.call_args.args[1]
            self.assertEqual(command[command.index(option) + 1], policy)
            self.assertNotIn("--max-numerator-rank", command)
            self.assertNotIn("--generate", command)
        for suffix in ([option, "ordered"], [option, "owner-batched"], [option, "ready"],
                       ["--follow-successors", option, "ready"],
                       ["--follow-successors", option, "ready", "--transfer-unreserved-lookahead", "50",
                        "--apply-subdivision-axis", "0", "--apply-subdivision-cut", "2"],
                       ["--follow-successors", option, "automatic"]):
            with patch("sys.argv", self.arguments() + suffix), \
                    patch.object(MATCH.os, "execve") as execute, \
                    patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()


if __name__ == "__main__":
    unittest.main()
