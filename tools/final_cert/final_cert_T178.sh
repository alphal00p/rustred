#!/usr/bin/env bash
# final_cert_T178.sh -- the SINGLE FINAL CERTIFICATION of the T178 unboxed table (lane parametric, 2026-11-17).
# usage: final_cert_T178.sh --candidate CORE --sha256 HEX --candidate BEYOND --sha256 HEX [--dry] [--label REHEARSAL] [--status]
#   T178 = the pruned two-file ship of lane t178-box3 (core then beyond; engine dispatch = core THEN beyond).
#   Refuses on any sha256 mismatch; --dry checks every input / tool / sha and prints the plan without starting anything.
#   Resume: run the same command again (steps with a .done marker are skipped).
# Node data live HERE (configuration); the engine fc_lib.sh / fc_tool.py are generic.
set -u
FC_SELF=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
P=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/parametric
HV=/common/build-cache/vhirschi/p_integral_dev/helper-share/verdicts-VH
ENG=${FC_ENGINE_DIR:-$FC_SELF}          # rustred / rr_node of batch 12a (frozen copy next to this kit)
cfg_node=T178
cfg_scratch=$P/final
cfg_engine_model=4895ee68               # dispatch semantics of rustred slot-alias 4895ee68 (in batch 11 d5b63a65 and 12a fa921f4e)
cfg_harness=${FC_HARNESS:-$FC_SELF/parametric_real}
cfg_rustred=$ENG/rustred
cfg_rr_node=$ENG/rr_node
cfg_region=simplex:32                   # conservative arrival cone of T178 (liveness region, probe cone)
cfg_cones="simplex:301;simplex:32;simplex:30;simplex:24;drbox:4:3:5;orthant"
cfg_exact_cones="simplex:301;simplex:32;simplex:30;simplex:24;drbox:4:3:5"
cfg_orthant_undecided=2                 # the two S{4,6} parts of F35 (5164), decided by argument (t178-s46-step, reviewed SOUND)
cfg_cert_rss_gb=28
cfg_terminals=$P/deliver/T178-unboxed-v2/certificate/T178.terminals.json
cfg_recipes=""
cfg_recipe_before_dp=-1
cfg_graph="0-1 0-2 0-3 0-4 1-2 1-3 1-4 2-3 2-5 3-5"; cfg_ext=4,5; cfg_lp=7
cfg_family=$WS/rules_study/classify/families/T178.sym
cfg_zeros=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/zeros2/deliver2/T178/T178.zero-proofs.jsonl
cfg_zero_summaries=$P/zv/T178.verify.summary.json
cfg_import_sets="0:plain 0:shared concat:plain concat:shared"   # core alone (vakint's default load) and core THEN beyond as one table
cfg_exact_config=$FC_SELF/T178.exact.json
cfg_side_dirs=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/sizegen/final-cert-inputs
cfg_arrivals=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/deliver/T178.keys.bin
cfg_fm_n=20000
cfg_accept_keys=""
cfg_drill_points=""
cfg_one_cap1=40; cfg_one_parallel=3
cfg_live_shards=8; cfg_live_units=50000000; cfg_live_wall=300; cfg_live_max=0
cfg_deliver_dir=$P/deliver/T178-final
cfg_delivery_notes=$FC_SELF/T178.delivery-notes.md
# resources per step: "MemoryMax_GB expected_peak_GB cpus writes_GB expected_wall"
cfg_res_prep="12 4 1 1 5m"; cfg_res_cert="32 12 1 0 20m"; cfg_res_term="12 5 1 0 4m"; cfg_res_b1="12 5 2 0 4m"; cfg_res_exact="8 3 1 0 4m"
cfg_res_imp_plain="16 6 4 3 8m"; cfg_res_imp_shared="16 7 4 1 8m"; cfg_res_live="24 8 8 0 20m"; cfg_res_fm="16 7 2 0 5m"; cfg_res_one="24 12 3 0 10m"
cfg_res_deliver="8 2 1 0 5m"
cfg_inputs=(
  "$cfg_terminals=d99a6134f698a2e311a135f31d4348ee0f40acb0aaf9f3a6c973eddd49787706"
  "$cfg_family=-" "$cfg_zeros=-" "$cfg_zero_summaries=-"
  "$HV/t178-exact-A.records.tsv=-" "$HV/t178-exact-B.records.tsv=-"
  "$cfg_arrivals=f8534177f54848a87e91079ba843704e9a27af5ff7aece58e47a6b96dd779933"
  "$cfg_harness=-" "$cfg_rustred=-" "$cfg_rr_node=-" "$cfg_exact_config=-"
)
[ -n "${FC_OVERRIDES:-}" ] && source "$FC_OVERRIDES"   # rehearsal: a file of cfg_* overrides (cut cones, small caps, label)
source "$FC_SELF/fc_lib.sh"
fc_main "$@"
