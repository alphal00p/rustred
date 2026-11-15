#!/usr/bin/env python3
"""Lane allnodes: one JSON line per node + a readable table from the sweep's run directories (inspect-only).
The LATEST run directory holding a node's final.json wins (pass 2 over pass 1). Ledger certificate = P5L_STATUS:
closed with `residual_masters` terminals (else close.residuals, else boxed.terminals for boxed nodes).
reproduces = yes iff the tool's verdict is closed (Tier C), or Tier K superset clean with no open key when Tier C was not
reached (then `Tier K only`), AND the terminal count equals the ledger's.
Usage: summarize.py P5L_STATUS.json specs/nodes.jsonl specs/not_run.jsonl OUT_PREFIX RUN_DIR [RUN_DIR ...]
"""
import json
import os
import re
import sys

st = {x["id"]: x for x in json.load(open(sys.argv[1]))["nodes"]}
specs = [json.loads(l) for l in open(sys.argv[2])]
not_run = [json.loads(l) for l in open(sys.argv[3])]
outp = sys.argv[4]
runs = sys.argv[5:]
cen_path = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(sys.argv[2]))), "census.jsonl")
census = {}
if os.path.exists(cen_path):
    for l in open(cen_path):
        try:
            c = json.loads(l)
            census[c["table"]] = c
        except ValueError:
            pass


def ledger_terminals(n):
    x = st[n]
    if x.get("residual_masters") is not None:
        return x["residual_masters"], "residual_masters"
    for k in ("close", "boxed"):
        v = x.get(k)
        if isinstance(v, dict):
            for f in ("residuals", "terminals"):
                if v.get(f) is not None:
                    return v[f], f"{k}.{f}"
    return None, "none"


def cpu_s(path):
    try:
        t = open(path).read()
    except OSError:
        return None
    m = re.search(r"CPU time consumed: ([0-9.hmin ]+?s)\b", t)
    if not m:
        return None
    tot = 0.0
    for v, u in re.findall(r"([0-9.]+)(h|min|ms|s)", m.group(1)):
        tot += float(v) * {"h": 3600, "min": 60, "s": 1, "ms": 0.001}[u]
    return tot


