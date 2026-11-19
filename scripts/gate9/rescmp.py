import json, sys
# rescmp.py REF.result OTHER.result : compares accepted / dispositions / terminals of `rr_campaign result` JSON
try:
    a = json.load(open(sys.argv[1])); b = json.load(open(sys.argv[2]))
except Exception as e:
    print(f"UNREADABLE ({e})"); sys.exit(0)
k = lambda d: (d.get("accepted"), d.get("dispositions"), d.get("terminals"))
print(("RESULT_SAME" if k(a) == k(b) else "RESULT_DIFFERS") + f" (accepted ref {len(a.get('accepted') or {})}, other {len(b.get('accepted') or {})})")
