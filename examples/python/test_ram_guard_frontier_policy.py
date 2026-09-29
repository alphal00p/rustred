"""W1.4 operations: host-aware RAM guard and the A10 frontier policy.

Steering-only tests: fake /proc readers and fake native children; no license,
native build or algebra.
"""
import importlib.util
from contextlib import redirect_stderr, redirect_stdout
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).with_name("shared_owner_campaign.py")
SPEC = importlib.util.spec_from_file_location("campaign_guard", SOURCE)
CAMPAIGN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAMPAIGN)
PRODUCTION_SPEC = importlib.util.spec_from_file_location(
    "production_guard", SOURCE.with_name("production_saved_owner_campaign.py"))
PRODUCTION = importlib.util.module_from_spec(PRODUCTION_SPEC)
PRODUCTION_SPEC.loader.exec_module(PRODUCTION)
MONITOR = CAMPAIGN.MONITOR
PAGE = 4096


def fake_proc(root, available_kib, pswpin=None, total_kib=1_200_000_000):
    """A /proc with meminfo and (optionally) vmstat, as host_memory and read_swap_in_pages read them."""
    root.mkdir(parents=True, exist_ok=True)
    (root / "meminfo").write_text(f"MemTotal: {total_kib} kB\nMemAvailable: {available_kib} kB\n")
    vmstat = root / "vmstat"
    if pswpin is None:
        vmstat.unlink(missing_ok=True)
    else:
        vmstat.write_text(f"nr_free_pages 1\npswpin {pswpin}\npswpout 7\n")
    return root


class ProcReaderTests(unittest.TestCase):
    def test_swap_readers_parse_fake_proc_and_tolerate_missing_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            proc = fake_proc(Path(temporary) / "proc", 8_000_000, pswpin=123)
            self.assertEqual(CAMPAIGN.read_swap_in_pages(proc), 123)
            (proc / "vmstat").write_text("pswpin not-a-number\n")
            self.assertIsNone(CAMPAIGN.read_swap_in_pages(proc))
            (proc / "vmstat").write_text("pswpout 1\n")
            self.assertIsNone(CAMPAIGN.read_swap_in_pages(proc))
            (proc / "vmstat").unlink()
            self.assertIsNone(CAMPAIGN.read_swap_in_pages(proc))
            (proc / "42").mkdir()
            (proc / "42" / "status").write_text("Name:\trustred\nVmRSS:\t 100 kB\nVmSwap:\t    2048 kB\n")
            self.assertEqual(CAMPAIGN.process_swap_bytes(42, proc), 2048 * 1024)
            self.assertIsNone(CAMPAIGN.process_swap_bytes(43, proc))
            (proc / "44").mkdir()
            (proc / "44" / "status").write_text("Name:\tchild\nVmSwap:\t    1024 kB\n")
            # The supervised tree's swap: readable members summed, None if none is readable.
            self.assertEqual(CAMPAIGN.tree_swap_bytes([42, 43, 44], proc), 3072 * 1024)
            self.assertIsNone(CAMPAIGN.tree_swap_bytes([43], proc))
            self.assertIsNone(CAMPAIGN.tree_swap_bytes([], proc))
            # host_memory keeps reading the same fake tree (no cgroup below it).
            snapshot = CAMPAIGN.host_memory(proc, Path(temporary) / "no-cgroup")
            self.assertEqual(snapshot["available_bytes"], 8_000_000 * 1024)
            self.assertIsNone(snapshot["zfs_arc_bytes"])
            arcstats = proc / "spl/kstat/zfs/arcstats"
            arcstats.parent.mkdir(parents=True)
            arcstats.write_text("38 1 0x01 148 40256 3191200222 2031450483189601\n"
                                "name                            type data\n"
                                "hits                            4    60513940083\n"
                                "c_min                           4    38030252416\n"
                                "size                            4    163863835592\n")
            self.assertEqual(CAMPAIGN.read_zfs_arc_bytes(proc), 163_863_835_592)
            self.assertEqual(CAMPAIGN.host_memory(proc, Path(temporary) / "no-cgroup")["zfs_arc_bytes"],
                             163_863_835_592)
            arcstats.write_text("size 4 many\n")
            self.assertIsNone(CAMPAIGN.read_zfs_arc_bytes(proc))


