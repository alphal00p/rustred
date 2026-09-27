# Combined four-loop owner run (common Luthe A4 basis)

One combined owner-domain walk over **all four-loop owners** in a single common
momentum basis, built to resemble the five-loop campaign as closely as possible:
one family, class owners generated in that family, every labelled sector routed
to its class owner, and physics-scoped entry queries from the generic planner.
It is proposed as the combined four-loop engine control, next to the four per-family controls
FG/BMW/H/X (four bases, 900 installed owners, zero Route records), which stay as the zero-Route
arm (section 5, "Recommended C-4L gate").

Evidence labels: **[M]** measured (a run directory under
`TMP/c4l-build.wBU9zC/` or `TMP/fable51-controls/c4l-*` is cited), **[E]**
estimate or interpretation. Nothing here is a family-closure, termination or
minimal-master claim; `family_closure_claim` is false in every artifact.

## 1. What is combined

- **Basis.** Luthe's A4 basis (thesis Table A.1 p.111, eq. (2.26) p.24):
  `k1, k2, k3, k4, k1-k4, k2-k4, k3-k4, k1-k2, k1-k3, k1-k2-k3`, i.e. the ten
  four-loop scalar products. RustRed convention `D_i = q_i^2 - 1`.
- **Roots.** The two cubic 9-line graphs: `1022 = 1111111110` (prism, the
  Vakint H parent; `--nonpositive-indices 9`) and `511 = 0111111111` (K3,3,
  the Vakint X parent; `--nonpositive-indices 0`). The 8-line Vakint parents
  are contractions of these roots, not roots: FG is `1020 = 1111111100`
  inside 1022, BMW (the 4-spoke wheel) is `510 = 0111111110 = 1022 AND 511`.
- **Owners.** One owner per Table B.1 class (16 = 10 non-factorized + 6
  factorized). Owner rule (deterministic): a class member generated under the
  K3,3 root first, then the representative's own labelled sector, then the
  largest mask. 12 owners come from root 511 and 4 from root 1022.
- **Routes.** All 508 labelled nonzero sectors of the two root downsets
  (314 + 328 - 134 shared) are routed to their class owner; 492 need numerator
  transport; the other 16 are the owners themselves. Five loops: 8,179 of 8,246 routes need
  transport.

| Owner | Rep. | Root | t | Class | V4min | Helper R | Roots | Class members | Program bytes |
|---|---:|---:|---:|---|---:|---:|---|---:|---:|
| `0111100000` | 960 | 511 | 4 | factorized | 3 | 9 | nested-d7p-a16-r9 | 111 | 135,416 |
| `0111110000` | 992 | 511 | 5 | factorized | 2 | 10 | nested-d7p-a17-r10 | 115 | 92,254 |
| `0111100100` | 961 | 511 | 5 | factorized | 3 | 9 | nested-d7p-a16-r9 | 50 | 201,222 |
| `0111100001` | 841 | 511 | 5 | non-entry (banana) | - | 5 | conv-d8-a13-r5, conv-d7-a11-r4 | 11 | 3,322,106 |
| `0111111000` | 1008 | 511 | 6 | factorized | 1 | 11 | nested-d7p-a18-r11 | 72 | 108,064 |
| `0111110010` | 993 | 511 | 6 | connected | 2 | 3 | phys-d8-a11-r3, phys-d7-a9-r2 | 48 | 454,719 |
| `0111010101` | 978 | 511 | 6 | factorized | 1 | 11 | nested-d7p-a18-r11 | 10 | 74,748 |
| `0111100110` | 952 | 511 | 6 | connected | 3 | 2 | phys-d8-a10-r2, phys-d7-a8-r1 | 8 | 180,500 |
| `1111111000` | 1016 | 1022 | 7 | connected | 0 | 5 | phys-d8-a13-r5, phys-d7-a11-r4 | 3 | 106,810 |
| `1111110100` | 1012 | 1022 | 7 | factorized | 1 | 11 | nested-d7p-a18-r11 | 6 | 83,420 |
| `0111111100` | 1010 | 511 | 7 | connected | 0 | 5 | phys-d8-a13-r5, phys-d7-a11-r4 | 32 | 104,601 |
| `0111111001` | 1009 | 511 | 7 | connected | 2 | 3 | phys-d8-a11-r3, phys-d7-a9-r2 | 23 | 1,226,920 |
| `1111111100` | 1020 | 1022 | 8 | connected | 0 | 5 | phys-d8-a13-r5, phys-d7-a11-r4 | 6 | 75,216 |
| `0111111110` | 1011 | 511 | 8 | connected | 1 | 4 | phys-d8-a12-r4, phys-d7-a10-r3 | 11 | 196,982 |
| `1111111110` | 1022 | 1022 | 9 | connected | 0 | 5 | phys-d8-a13-r5, phys-d7-a11-r4 | 1 | 81,355 |
| `0111111111` | 511 | 511 | 9 | connected | 0 | 5 | phys-d8-a13-r5, phys-d7-a11-r4 | 1 | 118,040 |

## 2. Files

