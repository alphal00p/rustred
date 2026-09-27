import json,math,sys
W=json.load(open(sys.argv[1]))
W=[w for w in W if w['dt']>300 and w.get('req_s') and w.get('spec_checks_per_req')]
def ols(X,y):
    # X list of rows (with intercept col included)
    n=len(X[0])
    XtX=[[sum(r[i]*r[j] for r in X) for j in range(n)] for i in range(n)]
    Xty=[sum(r[i]*yy for r,yy in zip(X,y)) for i in range(n)]
    # gaussian elimination
    M=[row+[v] for row,v in zip(XtX,Xty)]
    for i in range(n):
        p=max(range(i,n),key=lambda k:abs(M[k][i])); M[i],M[p]=M[p],M[i]
        for k in range(n):
            if k!=i:
                f=M[k][i]/M[i][i]
                M[k]=[a-f*b for a,b in zip(M[k],M[i])]
    beta=[M[i][n]/M[i][i] for i in range(n)]
    yh=[sum(b*x for b,x in zip(beta,r)) for r in X]
    my=sum(y)/len(y)
    ss=sum((a-b)**2 for a,b in zip(y,yh)); st=sum((a-my)**2 for a in y)
    return beta,1-ss/st
def fit(name,xs,ys,log=False):
    pts=[(x,y) for x,y in zip(xs,ys) if x is not None and y is not None and (not log or (x>0 and y>0))]
    if len(pts)<3:
        print(name,'n/a'); return None,None
    if log: X=[[1,math.log(x)] for x,_ in pts]; y=[math.log(y) for _,y in pts]
    else: X=[[1,x] for x,_ in pts]; y=[y for _,y in pts]
    b,r2=ols(X,y)
    if log: print(f'{name}: y = {math.exp(b[0]):.4g} * x^{b[1]:.3f}  R2={r2:.3f} n={len(pts)}')
    else: print(f'{name}: y = {b[0]:.4g} + {b[1]:.4g} x  R2={r2:.3f} n={len(pts)}')
    return b,r2
g=lambda k:[w.get(k) for w in W]
fit('spec_checks_per_req vs maint_per_new (loglog)',g('maint_per_new'),g('spec_checks_per_req'),True)
fit('spec_checks_per_req vs live_cand (loglog)',g('live_cand'),g('spec_checks_per_req'),True)
fit('spec_checks_per_req vs sched (loglog)',g('sched'),g('spec_checks_per_req'),True)
fit('maint_per_new vs live_cand (loglog)',g('live_cand'),g('maint_per_new'),True)
fit('wall_us_per_req vs spec_checks_per_req',g('spec_checks_per_req'),g('wall_us_per_req'))
fit('coord_us_per_req vs spec_checks_per_req',g('spec_checks_per_req'),g('coord_us_per_req'))
fit('commit_us_per_rec vs spec_checks_per_req',g('spec_checks_per_req'),g('commit_us_per_rec'))
fit('commit_us_per_rec vs commit_fwd_callbacks_per_req',g('commit_fwd_callbacks_per_req'),g('commit_us_per_rec'))
fit('prep_us_per_batch vs spec_checks_per_req',g('spec_checks_per_req'),g('prep_us_per_batch'))
fit('commit_us_per_rec vs sched',g('sched'),g('commit_us_per_rec'))
fit('wall_us_per_req vs sched (loglog)',g('sched'),g('wall_us_per_req'),True)
fit('wall_us_per_req vs live_cand (loglog)',g('live_cand'),g('wall_us_per_req'),True)
fit('req_s vs pending (loglog)',g('pending'),g('req_s'),True)
fit('compl_h vs pending (loglog)',g('pending'),g('compl_h'),True)
fit('compl_h vs sched (loglog)',g('sched'),g('compl_h'),True)
fit('compl_h vs 1/(wall_us_per_req*req_per_compl)',[3600e6/(w['wall_us_per_req']*w['req_per_compl']) if w.get('req_per_compl') else None for w in W],g('compl_h'))
fit('computing_mean vs wall_us_per_req (loglog)',g('wall_us_per_req'),g('computing_mean'),True)
fit('dpend_per_compl vs h',g('h'),g('dpend_per_compl'))
fit('dpend_per_compl vs req_per_compl',g('req_per_compl'),g('dpend_per_compl'))
fit('new share vs h',g('h'),g('new'))
fit('rss vs sched', g('sched'), [w['rss_GB'] for w in W])
fit('edges vs sched', g('sched'), g('edges'))
# multi regression wall_us_per_req ~ a + b*spec + c*commit_fwd
pts=[w for w in W if w.get("commit_fwd_callbacks_per_req") is not None]
if not pts: pts=None
if pts: b,r2=ols([[1,w["spec_checks_per_req"],w["commit_fwd_callbacks_per_req"],w['sched']/1e6] for w in pts],[w['wall_us_per_req'] for w in pts])
if pts: print('wall_us_per_req = %.3g + %.4g*spec + %.4g*commit_fwd + %.4g*sched_M  R2=%.3f'%(b[0],b[1],b[2],b[3],r2))
b,r2=ols([[1,w['spec_checks_per_req'],w['sched']/1e6] for w in W],[w['wall_us_per_req'] for w in W])
print('wall_us_per_req = %.3g + %.4g*spec + %.4g*sched_M  R2=%.3f'%(b[0],b[1],b[2],r2))
