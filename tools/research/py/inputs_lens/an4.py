import json
I=json.load(open('interim_ts.json')); V=json.load(open('v2_ts.json'))
def at_completed(S, c):
    for r in S:
        if r['completed_nodes'] and r['completed_nodes']>=c: return r
    return None
def fmt(r):
    if not r: return 'n/a'
    c=r['completed_nodes']
    return f"t={r['t']/3600:5.1f}h comp={c/1e6:6.2f}M disc={r['scheduled_nodes']/1e6:6.2f}M pend={r['queued_nodes']/1e6:6.2f}M disc/comp={r['scheduled_nodes']/c:4.2f} roots={r['roots']} closed={r['total_closed']/1e6:5.2f}M front={r['frontiers']} rank={r['max_scheduled_finite_rank']} ev/comp={r['events']/c:5.0f} succ/comp={r['successors']/c:5.0f} fo_hits/ev={r['full_orthant_hits']/r['events']:.2f} pre/ev={r['pre_admitted_orthant_hits']/r['events']:.3f} exact/ev={r['exact_domain_hits']/r['events']:.3f} sem/ev={r['containment_semantic_hits']/r['events']:.3f} cchk/ev={r['containment_checks']/r['events']:6.0f} dedup/ev={r['deduplication_hits']/r['events']:.2f} edges={r['edges']/1e6:6.0f}M rss={r['rss']/1e9:5.0f}GB"
for c in [1e6,2e6,4e6,6e6,8e6,10e6,12e6,14e6,17e6,20e6,24e6,27e6]:
    print(f"== completed >= {c/1e6:.0f}M"); print(' interim', fmt(at_completed(I,c))); print(' v2     ', fmt(at_completed(V,c)))
print()
for h in [1,2,4,6,8,12,16,18.5]:
    ri=min(I,key=lambda r:abs(r['t']-h*3600)); rv=min(V,key=lambda r:abs(r['t']-h*3600))
    print(f"== {h} h"); print(' interim', fmt(ri)); print(' v2     ', fmt(rv))
