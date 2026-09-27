import json,sys
path=sys.argv[1]
rows=[]
with open(path,'rb') as f:
    for line in f:
        if b'"event":"heartbeat"' not in line[:400] and b'"event":"heartbeat"' not in line: continue
        if b'ordered_commit_wall_seconds' not in line: continue
        try: e=json.loads(line)
        except Exception: continue
        p=e.get('progress',{})
        par=p.get('parallel',{})
        ap=par.get('admission_preparation',{})
        duty=par.get('coordinator_duty') or {}
        pre=par.get('containment_prefilter') or {}
        rows.append(dict(t=e.get('elapsed_seconds'),rss=e.get('process_rss_bytes'),
          sched=p.get('scheduled_nodes'),pub=p.get('committed_domains'),cand=p.get('containment_candidates'),
          retired=p.get('containment_retired_candidates'),
          req=ap.get('speculative_admission_requests'),spec=ap.get('speculative_containment_checks'),
          specrev=ap.get('speculative_reverse_checks'),
          commit=ap.get('ordered_commit_wall_seconds'),prep=ap.get('preparation_wall_seconds'),
          batches=ap.get('parallel_batches'),records=ap.get('prepared_batch_records'),
          fwd=pre.get('forward_callbacks'),rev=pre.get('reverse_callbacks'),
          native=p.get('completed_nodes'),events=p.get('committed_events'),
          comp=par.get('computing_workers'),edges=(p.get('descendant_closure') or {}).get('dependency_edges'),
          delegated=(p.get('delegation') or {}).get('delegated_publications'),
          transfers=(p.get('delegation') or {}).get('transferred_obligations'),
          dispatch=duty.get('dispatch_seconds'),poll=duty.get('poll_seconds'),pj=duty.get('progress_json_seconds'),
          publ=duty.get('publication_seconds'),cel=duty.get('coordinator_elapsed_seconds')))
rows=[r for r in rows if r['t'] and r['req'] is not None]
print(len(rows))
out=[]
last=None
for r in rows:
    if last is None or r['t']-last['t']>=1800:
        if last is not None:
            dt=r['t']-last['t']
            d=lambda k:(r[k] or 0)-(last[k] or 0)
            req=d('req'); rec=d('records'); b=d('batches')
            out.append(dict(t_h=round(r['t']/3600,2),sched_M=round(r['sched']/1e6,1),pub_M=round(r['pub']/1e6,1),
              cand_M=round((r['cand'] or 0)/1e6,1),rss_GB=round((r['rss'] or 0)/1e9,1),
              req_per_s=round(req/dt), spec_fwd_per_req=round(d('spec')/max(req,1),1),
              spec_rev_per_req=round(d('specrev')/max(req,1),1),
              commit_us_per_rec=round(1e6*d('commit')/max(rec,1),2), prep_us_per_batch=round(1e6*d('prep')/max(b,1),1),
              rec_per_batch=round(rec/max(b,1),1),
              coord_fwd_per_req=round(d('fwd')/max(req,1),1),
              commit_share=round(d('commit')/dt,3),prep_share=round(d('prep')/dt,3),
              native_per_h=round(d('native')*3600/dt), newdom_per_h=round(d('sched')*3600/dt),
              pub_per_h=round(d('pub')*3600/dt), deleg_per_h=round(d('delegated')*3600/dt),
              edges_per_newdom=round(d('edges')/max(d('sched'),1),1),
              req_per_native=round(req/max(d('native'),1),1)))
        last=r
for o in out: print(json.dumps(o))
