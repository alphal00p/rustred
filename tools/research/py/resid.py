import json, sys, collections
q=json.load(open('/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-v2/inputs/queries.json'))['queries']
helper={e['owner']:(e['max_numerator_rank'],e['power_bounds']['max_positive_power']) for e in q if e['id'].startswith('owner-anchor')}
sec_rank_inside=0.0; sec_rank_out=0.0; n_in=0; n_out=0
margin_sec=collections.Counter(); margin_n=collections.Counter()
minA_sec=collections.Counter()
lowA_inside=0; lowA_sec=0.0
tot=0.0
for line in open(sys.argv[1],'rb'):
    r=json.loads(line)
    if r.get('record_kind')!='native_inspection' or r.get('phase')!='Apply': continue
    o=r['owner']; hr,ha=helper[o]; s=r.get('seconds') or 0.0; tot+=s
    rk=r.get('rank'); A=r['power_bounds']['max_positive_power']
    # minimal A of the box: sum over active axes of (1+lower)
    act=[c=='1' for c in o]
    amin=sum(1+l for l,a in zip(r['lower'],act) if a)
    if rk is not None and rk<=hr:
        sec_rank_inside+=s; n_in+=1
        if ha is not None and A is not None:
            m=A-ha; margin_sec[m]+=s; margin_n[m]+=1
            if amin<=ha: lowA_inside+=1; lowA_sec+=s
    else:
        sec_rank_out+=s; n_out+=1
print(json.dumps({'total_apply_seconds':round(tot,1),'rank<=helper: n,sec':[n_in,round(sec_rank_inside,1)],'rank>helper: n,sec':[n_out,round(sec_rank_out,1)],
 'A_margin_over_helper (rank-inside) n':dict(sorted(margin_n.items())),'A_margin sec':{k:round(v,1) for k,v in sorted(margin_sec.items())},
 'rank-inside records whose box also has points with A<=helperA: n,sec':[lowA_inside,round(lowA_sec,1)]}))
