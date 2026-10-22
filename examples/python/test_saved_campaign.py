"""Public saved-campaign commands: scratch metadata only, no native solves."""
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location("saved_campaign", Path(__file__).with_name("saved_campaign.py"))
WRAPPER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WRAPPER)
ROOT = Path(__file__).resolve().parents[2]


class SavedCampaignTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "TMP").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="saved-campaign-test-", dir=ROOT / "TMP")
        self.addCleanup(self.temporary.cleanup)
        self.campaign = Path(self.temporary.name)
        self.binary = self.campaign / "rustred"
        self.binary.write_text("#!/bin/sh\nexit 99\n")
        self.binary.chmod(0o700)

    def invoke(self, action, *options):
        with patch.object(WRAPPER.os, "execv") as launch:
            WRAPPER.main([action, "--campaign", str(self.campaign), "--executable", str(self.binary), *options])
        return launch.call_args.args[1] if launch.called else None

    def test_run_starts_real_solve_and_publish_without_refinement(self):
        command = self.invoke("run", "--workers", "4")
        self.assertIn("--start", command)
        self.assertIn("--workers", command)
        self.assertNotIn("--refine-masters", command)
        self.assertNotIn("--resume", command)
        self.assertIn("--master-reduction-executable", command)

    def test_modular_strategy_and_policy_are_explicit_refinement_controls(self):
        policy = self.campaign / "modular.json"
        policy.write_text('{"reconstruction":{"max_primes":24}}')
        command = self.invoke("refine", "--refinement-strategy", "modular-circuits", "--modular-policy", str(policy))
        self.assertEqual(command[command.index("--master-refinement-strategy") + 1], "modular-circuits")
        self.assertEqual(command[command.index("--master-modular-policy") + 1], str(policy))
        self.assertNotIn("--master-refinement-strategy", self.invoke("refine"))
        for action in ("publish", "inspect", "finalize-refinement"):
            with patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                self.invoke(action, "--refinement-strategy", "modular-circuits")

    def test_run_autoresumes_but_does_not_refine(self):
        path = self.campaign / "checkpoints/main/latest.json"
        path.parent.mkdir(parents=True)
        path.write_text("{}")
        command = self.invoke("run")
        self.assertIn("--resume", command)
        self.assertNotIn("--refine-masters", command)

    def test_fresh_run_defaults_to_symbolic_domain_without_selecting_a_refinement_backend(self):
        import finite_saved_campaign as finite
        with patch.object(finite, "engine", wraps=finite.engine) as choose, \
             patch.object(finite, "public_main", side_effect=AssertionError("finite backend was not requested")):
            command = self.invoke("run")
        choose.assert_called_once_with(self.campaign, None)
        self.assertIn("production_saved_owner_campaign.py", command[2])
        self.assertNotIn("--refine-masters", command)
        self.assertNotIn("--master-refinement-strategy", command)

    def test_engine_help_names_fresh_legacy_default(self):
        output = io.StringIO()
        with patch.object(WRAPPER.sys, "stdout", output), self.assertRaises(SystemExit):
            WRAPPER.main(["run", "--help"])
        self.assertIn("Epoch/CP6", output.getvalue())
        self.assertIn("finite-demand is opt-in", output.getvalue())

    def test_refine_is_only_explicit_postprocess_and_seed_depth_is_forwarded(self):
        command = self.invoke("refine", "--seed-depth", "2")
        self.assertIn("--refine-masters", command)
        self.assertIn("--resume", command)
        self.assertEqual(command[command.index("--master-seed-depth") + 1], "2")
        self.assertNotIn("--executable", command)  # Never replace the phase-one engine.
        self.assertNotIn("--master-saved-rule-assistance", command)
        self.assertNotIn("--no-master-saved-rule-assistance", command)
        self.assertNotIn("--master-containing-sector-depth", command)
        self.assertNotIn("--master-circuit-symmetry-assistance", command)
        self.assertNotIn("--no-master-circuit-symmetry-assistance", command)
        self.assertNotIn("--master-numerator-symmetry-assistance", command)
        self.assertNotIn("--no-master-numerator-symmetry-assistance", command)
        self.assertNotIn("--master-sector-symmetry-assistance", command)
        self.assertNotIn("--no-master-sector-symmetry-assistance", command)
        self.assertNotIn("--master-sector-symmetry-policy", command)
        self.assertNotIn("--master-sector-symmetry-saved-projection", command)
        self.assertNotIn("--no-master-sector-symmetry-saved-projection", command)
        self.assertNotIn("--master-normalization-profile", command)

    def test_fresh_search_is_explicit_and_keeps_campaign_resume(self):
        self.assertNotIn("--master-fresh-search", self.invoke("refine"))
        for strategy, containing_depth in (("exact", 0), ("modular-circuits", 1)):
            with self.subTest(strategy=strategy, containing_depth=containing_depth):
                command = self.invoke("refine", "--fresh-search", "--refinement-strategy", strategy,
                                      "--seed-depth", "3", "--containing-sector-depth", str(containing_depth))
                self.assertEqual(command.count("--master-fresh-search"), 1)
                self.assertNotIn("--fresh-search", command)
                self.assertIn("--refine-masters", command)
                self.assertIn("--resume", command)
                self.assertIn("--start", command)
                self.assertEqual(command[command.index("--master-seed-depth") + 1], "3")
                self.assertEqual(command[command.index("--master-containing-sector-depth") + 1],
                                 str(containing_depth))

    def test_fresh_search_rejects_nonrefinement_actions_before_dispatch(self):
        for action in ("run", "extend", "inspect", "publish", "finalize-refinement"):
            options = ["--rank", "1"] if action == "extend" else []
            for flag in ("--fresh-search", "--master-fresh-search"):
                with self.subTest(action=action, flag=flag), \
                     patch.object(WRAPPER, "native_executable", side_effect=AssertionError("native preflight")), \
                     patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    self.invoke(action, *options, flag)

    def test_refinement_column_budget_can_be_changed_or_removed(self):
        self.assertNotIn("--master-max-relation-columns", self.invoke("refine"))
        for limit in (0, 100, 2_000_000):
            command = self.invoke("refine", "--max-relation-columns", str(limit))
            self.assertEqual(command[command.index("--master-max-relation-columns") + 1], str(limit))
        for action, value in (("refine", "-1"), ("publish", "100"), ("finalize-refinement", "100")):
            with patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                self.invoke(action, "--max-relation-columns", value)

    def test_refinement_parallel_window_options_are_operational_and_forwarded(self):
        command=self.invoke("refine","--relation-batch-rows","128","--relation-block-rows","8")
        self.assertEqual(command[command.index("--master-relation-batch-rows")+1],"128")
        self.assertEqual(command[command.index("--master-relation-block-rows")+1],"8")
        for option in ("--relation-batch-rows","--relation-block-rows"):
            for value in ("0","-1"):
                with patch.object(WRAPPER.sys,"stderr",io.StringIO()),self.assertRaises(SystemExit):
                    self.invoke("refine",option,value)

    def test_refinement_workers_and_affinity_do_not_upgrade_the_closure_engine(self):
        command=self.invoke("refine","--workers","128","--cpus","64-191")
        self.assertEqual(command[command.index("--master-refinement-workers")+1],"128")
        self.assertEqual(command[command.index("--master-refinement-cpus")+1],"64-191")
        self.assertNotIn("--workers",command)
        self.assertNotIn("--cpus",command)

    def test_finalize_refinement_is_explicit_postprocess_without_search_flags(self):
        command = self.invoke("finalize-refinement")
        self.assertIn("--finalize-refinement", command)
        self.assertIn("--resume", command)
        self.assertIn("--start", command)
        self.assertNotIn("--refine-masters", command)
        with patch.object(WRAPPER.sys, "stdout", io.StringIO()):
            self.assertIsNone(self.invoke("finalize-refinement", "--dry-run"))

    def test_finalize_refinement_recovery_is_explicit_and_forwards_without_search(self):
        self.assertNotIn("--recover-interrupted-refinement", self.invoke("finalize-refinement"))
        command = self.invoke("finalize-refinement", "--recover-interrupted")
        self.assertEqual(command.count("--recover-interrupted-refinement"), 1)
        self.assertNotIn("--recover-interrupted", command)
        self.assertIn("--finalize-refinement", command)
        self.assertIn("--resume", command)
        self.assertIn("--start", command)
        for option in ("--refine-masters", "--import-refinement", "--master-proof-recipe",
                       "--master-seed-depth", "--master-containing-sector-depth", "--master-fresh-search"):
            self.assertNotIn(option, command)

    def test_interrupted_recovery_rejects_other_actions_before_dispatch(self):
        recipe = self.campaign / "proof-recipe.json"
        for action in ("run", "extend", "inspect", "refine", "publish", "import-refinement"):
            options = (["--rank", "1"] if action == "extend" else
                       ["--proof-recipe", str(recipe)] if action == "import-refinement" else [])
            for flag in ("--recover-interrupted", "--recover-interrupted-refinement"):
                with self.subTest(action=action, flag=flag), \
                     patch.object(WRAPPER, "native_executable", side_effect=AssertionError("native preflight")), \
                     patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    self.invoke(action, *options, flag)

    def test_import_refinement_requires_recipe_and_forwards_without_search(self):
        with patch.object(WRAPPER, "native_executable", side_effect=AssertionError("native preflight")), \
             patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
            self.invoke("import-refinement")
        recipe = self.campaign / "proof-recipe.json"
        recipe.write_text('{"fixture":"proof import"}')
        original = recipe.read_bytes()
        command = self.invoke("import-refinement", "--proof-recipe", str(recipe))
        self.assertEqual(command.count("--import-refinement"), 1)
        self.assertEqual(command[command.index("--master-proof-recipe") + 1], str(recipe))
        self.assertNotIn("--proof-recipe", command)
        self.assertIn("--resume", command)
        self.assertIn("--start", command)
        for option in ("--refine-masters", "--publish-only", "--finalize-refinement",
                       "--recover-interrupted-refinement", "--master-seed-depth",
                       "--master-containing-sector-depth", "--master-fresh-search",
                       "--master-refinement-strategy", "--master-numerator-symmetry-assistance",
                       "--master-sector-symmetry-assistance", "--no-master-sector-symmetry-assistance",
                       "--master-sector-symmetry-policy", "--master-sector-symmetry-saved-projection",
                       "--no-master-sector-symmetry-saved-projection"):
            self.assertNotIn(option, command)
        with patch.object(WRAPPER.sys, "stdout", io.StringIO()) as output:
            self.assertIsNone(self.invoke("import-refinement", "--proof-recipe", str(recipe), "--dry-run"))
        self.assertIn("--import-refinement", output.getvalue())
        self.assertIn("--master-proof-recipe", output.getvalue())
        self.assertEqual(recipe.read_bytes(), original)

    def test_proof_recipe_rejects_other_actions_before_dispatch(self):
        recipe = self.campaign / "proof-recipe.json"
        for action in ("run", "extend", "inspect", "refine", "publish", "finalize-refinement"):
            options = ["--rank", "1"] if action == "extend" else []
            for flag in ("--proof-recipe", "--master-proof-recipe"):
                with self.subTest(action=action, flag=flag), \
                     patch.object(WRAPPER, "native_executable", side_effect=AssertionError("native preflight")), \
                     patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    self.invoke(action, *options, flag, str(recipe))

    def test_import_refinement_rejects_search_controls(self):
        recipe = self.campaign / "proof-recipe.json"
        for options in (("--seed-depth", "3"), ("--containing-sector-depth", "1"),
                        ("--fresh-search",), ("--refinement-strategy", "exact"),
                        ("--saved-rule-assistance",), ("--circuit-symmetry-assistance",),
                        ("--numerator-symmetry-assistance",), ("--sector-symmetry-assistance",),
                        ("--no-sector-symmetry-assistance",),
                        ("--sector-symmetry-policy", "generators"),
                        ("--sector-symmetry-saved-projection",),
                        ("--no-sector-symmetry-saved-projection",)):
            with self.subTest(options=options), \
                 patch.object(WRAPPER, "native_executable", side_effect=AssertionError("native preflight")), \
                 patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                self.invoke("import-refinement", "--proof-recipe", str(recipe), *options)

    def test_collection_peers_are_repeatable_and_only_refine(self):
        first, second = self.campaign / "first", self.campaign / "second"
        command = self.invoke("refine", "--collection-artifact", str(first), "--collection-artifact", str(second))
        self.assertEqual([command[i + 1] for i, value in enumerate(command) if value == "--master-collection-artifact"], [str(first), str(second)])
        with patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
            self.invoke("publish", "--collection-artifact", str(first))

    def test_normalization_profile_is_optional_explicit_refinement_only(self):
        for profile in ("conservative", "standard"):
            command = self.invoke("refine", "--normalization-profile", profile)
            self.assertEqual(command[command.index("--master-normalization-profile") + 1], profile)
            self.assertIn("--refine-masters", command)
        for action, profile in (("publish", "standard"), ("inspect", "standard"), ("refine", "unlimited")):
            with self.subTest(action=action, profile=profile), patch.object(WRAPPER.sys, "stderr", io.StringIO()):
                with self.assertRaises(SystemExit):
                    self.invoke(action, "--normalization-profile", profile)

    def test_containing_sector_depth_is_refine_only_and_forwarded(self):
        for depth in (0, 1, 2):
            command = self.invoke("refine", "--containing-sector-depth", str(depth))
            self.assertEqual(command[command.index("--master-containing-sector-depth") + 1], str(depth))
            self.assertIn("--refine-masters", command)
        for action, options in (("refine", ["--containing-sector-depth", "-1"]),
                                ("publish", ["--containing-sector-depth", "1"]),
                                ("inspect", ["--containing-sector-depth", "1"])):
            with self.subTest(action=action), patch.object(WRAPPER.sys, "stderr", io.StringIO()):
                with self.assertRaises(SystemExit):
                    self.invoke(action, *options)

    def test_refine_saved_rule_assistance_can_be_enabled_or_explicitly_disabled(self):
        for supplied, forwarded in (("--saved-rule-assistance", "--master-saved-rule-assistance"),
                                    ("--no-saved-rule-assistance", "--no-master-saved-rule-assistance")):
            with self.subTest(supplied=supplied):
                command = self.invoke("refine", supplied)
                self.assertIn(forwarded, command)
                self.assertIn("--refine-masters", command)
                self.assertIn("--start", command)

    def test_circuit_symmetry_assistance_is_independent_and_refine_only(self):
        for supplied, forwarded in (("--circuit-symmetry-assistance", "--master-circuit-symmetry-assistance"),
                                    ("--no-circuit-symmetry-assistance", "--no-master-circuit-symmetry-assistance")):
            command = self.invoke("refine", supplied, "--saved-rule-assistance")
            self.assertIn(forwarded, command)
            self.assertIn("--master-saved-rule-assistance", command)
            self.assertIn("--refine-masters", command)
        for action in ("publish", "inspect"):
            with self.subTest(action=action), patch.object(WRAPPER.sys, "stderr", io.StringIO()):
                with self.assertRaises(SystemExit):
                    self.invoke(action, "--circuit-symmetry-assistance")

    def test_numerator_symmetry_assistance_forwards_both_choices_independently(self):
        for supplied, forwarded, opposite in (
                ("--numerator-symmetry-assistance", "--master-numerator-symmetry-assistance",
                 "--no-master-numerator-symmetry-assistance"),
                ("--no-numerator-symmetry-assistance", "--no-master-numerator-symmetry-assistance",
                 "--master-numerator-symmetry-assistance")):
            with self.subTest(supplied=supplied):
                command = self.invoke("refine", supplied)
                self.assertEqual(command.count(forwarded), 1)
                self.assertNotIn(opposite, command)
                self.assertNotIn(supplied, command)
                self.assertIn("--refine-masters", command)
                self.assertIn("--resume", command)
                self.assertIn("--start", command)
                self.assertNotIn("--master-saved-rule-assistance", command)
                self.assertNotIn("--master-circuit-symmetry-assistance", command)
                self.assertNotIn("--master-sector-symmetry-assistance", command)
                self.assertNotIn("--no-master-sector-symmetry-assistance", command)
                combined = self.invoke("refine", supplied, "--saved-rule-assistance",
                                       "--circuit-symmetry-assistance")
                self.assertIn(forwarded, combined)
                self.assertNotIn(opposite, combined)
                self.assertIn("--master-saved-rule-assistance", combined)
                self.assertIn("--master-circuit-symmetry-assistance", combined)

    def test_sector_symmetry_assistance_forwards_both_choices_independently(self):
        for supplied, forwarded, opposite in (
                ("--sector-symmetry-assistance", "--master-sector-symmetry-assistance",
                 "--no-master-sector-symmetry-assistance"),
                ("--no-sector-symmetry-assistance", "--no-master-sector-symmetry-assistance",
                 "--master-sector-symmetry-assistance")):
            with self.subTest(supplied=supplied):
                command = self.invoke("refine", supplied)
                self.assertEqual(command.count(forwarded), 1)
                self.assertNotIn(opposite, command)
                self.assertNotIn(supplied, command)
                self.assertIn("--refine-masters", command)
                self.assertIn("--resume", command)
                self.assertIn("--start", command)
                for other in ("saved-rule", "circuit-symmetry", "numerator-symmetry"):
                    self.assertNotIn(f"--master-{other}-assistance", command)
                    self.assertNotIn(f"--no-master-{other}-assistance", command)
                for numerator, numerator_forwarded, numerator_opposite in (
                        ("--numerator-symmetry-assistance", "--master-numerator-symmetry-assistance",
                         "--no-master-numerator-symmetry-assistance"),
                        ("--no-numerator-symmetry-assistance", "--no-master-numerator-symmetry-assistance",
                         "--master-numerator-symmetry-assistance")):
                    with self.subTest(numerator=numerator):
                        combined = self.invoke("refine", supplied, numerator)
                        self.assertEqual(combined.count(forwarded), 1)
                        self.assertNotIn(opposite, combined)
                        self.assertEqual(combined.count(numerator_forwarded), 1)
                        self.assertNotIn(numerator_opposite, combined)
                        self.assertNotIn(supplied, combined)
                        self.assertNotIn(numerator, combined)

    def test_sector_symmetry_assistance_rejects_nonrefinement_actions_before_dispatch(self):
        recipe = self.campaign / "proof-recipe.json"
        for action in ("run", "extend", "inspect", "publish", "finalize-refinement", "import-refinement"):
            options = (["--rank", "1"] if action == "extend" else
                       ["--proof-recipe", str(recipe)] if action == "import-refinement" else [])
            for flag in ("--sector-symmetry-assistance", "--no-sector-symmetry-assistance",
                         "--master-sector-symmetry-assistance", "--no-master-sector-symmetry-assistance"):
                with self.subTest(action=action, flag=flag), \
                     patch.object(WRAPPER, "native_executable", side_effect=AssertionError("native preflight")), \
                     patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    self.invoke(action, *options, flag)

    def test_sector_symmetry_policy_forwards_only_explicit_valid_choices(self):
        self.assertNotIn("--master-sector-symmetry-policy",
                         self.invoke("refine", "--sector-symmetry-assistance"))
        for policy in ("generators", "polynomial-orbits"):
            with self.subTest(policy=policy):
                command = self.invoke("refine", "--sector-symmetry-assistance",
                                      "--sector-symmetry-policy", policy)
                self.assertEqual(command.count("--master-sector-symmetry-policy"), 1)
                self.assertEqual(command[command.index("--master-sector-symmetry-policy") + 1], policy)
                self.assertNotIn("--sector-symmetry-policy", command)
                self.assertIn("--master-sector-symmetry-assistance", command)
                self.assertNotIn("--master-sector-symmetry-saved-projection", command)
                self.assertNotIn("--no-master-sector-symmetry-saved-projection", command)
                self.assertIn("--refine-masters", command)
        with patch.object(WRAPPER, "native_executable", side_effect=AssertionError("native preflight")), \
             patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
            self.invoke("refine", "--sector-symmetry-assistance", "--sector-symmetry-policy", "unknown")

    def test_sector_symmetry_policy_rejects_nonrefinement_actions_before_dispatch(self):
        recipe = self.campaign / "proof-recipe.json"
        for action in ("run", "extend", "inspect", "publish", "finalize-refinement", "import-refinement"):
            options = (["--rank", "1"] if action == "extend" else
                       ["--proof-recipe", str(recipe)] if action == "import-refinement" else [])
            for flag in ("--sector-symmetry-policy", "--master-sector-symmetry-policy"):
                for policy in ("generators", "polynomial-orbits"):
                    with self.subTest(action=action, flag=flag, policy=policy), \
                         patch.object(WRAPPER, "native_executable", side_effect=AssertionError("native preflight")), \
                         patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                        self.invoke(action, *options, flag, policy)

    def test_sector_symmetry_saved_projection_forwards_both_choices_independently(self):
        for supplied, forwarded, opposite in (
                ("--sector-symmetry-saved-projection", "--master-sector-symmetry-saved-projection",
                 "--no-master-sector-symmetry-saved-projection"),
                ("--no-sector-symmetry-saved-projection", "--no-master-sector-symmetry-saved-projection",
                 "--master-sector-symmetry-saved-projection")):
            with self.subTest(supplied=supplied):
                command = self.invoke("refine", supplied)
                self.assertEqual(command.count(forwarded), 1)
                self.assertNotIn(opposite, command)
                self.assertNotIn(supplied, command)
                self.assertNotIn("--master-sector-symmetry-policy", command)
                self.assertIn("--refine-masters", command)
                for other in ("saved-rule", "circuit-symmetry", "numerator-symmetry", "sector-symmetry"):
                    self.assertNotIn(f"--master-{other}-assistance", command)
                    self.assertNotIn(f"--no-master-{other}-assistance", command)
                for policy in ("generators", "polynomial-orbits"):
                    for numerator, numerator_forwarded in (
                            ("--numerator-symmetry-assistance", "--master-numerator-symmetry-assistance"),
                            ("--no-numerator-symmetry-assistance", "--no-master-numerator-symmetry-assistance")):
                        with self.subTest(policy=policy, numerator=numerator):
                            combined = self.invoke("refine", supplied, "--sector-symmetry-assistance",
                                "--sector-symmetry-policy", policy, numerator,
                                "--saved-rule-assistance", "--circuit-symmetry-assistance")
                            self.assertEqual(combined.count(forwarded), 1)
                            self.assertNotIn(opposite, combined)
                            self.assertNotIn(supplied, combined)
                            self.assertEqual(combined[combined.index("--master-sector-symmetry-policy") + 1], policy)
                            for flag in (numerator_forwarded, "--master-sector-symmetry-assistance",
                                         "--master-saved-rule-assistance", "--master-circuit-symmetry-assistance"):
                                self.assertEqual(combined.count(flag), 1)

    def test_sector_symmetry_saved_projection_rejects_nonrefinement_actions_before_dispatch(self):
        recipe = self.campaign / "proof-recipe.json"
        for action in ("run", "extend", "inspect", "publish", "finalize-refinement", "import-refinement"):
            options = (["--rank", "1"] if action == "extend" else
                       ["--proof-recipe", str(recipe)] if action == "import-refinement" else [])
            for flag in ("--sector-symmetry-saved-projection", "--no-sector-symmetry-saved-projection",
                         "--master-sector-symmetry-saved-projection", "--no-master-sector-symmetry-saved-projection"):
                with self.subTest(action=action, flag=flag), \
                     patch.object(WRAPPER, "native_executable", side_effect=AssertionError("native preflight")), \
                     patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    self.invoke(action, *options, flag)

    def test_numerator_symmetry_assistance_rejects_nonrefinement_actions_before_dispatch(self):
        for action in ("run", "extend", "inspect", "publish", "finalize-refinement"):
            options = ["--rank", "1"] if action == "extend" else []
            for flag in ("--numerator-symmetry-assistance", "--no-numerator-symmetry-assistance",
                         "--master-numerator-symmetry-assistance", "--no-master-numerator-symmetry-assistance"):
                with self.subTest(action=action, flag=flag), \
                     patch.object(WRAPPER, "native_executable", side_effect=AssertionError("native preflight")), \
                     patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    self.invoke(action, *options, flag)

    def test_finite_feedback_mode_is_forwarded_only_by_explicit_refinement(self):
        self.assertNotIn("--master-finite-feedback", self.invoke("refine"))
        for supplied, forwarded in (("--finite-feedback", "--master-finite-feedback"),
                                    ("--no-finite-feedback", "--no-master-finite-feedback")):
            self.assertIn(forwarded, self.invoke("refine", supplied))
            for action in ("publish", "inspect"):
                with patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    self.invoke(action, supplied)

    def test_publish_is_only_postprocess_and_inspect_calls_native(self):
        self.assertIn("--publish-only", self.invoke("publish"))
        command = self.invoke("inspect", "--format", "json")
        self.assertEqual(command, [str(self.binary), "artifact-inspect", "--campaign-directory", str(self.campaign), "--format", "json"])

    def test_publication_verification_flags_reach_run_and_publish_only(self):
        for action in ("run", "publish"):
            self.assertNotIn("--deep-verification", self.invoke(action))
            for flag in ("--deep-verification", "--no-deep-verification"):
                self.assertIn(flag, self.invoke(action, flag))
        for action in ("refine", "inspect"):
            with patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                self.invoke(action, "--deep-verification")

    def test_extend_dry_run_prints_wrapper_not_unprepared_resume(self):
        helper = SimpleNamespace(PRESERVE_DIFFERENCE=object(), extend=unittest.mock.Mock(return_value={
            "rank": 1, "max_power_difference": 10, "new_required_queries": 116}))
        output = io.StringIO()
        with patch.object(WRAPPER, "sibling", return_value=helper), patch.object(WRAPPER.sys, "stdout", output):
            self.assertIsNone(self.invoke("extend", "--rank", "1", "--max-power-difference", "10", "--dry-run"))
        self.assertTrue(helper.extend.call_args.kwargs["dry_run"])
        command = output.getvalue().split("Command: ", 1)[1]
        self.assertIn("saved_campaign.py extend", command)
        self.assertNotIn("--dry-run", command)
        self.assertNotIn("production_saved_owner_campaign.py", command)

    def test_extend_publishes_without_implicitly_refining(self):
        helper = SimpleNamespace(PRESERVE_DIFFERENCE=object(), extend=unittest.mock.Mock(return_value={
            "rank": 1, "max_power_difference": 9, "new_required_queries": 67}))
        with patch.object(WRAPPER, "sibling", return_value=helper), patch.object(WRAPPER.sys, "stdout", io.StringIO()):
            command = self.invoke("extend", "--rank", "1", "--deep-verification")
        self.assertFalse(helper.extend.call_args.kwargs["dry_run"])
        self.assertIn("--resume", command)
        self.assertIn("--start", command)
        self.assertNotIn("--refine-masters", command)
        self.assertIn("--deep-verification", command)

    def test_paused_phase_reuses_frozen_binary_despite_new_local_build(self):
        phases = WRAPPER.sibling("campaign_phases")
        root = self.campaign / "master-reduction"
        root.mkdir()
        native = root / "bin/rustred-frozen"
        native.parent.mkdir()
        native.write_text("#!/bin/sh\nexit 0\n")
        native.chmod(0o700)
        phase = root / "scopes/paused"
        phase.mkdir(parents=True)
        (phase / "latest.json").write_text("{}")
        (root / "policy.json").write_text(json.dumps({
            "schema": phases.SCHEMA, "enabled": True, "seed_depth": 0,
            "executable": {"path": "bin/rustred-frozen", "sha256": "previous"}}))
        (root / "active-phase.json").write_text(json.dumps({"operation": "refine", "directory": str(phase)}))
        self.assertEqual(WRAPPER.native_executable(self.campaign), native)
        self.assertEqual(WRAPPER.native_executable(self.campaign, self.binary), self.binary)

    def test_completed_phase_does_not_pin_an_obsolete_refinement_binary(self):
        self.test_paused_phase_reuses_frozen_binary_despite_new_local_build()
        latest = self.campaign / "master-reduction/scopes/paused/latest.json"
        latest.write_text(json.dumps({"status": "completed_nonminimal"}))
        # Simulate an existing repository build without changing real binaries.
        original_is_file = Path.is_file
        current = ROOT / "target/release/rustred"
        with patch.object(Path, "is_file", lambda path: True if path == current else original_is_file(path)), \
                patch.object(WRAPPER.os, "access", return_value=True):
            self.assertEqual(WRAPPER.native_executable(self.campaign), current)


if __name__ == "__main__":
    unittest.main()
