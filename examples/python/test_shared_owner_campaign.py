"""Fast steering-only tests; no license, native build, algebra or large jobs."""
import importlib.util
import json
import os
import resource
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SOURCE=Path(__file__).with_name("shared_owner_campaign.py")
SPEC=importlib.util.spec_from_file_location("campaign",SOURCE)
CAMPAIGN=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAMPAIGN)


class SteeringTests(unittest.TestCase):
    def test_address_space_envelope_respects_reserves_and_inherited_soft_limit(self):
        self.assertEqual(CAMPAIGN.address_space_envelope(500_000_000_000,0,None,0,(-1,-1)),
                         (480_000_000_000,20_000_000_000))
        self.assertEqual(CAMPAIGN.address_space_envelope(500_000_000_000,0,None,0,(1_000_000,2_000_000))[0],1_000_000)
        for reserve, explicit, rss in [(500_000_000_000,None,0),(1,None,2),(0,490_000_000_000,0)]:
            with self.assertRaises(ValueError):
                CAMPAIGN.address_space_envelope(500_000_000_000,reserve,explicit,rss,(-1,-1))

    def test_native_child_receives_actual_address_space_and_affinity(self):
        with tempfile.TemporaryDirectory() as temporary:
            destination=Path(temporary)/"native.json"
            cap=128*1024*1024
            cpus={next(iter(os.sched_getaffinity(0)))}
            code="import json,os,resource; from pathlib import Path; Path("+repr(str(destination))+").write_text(json.dumps([resource.getrlimit(resource.RLIMIT_AS),sorted(os.sched_getaffinity(0))]))"
            with CAMPAIGN.owned_process([sys.executable,"-c",code],dict(os.environ),cpus,lambda _reason:None,cap) as child:
                self.assertEqual(child.wait(timeout=10),0)
            self.assertEqual(json.loads(destination.read_text()),[[cap,cap],sorted(cpus)])

    def test_external_jobs_need_explicit_memory_reservation(self):
        result=subprocess.run([sys.executable,str(SOURCE),"--executable","missing","--manifest","missing",
            "--targets","missing","--workers","1","--other-workers","1"],capture_output=True,text=True)
        self.assertEqual(result.returncode,2)
        self.assertIn("reserved-other-memory",result.stderr)

    def test_receipt_failure_after_spawn_reaps_owned_child(self):
        child=None
        def stop(_reason):
            child.terminate()
        with self.assertRaisesRegex(OSError,"journal failure"):
            with CAMPAIGN.owned_process([sys.executable,"-c","import time; time.sleep(60)"],dict(os.environ),
                    {next(iter(os.sched_getaffinity(0)))},stop) as child:
                raise OSError("journal failure")
        self.assertIsNotNone(child.returncode)
    def test_overlapping_roots_deduplicate_and_reused_pid_is_excluded(self):
        rows={1:{"ppid":0,"start":11},2:{"ppid":1,"start":22},3:{"ppid":2,"start":33}}
        self.assertEqual(set(CAMPAIGN.selected_tree(rows,{1:11,2:22})),{1,2,3})
        self.assertEqual(CAMPAIGN.selected_tree(rows,{1:999}),{})

    def test_fake_child_soft_stop_writes_incomplete_receipt_without_license(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory=Path(temporary)
            child=directory/"fake-rustred"
            child.write_text(f"#!{sys.executable}\nimport os,sys,time\nfrom pathlib import Path\n"
                "assert os.environ['RAYON_NUM_THREADS']=='1'\n"
                "stop=Path(sys.argv[sys.argv.index('--stop-file')+1])\n"
                "while not stop.exists(): time.sleep(.02)\nraise SystemExit(4)\n")
            child.chmod(0o700)
            manifest=directory/"selection.json"; manifest.write_text("{}")
            targets=directory/"targets.csv"; targets.write_text("1\n")
            result=subprocess.run([sys.executable,str(SOURCE),"--executable",str(child),"--manifest",str(manifest),
                "--targets",str(targets),"--workers","1","--soft-memory-bytes","1","--sample-seconds","0.1",
                "--tmp-root",str(directory/"receipts"),"--no-progress"],capture_output=True,text=True,timeout=10)
            self.assertEqual(result.returncode,4,result.stderr)
            receipt=next((directory/"receipts").iterdir())
            summary=json.loads((receipt/"supervisor-result.json").read_text())
            request=json.loads((receipt/"request.json").read_text())
            self.assertEqual(summary["operator_or_resource_stop"],"aggregate_rss_soft_limit")
            self.assertFalse(summary["family_closure_claim"])
            self.assertFalse(summary["work_checkpoint"])
            self.assertIsNone(request["hard_timeout"])
            self.assertNotIn("SYMBOLICA_LICENSE",(receipt/"request.json").read_text())
            self.assertIn(str(request["supervisor_pid"]),request["registered_roots"])
            self.assertGreater(request["child_rlimit_as_bytes"],0)

    def test_global_workers_rejected_before_launch(self):
        result=subprocess.run([sys.executable,str(SOURCE),"--executable","missing","--manifest","missing",
            "--targets","missing","--workers","50","--other-workers","1"],capture_output=True,text=True)
        self.assertEqual(result.returncode,2)
        self.assertIn("aggregate",result.stderr)


if __name__=="__main__": unittest.main()
