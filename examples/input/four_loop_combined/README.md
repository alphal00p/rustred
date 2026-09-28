# Combined four-loop owner run (common Luthe A4 basis)

One combined owner-domain walk over **all four-loop owners** in a single common
momentum basis, built to resemble the five-loop campaign as closely as possible:
one family, class owners generated in that family, every labelled sector routed
to its class owner, and physics-scoped entry queries from the generic planner.
It is an **additional** four-loop control. It does **not** replace the C-4L gate, which stays the
four per-family FG/BMW/H/X controls (four bases, 900 installed owners, zero Route records). The
owner asked for the combined run in addition to them (section 5, "How to use `four-all`").

**Scope and caveats, stated plainly.**
- `four-all` is the **planner physics class** (`--loops 4 --gauge feynman --difference-set 7,8`,
  i.e. D = A - R in {7, 8}, 42 rows) **plus 16 appended rank-12 owner orthants**. It is **not**
  the owner's historical A <= 19 / R <= 12 / D >= 7 saved-cover envelope.
- The envelope variant is `four-all-a19`. It drained in 3 of its 7 runs [M]. The other 4 were
  stopped undrained:
  - Ready W24: killed at its 600 s cap.
  - Ready W6 and Ordered W6: stopped by an operator at 2,541 s and 3,142 s, after the
    non-drain was seen.
  - Ordered W24: stopped at its 180 s cap.

  The Fable 5.1 critique counts "2 of 5 within 1 h"; that count covers the two W6 runs.
- **Ordered `four-all` is the identity oracle.** It gives 30,159 natives at W6, W24 and W96, with
  strictly identical records.
- **Ready `four-all` is schedule-sensitive.** One Ready W96 run did 239x the natives of a drained
  run (5,186,115 against 21,730) and did not drain in 1 h. Ready (and v3 Rolling) results are
  judged statistically. The rule is pre-registered: at least 13 runs per arm and width, interleaved
  with legacy runs in the same session on the same CPUs, and a one-sided Fisher test of the drain
  fractions (section 5, item 2). They are never a binary drain gate.
- **Owner decision pending (D-memo item D-C4L, section 5 end).** This scope substitution (physics class
  plus anchors instead of the A <= 19 envelope) and the choice of the Vakint generation policy over
  the five-loop policy (`four-all-p5`) have not yet been put to the owner.
- **Owner programs.** They use the Vakint package generation policy (`--exact-backend sparse
  --numerical-depth 2 --finite-case-policy search`, no rank scope). The five-loop owners were
  generated with `--max-numerator-rank 10 --exact-backend sparse-factorized --numerical-depth 0`.
  A five-loop-policy variant (`four-all-p5`) was measured (section 5).
- **Owner payloads.** The 6.56 MB of owner payloads are **not committed**. They are regenerated
  byte-for-byte with `regenerate_owners.py` in about 1.4 min and checked against the SHA-256 pins
  in `selection.json` (section 4).

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
| `tools/run_c4l.py` | Control runner with the mandatory recorder and the cooperative stops (`--timeout-seconds`, `--stop-natives`). It is self-contained: the argv rewriting is copied from the shared runner at `8dace33c` (section 5). |
| `tools/session.py` | Session driver for queued runs under a CPU lock. It logs the queue's SHA-256, since the queue is the pre-registration of a C-4L-comb-R comparison. |
| `tools/ready_stats.py` | Per-width statistics over run directories: drain fractions, natives, the non-drain signature and a slow-run flag (section 5). |
| `tools/comb_r_test.py` | The C-4L-comb-R decision rule (one-sided Fisher exact, candidate against interleaved legacy runs) with `--self-test` (section 5, item 2). |
| `tools/command-four-all.json` | The `owner-domain-match` argv template of every `four-all` walk (byte copy of `TMP/c4l-s2/commands/command-four-all.json`; its `--manifest`, `--owner-base` and `--queries` point at the staged folder of step 8). |
| `four-all/` | **The combined control** (`run_control.py --family four-all`; an addition to the C-4L gate, how to use it: section 5, "How to use `four-all`"): `queries.json` = the 42 physics rows followed by 16 rank-12 owner orthants (58 rows), its `matching-summary.json` and staging `input-receipt.json`. |
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

Runners, as used: session 1 ran the registered families through the shared
`TMP/fable51-controls/run_control.py` (at `8dace33c`, saved as `run_control.8dace33c.py`; the fix round
of 2026-09-28 corrected only its comments, now `4d10407c`). Sessions A, B2 and C ran through the lane-local
`TMP/c4l-s2/run_c4l.py`, which imports the shared runner's argv rewriting and cooperative stop and takes
`--command <argv template>`. It adds the foreign load on the run CPUs and the recorder of item 4 below.
With `--stop-natives N` it stops the run cooperatively once a heartbeat reports N natives. Session A used
the version without the stop option, saved as `run_c4l.v1.py`. The recorder-bearing Ready W6 repeats on
52-57 used an intermediate version with the recorder but without the stop option, which was not saved
separately; the current file differs from it only by that option. The argv templates are in
`TMP/c4l-s2/commands/`. Socket-1 runs held `TMP/locks/socket1.lock`, driven by `TMP/c4l-s2/session.py`
(logs `sessionA.log`, `sessionB2.log` and `sessionC.log`; session 1: `TMP/c4l-build.wBU9zC/socket1_session.log`).

