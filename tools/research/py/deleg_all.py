import sys,os,mmap,re
from array import array
from multiprocessing import Pool
HDR=32; INIT=67
def load(args):
    path,first,count=args
    with open(path,"rb") as f:
        mm=mmap.mmap(f.fileno(),0,access=mmap.ACCESS_READ); a=array("I"); a.frombytes(mm[HDR+8*first:HDR+8*(first+count)]); mm.close()
    return a
def pass1(args): a=load(args); return max(a[1::2])
def pass2(args):
    task,m=args; a=load(task); out=array("I")
    for x,y in zip(a[0::2],a[1::2]):
        if y>m: m=y; out.append(y); out.append(x)
    return out.tobytes()
pid=re.compile(rb'"id":(\d+)'); prep=re.compile(rb'"representative_id":(\d+)'); pph=re.compile(rb'"phase":"Apply"')
def recs(args):
    path,start,end=args; out=array("I")
    with open(path,"rb") as f:
        f.seek(start)
        if start: f.readline()
        pos=f.tell()
        while pos<=end:
            line=f.readline()
            if not line: break
            pos+=len(line)
            if b'delegated_not_inspected' not in line: continue
            out.append(int(pid.search(line).group(1))); out.append(int(prep.search(line).group(1))); out.append(1 if pph.search(line) else 0)
    return out.tobytes()
d="campaigns/five-loop-qcd-feynman-d9d10-v2/checkpoints/main/"
edges=[d+f"edges-0000000000000000000{g}.bin" for g in range(3,8)]
records=[d+f"records-0000000000000000000{g}.jsonl" for g in range(3,8)]
tasks=[]
for p in edges:
    n=(os.path.getsize(p)-HDR)//8
    for i in range(0,n,20_000_000): tasks.append((p,i,min(20_000_000,n-i)))
rt=[]
for p in records:
    size=os.path.getsize(p); n=max(1,size//(256<<20)); step=size//n
    for i in range(n): rt.append((p,i*step,size if i==n-1 else (i+1)*step-1))
creator=array("I",[0xFFFFFFFF])*75_000_000
with Pool(40) as pool:
    maxes=pool.map(pass1,tasks); prefix=[]; m=INIT-1
    for x in maxes: prefix.append(m); m=max(m,x)
    for b in pool.imap(pass2,list(zip(tasks,prefix))):
        a=array("I"); a.frombytes(b)
        for y,x in zip(a[0::2],a[1::2]): creator[y]=x
    tot=same=known=0; ph={0:[0,0,0],1:[0,0,0]}
    for b in pool.imap_unordered(recs,rt):
        a=array("I"); a.frombytes(b)
        for i,r,p in zip(a[0::3],a[1::3],a[2::3]):
            tot+=1; ci=creator[i]; cr=creator[r]; v=ph[p]; v[0]+=1
            if ci!=0xFFFFFFFF and cr!=0xFFFFFFFF:
                known+=1; v[1]+=1
                if ci==cr: same+=1; v[2]+=1
print("delegated",tot,"known creators",known,"same creator",same,"frac %.4f"%(same/max(1,known)))
print("Route [n,known,same]",ph[0],"Apply",ph[1])
