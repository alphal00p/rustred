import json, sys, collections, time
path = sys.argv[1]
t0 = time.time()
maxA = {}
maxR = {}
cnt = collections.Counter()
sec = collections.Counter()
succ = collections.Counter()
ahist = collections.Counter()
rhist = collections.Counter()
route_n = 0
route_sec = 0.0
deleg = 0
ids = [None, None]
unb_rank = 0
unb_A = 0
with open(path, "rb") as f:
    for line in f:
        try:
            r = json.loads(line)
        except Exception:
            continue
        i = r.get("id")
        if i is not None:
            if ids[0] is None or i < ids[0]:
                ids[0] = i
            if ids[1] is None or i > ids[1]:
                ids[1] = i
        k = r.get("record_kind")
        if k != "native_inspection":
            deleg += 1
            continue
        if r.get("phase") != "Apply":
            route_n += 1
            route_sec += r.get("seconds") or 0.0
            continue
        o = r["owner"]
        a = (r.get("power_bounds") or {}).get("max_positive_power")
        rk = r.get("rank")
        cnt[o] += 1
        sec[o] += r.get("seconds") or 0.0
        succ[o] += (r.get("stats") or {}).get("successors", 0)
        if a is None:
            unb_A += 1
        else:
            maxA[o] = max(maxA.get(o, 0), a)
            ahist[a] += 1
        if rk is None:
            unb_rank += 1
        else:
            maxR[o] = max(maxR.get(o, 0), rk)
            rhist[rk] += 1
out = {"path": path, "elapsed": round(time.time() - t0, 1), "id_range": ids,
       "apply": sum(cnt.values()), "apply_seconds": round(sum(sec.values()), 1),
       "apply_successors": sum(succ.values()), "route": route_n, "route_seconds": round(route_sec, 1),
       "non_native": deleg, "unbounded_A_apply": unb_A, "unbounded_rank_apply": unb_rank,
       "A_hist": dict(sorted(ahist.items())), "R_hist": dict(sorted(rhist.items())),
       "per_owner": {o: [cnt[o], maxA.get(o), maxR.get(o), round(sec[o], 1), succ[o]] for o in cnt}}
print(json.dumps(out))
