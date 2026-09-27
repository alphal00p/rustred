import json,sys
W=json.load(open(sys.argv[1]))
cols=['h','sched','live_cand','pending','req_s','compl_h','wall_us_per_req','req_per_compl','spec_checks_per_req','commit_fwd_callbacks_per_req','fwd_bit_reject','commit_us_per_rec','prep_us_per_batch','recs_per_batch','maint_per_new','spec_rev_per_new','computing_mean','duty_ordered_commit','duty_preparation','duty_wait','contain','exact','new','dpend_per_compl','succ_per_compl','rss_GB','jl_per_compl']
print('\t'.join(c[:10] for c in cols))
for d in W:
    out=[]
    for c in cols:
        v=d.get(c)
        if v is None: out.append('-')
        elif isinstance(v,float):
            out.append(f'{v:.3g}')
        else: out.append(str(v))
    print('\t'.join(out))
