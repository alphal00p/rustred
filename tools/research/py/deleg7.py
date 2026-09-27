import json,sys,mmap,re
from array import array
rec_path,edge_path,start=sys.argv[1],sys.argv[2],int(sys.argv[3])
HDR=32
with open(edge_path,"rb") as f:
    mm=mmap.mmap(f.fileno(),0,access=mmap.ACCESS_READ); a=array("I"); a.frombytes(mm[HDR:]); mm.close()
s=a[0::2]; t=a[1::2]
creator=array("I",[0xFFFFFFFF])*(80_000_000-start)
m=start-1
for x,y in zip(s,t):
    if y>m:
        m=y; creator[y-start]=x
print("max id",m)
pid=re.compile(rb'"id":(\d+)'); prep=re.compile(rb'"representative_id":(\d+)'); pph=re.compile(rb'"phase":"(\w+)"')
tot=same=known=0; byph={}
with open(rec_path,"rb") as f:
    for line in f:
        if b'delegated_not_inspected' not in line: continue
        i=int(pid.search(line).group(1)); r=int(prep.search(line).group(1)); ph=pph.search(line).group(1).decode()
        tot+=1
        if i>=start and r>=start:
            ci=creator[i-start]; cr=creator[r-start]
            if ci!=0xFFFFFFFF and cr!=0xFFFFFFFF:
                known+=1
                v=byph.setdefault(ph,[0,0]); v[0]+=1
                if ci==cr: same+=1; v[1]+=1
print("delegated records",tot,"both created in segment",known,"same creator",same, "frac %.3f"%(same/max(1,known)), byph)
