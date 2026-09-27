import json, sys, collections
OWN='011101110111000'
act=[c=='1' for c in OWN]; t=sum(act)
keys={(4,13),(5,13),(4,14),(3,14)}
def dist(ranges):
    d={0:1}
    for lo,up in ranges:
        nd=collections.Counter()
        for s,c in d.items():
            for v in range(lo,up+1): nd[s+v]+=c
        d=nd
    return d
def points(lo,up,r,a,dmin,dmax):
    # clamp unbounded axes by caps
    ar=[]; ir=[]
    for i in range(15):
        u=up[i]
        if act[i]:
            cap=a-t if a is not None else 60
            ar.append((lo[i], min(u,cap) if u is not None else cap))
        else:
            cap=r if r is not None else 60
            ir.append((lo[i], min(u,cap) if u is not None else cap))
    if any(l>u for l,u in ar+ir): return 0
    dx=dist(ar); dy=dist(ir); n=0
    for sx,cx in dx.items():
        A=t+sx
        if a is not None and A>a: continue
        for sy,cy in dy.items():
            if r is not None and sy>r: continue
            D=A-sy
            if dmin is not None and D<dmin: continue
            if dmax is not None and D>dmax: continue
            n+=cx*cy
    return n
acc=collections.defaultdict(lambda:[0,0,0.0])
for line in sys.stdin:
    rec=json.loads(line)
    if rec.get('record_kind')!='native_inspection' or rec['owner']!=OWN: continue
    pb=rec['power_bounds']; k=(rec['rank'],pb['max_positive_power'])
    if k not in keys: continue
    p=points(rec['lower'],rec['upper'],rec['rank'],pb['max_positive_power'],pb['min_power_difference'],pb['max_power_difference'])
    a=acc[k]; a[0]+=1; a[1]+=p; a[2]+=rec['seconds']
for k,(n,p,s) in sorted(acc.items()):
    cell=points([0]*15,[None]*15,k[0],k[1],None,None)
    print('key R<=%d A<=%d'%k,'fragments',n,'sum fragment points',p,'cell points',cell,'avg multiplicity',round(p/cell,1),'sec',round(s))
