import json,sys
path=sys.argv[1]
last=None
keys=set()
n=0
for l in open(path):
    try: e=json.loads(l)
    except: continue
    if e.get('event')!='heartbeat': continue
    n+=1
    p=e.get('progress',{})
    if n in (10,3000,6000): 
        def walk(d,pre=''):
            for k,v in d.items():
                if isinstance(v,dict): walk(v,pre+k+'.')
                else: keys.add(pre+k)
        walk(e)
    cl=None
    s=json.dumps(e)
    for k in ('initial_closed','closed_initial','roots_closed'):
        pass
print(sorted(k for k in keys if 'clos' in k or 'duty' in k or 'active' in k or 'comput' in k or 'initial' in k)[:80])
