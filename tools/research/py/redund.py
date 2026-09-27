import json, sys, collections
acc={}
for line in sys.stdin:
    r=json.loads(line)
    if r.get('record_kind')!='native_inspection': continue
    k=(r['owner'],r['rank'],r['power_bounds']['max_positive_power'])
    up=tuple(-1 if u is None else u for u in r['upper']); lo=tuple(r['lower'])
    a=acc.get(k)
    if a is None:
        a=acc[k]={'n':0,'sec':0.0,'max':0.0,'maxbox':None,'lo':list(lo),'up':list(up),'succ':0}
    a['n']+=1; s=r['seconds']; a['sec']+=s; a['succ']+=(r.get('stats') or {}).get('successors',0)
    if s>a['max']: a['max']=s; a['maxbox']=(lo,up)
    a['lo']=[min(x,y) for x,y in zip(a['lo'],lo)]
    a['up']=[(-1 if (x==-1 or y==-1) else max(x,y)) for x,y in zip(a['up'],up)]
tot=sum(a['sec'] for a in acc.values()); totmax=sum(a['max'] for a in acc.values())
hullmax=sum(1 for a in acc.values() if a['maxbox']==(tuple(a['lo']),tuple(a['up'])))
print('apply keys',len(acc),'total sec',round(tot),'sum of per-key max sec',round(totmax),'keys whose slowest fragment equals hull box',hullmax)
# weighted by seconds: share of seconds in keys where slowest fragment equals hull
w=sum(a['sec'] for a in acc.values() if a['maxbox']==(tuple(a['lo']),tuple(a['up'])))
print('seconds share in such keys',round(w/tot,3))
big=sorted(acc.items(), key=lambda kv:-kv[1]['sec'])
cum=0
for i,(k,a) in enumerate(big[:25]):
    cum+=a['sec']
    print(k,'n',a['n'],'sum',round(a['sec']),'max',round(a['max'],1),'ratio',round(a['sec']/max(a['max'],1e-9)),'max==hull',a['maxbox']==(tuple(a['lo']),tuple(a['up'])),'cum',round(cum/tot,3))
nkeys=[len([1 for a in acc.values() if a['sec']>x]) for x in (1,10,100,1000)]
print('keys with sum sec >1,10,100,1000:',nkeys)
