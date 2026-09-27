import json, sys, collections
keys={('011101110111000',3,14),('011101110111000',4,13),('010011111101011',5,14)}
acc={}
for line in sys.stdin:
    r=json.loads(line)
    if r.get('phase')!='Apply' or r.get('record_kind')!='native_inspection': continue
    k=(r['owner'],r['rank'],r['power_bounds']['max_positive_power'])
    if k not in keys: continue
    a=acc.setdefault(k,{'n':0,'lo':[10**9]*15,'up':[0]*15,'unb':[0]*15,'dmin':collections.Counter(),'dmax':collections.Counter(),'sec':0.0,'succ':0,'pieces':0,'secs':[]})
    a['n']+=1; a['sec']+=r['seconds']; a['secs'].append(r['seconds'])
    st=r.get('stats') or {}; a['succ']+=st.get('successors',0); a['pieces']+=st.get('selected_pieces',0)
    for i,(l,u) in enumerate(zip(r['lower'],r['upper'])):
        a['lo'][i]=min(a['lo'][i],l)
        if u is None: a['unb'][i]+=1
        else: a['up'][i]=max(a['up'][i],u)
    pb=r['power_bounds']; a['dmin'][pb['min_power_difference']]+=1; a['dmax'][pb['max_power_difference']]+=1
for k,a in acc.items():
    s=sorted(a['secs'],reverse=True)
    print(k,'n',a['n'],'sec',round(a['sec'],1),'succ',a['succ'],'pieces',a['pieces'])
    print('  lo',a['lo']); print('  up',a['up']); print('  unbounded counts',a['unb'])
    print('  dmin',dict(a['dmin'].most_common(6)),'dmax',dict(a['dmax'].most_common(6)))
    print('  top secs',[round(x,1) for x in s[:8]],'median',round(s[len(s)//2],3))
