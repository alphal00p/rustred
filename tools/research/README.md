# Research lenses (fable_5_1 next push, W0.1 / W0.4)

Read-only analysis tools behind the measured figures of
`docs/research/fable51_next_push_master_plan_2026-09-27.md` (§1, labels
`[M-r]` with `$S/...` paths), persisted from the review session's ephemeral
scratchpad (`$S` = `/tmp/claude-1125/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/scratchpad`),
plus the W0.4 index replay `idxreplay`. None of these tools writes into a
checkpoint or a campaign; all of them only read.

A full byte copy of the scratchpad as it was at persistence time (tools,
intermediate data, logs) is kept outside git at
`/common/dev/rustred/TMP/w0/intel/lens-scratch-snapshot/`. The original
command lines below were recovered from the lens agents' transcripts; the
paths are rewritten to the conventions of this push:

- `CK` = a block clone of a v2 checkpoint generation, e.g.
  `cp -r /common/dev/rustred/TMP/v2-checkpoint-copy-gen7 $MY_TMP/gen7` (never
  read or write `campaigns/*/checkpoints` for new runs; the original lens runs
  read `campaigns/five-loop-qcd-feynman-d9d10-v2/checkpoints/main` read-only
  while the campaign was stopped, and the gen-3/6/7 fixtures are byte copies
  of it).
- `PILOT` = `/common/dev/rustred/TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/hot-owner-physics-ordered/run/result.json`
  (the drained C-HOT pilot).
- `V2RUN` = `/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-v2/runs/20260926T151353.794886Z` (read only).
- Pin every run: `nice -n 5 taskset -c <your CPUs>`; the original lens runs used
  `taskset -c 200-255`.

## Building

Each Rust directory is its own Cargo project with its own `target/`
(`[workspace]` table, no change to the rustred workspace). Build under the
shared build lock:

```sh
cd tools/research/<project>
flock -w 14400 /common/dev/rustred/TMP/locks/build-0.lock \
  nix develop /common/dev/rustred --command cargo build --release --offline
```

`lens-rs` and `idxreplay` are std-only. `rtool` needs `serde`/`serde_json`
from the offline cargo cache (its `Cargo.lock` is committed). Python scripts
run with `nix develop /common/dev/rustred --command python <script> ...`
(`python3` is not on PATH).

`idxreplay` is built for `target-cpu=znver4` (`.cargo/config.toml`) and
asserts AVX-512BW/VL at start-up.

## idxreplay (W0.4, new)

Offline replay of admission lookups against a CP5 checkpoint's live
candidate index, and of real successor streams recorded by the research
`admission-trace` feature (`crates/rustred-app` feature `admission-trace`,
env `RUSTRED_ADMISSION_TRACE_DIR`). Source map:

> Integration note (2026-09-28): the `admission-trace` engine feature (commit
> 055986ba) was NOT merged into `fable_5_1`; it stays on branch
> `fable_5_1-v3-intel`. Build a trace binary from that branch to record new
> streams; the replay tool itself needs no RustRed dependency.

| file | content |
|---|---|
| `src/ckpt.rs` | CP5 section decoding; exact port of `power_domain/geometry.rs::project`, `DomainPowerSummary::contains`, `bits::word`, `Signature` |
| `src/l0.rs` | today's layout replicated type for type (`index.rs`, `index/blocks.rs`, `bits.rs`, `compact.rs` `CompactSummary`/`SummarySlab`), `find_controlled` min-ID and first-found |
| `src/soa.rs` | 32-entry struct-of-arrays blocks: u32 ids, u64 words, 36 u8 lanes (monotone image of every tight extremum), block OR/AND words and u8 envelopes; AVX-512 forward/reverse kernels plus scalar reference kernels |
| `src/stat.rs` | `static` mode: sampled requests, layouts x policies x thinning x threads |
| `src/dynamic.rs` | `dynamic` mode: rebuild today's index from scratch in the coordinator commit order of a traced run and check every admission's outcome and counters |
| `src/pipeline.rs` | §3.2 resolution pipeline on real streams (exact-job, self, Local, MRU k, exact store, helpers/orthants, layers, miss) |
| `src/streams.rs` | `streams` mode: traced run resumed from the checkpoint; pipeline, then layer-reaching requests on the checkpoint index |
| `src/lag.rs` | `lag` mode: container age of every committed hit edge (stale-miss rate vs lag) |

