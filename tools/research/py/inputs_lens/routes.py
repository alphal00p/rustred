import json, collections, re
from fractions import Fraction as F
man=json.load(open('/common/dev/rustred/examples/input/tide_five_loop_manifest.json'))
def parse(mom):
    v=[0]*5
    for sign,var in re.findall(r'([+-]?)k(\d)', mom):
        v[int(var)-1]+= -1 if sign=='-' else 1
    return v
C=[parse(m['momentum']) for m in sorted(man['momenta'], key=lambda m:m['index_one_based'])]
sel=json.load(open('/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-v2/inputs/selection.json'))
def M(x): return [[F(int(a)) for a in r] for r in x]
def mul(A,B): return [[sum(A[i][k]*B[k][j] for k in range(5)) for j in range(5)] for i in range(5)]
def inv(A):
    n=5; A=[r[:]+[F(int(i==j)) for j in range(n)] for i,r in enumerate(A)]
    for c in range(n):
        p=next(r for r in range(c,n) if A[r][c]!=0); A[c],A[p]=A[p],A[c]
        pv=A[c][c]; A[c]=[x/pv for x in A[c]]
        for r in range(n):
            if r!=c and A[r][c]!=0:
                f=A[r][c]; A[r]=[a-f*b for a,b in zip(A[r],A[c])]
    return [r[n:] for r in A]
def row(v,X): return [sum(v[k]*X[k][j] for k in range(5)) for j in range(5)]
def col(X,v): return [sum(X[i][k]*v[k] for k in range(5)) for i in range(5)]
slotidx={}
for i,c in enumerate(C):
    slotidx[tuple(c)]=i; slotidx[tuple(-x for x in c)]=i
def act(mask): return [i for i,ch in enumerate(mask) if ch=='1']
# traffic per route source mask (edges out of route nodes of that mask) from census transitions
traffic=collections.Counter(); rr=collections.Counter()
for line in open('v2g7/transitions.tsv').readlines()[1:]:
    s,t,c=map(int,line.split())
    if s>=200000:
        m=s-200000; ms=''.join('1' if m>>i&1 else '0' for i in range(15))
        traffic[ms]+=c
        if t>=200000: rr[ms]+=c
routes=sel['initial_frontier_routes']
res=collections.Counter(); per=[]
variants=collections.Counter()
for r in routes:
    As=M(r['source_to_representative']); Ao=M(r['owner_to_representative'])
    sa=act(r['source_mask']); oa=set(act(r['owner_mask']))
    best=None
    for name,X in (('AoinvAs_col',mul(inv(Ao),As)),('AsinvAo_col',mul(inv(As),Ao)),('AoinvAs_row',mul(inv(Ao),As)),('AsinvAo_row',mul(inv(As),Ao)),('AsAoinv_row',mul(As,inv(Ao))),('AoAsinv_row',mul(Ao,inv(As))),('AsAoinv_col',mul(As,inv(Ao))),('AoAsinv_col',mul(Ao,inv(As)))):
        f=(lambda v: col(X,v)) if name.endswith('col') else (lambda v: row(v,X))
        imgs=[slotidx.get(tuple(f(C[j]))) for j in range(15)]
        ok=all(imgs[j] is not None and imgs[j] in oa for j in sa)
        if ok:
            best=(name,imgs); break
    if best is None:
        res['no_variant_maps_actives']+=1; continue
    variants[best[0]]+=1
    name,imgs=best
    isp=[j for j in range(15) if j not in sa]
    lit_isp=sum(1 for j in isp if imgs[j] is not None and imgs[j] not in oa)
    to_prop=sum(1 for j in isp if imgs[j] is not None and imgs[j] in oa)
    nonlit=sum(1 for j in isp if imgs[j] is None)
    kind='literal' if nonlit==0 and to_prop==0 else 'nonliteral'
    w=traffic.get(r['source_mask'],0)
    res[(kind,'routes')]+=1; res[(kind,'traffic')]+=w; res[(kind,'rr')]+=rr.get(r['source_mask'],0)
    per.append((w,r['source_mask'],r['owner_mask'],len(sa),lit_isp,to_prop,nonlit))
print('variants',variants); print(dict(res))
per.sort(reverse=True)
print('top traffic route sources: traffic src owner t lit_isp isp->prop nonliteral_isp')
for p in per[:25]: print(p)
hist=collections.Counter(); hw=collections.Counter()
for w,s,o,t,l,tp,n in per: hist[n]+=1; hw[n]+=w
print('nonliteral ISP count histogram (routes):',sorted(hist.items())); print('weighted by traffic:',sorted(hw.items()))
tot=sum(traffic.values()); print('total route-source traffic', tot, 'covered by records', sum(p[0] for p in per))