rows = []
for sp in specs:
    n = sp["node"]
    src = None
    for rd in runs:
        if os.path.exists(os.path.join(rd, "nodes", n, "final.json")):
            src = rd
    lt, lfrom = ledger_terminals(n)
    row = {"node": n, "tables": 1, "table_bytes": sp["bytes"], "box": f"BOXDEF.v3 A ({sp['support_class']}, L={sp['L']})",
           "box_keys_expected": sp["box_keys_expected"], "table_terminals": sp["table_terminals"],
           "ledger_terminals": lt, "ledger_terminals_from": lfrom, "sha_from": sp["sha_from"]}
    cb = census.get(sp["table"])
    row["rules_with_domain_bounds"] = cb["rules_with_domain_bounds"] if cb else None
    row["domain_bounds_nontrivial_target"] = cb["with_nontrivial_target"] if cb else None
    if src is None:
        row["status"] = "pending"
        row["reproduces"] = "pending"
        rows.append(row)
        continue
    nd = os.path.join(src, "nodes", n)
    fin = json.load(open(os.path.join(nd, "final.json")))
    lines = [json.loads(l) for l in open(os.path.join(nd, "results.jsonl"))] if os.path.exists(
        os.path.join(nd, "results.jsonl")) else []
    k = next((l for l in lines if l["mode"] == "k"), None)
    cs = [l for l in lines if l["mode"] == "c"]
    c = cs[-1] if cs else None
    z2 = next((l for l in lines if l["mode"] == "z2"), None)
    row["run"] = os.path.basename(src)
    row["prefetch"] = fin.get("prefetch", "off")
    row["budget"] = fin.get("budget")
    row["stop"] = fin["stop"]
    row["rounds"] = fin["rounds"]
    row["requested_keys"] = fin["requested_keys"]
    row["wall_s"] = fin["wall_s"]
    cpu = [cpu_s(os.path.join(nd, f)) for f in os.listdir(nd) if f.startswith("unit.")]
    row["cpu_s"] = sum(x for x in cpu if x) if any(cpu) else None
    rss = [l.get("peak_rss_kb", 0) for l in lines]
    for f in os.listdir(nd):
        if f.startswith("proof.") and f.endswith(".log"):
            m = re.search(r'"peak_rss_mb":([0-9.]+)', open(os.path.join(nd, f)).read())
            if m:
                rss.append(int(float(m.group(1)) * 1024))
    row["peak_rss_gb"] = round(max(rss) / 1048576, 2) if rss else None
    if k is None or "error" in k:
        err = (k or {}).get("error") or (c or {}).get("error") or "no Tier K line"
        row.update({"status": "refused", "load": "refused", "error": err, "reproduces": f"no (refused: {err[:200]})"})
        rows.append(row)
        continue
    row["load"] = "ok"
    ld = k.get("load")
    if ld:
        row["loader_bounds_records"] = sum(x.get("bounds_records") or 0 for x in ld)
        row["files_without_end"] = sum(len(x.get("files_without_end") or []) for x in ld)
    # end_record: the loader's files_without_end when the line has it, else the record-kind scan (scan.jsonl kinds.end)
    if row.get("files_without_end") is not None:
        row["end_record"] = "no" if row["files_without_end"] else "yes"
    else:
        row["end_record"] = "yes" if sp.get("kinds", {}).get("end") else "no"
        row["end_record_from"] = "record-kind scan"
    row["records"] = k.get("records")
    row["box_keys"] = k.get("keys_in_start")
    row["closure_keys_k"] = k.get("keys")
    row["tier_k"] = k.get("verdict_if_not_diagnostic")
    row["tier_k_open"] = k.get("open")
    row["tier_k_open_by_status"] = k.get("open_by_status")
    row["terminals_k"] = k.get("terminals")
    row["descent_violations_k"] = k.get("descent_violations")
    row["tier_k_s"] = k.get("seconds")
    term = k.get("terminals")
    tc = None
    if c is not None and "error" in c:
        tc = f"Tier K only (Tier C error: {c['error'][:120]})"
    elif fin["stop"] == "decided" and c is not None:
        row["closure_keys_c"] = c.get("keys")
        row["terminals_c"] = c.get("terminals")
        row["terminal_keys"] = c.get("terminal_keys")
        row["descent_violations_c"] = c.get("descent_violations")
        row["zero_terms"] = c.get("zero_terms")
        row["decisive_zeros"] = c.get("decisive_zeros")
        term = c.get("terminals")
        if c.get("verdict") == "closed":
            tc = "closed"
        else:
            tc = f"open {c.get('open')} {c.get('open_by_status')} blockers {c.get('blockers')}"
        dz = c.get("decisive_zeros") or 0
        if dz == 0:
            row["zeros"] = "decisive 0 (second route: nothing to confirm)"
        elif z2 is not None and "second_route" in z2:
            s = z2["second_route"]
            row["zeros"] = (f"decisive {dz}: {s['confirmed']}/{s['pairs']} pairs confirmed ({s['routes']}), "
                            f"{s['disagreements']} disagree, {s['unanswered']} unanswered")
            if s["disagreements"] or s["unanswered"]:
                tc = f"open (second route: {s['disagreements']} disagree, {s['unanswered']} unanswered)"
        else:
            row["zeros"] = f"single-route zeros: {dz} (second route {fin['second_route']})"
            tc = f"open (single-route decisive zeros: {dz})"
    else:
        tc = {"over_budget": "Tier K only (budget)", "tier_k_only_by_brief": "Tier K only (brief: cheap tier)",
              "max_rounds": "Tier K only (max rounds)"}.get(fin["stop"], f"Tier K only ({fin['stop']})")
    row["tier_c"] = tc
    row["terminals_tool"] = term
    reasons = []
    if tc == "closed":
        level = "Tier C"
    elif tc.startswith("Tier K only"):
        level = "Tier K"
        if row["tier_k"] != "superset_clean" or row["tier_k_open"]:
            reasons.append(f"Tier K {row['tier_k']} open {row['tier_k_open']} {row['tier_k_open_by_status']}")
    else:
        level = "Tier C"
        reasons.append(f"Tier C {tc}")
    if lt is None:
        reasons.append("ledger has no terminal count")
    elif term != lt:
        reasons.append(f"terminals {term} vs ledger {lt}")
    row["status"] = "done"
    row["reproduces"] = f"yes ({level})" if not reasons else "no (" + "; ".join(reasons) + ")"
    rows.append(row)

