"""Fast steering-only tests; no license, native build, algebra or large jobs."""
import importlib.util
import json
import os
import resource
import signal
from pathlib import Path
import subprocess
import sys
import tempfile
import time
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
    def test_default_ram_guard_stops_at_95_percent_of_effective_ceiling(self):
        host = {"host_total_bytes": 1_200_000_000_000, "available_bytes": 730_000_000_000}
        self.assertEqual(CAMPAIGN.memory_admission(500_000_000_000, None, host, None),
                         (500_000_000_000, 475_000_000_000, 20_000_000_000))
        constrained = {"host_total_bytes": 1_200_000_000_000, "available_bytes": 800_000_000,
                       "cgroup_capacity_bytes": 1_000_000_000}
        self.assertEqual(CAMPAIGN.memory_admission(500_000_000_000, None, constrained, None),
                         (750_000_000, 712_500_000, 50_000_000))
        self.assertEqual(CAMPAIGN.memory_admission(500_000_000_000, 100, host, None)[1], 100)
        with self.assertRaisesRegex(ValueError, "no campaign headroom"):
            CAMPAIGN.memory_admission(500_000_000_000, None, host, 800_000_000_000)

    def test_host_ram_monitor_honors_enclosing_cgroup_limit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            proc = root / "proc"
            (proc / "self").mkdir(parents=True)
            (proc / "meminfo").write_text("MemTotal: 12000000 kB\nMemAvailable: 8000000 kB\n")
            (proc / "self/cgroup").write_text("0::/parent/child\n")
            cgroup = root / "cgroup"
            (cgroup / "parent/child").mkdir(parents=True)
            for directory, maximum, current in ((cgroup, "8000000000", "1000000000"),
                    (cgroup / "parent", "4000000000", "1000000000"),
                    (cgroup / "parent/child", "max", "1")):
                (directory / "memory.max").write_text(maximum)
                (directory / "memory.current").write_text(current)
            snapshot = CAMPAIGN.host_memory(proc, cgroup)
            self.assertEqual(snapshot["available_bytes"], 3_000_000_000)
            self.assertEqual(snapshot["cgroup_capacity_bytes"], 4_000_000_000)

    def test_ctrl_c_waits_for_native_paused_receipt_and_preserves_prior_durable_save(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            child = directory / "fake-rustred"
            child.write_text(f"#!{sys.executable}\n" + """import json,sys,time
from pathlib import Path
arg=lambda name: Path(sys.argv[sys.argv.index(name)+1])
stop=arg('--stop-file'); events=arg('--events'); output=arg('--output')
if '--resume' in sys.argv:
    checkpoint=arg('--resume')
    if not (checkpoint/'state.json').is_file(): raise SystemExit(8)
    result={'status':'completed','family_closure_claim':False}
    output.write_text(json.dumps(result)); events.write_text(json.dumps({'progress':result})+'\\n')
    raise SystemExit(0)
checkpoint=arg('--checkpoint')
checkpoint.mkdir(parents=True)
state=checkpoint/'state.json'; state.write_text('{"test_fixture_only":true}')
saved={'state':'saved','directory':str(checkpoint),'generation':1,'state_path':str(state),'paused':False,'saved_unix_time':int(time.time())}
events.write_text(json.dumps({'event':'heartbeat','progress':{'event':'loaded','checkpoint':saved}})+'\\n')
deadline=time.monotonic()+8
while not stop.exists() and time.monotonic()<deadline: time.sleep(.01)
if not stop.exists(): raise SystemExit(9)
time.sleep(.1)
result={'status':'paused','checkpoint':saved,'preparation_interrupted':True,'family_closure_claim':False}
with events.open('a') as stream: stream.write(json.dumps({'event':'heartbeat','progress':result})+'\\n')
output.write_text(json.dumps(result))
raise SystemExit(4)
""")
            child.chmod(0o700)
            manifest = directory / "selection.json"; manifest.write_text("{}")
            queries = directory / "queries.json"; queries.write_text("{}")
            run = directory / "run"
            process = subprocess.Popen([sys.executable, str(SOURCE), "--executable", str(child),
                "--manifest", str(manifest), "--queries", str(queries), "--workers", "1",
                "--sample-seconds", ".1", "--run-directory", str(run), "--checkpoint", str(directory / "checkpoint"),
                "--unbounded-work", "--apply-subdivision-axis", "0", "--apply-subdivision-cut", "0", "--no-progress"],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            try:
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    if (run / "status.json").exists() and json.loads((run / "status.json").read_text())["state"] == "running":
                        break
                    if process.poll() is not None:
                        self.fail(process.communicate())
                    time.sleep(.02)
                else:
                    self.fail("supervisor never published running status")
                process.send_signal(signal.SIGINT)
                stdout, stderr = process.communicate(timeout=5)
                self.assertEqual(process.returncode, 4, (stdout, stderr))
                status = json.loads((run / "status.json").read_text())
                request = json.loads((run / "request.json").read_text())
                self.assertEqual(status["state"], "paused")
                self.assertEqual(status["checkpoint"]["generation"], 1)
                self.assertFalse(status["checkpoint"]["paused"])
                self.assertEqual(status["stop_reason"], "operator_signal_2")
                self.assertEqual(request["checkpoint_interval_seconds"], 3600)
                self.assertEqual(request["ram_guard_margin_percent"], 5)
                self.assertIsNone(request["child_rlimit_as_bytes"])
                self.assertEqual(request["apply_subdivision"], {"axis": 0, "cut": 0})
                self.assertTrue((run / "processes.json").is_file())
                self.assertIn("Durable checkpoint:", stdout)
                self.assertIn("Resume with fresh receipts:", stdout)
                restart = status["resume_command"]
                self.assertNotIn("--checkpoint", restart)
                self.assertIn("--resume", restart)
                self.assertIn("--apply-subdivision-axis", restart)
                next_run = Path(restart[restart.index("--run-directory") + 1])
                self.assertNotEqual(next_run, run)
                self.assertFalse(next_run.exists())
                resumed = subprocess.run(restart, capture_output=True, text=True, timeout=5)
                self.assertEqual(resumed.returncode, 0, (resumed.stdout, resumed.stderr))
                self.assertEqual(json.loads((next_run / "status.json").read_text())["state"], "completed")
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.communicate(timeout=10)

    def test_address_space_envelope_respects_reserves_and_inherited_soft_limit(self):
        self.assertEqual(CAMPAIGN.address_space_envelope(500_000_000_000,0,None,0,(-1,-1)),
                         (None,0))
        self.assertIsNone(CAMPAIGN.address_space_envelope(500_000_000_000,0,None,0,(1_000_000,2_000_000))[0])
        self.assertEqual(CAMPAIGN.address_space_envelope(500_000_000_000,0,3_000_000,0,(1_000_000,2_000_000))[0],1_000_000)
        for reserve, explicit, rss in [(500_000_000_000,None,0),(1,None,2),(0,501_000_000_000,0)]:
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
            self.assertIsNone(request["child_rlimit_as_bytes"])
            resources=[json.loads(line) for line in (receipt/"resources.jsonl").read_text().splitlines()]
            self.assertTrue(resources)
            self.assertEqual({row["role"] for row in resources[0]["process_cpu"]},
                             {"supervisor","owned_native"})
            self.assertEqual(resources[0]["processes"],2)
            self.assertIn("stat_reads",resources[0]["collection"])

    def test_initial_cpu_tick_is_unknown_and_later_activity_is_not_clamped(self):
        table={1:{"ppid":0,"start":11,"cpu_seconds":0.05,"rss_bytes":4096}}
        delta,baseline,rows=CAMPAIGN.process_cpu_sample(table,{(1,11):0.04},None,1,2)
        self.assertEqual(delta,0)
        self.assertIsNone(rows[0]["sampled_cpu_delta_seconds"])
        self.assertIsNone(rows[0]["observed_busy_cores"])
        table[1]["cpu_seconds"]=1.05
        delta,_,rows=CAMPAIGN.process_cpu_sample(table,baseline,0.1,1,2)
        self.assertEqual(delta,1)
        self.assertEqual(rows[0]["observed_busy_cores"],10)

    def test_fake_child_cpu_status_warms_up_then_reports_full_interval(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory=Path(temporary)
            child=directory/"fake-rustred"
            child.write_text(f"#!{sys.executable}\nimport time\ntime.sleep(.35)\n")
            child.chmod(0o700)
            manifest=directory/"selection.json"; manifest.write_text("{}")
            targets=directory/"targets.csv"; targets.write_text("1\n")
            receipt=directory/"receipt"
            result=subprocess.run([sys.executable,str(SOURCE),"--executable",str(child),
                "--manifest",str(manifest),"--targets",str(targets),"--workers","1",
                "--sample-seconds",".1","--run-directory",str(receipt),"--no-progress"],
                capture_output=True,text=True,timeout=10)
            self.assertEqual(result.returncode,0,result.stderr)
            rows=[json.loads(line) for line in (receipt/"resources.jsonl").read_text().splitlines()]
            self.assertGreaterEqual(len(rows),2)
            self.assertIsNone(rows[0]["observed_busy_cores"])
            self.assertIsNone(rows[0]["native_busy_cores"])
            self.assertIsNone(rows[0]["cpu_sample_interval_seconds"])
            self.assertTrue(all(p["observed_busy_cores"] is None for p in rows[0]["process_cpu"]))
            for row in rows[1:]:
                self.assertGreaterEqual(row["cpu_sample_interval_seconds"],0.1)
                self.assertIsNotNone(row["observed_busy_cores"])
                self.assertIsNotNone(row["native_busy_cores"])
            status=json.loads((receipt/"status.json").read_text())
            self.assertEqual(status["resources"],rows[-1])

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
                    command += ["--" + CAMPAIGN.DOMAIN.PUBLICATION_POLICY, "owner-batched"]
                    command += ["--" + CAMPAIGN.DOMAIN.INSPECTION_WORKERS, "1"]
                result=subprocess.run(command,capture_output=True,text=True,timeout=10)
                self.assertEqual(result.returncode,4,result.stderr)
                receipt=next((directory/"receipts").iterdir())
                request=json.loads((receipt/"request.json").read_text())
                summary=json.loads((receipt/"supervisor-result.json").read_text())
                actual=request["command"]
                self.assertEqual(request["input_scope"],"symbolic_domains")
                self.assertIsNone(request["hard_timeout"])
                self.assertEqual(request["workers"],1)
                for name in ("max_queries", "max_query_bytes"):
                    value = int(expected[name.replace("_", "-")]) if explicit else None
                    self.assertEqual(request["requested_" + name], value)
                    self.assertEqual(summary["requested_" + name], value)
                self.assertEqual(request["hard_memory_bytes"],500_000_000_000)
                self.assertIsNone(request["child_rlimit_as_bytes"])
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
                publication_option = "--" + CAMPAIGN.DOMAIN.PUBLICATION_POLICY
                policy = "owner-batched" if explicit == "unlimited" else "ordered"
                self.assertEqual(request["publication_policy"], policy)
                self.assertEqual(summary["publication_policy"], policy)
                inspectors = 1 if explicit == "unlimited" else None
                self.assertEqual(request["requested_inspection_workers"], inspectors)
                self.assertEqual(summary["requested_inspection_workers"], inspectors)
                self.assertEqual(request["reuse_initial_d_bands"], explicit == "unlimited")
                self.assertEqual(summary["reuse_initial_d_bands"], explicit == "unlimited")
                self.assertEqual(actual.count(reuse_option), int(explicit == "unlimited"))
                if explicit == "unlimited":
                    self.assertEqual(actual[actual.index(transfer_option)+1], "50")
                    self.assertEqual(actual[actual.index(publication_option)+1], "owner-batched")
                    self.assertEqual(actual[actual.index("--inspection-workers")+1], "1")
                else:
                    self.assertNotIn(transfer_option, actual)
                    self.assertNotIn(publication_option, actual)
                    self.assertNotIn("--inspection-workers", actual)
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
            ("--targets",["--publication-policy","ordered"],"require --queries"),
            ("--targets",["--publication-policy","owner-batched"],"require --queries"),
            ("--targets",["--inspection-workers","1"],"require --queries"),
            ("--queries",["--inspection-workers","2"],"leave one coordinator"),
            ("--queries",["--inspection-workers","0"],"positive integer"),
            ("--queries",["--inspection-workers","1","--inspection-workers","1"],"only once"),
            ("--queries",["--workers","6","--inspection-workers","1","--max-containment-checks","7"],"finite containment cap"),
            ("--queries",["--publication-policy","automatic"],"invalid choice"),
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
