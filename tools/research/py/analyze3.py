import json,sys
a=json.load(open(sys.argv[1])); N=a["names"]
print("records",a["records"])
for k,v in a["phase"].items():
    if k.startswith("other"): print(k,v); continue
    d=dict(zip(N,v)); print(k,{x:(round(y,1) if isinstance(y,float) else y) for x,y in d.items() if y})
A=dict(zip(N,a["phase"]["Apply"])); R=dict(zip(N,a["phase"]["Route"]))
print("Apply mean s",A["seconds"]/A["count"],"us/succ",1e6*A["seconds"]/A["successors"],"succ/insp",A["successors"]/A["count"],"zero/term",A["zero_terms"]/A["term_visits"])
print("Route total s",R["seconds"],"Apply total s",A["seconds"])
for ph in ("Apply",):
    rows=sorted(((int(k.split("|")[1]),v) for k,v in a["sec_hist"].items() if k.startswith(ph)))
    tot=sum(v[1] for _,v in rows); cnt=sum(v[0] for _,v in rows); succ=sum(v[2] for _,v in rows); cum=0
    for b,v in reversed(rows):
        cum+=v[1]; lo=10**(b/4) if b>-99 else 0
        print(f"  >={lo:10.4g}s n={v[0]:9d} {100*v[0]/cnt:7.3f}% s={v[1]:10.1f} {100*v[1]/tot:6.2f}% cum={100*cum/tot:6.2f}% succ%={100*v[2]/succ:6.2f} us/succ={1e6*v[1]/max(1,v[2]):7.1f}")
    rows=sorted(((int(k.split("|")[1]),v) for k,v in a["succ_hist"].items() if k.startswith(ph)))
    print(" by successor count")
    for b,v in rows:
        print(f"  2^{b:3d} n={v[0]:9d} s={v[1]:10.1f} ({100*v[1]/tot:5.2f}%) succ%={100*v[2]/succ:5.2f} us/succ={1e6*v[1]/max(1,v[2]):7.1f} mean_s={v[1]/v[0]:.4f}")
print("rank")
for k,v in sorted(a["rank"].items()):
    print("  R",k,v[0],round(v[1],1),v[2], "mean_s",round(v[1]/v[0],4))
print("top")
for t in a["top"][:15]: print("  ",t)
