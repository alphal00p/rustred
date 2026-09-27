import re,sys,math,collections
for path in sys.argv[1:]:
    rows=collections.defaultdict(list)
    for line in open(path):
        if not line.startswith("SCALE "): continue
        f=dict(kv.split("=") for kv in line.split()[2:] if "=" in kv)
        key=line.split()[1]
        rows[key].append((int(f["log10pts"]),float(f["n"]),float(f["mean_points"]),float(f["mean_seconds"]),float(f["mean_successors"]),float(f["sec_share"])))
    out=[]
    tot=sum(sum(r[1]*r[3] for r in v) for k,v in rows.items() if k!="ALL")
    for k,v in rows.items():
        v=[r for r in v if r[1]>=10 and r[2]>0 and r[3]>0]
        if len(v)<3: continue
        # weighted least squares on log-log of bin means, weight = n
        W=sum(r[1] for r in v); x=[math.log(r[2]) for r in v]; y=[math.log(r[3]) for r in v]; ys=[math.log(max(r[4],1e-9)) for r in v]
        mx=sum(r[1]*xi for r,xi in zip(v,x))/W; my=sum(r[1]*yi for r,yi in zip(v,y))/W; mys=sum(r[1]*yi for r,yi in zip(v,ys))/W
        sxx=sum(r[1]*(xi-mx)**2 for r,xi in zip(v,x)); sxy=sum(r[1]*(xi-mx)*(yi-my) for r,xi,yi in zip(v,x,y)); sxs=sum(r[1]*(xi-mx)*(yi-mys) for r,xi,yi in zip(v,x,ys))
        secs=sum(r[1]*r[3] for r in rows[k])
        out.append((secs,k,sxy/sxx,sxs/sxx,min(r[2] for r in v),max(r[2] for r in v)))
    out.sort(reverse=True)
    print("==",path,"total owner Apply seconds",round(tot))
    cum=0
    for secs,k,b,bs,lo,hi in out[:14]:
        cum+=secs if k!="ALL" else 0
        print(f"{k:32s} secs={secs:9.0f} ({100*secs/tot:5.1f}%) cost_exp={b:5.2f} succ_exp={bs:5.2f} pts {lo:.0f}..{hi:.0f}")
