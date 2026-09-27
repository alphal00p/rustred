import sys,os,mmap
from array import array
from multiprocessing import Pool
HDR=32; INIT=67
def work(args):
    path,first,count=args
    with open(path,"rb") as f:
        mm=mmap.mmap(f.fileno(),0,access=mmap.ACCESS_READ)
        a=array("I"); a.frombytes(mm[HDR+8*first:HDR+8*(first+count)])
        mm.close()
    s=a[0::2]; t=a[1::2]
    selfe=anchor=older=newer=0
    for x,y in zip(s,t):
        if x==y: selfe+=1
        elif y<INIT: anchor+=1
        elif y<x: older+=1
        else: newer+=1
    return (selfe,anchor,older,newer,count)
tasks=[]
for p in sys.argv[1:]:
    n=(os.path.getsize(p)-HDR)//8; step=20_000_000
    for i in range(0,n,step): tasks.append((p,i,min(step,n-i)))
tot=[0]*5
with Pool(40) as pool:
    for r in pool.imap_unordered(work,tasks):
        tot=[a+b for a,b in zip(tot,r)]
print("edges",tot[4],"self",tot[0],"anchor",tot[1],"older(t<s)",tot[2],"newer(t>s)",tot[3])
print({k:round(100*v/tot[4],2) for k,v in zip(["self","anchor","older","newer"],tot[:4])})
