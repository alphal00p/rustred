import json,sys
path=sys.argv[1]
s=0;n=0;last=None;keys=None
for l in open(path):
    try: e=json.loads(l)
    except: continue
    if e.get('event')!='heartbeat': continue
    par=e.get('progress',{}).get('parallel',{})
    a=par.get('active_workers')
    if a is not None: s+=a;n+=1
    last=par
print('mean active',s/max(n,1),n)
print(json.dumps(last)[:3000])
