import json,sys
a=json.load(open(sys.argv[1]))
print("succ histogram Apply (log2 bin of successors, count, sum s, sum succ, us/succ)")
rows=sorted(((int(k.split("|")[1]),v) for k,v in a["succ_hist"].items() if k.startswith("Apply")))
T=sum(v[1] for _,v in rows); S=sum(v[2] for _,v in rows)
for b,v in rows:
    print(f"  2^{b:3d} n={v[0]:9d} s={v[1]:10.1f} ({100*v[1]/T:5.2f}%) succ={v[2]:12d} ({100*v[2]/S:5.2f}%) us/succ={1e6*v[1]/max(1,v[2]):8.1f} mean s={v[1]/v[0]:.4f}")
print("\nTop owners by Apply seconds")
own=sorted(a["owner"].items(), key=lambda kv:-kv[1][1])
for k,v in own[:15]:
    print(f"  {k} n={v[0]:8d} s={v[1]:10.1f} ({100*v[1]/T:5.2f}%) succ={v[2]:11d} us/succ={1e6*v[1]/max(1,v[2]):6.1f} mean_s={v[1]/v[0]:.4f} term/succ={v[4]/max(1,v[2]):.2f}")
print("owners total",len(own))
print("\nBy rank")
for k,v in sorted(a["rank"].items(), key=lambda kv: int(kv[0]) if kv[0]!='None' else 99):
    print(f"  R={k:>4} n={v[0]:8d} s={v[1]:10.1f} ({100*v[1]/T:5.2f}%) succ={v[2]:11d} mean_s={v[1]/v[0]:.4f} succ/insp={v[2]/v[0]:.0f}")
print("\nBy A max")
for k,v in sorted(a["apow"].items(), key=lambda kv: int(kv[0]) if kv[0]!='None' else 99):
    print(f"  A={k:>4} n={v[0]:8d} s={v[1]:10.1f} ({100*v[1]/T:5.2f}%) succ={v[2]:11d} mean_s={v[1]/v[0]:.4f}")
print("\nTop 25 inspections")
for t in a["top"][:25]:
    s,i,seg,ph,own,rank,pb,lo,up,ev,st,mc=t
    print(f"  {s:7.1f}s id={i} {seg[8:28]} {ph} {own} R={rank} A={pb.get('max_positive_power')} D={pb.get('max_power_difference')} ev={ev} succ={st['successors']} tv={st['term_visits']} sel={st['selected_pieces']} mcells={mc} us/succ={1e6*s/max(1,st['successors']):.1f}")
    print("        lower",lo,"upper",up)
