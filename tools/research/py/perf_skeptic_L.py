import json
L={int(x) for x in open('inputs_lens/L.txt').read().strip().split(',')}
own={}
for line in open('inputs_lens/owners.tsv'):
    f=line.split('\t'); own[f[1]]=int(f[0])
for g in (3,4,5,6):
    try: d=json.load(open(f'env{g}.json'))
    except Exception as e: print(g,e); continue
    po=d['per_owner']; tot=sum(v[3] for v in po.values()); ts=sum(v[4] for v in po.values()); tn=sum(v[0] for v in po.values())
    ls=sum(v[3] for k,v in po.items() if own.get(k) in L); lsu=sum(v[4] for k,v in po.items() if own.get(k) in L); ln=sum(v[0] for k,v in po.items() if own.get(k) in L)
    hot=po.get('011101110111000',[0,0,0,0,0])
    top=sorted(po.items(),key=lambda kv:-kv[1][3])[:3]
    print(f"gen{g}: apply n={tn} sec={tot:.0f} succ={ts}; L share: n={ln/tn:.3f} sec={ls/tot:.3f} succ={lsu/ts:.3f}; hot owner sec share={hot[3]/tot:.3f} succ share={hot[4]/ts:.3f}; route_sec={d.get('route_seconds')} route_n={d.get('route')}")
    print('   top3 by sec:',[(k,own.get(k),round(v[3]),v[0]) for k,v in top])
