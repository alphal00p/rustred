import json,sys,mmap
from array import array
rec_path,edge_path,start_max=sys.argv[1],sys.argv[2],int(sys.argv[3])
info={}
with open(rec_path,"rb") as f:
    for line in f:
        if b'"record_kind":"native_inspection"' not in line: continue
        r=json.loads(line)
        ph=r["phase"]; st=r["stats"]
        succ=st.get("successors",r["accepted_events"]) if ph=="Apply" else r["accepted_events"]
        info[r["id"]]=(ph,succ,r["accepted_events"],r["seconds"])
print("records",len(info))
HDR=32
with open(edge_path,"rb") as f:
    mm=mmap.mmap(f.fileno(),0,access=mmap.ACCESS_READ); a=array("I"); a.frombytes(mm[HDR:]); mm.close()
s=a[0::2]; t=a[1::2]
m=start_max-1
distinct={}; new={}
for x,y in zip(s,t):
    distinct[x]=distinct.get(x,0)+1
    if y>m:
        m=y; new[x]=new.get(x,0)+1
print("sources",len(distinct),"edges",len(s))
agg={"Apply":[0,0,0,0,0],"Route":[0,0,0,0,0]}
# per-source ratios histogram for Apply
import math
hist_new={}; hist_d={}
for sid,(ph,succ,ev,sec) in info.items():
    d=distinct.get(sid,0); n=new.get(sid,0)
    A=agg[ph]; A[0]+=1; A[1]+=succ; A[2]+=ev; A[3]+=d; A[4]+=n
    if ph=="Apply" and succ>0:
        fr=n/succ; b=-1 if fr==0 else int(math.floor(math.log10(fr)*2))
        hv=hist_new.setdefault(b,[0,0,0]); hv[0]+=1; hv[1]+=succ; hv[2]+=sec
        fd=d/succ; b2=int(math.floor(fd*10)) if fd<1 else 10
        hd=hist_d.setdefault(b2,[0,0]); hd[0]+=1; hd[1]+=succ
for ph,A in agg.items():
    print(ph,"n",A[0],"succ/events",A[1],A[2],"distinct targets",A[3],"new",A[4],"distinct/events %.4f new/events %.5f"%(A[3]/max(1,A[2]),A[4]/max(1,A[2])))
print("Apply: per-source new/successors fraction histogram (half-decade bins; -1 = zero new)")
tot=sum(v[1] for v in hist_new.values())
for b in sorted(hist_new):
    v=hist_new[b]; lo=0 if b==-1 else 10**(b/2)
    print(f"  >= {lo:.4g}: sources {v[0]} succ {v[1]} ({100*v[1]/tot:.2f}% of successors) seconds {v[2]:.1f}")
print("Apply: distinct-containers/successors per source (tenths)")
for b in sorted(hist_d):
    v=hist_d[b]; print(f"  [{b/10:.1f},{(b+1)/10:.1f}) sources {v[0]} succ {v[1]} ({100*v[1]/tot:.2f}%)")
