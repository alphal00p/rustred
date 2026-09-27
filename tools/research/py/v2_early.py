import json,sys
marks=[900,1800,2700,3600,5400,7200,10800,21600]; j=0
for l in open(sys.argv[1]):
    if '"event":"heartbeat"' not in l: continue
    try: e=json.loads(l)
    except: continue
    t=e.get('elapsed_seconds',0)
    if j<len(marks) and t>=marks[j]:
        par=e.get('progress',{}).get('parallel',{})
        print(round(t), 'expanded', e.get('expanded_nodes'), 'discovered', e.get('currently_discovered_nodes'), 'rss_GB', round(e.get('process_rss_bytes',0)/1e9,1), 'active', par.get('active_workers'))
        j+=1
        if j==len(marks): break
