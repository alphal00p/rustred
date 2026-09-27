import json,sys
rows=json.load(open(sys.argv[1]))
def solve(A,b):
    n=len(b); M=[A[i][:]+[b[i]] for i in range(n)]
    for c in range(n):
        p=max(range(c,n),key=lambda r:abs(M[r][c])); M[c],M[p]=M[p],M[c]
        for r in range(n):
            if r!=c and M[c][c]!=0:
                f=M[r][c]/M[c][c]; M[r]=[x-f*y for x,y in zip(M[r],M[c])]
    return [M[i][n]/M[i][i] for i in range(n)]
def fit(sel,label):
    feats=lambda r:[1.0, r[2], r[7], r[6]]
    names=["const","term_visits","m_predicates","m_cells"]
    k=len(names); XtX=[[0.0]*k for _ in range(k)]; Xty=[0.0]*k; n=0; sy=0
    for r in sel:
        x=feats(r); y=r[0]; n+=1; sy+=y
        for i in range(k):
            Xty[i]+=x[i]*y
            for j in range(k): XtX[i][j]+=x[i]*x[j]
    beta=solve(XtX,Xty)
    # contributions
    tot=[0.0]*k
    for r in sel:
        x=feats(r)
        for i in range(k): tot[i]+=beta[i]*x[i]
    ss_res=0; mean=sy/n; ss_tot=0
    for r in sel:
        x=feats(r); p=sum(b*v for b,v in zip(beta,x)); ss_res+=(r[0]-p)**2; ss_tot+=(r[0]-mean)**2
    print(f"{label}: n={n} sum_s={sy:.1f} R2={1-ss_res/ss_tot:.3f}")
    for nm,b,t in zip(names,beta,tot): print(f"   {nm:16s} coef={b*1e6:10.3f} us  share={100*t/sy:6.1f}%")
small=[r for r in rows if r[1]<8000]
fit(small,"all owners, successors<8000")
fit([r for r in small if r[12]=="011101110111000"],"hot owner, successors<8000")
fit([r for r in small if r[12]!="011101110111000"],"other owners, successors<8000")
big=[r for r in rows if r[1]>=8000]
fit(big,"all owners, successors>=8000 (backpressure-contaminated)")
