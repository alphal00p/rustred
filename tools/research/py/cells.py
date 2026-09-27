import re, collections, sys
p='TMP/v2-checkpoint-copy-gen3/records-00000000000000000003.jsonl'
rx=re.compile(r'"owner":"([01]+)","phase":"(\w+)","power_bounds":\{"max_positive_power":(null|\d+),"max_power_difference":(null|-?\d+),"min_power_difference":(null|-?\d+)\},"rank":(null|\d+),"record_kind":"(\w+)","seconds":([0-9.e+-]+)')
cells=collections.Counter(); secs=collections.defaultdict(float)
kinds=collections.Counter(); n=0; miss=0
by_t=collections.Counter(); secs_t=collections.defaultdict(float)
masks=set()
with open(p,'rb') as f:
    for line in f:
        n+=1
        m=rx.search(line.decode())
        if not m: miss+=1; continue
        owner,phase,A,dmax,dmin,R,kind,s=m.groups()
        kinds[(phase,kind)]+=1
        key=(phase,owner,R,A)
        cells[key]+=1; secs[key]+=float(s)
        t=owner.count('1'); by_t[(phase,t)]+=1; secs_t[(phase,t)]+=float(s)
        masks.add((phase,owner))
print('records',n,'unparsed',miss)
print('kinds',kinds)
print('distinct (phase,mask)',len(masks),'distinct (phase,mask,R,A) keys',len(cells))
tot=sum(secs.values()); print('total inspection seconds',tot)
top=sorted(secs.items(), key=lambda kv:-kv[1])[:15]
for k,v in top: print('key',k,'records',cells[k],'sec',round(v,1))
for k in sorted(by_t): print('t',k,by_t[k],round(secs_t[k],1))
