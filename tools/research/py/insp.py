import re, sys, json
kind_re = re.compile(rb'"record_kind":"([a-z_]+)"')
sec_re = re.compile(rb'"seconds":([0-9.eE+-]+)')
succ_re = re.compile(rb'"stats":\{.*?"successors":([0-9]+)')
ev_re = re.compile(rb'"events":([0-9]+)')
kinds = {}
secs = []
succ = []
n = 0
for path in sys.argv[1:]:
    with open(path, 'rb') as f:
        for line in f:
            n += 1
            k = kind_re.search(line)
            k = k.group(1).decode() if k else 'none'
            kinds[k] = kinds.get(k, 0) + 1
            if k == 'native_inspection':
                s = sec_re.search(line)
                if s: secs.append(float(s.group(1)))
                m = succ_re.search(line)
                if m: succ.append(int(m.group(1)))
secs.sort(); succ.sort()
def q(a, p): return a[min(len(a)-1, int(p*len(a)))] if a else None
tot = sum(secs)
top = secs[int(0.99*len(secs)):]
print(json.dumps({"lines": n, "kinds": kinds, "native": len(secs),
  "seconds_sum": tot, "mean": tot/len(secs) if secs else None,
  "p50": q(secs,.5), "p90": q(secs,.9), "p99": q(secs,.99), "p999": q(secs,.999), "max": secs[-1] if secs else None,
  "share_of_time_in_top1pct": sum(top)/tot if tot else None,
  "succ_mean": sum(succ)/len(succ) if succ else None, "succ_p50": q(succ,.5), "succ_p99": q(succ,.99), "succ_max": succ[-1] if succ else None}, indent=1))
