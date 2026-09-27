import re, sys, collections
rx=re.compile(rb'"owner":"([01]+)","phase":"(\w+)","power_bounds":\{"max_positive_power":(null|\d+),"max_power_difference":(null|-?\d+),"min_power_difference":(null|-?\d+)\},"rank":(null|\d+),"record_kind":"native_inspection","seconds":([0-9.e+-]+)')
keys=collections.Counter(); n=0; nat=collections.Counter(); secs=collections.Counter(); masks=set(); anull=0; rnull=0
for s in sys.argv[1:]:
    with open(s,'rb') as f:
        for line in f:
            m=rx.search(line)
            if not m: continue
            owner,phase,A,dmax,dmin,R,sec=m.groups()
            n+=1; nat[phase]+=1; secs[phase]+=float(sec)
            keys[(phase,owner,R,A)]+=1; masks.add((phase,owner))
            if A==b'null': anull+=1
            if R==b'null': rnull+=1
print('native records',n,dict(nat),{k.decode() if isinstance(k,bytes) else k:round(v) for k,v in secs.items()})
print('distinct (phase,mask)',len(masks),'distinct (phase,mask,R,A)',len(keys), 'by phase',collections.Counter(k[0] for k in keys))
print('records with A cap null',anull,'R cap null',rnull)