| File | Content |
|---|---|
| `four_loop_common_basis.toml` | Family input (name `four_loop_common_basis`; the name enters the family fingerprint). |
| `four_loop_common_manifest.json` | Census manifest: momenta, the two roots, the 16 Table B.1 representatives with factorized flags. |
| `four_loop_common_parent_vertices.json` | Vertex witnesses of the two roots, used by the planner (validated against the momenta). |
| `four_loop_common_routing_witnesses.json` | Forward maps of the 16 representatives into the roots, summary of the labelled routes, and (documentation only) the maps of the four Vakint bases into A4. |
| `four_loop_common_labelled_routes.jsonl` | The 508 labelled-sector routing witnesses (census producer output, byte-identical witness lines on two runs). |
| `selection.json` | Owner selection: 16 owners (masks, relative paths, bytes, SHA-256, representative, root, native ordinal) and the 508 routes. Owner payloads are not committed (section 4). |
| `regenerate_owners.py` | Regenerates the 16 owner payloads with the pinned binary and checks them against `selection.json` (section 4). |
| `tools/records_breakdown.py` | Read-only per-owner / per-rank / time-bin breakdown of a walk's committed-records sidecar (section 5). |
| `four-all/` | **The combined control** (`run_control.py --family four-all`; how to use it: section 5, "Recommended C-4L gate"): `queries.json` = the 42 physics rows followed by 16 rank-12 owner orthants (58 rows), its `matching-summary.json` and staging `input-receipt.json`. |
| `physics/` | Planner physics class: `queries.json` (42 rows), `entry-plan-receipt.json`, `skeleton-classification.json`, `entry-plans/`, `matching-summary.json`, staging `input-receipt.json`. |
| `envelope-a19r12d7/` | Historical saved-cover envelope: `queries-boxes.json` (16 boxes), `queries.json` (boxes + 16 rank-12 owner orthants), `entry-spec.json` / `entry-plan.json` (Rust counts), `matching-summary.json`, `input-receipt.json`. |

## 3. How the inputs were produced

All commands ran on CPUs 320-383 with the release binary
`TMP/fable51-controls/bin/rustred-4a17f9c7`
(SHA-256 `4a17f9c7c0447713370a1aed2e54c9a04251106a9183fd1b8a86dc5d1abb395e`),
copied to `TMP/c4l-build.wBU9zC/bin/`. Evidence directory:
`TMP/c4l-build.wBU9zC/`.

1. **Owner programs** (`gen/run_gen.sh`). One `family-candidates` run per
   root with the policy of the Vakint four-loop packages
   (`--exact-backend sparse --numerical-depth 2 --finite-case-policy search`,
   no rank scope) and `--checkpoint-dir <root>/sectors`, the path that also
   produced the five-loop owner shards (one single-sector bundle per sector,
   `sector-<ordinal>.rrbin`, ordinal = index in `checkpoint.toml` `sectors`):

   ```
   rustred-4a17f9c7 family-candidates --input four_loop_common_basis.toml --input-format toml \
     --nonpositive-indices 9 --n-cores 16 --exact-backend sparse --numerical-depth 2 \
     --finite-case-policy search --bundle-max-bytes 1073741824 --bundle-max-entries 8000000 \
     --bundle-max-total-coefficient-bytes 536870912 --checkpoint-dir root1022/sectors \
     --checkpoint-max-bytes 4294967296 --output root1022.candidates.rrbin --report-output root1022.generation.toml
   ```
   (root 511: `--nonpositive-indices 0`, `root511/...`.)

   | Root | Sectors | Rules | Finite residuals | Zero sectors (family) | Wall | Peak RSS |
   |---|---:|---:|---:|---:|---:|---:|
   | 1022 | 314 | 22,715 | 386 | 281 | 58.6 s | 0.61 GB |
   | 511 | 328 | 22,228 | 445 | 281 | 25.3 s | 0.88 GB |

   [M] 16 cores each, both runs concurrent on a shared host. The sector, rule
   and residual counts equal the scratch generation with the older binary
   5746feb (TMP/c4l/gen). The 281 zero sectors equal thesis Table 9.1. Both
   roots share one family fingerprint and the saved solver policy
   `ordinary-source-port-default-v1`. Programs of the 134 shared sectors
   differ byte-wise between the two roots.

2. **Routing census** (`census/`). Inputs from `census/make_inputs.py`: the
   momentum rows, the 16 representatives, and the labelled worklist taken
   from the union of the sectors RustRed generated for the two roots.
   Producer: the Symbolica-native census `TMP/tide-routing-census.f3acKs/census`
   (the tool used for the five-loop census). [M] 16/16 forward and 508/508
   reverse witnesses. The witness lines are byte-identical to the scratch run
   in TMP/c4l.

3. **Independent replay** (`audit/`, adapted from the five-loop audit
   `TMP/tide-full-job-routing-audit.0Dz3Dz`; Symbolica only, no RustRed
   linkage, generic in L). It checks the following.
   - The momentum rows against the manifest and the TOML.
   - The root complements and representative bits against the manifest.
   - The zero screen: masks whose active momenta have rank < 4, recomputed with
     Symbolica. This gives 281 in the family and 260 in the union of the root
     downsets.
   - That the worklist equals the downsets minus the zero-screened masks.
   - Every witness: unimodularity, the bijection, and `q_source R = sign q_target`.
   [M] PASS: 508 sources, 2,754 reverse and 105 forward signed momentum
   equalities, and 28 corruption controls rejected (14 per census: matrix
   entry, bijection, indices, source, target, root, deleted edges, sign,
   determinant, non-integral, dimensions, swapped rows, missing witness,
   duplicate witness).

4. **Native load and loader controls** (`loadcheck/`).
   `routed-campaign` loads the 16 owners and verifies all 508 routes with
   `symmetry::verify` and `integral_transport::compile`.
   - [M] A finite trace of 30 targets completes: `finite_trace_complete`,
     20,159 nodes, 9,519 transports, 28 declared terminals, 0 missing
     owners or rules, 2.2 s.
   - [M] Eight corrupted selections are refused at load: sign, coefficient,
     wrong-class owner, swapped owner files, transport flag, duplicate
     source, zero-sector source, swapped rows.