**Committed tools (use these from now on).** `tools/run_c4l.py` is the self-contained successor of the
lane runner. Its argv rewriting, result extraction and CPU parsing were tested identical to the shared
runner's on the `four-all`, `-p5` and `-r13anchors` templates. It adds user and system seconds, page
faults, context switches and the effective user clock to the recorder. The other committed tools are
`tools/session.py` (logs the queue digest), `tools/ready_stats.py` (per-width statistics, the non-drain
signature and slow-run flags) and `tools/comb_r_test.py` (the C-4L-comb-R decision rule). The argv
template of `four-all` is `tools/command-four-all.json`. The statistics in this section are regenerated
from the run directories by `tools/ready_stats.py` (`TMP/c4l-s2/fix/ready_stats.{json,md}`; the
session-2 version was `TMP/c4l-s2/ready_stats.py`).

**Regenerating the Ordered reference.** The reference `result.json` (58 MB) lives only in TMP. It is
regenerated with the pinned binary 4a17f9c7, which is deterministic under Ordered. The steps are:
regenerate and stage the owners (section 4), then run `tools/run_c4l.py --binary rustred-4a17f9c7
--command tools/command-four-all.json` (with the staged paths substituted) `--policy ordered --workers 24`.
Compare records with `examples/python/compare_walk_records.py --mode strict <new>/result.json
<reference>/result.json`. The identity is a record identity (65,444 records, 30,159 natives, 0 differing
records), not a file digest: `result.json` bytes differ between widths through timing and worker fields
(W24 `6fbe2e96...`, W96 `7b8b7507...`).

### Registered families and variants

| Family | Queries | Content |
|---|---:|---|
| `four-all` | 58 | The 42 planner physics rows plus 16 appended rank-12 owner orthants (`stage_saved_owner_campaign.py --anchor-max-numerator-rank 12`; 12 = 3L, the rank of the historical rank-only anchors). **The combined control**, an addition to the C-4L gate (see "How to use `four-all`" below). |
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
socket 1, CPUs 128-223, foreign load 27-88 % where recorded, so **W96 timings are void**
(counts stay valid). "Drained" means `result.json` with an exhausted worklist and audit PASS.

| Family | Policy, W | Runs | Drained | Natives (drained runs) | Max rank | Not drained |
|---|---|---:|---:|---|---:|---|
| `four-all` | Ordered, W6 | 2 | 2 | 30,159 (both) | 14 | - |
| `four-all` | Ordered, W24 | 2 | 2 | 30,159 (both) | 14 | - |
| `four-all` | Ordered, W96 | 1 | 1 | 30,159 | 14 | - |
| `four-all` | Ready, W6 | 5 | 5 | 24,482 / 25,935 / 26,065 (min / median / max) | 14 | - |
| `four-all` | Ready, W24 | 10 | 10 | 22,290-24,425 | 14 | - |
| `four-all` | Ready, W24 on socket 1 (CPUs 128-151, session C) | 8 | 7 | 21,904 / 24,288 / 24,307 | 14 | 1 (rep8) at the 60 k early stop (63,477 natives after 33 s of traversal); rank-13 point-like flood, 12,457 rank-13 Apply natives on `0111110010` (12,459 over all ranks). Drained rep5 took 92.0 s of traversal against 7.9-10.8 s for its siblings (see item 4 below) |
| `four-all` | Ready, W96 | 13 | 7 | 21,730 / 23,683 / 24,107 (min / median / max) | 14 | 6, all rank-13 floods on `0111110010` (section below): session 1 rep2 stopped at 3,605 s with 5,186,115 natives; session A rep5 at 900 s with 1,583,327; session B2 rep6, rep7, rep8, rep10 at the 60 k early stop (61,483-63,839 natives after 28-51 s of traversal) |
| `four-all-r13anchors` | Ordered W24 / Ready W24 | 1 / 1 | 1 / 1 | 33,750 / 26,600 | 15 | - |
| `four-all-r13anchors` | Ready, W96 | 11 | 9 | 25,473 / 25,934 / 31,693 | 15 (one run 16) | 2 (B2 rep5, rep7) at the 60 k early stop (66,767 and 63,034 natives); rank-14 flood (anchor + 1) on `0111110010` |
| `four-all-r14anchors` | Ordered W24 / Ready W24 | 1 / 2 | 1 / 2 | 38,173 / 29,043-29,414 | 16 | - |
| `four-all-r14anchors` | Ready, W96 | 3 | 2 | 28,596 / 31,398 | 16 | 1: rep2 stopped at 900 s with 1,996,696 natives; rank-15 flood on `0111110010` |
| `four-all-r16anchors` | Ready, W24 | 1 | 1 | 36,531 | 18 | - |
| `four-all-h993r14` | Ready, W24 | 1 | 0 | - | - | 1,381,632 natives at 900 s; flood moved to `0111100100` / `0111100000` at rank 14 |
| `four-all-h993r14` | Ordered, W24 | 1 | 0 | - | - | 730,570 natives at 900 s; rank-14 flood on `0111100100` / `0111100000` |
| `four-all-p5` | Ordered W24 / Ready W24 | 1 / 2 | 1 / 2 | 31,717 / 22,855-23,250 | 14 | - |
| `four-all-p5` | Ordered, W6 and W96 (fix round, committed runner; W6 on CPUs 52-57, W96 on 128-223 in session F, 23 s of socket-1 hold) | 1 / 1 | 1 / 1 | 31,717 at both widths; records strictly identical to Ordered W24 (68,483 records, `TMP/c4l-s2/fix/compare-p5-ordered-w24own-vs-w{6own,96}-strict.json`), so `four-all-p5` is width-invariant under Ordered at W6, W24 and W96 | 14 | - |
| `four-all-p5` | Ready, W96 | 3 | 2 | 22,152 / 25,292 | 14 | 1 (B2 `c4l-4a17f9c7-ready-w96`) at the 60 k early stop (63,392 natives); rank-13 flood on `0111110010` |
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
containment disagreements. Each check takes 126-163 s. Binary 89558210 predates the oracle gate fields
`roots_total` and `roots_independently_verified`. In the fix round the Ordered W24 reference was
therefore re-verified with the current oracle binary `TMP/w0/oracle/bin/rustred-46d4dd28` (22 threads,
`--reinspect all --require-closure`, 177 s). The report `TMP/c4l-s2/fix/verify-46d4dd28-ordered-w24_four-all.json`
gives verdict PASS, 32/32 roots independently verified, 30,159 natives re-inspected and 0 uncovered.
`examples/python/assert_oracle_pass.py` (oracle branch, commit 4395ae41) reports GATE-PASS on it.

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
- Session B2 (Ready W96, 60 k early stop, `TMP/c4l-s2/post_b2.json` and `bd-ready-w96-rep*-*.json`):
  all 7 non-drained runs carry the same signature. `0111110010` has 1,500-13,755 Apply natives at
  anchor rank + 1 (rank 13 for `four-all` and `four-all-p5`, rank 14 for `four-all-r13anchors`),
  against about 30 in a drained run. The two flavours of session A recur:
  - point-like, as in rep2: 13.3-13.8 k on `0111110010` alone (`four-all` rep8 and rep10,
    `four-all-p5`, `four-all-r13anchors` rep5);
  - wide boxes, as in rep5: 1.5-1.7 k on `0111110010` plus 2.8-3.0 k on `0111111001` (`four-all`
    rep6 and rep7, `four-all-r13anchors` rep7).

  At a 60 k stop the flood does not yet dominate native seconds: `0111110010` takes 18-57 % of them
  in the session-B2 non-drains. The banana owner's ordinary early work still takes 34-70 %.
