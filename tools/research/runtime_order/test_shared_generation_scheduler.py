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

    def test_projection_cannot_modify_supervisor_owned_jobs_or_publication_state(self):
        progress=Progress(self.path, [dict(id='native',state='running')], {},
            projection=lambda value: dict(value,jobs=[dict(id='A',state='pending')]))
        try:
            progress.emit('started')
            self.assertEqual(progress.value['jobs'][0]['id'],'native')
            self.assertEqual(json.loads((self.path/'snapshot.json').read_bytes())['jobs'][0]['id'],'A')
        finally: progress.close()

    def test_partial_completed_parent_is_retained_after_native_failure(self):
        def fail(jobs,directory,resources,callback,**options):
            path=Path(jobs[0].command[5])
            path.write_text(json.dumps(dict(parents=[dict(id='A',state='completed'),dict(id='B',state='failed')])))
            raise RuntimeError('native failure')
        completed=[]
        with patch.object(shared.supervisor,'run',side_effect=fail), self.assertRaisesRegex(RuntimeError,'native failure'):
            shared.run(self.jobs(),self.path/'failed',self.resources,'rustred',lambda job:completed.append(job.id))
        self.assertEqual(completed,['A'])


@unittest.skipUnless(os.environ.get('RUSTRED_PREPARATION_VIEWER'), 'set RUSTRED_PREPARATION_VIEWER for the native failure/resume gate')
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
                executable=Path(os.environ['RUSTRED_PREPARATION_VIEWER']),workers=2,cpus=','.join(map(str,cpus)),
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


if __name__=='__main__': unittest.main()
