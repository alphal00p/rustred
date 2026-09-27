import json,sys
path=sys.argv[1]
first=None; marks=[1800,3600,7200,14400,28800,57600,80000]; j=0
for l in open(path):
    if '"heartbeat"' not in l[:400] and '"event":"heartbeat"' not in l: 
        continue
    try: e=json.loads(l)
    except: continue
    if e.get('event')!='heartbeat': continue
    p=e.get('progress',{}); w=p.get('work',{}) if isinstance(p.get('work'),dict) else {}
    fr=w.get('frontiers', p.get('frontiers'))
    t=e.get('elapsed_seconds',0)
    if fr and first is None:
        first=(t,fr); print('first frontier', first)
    if j<len(marks) and t>=marks[j]:
        print(round(t), 'frontiers', fr, 'expanded', e.get('expanded_nodes')); j+=1
