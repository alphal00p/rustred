import sys, struct, json, collections, time
path=sys.argv[1]
t0=time.time()
data=open(path,'rb').read()
assert data[:4]==b'RRW5' and data[4:8]==b'INDX', data[:8]
arity=struct.unpack_from('<H',data,8)[0]
count=struct.unpack_from('<Q',data,16)[0]
pos=32
def uv():
    global pos
    b=data[pos]
    if b<251:
        pos+=1; return b
    if b==251:
        v=struct.unpack_from('<H',data,pos+1)[0]; pos+=3; return v
    if b==252:
        v=struct.unpack_from('<I',data,pos+1)[0]; pos+=5; return v
    if b==253:
        v=struct.unpack_from('<Q',data,pos+1)[0]; pos+=9; return v
    if b==254:
        lo,hi=struct.unpack_from('<QQ',data,pos+1); pos+=17; return lo|(hi<<64)
    raise ValueError('bad varint %d at %d'%(b,pos))
def sv():
    v=uv(); return (v>>1) ^ -(v&1)
def opt(f):
    global pos
    t=data[pos]; pos+=1
    if t==0: return None
    return f()
def upper():
    v=uv()
    return ('F',uv()) if v==0 else ('Inf',)
def lower():
    v=uv()
    return ('-Inf',) if v==0 else ('F',sv())
def signature():
    v=uv()
    if v==0: return ('Empty',)
    return (upper(),upper(),lower())
n=uv()
assert n==count,(n,count)
buckets=[]
for b in range(n):
    phase=uv()
    olen=uv(); owner=''.join('1' if data[pos+i] else '0' for i in range(olen)); pos+=olen
    ids_len=uv()
    for _ in range(ids_len): uv()
    ngroups=uv()
    groups=[]
    for g in range(ngroups):
        sig=signature()
        nblocks=uv()
        fills=[]; spans=[]; envw=[]
        for k in range(nblocks):
            ids=[uv() for _ in range(32)]
            ln=uv()
            ne=uv()
            env=[]
            for e in range(ne):
                a=uv(); c=uv(); d=opt(uv); f=opt(uv)
                env.append((a,c,d,f))
            fills.append(ln)
            if ln: spans.append(ids[ln-1]-ids[0])
            # envelope width: sum over axes of (max_lower-min_lower)
            envw.append(sum(c-a for a,c,d,f in env))
        live=uv()
        groups.append(dict(sig=sig,blocks=nblocks,live=live,fill=sum(fills),envw=sum(envw)/max(1,len(envw)),span=(sum(spans)/max(1,len(spans)))))
    blive=uv()
    orth=opt(uv)
    buckets.append(dict(phase=phase,owner=owner,live=blive,groups=len(groups),blocks=sum(g['blocks'] for g in groups),
        fill=sum(g['fill'] for g in groups),orthant=orth,
        maxgroup_blocks=max([g['blocks'] for g in groups] or [0]),
        mean_envw=(sum(g['envw']*g['blocks'] for g in groups)/max(1,sum(g['blocks'] for g in groups))),
        mean_span=(sum(g['span']*g['blocks'] for g in groups)/max(1,sum(g['blocks'] for g in groups)))))
assert pos==len(data),(pos,len(data))
tot_live=sum(b['live'] for b in buckets); tot_blocks=sum(b['blocks'] for b in buckets); tot_groups=sum(b['groups'] for b in buckets)
print(json.dumps(dict(seconds=round(time.time()-t0,1),arity=arity,buckets=n,live=tot_live,groups=tot_groups,blocks=tot_blocks,
   mean_fill=round(tot_live/max(1,tot_blocks),2),
   est_block_bytes=tot_blocks*(288+arity*48+16),
   apply_buckets=sum(1 for b in buckets if b['phase']==0),route_buckets=sum(1 for b in buckets if b['phase']==1))))
buckets.sort(key=lambda b:-b['live'])
cum=0
for b in buckets[:25]:
    cum+=b['live']
    print(json.dumps(dict(phase='Apply' if b['phase']==0 else 'Route',owner=b['owner'],live=b['live'],share=round(b['live']/tot_live,4),cum=round(cum/tot_live,3),
      groups=b['groups'],blocks=b['blocks'],fill=round(b['fill']/max(1,b['blocks']),1),maxgroup_blocks=b['maxgroup_blocks'],
      mean_env_width=round(b['mean_envw'],1),mean_id_span=round(b['mean_span']),orthant=b['orthant'])))
# distribution of live per bucket
import statistics
lives=[b['live'] for b in buckets]
print('buckets with live>0:',sum(1 for x in lives if x>0),'median live',statistics.median(lives),'top10 share',round(sum(lives[:10])/tot_live,3))
