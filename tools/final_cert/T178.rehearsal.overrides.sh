# REHEARSAL of the launcher on T178 (NOT a certificate of the real table): cut cones D + r <= 12 and the box, small samples,
# caps <= 8 GB, plain imports only on the pre-12a engine (shared + stream needs 12a), outputs labelled REHEARSAL
cfg_region=simplex:12
cfg_cones="simplex:12;drbox:4:3:5"; cfg_exact_cones="simplex:12;drbox:4:3:5"; cfg_orthant_undecided=""
cfg_fm_n=2000; cfg_live_shards=2; cfg_live_wall=30; cfg_live_max=10
cfg_import_sets="0:plain concat:plain"
cfg_accept_keys=/shared/vhirschi/t3code/vakint_dev/p_integral_dev/wt/ac-parametric/tools/final_cert/T178.rehearsal.accept.jsonl; cfg_one_cap1=6; cfg_one_parallel=2
cfg_deliver_dir=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/parametric/final/rehearsal-deliver/T178-REHEARSAL
cfg_res_prep="8 3 1 1 3m"; cfg_res_cert="8 4 1 0 5m"; cfg_res_term="8 4 1 0 3m"; cfg_res_b1="8 4 2 0 3m"; cfg_res_exact="8 3 1 0 3m"
cfg_res_imp_plain="8 5 4 3 8m"; cfg_res_imp_shared="8 1 1 0 1m"; cfg_res_live="8 4 2 0 5m"; cfg_res_fm="8 6 2 0 3m"; cfg_res_one="8 6 2 0 5m"; cfg_res_deliver="8 2 1 0 3m"
