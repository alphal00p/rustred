import json,sys,os,random
from multiprocessing import Pool
# sample Apply native records from JSONL segments: every K-th
K=int(os.environ.get("K","20"))
def work(args):
    path,start,end=args; rows=[]; n=0
    with open(path,"rb") as f:
        f.seek(start)
        if start: f.readline()
        pos=f.tell()
        while pos<=end:
            line=f.readline()
            if not line: break
            pos+=len(line)
            if b'"record_kind":"native_inspection"' not in line or b'"phase":"Apply"' not in line: continue
            n+=1
            if n%K: continue
            r=json.loads(line); st=r["stats"]; m=st["matching"]
            rows.append([r["seconds"],st["successors"],st["term_visits"],st["zero_terms"],st["shift_groups"],st["selected_pieces"],m["cells"],m["predicates"],m["rules"],m["coordinate_cells"],st["sign_splits"],st["native_operations"],r["owner"],r.get("rank"),r["power_bounds"].get("max_positive_power")])
    return rows
files=sys.argv[2:]; tasks=[]
for p in files:
    size=os.path.getsize(p); n=max(1,size//(256<<20)); step=size//n
    for i in range(n): tasks.append((p,i*step,size if i==n-1 else (i+1)*step-1))
out=[]
with Pool(40) as pool:
    for r in pool.imap_unordered(work,tasks): out.extend(r)
json.dump(out,open(sys.argv[1],"w")); print(len(out))
