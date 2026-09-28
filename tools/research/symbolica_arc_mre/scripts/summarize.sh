#!/usr/bin/env bash
# Summarize runs.jsonl of scripts/run.sh as Markdown tables (awk only).
# CPU per op = total thread CPU time (user + system) of the timed loop / total operations; the table
# shows the median over repeats and the min-max range; ratios use medians.
# Rows are keyed by variant, layout "[c<ctx offset>/m<map offset>]" (+ ",ahash-tl" with the
# per-thread ahash seed source) and "{label}" if run.sh set LABEL. "x K=1" = ratio to the same row
# at K=1; "x private" = ratio to
# `private` with the same layout and label at the same K. busy = max foreign busy % on the run CPUs in the
# window before / after each run (the run itself excluded). perf columns (if recorded) are per
# timed operation when the binary controlled the counters (perf_timed_loop_only), otherwise they
# include set-up and warm-up (upper bounds, marked "*"). GHz(u) = user-mode cycles / CPU time (means):
# well below the clock rate means that time went to the kernel (e.g. page faults) in some run.
set -euo pipefail
DIR=${1:?usage: summarize.sh <run dir>}
awk '
function val(s, key,   m) {
  if (match(s, "\"" key "\":\"?[^,\"}]*")) { m = substr(s, RSTART, RLENGTH); sub(/^"[^"]*":"?/, "", m); return m }
  return ""
}
{
  v = val($0, "variant"); w = val($0, "work"); k = val($0, "threads") + 0
  lay = "[c" val($0, "ctx_line_offset") "/m" val($0, "map_line_offset") (val($0, "ahash_source") == "thread-local" ? ",ahash-tl" : "") "]"
  lab = val($0, "label"); suf = lay (lab != "" ? " {" lab "}" : ""); row = v " " suf
  key = w SUBSEP row SUBSEP k
  n[key]++; cpu[key] += val($0, "cpu_ns_per_op"); vals[key] = vals[key] " " (val($0, "cpu_ns_per_op") + 0)
  c = val($0, "cpu_ns_per_op") + 0
  if (!(key in lo) || c < lo[key]) lo[key] = c
  if (!(key in hi) || c > hi[key]) hi[key] = c
  b = val($0, "busy_pre_pct") + 0; if (val($0, "busy_post_pct") + 0 > b) b = val($0, "busy_post_pct") + 0
  if (b > busy[key]) busy[key] = b
  timed = (val($0, "perf_timed_loop_only") == "true")
  ops = val($0, "ops_per_thread") * k
  if (!timed) ops = ops * 1.1  # warm-up OPS/10 counted too (set-up still included)
  cyc = val($0, "perf_cycles"); ins = val($0, "perf_instructions")
  if (cyc != "") {
    hasperf = 1
    pc[key] += cyc / ops; pi[key] += ins / ops
    nf = val($0, "perf_ls_dmnd_fills_from_sys_near_cache"); ff = val($0, "perf_ls_dmnd_fills_from_sys_far_cache")
    if (nf != "" || ff != "") { px[key] += (nf + ff) / ops; hasx[key] = 1 }
    pn[key]++
    if (!timed) untimed[key] = 1
  }
  chk[key] = chk[key] (chk[key] == "" ? "" : "/") val($0, "checksum")
  works[w] = 1; rows[row] = 1; ks[k] = 1
  if (!(row in rorder)) { rorder[row] = ++nr; rlist[nr] = row }
}
function median(s,   a, n, i, j, t) {
  n = split(s, a, " ")
  for (i = 1; i <= n; i++) { a[i] += 0; for (j = i - 1; j >= 1 && a[j] > a[j + 1]; j--) { t = a[j]; a[j] = a[j + 1]; a[j + 1] = t } }
  return (n % 2) ? a[(n + 1) / 2] : (a[n / 2] + a[n / 2 + 1]) / 2
}
END {
  for (key in vals) med[key] = median(vals[key])
  nk = 0; for (k in ks) kl[++nk] = k + 0
  for (i = 1; i <= nk; i++) for (j = i + 1; j <= nk; j++) if (kl[j] < kl[i]) { t = kl[i]; kl[i] = kl[j]; kl[j] = t }
  # row order: by variant (shared, private-ctx, rehome, rehome-api, private), then first appearance
  split("shared private-ctx rehome rehome-api private", vo, " ")
  no = 0
  for (vi = 1; vi <= 5; vi++) for (ri = 1; ri <= nr; ri++) { split(rlist[ri], rp, " "); if (rp[1] == vo[vi]) ordered[++no] = rlist[ri] }
  split("full specialize split", wo, " ")
  for (wi = 1; wi <= 3; wi++) {
    w = wo[wi]; if (!(w in works)) continue
    printf "\n### work = %s\n\n", w
    printf "| variant [layout] | K | runs | CPU/op ns (median) | min-max | x K=1 | x private | busy %% |%s\n", (hasperf ? " cycles/op | instr/op | IPC | x-CCX fills/op | GHz(u) |" : "")
    printf "|---|---:|---:|---:|---|---:|---:|---:|%s\n", (hasperf ? "---:|---:|---:|---:|---:|" : "")
    for (oi = 1; oi <= no; oi++) {
      row = ordered[oi]
      suf = substr(row, index(row, " ") + 1)
      for (i = 1; i <= nk; i++) {
        k = kl[i]; key = w SUBSEP row SUBSEP k; if (!(key in n)) continue
        m = med[key]
        b1 = w SUBSEP row SUBSEP 1; r1 = (b1 in n) ? sprintf("%.2f", m / med[b1]) : "-"
        bp = w SUBSEP "private " suf SUBSEP k; rp2 = (bp in n) ? sprintf("%.2f", m / med[bp]) : "-"
        extra = ""
        if (hasperf && pn[key] > 0) extra = sprintf(" %.3g%s | %.3g | %.2f | %s | %.2f |", pc[key] / pn[key], (key in untimed ? "*" : ""), pi[key] / pn[key], pi[key] / pc[key], (key in hasx) ? sprintf("%.1f", px[key] / pn[key]) : "-", (pc[key] / pn[key]) / (cpu[key] / n[key]))
        else if (hasperf) extra = " - | - | - | - | - |"
        printf "| %s | %d | %d | %.0f | %.0f-%.0f | %s | %s | %.0f |%s\n", row, k, n[key], m, lo[key], hi[key], r1, rp2, busy[key], extra
      }
    }
  }
  # identity: every variant and layout must produce the same checksum at the same K and work
  bad = 0
  for (key in chk) { split(key, p, SUBSEP); split(chk[key], cs, "/"); for (x in cs) { id = p[1] SUBSEP p[3]; if (!(id in ref)) ref[id] = cs[x]; else if (ref[id] != cs[x]) bad++ } }
  printf "\nchecksum identity across variants, layouts and repeats: %s\n", (bad ? bad " MISMATCH(ES)" : "all equal per (work, K)")
}' "$DIR/runs.jsonl"