5. **Physics queries** (`plan.sh`, `plan-v1/`): the generic planner at L = 4,
   the analogue of the five-loop entry class. Log-divergent means
   D = A - R = 2L = 8, and the dimension-2 term is D = 7.

   ```
   plan_renormalization_entry_queries.py --loops 4 --manifest selection.json \
     --momenta four_loop_common_manifest.json --parent-witnesses four_loop_common_parent_vertices.json \
     --gauge feynman --difference-set 7,8 --executable rustred-4a17f9c7 --output-directory plan-v1
   ```

   The bounds follow the planner's formulas.
   - Connected: `R_max = D - 3 - V4min`, `A_max = R_max + D`.
   - Nested factorized: `A <= 19 - V4min`, `R <= 12 - V4min`, `D >= 7`.
   - Non-entry: the connected formulas at V4 = 0.
   - Helpers: full orthants at the largest root rank, with unbounded A.

   [M] Results.
   - Classification: 9 connected, 6 factorized and 1 non-entry (841, the
     four-loop banana). Connected V4min histogram {0: 5, 1: 1, 2: 2, 3: 1};
     factorized {1: 3, 2: 1, 3: 2}.
   - Queries: 42 (16 helpers and 26 roots), 61,683,424 finite starting tuples
     counted by Rust `entry-domain-plan`.
   - `check_renormalization_entry_queries.py`: pass (5,200 probes, 0
     disagreements, Rust counts verified).
   - Planner time: 0.8 s.

6. **Historical envelope** (`build_envelope.py`, `envelope/`). This is the
   comparability variant, with the same row shape as the per-family controls.
   - Per owner, local lower 0, active upper `19 - t`, inactive upper 12,
     rank 12, `A <= 19`, `D >= 7`.
   - `stage_saved_owner_campaign.py --anchor-max-numerator-rank 12` appends one
     rank-12 owner orthant per owner after the boxes. This is the historical
     box-then-anchor order.
   - [M] 362,692,824 starting tuples (Rust `entry-domain-plan`).

7. **Matching-only diagnostics** (`match.sh`: `owner-domain-match` without
   `--follow-successors`, unlimited per-query allowances,
   `--max-guard-univariate-degree 64 --bounded-refinement-axes finite-axes`).
   [M] On the final staged bytes, every query is locally applicable, with 0
   unresolved, 0 exact gap and 0 invalid source condition. No helper needed a
   positive-power bound.

   | Variant | Queries | Selected-rule pieces | Terminal pieces |
   |---|---:|---:|---:|
   | physics | 42 | 7,024 | 36 |
   | envelope | 32 | 7,510 | 39 |
   | physics + rank-12 anchors (`four-all`) | 58 | 11,495 | 64 |
   | physics + rank-6 anchors (diagnostic) | 58 | 10,570 | 64 |

   Each run took under 3 s.

8. **Staging** with `examples/python/stage_saved_owner_campaign.py` into
   `TMP/c4l-build.wBU9zC/staged-{physics,envelope,physics-r12anchors}/`.
   These are campaign-style input folders; only the owner paths change.

## 4. Owner payloads (not committed; regenerate and verify)

The 16 single-sector programs total 6,562,373 bytes and are **not committed** (no binary
payload is committed anywhere in this repository; the banana owner alone is 3.3 MB).
`selection.json` pins each owner's SHA-256, byte size, root and native ordinal, and the
SHA-256 of both checkpoint manifests (`receipts.checkpoint_manifest_sha256`):
- root 1022: `47d46b3ed46de97c87338ef32f9f4d372bbec0125e29a6533b74fe9817610882`
- root 511: `9239fb06edbb6d501cf154b19fe6bede7d171b74acf2d3358fc7d663d0ac6952`

The supported path is regeneration with the pinned binary 4a17f9c7 (step 1 of section 3),
which `regenerate_owners.py` automates and verifies:

```
python examples/input/four_loop_combined/regenerate_owners.py \
  --executable TMP/fable51-controls/bin/rustred-4a17f9c7 --work-directory <new dir> [--cpus <list>]
python examples/python/stage_saved_owner_campaign.py \
  --manifest examples/input/four_loop_combined/selection.json --owner-base <new dir> \
  --queries examples/input/four_loop_combined/physics/queries.json --anchor-max-numerator-rank 12 \
  --attach examples/input/four_loop_combined/physics/entry-plan-receipt.json --destination <staged dir>
```

[M] Reproducibility (c4l session 2, 2026-09-27, CPUs 52-63,308-311, 16 generator cores):
- Two independent regenerations (`TMP/c4l-s2/regen/`, and the script in
  `TMP/c4l-s2/regen-script-test/`, receipt `regeneration-receipt.json` status PASS) reproduce
  **all 642 sector programs of both roots byte-for-byte**, the two `checkpoint.toml` files and
  the two candidate bundles, hence all 16 owner payloads (16/16 SHA-256 and sizes match).
- Wall time 60.4 s + 23.0 s (first) and 45.5 s + 34.9 s (script), roots run one after the other;
  peak RSS 0.59 GB and 0.83 GB.
- Staging the regenerated owners from the committed `physics/queries.json` with rank-12 anchors
  reproduces `four-all/queries.json` (`d1ac816e...`) and the staged `selection.json`
  (`b228ac95...`) byte-for-byte.
A different binary may legitimately produce different bytes; the script then reports FAIL and
these pins no longer apply.

## 5. Walks

All walks use the release binary 4a17f9c7 (SHA-256 `4a17f9c7...395e`). The runner calls
`owner-domain-match --follow-successors` with the native options of the five-loop campaign:
`--route-domain-overcover --transfer-unreserved-lookahead 256 --reuse-initial-d-bands
--bounded-refinement-axes finite-axes --max-guard-univariate-degree 64 --unbounded-work`,
checkpoint enabled. The audit is `examples/python/audit_owner_domain_walk.py`; the per-owner /
per-rank breakdown of a records sidecar is `tools/records_breakdown.py`.

