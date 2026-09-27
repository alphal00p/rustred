import json, collections, re, sys, time
from fractions import Fraction as F
exec(open('routes.py').read().split('# traffic per route')[0])  # reuse C, sel, mul, inv, row, slotidx, act
def rank_basis(idxs):
    # greedy pick 5 independent slot vectors from idxs
    basis=[]; mat=[]
    for i in idxs:
        cand=mat+[[F(x) for x in C[i]]]
        # rank check via elimination
        M_=[r[:] for r in cand]; r=0
        for c in range(5):
            p=next((k for k in range(r,len(M_)) if M_[k][c]!=0),None)
            if p is None: continue
            M_[r],M_[p]=M_[p],M_[r]
            for k in range(len(M_)):
                if k!=r and M_[k][c]!=0:
                    f=M_[k][c]/M_[r][c]; M_[k]=[a-f*b for a,b in zip(M_[k],M_[r])]
            r+=1
        if r==len(cand):
            basis.append(i); mat=cand
        if len(basis)==5: break
    return basis
def automorphisms(mask):
    A=act(mask); Aset=set(A)
    B=rank_basis(A)
    if len(B)<5: return None
    Bm=[[F(x) for x in C[i]] for i in B]; Binv=inv(Bm)
    # coefficients of every active slot in the basis: c_j = a_j . B  -> a_j = c_j . Binv
    coef={j:row([F(x) for x in C[j]],Binv) for j in A}
    signed=[(i,s) for i in A for s in (1,-1)]
    out=[]
    def rec(k,assign,used):
        if k==5:
            Bp=[[s*F(x) for x in C[i]] for (i,s) in assign]
            T=mul(Binv,Bp)
            imgs={}
            for j in A:
                v=tuple(row([F(x) for x in C[j]],T))
                t=slotidx.get(v)
                if t is None or t not in Aset: return
                imgs[j]=t
            if len(set(imgs.values()))!=len(A): return
            out.append(T); return
        for (i,s) in signed:
            if i in used: continue
            assign.append((i,s))
            # prune: slots whose coefficient support is within first k+1 basis elements
            ok=True
            for j in A:
                a=coef[j]
                if all(a[m]==0 for m in range(k+1,5)):
                    v=[sum(a[m]*s2*C[i2][c] for m,(i2,s2) in enumerate(assign)) for c in range(5)]
                    t=slotidx.get(tuple(v))
                    if t is None or t not in Aset: ok=False; break
            if ok: rec(k+1,assign,used|{i})
            assign.pop()
    rec(0,[],set())
    return out
t0=time.time()
owners=sorted({r['owner_mask'] for r in sel['initial_frontier_routes']})
AUT={}
for o in owners:
    AUT[o]=automorphisms(o)
    print(o, o.count('1'), 'auts', None if AUT[o] is None else len(AUT[o]), f'{time.time()-t0:.1f}s', flush=True)
import pickle; pickle.dump({k:[[[str(x) for x in r] for r in T] for T in v] if v else v for k,v in AUT.items()}, open('auts.pkl','wb'))
