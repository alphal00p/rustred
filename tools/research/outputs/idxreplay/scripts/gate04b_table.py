import json, sys
rows=[json.loads(l) for l in open(sys.argv[1] if len(sys.argv)>1 else 'gate04b.jsonl')]
tags=['t1','t48','t90','tf2.4','tf2.6','t90-native3.75','t1-k64','t1-cheap300','t90-cheap300']
desc={'t1':'1-thread costs','t48':'per-set factors 48 vs 1 thread','t90':'per-set factors 90 vs 1 thread','tf2.4':'scalar factor 2.4','tf2.6':'scalar factor 2.6',
      't90-native3.75':'90-thread factors, native x3.75 (gate 0.3)','t1-k64':'1 thread, MRU k=64','t1-cheap300':'1 thread, cheap tier 300 ns','t90-cheap300':'90-thread factors, cheap 300 ns'}
designs=['pipeline + SoA-pattern first-found','pipeline + SoA-id first-found','pipeline + SoA-id min-ID','pipeline + SoA-pattern min-ID','pipeline + L0 first-found',"today's engine (counters x L0 cost)"]
al=['0.44','0.65','0.69','0.73','0.82']
print('| cost setting | design | admission ms / native at 74M | share at 74M | share at 1G, alpha ' + ' / '.join(al) + ' |')
print('|---|---|---:|---:|---|')
for t in tags:
    for d in designs:
        r=[x for x in rows if x['tag']==t and x['design']==d]
        if not r: continue
        r=r[0]
        sh=' / '.join(f"{100*r['at_n1'][a]['share']:.1f}%" for a in al)
        print(f"| {desc[t]} | {d.replace('pipeline + ','pipeline, ')} | {r['admission_ms_74M']:.3f} | {100*r['share_74M']:.1f}% | {sh} |")