Runners: the registered families go through the shared `TMP/fable51-controls/run_control.py`.
The session-2 variants (`four-all-r13anchors`, `-r14anchors`, `-r16anchors`, `-h993r14`,
`-p5`) go through the lane-local copy `TMP/c4l-s2/run_c4l.py` (same argv rewriting and
cooperative stop, imported from the shared runner; `--command <argv template>`; it also records
the foreign load on the run CPUs). Their argv templates are `TMP/c4l-s2/commands/`. Socket-1
(W96) runs held `TMP/locks/socket1.lock` (`TMP/c4l-s2/session.py`, log `sessionA.log`).

### Registered families and variants

| Family | Queries | Content |
|---|---:|---|
| `four-all` | 58 | The 42 planner physics rows plus 16 appended rank-12 owner orthants (`stage_saved_owner_campaign.py --anchor-max-numerator-rank 12`; 12 = 3L, the rank of the historical rank-only anchors). **The combined control** (see "Recommended C-4L gate" below for how to use it). |
| `four-all-physics` | 42 | The planner physics class alone (`physics/queries.json`). |
| `four-all-a19` | 32 | The historical envelope (`envelope-a19r12d7/queries.json`, boxes before orthants). |
| `four-all-r12anchors` | 58 | The same query bytes as `four-all`; the label was used before the rename. |
| `four-all-r6anchors` | 58 | Diagnostic: physics plus rank-6 owner orthants. |
| `four-all-r13anchors`, `-r14anchors`, `-r16anchors` | 58 | Session 2: physics plus rank-13 / 14 / 16 owner orthants (queries `335ed006...`, `9086993c...`, `6682149f...`; `TMP/c4l-s2/staged-physics-r{13,14,16}anchors/`). |
| `four-all-h993r14` | 59 | Session 2: `four-all` plus one rank-14 orthant for the flooding owner `0111110010` (`c3fd3890...`, `TMP/c4l-s2/staged-four-all-h993r14/`). |
| `four-all-p5` | 58 | Session 2: `four-all` query bytes on owners regenerated with the **five-loop generation policy** (`--max-numerator-rank 10 --exact-backend sparse-factorized --numerical-depth 0`; `TMP/c4l-s2/gen-p5/`, `selection-p5.json`, `staged-p5-r12anchors/`). |

### Results [M]

Timings are single observations on a shared host and are not a benchmark. W6/W24 rows of
session 1 ran on CPUs 320-383 (SMT siblings of 64-127); session-2 W24 rows ("own") on
52-63,308-319 (12 physical cores with their SMT siblings), foreign load 6-24 %; W96 rows on
socket 1, CPUs 128-223, foreign load 27-78 % where recorded, so **W96 timings are void**
(counts stay valid). "Drained" means `result.json` with an exhausted worklist and audit PASS.

| Family | Policy, W | Runs | Drained | Natives (drained runs) | Max rank | Not drained |
|---|---|---:|---:|---|---:|---|
| `four-all` | Ordered, W6 | 2 | 2 | 30,159 (both) | 14 | - |
| `four-all` | Ordered, W24 | 2 | 2 | 30,159 (both) | 14 | - |
| `four-all` | Ordered, W96 | 1 | 1 | 30,159 | 14 | - |
| `four-all` | Ready, W6 | 3 | 3 | 24,482-26,065 | 14 | - |
| `four-all` | Ready, W24 | 10 | 10 | 22,290-24,425 | 14 | - |
| `four-all` | Ready, W96 | 5 | 3 | 21,730 / 23,811 / 23,683 | 14 | 2: rep2 stopped at 3,605 s with 5,186,115 natives; rep5 stopped at 900 s with 1,583,327; both rank-13 floods (section below) |
| `four-all-r13anchors` | Ordered W24 / Ready W24 | 1 / 1 | 1 / 1 | 33,750 / 26,600 | 15 | - |
| `four-all-r13anchors` | Ready, W96 | 3 | 3 | 25,473-25,934 | 15 | - |
| `four-all-r14anchors` | Ordered W24 / Ready W24 | 1 / 2 | 1 / 2 | 38,173 / 29,043-29,414 | 16 | - |
| `four-all-r14anchors` | Ready, W96 | 3 | 2 | 28,596 / 31,398 | 16 | 1: rep2 stopped at 900 s with 1,996,696 natives; rank-15 flood on `0111110010` |
| `four-all-r16anchors` | Ready, W24 | 1 | 1 | 36,531 | 18 | - |
| `four-all-h993r14` | Ready, W24 | 1 | 0 | - | - | 1,381,632 natives at 900 s; flood moved to `0111100100` / `0111100000` at rank 14 |
| `four-all-h993r14` | Ordered, W24 | 1 | 0 | - | - | 730,570 natives at 900 s; rank-14 flood on `0111100100` / `0111100000` |
| `four-all-p5` | Ordered W24 / Ready W24 | 1 / 2 | 1 / 2 | 31,717 / 22,855-23,250 | 14 | - |
| `four-all-r6anchors` | Ready, W24 | 3 | 3 | 12,527-13,370 | 12 | - |
| `four-all-a19` | Ready, W24 | 3 | 2 | 22,846 / 22,847 | 14 | 1 killed at 600 s with >= 1,150,778 natives (exit 124, no result) |
| `four-all-a19` | Ready, W6 | 1 | 0 | - | - | 2,660,400 natives at 2,539 s; rank-13 flood on `0111110010` |
| `four-all-a19` | Ordered, W6 | 1 | 0 | - | - | 3,168,826 natives at 3,140 s; rank-13 flood on `0111110010` |
| `four-all-a19` | Ordered, W24 (180 s cap) | 1 | 0 | - | - | 669,422 natives at 180 s; rank-13 flood on `0111110010`; first 600,000 committed records identical to Ordered W6 (ignoring `seconds`) |
| `four-all-a19` | Ready, W96 | 1 | 1 | 23,872 (32 partial initial inspections) | 14 | - |
| `four-all-physics` | Ready, W24 | 1 | 0 | - | - | 2,367,249 natives at 3,600 s (SIGINT, no result); rank-5 flood on `0111110010` and `0111111001` |

