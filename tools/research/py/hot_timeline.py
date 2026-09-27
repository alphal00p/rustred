import json,sys
path=sys.argv[1]
rows=[]
for l in open(path):
    try: e=json.loads(l)
    except: continue
    if e.get('event')!='heartbeat': continue
    p=e.get('progress',{})
    rows.append((e.get('elapsed_seconds'), e.get('expanded_nodes'), e.get('currently_discovered_nodes'), p.get('pending_domains') if isinstance(p,dict) else None, e.get('process_rss_bytes')))
print(len(rows))
marks=[600,1800,3600,5400,7200,10800,13400]
j=0
for r in rows:
    if j<len(marks) and r[0] and r[0]>=marks[j]:
        print(r); j+=1
print(rows[-1])
