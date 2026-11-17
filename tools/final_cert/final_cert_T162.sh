#!/usr/bin/env bash
# final_cert_T162.sh -- the SINGLE FINAL CERTIFICATION of the T162 unboxed table (lane parametric, 2026-11-17).
# usage: final_cert_T162.sh --candidate TABLE --sha256 HEX [--dry] [--label L] [--status]
#   T162 = the size lane's candidate (pruned table + r1's record 1289 + record 1292's sub-table) WITH its two bounded recipes,
#   placed before the first rule whose origin.dispatch_position >= 3655 (their position in the certified f0bd7e76 dispatch).
#   Refuses on any sha256 mismatch; --dry checks every input / tool / sha and prints the plan without starting anything.
#   Resume: run the same command again (steps with a .done marker are skipped).
set -u
FC_SELF=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
P=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/parametric
HV=/common/build-cache/vhirschi/p_integral_dev/helper-share/verdicts-VH
ENG=${FC_ENGINE_DIR:-$FC_SELF}
cfg_node=T162
cfg_scratch=$P/final
cfg_engine_model=4895ee68
cfg_harness=${FC_HARNESS:-$FC_SELF/parametric_real}
cfg_rustred=$ENG/rustred
cfg_rr_node=$ENG/rr_node
cfg_region=simplex:39                   # the certified cone R = 39 (recipes bounded at 39)
cfg_cones="simplex:39;simplex:36;simplex:33;simplex:28;drbox:4:2:5;orthant"
cfg_exact_cones="simplex:39;simplex:36;simplex:33;simplex:28;drbox:4:2:5"
cfg_orthant_undecided=""                # whole sector REPORTED, not claimed (the pruned file's statement is D + r <= 39)
cfg_cert_rss_gb=38
cfg_terminals=$P/h162/cert/T162.terminals.json
cfg_recipes=$P/deliver/T162-unboxed-pruned/table/T162.pruned.recipes.jsonl
cfg_recipe_before_dp=3655
cfg_graph="0-1 0-2 0-3 0-4 1-2 1-3 1-4 2-3 2-4"; cfg_ext=3,4; cfg_lp=7
cfg_family=$WS/rules_study/classify/families/T162.sym
cfg_zeros=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/zeros2/deliver2/T162/T162.zero-proofs.jsonl:/common/build-cache/vhirschi/p_integral_dev/t162-handdown/zeros/own.zero-proofs.jsonl
cfg_zero_summaries=$P/zv/T162.verify.summary.json:$P/zv/own.verify.summary.json
cfg_import_sets="0:plain 0:shared"
cfg_exact_config=$FC_SELF/T162.exact.json
cfg_side_dirs=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/sizegen/final-cert-inputs
cfg_arrivals=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/deliver/T162.keys.bin
cfg_fm_n=20000
cfg_accept_keys=$FC_SELF/T162.accept.jsonl
cfg_drill_points=$P/deliver/T162-unboxed/recipes/recipe-relations.acceptance-chains.points.jsonl
cfg_one_cap1=40; cfg_one_parallel=3
cfg_live_shards=16; cfg_live_units=50000000; cfg_live_wall=120; cfg_live_max=0
cfg_deliver_dir=$P/deliver/T162-final
cfg_res_prep="16 3 1 1 3m"; cfg_res_cert="48 8 1 0 20m"; cfg_res_term="24 8 1 0 8m"; cfg_res_b1="24 8 2 0 8m"; cfg_res_exact="16 4 1 0 15m"
cfg_res_imp_plain="48 28 4 9 25m"; cfg_res_imp_shared="48 28 4 1 25m"; cfg_res_live="48 16 16 0 40m"; cfg_res_fm="24 8 2 0 10m"; cfg_res_one="40 21 3 6 20m"
cfg_res_deliver="8 2 1 0 10m"
cfg_inputs=(
  "$cfg_terminals=60d994e1c27747825f3a720cbf3003db112e18809ec407c5f358244455babb31"
  "$cfg_recipes=-" "$cfg_family=-" "${cfg_zeros%%:*}=-" "${cfg_zeros#*:}=-" "${cfg_zero_summaries%%:*}=-" "${cfg_zero_summaries#*:}=-"
  "$HV/t162-table-A-exact.records.tsv=-" "$HV/t162-table-B-exact.records.tsv=-"
  "$cfg_arrivals=73826829f2f20eac9d6a90d6cc2e89abc48956a7ee7d70a8b511643638583f10"
  "$cfg_drill_points=-" "$cfg_accept_keys=-"
  "$cfg_harness=-" "$cfg_rustred=-" "$cfg_rr_node=-" "$cfg_exact_config=-"
)
[ -n "${FC_OVERRIDES:-}" ] && source "$FC_OVERRIDES"
source "$FC_SELF/fc_lib.sh"
fc_main "$@"