Run directories: `TMP/fable51-controls/c4l-4a17f9c7-<policy>-w<W>[own][-repN]/<family>/`
(`command.json`, `events.jsonl`, `result.json`, `audit.json`, `metrics.json`, `checkpoint/`).
Strict record comparisons (`examples/python/compare_walk_records.py --mode strict`, identical,
0 differing records): Ordered W6 vs W6, W6 vs W24 (session 1), W24 vs W24-own
(`TMP/c4l-s2/compare-ordered-w24-vs-w24own-strict.json`) and **W24 vs W96**
(`TMP/c4l-s2/compare-ordered-w24-vs-w96-strict.json`). Reference: `c4l-4a17f9c7-ordered-w24/
four-all/result.json`, SHA-256 `6fbe2e96dbe004a6a94e8b4dc3b9c3c3c656436da0db4143a3cff8846b05def9`,
65,444 logical records, 30,159 natives.

Closure verification [M]: `rustred walk-verify-closure --require-closure` with full re-inspection
(oracle-branch verifier `TMP/w0/oracle/bin/rustred-89558210`, SHA-256 `89558210dda816d3...`,
22 threads on CPUs 52-63,308-319; reports `TMP/c4l-s2/verify/*.json`) passes on five drained runs:
Ordered W24 and W96 and Ready W96 (w96, rep3, rep4). In each, all 32 initial records (26 physics
roots) are certified and independently verified, every native is re-inspected (21,730-30,159),
there are 0 uncovered successors among 2.01-2.18 M, and there are 0 exact-vs-brute-force
containment disagreements. Each check takes 126-163 s.

Details of the drained `four-all` runs:
- Zero frontiers, errors and violations; every ledger obligation discharged; maximum scheduled
  finite rank 14; 0 partial initial inspections.
- 98-99 % of natives are Route inspections (for example 21,489 Route and 241 Apply at Ready W96).
- Apply carries most native seconds; the non-entry banana owner `0111100001` alone takes
  15-37 s over about 20 natives (71-86 % of native seconds).
- Peak process RSS 0.30-0.34 GB at W24 and 0.42-0.46 GB at W96.
- The five-loop-policy owners (`four-all-p5`) drain like the Vakint-policy owners (+5 % natives
  in Ordered W24).

### The fragmenting mode: a flood one rank above the anchors

**Measured [M]** (full breakdown of the 9,121,559-record sidecar of the non-drained
`c4l-4a17f9c7-ready-w96-rep2/four-all`, `TMP/c4l-s2/bd-ready-w96-rep2-four-all.json`, 9.6 s
with `tools/records_breakdown.py`):
- 5,186,115 natives (1,742,023 Apply, 3,444,092 Route) with only 687.6 native-seconds in
  3,605 s of wall at W96: the run was coordinator-bound.
- Owner `0111110010` (class 993, connected, V4min 2): 1,712,757 Apply natives, 517.4 s =
  75.3 % of all native seconds; **1,712,755 of them at rank 13**, one above its rank-12 anchor.
  Secondary rank-13 sweeps: `0111100100` (26,742) and `0111100000` (2,222).
- Self-sustained: only 1,013 Route natives route into `0111110010`. The 3.44 M Route natives are
  on its subsectors (`0111010010`, `0101110010` -> owner `0111110000`; `0111100010` ->
  `0111100100`; `0110110010` -> `0111100001`), i.e. descendants of the flood routed to other
  owners. (`HANDOFF_opus_5_5.md` §8 called them feeders; the direction is the reverse.)
- Shape: point-like in the dots of lines 1 and 2, unbounded dots on line 3, numerator powers
  0-2 on inactive lines, rank <= 13. The largest line-2 dots grow 245, 399, 519, 622, 713, 795,
  872, 943, 1,009, 1,072 over the ten id-deciles, natives per decile only fall from 226 k to
  150 k, mean native 0.29-0.34 ms. [E] The front grows like the square root of the domain count:
  a two-dimensional region of the (dots1, dots2) lattice is being filled with no boundary in sight.
- The second non-drained `four-all` Ready W96 run (rep5, stopped at 900 s with 1,583,327 natives,
  `TMP/c4l-s2/bd-ready-w96-rep5-four-all.json`) floods the same layer with a different box
  family: 58,977 rank-13 Apply natives on `0111110010` (73.3 % of 3,089 native-seconds; wide boxes
  with growing bounds on the dots of lines 1 and 2, up to 0.66 s each) plus rank-13 sweeps on
  `0111111001` (169,082), `0111100100` (108,783) and `0111100000` (47,270).
- The same layer floods in every other non-drained run of this directory: `four-all-a19` Ready W6
  (887,159 rank-13 natives on `0111110010`) and Ordered W6 (1,058,541, 77.6 % of native
  seconds), `four-all-r14anchors` Ready W96 rep2 (690,152 natives on `0111110010` at **rank 15**,
  72.1 %), and `four-all-physics` Ready W24 (1,177,574 at rank 5 = helper rank 3 + 2, 74.1 %;
  `0111111001` 22.3 %).
- In the drained runs checked (Ready W96, Ordered W24, rank-14 anchors at W24) the same owner has
  about 30 Apply natives at anchor rank + 1, with lower corners of at most 5 dots. The anchors'
  own inspections create that layer: the maximum scheduled finite rank is anchor rank + 2 for
  anchors 12, 13, 14 and 16 (14, 15, 16, 18).