Commands (results: `/common/dev/rustred/TMP/w0/intel/RESULTS.md`):

```sh
B=tools/research/idxreplay/target/release/idxreplay
# layouts/policies/units/thinning at gen 7 (single thread on the lane CPUs)
nice -n 5 taskset -c 10-27 $B static --ckpt $CK --gen 00000000000000000007 \
   --samples 20000 --fracs 1024,512,256 --threads 1 --out static.jsonl
# 1/48/90 threads on socket 1 (hold the socket lock, interleave nodes 4-7)
flock -w 14400 /common/dev/rustred/TMP/locks/socket1.lock \
  numactl --interleave=4-7 taskset -c 128-217 $B static --ckpt $CK \
   --gen 00000000000000000007 --fracs 1024 --threads 1,48,90 --seconds 8 --out threads.jsonl
# complete four-loop validation (traces from the admission-trace binary)
$B dynamic --trace TMP/w0/intel/trace-4l/fg --label 4l-fg --out dynamic.jsonl
# real gen-7 streams (trace of a resumed gen-7 clone)
$B streams --ckpt $CK --gen 00000000000000000007 --trace TMP/w0/intel/g7-trace-resume/trace --out streams.jsonl
# stale-miss rate vs lag from all committed edges
$B lag --ckpt $CK --out lag.jsonl
# markdown tables for RESULTS.md (static|thin|threads|dynamic|pipeline|streams|lag|ratios)
python tools/research/idxreplay/tables.py pipeline_k 1,16,64 streams.jsonl
python tools/research/idxreplay/tables.py ratios static.jsonl threads.jsonl streams.jsonl
# gate 0.4(b) projection [E] of the admission share of worker CPU at N = 1G
python tools/research/idxreplay/project.py streams.jsonl --alpha 0.44,0.65,0.69,0.73,0.82 \
   [--thread-sweep threads.jsonl --threads 90] [--native-scale 3.75] [--k 64] [--cheap-ns 300]
# per-class exponents (a= then applies to the parts not overridden, here the hits)
python tools/research/idxreplay/project.py streams.jsonl --alpha 0.0,0.46,0.69 \
   --class-alpha miss_scans=0.46,reverse=0.50
```

Since v4 (close-out, 2026-09-27) the `streams`/`dynamic` pipeline rows carry
`native_seconds`, `native_ms_per_job` (sum of the jobs' physical-call wall
seconds from the trace job headers) and `requests_per_job`; a
`stale-lag-requests` row gives the container age of layer-hit requests at
MRU k=16; `static` rows carry `foreign_busy_own_cpus` and `busy_smt_siblings`
(busy share of the process's allowed CPUs not due to the process, and of their
SMT siblings, from `/proc/stat`), and `--skip-stats 1` skips the one-thread
stats pass. `project.py` needs a v4 `streams` output (or `--native-ms`).

## rtool (serde): point-set lenses on walk records

`src/main.rs` modes over a drained `result.json` (pretty `domains` array) or
CP5 `records-*.jsonl`; `owner_class.txt` (in `outputs/rtool/`) classifies
owners. `src/bin/rtool_owner.rs` is the per-owner variant of the `scale`
mode (the only difference: SCALE rows keyed by `owner|class`).

