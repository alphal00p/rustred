import json,sys
for run in sys.argv[1:]:
    xs=[];ys=[]
    for line in open(run+'/events.jsonl'):
        if '"heartbeat"' not in line: continue
        e=json.loads(line)
        if e.get('event')!='heartbeat': continue
        c=e.get('committed_domains') or e.get('progress',{}).get('committed_domains')
        r=e.get('process_rss_bytes')
        if c and r: xs.append(c); ys.append(r)
    n=len(xs)
    if n<2: print(run,'points',n); continue
    mx=sum(xs)/n; my=sum(ys)/n
    b=sum((x-mx)*(y-my) for x,y in zip(xs,ys))/sum((x-mx)**2 for x in xs)
    print(run,'points',n,'slope B/committed %.0f'%b,'max heartbeat rss',max(ys))