class RamGuardTests(unittest.TestCase):
    def guard(self, **overrides):
        policy = {"hard_bytes": 1_000_000, "soft_bytes": 950_000, "floor_bytes": 100_000,
                  "swap_growth_bytes_per_second": 10 * PAGE, "swap_growth_seconds": 10.0, "page_bytes": PAGE}
        policy.update(overrides)
        return CAMPAIGN.RamGuard(**policy)

    def test_rss_and_mem_available_thresholds_keep_todays_semantics(self):
        guard = self.guard()
        self.assertEqual(guard.observe(0, 949_999, 100_001, None)["cooperative"], None)
        self.assertEqual(guard.observe(1, 950_000, 10**9, None)["cooperative"], "aggregate_rss_soft_limit")
        self.assertEqual(guard.observe(2, 1, 100_000, None)["cooperative"], "host_memory_reserve")
        self.assertIsNone(guard.observe(3, 1, 100_000, None)["hard"])
        self.assertEqual(guard.observe(4, 1, 25_000, None)["hard"], "host_memory_emergency")
        decision = guard.observe(5, 1_000_000, None, None)
        self.assertEqual((decision["cooperative"], decision["hard"]),
                         ("aggregate_rss_soft_limit", "aggregate_rss_hard_limit"))
        # An unreadable host reading never trips the host thresholds by itself.
        self.assertEqual(guard.observe(6, 1, None, None), {
            "cooperative": None, "hard": None, "own_swap_bytes": None,
            "own_swap_growth_bytes_per_second": None, "own_swap_growth_sustained_seconds": 0.0,
            "host_swap_in_pages": None, "host_swap_in_bytes_per_second": None})

    def test_default_floor_and_policy_match_the_supervisor_admission(self):
        hard, soft, floor = CAMPAIGN.memory_admission(
            600_000_000_000, None, {"host_total_bytes": 1_130_000_000_000, "available_bytes": 700_000_000_000},
            None, 5.0)
        self.assertEqual(CAMPAIGN.DEFAULT_HOST_MEMORY_RESERVE_BYTES, 50_000_000_000)
        self.assertEqual((hard, floor), (600_000_000_000, 50_000_000_000))
        policy = CAMPAIGN.RamGuard(hard, soft, floor).policy()
        self.assertEqual(policy["host_available_floor_bytes"], 50_000_000_000)
        self.assertEqual(policy["host_available_emergency_bytes"], 12_500_000_000)
        self.assertEqual(policy["soft_rss_bytes"], 570_000_000_000)
        self.assertEqual(policy["swap_growth_stop_bytes_per_second"], CAMPAIGN.DEFAULT_SWAP_GROWTH_STOP_BYTES_PER_SECOND)
        self.assertEqual(policy["swap_growth_stop_seconds"], CAMPAIGN.DEFAULT_SWAP_GROWTH_STOP_SECONDS)
        self.assertEqual(policy["swap_scope"], "supervised_tree_vmswap_growth")
        self.assertEqual(policy["host_swap_in_scope"], "telemetry_only_host_wide_proc_vmstat_pswpin")
        # The admitted hard cap is min(requested, MemAvailable - floor): 600 GB needs 650 GB available.
        host = {"host_total_bytes": 1_130_000_000_000, "available_bytes": 636_000_000_000,
                "host_available_bytes": 636_000_000_000, "zfs_arc_bytes": 326_000_000_000}
        hard, soft, floor = CAMPAIGN.memory_admission(600_000_000_000, None, host, None, 5.0)
        self.assertEqual((hard, soft), (586_000_000_000, 556_700_000_000))
        record = CAMPAIGN.memory_admission_record(600_000_000_000, host, floor, hard, soft)
        self.assertTrue(record["hard_capped_by_available_memory"])
        self.assertEqual(record["effective_hard_memory_bytes"], 586_000_000_000)
        self.assertEqual(record["zfs_arc_bytes"], 326_000_000_000)
        self.assertIn("min(requested hard, available_bytes - host_memory_reserve_bytes)", record["rule"])
        self.assertFalse(CAMPAIGN.memory_admission_record(600_000_000_000, host, floor, 600_000_000_000,
                                                          570_000_000_000)["hard_capped_by_available_memory"])
        # The floor is flat (no host-size fraction) and an explicit floor wins.
        small = {"host_total_bytes": 200_000_000_000, "available_bytes": 150_000_000_000}
        self.assertEqual(CAMPAIGN.memory_admission(600_000_000_000, None, small, None)[::2],
                         (100_000_000_000, 50_000_000_000))
        self.assertEqual(CAMPAIGN.memory_admission(600_000_000_000, None, small, 20_000_000_000)[::2],
                         (130_000_000_000, 20_000_000_000))
        with self.assertRaisesRegex(ValueError, "no campaign headroom"):
            CAMPAIGN.memory_admission(600_000_000_000, None, {**small, "available_bytes": 50_000_000_000}, None)

    def test_sustained_own_swap_growth_stops_only_after_the_whole_window(self):
        guard = self.guard()
        swapped = 1_000 * PAGE

        def sample(now, delta, readable=True, host_pages=None):
            nonlocal swapped
            swapped += delta
            return guard.observe(now, 1, 10**9, swapped if readable else None, host_pages)

        self.assertIsNone(sample(0, 0)["own_swap_growth_bytes_per_second"])  # baseline only
        for second in range(1, 10):  # 20 pages/s >= 10 pages/s for 9 s: not yet
            decision = sample(second, 20 * PAGE)
            self.assertEqual(decision["own_swap_growth_bytes_per_second"], 20 * PAGE)
            self.assertIsNone(decision["cooperative"], second)
        self.assertEqual(decision["own_swap_growth_sustained_seconds"], 9)
        decision = sample(10, 20 * PAGE)
        self.assertEqual(decision["cooperative"], "own_swap_growth_sustained")
        self.assertEqual(decision["own_swap_growth_sustained_seconds"], 10)
        self.assertNotIn("attribution", decision)  # an own signal needs no attribution
        # One slow interval resets the window.
        self.assertIsNone(sample(11, 5 * PAGE)["cooperative"])
        self.assertEqual(guard.high_since, None)
        for second in range(12, 21):
            self.assertIsNone(sample(second, 50 * PAGE)["cooperative"], second)
        # An unreadable reading resets too, and the next reading is a baseline.
        decision = sample(21, 50 * PAGE, readable=False)
        self.assertEqual((decision["cooperative"], decision["own_swap_growth_bytes_per_second"]), (None, None))
        self.assertIsNone(sample(22, 50 * PAGE)["own_swap_growth_bytes_per_second"])
        # Shrinking swap (pages swapped back in) is a zero growth rate, never negative.
        self.assertEqual(sample(23, -10_000 * PAGE)["own_swap_growth_bytes_per_second"], 0)

    def test_host_wide_swap_in_is_telemetry_only(self):
        # Another user's swap-in at 1000x the stop rate for far longer than the
        # window, with the campaign's own swap flat: recorded, never a stop.
        guard = self.guard()
        for second in range(0, 60):
            decision = guard.observe(second, 1, 10**9, 7 * PAGE, second * 10_000)
            self.assertIsNone(decision["cooperative"], second)
        self.assertEqual(decision["host_swap_in_bytes_per_second"], 10_000 * PAGE)
        self.assertEqual(decision["host_swap_in_pages"], 590_000)
        self.assertEqual(decision["own_swap_growth_bytes_per_second"], 0)
        # A backwards host counter (reset) is a zero rate.
        self.assertEqual(guard.observe(60, 1, 10**9, 7 * PAGE, 5)["host_swap_in_bytes_per_second"], 0)

    def test_host_wide_stops_carry_an_own_memory_attribution(self):
        # Another user's job takes the memory: MemAvailable falls 900 kB in 10 s,
        # the campaign's RSS is flat.
        guard = self.guard(attribution_seconds=10.0)
        for second, available in ((0, 1_000_000), (5, 600_000), (10, 400_000)):
            self.assertNotIn("attribution", guard.observe(second, 50_000, available, 0))
        decision = guard.observe(15, 50_000, 100_000, 0)
        self.assertEqual(decision["cooperative"], "host_memory_reserve")
        attribution = decision["attribution"]
        self.assertEqual(attribution["window_seconds"], 10)  # oldest sample at or before now - window
        self.assertEqual((attribution["own_rss_growth_bytes"], attribution["host_available_drop_bytes"]),
                         (0, 500_000))
        self.assertFalse(attribution["own_memory_signal"])
        # The campaign itself grows by most of the drop: attributed.
        guard = self.guard(attribution_seconds=10.0)
        for second, rss, available in ((0, 10_000, 1_000_000), (10, 500_000, 500_000)):
            guard.observe(second, rss, available, 0)
        attribution = guard.observe(20, 800_000, 20_000, 0)["attribution"]
        self.assertEqual(guard.observe(21, 800_000, 20_000, 0)["hard"], "host_memory_emergency")
        self.assertEqual(attribution["own_rss_growth_bytes"], 300_000)
        self.assertEqual(attribution["host_available_drop_bytes"], 480_000)
        self.assertAlmostEqual(attribution["own_share_of_available_drop"], 0.625)
        self.assertTrue(attribution["own_memory_signal"])
        # Own swap growing at the stop rate is an own signal whatever the RSS did.
        guard = self.guard(attribution_seconds=10.0)
        guard.observe(0, 10_000, 1_000_000, 0)
        attribution = guard.observe(1, 10_000, 90_000, 20 * PAGE)["attribution"]
        self.assertTrue(attribution["own_swap_growing"])
        self.assertTrue(attribution["own_memory_signal"])

    def test_swap_guard_can_be_disabled_and_policy_is_validated(self):
        guard = self.guard(swap_growth_bytes_per_second=0)
        for second in range(0, 100):
            self.assertIsNone(guard.observe(second, 1, None, second * 10**6)["cooperative"])
        for overrides in ({"soft_bytes": 1_000_000}, {"soft_bytes": 0}, {"floor_bytes": -1},
                          {"swap_growth_bytes_per_second": -1}, {"swap_growth_seconds": 0.0},
                          {"swap_growth_seconds": float("inf")}):
            with self.subTest(overrides=overrides), self.assertRaises(ValueError):
                self.guard(**overrides)


