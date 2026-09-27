import sys, json, struct, array, collections
recfn = sys.argv[1]; edgefns = sys.argv[2:]
apply = {}  # id -> successors
route = set()
n=0
with open(recfn,'rb') as f:
    for line in f:
        n+=1
        if n>600000: break
        d=json.loads(line)
        if d.get('record_kind')!='native_inspection': continue
        if d['phase']=='Apply':
            apply[d['id']] = ((d.get('stats') or {}).get('successors') or 0, d.get('seconds') or 0.0, d.get('owner'))
        else:
            route.add(d['id'])
print('apply', len(apply), 'route', len(route))
cnt = collections.Counter(); rcnt = collections.Counter()
helper_hits = collections.Counter()
for fn in edgefns:
    with open(fn,'rb') as f:
        hdr=f.read(32)
        _,_,_,_,_,count,first = struct.unpack('<4s4sHHIQQ',hdr)
        rem = count
        while rem>0:
            k=min(rem, 16_000_000); buf=f.read(k*8); rem-=k
            a=array.array('I'); a.frombytes(buf)
            src=a[0::2]; tgt=a[1::2]
            for s,t in zip(src,tgt):
                if s in apply:
                    cnt[s]+=1
                    if t<67: helper_hits[s]+=1
                elif s in route:
                    rcnt[s]+=1
    print('read', fn)
tot_s=0; tot_e=0; covered=0; ratios=[]
heavy_s=0; heavy_e=0
for i,(s,sec,o) in apply.items():
    if i in cnt:
        covered+=1; tot_s+=s; tot_e+=cnt[i]; 
        if s>0: ratios.append(cnt[i]/s)
        if s>=10000: heavy_s+=s; heavy_e+=cnt[i]
ratios.sort()
q=lambda p: ratios[min(len(ratios)-1,int(p*len(ratios)))]
print('apply sources with edges', covered, 'successors', tot_s, 'distinct targets', tot_e, 'targets/successor %.4f'%(tot_e/tot_s))
print('per-source ratio quantiles p10 %.3f p50 %.3f p90 %.3f'%(q(.1),q(.5),q(.9)))
print('heavy (>=10k successors) sources: successors', heavy_s, 'targets', heavy_e, 'ratio %.4f'%(heavy_e/heavy_s if heavy_s else 0))
print('route sources with edges', len(rcnt), 'edges', sum(rcnt.values()), 'per route %.2f'%(sum(rcnt.values())/max(1,len(rcnt))))
print('apply edges to helpers', sum(helper_hits.values()))