- Session C's one W24 non-drain on socket 1 (rep8, `bd-ready-w24s1-rep8-four-all.json`) is the
  point-like flavour: 12,457 rank-13 Apply natives on `0111110010` at the 60 k stop (12,459 over all
  ranks). Their share of native seconds is only 9.5 %; the banana owner still takes most of them. So the
  share at the stop covers 9.5-57 % over the eight early-stopped non-drains, and the signature is stated as a
  count.
- The same layer floods in every other non-drained run of this directory: `four-all-a19` Ready W6
  (887,159 rank-13 natives on `0111110010`) and Ordered W6 (1,058,541, 77.6 % of native
  seconds), `four-all-r14anchors` Ready W96 rep2 (690,152 natives on `0111110010` at **rank 15**,
  72.1 %), and `four-all-physics` Ready W24 (1,177,574 at rank 5 = helper rank 3 + 2, 74.1 %;
  `0111111001` 22.3 %).
- Drained baseline [M, 19 breakdowns]. The four session-2 breakdowns cover Ready W96 (the session-1 run),
  Ordered W24, and the rank-14 anchors at Ordered and Ready W24. The fix round added 15 drained runs of sessions B2 and C
  (`TMP/c4l-s2/fix/bd-*.json`): four-all W96 rep9/11/12/13, four-all W24 on socket 1 rep1-7,
  `-r13anchors` W96 rep9/11 and `-p5` W96 rep2/3. `0111110010` has exactly 30 Apply natives at anchor
  rank + 1 in 18 of the 19 runs, with lower corners of at most 5 (`A_lo`). The exception is 188 (`A_lo` up
  to 16), in the `-r13anchors` run B2 rep11 that reached rank 16. `0111111001` has 0 in all 19, and `0111100100` has 11-14.
  The anchors' own inspections create that layer. The maximum scheduled finite rank is anchor rank + 2
  (14, 15, 16, 18 for anchors 12, 13, 14, 16) in 55 of the 56 drained runs with such anchors, or 58 of 59
  when the three drained `four-all-a19` runs (rank-12 orthants) are included. The one exception, B2 rep11
  (31,693 natives), reached rank 16.
- Early stop [M, `events.jsonl` heartbeats, process-elapsed seconds]. Ten non-drained runs of this
  directory ran past 60 s: `four-all` W96 rep2 and rep5, `-a19` x4, `-h993r14` x2, `-r14anchors` W96 rep2
  and `-physics`. By 60 s they had 64-261 k natives, while no drained run of any variant exceeded 38,173 natives in
  total. This supports the 60 k early stop. At 20 s the ranges overlap: non-drains had 29-101 k
  natives (Ordered W6 `-a19` 29,330), and the drained Ordered W24 `-r14anchors` run had 37,463.
  So no 20-s rule is stated. (The earlier "41-58 k at 20 s and 181-479 k at 120 s" fitted only three W96
  runs on traversal time and is withdrawn.) The seven session-B2 non-drains reached the 60 k stop after
  28-51 s of traversal, and the session-C one after 33 s.

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
- **Rank-13 anchors** behave the same way: 9 of 11 Ready W96 runs drained, and the other two
  flood at rank 14 = anchor + 1 on `0111110010` (13,755 and 1,500 Apply natives at that rank at
  the 60 k stop). Their drain fraction is not distinguishable from `four-all` (9/11 against 7/13, Fisher
  exact p = 0.21).
- **A helper for the flooding owner** moves the flood: a rank-14 orthant for `0111110010` alone
  fragments already at Ready W24 (1,381,632 natives at 900 s), now on `0111100100` (637,289) and
  `0111100000` (589,355) at rank 14, fed through the routes from the helper's rank-14 subsector
  descendants (`TMP/c4l-s2/bd-ready-w24own-rep1-four-all-h993r14.json`).
