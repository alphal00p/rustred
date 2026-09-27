import json, sys
rows=[]
with open('/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-v2/runs/20260926T151353.794886Z/events.jsonl') as f:
    for line in f:
        try: e=json.loads(line)
        except Exception: continue
        p=e.get('progress') or {}
        par=p.get('parallel') or {}
        if 'coordinator_duty' not in par: continue
        d=par['coordinator_duty']; a=par['admission_preparation']; c=par['containment_prefilter']
        rows.append([d['coordinator_elapsed_seconds'],d['ordered_commit_seconds'],d['preparation_seconds'],a['speculative_admission_requests'],a['prepared_batch_records'],a['parallel_batches'],p['scheduled_nodes'],p['containment_maintenance_checks'],c['forward_callbacks']-c['forward_bit_rejections'],c['reverse_callbacks'],a['speculative_containment_checks'],a.get('speculative_reverse_checks',0),p['events'],p['exact_domain_hits'],c['forward_callbacks'],d['dispatch_seconds'],d['poll_seconds'],d['progress_json_seconds'],d['publication_seconds'],p['completed_nodes'],p['delegation']['delegated_publications'],p['descendant_closure']['dependency_edges']])
W=[]; last=0
for i in range(1,len(rows)):
    if rows[i][0]-rows[last][0]>=float(sys.argv[1] if len(sys.argv)>1 else 60):
        W.append([b-a for a,b in zip(rows[last],rows[i])]); last=i
print('windows',len(W))
col=lambda k:[w[k] for w in W]
def solve(A,b):
    n=len(A)
    M=[row[:]+[bb] for row,bb in zip(A,b)]
    for i in range(n):
        p=max(range(i,n),key=lambda r:abs(M[r][i])); M[i],M[p]=M[p],M[i]
        for r in range(n):
            if r!=i:
                f=M[r][i]/M[i][i]
                for c in range(i,n+1): M[r][c]-=f*M[i][c]
    return [M[i][n]/M[i][i] for i in range(n)]
def fit(y,cols,names):
    k=len(cols); A=[[sum(cols[i][t]*cols[j][t] for t in range(len(y))) for j in range(k)] for i in range(k)]
    b=[sum(cols[i][t]*y[t] for t in range(len(y))) for i in range(k)]
    c=solve(A,b); pred=[sum(c[i]*cols[i][t] for i in range(k)) for t in range(len(y))]
    my=sum(y)/len(y); r2=1-sum((a-p)**2 for a,p in zip(y,pred))/sum((a-my)**2 for a in y)
    att=[c[i]*sum(cols[i]) for i in range(k)]
    print('  ','  '.join(f'{n}={v:.3g}[{a:.0f}s]' for n,v,a in zip(names,c,att)),f'R2={r2:.3f} actual={sum(y):.0f}s')
commit,prep=col(1),col(2)
req,rec,bat,dom,mc,fwdfull,rev,spec,srev=col(3),col(4),col(5),col(6),col(7),col(8),col(9),col(10),col(11)
nonadm=[a-b for a,b in zip(rec,req)]
print('commit ~ req + new'); fit(commit,[req,dom],['req','new'])
print('commit ~ req + new + maint'); fit(commit,[req,dom,mc],['req','new','maint'])
print('commit ~ req + new + maint + fwdfull + revcb'); fit(commit,[req,dom,mc,fwdfull,rev],['req','new','maint','fwdfull','revcb'])
print('commit ~ req + maint'); fit(commit,[req,mc],['req','maint'])
print('prep ~ bat + spec'); fit(prep,[bat,spec],['bat','spec'])
print('prep ~ bat + new + spec'); fit(prep,[bat,dom,spec],['bat','new','spec'])
print('prep ~ bat + req + spec + srev'); fit(prep,[bat,req,spec,srev],['bat','req','spec','srev'])
disp,poll,pj,pub,done,deleg=col(15),col(16),col(17),col(18),col(19),col(20)
print('dispatch ~ done + deleg + bat'); fit(disp,[done,deleg,bat],['native_pub','deleg_pub','bat'])
print('progress_json ~ deleg + done'); fit(pj,[deleg,done],['deleg','done'])
print('poll ~ done + bat'); fit(poll,[done,bat],['native_pub','bat'])
print('publication ~ done + deleg'); fit(pub,[done,deleg],['native','deleg'])
print('sums: req %.4g new %.4g maint %.4g bat %.4g done %.4g deleg %.4g' % (sum(req),sum(dom),sum(mc),sum(bat),sum(done),sum(deleg)))
