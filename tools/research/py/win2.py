import json, sys
rows = json.load(open(sys.argv[1]))
full = [r for r in rows if r['req'] is not None and r['successors'] is not None]
step=3600; edges=[]; cur=full[0]; target=cur['t']+step
for r in full[1:]:
    if r['t']>=target: edges.append((cur,r)); cur=r; target=r['t']+step
print('%5s %9s %9s %8s %8s %7s %7s %7s' % ('h','succ/s','req/succ','us/succ','admit%','newdom/s','edges/s','dom(M)'))
for a,b in edges:
    dt=b['t']-a['t']; ds=b['successors']-a['successors']; dreq=b['req']-a['req']
    dadm=(b['commit']-a['commit'])+(b['prep']-a['prep'])
    dsch=b['scheduled']-a['scheduled']; de=(b['edges'] or 0)-(a['edges'] or 0)
    print('%5.1f %9.0f %9.3f %8.2f %8.1f %7.0f %7.0f %7.1f'%(b['t']/3600, ds/dt, dreq/ds if ds else 0, dadm/ds*1e6 if ds else 0, 100*dadm/dt, dsch/dt, de/dt, b['scheduled']/1e6))