- [E] No finite orthant rank closes the gap (each anchor creates rank r+1 and r+2 descendants), and
  rank-free helpers left 85-556 frontiers per family in
  `docs/research/four_loop_helper_bounds_2026-09-25.md`. Robust Ready drainage is therefore an
  engine property (admission order / coverage of the r+1 layer), not an input fix.

### How to use `four-all` (in addition to the C-4L gate)

0. **C-4L stays as defined in the master plan.** It is the per-family FG/BMW/H/X controls, the
   zero-Route arm. `four-all` is added next to it and never replaces it.
1. **C-4L-comb-O, identity and closure (deterministic schedules).**
   - *Legacy engine, record-keeping changes* (for example the MVP-A levers or the SoA kernel). Run
     `four-all` Ordered at W6 and W24, and at W96 on socket 1 (CPUs 128-223). The W96 run holds
     `TMP/locks/socket1.lock` and uses the recorder; socket 1 stays shared (HANDOFF §0.1 item 11).
     Pass requires:
     - drained, audit PASS and 0 frontiers;
     - strict record identity with the reference above (30,159 natives, 65,444 records; measured
       width-invariant at W6, W24 and W96);
     - `walk-verify-closure --require-closure --reinspect all` with the current oracle binary, gate
       PASS: `verdict == PASS` and `roots_independently_verified == roots_total` (32 root records),
       checked with `examples/python/assert_oracle_pass.py` (oracle branch). This takes about 3 min at
       22 threads.
   - *Engines or levers that change the records on purpose* (the v3 epoch engine; work-volume levers
     such as G2'). Run the engine's deterministic schedule at W6, W24 and W96: v3
     `Lockstep{B, depth 1}` with `--canonical-resolution` (v3 design §3.4), or Ordered for a
     legacy-engine lever. Pass requires:
     - drained, audit PASS, 0 frontiers and 0 errors;
     - the verify-closure gate PASS as above (32/32 roots independently verified);
     - width invariance: byte-identical records across W6, W24 and W96 in canonical mode (the v3 S2
       gate names W6, W12 and W24; W96 is added here);
     - a one-sided upper guard: natives <= 33,175 = 1.1 x the Ordered reference. If the lever
       pre-registered its own expected range before the run, that range replaces the guard. There is
       no lower bound. Fewer natives are the aim of the work-volume levers, and soundness rests on
       verify-closure, not on the count.
     - Report, for context, the natives and scheduled domains (`scheduled_nodes`) against both legacy
       references: the Ordered 30,159, and the drained legacy Ready distribution, 21,730-26,065 over
       29 runs (W6 24,482-26,065; W24 21,904-24,425; W96 21,730-24,107).
   - The earlier two-sided band of ±10 % around 30,159 [E, chosen, not measured] is **withdrawn**. All
     29 drained legacy Ready runs lie below its lower edge (27,143). A measured lever such as G2'
     (scheduled domains 0.79-0.80x, orchestrator decision 7) would fall outside it by design.
2. **C-4L-comb-R, Ready/Rolling statistics with a pre-registered decision rule (never a binary drain
   gate).**
   - *Arms.* Arm C is the candidate in its production, non-deterministic schedule. For the v3 engine
     that is `Rolling{D, cut}` with the parameters planned for the launch, scaled to W. For a
     legacy-engine lever it is 4a17f9c7 Ready with the lever on. Arm L is the frozen legacy binary
     4a17f9c7 at Ready, with the same argv template. Lockstep and Ordered are deterministic and belong
     to comb-O.
   - *Design (pre-registered).* Per width W in {6, 24, 96}:
     - n >= 13 runs per arm (20 recommended at W96), interleaved L, C, L, C, ... in one session on
       one fixed CPU set (W96: CPUs 128-223; W24: 24 fixed CPUs);
     - the lock that covers those CPUs is held (socket 1: `TMP/locks/socket1.lock`), and the recorder
       is on;
     - each run is capped at 15 min and stopped early at 60,000 natives (`tools/run_c4l.py
       --stop-natives 60000`). An early stop counts as "not drained".

     The queue file (arms, widths, n, order, CPUs, caps) is the pre-registration. It is written before
     the session, and `tools/session.py` logs its SHA-256; it is not edited afterwards. There is no
     optional stopping; a second batch is a new pre-registered comparison. Cost [E, from sessions B2
     and C]: about 25-30 s of hold per W96 run with the 60 k stop (B2: 19 runs in 476 s), so 26 runs
     take about 11-13 min. The historical legacy table
     below is context only, not the comparator. Its W96 runs all ran on socket 1, spanning NUMA nodes 4-6,
     under 27-88 % foreign load where recorded. Its W6 and most of its W24 runs ran on socket 0. Width
     and host are therefore confounded in it.
   - *Test.* Per width, `tools/comb_r_test.py` computes the one-sided Fisher exact p-values on the
     drained/total table, at alpha = 0.05 per width:
     - p_regression, with H1: the candidate drain fraction is below legacy;
     - p_improvement, with H1: it is above legacy.
   - *Promotion is BLOCKED* by any of:
     - a candidate frontier, error exit, or run that finished on its own with audit FAIL;
     - a candidate non-drain without the known signature (below);
     - p_regression < 0.05 at any width;
     - a verify-closure report on a drained candidate run that is not a gate PASS.

     The comparison is **VOID** (repeat it; promote nothing) if an arm has n < 13 at a width, or if
     the two arms used different CPU sets. It is **INCOMPLETE** until two drained candidate runs per
     width carry a gate-PASS verify-closure report. Otherwise it is **PASS**. An improvement
     (p_improvement < 0.05) is reported. It is required only when the change is claimed to make
     Ready/Rolling drainage robust.
   - *Sensitivity at n = 13 per arm* [M, exact, `comb_r_test.py`]:
     - legacy at 7/13 (W96-like): a regression is declared for candidate <= 2/13, an improvement for
       >= 12/13;
     - legacy at 12/13 or 13/13 (W24- or W6-like): a regression is declared for candidate <= 7/13 or
       <= 9/13.

     Power [E, two independent binomials, exact enumeration]:

     | Legacy fraction | Candidate fraction | Test | Power at n = 13 | Power at n = 20 |
     |---:|---:|---|---:|---:|
     | 0.54 | 0.1 | regression | 0.72 | 0.90 |
     | 0.54 | 0.0 | regression | 0.98 | 1.00 |
     | 0.54 | 1.0 | improvement | 0.92 | 0.98 |
     | 0.94 | 0.5 | regression | 0.77 | 0.93 |
     | 1.0 | 0.7 | regression | 0.58 | 0.76 |

     Moderate shifts (0.54 to 0.3) stay undetectable at these n (power 0.21 and 0.32). Under the null,
     the family-wise false-block rate over three widths is at most 1 - 0.95^3 = 0.14 [E; Fisher is
     conservative]. The superseded n >= 5 could not fail at W96: 0/5 against 7/13 gives two-sided
     p = 0.10, one-sided 0.054.
   - *Known legacy non-drain signature (quantitative).* Let N993 and N1009 be the Apply natives of
     `0111110010` and `0111111001` at rank anchor + 1 at the stop, from `tools/records_breakdown.py`.
     The signature is known iff max(N993, N1009) >= 1,000. Anchor + 1 natives on other owners are
     allowed: `0111100100` (606-851 at a 60 k stop), `0111100000`, `0111110000`, `0111100110` and
     `0111111000`. Measured [M]:
     - drained runs: N993 = 30 (188 in one run) and N1009 = 0, over 19 breakdowns;
     - the eight 60 k-stopped non-drains: point-like, N993 12,457-13,755 with N1009 = 0; or wide boxes,
       N993 1,500-1,697 with N1009 2,813-3,050;
     - long runs: N993 up to 1.71 M.

     In the long runs the layer takes 72-78 % of native seconds (`four-all` W96 rep2 and rep5,
     `-r14anchors` W96 rep2, `-a19` Ordered W6). `-a19` Ready W6 had only 8.3 %, with the banana owner at
     88.7 %. So the signature is a count, not a share.
   - *Report per width:* n and drained per arm, the p-values, the natives distribution of the drained
     runs (min / median / max), the natives at stop with (N993, N1009) for each non-drain, the recorder
     fields and any slow-run flag (`tools/ready_stats.py`).
   - A run may be stopped early once it passes 60,000 natives, twice the Ordered reference. Ten
     non-drained runs of this lane ran past 60 s, and they had 64-261 k natives by then. No drained run
     of any `four-all` variant exceeded 38,173 natives [M].
   - Measured noise floor for drained Ready runs [M]: natives spread 6.5 % at W6 (n = 5), 9.6 % at W24
     (n = 10; 11.0 % for the 7 on socket 1) and 10.9 % at W96 (n = 7); Ordered spread 0.
   - Legacy baseline drain fractions [M] (context, not the comparator):
     - W6: 5/5 (95 % Clopper-Pearson interval 0.48-1). No W6 non-drain of `four-all` has been observed.
       All W6 runs ran on socket 0 (320-383 and 52-57).
     - W24: 10/10 on CPUs 320-383 and 52-63,308-319 (foreign load 6-24 % where recorded), plus
       7/8 on socket 1 (session C, CPUs 128-151, NUMA node 4; 52-74 % foreign load in 7 of the 8 runs).
       Together 17/18 (0.73-1).
     - W96 (CPUs 128-223, NUMA nodes 4-6): **7/13** (0.54; 0.25-0.81). By session:
       - session 1: 1/2 (`ready-w96` drained, `-rep2` flooded; foreign load not recorded);
       - session A: 2/3 (rep3 and rep4 drained, rep5 flooded; foreign load 27-45 %);
       - session B2: 4/8 (57-79 of the 96 CPUs busy with foreign work).

       No difference between sessions is detectable at this n (session A 2/3 against B2 4/8: Fisher
       exact p = 1.0). Session 1 has no load record, so no load effect can be read off either way.
   - The lower drain fraction at W96 is beyond the noise: W96 against all W24 (7/13 against
     17/18) gives Fisher exact p = 0.012.
   - What causes it is not resolved. Every W96 run was on socket 1 under foreign load. Session C
     ran W24 on socket 1 to separate width from host. It gave 7/8, and its one non-drain is the
     same rank-13 flood, so **W24 is not immune**. At this n, 7/8 differs neither from W24
     elsewhere (10/10, p = 0.44) nor from W96 (7/13, p = 0.17). The interleaved design above removes
     this confound from any candidate-versus-legacy comparison. It does not remove it from comparisons
     across widths.
   - Any other signature, a frontier or an audit violation is a failure in its own right.
3. **`four-all-physics`** (the planner rows alone, the helper ranks of the five-loop plan) is the
   small, campaign-shaped stress case. It floods on the same owner and is not a pass/fail control.
4. The mandatory recorder. The session-2 runner `TMP/c4l-s2/run_c4l.py` stamps it into every
   `metrics.json` from the two Ready W6 repeats on 52-57 (21:24Z) onward, including all
   session B2 and C runs:
   - foreign busy CPUs;
   - schedstat run delay summed over the native's threads;
   - user-mode instructions, cycles and IPC of the native process;
   - instructions per native.

   Session B2 at W96 [M]: the drained `four-all` runs show 11.3-13.2 M instructions per native, IPC
   1.34-1.60 and 57-79 foreign busy CPUs. The 60 k-stopped runs show 9.6-27.4 M instructions per
   native. On the same socket at W24 (session C), the drained runs show 9.5-10.4 M instructions
   per native and IPC 2.26-2.66. So W96 costs about 1.2x the user instructions per native, at
   about 0.6x the IPC.
   Timings stay load-sensitive and are labelled as such. Example, two Ready W6 runs on 52-57 [M]:
   9.38 M and 8.87 M instructions per native, IPC 1.82 and 2.05, run delay 9.8 s and 14.6 s. Total
   user instructions were 230.19 G and 230.40 G (0.09 % apart), although the natives differ by
   5.8 %: the Ready variation is in cheap Route natives.

   **Unexplained slow run: session C rep5** (`c4l-4a17f9c7-ready-w24s1-rep5/four-all`) [M].
   - It drained (24,288 natives, audit PASS) with 92.0 s of traversal, against 7.9-10.8 s for its six
     drained siblings.
   - Its user instructions and user cycles match its siblings': 229.85 G (9.46 M per native) and 86.4 G,
     against 228.8-231.7 G and 90.4-101.5 G.
   - Its schedstat on-CPU time was 275 s against 37-43 s, and its run delay 621 s against 51-82 s.
     Yet foreign load on its CPUs was 1 % (0.24 busy CPUs), against 12-18 busy CPUs for the siblings.
   - At any clock at or above the host's 401 MHz minimum, its user time is at most 215 s. So at least
     60 s of the on-CPU time was not user mode; at 3.1 GHz, about 250 s would not be.
   - [E] The cause cannot be separated from the fields that recorder captured: system time, a clock
     drop on lightly loaded cores, or both. cgroup CPU throttling is excluded (`cpu.max` = max for the
     user slice).

   Its counts stay valid; its timing is void. The committed runner `tools/run_c4l.py` therefore also
   records user and system seconds (getrusage), minor and major faults, voluntary and involuntary context
   switches, and the effective user clock (user cycles / user seconds). `tools/ready_stats.py` flags a
   drained run whose traversal exceeds 3x its group's median; rep5 is the only such run in this
   directory. Self-test of the extended recorder [M]: `four-all-p5` Ordered W6 on 52-57 recorded user
   34.2 s, system 3.3 s (8.8 % of own CPU), 3.03 GHz effective user clock and 36 % foreign load. The
   same control at Ordered W96 on socket 1 recorded user 119.0 s and system 67.8 s (36 % of own CPU),
   2.96 GHz, IPC 1.15, 12.8 M user instructions per native and 58 % foreign load. That is one run each,
   so the system share at W96 is an observation, not a measured law.

Anchors at rank >= 13 are **not** adopted. They fragment at Ready W96 as well: rank 13 in 2 of
11 runs, rank 14 in 1 of 3. They also raise the work (Ordered W24 33,750 and 38,173 natives
against 30,159).

### Owner decision item for the D-memo (D-C4L; not yet put to the owner)

This text is ready to paste into the D-session memo. The facts are [M] from this section; the
defaults are the lane's [E] recommendation.
1. **Scope substitution.** The owner asked for one combined run over all four-loop owners. The
   combined control (`four-all`) is the planner physics class at L = 4 (`--difference-set 7,8`: 26
   roots, 16 helpers) plus 16 rank-12 owner orthants. It is **not** the owner's historical saved-cover
   envelope A <= 19 / R <= 12 / D >= 7. That envelope in combined form (`four-all-a19`) drained in 3 of 7
   runs and floods at rank 13 **even under Ordered** at W6 and W24, so it cannot serve as an identity
   oracle. The combined control also differs from the five-loop campaign in two ways:
   - its 16 anchors have no campaign counterpart;
   - its helpers carry no A bound, while 54 of the campaign's 67 helpers do.

   In v2 generations 3-7, 66.6 % of the Apply natives lie at or below their owner's helper rank (59.3 %
   for the hot owner) [M, research note, Consequences 3]. The note reads these [E] as mostly escapes in
   A, and `four-all` does not exercise that part.

   Options: (a) keep `four-all` as the combined control, next to the unchanged per-family C-4L
   [default]; (b) also build an A-bounded variant as a stress arm (not built); (c) use the envelope
   variant (not usable as an identity oracle).