- The fragmenting runs are recognisable early: 41-58 k natives at 20 s and 181-479 k at 120 s
  (`events.jsonl` heartbeats), against 22-31 k natives in 9-20 s of traversal for drained runs.

**Interpretation [E]** (the boxes are measured; the causal reading was not replayed). An owner
orthant at rank r contains only rank <= r. Its inspection emits rank r+1 and r+2 successors,
which are admitted as ordinary domains; a later one aliases only if an earlier admitted box
contains it. In the drained W96 run the first wide rank-13 domain of `0111110010` was
`upper = [-,0,-,0,0,0,-,-,1,-]` (line-2 dots unbounded); in rep2 the wide rank-13 boxes admitted
first were `[-,0,1,-,0,0,-,-,1,-]` and siblings (line-2 dots <= 1 or 2, line-3 unbounded), which
leave (dots1 >= 1, dots2 >= 2) uncovered, and the point-like chain starts there. Ready publishes
in completion order, so which boxes win differs from run to run; Ordered is deterministic, so
`four-all` Ordered drained identically at W6, W24 and W96 (30,159), while `four-all-a19`
Ordered floods identically at W6 and W24 (the first 600,000 committed records agree). The partial-initial-overlap timing proposed
in session 1 is not needed: rep2 had 0 partial initial inspections.

### Input-level fixes tested [M]

- **Higher anchors** only shift the uncovered layer: with rank-14 anchors the drained runs are the
  rank-12 picture shifted by two (30 Apply natives of `0111110010` at rank 15, max rank 16), and
  Ready W96 still fragments: 2 of 3 runs drained, and rep2 was stopped at 900 s with 1,996,696
  natives, 690,152 of them rank-15 Apply natives of `0111110010`.
- **A helper for the flooding owner** moves the flood: a rank-14 orthant for `0111110010` alone
  fragments already at Ready W24 (1,381,632 natives at 900 s), now on `0111100100` (637,289) and
  `0111100000` (589,355) at rank 14, fed through the routes from the helper's rank-14 subsector
  descendants (`TMP/c4l-s2/bd-ready-w24own-rep1-four-all-h993r14.json`).
- [E] No finite orthant rank closes the gap (each anchor creates rank r+1 and r+2 descendants), and
  rank-free helpers left 85-556 frontiers per family in
  `docs/research/four_loop_helper_bounds_2026-09-25.md`. Robust Ready drainage is therefore an
  engine property (admission order / coverage of the r+1 layer), not an input fix.

### Recommended C-4L gate

1. **C-4L-O (identity, required):** `four-all` Ordered at W6 and W24 (W96 when socket 1 is
   free). Pass = drained, audit PASS, 0 frontiers, and strict record identity with the
   reference above (30,159 natives, 65,444 records), plus `walk-verify-closure --require-closure`
   with full re-inspection PASS (about 2-3 min). Measured width-invariant at W6/W24/W96.
   For an engine change that intentionally changes records (the v3 epoch engine), replace
   identity by: drained, audit and verify-closure PASS, identical results across W6/W24/W96,
   and natives within +-10 % of 30,159 [E: band chosen, not measured].
2. **C-4L-R (Ready drain at width, diagnostic on the legacy engine):** `four-all` Ready W96 x3,
   each capped at 15 min; a run may be stopped early once it passes 60,000 natives (twice the
   Ordered reference): every fragmenting run of this lane (W6-W96) passed 64-263 k natives by 60
   s, and no drained run of any `four-all` variant exceeded 38,173 natives [M]. Record drained /
   not drained and, for a non-drained run, the `tools/records_breakdown.py` signature. On the
   legacy engine a non-drain whose native seconds are dominated by Apply natives at anchor rank +
   1 on the V4min-2 connected owners (`0111110010`, `0111111001`) is known behaviour (3 of 5 runs
   drained at W96), not a regression; any other signature, a frontier or an audit violation is a
   failure. For the v3 epoch engine (deterministic by design) it becomes a required arm: 3/3
   drained with identical results.
3. **Keep** the per-family FG/BMW/H/X Ordered W6 controls as the zero-Route arm (owner rule:
   "in addition to" the combined run), and keep `four-all-physics` as the small C-HOT-like stress
   case (planner helper ranks as at five loops, same flood).

Anchors at rank >= 14 are **not** adopted: they fragment at Ready W96 as well and raise the work
(Ordered W24 38,173 natives against 30,159).

## 6. Digests (SHA-256)

Committed files (paths relative to this directory), then the main evidence files:

