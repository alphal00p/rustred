import sys,os,mmap
from array import array
from multiprocessing import Pool
HDR=32; INIT=67
BOUNDS=[0,1,16,256,1024,4096,16384,65536,262144,1048576,4194304,1<<62]
def load(args):
    path,first,count=args
    with open(path,"rb") as f:
        mm=mmap.mmap(f.fileno(),0,access=mmap.ACCESS_READ)
        a=array("I"); a.frombytes(mm[HDR+8*first:HDR+8*(first+count)]); mm.close()
    return a
def pass1(args):
    a=load(args); return max(a[1::2])
def pass2(args):
    (path,first,count),start_max=args
    a=load((path,first,count)); s=a[0::2]; t=a[1::2]
    m=start_max; hist=[0]*len(BOUNDS); new=0; selfe=0
    for x,y in zip(s,t):
        if y>m:
            m=y; new+=1; continue
        if x==y: selfe+=1
        age=m-y
        # bucket
        i=0
        while age>=BOUNDS[i+1]: i+=1
        hist[i]+=1
    return hist,new,selfe,count
tasks=[]
for p in sys.argv[1:]:
    n=(os.path.getsize(p)-HDR)//8; step=20_000_000
    for i in range(0,n,step): tasks.append((p,i,min(step,n-i)))
with Pool(40) as pool:
    maxes=pool.map(pass1,tasks)
    prefix=[]; m=INIT-1
    for x in maxes: prefix.append(m); m=max(m,x)
    res=pool.map(pass2,list(zip(tasks,prefix)))
hist=[0]*len(BOUNDS); new=selfe=tot=0
for h,n_,se,c in res:
    hist=[a+b for a,b in zip(hist,h)]; new+=n_; selfe+=se; tot+=c
print("edges",tot,"creation edges",new,"self",selfe)
hits=sum(hist); cum=0
for i in range(len(BOUNDS)-1):
    cum+=hist[i]
    print(f"  age [{BOUNDS[i]:>8},{BOUNDS[i+1]:>8}) {hist[i]:12d} {100*hist[i]/hits:6.2f}% cum {100*cum/hits:6.2f}%")
