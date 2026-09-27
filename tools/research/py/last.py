import json,sys
last=None
for line in sys.stdin:
    if '"containment_checks":' not in line: continue
    try: d=json.loads(line)
    except: continue
    p=d.get('progress') or d
    if p.get('containment_checks') is not None and p.get('pre_admitted_orthant_hits') is not None:
        last=(d,p)
d,p=last
print('event',d.get('event'),'t',d.get('elapsed_seconds'), 'pevent', p.get('event'))
for k in ['completed_nodes','scheduled_nodes','queued_nodes','committed_domains','successors','conditional_successors','deduplication_hits','exact_domain_hits','full_orthant_hits','containment_checks','containment_maintenance_checks','containment_retired_candidates','containment_semantic_hits','containment_semantic_retirements','containment_candidates','job_local_reuse_hits','pre_admitted_orthant_hits','max_scheduled_finite_rank','frontiers','routed_domains','route_masks','events']:
    print(k,p.get(k))
print('closure',{k:(p.get('descendant_closure') or {}).get(k) for k in ['initial_closed','total_closed','dependency_edges']})
