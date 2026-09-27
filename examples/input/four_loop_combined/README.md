# Combined four-loop owner run (common Luthe A4 basis)

One combined owner-domain walk over **all four-loop owners** in a single common
momentum basis, built to resemble the five-loop campaign as closely as possible:
one family, class owners generated in that family, every labelled sector routed
to its class owner, and physics-scoped entry queries from the generic planner.
It is proposed to replace the four separate per-family controls FG/BMW/H/X (four bases, 900
installed owners, zero Route records) as the four-loop engine control.

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
| `four-all/` | **The engine control** (`run_control.py --family four-all`): `queries.json` = the 42 physics rows followed by 16 rank-12 owner orthants (58 rows), its `matching-summary.json` and staging `input-receipt.json`. |
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

## 4. Owner payloads (not committed)

The 16 single-sector programs total 6,562,373 bytes. They live in
`TMP/c4l-build.wBU9zC/staged-*/owners/` and are hard links of
`TMP/c4l-build.wBU9zC/gen/root{1022,511}/sectors/sector-<ordinal>.rrbin`.
`selection.json` records each owner's SHA-256, byte size, root and native
ordinal. Regenerate them with step 1 above; the per-sector ordinals come from
`checkpoint.toml`. Digests of the two checkpoint manifests:
- root 1022: `47d46b3ed46de97c87338ef32f9f4d372bbec0125e29a6533b74fe9817610882`
- root 511: `9239fb06edbb6d501cf154b19fe6bede7d171b74acf2d3358fc7d663d0ac6952`

## 5. Walks

All walks use the release binary 4a17f9c7 through
`TMP/fable51-controls/run_control.py`. The runner calls `owner-domain-match
--follow-successors` with the native options of the five-loop campaign:
`--route-domain-overcover --transfer-unreserved-lookahead 256
--reuse-initial-d-bands --bounded-refinement-axes finite-axes
--max-guard-univariate-degree 64 --unbounded-work`, checkpoint enabled. The
audit is `examples/python/audit_owner_domain_walk.py`.

### Registered families

| Family | Queries | Content |
|---|---:|---|
| `four-all` | 58 | The 42 planner physics rows plus 16 appended rank-12 owner orthants (`stage_saved_owner_campaign.py --anchor-max-numerator-rank 12`; 12 = 3L, the rank of the historical rank-only anchors). This is **the combined engine control**. |
| `four-all-physics` | 42 | The planner physics class alone (`physics/queries.json`). |
| `four-all-a19` | 32 | The historical envelope (`envelope-a19r12d7/queries.json`, boxes before orthants). |
| `four-all-r12anchors` | 58 | The same query bytes as `four-all`; the label was used before the rename. |
| `four-all-r6anchors` | 58 | Diagnostic: physics plus rank-6 owner orthants. |

To run the control:

```
nix develop --command python TMP/fable51-controls/run_control.py \
  --binary /common/dev/rustred/TMP/fable51-controls/bin/rustred-4a17f9c7 \
  --family four-all --label <label> --cpus 320-343 --policy ready --workers 24 --timeout-seconds 3600
```

The native argv is `TMP/c4l-build.wBU9zC/commands/command-four-all.json`; the
runner rewrites output paths, policy, workers and CPUs. `--timeout-seconds`
writes the native stop file (cooperative stop, which writes `result.json` and a
checkpoint).

### Results [M]

Shared host, CPUs 320-383 (SMT siblings of 64-127). Timings are single
observations, not a benchmark.

