import json,sys
path=sys.argv[1]
prev=None
for l in open(path):
    try: e=json.loads(l)
    except: continue
    if e.get('event')!='heartbeat': continue
    dc=e.get('progress',{}).get('descendant_closure',{})
    ic=dc.get('initial_closed'); tc=dc.get('total_closed')
    key=ic
    if key!=prev:
        print(round(e['elapsed_seconds']), ic, dc.get('initial_total'), tc, dc.get('total_domains'), e.get('expanded_nodes'), e.get('progress',{}).get('parallel',{}).get('active_workers'))
        prev=key
