import json, sys
KEYS=['completed_nodes','scheduled_nodes','queued_nodes','committed_domains','frontiers','max_scheduled_finite_rank','full_orthant_hits','pre_admitted_orthant_hits','exact_domain_hits','containment_semantic_hits','containment_checks','deduplication_hits','successors','route_masks','routed_domains','conditional_successors','job_local_reuse_hits','events']
def series(path, every=300):
    out=[]; last=-1e9
    with open(path) as f:
        for line in f:
            if '"domain_progress"' not in line: continue
            try: d=json.loads(line)
            except Exception: continue
            if d.get('event')!='heartbeat': continue
            p=d.get('progress',{})
            if p.get('event')!='domain_progress': continue
            e=d.get('elapsed_seconds',0)
            if e-last<every: continue
            dc=p.get('descendant_closure') or {}
            row={'t':e,'rss':d.get('process_rss_bytes'),'roots':dc.get('initial_closed'),'total_closed':dc.get('total_closed'),'edges':dc.get('dependency_edges')}
            for k in KEYS: row[k]=p.get(k)
            out.append(row); last=e
    return out
name=sys.argv[1]; path=sys.argv[2]
s=series(path)
json.dump(s, open(name+'_ts.json','w'))
print(name, len(s)); print(json.dumps(s[-1]))
