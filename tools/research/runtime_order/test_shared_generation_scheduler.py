"""Shared native scheduling presentation and launch contract, without CAS."""
import copy
import json
import os
from pathlib import Path
import tempfile
import time
import unittest
import argparse
import sys
from unittest.mock import patch

import generation_scheduler
from generation_state import Progress
import shared_generation_scheduler as shared


class SharedSchedulerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=generation_scheduler.ROOT/'TMP')
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name)
        self.resources = dict(workers=4,cpus='0-3',generation_jobs=3,
            max_active_sectors=None,max_memory_bytes=1000)
        self.enterContext(patch.object(shared.os,'sched_getaffinity',return_value=set(range(4))))

    def jobs(self):
        return [generation_scheduler.Job(key,('rustred','family-candidates','--input',key),
            dict(parent=i+1,root='1',selected_sectors=['1']), self.path/f'{key}.json')
            for i,key in enumerate(['A','B'])]

    def test_parent_slots_need_not_divide_global_compute_budget(self):
        plan=shared.schedule(self.resources)
        self.assertEqual(plan['jobs'],3)
        self.assertTrue(plan['shared_pool'])
        self.assertEqual(plan['slots'],[[0,1,2,3]])
        self.assertEqual(plan['max_active_sectors'],None)
        for cap in [0,-1,False]:
            with self.assertRaises(ValueError): shared.schedule(dict(self.resources,max_active_sectors=cap))

    def test_one_process_is_launched_for_all_parents_with_explicit_optional_cap(self):
        completed=[]
        def run(jobs,directory,resources,callback,**options):
            self.assertEqual(len(jobs),1)
            self.assertEqual(resources['generation_jobs'],1)
            config=json.loads(Path(jobs[0].command[3]).read_bytes())
            self.assertEqual(config['max_active_sectors'],2)
            self.assertEqual(config['workers'],4)
            self.assertEqual([job['id'] for job in config['jobs']],['A','B'])
            callback(jobs[0])
            return {'status':'done'}
        with patch.object(shared.supervisor,'run',side_effect=run):
            result=shared.run(self.jobs(),self.path/'attempt',dict(self.resources,max_active_sectors=2),
                'rustred',lambda job: completed.append(job.id))
        self.assertEqual(result,{'status':'done'})
        self.assertEqual(completed,['A','B'])

    def test_projection_uses_per_parent_status_without_double_counting_cpu_or_ram(self):
        status=self.path/'native.json'
        status.write_text(json.dumps(dict(schema='rustred.shared-generation-progress.v1',pid=123,
            timestamp_unix_seconds=time.time(), pool=dict(running=3,waiting_for_worker=12,admitted=15,
                completed=2,peak_running=4), parents=[dict(id='A',state='preparing'),dict(id='B',state='running')])))
        original=dict(jobs=[dict(id='shared-worker-pool',pid=123,pid_start=50,
            started_unix=time.time()-1,state='running',cpus=[0,1,2,3])],
            aggregate=dict(observed_cores=3.5,sampled_rss_bytes=900), resources={})
        result=shared.Projection(self.jobs(),[],self.resources,status)(copy.deepcopy(original))
        self.assertEqual([job['id'] for job in result['jobs']],['A','B'])
        self.assertTrue(all(job['shared_process'] for job in result['jobs']))
        self.assertTrue(all(job['observed_cores'] is None and job['sampled_rss_bytes'] is None for job in result['jobs']))
        self.assertEqual(result['aggregate']['observed_cores'],3.5)
        self.assertEqual(result['aggregate']['pool']['running'],3)
        self.assertEqual(original['jobs'][0]['id'],'shared-worker-pool')

    def final_fixture(self, second="running"):
        now = time.time()
        status = self.path / "native.json"
        status.write_text(json.dumps(dict(schema="rustred.shared-generation-progress.v1", pid=123,
            timestamp_unix_seconds=now, parents=[dict(id="A",state="completed"), dict(id="B",state=second)])))
        for job in self.jobs():
            job.native_progress.write_text(json.dumps(dict(schema="rustred.family-generation-progress.v1",
                pid=123, process_start_ticks=50, state="output_written", family_closure_claim=False,
                counts=dict(generated=1,reused=17,rules_generated=1781,finite_residuals_generated=31))))
            os.utime(job.native_progress, (now-300, now-300))
        snapshot = dict(jobs=[dict(id="shared-worker-pool", pid=123, pid_start=50,
            started_unix=now-1000,state="running",cpus=[0,1,2,3])],
            aggregate=dict(observed_cores=3.5,sampled_rss_bytes=900),resources={})
        return shared.Projection(self.jobs(), [], self.resources, status), snapshot, status

    def test_old_terminal_snapshot_is_final_only_for_completed_parent(self):
        for second in ("running", "pending", "failed"):
            with self.subTest(second=second):
                project, snapshot, _ = self.final_fixture(second)
                result = project(copy.deepcopy(snapshot))
                completed, other = result["jobs"]
                self.assertEqual(completed["state"], "completed")
                self.assertEqual(completed["native_progress"]["status"], "final")
                self.assertGreaterEqual(completed["native_progress"]["age_seconds"], 300)
                self.assertEqual(completed["native_progress"]["snapshot"]["counts"]["rules_generated"], 1781)
                self.assertEqual(completed["native_progress_path"], str(self.jobs()[0].native_progress.resolve()))
                self.assertEqual(completed["started_unix"], snapshot["jobs"][0]["started_unix"])
                self.assertEqual(other["native_progress"]["status"], "stale")
                self.assertIsNone(other["native_progress"]["snapshot"])
                self.assertFalse(result.get("family_closure_claim", False))

    def test_completed_outcome_cache_is_invocation_bound_and_does_not_override_failure(self):
        project, snapshot, status = self.final_fixture()
        self.assertEqual(project(copy.deepcopy(snapshot))["jobs"][0]["native_progress"]["status"], "final")
        status.unlink()
        cached = project(copy.deepcopy(snapshot))
        self.assertEqual(cached["jobs"][0]["state"], "completed")
        self.assertEqual(cached["jobs"][0]["native_progress"]["status"], "final")
        status.write_text(json.dumps(dict(schema="rustred.shared-generation-progress.v1",pid=123,
            timestamp_unix_seconds=time.time(),parents=[dict(id="A",state="failed")])))
        failed = project(copy.deepcopy(snapshot))
        self.assertEqual(failed["jobs"][0]["state"], "failed")
        self.assertIsNone(failed["jobs"][0]["native_progress"]["snapshot"])
        status.unlink()
        self.assertNotEqual(project(copy.deepcopy(snapshot))["jobs"][0]["state"], "completed")
        for identity in (dict(pid=124,pid_start=51), dict(started_unix=time.time())):
            project, snapshot, status = self.final_fixture()
            self.assertEqual(project(copy.deepcopy(snapshot))["jobs"][0]["native_progress"]["status"], "final")
            status.unlink()
            changed = copy.deepcopy(snapshot)
            changed["jobs"][0].update(identity)
            reset = project(changed)
            self.assertNotEqual(reset["jobs"][0]["state"], "completed")
            self.assertIsNone(reset["jobs"][0]["native_progress"]["snapshot"])

    def test_drained_partial_observations_stay_historical_and_final_stays_final(self):
        project, snapshot, _ = self.final_fixture()
        snapshot["jobs"][0]["state"] = "failed"
        path = self.jobs()[1].native_progress
        value = json.loads(path.read_text())
        value["state"] = "running"
        path.write_text(json.dumps(value))
        completed, interrupted = project(snapshot)["jobs"]
        self.assertEqual(completed["native_progress"]["status"], "final")
        self.assertEqual(interrupted["state"], "interrupted")
        self.assertEqual(interrupted["native_progress"]["status"], "historical")
        self.assertEqual(interrupted["native_progress"]["snapshot"]["state"], "running")

    def test_completed_parent_rejects_old_attempt_or_wrong_native_identity(self):
        for mutation in ({"pid":124}, {"process_start_ticks":51}, {"state":"running"},
                         {"family_closure_claim":True}, {"schema":"other"}):
            project, snapshot, _ = self.final_fixture()
            path = self.jobs()[0].native_progress
            value = json.loads(path.read_text())
            value.update(mutation)
            path.write_text(json.dumps(value))
            result = project(snapshot)["jobs"][0]["native_progress"]
            self.assertNotEqual(result["status"], "final")
            self.assertIsNone(result["snapshot"])
        project, snapshot, _ = self.final_fixture()
        path = self.jobs()[0].native_progress
        old = snapshot["jobs"][0]["started_unix"]-1
        os.utime(path, (old, old))
        result = project(snapshot)["jobs"][0]["native_progress"]
        self.assertEqual(result["status"], "stale")
        self.assertIsNone(result["snapshot"])

    def test_projection_cannot_modify_supervisor_owned_jobs_or_publication_state(self):
        progress=Progress(self.path, [dict(id='native',state='running')], {},
            projection=lambda value: dict(value,jobs=[dict(id='A',state='pending')]))
        try:
            progress.emit('started')
            self.assertEqual(progress.value['jobs'][0]['id'],'native')
            self.assertEqual(json.loads((self.path/'snapshot.json').read_bytes())['jobs'][0]['id'],'A')
        finally: progress.close()

    def test_dead_shared_process_cannot_leave_parents_or_pool_running(self):
        status = self.path/'native.json'
        status.write_text(json.dumps(dict(schema='rustred.shared-generation-progress.v1',pid=123,
            timestamp_unix_seconds=time.time(), pool=dict(running=4,waiting_for_worker=12,admitted=16,
                completed=3,peak_running=4), parents=[dict(id='A',state='running'),dict(id='B',state='pending')])))
        snapshot = dict(jobs=[dict(id='shared-worker-pool',pid=123,pid_start=50,
            started_unix=time.time()-1,state='failed',cpus=[0,1,2,3])],
            aggregate=dict(observed_cores=2.5,sampled_rss_bytes=0),resources={})
        value=shared.Projection(self.jobs(),[],self.resources,status)(snapshot)
        self.assertEqual([row['state'] for row in value['jobs']], ['interrupted','not_started'])
        self.assertEqual(value['aggregate']['pool']['running'],0)
        self.assertEqual(value['aggregate']['pool']['waiting_for_worker'],0)
        self.assertEqual(value['aggregate']['observed_cores'],0)

    def test_partial_completed_parent_is_retained_after_native_failure(self):
        def fail(jobs,directory,resources,callback,**options):
            path=Path(jobs[0].command[5])
            path.write_text(json.dumps(dict(parents=[dict(id='A',state='completed'),dict(id='B',state='failed')])))
            raise RuntimeError('native failure')
        completed=[]
        with patch.object(shared.supervisor,'run',side_effect=fail), self.assertRaisesRegex(RuntimeError,'native failure'):
            shared.run(self.jobs(),self.path/'failed',self.resources,'rustred',lambda job:completed.append(job.id))
        self.assertEqual(completed,['A'])