| Family | Policy, W | Runs | Natives | Logical records | Traversal (s) | Whole command (s) | Audit |
|---|---|---:|---:|---:|---:|---:|---|
| `four-all` | Ready, W24 | 5 | 22,290-24,425 | 58,209-63,513 | 6.96-9.68 | 10.0-13.5 | PASS x5 |
| `four-all` | Ready, W6 | 3 | 24,482-26,065 | 65,420-65,524 | 10.3-13.4 | 13.0-17.0 | PASS x3 |
| `four-all` | Ordered, W6 | 2 | 30,159 (both) | 65,444 | 13.7-17.0 | 16.5-20.5 | PASS x2; strict record comparison PASS |
| `four-all` | Ordered, W24 | 1 | 30,159 | 65,444 | 16.2 | 19.5 | PASS; records strictly identical to Ordered W6 |
| `four-all` | Ready, W96 (CPUs 128-223) | queued | - | - | - | - | pending the socket-1 lock (`TMP/c4l-build.wBU9zC/socket1_session.sh`) |
| `four-all-r6anchors` | Ready, W24 | 3 | 12,527-13,370 | 43,190 (first) | 4.36-4.50 | 7.5 (first) | PASS x3 |
| `four-all-a19` | Ready, W24 | 3 | 22,846 / 22,847 in 2 runs; 3rd run >= 1,150,778 at 600 s | 60,721 / 60,498 | 7.32 / 8.98 | 10.0 / 12.5 | PASS x2; 3rd run not drained |
| `four-all-a19` | Ready, W6 | 1 | 2,660,400 at 2,539 s, stopped | - | - | 2,541 | not drained (audit FAIL: not exhausted) |
| `four-all-a19` | Ordered, W6 | 1 | 3,168,826 at 3,140 s, stopped | - | - | 3,142 | not drained (audit FAIL: not exhausted) |
| `four-all-physics` | Ready, W24 | 1 | 2,367,249 at 3,600 s | 2,532,442 committed | - | 3,601 | not drained; no `result.json` (see below) |

Details of the drained `four-all` runs:
- Zero frontiers, errors and violations; every ledger obligation discharged;
  maximum scheduled finite rank 14.
- 98-99 % of natives are Route inspections: 23,746 Route and 245 Apply in
  the canonical W24 run.
- In that run, Apply carries 28.7 of the 31.3 native seconds. The non-entry
  banana owner `0111100001` alone takes 23.6 s over 20 natives.
- Peak process RSS from the heartbeats is 0.34 GB at W24 and 0.16-0.18 GB at W6.

`four-all-physics` (the planner rows alone) **does not drain within 1 h** at
Ready W24.
- At 3,600 s: 2,367,249 natives and 2,532,735 discovered domains; 11 of the 16
  initial roots were closed and 1.40 M domains were unresolved. Zero
  frontiers, RSS 1.36 GB. The coordinator spent 2,098 s in admission
  preparation and 1,156 s in ordered commit.
- The committed-records sidecar shows where the time goes: 96.3 % of native
  seconds are on the two connected owners with V4min = 2, whose helper rank is
  3.
  - `0111110010` (class 993): 74.0 %, 1,177,608 natives.
  - `0111111001` (class 1009): 22.3 %, 160,274 natives.
  - Nearly all of these natives are at rank 5, i.e. descendants arriving above
    the owner's helper rank, so no admitted orthant covers them.
- Appending rank-6 orthants removes the fragmentation: 13 k natives, 4.4 s.
  This is the same mechanism as the per-family "without anchors" failure
  (`docs/research/four_loop_saved_cover_control_2026-09-24.md`).
- [E] It is the four-loop image of the five-loop hot owner `011101110111000`
  (connected, V4min 3, helper rank 3). In the first 10 h of the live run
  described in `docs/research/fable51_design_inputs_pilots_launch_2026-09-26.md`
  §0, that owner took 58.5 % of heavy-head wall. The analogy is an
  interpretation, not a measurement at five loops.
- This run was stopped by SIGINT from an earlier version of the runner, so no
  `result.json` exists. The heartbeat stream and the 2,532,494-record
  checkpoint sidecar are kept in
  `TMP/fable51-controls/c4l-4a17f9c7-ready-w24/four-all-physics/`. The
  runner now stops runs through the stop file.

`four-all-a19` (boxes before orthants) is **schedule-sensitive** on identical
input bytes.
- Two of three Ready W24 runs drain in 9 s.
- The third run, and both W6 runs, exceed 1.1-3.2 M natives with only 20 of
  32 initial records closed.
- [E] In this order, every rank-12 orthant overlaps an earlier non-orthant
  box, so it is inspected as a partial initial overlap
  (`pinned_anchor_plus_native_residual`, 31 partial inspections). Whether its
  coverage is in place before descendants are admitted then depends on timing.
- Orders in which each owner's first row is a full orthant drained in all 13
  runs (`four-all`, `four-all-r6anchors`). This is the planner's helper-first
  order: later boxes of the same owner alias into the orthant, and appended
  orthants overlap only orthants.

Comparison with the four separate per-family controls (W6, Ordered, 32fdec
baseline: FG/BMW/H/X, 317,608 natives in total, 1,800 queries on 900
installed owners, zero Route records). The combined `four-all` at W6 needs
24-30 k natives. This is **not** a speed ratio: the owners (16 against 900),
the programs (fresh A4 against Vakint) and the queries differ.

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
```
