# Band sizes of lane basis-census (its own epsband.py, imported read-only) per eps node of eps_nodes_full2.json:
# total keys and the D <= 4 subset. Inspect-only.
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("epsband", "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/basis-census/tools/epsband.py")
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
nodes = json.load(open('/common/build-cache/vhirschi/p_integral_dev/basis-census/eps_nodes_full2.json'))
out = open(sys.argv[1], 'w')
for t, info in sorted(nodes.items()):
    b = m.node_band(info)
    tot = d4 = 0
    for (D, r) in b['cells']:
        n = len(m.cell_keys(info, b, D, r)); tot += n
        if D <= 4: d4 += n
    out.write(json.dumps({"node": t, "E": b['E'], "e": b['e'], "weights": sorted(w for _, w in b['epsl']), "census_keys": tot, "census_keys_D4": d4}) + "\n"); out.flush()
