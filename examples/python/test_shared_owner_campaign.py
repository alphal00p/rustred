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
from unittest.mock import patch

SOURCE=Path(__file__).with_name("shared_owner_campaign.py")
SPEC=importlib.util.spec_from_file_location("campaign",SOURCE)
CAMPAIGN=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAMPAIGN)


def fake_process(root, pid, parent, start, children=(), tgid=None, task_children=None):
    directory=root/str(pid)
    directory.mkdir(exist_ok=True)
    fields=["0"]*22
    fields[0]="S"; fields[1]=str(parent); fields[2]=str(pid)
    fields[11]="10"; fields[12]="5"; fields[19]=str(start); fields[21]="3"
    (directory/"stat").write_text(f"{pid} (name with ) space) "+" ".join(fields))
    (directory/"status").write_text(f"Tgid:\t{pid if tgid is None else tgid}\n")
    tasks={pid:children, **(task_children or {})}
    for tid, descendants in tasks.items():
        task=directory/"task"/str(tid)
        task.mkdir(parents=True,exist_ok=True)
        (task/"children").write_text(" ".join(map(str,descendants)))


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
    def test_rooted_collection_threads_overlaps_and_known_reparenting(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            fake_process(root,1,0,11,[99],task_children={17:[2]})
            fake_process(root,2,1,22,[3])
            fake_process(root,3,2,33)
            fake_process(root,99,1,99,tgid=1)  # Never sum a task as another process.
            fake_process(root,9000,0,9000)  # Unrelated host PID must not be visited.
            collector=CAMPAIGN.ProcessTreeCollector(root)
            collector.register(1)
            actual_iterdir=Path.iterdir
            visited=[]
            def bounded_iterdir(path):
                self.assertNotEqual(path,root,"collector scanned all host processes")
                visited.append(path)
                return actual_iterdir(path)
            with patch.object(Path,"iterdir",bounded_iterdir):
                rows,stats=collector.sample()
            self.assertEqual(set(rows),{1,2,3})
            self.assertEqual(stats["task_directories"],3)
            self.assertEqual(stats["children_files"],4)
            self.assertFalse(any("9000" in path.parts for path in visited))
            collector.register(2)  # Explicit overlapping root still counts once.
            self.assertEqual(set(collector.sample()[0]),{1,2,3})
            fake_process(root,2,1,22)
            fake_process(root,3,0,33)  # Observed child remains tracked after reparenting.
            self.assertEqual(set(collector.sample()[0]),{1,2,3})

    def test_rooted_collection_retains_unreadable_but_prunes_dead_and_reused(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            fake_process(root,2,0,22)
            collector=CAMPAIGN.ProcessTreeCollector(root)
            collector.register(2)
            with patch.object(collector,"_stat",side_effect=PermissionError("transient")):
                rows,stats=collector.sample()
            self.assertEqual(rows,{})
            self.assertEqual(collector.identities,{2:22})
            self.assertEqual(stats["read_races"],1)
            self.assertEqual(set(collector.sample()[0]),{2})
            fake_process(root,2,0,222)
            self.assertEqual(collector.sample()[0],{})
            self.assertEqual(collector.identities,{})
            collector.register(2)
            (root/"2").rename(root/"removed")
            self.assertEqual(collector.sample()[0],{})
            self.assertEqual(collector.identities,{})

    def test_rooted_collection_checks_discovery_parent_and_tolerates_task_races(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            fake_process(root,1,0,11,[2,3])
            fake_process(root,2,999,22)  # Child already reparented before first observation.
            fake_process(root,3,1,33)
            (root/"1"/"task"/"disappeared").mkdir()
            (root/"1"/"task"/"18").mkdir()  # No children file after task exit.
            collector=CAMPAIGN.ProcessTreeCollector(root)
            collector.register(1)
            original=collector._stat
            def raced(pid):
                row=original(pid)
                if pid==1 and calls[0]:
                    row["start"]=111
                if pid==1:
                    calls[0]+=1
                return row
            calls=[0]
            with patch.object(collector,"_stat",side_effect=raced):
                rows,stats=collector.sample()
            self.assertEqual(set(rows),{1})
            self.assertEqual(set(collector.identities),{1})
            self.assertGreaterEqual(stats["read_races"],1)

    def test_per_process_cpu_deltas_separate_roles_and_do_not_charge_history(self):
        def row(parent,start,cpu):
            return {"ppid":parent,"start":start,"cpu_seconds":cpu,"rss_bytes":4096}
        table={1:row(0,11,100.2),2:row(1,22,52),3:row(2,333,900)}
        delta,current,rows=CAMPAIGN.process_cpu_sample(
            table,{(1,11):100,(2,22):50,(3,33):1},2,1,2)
        self.assertAlmostEqual(delta,2.2)
        self.assertEqual([r["role"] for r in rows],["supervisor","owned_native","registered_descendant"])
        self.assertAlmostEqual(rows[0]["observed_busy_cores"],0.1)
        self.assertEqual(rows[1]["observed_busy_cores"],1)
        self.assertIsNone(rows[2]["sampled_cpu_delta_seconds"])
        self.assertNotIn((3,33),current)

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
            policy=directory/"limits.json"; policy.write_text('{"max_native_polynomial_operations":1234}')
            entries=directory/"entries.json"; entries.write_text('{"schema":"rustred.owner-domain-queries.json.v2","queries":[]}')
            result=subprocess.run([sys.executable,str(SOURCE),"--executable",str(child),"--manifest",str(manifest),
                "--targets",str(targets),"--workers","1","--soft-memory-bytes","1","--sample-seconds","0.1",
                "--tmp-root",str(directory/"receipts"),"--no-progress","--expansion-limits",str(policy),
                "--entry-domains",str(entries)],capture_output=True,text=True,timeout=10)
            self.assertEqual(result.returncode,4,result.stderr)
            receipt=next((directory/"receipts").iterdir())
            summary=json.loads((receipt/"supervisor-result.json").read_text())
            request=json.loads((receipt/"request.json").read_text())
            self.assertEqual(summary["operator_or_resource_stop"],"aggregate_rss_soft_limit")
            self.assertFalse(summary["family_closure_claim"])
            self.assertFalse(summary["work_checkpoint"])
            self.assertEqual(request["input_scope"], "concrete_targets")
            self.assertFalse(request["reuse_initial_d_bands"])
            self.assertFalse(summary["reuse_initial_d_bands"])
            self.assertNotIn("--" + CAMPAIGN.DOMAIN.INITIAL_D_REUSE, request["command"])
            self.assertIsNone(request["hard_timeout"])
            option=request["command"].index("--expansion-limits")
            self.assertEqual(request["command"][option+1],str(policy.resolve()))
            option=request["command"].index("--entry-domains")
            self.assertEqual(request["command"][option+1],str(entries.resolve()))
            self.assertNotIn("SYMBOLICA_LICENSE",(receipt/"request.json").read_text())
            self.assertIn(str(request["supervisor_pid"]),request["registered_roots"])
            self.assertGreater(request["child_rlimit_as_bytes"],0)
            resources=[json.loads(line) for line in (receipt/"resources.jsonl").read_text().splitlines()]
            self.assertTrue(resources)
            self.assertEqual({row["role"] for row in resources[0]["process_cpu"]},
                             {"supervisor","owned_native"})
            self.assertEqual(resources[0]["processes"],2)
            self.assertIn("stat_reads",resources[0]["collection"])

    def test_global_workers_rejected_before_launch(self):
        result=subprocess.run([sys.executable,str(SOURCE),"--executable","missing","--manifest","missing",
            "--targets","missing","--workers","50","--other-workers","1"],capture_output=True,text=True)
        self.assertEqual(result.returncode,2)
        self.assertIn("aggregate",result.stderr)

    def test_symbolic_supervisor_reuses_native_controls_and_resource_stop(self):
        for explicit in (False, True, "unlimited"):
            with self.subTest(explicit=explicit), tempfile.TemporaryDirectory() as temporary:
                directory=Path(temporary)
                child=directory/"fake-rustred"
                child.write_text(f"#!{sys.executable}\nimport os,sys,time\nfrom pathlib import Path\n"
                    "assert sys.argv[1]=='owner-domain-match'\n"
                    "assert '--follow-successors' in sys.argv and '--targets' not in sys.argv\n"
                    "assert os.environ['RAYON_NUM_THREADS']=='1'\n"
                    "stop=Path(sys.argv[sys.argv.index('--stop-file')+1])\n"
                    "while not stop.exists(): time.sleep(.02)\nraise SystemExit(4)\n")
                child.chmod(0o700)
                manifest=directory/"selection.json"; manifest.write_text("{}")
                queries=directory/"queries.json"; queries.write_text("{}")
                command=[sys.executable,str(SOURCE),"--executable",str(child),"--manifest",str(manifest),
                    "--queries",str(queries),"--workers","1","--soft-memory-bytes","1",
                    "--sample-seconds","0.1","--tmp-root",str(directory/"receipts"),"--no-progress"]
                expected={}
                if explicit:
                    command += ["--route-domain-overcover"]
                    axes = "finite-axes" if explicit == "unlimited" else "inactive-only"
                    command += ["--" + CAMPAIGN.DOMAIN.REFINEMENT_AXES, axes]
                    for index, option in enumerate(CAMPAIGN.SYMBOLIC_ALLOWANCES, 2):
                        value="0" if option==CAMPAIGN.DOMAIN.REFINEMENT else str(index)
                        if option == "max-containment-checks" and explicit == "unlimited":
                            value = "unlimited"
                        command += ["--"+option,value]
                        expected[option]=value
                if explicit == "unlimited":
                    command += ["--" + CAMPAIGN.DOMAIN.TRANSFER_LOOKAHEAD, "50"]
                    command += ["--" + CAMPAIGN.DOMAIN.INITIAL_D_REUSE]
                result=subprocess.run(command,capture_output=True,text=True,timeout=10)
                self.assertEqual(result.returncode,4,result.stderr)
                receipt=next((directory/"receipts").iterdir())
                request=json.loads((receipt/"request.json").read_text())
                summary=json.loads((receipt/"supervisor-result.json").read_text())
                actual=request["command"]
                self.assertEqual(request["input_scope"],"symbolic_domains")
                self.assertIsNone(request["hard_timeout"])
                self.assertEqual(request["workers"],1)
                self.assertEqual(request["hard_memory_bytes"],500_000_000_000)
                self.assertGreater(request["child_rlimit_as_bytes"],0)
                self.assertEqual(summary["operator_or_resource_stop"],"aggregate_rss_soft_limit")
                self.assertFalse(summary["family_closure_claim"])
                self.assertEqual(actual[actual.index("--queries")+1],str(queries.resolve()))
                self.assertEqual("--route-domain-overcover" in actual,bool(explicit))
                for option in CAMPAIGN.SYMBOLIC_ALLOWANCES:
                    if explicit:
                        self.assertEqual(actual[actual.index("--"+option)+1],expected[option])
                    else:
                        self.assertNotIn("--"+option,actual)
                axes_option = "--" + CAMPAIGN.DOMAIN.REFINEMENT_AXES
                if explicit:
                    self.assertEqual(actual[actual.index(axes_option)+1], axes)
                else:
                    self.assertNotIn(axes_option, actual)
                transfer_option = "--" + CAMPAIGN.DOMAIN.TRANSFER_LOOKAHEAD
                reuse_option = "--" + CAMPAIGN.DOMAIN.INITIAL_D_REUSE
                self.assertEqual(request["reuse_initial_d_bands"], explicit == "unlimited")
                self.assertEqual(summary["reuse_initial_d_bands"], explicit == "unlimited")
                self.assertEqual(actual.count(reuse_option), int(explicit == "unlimited"))
                if explicit == "unlimited":
                    self.assertEqual(actual[actual.index(transfer_option)+1], "50")
                else:
                    self.assertNotIn(transfer_option, actual)
                for option in CAMPAIGN.FINITE_ALLOWANCES:
                    self.assertNotIn("--"+option,actual)

    def test_symbolic_and_concrete_scope_controls_cannot_be_mixed(self):
        base=[sys.executable,str(SOURCE),"--executable","missing","--manifest","missing","--workers","1"]
        for scope, flags, diagnostic in [
            ("--queries",["--max-nodes","7"],"require --targets"),
            ("--queries",["--expansion-limits","missing"],"require --targets"),
            ("--queries",["--entry-domains","missing"],"require --targets"),
            ("--targets",["--max-frontiers","17"],"require --queries"),
            ("--targets",["--bounded-refinement-axes","finite-axes"],"require --queries"),
            ("--queries",["--bounded-refinement-axes","all"],"invalid choice"),
            ("--targets",["--max-containment-checks","unlimited"],"require --queries"),
            ("--targets",["--transfer-unreserved-lookahead","50"],"require --queries"),
            ("--targets",["--reuse-initial-d-bands"],"require --queries"),
            ("--queries",["--reuse-initial-d-bands"],"requires --transfer-unreserved-lookahead"),
            ("--queries",["--transfer-unreserved-lookahead","50","--reuse-initial-d-bands","--max-containment-checks","99"],"unlimited containment checks"),
            ("--queries",["--transfer-unreserved-lookahead","50","--reuse-initial-d-bands","--reuse-initial-d-bands"],"only once"),
            ("--queries",["--transfer-unreserved-lookahead","0"],"positive integer"),
            ("--queries",["--transfer-unreserved-lookahead","50","--max-containment-checks","99"],"unlimited containment checks"),
            ("--queries",["--max-containment-checks","0"],"positive integer"),
            ("--queries",["--max-containment-checks","Unlimited"],"positive integer"),
            ("--targets",["--route-domain-overcover"],"require --queries"),
            ("--queries",["--max-route-masks-per-query","17"],"requires --route-domain-overcover"),
            ("--queries",["--targets","missing"],"not allowed"),
        ]:
            result=subprocess.run(base+[scope,"missing"]+flags,capture_output=True,text=True)
            self.assertEqual(result.returncode,2,result.stderr)
            self.assertIn(diagnostic,result.stderr)


if __name__=="__main__": unittest.main()
