import json,sys
src,dst=sys.argv[1],sys.argv[2]
P=['committed_domains','completed_nodes','conditional_successors','containment_candidates','containment_checks','containment_maintenance_checks','containment_retired_candidates','containment_semantic_hits','containment_semantic_retirements','containment_summary_builds','deduplication_hits','events','exact_domain_hits','frontiers','full_orthant_hits','job_local_reuse_hits','max_scheduled_finite_rank','pre_admitted_orthant_hits','queued_nodes','routed_domains','route_masks','scheduled_nodes','successors','id','owner','phase','rank','unbounded_rank_domains','pending_descendant_domains','initial_entry_domains_inspected','ready_accepted_source_prefixes']
AP=['ordered_commit_wall_seconds','preparation_wall_seconds','parallel_batches','prepared_batch_records','prepared_retirements_applied','speculative_admission_requests','speculative_containment_checks','speculative_forward_bit_rejections','speculative_reverse_bit_rejections','speculative_reverse_checks']
PAR=['attempted_events','attempted_native_operations','attempted_predicates','attempted_rule_checks','backpressure_seconds','computing_workers','returned_inspections','finished_uncommitted_domains','finished_awaiting_poll','completed_escrow_entries','occupied_native_slots','worker_buffered_events','completed_slots_reclaimed']
out=open(dst,'w')
n=0
for line in open(src):
    try: d=json.loads(line)
    except Exception: continue
    if d.get('event')!='heartbeat': continue
    r={'t':d.get('elapsed_seconds'),'rss':d.get('process_rss_bytes'),'disc':d.get('currently_discovered_nodes'),'exp':d.get('expanded_nodes')}
    p=d.get('progress') or {}
    r['pev']=p.get('event')
    for k in P:
        if k in p: r[k]=p[k]
    pb=p.get('power_bounds')
    if pb: r['maxpos']=pb.get('max_positive_power'); r['maxdiff']=pb.get('max_power_difference')
    dc=p.get('descendant_closure') or {}
    for k in ['total_closed','initial_closed','dependency_edges','refresh_seconds','refresh_count','unresolved_domains','snapshot_revision','graph_revision']:
        if k in dc: r['dc_'+k]=dc[k]
    dl=p.get('delegation') or {}
    for k in ['delegated_publications','native_publications','pending_native_publications','transferred_obligations','logical_publications']:
        if k in dl: r['dl_'+k]=dl[k]
    par=p.get('parallel') or {}
    if par:
        r['has_par']=1
        for k in PAR:
            if k in par: r['par_'+k]=par[k]
        ap=par.get('admission_preparation') or {}
        for k in AP:
            if k in ap: r['ap_'+k]=ap[k]
        cp=par.get('containment_prefilter') or {}
        for k in ['forward_callbacks','forward_bit_rejections','reverse_callbacks','reverse_bit_rejections']:
            if k in cp: r['cp_'+k]=cp[k]
        cd=par.get('coordinator_duty') or {}
        for k,v in cd.items():
            if isinstance(v,(int,float)): r['cd_'+k]=v
        for k in ['slot_busy_seconds','slot_backpressure_seconds','slot_idle_seconds']:
            if k in par and isinstance(par[k],list): r['sum_'+k]=sum(par[k]); r['n_'+k]=len(par[k])
        h=par.get('heaviest_active_stream')
        if h: r['heavy']=h
    ck=p.get('checkpoint') or {}
    if ck: r['ck_gen']=ck.get('generation'); r['ck_bytes']=ck.get('bytes'); r['ck_state']=ck.get('state')
    out.write(json.dumps(r)+'\n'); n+=1
print(n)