| mode / bin | reproduces | command |
|---|---|---|
| `rtool cover` | plan §1 "Waste": Apply points overlap 19.5x, 3.4% new points; 71.1% of Apply inspections / 69.3% of Apply native seconds fully covered by earlier IDs (`outputs/rtool/pilot_cover2.txt`, lines `COVER2 ...`) | `rtool cover $PILOT outputs/rtool/owner_class.txt > pilot_cover2.txt 2> pilot_cover2.err` |
| `rtool union` | per-owner union of admitted lattice points (M-pts; `pilot_union.txt`, `pilot_levels.txt`) | `rtool union $PILOT owner_class.txt > pilot_union.txt` |
| `rtool sum` | per-class record census of the pilot (`pilot_sum.txt`) | `rtool sum $PILOT owner_class.txt > pilot_sum.txt` |
| `rtool scale` | pooled points^0.2-0.6 law (a mix effect) | `rtool scale $PILOT owner_class.txt`; `rtool scale $CK/records-00000000000000000006.jsonl owner_class.txt 4000000` |
| `rtool_owner scale` | per-owner cost law, cost ∝ points^0.74-0.90 (`outputs/perfskeptic/scale_owner.txt` on the pilot, `scale_v2g6.txt` on gen-6 records); slopes via `py/perfskeptic/slopes.py` | `rtool_owner scale $PILOT owner_class.txt > scale_owner.txt`; `rtool_owner scale $CK6/records-00000000000000000006.jsonl owner_class.txt > scale_v2g6.txt`; `python py/perfskeptic/slopes.py scale_owner.txt scale_v2g6.txt` |
| `deleg` | creator of each delegated domain vs its representative's creator | `grep -hF '"record_kind":"delegated_not_inspected"' $CK/records-*.jsonl > deleg_lines.jsonl; deleg 74156033 deleg_lines.jsonl $CK/edges-*.bin` |
| `keys` | staircase-cell count (phase, owner, rank, A max, D min/max) of admitted domains | `keys $CK/records-0000000000000000000{3,4,5,6,7}.jsonl`; `keys $PILOT` |

## lens-rs (std only): checkpoint scans

| bin | reproduces | command (from the checkpoint directory) |
|---|---|---|
| `indexscan` | index census: live candidates, groups, blocks, fill, inspected-then-covered | `indexscan $CK/index-00000000000000000007.bin $CK/nodes-00000000000000000007.bin` |
| `doms` | plan §1 "Waste": 10,129,834 of 27,465,422 inspected domains (36.9%) later contained by a newer admission (mistake census) | `doms $CK 00000000000000000007` |
| `scan` | first replay (raw syntactic boxes; superseded by `scan2`) | `cd $CK && scan . 00000000000000000007 20000 > scan7.out` |
| `scan2` | plan §1 index: 0 of 74.16M tight summaries escape u8 lanes; miss tests 11.2k candidates (p99 ~96k); first-found cuts candidates per hit 7,374 -> 3,633; Σ-lower clustering worse (`outputs/perfskeptic_cwe/scan2_7.out`) | `cd $CK && /usr/bin/env time -v scan2 . 00000000000000000007 20000 > scan2_7.out 2> scan2_7.err` |
| `scan3` | layouts id-order / finite-pattern+lex-lower / Morton at 100% and 25% live: pattern+lex best (9.9k per miss, 3.2k per hit), N^0.52-0.66 thinning exponents, top bucket 4.8% of new IDs (`scan3_7.out`) | `cd $CK && /usr/bin/env time -v scan3 . 00000000000000000007 10000 > scan3_7.out 2> scan3_7.err` |
| `deg` | out-degree distribution of edge sources: 81% of gen-7 edges from jobs with >= 32 distinct targets | `deg $CK/edges-00000000000000000007.bin` |
| `edges_indeg` | in-degree / top-target census of one edge segment | `edges_indeg $CK/edges-00000000000000000003.bin` |
| `membench` | host memory: dependent random load 143 ns local / 240 ns cross-socket, streaming 2.2-2.7 ns per candidate, THP effect (`outputs/membench/*.json`) | `numactl --physcpubind=240 --membind=7 membench small`; `... --membind=1 membench huge > huge_node1.json` |
| `census` | per-owner census of a generation (`inputs_lens/v2g7/*.tsv`), with `bfs` the per-root reach (`v2g7bfs/`) | `census $CK 7 py/inputs_lens/owners.tsv v2g7`; `CENSUS_THREADS=12 census $CK 7 owners.tsv v2g7bfs bfs` |
| `hybrid` | §4 I1 counterfactual: absorbed helpers as sinks, −21..−32% nodes; scope trimming ≤1.2% | `L=$(cat outputs/inputs_lens/L.txt); hybrid $CK 7 outputs/inputs_lens/owners.tsv "R_hybridL40_rankbounded:$L"` (other scenario strings in the snapshot transcripts) |
| `routecount`, `vol` | route-mask and box-volume census of domain segments | `vol $CK 7 > vol.tsv`; `routecount $CK 7` |
| `witness6` (and iterations `witness`..`witness5`) | §4 I2 route witnesses: traffic-weighted cancellation support 7.56 -> 5.56 (`outputs/inputs_lens/witness6.tsv`, `.log`) | `witness6 > witness6.tsv 2> witness6.log` (reads `routes.txt`, `auts.tsv`, `orbits.tsv` from the working directory; `routes.txt` is in the snapshot) |

