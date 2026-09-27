import json, os, collections
f='TMP/v2-checkpoint-copy-gen3/records-00000000000000000003.jsonl'
size=os.path.getsize(f)
span=collections.Counter(); keys=collections.Counter(); n=0; unb=0
spanR=collections.Counter()
with open(f,'rb') as fh:
    for k in range(40):
        fh.seek(int(size*k/40))
        if k: fh.readline()
        for _ in range(25000):
            line=fh.readline()
            if not line: break
            r=json.loads(line)
            if r.get('record_kind')!='native_inspection': continue
            ph=r['phase']; own=r['owner']; lo=r['lower']; up=r['upper']
            act=[c=='1' for c in own]; m=sum(act)
            pb=r['power_bounds'] or {}
            acap=pb.get('max_positive_power'); rank=r.get('rank')
            Alo=m+sum(l for l,a in zip(lo,act) if a)
            if any(u is None for u,a in zip(up,act) if a): Ahi=acap
            else:
                Ahi=m+sum(u for u,a in zip(up,act) if a); Ahi=min(Ahi,acap) if acap is not None else Ahi
            Rlo=sum(l for l,a in zip(lo,act) if not a)
            if any(u is None for u,a in zip(up,act) if not a): Rhi=rank
            else:
                Rhi=sum(u for u,a in zip(up,act) if not a); Rhi=min(Rhi,rank) if rank is not None else Rhi
            n+=1
            if Ahi is None or Rhi is None: unb+=1; span[(ph,'unbounded')]+=1; continue
            s=(Ahi+Rhi)-(Alo+Rlo)
            span[(ph,min(s,20))]+=1
            spanR[(ph,min(Rhi-Rlo,10))]+=1
            keys[(m,Ahi+Rhi)]+=1
print('native sampled',n,'unbounded',unb)
for ph in ('Apply','Route'):
    print(ph,'P-span hist',sorted([(k[1],v) for k,v in span.items() if k[0]==ph],key=lambda x:str(x[0]).zfill(3)))
print('distinct (m,Pmax) keys',len(keys)); print(sorted(keys.items())[:5], sorted(keys.items())[-5:])
