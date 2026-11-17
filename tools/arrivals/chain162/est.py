# lane arrivals: start-set bounds of the not-yet-walked chain nodes from the arrival files on disk (counts from each
# file's summary line: sum = upper bound of the incoming union, max = lower bound); box sizes from finished rows summaries.
import json, os, sys
R = sys.argv[1]
plan = [json.loads(l) for l in open(R + '/plan.jsonl')]
def keys(f):
    try:
        with open(f, 'rb') as h:
            h.seek(0, 2); n = h.tell(); h.seek(max(0, n - 65536)); last = h.read().splitlines()[-1]
        return json.loads(last)['keys']
    except (OSError, ValueError, KeyError, IndexError):
        return None
for o in plan:
    x = o['node']
    inc = [l.strip() for l in open(o['incoming']) if l.strip()]
    ks = [keys(f) for f in inc]
    have = [k for k in ks if k is not None]
    print(f"{x} L{o['lines']} wave={o['wave']} in_edges={len(inc)} files_on_disk={len(have)} "
          f"incoming_sum={sum(have)} incoming_max={max(have) if have else 0} done={os.path.exists(f'{R}/{x}/done')}")