## py/: Python lenses (stdlib only unless noted)

| script | reproduces | command |
|---|---|---|
| `meas/extract.py` | heartbeat extraction from events.jsonl to `*_hb.jsonl` | `python py/meas/extract.py $V2RUN/events.jsonl v2_hb.jsonl` |
| `meas/win.py` | windowed series (`outputs/meas/v2_win600.json`, `v2_win1800.json`, `int_win*.json`, `prdy_win900.json`) | `python py/meas/win.py v2_hb.jsonl 1800 h,wall_us_per_req,coord_us_per_req,commit_us_per_rec,prep_us_per_batch,...` |
| `meas/fits.py` | plan §1 growth law: checks ∝ live^1.05 (R² 0.61), maintenance ∝ live^1.26, cumulative wall ∝ N^1.59 | `python py/meas/fits.py outputs/meas/v2_win600.json` |
| `meas/recscan.py`, `meas/recmerge.py` | record-level merge of heartbeats and records (`recscan.pkl`, `recmerged.pkl` in the snapshot) | see headers |
| `join7.py` | ~85% of Apply successor events hit a container the same inspection already hit (0.114 distinct targets per successor) | `python py/join7.py $CK/records-00000000000000000007.jsonl $CK/edges-00000000000000000007.bin 68873730` |
| `edges_age.py` | 86.3% of first-hit containers ≥ 4.2M admissions old, 2.53% younger than 4,096 | `python py/edges_age.py $CK/edges-0000000000000000000{3,4,5,6,7}.bin` |
| `fanin.py` | targets per successor for Apply sources (fan-in) | `python py/fanin.py $CK/records-00000000000000000007.jsonl $CK/edges-00000000000000000006.bin $CK/edges-00000000000000000007.bin` |
| `records_scan.py` | §1 native work: Apply 5.95M inspections / 241,437 record-seconds, Route 21.52M / 1,951 s; hot owner 63.6-64.1% of record-seconds; top 1% of natives carry 77.7% of gen-7 seconds | `NPROC=40 python py/records_scan.py v2_records.json $CK/records-0000000000000000000{3,4,5,6,7}.jsonl` |
| `index_census.py` | §1 index at gen 7: 8,040 buckets, 37.9M live candidates, 459,075 groups, 1,993,524 blocks, fill 59.4%, IDs ~696k apart in a block | `python py/index_census.py $CK/index-00000000000000000007.bin` |
| `records_census.py`, `rec_census.py`, `records_cost.py`, `edges_census.py`, `edges_scan.py`, `seg_envelope.py`, `sample_rows.py`, `sample_wrap.py`, `recs.py`, `insp.py`, `split.py`, `analyze1-3.py`, `regress.py` | archive surveys (cost vs volume, self loops, per-segment envelopes, samples) | `python py/<script> $CK/records-0000000000000000000N.jsonl [MAX_BYTES]` (see each header) |
| `deleg.py`, `deleg7.py`, `deleg_all.py` | delegation vs creator joins | `python py/deleg7.py $CK/records-00000000000000000007.jsonl $CK/edges-00000000000000000007.bin 68873730` |
| `lag.py` | per-edge lag of one segment | `python py/lag.py $CK/edges-00000000000000000007.bin` |
| `pilot_scan.py` | C-HOT pilot record scan (`pilot_ordered.json`) | `NPROC=40 python py/pilot_scan.py pilot_ordered.json $PILOT` |
| `cells.py`, `hull.py`, `strata_hull.py`, `redund.py`, `overlap.py`, `keys.py`, `sample_strata.py`, `sample_support.py` | staircase cells, stratum hulls and overlap of the hot owner (G1 / cells-first evidence) | e.g. `grep '"phase":"Apply"' $CK3/records-00000000000000000003.jsonl \| python py/redund.py` |
| `hb.py`, `win.py`, `win2.py`, `series.py`, `util.py`, `duty.py`, `hot_*.py`, `frontier_time.py`, `v2_early.py`, `perf_skeptic_win.py`, `perf_skeptic_L.py`, `last.py`, `show.py`, `fit.py`, `slope.py`, `rss_slope.py`, `live_rss.py`, `resid.py`, `edges.py` (numpy) | heartbeat windows, coordinator duty (`util.py $V2RUN/result.json`: ordered_commit 51.4%, preparation 38.0%, ...), interim frontier timing (first frontier 538 s), RSS slopes | `python py/util.py $V2RUN/result.json`; `python py/hb.py $V2RUN/events.jsonl hb_v2.json; python py/win.py hb_v2.json 3600` |
| `inputs_lens/an1.py`..`an5.py`, `ts.py`, `routes.py`, `dump_routes.py`, `auts.py` | §1/§4 inputs lens: 82% of Apply domains of the 47 v2-only-bounded owners inside the interim helper box (`an2.py`), observed L40 identical at gen 3 and 7 (`an3.py`), automorphism orbits | `cd <census output dir> && python py/inputs_lens/an2.py` (reads `v2g7/census.tsv`, `owners.tsv`, `interim_ts.json`) |
| `perfskeptic/slopes.py`, `perfskeptic/cpubusy.sh` | per-owner slope fits; foreign busy CPUs on socket 1 | `python py/perfskeptic/slopes.py scale_owner.txt scale_v2g6.txt`; `bash py/perfskeptic/cpubusy.sh` |

Scripts with a hard-coded path (for example `cells.py`, `sample_*.py`,
`edges.py` read `TMP/v2-checkpoint-copy-gen3/...`; `rec_census.py`,
`resid.py` read the v2 `queries.json`) are kept verbatim so that they
reproduce their original output; edit the constant to point at a clone.

## outputs/

Small receipts the plan cites, copied verbatim: `rtool/` (pilot cover,
union, levels, sum; `owner_class.txt`), `perfskeptic/` (scale laws, CPU-busy
samples), `perfskeptic_cwe/` (scan/scan2/scan3 outputs with `time -v`),
`membench/`, `meas/` (window series), `inputs_lens/` (owners, L40,
automorphisms, orbits, witness6, time series, census summaries), gen-7
record census heads and per-segment envelopes. Larger intermediates (census
`transitions.tsv`, heartbeat jsonl, pickles, `routes.txt`) are in the
snapshot directory named above.