2. **Generation policy.** The owners use the Vakint package policy (sparse, numerical depth 2,
   finite-case search, no rank scope). The five-loop owners were generated with `--max-numerator-rank
   10 --exact-backend sparse-factorized --numerical-depth 0`. The five-loop-policy variant `four-all-p5`
   (596/797 finite residuals against 386/445) drains the same way:
   - Ordered: 31,717 natives (+5.2 %), strictly identical records at W6, W24 and W96;
   - `walk-verify-closure` PASS (89558210, all roots independently verified);
   - Ready: W24 2/2 and W96 2/3, the third with the known signature.

   Options: (a) keep the Vakint-policy `four-all` as the canonical reference [default: it has the
   larger baseline (29 drained Ready runs), identity at three widths and the gate-PASS re-verification];
   (b) switch the canonical reference to `four-all-p5` (closer to the campaign's program generation;
   new reference of 31,717 natives and 68,483 records). A C-4L-comb-R comparison is interleaved per
   session, so either choice only changes which argv template both arms use.

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
a75571247374e98e9bad693665c75354b3e1a2e0fff4dfce4c3a27495b7106ae  ./four-all/input-receipt.json
7977ec23f124ddb0762eb05191d1c6c0de0229a08c0306b8bf376dbdf4187488  ./four-all/matching-summary.json
d1ac816eb5ad5473e05ba5cc618d9f9273fd05dd31dff0c90b05704b1a072e81  ./four-all/queries.json
f8511200625e1c94bcf3ea8038cf86b40dc1b0f12ef0502b1018b422c655650e  ./tools/run_c4l.py
0923628aee8fc9bcf1d0d0396b4e1ed2da60cf21a549980f0422b12bde493dd3  ./tools/session.py
53d777d66cb9ed4cd4c26823948b3202bd550fc80f0e8b4a099271a5ceebb2ec  ./tools/ready_stats.py
5ebbb7e78918e41a689f289a30e9d1621ba8318aeaef99d972504c3322f27dd2  ./tools/comb_r_test.py
cfdf07b6723a55a4b2ed3a8d875f1bda658c9d64445e6cd34ebd2458094ed19e  ./tools/command-four-all.json

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
8dace33c1bdd5e9dce251354c3743bef29e7625ee4b6b153847b0e3130de9051  /common/dev/rustred/TMP/fable51-controls/run_control.8dace33c.py
# session 2 evidence (absolute)
# runner and session driver as used by session A (saved copies)
104453012643e0919495417f66ba768593911a348e722d2530c287b3420d9fd7  /common/dev/rustred/TMP/c4l-s2/run_c4l.v1.py
da133bd0dfe0e126f80981c467b15b390973464bda960532d4d89904fe20bdb5  /common/dev/rustred/TMP/c4l-s2/session.v1.py
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
# session B2 (runner with recorder and --stop-natives, session driver with budget / yield; statistics)
42a30903a67b7f6b0b52f66e07c2f5cf731bd4456d0ffdcae5777d21c7cfd54e  /common/dev/rustred/TMP/c4l-s2/run_c4l.py
451149b5835adedd72a8f59b21170df0f0cd2cef2271d0d49f54b21f8b63e29f  /common/dev/rustred/TMP/c4l-s2/session.py
ce2fb5ea7d66e7e2f2345fab57c267a31b4619f721ed2a7ef94f68f265d90f4d  /common/dev/rustred/TMP/c4l-s2/queueB2.json
c0668ce543be4a91164ae7e27dc1e9c6b4af5fcca55ea682573d5595878f4654  /common/dev/rustred/TMP/c4l-s2/sessionB2.log
3cd4e26c6735b1a4a2b63b066d5fa00be8b8f3ec5e212741cbb365fe18b58eb0  /common/dev/rustred/TMP/c4l-s2/ready_stats.py
00149b1468fc01d747ff879e951cb911cc259533d2e94d3c8caf1dd3250698e1  /common/dev/rustred/TMP/c4l-s2/ready_stats.json
cdaa5fd39c5ba3781d515cabe875128e4c39979a504ac5315621e0438c4d881a  /common/dev/rustred/TMP/c4l-s2/post_b2.py
f99e9f403b209c32ba3ba79447681c860e1afb7be19895037b7d157c86768303  /common/dev/rustred/TMP/c4l-s2/post_b2.json
6dc3c6f1baee539e9c2c4d0ac553dab103c37f580c415a86e4a33b9d530a32bf  /common/dev/rustred/TMP/c4l-s2/bd-ready-w96-rep6-four-all.json
9e72790a2956a2e5ee1800ba6c7d3456459591dbcf69fbb42fc618d98db4d9c3  /common/dev/rustred/TMP/c4l-s2/bd-ready-w96-rep7-four-all.json
b005b54704490ddfb0ffe76dfb5f75c5b63b37d01654b8091282097ed7db341f  /common/dev/rustred/TMP/c4l-s2/bd-ready-w96-rep8-four-all.json
155f09f96369ce712d92643900e970af2f99ef7d98df572cd5a2a43d364d8c06  /common/dev/rustred/TMP/c4l-s2/bd-ready-w96-rep10-four-all.json
c5ea4c26587ce43c314b9868ed3b81107af8240211292942de75adc29eafb0f8  /common/dev/rustred/TMP/c4l-s2/bd-ready-w96-four-all-p5.json
a871c797d435d83c50db5063119210f9a76480075e9695eaa7701574d03057bd  /common/dev/rustred/TMP/c4l-s2/bd-ready-w96-rep5-four-all-r13anchors.json
ebf56a81dc4b34e8f7966b41c33d101b0a145ae964a713a0948611ccdd218ef3  /common/dev/rustred/TMP/c4l-s2/bd-ready-w96-rep7-four-all-r13anchors.json
# session C (four-all Ready W24 on socket 1)
f46c9cf45d203d402947b7a5ba6e6b85bf37e8a04940e1e76b36855e94464ed0  /common/dev/rustred/TMP/c4l-s2/queueC.json
de7d82e66e20f994c56da319168376ada708b36b080d838d33abc56e118da62f  /common/dev/rustred/TMP/c4l-s2/sessionC.log
68baef60b73752e1b45be76be36efed1027c6959e13bd786569511727c5d3530  /common/dev/rustred/TMP/c4l-s2/post_c.json
a9d56d37504d33f20d8a5c31c5bb9f8f85a3807dd10a220fbab4ad9f5363a90b  /common/dev/rustred/TMP/c4l-s2/bd-ready-w24s1-rep8-four-all.json
# fix round 2026-09-28 (TMP/c4l-s2/fix; shared runner after the comment fix; oracle verifier; p5 Ordered W6; rep5; drained breakdowns)
4d10407c78742107c8c7568f5b6fec5ae32120eb6ee7468b5fc1bdcceed857ff  /common/dev/rustred/TMP/fable51-controls/run_control.py
46d4dd286df25fd47ef1ed701aa515ffed26c493aad45f816e947a40f0c6eca7  /common/dev/rustred/TMP/w0/oracle/bin/rustred-46d4dd28
cefdbb435a901b4a471b7a6c05846f89318d5426c34c2157ee602d499c0ff7fe  /common/dev/rustred/TMP/c4l-s2/fix/verify-46d4dd28-ordered-w24_four-all.json
ffbc8aa8039dd74a713e08e0b19004b71f9471f10c787ec04f2f114d4f09656f  /common/dev/rustred/TMP/c4l-s2/fix/compare-p5-ordered-w24own-vs-w6own-strict.json
dc8eae474cce8cb9fb51154c220417a7448519338774ff6fcc2edd75111dabd1  /common/dev/rustred/TMP/fable51-controls/c4l-4a17f9c7-ordered-w6own/four-all-p5/metrics.json
714c6406f2d660bcc1877ca3aea7e853974638a07e9b4d75039c82aa84780aba  /common/dev/rustred/TMP/fable51-controls/c4l-4a17f9c7-ordered-w6own/four-all-p5/audit.json
657ffa3b7fee87d69267880d1ab87ecbf6ea804ffc3652b55dac0179c5aa3d89  /common/dev/rustred/TMP/fable51-controls/c4l-4a17f9c7-ready-w24s1-rep5/four-all/metrics.json
8ec36b51014d019bf964145d5e8124e08ce03691d5bed64510ac7dedfdfad3c3  /common/dev/rustred/TMP/c4l-s2/fix/bd_drained.sh
98b17b8a93ef69556491312cc948db9df70982377867528733bfe46f641f3ddb  /common/dev/rustred/TMP/c4l-s2/fix/ready_stats.json
69dbdc862debdd34d7135afdac11ba0bd44342f32a0797d7d63128777c8c66d6  /common/dev/rustred/TMP/c4l-s2/fix/ready_stats.md
fa6083549f1b51b509c1ea89c86a414269110003c6f977a773e084814743e5cc  /common/dev/rustred/TMP/c4l-s2/fix/queueF.json
ee78d96697868fe8cfa319e4e0947d9687cd3c863441cee5c0356f63cd37f861  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w24s1-rep1-four-all.json
83c0724646fc066be9ebda2c9794b00de3c8746c1590c3390a9838a132c6cb10  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w24s1-rep2-four-all.json
43bd28d50254706870e4472de05bc746159fd5829b5ea0402ddd80062dee36fe  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w24s1-rep3-four-all.json
c735854682209dca177abfcae7274eb4f20cec7fff851e0eb421f6ad87856438  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w24s1-rep4-four-all.json
187aee324c248aaceae45ed62b0959e42923f00599284028e2fcdaa25ff24487  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w24s1-rep5-four-all.json
0e00fda825997ba8619f677d1bbed6fed0331ce6549da05b4aa2d758c133d5d1  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w24s1-rep6-four-all.json
6ffad04927179adacbd17948f25ebc6a37c0d927cf4e0cb58880dc1a6255348d  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w24s1-rep7-four-all.json
3a1f0b2b238a2614ee05d7f009f205cabc395b0aecb01c13a7d8463d80d2a63a  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w96-rep11-four-all-r13anchors.json
3e9b5dd5c633799f6c19c4e94daf378d2599e73fc366be66667faae60aa49d9e  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w96-rep11-four-all.json
7295f2afe8b8244932f1d2e369edd4d5aa93bef9642e1adf4b89efdd2b59f75c  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w96-rep12-four-all.json
1bdf13e690f7dda315ab061b1d8829b42706fca784fa181dfd0ced888d338bc0  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w96-rep13-four-all.json
5ec97c672845045cd3021e00d468944436f846daeabf1b6f364b2123ddacb9bb  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w96-rep2-four-all-p5.json
d8000db2a7aaf792feab1e4e9acab18178925b9752e1a7b39cb434805f14495e  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w96-rep3-four-all-p5.json
a306394e5a8f047e0efae5da8058c9fcb1a531e30950504e3164175bb7d888b1  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w96-rep9-four-all-r13anchors.json
4c02bbafa6f5a4f98b04ac98e6b68afb04c694ac511c6a0d63d84db24b00f708  /common/dev/rustred/TMP/c4l-s2/fix/bd-ready-w96-rep9-four-all.json
e76684edb95dcbcbba403b5e04cfc862d82d1c232690db38b8871c7ecf17e7b8  /common/dev/rustred/TMP/c4l-s2/fix/compare-p5-ordered-w24own-vs-w96-strict.json
d48bd34163e561f9deea5cca67f30ed5b8f481b0f6444f2ec1ede2af2e075bf9  /common/dev/rustred/TMP/fable51-controls/c4l-4a17f9c7-ordered-w96/four-all-p5/metrics.json
c109bb42b417f73f85a1cd9cbd21ffb0299d95374d51ff74311363f1d0b5c3e0  /common/dev/rustred/TMP/fable51-controls/c4l-4a17f9c7-ordered-w96/four-all-p5/audit.json
c419d78602db9412e3cf362d663a2507881788687a27ff97aeb6a19eb4ac51a7  /common/dev/rustred/TMP/c4l-s2/fix/sessionF.log
```