def fake_child(directory, body):
    child = directory / "fake-rustred"
    child.write_text(f"#!{sys.executable}\nimport json,os,sys,time\nfrom pathlib import Path\n"
                     "arg=lambda name: Path(sys.argv[sys.argv.index(name)+1])\n" + body)
    child.chmod(0o700)
    return child


class SupervisorGuardTests(unittest.TestCase):
    def supervise(self, directory, name, child_body, fakes, *extra):
        """Run the supervisor under test with only its /proc swap readers faked."""
        child = fake_child(directory, child_body)
        manifest = directory / "selection.json"; manifest.write_text("{}")
        targets = directory / "targets.csv"; targets.write_text("1\n")
        run = directory / name
        driver = directory / f"driver-{name}.py"
        driver.write_text(
            "import importlib.util,itertools,sys\n"
            f"spec=importlib.util.spec_from_file_location('campaign',{str(SOURCE)!r})\n"
            "module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)\n"
            + fakes +
            "sys.argv=[module.__file__,*sys.argv[1:]]\n"
            "raise SystemExit(module.main())\n")
        result = subprocess.run([sys.executable, "-W", "error::DeprecationWarning", str(driver),
                                 "--executable", str(child), "--manifest", str(manifest), "--targets", str(targets),
                                 "--workers", "1", "--sample-seconds", "0.1", "--run-directory", str(run),
                                 "--no-progress", *extra],
                                capture_output=True, text=True, timeout=20)
        return result, run

    def test_sustained_own_swap_growth_saves_and_stops_the_native_child(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            result, run = self.supervise(
                directory, "run", "stop=arg('--stop-file')\nwhile not stop.exists(): time.sleep(.02)\n"
                "raise SystemExit(4)\n",
                "swapped=itertools.count(0,10**9)\n"
                "module.tree_swap_bytes=lambda pids, proc_root=None: next(swapped)\n"
                "module.read_swap_in_pages=lambda proc_root=None: 5\n",
                "--swap-growth-stop-seconds", "0.3")
            self.assertEqual(result.returncode, 4, result.stderr)
            self.assertNotIn("DeprecationWarning", result.stderr)
            summary = json.loads((run / "supervisor-result.json").read_text())
            self.assertEqual(summary["operator_or_resource_stop"], "own_swap_growth_sustained")
            self.assertEqual(summary["ram_guard_stop"]["reason"], "own_swap_growth_sustained")
            self.assertFalse(summary["ram_guard_stop"]["host_wide"])
            self.assertFalse(summary["hard_stopped"])
            self.assertEqual(summary["ram_guard"]["swap_growth_stop_seconds"], 0.3)
            request = json.loads((run / "request.json").read_text())
            self.assertEqual(request["ram_guard"]["swap_scope"], "supervised_tree_vmswap_growth")
            self.assertEqual(request["memory_admission"]["effective_hard_memory_bytes"],
                             request["effective_hard_memory_bytes"])
            self.assertIn("rule", request["memory_admission"])
            self.assertEqual(summary["memory_admission"], request["memory_admission"])
            self.assertIsNone(request["frontier_policy"])  # concrete targets have no frontier policy
            rows = [json.loads(line) for line in (run / "resources.jsonl").read_text().splitlines()]
            self.assertIsNone(rows[0]["own_swap_growth_bytes_per_second"])
            self.assertTrue(any((row["own_swap_growth_bytes_per_second"] or 0) > 10**9 for row in rows))
            self.assertTrue(any(row["own_swap_growth_sustained_seconds"] >= 0.3 for row in rows))
            self.assertIn("native_swap_bytes", rows[0])
            for row in rows:
                self.assertIn("zfs_arc_bytes", row)
                self.assertIn("host_swap_in_bytes_per_second", row)
                self.assertIsInstance(row["host_available_bytes"], int)
            self.assertIsInstance(summary["last_host_available_bytes"], int)
            self.assertIsInstance(summary["min_observed_host_available_bytes"], int)
            self.assertIn("last_zfs_arc_bytes", summary)
            self.assertIn("zfs_arc_bytes", summary["initial_host_memory"])
            status = json.loads((run / "status.json").read_text())
            self.assertEqual(status["stop_reason"], "own_swap_growth_sustained")
            self.assertEqual(status["ram_guard"], summary["ram_guard"])
            self.assertEqual(status["ram_guard_stop"], summary["ram_guard_stop"])

    def test_host_wide_swap_in_never_stops_the_native_child(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            result, run = self.supervise(
                directory, "run", "time.sleep(1.2)\nraise SystemExit(0)\n",
                "pages=itertools.count(0,10**7)\n"
                "module.read_swap_in_pages=lambda proc_root=None: next(pages)\n"
                "module.tree_swap_bytes=lambda pids, proc_root=None: 4096\n",
                "--swap-growth-stop-seconds", "0.3")
            self.assertEqual(result.returncode, 0, result.stderr)
            summary = json.loads((run / "supervisor-result.json").read_text())
            self.assertIsNone(summary["operator_or_resource_stop"])
            self.assertIsNone(summary["ram_guard_stop"])
            self.assertEqual(summary["state"], "completed")
            self.assertGreater(summary["last_host_swap_in_bytes_per_second"], 10**9)
            self.assertEqual(summary["last_own_swap_growth_bytes_per_second"], 0)

    def test_frontier_stop_is_forwarded_recorded_and_reported_as_a_paused_native_stop(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            child = fake_child(directory, """
assert sys.argv[sys.argv.index('--frontier-policy')+1]=='stop'
events=arg('--events'); output=arg('--output'); checkpoint=arg('--checkpoint')
checkpoint.mkdir(parents=True)
saved={'state':'saved','directory':str(checkpoint),'generation':2,'paused':True,'stop_reason':'frontier_policy',
       'saved_unix_time':int(time.time())}
finished={'event':'finished','status':'paused','stop_reason':'frontier_policy','frontier_policy':'stop',
          'frontiers':1,'checkpoint':saved,'family_closure_claim':False}
events.write_text(json.dumps({'event':'frontier_stop','stop_reason':'frontier_policy','frontiers':1})+'\\n'
                  +json.dumps(finished)+'\\n')
output.write_text(json.dumps(finished))
time.sleep(.3)
raise SystemExit(4)
""")
            manifest = directory / "selection.json"; manifest.write_text("{}")
            queries = directory / "queries.json"; queries.write_text("{}")
            run = directory / "run"
            base = [sys.executable, str(SOURCE), "--executable", str(child), "--manifest", str(manifest),
                    "--queries", str(queries), "--workers", "1", "--sample-seconds", "0.1", "--no-progress"]
            refused = subprocess.run([*base, "--frontier-policy", "stop", "--run-directory", str(directory / "no")],
                                     capture_output=True, text=True, timeout=10)
            self.assertEqual(refused.returncode, 2)
            self.assertIn("--frontier-policy stop requires --checkpoint or --resume", refused.stderr)
            self.assertFalse((directory / "no").exists())
            result = subprocess.run([*base, "--frontier-policy", "stop", "--checkpoint", str(directory / "checkpoint"),
                                     "--run-directory", str(run)], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 4, result.stderr)
            request = json.loads((run / "request.json").read_text())
            summary = json.loads((run / "supervisor-result.json").read_text())
            status = json.loads((run / "status.json").read_text())
            self.assertEqual(request["frontier_policy"], "stop")
            self.assertEqual(request["command"].count("--frontier-policy"), 1)
            self.assertEqual(summary["frontier_policy"], "stop")
            self.assertEqual(summary["native_stop_reason"], "frontier_policy")
            self.assertIsNone(summary["operator_or_resource_stop"])
            self.assertEqual(summary["state"], "paused")
            self.assertEqual(status["native_stop_reason"], "frontier_policy")
            restart = status["resume_command"]
            self.assertEqual(restart[restart.index("--frontier-policy") + 1], "stop")
            text = "\n".join(MONITOR.dashboard(status))
            self.assertIn("PAUSED · stop frontier_policy", text)

    def test_thin_driver_validates_the_frontier_policy(self):
        driver = SOURCE.with_name("match_shared_owner_domains.py")
        base = [sys.executable, str(driver), "--executable", "missing", "--manifest", "m", "--queries", "q",
                "--output", "o"]
        for extra, fragment in ((["--frontier-policy", "stop"], "require --follow-successors"),
                                (["--follow-successors", "--frontier-policy", "stop"], "requires --checkpoint"),
                                (["--follow-successors", "--frontier-policy", "halt"], "invalid choice")):
            with self.subTest(extra=extra):
                result = subprocess.run([*base, *extra], capture_output=True, text=True, timeout=10)
                self.assertEqual(result.returncode, 2)
                self.assertIn(fragment, result.stderr)


def production_fixture(directory):
    inputs = directory / "inputs"; inputs.mkdir()
    (inputs / "selection.json").write_text("{}")
    (inputs / "queries.json").write_text(json.dumps({
        "schema": "rustred.owner-domain-queries.json.v2", "queries": [{"id":"p"}],
        "query_roles":{"required":["p"],"auxiliary":[]}}))
    (inputs / "input-receipt.json").write_text(json.dumps({
        "selection_sha256": PRODUCTION.digest(inputs / "selection.json"),
        "queries_sha256": PRODUCTION.digest(inputs / "queries.json"), "owners": []}))
    executable = directory / "fake-rustred"
    executable.write_text(f"#!{sys.executable}\nraise SystemExit('must not execute')\n")
    executable.chmod(0o700)
    return executable


def production_plan(directory, *options):
    output = io.StringIO()
    with redirect_stdout(output):
        result = PRODUCTION.main(["--campaign-directory", str(directory), "--json", *options])
    assert result == 0
    return json.loads(output.getvalue())


class ProductionPolicyTests(unittest.TestCase):
    def test_new_campaigns_freeze_frontier_stop_and_the_ram_guard(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); executable = production_fixture(directory)
            plan = production_plan(directory, "--executable", str(executable), "--workers", "1")
            policy = plan["steering_policy"]
            self.assertEqual(policy["schema"], "rustred.production-steering.v5")
            options = policy["options"]
            self.assertEqual(options["frontier_policy"], "stop")
            self.assertEqual(plan["frontier_policy"], "stop")
            self.assertEqual(options["host_memory_reserve_bytes"], CAMPAIGN.DEFAULT_HOST_MEMORY_RESERVE_BYTES)
            self.assertEqual(options["swap_growth_stop_bytes_per_second"],
                             CAMPAIGN.DEFAULT_SWAP_GROWTH_STOP_BYTES_PER_SECOND)
            self.assertEqual(options["swap_growth_stop_seconds"], CAMPAIGN.DEFAULT_SWAP_GROWTH_STOP_SECONDS)
            command = policy["command_arguments"]
            self.assertEqual(command[command.index("--frontier-policy") + 1], "stop")
            self.assertEqual(command[command.index("--host-memory-reserve-bytes") + 1], "50000000000")
            self.assertEqual(command[command.index("--swap-growth-stop-seconds") + 1], "120.0")
            # The plan previews the supervisor's admission against the host now.
            preview = plan["memory_admission_preview"]
            self.assertIn("min(requested hard, available_bytes - host_memory_reserve_bytes)", preview["rule"])
            if "error" not in preview:
                self.assertEqual(preview["effective_hard_memory_bytes"],
                                 min(500_000_000_000, preview["available_bytes"] - 50_000_000_000))
            # The supervisor accepts exactly this frozen argv.
            self.assertEqual(plan["command"][plan["command"].index("--checkpoint") - 2], "--run-directory")
            # Frozen: a changed frontier policy needs a new campaign; RAM options may be overridden.
            for options_ in (("--resume", "--frontier-policy", "record"),):
                with self.subTest(options=options_), redirect_stderr(io.StringIO()) as errors, \
                        self.assertRaises(SystemExit):
                    production_plan(directory, *options_)
                self.assertIn("differs from frozen policy", errors.getvalue())
            self.assertEqual(production_plan(directory, "--resume", "--frontier-policy", "stop")["steering_policy"],
                             policy)
            resumed = production_plan(directory, "--resume", "--host-memory-reserve-bytes", "30000000000",
                                      "--swap-growth-stop-bytes-per-second", "0", "--swap-growth-stop-seconds", "60")
            self.assertEqual(resumed["supervisor_ram_overrides"], {
                "host_memory_reserve_bytes": 30_000_000_000, "swap_growth_stop_bytes_per_second": 0,
                "swap_growth_stop_seconds": 60.0})
            command = resumed["command"]
            self.assertEqual(command[command.index("--host-memory-reserve-bytes") + 1], "30000000000")
            self.assertEqual(command[command.index("--swap-growth-stop-bytes-per-second") + 1], "0")
            self.assertEqual(command[command.index("--swap-growth-stop-seconds") + 1], "60.0")
            self.assertEqual(resumed["steering_policy"], policy)

    def test_explicit_record_and_invalid_guard_options(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); executable = production_fixture(directory)
            plan = production_plan(directory, "--executable", str(executable), "--workers", "1",
                                   "--frontier-policy", "record", "--host-memory-reserve-bytes", "25000000000")
            command = plan["steering_policy"]["command_arguments"]
            self.assertEqual(command[command.index("--frontier-policy") + 1], "record")
            self.assertEqual(command[command.index("--host-memory-reserve-bytes") + 1], "25000000000")
        for options, fragment in ((["--host-memory-reserve-bytes", "0"], "host memory floor"),
                                  (["--swap-growth-stop-bytes-per-second", "-1"], "swap-growth rate"),
                                  (["--swap-growth-stop-seconds", "0"], "swap-growth window"),
                                  (["--swap-growth-stop-seconds", "inf"], "swap-growth window"),
                                  (["--frontier-policy", "halt"], "invalid choice")):
            with self.subTest(options=options), patch.object(PRODUCTION, "verify_inputs") as verify, \
                    redirect_stderr(io.StringIO()) as errors, self.assertRaises(SystemExit) as error:
                PRODUCTION.main(options)
            self.assertEqual(error.exception.code, 2)
            self.assertIn(fragment, errors.getvalue())
            verify.assert_not_called()

    def test_steering_written_before_a10_keeps_record_and_supervisor_defaults(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); executable = production_fixture(directory)
            plan = production_plan(directory, "--executable", str(executable), "--workers", "1")
            legacy = json.loads((directory / "bin/steering.json").read_text())
            legacy["schema"] = "rustred.production-steering.v2"
            command = legacy["command_arguments"]
            for flag in ("--frontier-policy", "--host-memory-reserve-bytes", "--swap-growth-stop-bytes-per-second",
                         "--swap-growth-stop-seconds", "--helper-id-prefix", "--max-rescues", "--amendments-directory"):
                index = command.index(flag)
                del command[index:index + 2]
            command.remove("--auto-rescue")
            for name in ("frontier_policy", *PRODUCTION.OPTIONAL_RAM_POLICY_OPTIONS, *PRODUCTION.RESCUE_OPTIONS):
                legacy["options"].pop(name)
            path = directory / "bin/steering.json"
            path.chmod(0o644)
            PRODUCTION.write_json(path, legacy)
            options = PRODUCTION.frozen_options(legacy)
            self.assertEqual(options["frontier_policy"], "record")
            self.assertIsNone(options["swap_growth_stop_seconds"])
            self.assertFalse(options["auto_rescue"])
            resumed = production_plan(directory, "--resume")
            self.assertNotIn("--frontier-policy", resumed["command"])
            self.assertNotIn("--auto-rescue", resumed["command"])
            self.assertEqual(resumed["frontier_policy"], "record")
            # A RAM guard override on legacy steering appends the flag once.
            resumed = production_plan(directory, "--resume", "--swap-growth-stop-seconds", "30")
            self.assertEqual(resumed["command"].count("--swap-growth-stop-seconds"), 1)
            self.assertEqual(resumed["steering_policy"], legacy)
            with redirect_stderr(io.StringIO()) as errors, self.assertRaises(SystemExit):
                production_plan(directory, "--resume", "--frontier-policy", "stop")
            self.assertIn("differs from frozen policy", errors.getvalue())
            self.assertEqual(plan["frontier_policy"], "stop")


def fake_run(campaign, name, started, stop, progress, state="saved", own=None):
    run = campaign / "runs" / name
    run.mkdir(parents=True)
    (run / "request.json").write_text("{}")
    os.utime(run / "request.json", (started, started))
    checkpoint = None if progress is None else dict(zip(PRODUCTION.PROGRESS_KEYS, progress), state=state)
    result = {"operator_or_resource_stop": stop, "checkpoint": checkpoint, "state": "paused"}
    if own is not None:
        result["ram_guard_stop"] = {"reason": stop, "host_wide": True, "own_memory_signal": own}
    (run / "supervisor-result.json").write_text(json.dumps(result))
    (run / "run.status").write_text("4\n")
    return run


class RamGuardLivenessTests(unittest.TestCase):
    def test_streak_counts_only_trailing_zero_progress_ram_stops(self):
        with tempfile.TemporaryDirectory() as temporary:
            campaign = Path(temporary)
            self.assertEqual(PRODUCTION.ram_guard_liveness(campaign)["consecutive_zero_progress_ram_stops"], 0)
            fake_run(campaign, "a", 100, "operator_signal_2", (10, 5, 50))
            fake_run(campaign, "b", 200, "aggregate_rss_soft_limit", (20, 9, 80))  # progress: not counted
            fake_run(campaign, "c", 300, "host_memory_reserve", (20, 9, 80), own=True)
            report = PRODUCTION.ram_guard_liveness(campaign)
            self.assertEqual(report["consecutive_zero_progress_ram_stops"], 1)
            self.assertEqual(report["stop_reasons"], ["host_memory_reserve"])
            self.assertEqual(report["progress"], {"committed_domains": 20, "completed_native_inspections": 9,
                                                  "committed_events": 80})
            fake_run(campaign, "d", 400, "aggregate_rss_hard_limit", (20, 9, 80))
            self.assertEqual(PRODUCTION.ram_guard_liveness(campaign)["consecutive_zero_progress_ram_stops"], 2)
            # Ordering is by start time, not by name.
            fake_run(campaign, "0-late", 500, "own_swap_growth_sustained", (20, 9, 80))
            self.assertEqual(PRODUCTION.ram_guard_liveness(campaign)["consecutive_zero_progress_ram_stops"], 3)
            # A host-wide stop not attributed to the campaign (other users' memory
            # on a shared host, or a receipt without attribution, e.g. the retired
            # host-wide swap-in reason) ends the streak like any other stop.
            for name, started, stop, own in (("h1", 510, "host_memory_reserve", False),
                                             ("h2", 520, "host_memory_emergency", None),
                                             ("h3", 530, "host_swap_in_sustained", None)):
                with self.subTest(stop=stop, own=own):
                    fake_run(campaign, name, started, stop, (20, 9, 80), own=own)
                    self.assertEqual(PRODUCTION.ram_guard_liveness(campaign)["consecutive_zero_progress_ram_stops"],
                                     0)
            fake_run(campaign, "h4", 540, "host_memory_emergency", (20, 9, 80), own=True)
            self.assertEqual(PRODUCTION.ram_guard_liveness(campaign)["stop_reasons"], ["host_memory_emergency"])
            # A non-RAM stop, a run with progress or an unsaved checkpoint ends the streak.
            for name, started, stop, progress in (("e", 600, "operator_signal_2", (20, 9, 80)),
                                                  ("f", 700, "aggregate_rss_soft_limit", (21, 9, 80))):
                fake_run(campaign, name, started, stop, progress)
                self.assertEqual(PRODUCTION.ram_guard_liveness(campaign)["consecutive_zero_progress_ram_stops"], 0)
            fake_run(campaign, "g", 800, "aggregate_rss_soft_limit", None)
            self.assertEqual(PRODUCTION.ram_guard_liveness(campaign)["consecutive_zero_progress_ram_stops"], 0)

    def test_launcher_refuses_a_looping_resume_unless_overridden(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); executable = production_fixture(directory)
            production_plan(directory, "--executable", str(executable), "--workers", "1")
            fake_run(directory, "a", 100, "operator_signal_2", (10, 5, 50))
            fake_run(directory, "b", 200, "aggregate_rss_soft_limit", (10, 5, 50))
            plan = production_plan(directory, "--resume")
            self.assertEqual(plan["ram_guard_liveness"]["consecutive_zero_progress_ram_stops"], 1)
            self.assertEqual(plan["ram_guard_liveness"]["limit"], 2)
            fake_run(directory, "c", 300, "host_memory_reserve", (10, 5, 50), own=True)
            with redirect_stderr(io.StringIO()) as errors, self.assertRaises(SystemExit) as error:
                production_plan(directory, "--resume")
            self.assertEqual(error.exception.code, 2)
            self.assertIn("stopped by the RAM guard", errors.getvalue())
            self.assertFalse((directory / "active-run.json").exists())
            self.assertEqual(production_plan(directory, "--resume", "--max-zero-progress-ram-stops", "3")
                             ["ram_guard_liveness"]["consecutive_zero_progress_ram_stops"], 2)
            self.assertIn("ram_guard_liveness", production_plan(directory, "--resume",
                                                                "--max-zero-progress-ram-stops", "0"))
            # A fresh preparation never consults it.
            self.assertNotIn("ram_guard_liveness", production_plan(directory))
            with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                production_plan(directory, "--resume", "--max-zero-progress-ram-stops", "-1")


if __name__ == "__main__":
    unittest.main()
