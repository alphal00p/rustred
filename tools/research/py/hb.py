import json,sys
src=sys.argv[1]
out=open('hb.tsv','w')
n=0
with open(src,'rb') as f:
    for line in f:
        if b'"admission_preparation"' not in line: continue
        try: d=json.loads(line)
        except Exception: continue
        p=d.get('progress') or {}
        par=p.get('parallel') or {}
        ap=par.get('admission_preparation') or {}
        du=par.get('coordinator_duty') or {}
        cp=par.get('containment_prefilter') or {}
        row=[d.get('elapsed_seconds'),p.get('committed_events'),p.get('completed_nodes'),p.get('committed_domains'),d.get('currently_discovered_nodes'),p.get('pending_descendant_domains'),
             ap.get('preparation_wall_seconds'),ap.get('ordered_commit_wall_seconds'),ap.get('parallel_batches'),ap.get('prepared_batch_records'),ap.get('speculative_admission_requests'),ap.get('speculative_containment_checks'),ap.get('speculative_reverse_checks'),ap.get('speculative_forward_bit_rejections'),
             cp.get('forward_callbacks'),cp.get('reverse_callbacks'),du.get('coordinator_elapsed_seconds'),du.get('dispatch_seconds'),du.get('publication_seconds'),du.get('progress_json_seconds'),du.get('poll_seconds'),du.get('closure_refresh_seconds'),du.get('checkpoint_seconds'),par.get('computing_workers'),d.get('process_rss_bytes'),(p.get('descendant_closure') or {}).get('dependency_edges'),(p.get('delegation') or {}).get('transferred_obligations')]
        out.write('\t'.join(str(x) for x in row)+'\n'); n+=1
print(n)
