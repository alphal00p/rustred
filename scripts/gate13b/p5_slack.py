"""gate13b item 6: for every group signal recorded AFTER its attempt's finished record, the delay (record utc - finish
utc) and the number of supervisor records between; read-only on the given campaign directories."""
import datetime, glob, json, sys

def t(u):
    return datetime.datetime.fromisoformat(u.replace("Z", "+00:00"))

for cdir in sys.argv[1:]:
    att, late, n_group = {}, [], 0
    recs = []
    for f in sorted(glob.glob(cdir + "/ledger/segment-*.jsonl")):
        for line in open(f, errors="replace"):
            try:
                recs.append(json.loads(line))
            except ValueError:
                pass
    for r in recs:
        e = r.get("event", {}); b = e.get("body") or {}
        if e.get("kind") == "job" and isinstance(b.get("ev"), dict):
            k = (b["job"], b["attempt"]); a = att.setdefault(k, {})
            ev = b["ev"]
            if ev.get("ev") == "started":
                a["pid"] = ev.get("pid")
            elif ev.get("ev") == "finished":
                a["end"], a["end_utc"] = r["seq"], t(r["utc"])
        elif e.get("kind") == "signal_sent" and b.get("target") == "group":
            n_group += 1
            for k, a in att.items():
                if k[0] == b.get("job") and a.get("pid") == b.get("pid") and a.get("end") is not None and r["seq"] > a["end"]:
                    late.append(((t(r["utc"]) - a["end_utc"]).total_seconds(), r["seq"] - a["end"], b.get("signal"), b.get("by", "")[:40]))
    late.sort()
    print(f"{cdir}: group signals {n_group}; recorded after the attempt's finish: {len(late)}; "
          f"max delay {late[-1][0] if late else None} s; max records between {max((x[1] for x in late), default=None)}")
    for x in late[-5:]:
        print("   ", x)
