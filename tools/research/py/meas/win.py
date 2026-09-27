import json,sys,math
src=sys.argv[1]; W=float(sys.argv[2]) if len(sys.argv)>2 else 1800
rows=[json.loads(l) for l in open(src)]
rows=[r for r in rows if r.get('has_par') and r.get('deduplication_hits') is not None and r.get('ap_ordered_commit_wall_seconds') is not None and r.get('completed_nodes') is not None]
full=[r for r in rows if r.get('cd_coordinator_elapsed_seconds') is not None and r.get('cp_forward_callbacks') is not None and r.get('par_computing_workers') is not None]
def A(r): # queue admission requests
    return r['deduplication_hits']-r['job_local_reuse_hits']-r['pre_admitted_orthant_hits']+r['scheduled_nodes']
def cont(r):
    return r['deduplication_hits']-r['job_local_reuse_hits']-r['pre_admitted_orthant_hits']-r['exact_domain_hits']-r['full_orthant_hits']
# windows by time
out=[]
t0=rows[0]['t']; tend=rows[-1]['t']
import bisect
ts=[r['t'] for r in rows]
fts=[r['t'] for r in full]
k=0
edges=[]
t=t0
while t<tend:
    edges.append(t); t+=W
edges.append(tend)
for a,b in zip(edges[:-1],edges[1:]):
    i=bisect.bisect_left(ts,a); j=bisect.bisect_right(ts,b)-1
    if j<=i: continue
    r0,r1=rows[i],rows[j]
    dt=r1['t']-r0['t']
    dC=r1['completed_nodes']-r0['completed_nodes']
    dS=r1['scheduled_nodes']-r0['scheduled_nodes']
    dP=r1['queued_nodes']-r0['queued_nodes']
    dA=A(r1)-A(r0)
    dsucc=r1['successors']-r0['successors']
    dev=r1['events']-r0['events']
    d=dict(t0=r0['t'],t1=r1['t'],h=r1['t']/3600,dt=dt,compl_h=dC/dt*3600,disc_h=dS/dt*3600,pending=r1['queued_nodes'],sched=r1['scheduled_nodes'],
      dpend_per_compl=dP/dC if dC else None, succ_per_compl=dsucc/dC if dC else None, req_per_compl=dA/dC if dC else None,
      events_per_compl=dev/dC if dC else None,
      req_s=dA/dt,
      exact=(r1['exact_domain_hits']-r0['exact_domain_hits'])/dA if dA else None,
      orth=(r1['full_orthant_hits']-r0['full_orthant_hits'])/dA if dA else None,
      contain=(cont(r1)-cont(r0))/dA if dA else None,
      new=dS/dA if dA else None,
      sem_of_cont=(r1['containment_semantic_hits']-r0['containment_semantic_hits'])/max(1,cont(r1)-cont(r0)),
      jl_per_compl=(r1['job_local_reuse_hits']-r0['job_local_reuse_hits'])/dC if dC else None,
      checks_per_req=(r1['containment_checks']-r0['containment_checks'])/dA if dA else None,
      spec_checks_per_req=(r1['ap_speculative_containment_checks']-r0['ap_speculative_containment_checks'])/max(1,r1['ap_speculative_admission_requests']-r0['ap_speculative_admission_requests']),
      maint_per_new=(r1['containment_maintenance_checks']-r0['containment_maintenance_checks'])/dS if dS else None,
      commit_us_per_rec=1e6*(r1['ap_ordered_commit_wall_seconds']-r0['ap_ordered_commit_wall_seconds'])/max(1,r1['ap_prepared_batch_records']-r0['ap_prepared_batch_records']),
      prep_us_per_batch=1e6*(r1['ap_preparation_wall_seconds']-r0['ap_preparation_wall_seconds'])/max(1,r1['ap_parallel_batches']-r0['ap_parallel_batches']),
      recs_per_batch=(r1['ap_prepared_batch_records']-r0['ap_prepared_batch_records'])/max(1,r1['ap_parallel_batches']-r0['ap_parallel_batches']),
      coord_us_per_req=1e6*((r1['ap_ordered_commit_wall_seconds']-r0['ap_ordered_commit_wall_seconds'])+(r1['ap_preparation_wall_seconds']-r0['ap_preparation_wall_seconds']))/dA if dA else None,
      wall_us_per_req=1e6*dt/dA if dA else None,
      live_cand=r1['containment_candidates'], retired=r1['containment_retired_candidates'],
      rss_GB=r1['rss']/1e9, rss_per_sched=r1['rss']/r1['scheduled_nodes'],
      edges=r1['dc_dependency_edges'], edges_per_sched=r1['dc_dependency_edges']/r1['scheduled_nodes'],
      closed=r1['dc_total_closed'], roots_closed=r1['dc_initial_closed'],
      maxrank=r1['max_scheduled_finite_rank'], maxpos=r1.get('maxpos'),
      natops_per_compl=(r1['par_attempted_native_operations']-r0['par_attempted_native_operations'])/dC if dC else None,
      rulechecks_per_compl=(r1['par_attempted_rule_checks']-r0['par_attempted_rule_checks'])/dC if dC else None,
      backpressure_share=(r1['par_backpressure_seconds']-r0['par_backpressure_seconds'])/dt,
      deleg_pub=(r1['dl_delegated_publications']-r0['dl_delegated_publications'])/dt*3600,
      routed_frac=(r1['routed_domains']-r0['routed_domains'])/dS if dS else None,
    )
    # duty from full rows
    fi=bisect.bisect_left(fts,a); fj=bisect.bisect_right(fts,b)-1
    if fj>fi:
        f0,f1=full[fi],full[fj]
        ce=f1['cd_coordinator_elapsed_seconds']-f0['cd_coordinator_elapsed_seconds']
        for kk in ['ordered_commit_seconds','preparation_seconds','dispatch_seconds','publication_seconds','progress_json_seconds','poll_seconds','closure_refresh_seconds','checkpoint_seconds','wait_seconds','ready_service_seconds']:
            d['duty_'+kk.replace('_seconds','')]=(f1['cd_'+kk]-f0['cd_'+kk])/ce if ce>0 else None
        d['duty_busy']=1-(d['duty_wait'] or 0)
        cw=[r['par_computing_workers'] for r in full[fi:fj+1]]
        d['computing_mean']=sum(cw)/len(cw)
        fa=f1['cp_forward_callbacks']-f0['cp_forward_callbacks']
        d['commit_fwd_callbacks_per_req']=fa/dA if dA else None
        d['fwd_bit_reject']=(f1['cp_forward_bit_rejections']-f0['cp_forward_bit_rejections'])/fa if fa else None
        sr=f1['ap_speculative_reverse_checks']-f0['ap_speculative_reverse_checks']
        d['spec_rev_per_new']=sr/dS if dS else None
    out.append(d)
json.dump(out,open(src.replace('_hb.jsonl',f'_win{int(W)}.json'),'w'))
cols=sys.argv[3].split(',') if len(sys.argv)>3 else None
def fmt(v):
    if v is None: return '-'
    if isinstance(v,float):
        if abs(v)>=1e6: return f'{v:.3e}'
        if abs(v)>=100: return f'{v:.0f}'
        if abs(v)>=1: return f'{v:.2f}'
        return f'{v:.4f}'
    return str(v)
if cols:
    print('\t'.join(cols))
    for d in out: print('\t'.join(fmt(d.get(c)) for c in cols))
