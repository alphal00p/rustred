import json,sys
a=json.load(open(sys.argv[1]))
an=a["apply_stat_names"]; rn=a["route_stat_names"]
print("delegated",a["delegated"])
for k in sorted(a["phase"]):
    v=a["phase"][k]; names=an if k.endswith("Apply") else rn
    d=dict(zip(names,v))
    print(k, {n:(round(x,1) if isinstance(x,float) else x) for n,x in d.items()})
A=dict(zip(an,a["phase"]["ALL|Apply"])); R=dict(zip(rn,a["phase"]["ALL|Route"]))
T=A["seconds"]+R["seconds"]
print("total seconds",T,"apply share",A["seconds"]/T, "route share",R["seconds"]/T)
print("apply mean s",A["seconds"]/A["count"],"route mean s",R["seconds"]/R["count"])
print("apply succ/insp",A["successors"]/A["count"],"us per successor",1e6*A["seconds"]/A["successors"])
print("zero_terms/term_visits",A["zero_terms"]/A["term_visits"],"coalescing/term",A["coalescing_additions"]/A["term_visits"],"cancelled_groups",A["cancelled_groups"])
print("term_visits/shift_groups",A["term_visits"]/A["shift_groups"],"successors/term_visits",A["successors"]/A["term_visits"],"succ/boundary",A["successors"]/A["boundary_cells"])
print("conditional/succ",A["conditional_successors"]/A["successors"],"same_support/succ",A["same_support_successors"]/A["successors"],"strict_sub/succ",A["strict_subsupport_successors"]/A["successors"])
print("native_ops/succ",A["native_operations"]/A["successors"],"m_cells/insp",A["m_cells"]/A["count"],"m_coord/insp",A["m_coordinate_cells"]/A["count"],"pieces/insp",A["m_pieces"]/A["count"],"selected/insp",A["selected_pieces"]/A["count"])
print("route events/insp",R["events"]/R["count"],"route us per event",1e6*R["seconds"]/max(1,R["events"]))
print()
for ph in ("Apply","Route"):
    rows=sorted(((int(k.split("|")[1]),v) for k,v in a["sec_hist"].items() if k.startswith(ph)))
    tot=sum(v[1] for _,v in rows); cnt=sum(v[0] for _,v in rows); succ=sum(v[2] for _,v in rows)
    print(ph,"seconds histogram (bin lower edge s, count, %count, sum s, %s, cum %s from top, %succ)")
    cum=0
    for b,v in reversed(rows):
        cum+=v[1]
        lo=10**(b/4) if b>-99 else 0
        print(f"  >={lo:10.4g}s n={v[0]:10d} {100*v[0]/cnt:7.3f}%  s={v[1]:12.1f} {100*v[1]/tot:6.2f}%  cum={100*cum/tot:6.2f}%  succ%={100*v[2]/max(1,succ):6.2f}")
