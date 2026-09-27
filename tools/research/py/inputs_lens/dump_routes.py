exec(open('routes.py').read().split('routes=sel[')[0])
with open('routes.txt','w') as f:
    f.write(' '.join(' '.join(map(str,c)) for c in C)+'\n')
    for r in sel['initial_frontier_routes']:
        As=[x for row_ in r['source_to_representative'] for x in row_]; Ao=[x for row_ in r['owner_to_representative'] for x in row_]
        f.write(f"{r['source_mask']} {r['owner_mask']} {traffic.get(r['source_mask'],0)} {rr.get(r['source_mask'],0)} {' '.join(As)} {' '.join(Ao)}\n")
print('ok')
