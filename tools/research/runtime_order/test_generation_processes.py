"""Small real subprocess lifecycle tests; Python sleepers only, never CAS.

Run explicitly on reserved free CPUs with RUSTRED_TEST_GENERATION_PROCESS_CPUS
set. Ordinary discovery skips them so it cannot borrow a production CPU.
"""
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import time
import unittest

import generation_scheduler as scheduler


@unittest.skipUnless(os.environ.get("RUSTRED_TEST_GENERATION_PROCESS_CPUS"), "explicit test CPU grant required")
class ProcessLifecycleTests(unittest.TestCase):
    def setUp(self):
        evidence = os.environ.get("RUSTRED_TEST_GENERATION_PROCESS_EVIDENCE")
        if evidence:
            evidence = Path(evidence).resolve()
            if (scheduler.ROOT / "TMP") not in evidence.parents:
                raise ValueError("process-test evidence must stay in workspace TMP")
            evidence.mkdir(parents=True, exist_ok=True)
            self.path = Path(tempfile.mkdtemp(prefix=self._testMethodName+"-", dir=evidence))
        else:
            self.temp = tempfile.TemporaryDirectory(dir=scheduler.ROOT / "TMP")
            self.addCleanup(self.temp.cleanup)
            self.path = Path(self.temp.name)
        cpus = os.environ["RUSTRED_TEST_GENERATION_PROCESS_CPUS"]
        self.resources = dict(workers=len(scheduler.SUPERVISOR.parse_cpu_set(cpus)), cpus=cpus,
            generation_jobs=2, max_memory_bytes=1_000_000_000,
            host_memory_reserve_bytes=500_000_000, ram_guard_margin_percent=5)

    def job(self, name, seconds, mode="sleep"):
        script = (
            "import json,os,signal,time; from pathlib import Path; "
            f"p=Path({str(self.path / (name+'.json'))!r}); "
            "child=os.fork() if " + repr(mode) + "=='forkfail' else -1; "
            "\nif child==0:\n signal.signal(signal.SIGTERM,signal.SIG_IGN)\n time.sleep(60)\n os._exit(0)\n"
            "p.write_text(json.dumps({'pid':os.getpid(),'child':child,'cpus':sorted(os.sched_getaffinity(0))})); "
            "\nif child>0:\n os._exit(7)\n"
            f"if {mode!r}=='wait-c':\n"
            " for _ in range(1000):\n"
            f"  if Path({str(self.path / 'C.json')!r}).exists(): break\n"
            "  time.sleep(0.01)\n"
            " else: raise RuntimeError('queued C never dispatched')\n"
            f"time.sleep({seconds!r})\n"
        )
        return scheduler.Job(name, (sys.executable, "-c", script), {})

    def drained(self, directory):
        result = json.loads((directory / "result.json").read_text())
        self.assertTrue(result["owned_groups_drained"])
        self.assertTrue(all(not scheduler.group_running(pid) for pid in result["process_groups"]))
        return result

    def test_real_slots_refill_without_affinity_overlap(self):
        completed = []
        jobs = [self.job("A",0.04,"wait-c"), self.job("B",0.04), self.job("C",0.04)]
        scheduler.run(jobs, self.path / "run", self.resources,
            lambda job: completed.append(job.id), poll_seconds=0.02, grace_seconds=0.05)
        self.drained(self.path / "run")
        self.assertEqual(completed, ["A","B","C"])
        a,b,c = [json.loads((self.path / (name+".json")).read_text()) for name in "ABC"]
        self.assertTrue(set(a["cpus"]).isdisjoint(b["cpus"]))
        self.assertEqual(b["cpus"], c["cpus"])

    def test_real_sigterm_stops_two_groups_and_does_not_launch_queue(self):
        previous = signal.getsignal(signal.SIGTERM)
        starts = []
        def observe(event):
            if event["event"] == "job_started":
                starts.append(event["snapshot"]["snapshot_seq"])
                if len(starts) == 2:
                    os.kill(os.getpid(), signal.SIGTERM)
        with self.assertRaises(RuntimeError):
            scheduler.run([self.job(name,60) for name in "ABC"], self.path / "signal",
                self.resources, lambda _:None, observer=observe, poll_seconds=0.02, grace_seconds=0.05)
        result = self.drained(self.path / "signal")
        self.assertEqual(result["stop_reason"], "operator_signal_15")
        self.assertEqual(len(result["process_groups"]), 2)
        self.assertFalse((self.path / "C.json").exists())
        self.assertEqual(signal.getsignal(signal.SIGTERM), previous)

    def test_failed_leader_with_lingering_child_stops_sibling_and_queue(self):
        started = time.monotonic()
        with self.assertRaises(RuntimeError):
            scheduler.run([self.job("A",0,"forkfail"), self.job("B",60), self.job("C",60)],
                self.path / "failure", self.resources, lambda _:None,
                poll_seconds=0.02, grace_seconds=0.05)
        result = self.drained(self.path / "failure")
        self.assertIn("native_job_failed:A:exit_7", result["stop_reason"])
        self.assertEqual(len(result["process_groups"]), 2)
        self.assertFalse((self.path / "C.json").exists())
        self.assertLess(time.monotonic()-started, 10)
        child = json.loads((self.path / "A.json").read_text())["child"]
        try:
            status = Path(f"/proc/{child}/stat").read_text().rsplit(")",1)[1].split()[0]
        except FileNotFoundError:
            status = "gone"
        self.assertIn(status, ("gone","Z"))


if __name__ == "__main__":
    unittest.main()