@unittest.skipUnless(os.environ.get('RUSTRED_GENERATION_TEST_BINARY') or os.environ.get('RUSTRED_PREPARATION_VIEWER'),
                     'set RUSTRED_GENERATION_TEST_BINARY for the native failure/resume gate')
class NativeFailureResumeTest(unittest.TestCase):
    def test_failed_parent_preserves_completed_other_parent_and_resumes(self):
        sys.path.insert(0,str(generation_scheduler.ROOT/'six_loop_study'))
        import generate
        with tempfile.TemporaryDirectory(dir=generate.ROOT/'TMP') as name:
            root=Path(name)
            top=root/'inputs'
            top.mkdir()
            entries=[]
            for key,mass in [('A',1),('B',4)]:
                (top/f'{key}.toml').write_text(f'''schema = "rustred.project.toml.v1"
[family]
name = "failure_resume_{key}"
loop_momenta = ["q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "D"
expression = "q^2-{mass}"
''')
                (top/f'{key}.dot').write_text('graph control { 0 -- 0; }\n')
                entries.append(dict(id=key,input=f'{key}.toml',dot=f'{key}.dot',loop_count=1,
                    physical_propagators=[0],auxiliary_slots=[],selected_sectors=['1']))
            manifest=top/'manifest.json'
            manifest.write_text(json.dumps(dict(schema=generate.TOPOLOGY_SCHEMA,entries=entries)))
            cpus=sorted(os.sched_getaffinity(0))[:2]
            plan=generate.prepare(argparse.Namespace(directory=root/'run',manifest=manifest,
                executable=Path(os.environ.get('RUSTRED_GENERATION_TEST_BINARY') or os.environ['RUSTRED_PREPARATION_VIEWER']),workers=2,cpus=','.join(map(str,cpus)),
                jobs=1,max_memory_bytes=8_000_000_000,ram_margin_percent=None,rank=0,numerical_depth=None,
                finite_case_policy=None,exact_backend=None,discovery_strategy=None,full_downsets=False,
                execution_mode='shared',max_active_sectors=1))
            jobs=[]
            for i,owner in enumerate(plan['owners']):
                command=generate.command(root/'run',plan,owner)
                if owner['id']=='B': command[command.index('--selected-sectors')+1]='0'
                jobs.append(generation_scheduler.Job(owner['id'],tuple(command),
                    dict(parent=i+1,root='1',selected_sectors=['1']),
                    native_progress=root/f"run/generation/{owner['id']}/native-progress.json"))
            by_id={owner['id']:owner for owner in plan['owners']}
            with self.assertRaises(RuntimeError):
                shared.run(jobs,root/'run/attempts/failed',plan['resources'],root/'run/bin/rustred',
                    lambda job:generate.record_completed(root/'run',plan,by_id[job.id]))
            self.assertIsNotNone(generate.completed(root/'run',plan,by_id['A']))
            self.assertIsNone(generate.completed(root/'run',plan,by_id['B']))
            result=generate.execute(root/'run',plan,dashboard=False)
            self.assertEqual((result['parents'],result['generated_rules'],result['finite_residuals']),(2,2,2))
            # Repack from the actual native completed-sector checkpoints under
            # a different worker/CPU/parent budget, never regenerate the math.
            available=sorted(os.sched_getaffinity(0))
            variants=[(1,[available[-1]],3)]
            if len(available)>=4: variants.append((4,available[-4:],1))
            for workers,affinity,parents in variants:
                for owner in plan['owners']:
                    (root/f"run/generation/{owner['id']}/completed.json").unlink()
                changed=generate.resume_resources(plan,parents,1,workers=workers,
                    cpus=','.join(map(str,affinity)))
                result=generate.execute(root/'run',changed,dashboard=False)
                self.assertEqual((result['parents'],result['generated_rules'],result['finite_residuals']),(2,2,2))
                self.assertEqual(generate.read(root/'run/generation.json')['resources']['workers'],workers)
                for owner in plan['owners']:
                    import tomllib
                    report=tomllib.loads((root/f"run/generation/{owner['id']}/generation.toml").read_text())
                    self.assertEqual(report['checkpoint']['reused_sectors'],1)
                    self.assertEqual(report['checkpoint']['newly_solved_sectors'],0)
                plan=changed


if __name__=='__main__': unittest.main()
