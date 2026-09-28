#!/usr/bin/env bash
# Summarize runs.jsonl of scripts/run.sh as Markdown tables (awk only).
# CPU per op = total thread CPU time of the timed loop / total operations (mean over repeats).
# "x K=1" = ratio to the same variant and work at K=1; "x private" = ratio to `private` at the same K.
# busy = max foreign busy % on the run CPUs in the 1 s before / after each run (the run itself excluded).
# perf columns (if recorded) are per operation and include set-up and warm-up (upper bounds).
set -euo pipefail
DIR=${1:?usage: summarize.sh <run dir>}
awk '
function val(s, key,   m) {
  if (match(s, "\"" key "\":\"?[^,\"}]*")) { m = substr(s, RSTART, RLENGTH); sub(/^"[^"]*":"?/, "", m); return m }
  return ""
}
{
  v = val($0, "variant"); w = val($0, "work"); k = val($0, "threads") + 0
  key = w SUBSEP v SUBSEP k
  n[key]++; cpu[key] += val($0, "cpu_ns_per_op")
  c = val($0, "cpu_ns_per_op") + 0
  if (!(key in lo) || c < lo[key]) lo[key] = c
  if (!(key in hi) || c > hi[key]) hi[key] = c
  b = val($0, "busy_pre_pct") + 0; if (val($0, "busy_post_pct") + 0 > b) b = val($0, "busy_post_pct") + 0
  if (b > busy[key]) busy[key] = b
  ops = val($0, "ops_per_thread") * k
  cyc = val($0, "perf_cycles"); ins = val($0, "perf_instructions")
  if (cyc != "") {
    hasperf = 1
    pc[key] += cyc / ops; pi[key] += ins / ops
    px[key] += (val($0, "perf_ls_dmnd_fills_from_sys_near_cache") + val($0, "perf_ls_dmnd_fills_from_sys_far_cache")) / ops
    pn[key]++
  }
  chk[key] = chk[key] (chk[key] == "" ? "" : "/") val($0, "checksum")
  works[w] = 1; variants[v] = 1; ks[k] = 1
}
END {
  nk = 0; for (k in ks) kl[++nk] = k + 0
  for (i = 1; i <= nk; i++) for (j = i + 1; j <= nk; j++) if (kl[j] < kl[i]) { t = kl[i]; kl[i] = kl[j]; kl[j] = t }
  split("full specialize split", wo, " "); split("shared private-ctx rehome rehome-api private", vo, " ")
  for (wi = 1; wi <= 3; wi++) {
    w = wo[wi]; if (!(w in works)) continue
    printf "\n### work = %s\n\n", w
    printf "| variant | K | runs | CPU/op ns (mean) | min-max | x K=1 | x private | busy %% |%s\n", (hasperf ? " cycles/op | instr/op | IPC | x-CCX fills/op |" : "")
    printf "|---|---:|---:|---:|---|---:|---:|---:|%s\n", (hasperf ? "---:|---:|---:|---:|" : "")
    for (vi = 1; vi <= 5; vi++) {
      v = vo[vi]; if (!(v in variants)) continue
      for (i = 1; i <= nk; i++) {
        k = kl[i]; key = w SUBSEP v SUBSEP k; if (!(key in n)) continue
        m = cpu[key] / n[key]
        b1 = w SUBSEP v SUBSEP 1; r1 = (b1 in n) ? sprintf("%.2f", m / (cpu[b1] / n[b1])) : "-"
        bp = w SUBSEP "private" SUBSEP k; rp = (bp in n) ? sprintf("%.2f", m / (cpu[bp] / n[bp])) : "-"
        extra = ""
        if (hasperf && pn[key] > 0) extra = sprintf(" %.3g | %.3g | %.2f | %.1f |", pc[key] / pn[key], pi[key] / pn[key], pi[key] / pc[key], px[key] / pn[key])
        else if (hasperf) extra = " - | - | - | - |"
        printf "| %s | %d | %d | %.0f | %.0f-%.0f | %s | %s | %.0f |%s\n", v, k, n[key], m, lo[key], hi[key], r1, rp, busy[key], extra
      }
    }
  }
  # identity: every variant must produce the same checksum at the same K and work
  bad = 0
  for (key in chk) { split(key, p, SUBSEP); split(chk[key], cs, "/"); for (x in cs) { id = p[1] SUBSEP p[3]; if (!(id in ref)) ref[id] = cs[x]; else if (ref[id] != cs[x]) bad++ } }
  printf "\nchecksum identity across variants and repeats: %s\n", (bad ? bad " MISMATCH(ES)" : "all equal per (work, K)")
}' "$DIR/runs.jsonl"