for x in not_run:
    rows.append({"node": x["node"], "status": "not run", "why": x["why"], "reproduces": "n/a (no table in the ledgers)"})

with open(outp + ".jsonl", "w") as f:
    for r in rows:
        f.write(json.dumps(r) + "\n")
hdr = ["node", "MB", "box keys", "closure keys", "Tier K", "term tool/P5L", "Tier C", "req keys", "zeros", "s", "GB",
       "dom.bounds loader/scan (nontriv. target)", "end", "prefetch", "reproduces"]
CAVEAT = ("CAVEAT (implementer 2026-11-15T07:10Z; WS/notes/implementer.reaudit-part4.md B1/B2): the core's assembly-path reader "
          "drops rule-level domain_bounds and the engine reads bounds with target patterns differently from the core; whether "
          "the IMPORT dispatcher used here is affected is being established by lane partfix4. Column 'dom.bounds' = rule records "
          "carrying domain_bounds (of them with a non-trivial target pattern): the nodes that could be affected.")
out = [CAVEAT, "", "Box: BOXDEF.v3 dataset A_integer, enlarged form (L=5 clean: 0<=D<=4, r<=E-7 at every D, power<=5). "
       "Tier K = keys-only superset (witness check off, diagnostic). Tier C = strict, rr_proof 7b facts.", "",
       "| " + " | ".join(hdr) + " |", "|" + "---|" * len(hdr)]
for r in rows:
    if r["status"] in ("pending", "not run"):
        out.append(f"| {r['node']} | | | | | | {r['status']} | | | | | | | | {r.get('reproduces', '')} |")
        continue
    ck = r.get("closure_keys_c") or r.get("closure_keys_k")
    tk = f"{r.get('tier_k')} open {r.get('tier_k_open')}" if r.get("load") == "ok" else "refused"
    out.append("| " + " | ".join(str(v) for v in [
        r["node"], round(r["table_bytes"] / 1e6, 1), r.get("box_keys"), ck, tk,
        f"{r.get('terminals_tool')}/{r['ledger_terminals']}", r.get("tier_c", r.get("error", ""))[:90],
        r.get("requested_keys"), r.get("zeros", "-"), r.get("wall_s"), r.get("peak_rss_gb"),
        f"{r.get('loader_bounds_records', '?')}/{r.get('rules_with_domain_bounds')} ({r.get('domain_bounds_nontrivial_target')})",
        r.get("end_record"), r.get("prefetch"), r["reproduces"][:120]]) + " |")
open(outp + ".md", "w").write("\n".join(out) + "\n")
done = [r for r in rows if r["status"] == "done"]
cnt = {}
for r in rows:
    rp = r["reproduces"]
    key = rp.split(")")[0] + ")" if rp.startswith("yes") else rp.split(" ")[0]
    cnt[key] = cnt.get(key, 0) + 1
cpu = sum(r.get("cpu_s") or 0 for r in rows)
print(json.dumps({"rows": len(rows), "done": len(done), "counts": cnt, "cpu_h_units": round(cpu / 3600, 2)}))