```
7c15069db344719b4f0ef263ac9968b49eb0e11b84cf31163298553bbfbe0166  ./envelope-a19r12d7/entry-plan.json
f28578a190af1f788abf332c1dbe2e10d8a7748ee7dadf9c474551f195fa336c  ./envelope-a19r12d7/entry-spec.json
9bd07ccd8d5fbae4995c56acca2e6ebc4df4e37d233915a19f5a9bb398205965  ./envelope-a19r12d7/input-receipt.json
3950672959ba23e8e46e837056c3fe797f7e527436982c1df941301a5d80cacb  ./envelope-a19r12d7/matching-summary.json
d9015446ac7dc868d86d200a70771812f3f6785bd3082f319021a4d80a5116c0  ./envelope-a19r12d7/queries-boxes.json
78d632922db5da260e719d22cd2cd224a2ad3e2e3c946e2026d455a5509cbf65  ./envelope-a19r12d7/queries.json
33cf58c776104f1768cd676602136298145a8b04e596f1c7e1f327575940a934  ./four_loop_common_basis.toml
20a0ca15010154d0073617205b435904e4b9600932505bbe8ad0d357424fcb39  ./four_loop_common_labelled_routes.jsonl
a619209e9a7ab4a504eb23f4e48ec730411716469bcbda12aa09df49579e615e  ./four_loop_common_manifest.json
2d716da1e9156e79b563d2b4bd5766fccf3f4f5bc8d88dea3e50d05afa7d18c7  ./four_loop_common_parent_vertices.json
5596695b976ac7c9676e71627693b41e6487c9f8fab20d1bdb8bbd90a3838d6f  ./four_loop_common_routing_witnesses.json
af96e2838a277180dd7ef5e90e0b683e06f9b6895845ba889ab39faa396bdcee  ./physics/entry-plan-receipt.json
114cb10fc0b93614ec2b83b6db0fa006496fe831cae4e381179a8e139d0f843b  ./physics/entry-plans/a10-r2-dmin8-dmax8.json
f0e50307c9652b7979676ff7a6e5215ab00f69a8fa01482b5161c1560d73ce28  ./physics/entry-plans/a10-r3-dmin7-dmax7.json
d955363064e64a1731748869340fad19eb9e5071e45c0c4875366a0ed0252013  ./physics/entry-plans/a11-r3-dmin8-dmax8.json
e601f7c3faa6bf8c7c92ed54307f21304be8dc8fe3e693bd8e57e1549dfd9115  ./physics/entry-plans/a11-r4-dmin7-dmax7.json
b210d97760ed9269cde19dc280ed6ffef990752c08719b1adc86812392172c85  ./physics/entry-plans/a12-r4-dmin8-dmax8.json
e2b1a38b55c594ef05a380d5a03755dfad7a6930e7510a4ccced782f4e612c2a  ./physics/entry-plans/a13-r5-dmin8-dmax8.json
2e4587e58481f656c31a2cc478c567729384352ef705ae7d89413a6f82ac9cd6  ./physics/entry-plans/a16-r9-dmin7-dmaxnone.json
11df2ba3ef85bff2f771e5c5b9704d8cd8a40a48ef6d7c7ba34bcff00f2efd95  ./physics/entry-plans/a17-r10-dmin7-dmaxnone.json
1b98254c0a6742a8cbf4a1976b6e5708273fcb781104c3496584f10fae054bf7  ./physics/entry-plans/a18-r11-dmin7-dmaxnone.json
2f22122d77ffb20f663699ea1a9e80fbc9c6b17de8d560b038c4dc5a14426364  ./physics/entry-plans/a8-r1-dmin7-dmax7.json
77602489c9cf79e3d5cf335597c88427aa3c6a1999df8c5ce415a7b100c6d208  ./physics/entry-plans/a9-r2-dmin7-dmax7.json
2e6ec61205366be15363854a882687a0e4d5e392741328984d53f2108591519c  ./physics/input-receipt.json
2b8fb08b2491578df53ec31dddbcda8c6410ca933a258055d9dedcb327c8b367  ./physics/matching-summary.json
bc30baf241c2aac5a8fc035c9124d068e9ebf8d077afcd440bd3ee0b3dbb2037  ./physics/queries.json
b358817991a3e4577a6e3edcf32f5484545145f9aa2d3e37ce29f504c1143245  ./physics/skeleton-classification.json
b228ac95f94bd9b0f14103fe1a5520cffaebb8538b7c32d74c823aa65611aec5  ./selection.json
29e4b788d7c204d179d405988bf90b14b5ee976ccdc5ede4f3c7f5261611ade4  ./regenerate_owners.py
3d81cefb5bb2be6da870852171b15c58dd436a3a2f6dbaf0fbb05b95fdc8dc1d  ./tools/records_breakdown.py

# evidence (TMP/c4l-build.wBU9zC unless absolute)
4a17f9c7c0447713370a1aed2e54c9a04251106a9183fd1b8a86dc5d1abb395e  bin/rustred-4a17f9c7
9a6915086034857fd666f458c7c5397287af6ad103684f4dca3443c22b5119fb  gen/root1022.candidates.rrbin
8a0e663a34db7ab8cfff0f15f5c5be72f1c9cfea96daef22be874e033a63e5b2  gen/root511.candidates.rrbin
47d46b3ed46de97c87338ef32f9f4d372bbec0125e29a6533b74fe9817610882  gen/root1022/sectors/checkpoint.toml
9239fb06edbb6d501cf154b19fe6bede7d171b74acf2d3358fc7d663d0ac6952  gen/root511/sectors/checkpoint.toml
6b637fdaa158f9a2af52bcd84c93a47e98200266ee7045a45dc0583cfffec07e  gen/run_gen.sh
10e0d2bd92ab263730b1fe65a0e76db0f8483a3751975c7022db46f1beb0d3b0  census/make_inputs.py
d5c7cef8551545ea491b9d69b77f64ce53b037f90c982665d3a060cf97881ee9  census/forward.json
76c5dcbacf4b241e22a94242aaf349e8eee134aff60d84c811b4cd492bf47723  census/reverse.json
d3196af189356f70e60a9f9c3db7f5c53b89a57c739b0f7a40ee42e4d7958563  /common/dev/rustred/TMP/tide-routing-census.f3acKs/census
e789695c725a97fb0203f3aaf134aacaebfb3329fe75f817699122ba2cd82725  audit/main.rs
f536db87b4c71ba36b32b651a2e36f74654d416c74a7d434189d29a431dd030a  audit/audit
e80d91cae56a2e1eefc0052373057d9279f622153242a118aebb5fbd56711e7a  audit/run.stdout
72a29ab2b869d006946d13f9190013753f5e730e374380a07c7d2e517ac2f152  build_selection.py
9ac31d94b781044faf1f3e60fe68de432b4e80cc27cbe9e348470a84335445b2  build_envelope.py
0cc0f71ad9fbecc7836511a05ea16138404ee5af0e5609086e7c993207577dc7  plan.sh
fc2e6462760d86a8ea897606122940dc11162a03f316eaf56b2db974afff7143  match.sh
6629b744eade6529a1cd3a46f310e01b71d19808f94787f2cfd880ee56368d9e  socket1_session.sh
cfdf07b6723a55a4b2ed3a8d875f1bda658c9d64445e6cd34ebd2458094ed19e  commands/command-four-all.json
10640e2d71043b0cb841581661f1c63e270f11b7b13384604c4fd6759d785dfd  commands/command-four-all-physics.json
3022c6047c09dcd35944f7080c873abf18aa758495b51aa5c923f2d95abc8fb5  commands/command-four-all-a19.json
eed100407d7ffaeab9f4bbcf4d77caa1cf1543e3834819d8549254af8952c432  commands/command-four-all-r6anchors.json
d1ac816eb5ad5473e05ba5cc618d9f9273fd05dd31dff0c90b05704b1a072e81  staged-physics-r12anchors/queries.json
e334f4d353be4736c111d2e1a5789bae446e2ef186463cd28bb064721beb37fd  staged-physics-r6anchors/queries.json
8dace33c1bdd5e9dce251354c3743bef29e7625ee4b6b153847b0e3130de9051  /common/dev/rustred/TMP/fable51-controls/run_control.py
# session 2 evidence (absolute)
104453012643e0919495417f66ba768593911a348e722d2530c287b3420d9fd7  /common/dev/rustred/TMP/c4l-s2/run_c4l.py
da133bd0dfe0e126f80981c467b15b390973464bda960532d4d89904fe20bdb5  /common/dev/rustred/TMP/c4l-s2/session.py
0ba969774349b83784a56bb7b926dbe1962c3b00aa61cdd4c8de0fd1f5463345  /common/dev/rustred/TMP/c4l-s2/queueA.json
367b672d039b2b6d306fe803afe30546639f24cbf8884da82f63b5a313f3b192  /common/dev/rustred/TMP/c4l-s2/breakdown.py
1a66e7cf84a811f59ca7a40849838bd86d004591e410cd2ec8c0f3660dd6f5d0  /common/dev/rustred/TMP/c4l-s2/bd-ready-w96-rep2-four-all.json
81ade07599790af491e5df30c41ff03a628f11d5ac610d6ccf5b673e0a259391  /common/dev/rustred/TMP/c4l-s2/bd-ready-w96-rep2-four-all-r14anchors.json
e684a753f7aa4632751ed036f5cf8fce8e56fed2778125019aa523484ee5dc41  /common/dev/rustred/TMP/c4l-s2/bd-ready-w24own-rep1-four-all-h993r14.json
88effbe0ccf6a3c220d9cc2a8b4bffaa500fb809b610f582ea9314cfb8b1a1b3  /common/dev/rustred/TMP/c4l-s2/bd-ready-w6-four-all-a19.json
188b98507b0ed4ea7deced5339f0f0d129e72ff06bb646f7eeaa6009aad787af  /common/dev/rustred/TMP/c4l-s2/bd-ordered-w6-four-all-a19.json
852dd5e5e1494b1d4f8ac443ef222369dae03b02ec5312a922257d3080758bcb  /common/dev/rustred/TMP/c4l-s2/bd-ready-w24-four-all-physics.json
36f80dd7f5c77a5540a985d6866e5b5400467057097a90582e06f55b85be0a6f  /common/dev/rustred/TMP/c4l-s2/compare-ordered-w24-vs-w24own-strict.json
f4dadd7bee205f68f0bbfc0429eaf5ef53354b43936552abd6d3e861283435c0  /common/dev/rustred/TMP/c4l-s2/compare-ordered-w24-vs-w96-strict.json
335ed006ecd0a30fd99ff2d6b9f1399ceda49c3d631c610fa97f1a2aa7ce3268  /common/dev/rustred/TMP/c4l-s2/staged-physics-r13anchors/queries.json
9086993cee3e43697c82a251aeb001dc953f6b6e257071e832af2ba69b3b3400  /common/dev/rustred/TMP/c4l-s2/staged-physics-r14anchors/queries.json
6682149f61375ba952a5e37ea19036a388a6e2bd5d0efb8e2a4bc4042b6689b8  /common/dev/rustred/TMP/c4l-s2/staged-physics-r16anchors/queries.json
c3fd3890d6456c3347da50e3033dff52aed69d824b96dda4ad98ccb1db45e485  /common/dev/rustred/TMP/c4l-s2/staged-four-all-h993r14/queries.json
fd0e259f41a6dbad777cc2e28726c814938a277976ce40b41a43cc7e70e561dd  /common/dev/rustred/TMP/c4l-s2/selection-p5.json
4e8a93d4b29a93f1d65125ec78f431e833cb4f173decee9deb30cc582fbcbd44  /common/dev/rustred/TMP/c4l-s2/gen-p5/root1022/sectors/checkpoint.toml
ff79221efaba44a1c9ed7cd5a9361a4b0b72f6ee6434ef4397e8968e9de175ad  /common/dev/rustred/TMP/c4l-s2/gen-p5/root511/sectors/checkpoint.toml
e6b704064a0a6580fcb4daf7fc9d749b6b6c89621c27b8ca5c14f18c54f3bd68  /common/dev/rustred/TMP/c4l-s2/regen-script-test/regeneration-receipt.json
6fbe2e96dbe004a6a94e8b4dc3b9c3c3c656436da0db4143a3cff8846b05def9  /common/dev/rustred/TMP/fable51-controls/c4l-4a17f9c7-ordered-w24/four-all/result.json
```
