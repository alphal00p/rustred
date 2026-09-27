import sys, json
for path in sys.argv[1:]:
    for l in open(path):
        d = json.loads(l)
        if d.get('kind') != 'pipeline':
            continue
        sh = ' '.join(f"{k[6:]}={d[k]:.3f}" for k in d if k.startswith('share_') and d[k] > 0.0005)
        print(f"{d['label']:12} {d['scope']:10} k={d['mru_k']:2} jobs={d['jobs']:8} skipped(unjoined={d['jobs_unjoined_skipped']},stopped={d['jobs_stopped_skipped']},err={d['jobs_error_skipped']}) req={d['requests']:10} cheap={d['cheap_share']:.3f} | {sh} | mru_t/r={d['mru_tests_per_req']:.2f} loc_t/r={d['local_tests_per_req']:.2f} hlp_t/r={d['helper_tests_per_req']:.2f}")
