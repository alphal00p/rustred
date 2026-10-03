# Second A1 five-loop frontier: a diagonal exceptional slice

The older `five-loop-a1-epoch-frontier-repaired-20261001` campaign stopped
with one saved `NativeFrontier`, zero native errors and zero abandoned
obligations. CP6 generation 38 records 222,194,016 completed native inspections,
260,112,277 committed domains and 6,985,518 pending domains. The newer
`five-loop-a1-new-rules-20261002` campaign is a separate live run. This
investigation neither resumed nor modified either production campaign.

## Exact saved failure

Node 259,802,965 is an Apply obligation for owner `011001110110100`. Its
retained frontier is `ExactGap` at the exact local-coordinate point

```text
x = [5,0,0,0,0,0,0,0,5,0,0,0,0,1,0].
```

Using `n_i=1+x_i` on active axes and `n_i=-x_i` on inactive axes gives the
zero-based physical indices

```text
n = [-5,1,1,0,0,1,1,1,-5,1,1,0,1,-1,0].
```

The numerator rank is 11, positive-power sum 8 and power difference −3.
The stored parent domain varies `x8=2..5` and `x14=0..3`, with the other
coordinates fixed as above. Its exact power band forces `x8+x14=5`, so it
contains four feasible lattice points, not all sixteen points of the box.
The final point is the retained frontier.

The native record reports no error, optional refusal or truncated refusal
inventory. Its inspection lasted 0.005021868 s. `ExactGap` means the installed
owner exhausted its saved rules and terminals; it is distinct from an
undecided predicate or a resource stop. It does not prove that a required
physical input reaches this integral with nonzero coefficient.

The owner was generated from parent 31740, native ordinal 2, native sector 5862,
published sector 13236, representative 30222. Its unchanged bundle is
`inputs/owners/0005-011001110110100.rrbin` (9,898,171 bytes; SHA256
`12a76130a69306f7a630b3c16327ffb8365a3bcf746b8546fdf02562f3d472e2`).
The generation configuration uses `max_numerator_rank=10` and finite-case
policy `search`. This is a different owner from the first repaired frontier.

## Independent local reproduction and neighborhood

The cached optimized CLI with SHA256
`8ef80b52da13866dbe09d817acd004ec3fc9ca2130d85411ac758398086cf0a7`
loaded only this owner, without routes or successor traversal. It reproduced
one exact gap, zero selected rules, zero terminals, zero unresolved predicates
and no error. Classification completed in 2.358 s inclusive, with 1.069 s native
preparation and 0.000955 s matching. Sampled process-tree peak was 124.2 MB.

A 79-query screen then varied `x0,x8=3..7` and `x13=0..2`, preserving every
other coordinate, and separately checked the four feasible parent points.
Each query used its actual numerator rank and exact power difference. It
completed in 2.546 s inclusive with 73 rule selections, two terminals, four
gap query rows and no unresolved predicate or error. One gap row duplicates
the original point in the parent census.

| Tested slice | Saved classification |
| --- | --- |
| `x13=0` | Rule 282 throughout the sampled 5×5 grid |
| `x13=2` | Rule 274 throughout the sampled 5×5 grid |
| `x13=1`, `x0!=x8` | Rule 274 |
| `x13=1`, `x0=x8=3,4` | Existing explicit terminals, ranks 7 and 9 |
| `x13=1`, `x0=x8=5,6,7` | Exact gaps, ranks 11, 13 and 15 |
| Parent `(x8,x14)=(2,3),(3,2),(4,1)` | Rule 115 |
| Parent `(x8,x14)=(5,0)` | Original exact gap |

These observations support rank-limited exceptional-case coverage as the
origin of the gap. They do not establish that all higher-rank points are
uncovered or that an entry-rank restriction may be reimposed on descendants.
Required-versus-auxiliary ancestry remains unknown; routing overcoverage has
not been ruled out.

## Exact neighboring guard

The native guarded-rule diagnostic inspected rules 115, 274 and 282 at the
frontier in 2.513 s inclusive. Rule 115 excludes `n14=0`; rule 282 does not have
the required fixed face. Rule 274 excludes the conjunction `F=0` and
`n0-n8=0`, where

```text
F = -2-3*n13-n13^2-7*n8-n8*n13-3*n8^2+6*n0+3*n0*n8.
Q = -F+3*d*(n0-n8).
```

Its displayed original denominators are `Q` and `2*Q`. Symbolica 1.5.1
independently verifies

```text
F | n0=n8 = -(n13+1)*(n13+n8+2)
F | n0=n8,n13=-1 = 0
Q | n0=n8,n13=-1 = 0.
```

Thus this recurrence is singular on the entire diagonal slice
`n0=n8=-t,n13=-1`, with all other indices fixed as above. Dividing by `Q`
there cannot repair the gap. This guard identity concerns rule 274; proving
coverage or noncoverage of the whole installed owner needs a separate native
classification. The second factor also vanishes when `n13+n8=-2`; that
condition alone does not establish another uncovered slice.

## Source search and current limits

A depth-two search against the owner's original source system found one
relation at the exact rank-eleven point: 230 RHS terms, 317 retained source-trace
rows and no residuals. Independent source identity/guard replay passed; the
installed point selected the new rule and all 47 existing terminals remained
unchanged. The guarded run took 3.341 s inclusive. This finite relation is an
existence diagnostic, not the desired unbounded repair.

The next native search freed both `n0` and `n8`, retaining all other physical
indices and specifying no rank cutoff. It found 11 rules and zero finite
residuals; independent source identity/guard replay passed. The 827,554-byte
partial-rule payload was exported in the ignored evidence directory. This
run took 14.442 s inclusive, with 3.633 s source search.

The installed inspection of the full two-dimensional rectangular input hull
retained 18 selected-rule pieces, four existing terminals and one unresolved
piece. Therefore this result does **not** yet establish unbounded local
coverage. It also does not prove failure of the new diagonal relation:
classification of a rectangular hull can remain undecided across a diagonal
guard. No continuum, recursive-successor closure or five-loop closure claim
follows from the source-search exit status.

### Cold validation and finite applicability

A separate process cold-loaded the 827,554-byte payload, regenerated its
original source identities, replayed all 11 rules and verified strict descent.
It completed in 8.335 s inclusive. The payload's SHA256 is
`7e1f93615282130555f29a885d1bbfe7b5b0c08074d7c2d1e542378b4457a86a`.
All 47 old terminals remain unchanged. The cold inspection retains the same
one unresolved rectangular-hull piece; cold validation does not remove that
coverage limitation.

The current cached production CLI then cold-loaded the same sidecar and
classified 84 exact queries: the original point, all 79 earlier query rows
(including the four correlated parent points), and held-out diagonal points
at `t=10,20,50,100`. Their actual numerator ranks are 21, 41, 101 and 201,
with positive-power sum 8 and exact difference `7-2*t`.

All 84 queries classified completely and were locally applicable: 82 selected
rules, two existing terminals, zero exact gaps, zero unresolved predicates and
no error. The original point and every tested high-rank diagonal point select
overlay batch 1, rule 411. Comparing the 79 repeated queries with their old
results, only the four gap query rows change, all to rule 411; every previously
selected rule and terminal is unchanged. Inclusive time was 9.133 s, comprising
7.697 s native preparation and 0.158 s matching; sampled peak was 180.9 MB.

A separate native check retained the original parent **box** and its exact
rank/power band. It split into two selected pieces: rule 115 covers the
correlated three-point remainder, and rule 411 covers the missing endpoint.
The separately submitted original singleton also selects rule 411. Both queries
completed with no gap, unresolved predicate or error in 8.353 s inclusive
(7.359 s preparation, 0.002781 s matching, 179.3 MB sampled peak).

These checks establish finite local applicability and exercise the original
domain-splitting path with the persisted repair. They do not establish
unbounded-hull applicability, actual required-query ancestry, or closure of
the repair's RHS obligations. No successor traversal was requested.

### Reproduce the source search and cold load

The existing generic `probe_owner_case` example accepts the owner and case as
inputs; this investigation introduced no owner-specific engine code. Its
frozen executable SHA256 is
`bc6e2301d4864c91f3c240fffc5b398addb7cd53096dab5420b217f7ec343fcb`.
The selection SHA256 is
`854624bbae2c6068b196f5cb56b7fa11b4705008ce4db7e862cc43f2ccd640b0`.

These are the native invocations used by `run_source_probe.py` and its saved
plans, with a fresh output directory substituted. They ran inside the existing
owned-process controller on CPU 32, one thread per inner pool, with the memory
and deadline settings recorded below. Keep the established native environment
and Symbolica license. The two `*` entries are free physical indices, and
`--search-rank none` explicitly removes a numerator-rank search cutoff.

```bash
repair_probe=/common/dev/rustred/TMP/releases/20261001-frontier-repair/bin/probe_owner_case
repair_selection=/common/dev/rustred/TMP/frontier-repair-20261003/singleton-selection.json
repair_owner_base=/common/dev/rustred/campaigns/five-loop-a1-epoch-frontier-repaired-20261001/inputs
repair_output=$(mktemp -d /common/dev/rustred/TMP/frontier-repair-20261003/reproduction.XXXXXX)

"$repair_probe" \
  --selection "$repair_selection" --owner-base "$repair_owner_base" \
  --owner 011001110110100 \
  --powers '*,1,1,0,0,1,1,1,*,1,1,0,1,-1,0' \
  --numerical-depth 2 --search-rank none --replay --inspect-installed \
  --output-overlay "$repair_output/repair.rrbin"

"$repair_probe" \
  --selection "$repair_selection" --owner-base "$repair_owner_base" \
  --owner 011001110110100 \
  --powers '*,1,1,0,0,1,1,1,*,1,1,0,1,-1,0' \
  --numerical-depth 2 --search-rank none --replay --inspect-installed \
  --load-overlay "$repair_output/repair.rrbin"
```

The first invocation derives and exports rules from original source equations;
the second performs no new source search and validates the persisted payload
in a fresh process. A successful exit alone is not the applicability gate:
inspect source replay, strict descent, finite residual counts, installed
classification and the unchanged terminal inventory separately. Here cold
load passed in 8.335 s and all 84 finite checks passed, while full-face
applicability and recursive closure remain unproved.

## Evidence and reproducibility

Ignored local evidence is under `TMP/frontier-repair-20261003/`:
`checkpoint-diagnosis.json`, `singleton/`, `neighborhood/`, `guarded/`,
`source-probe/point-depth2/`, `source-probe/two-free-depth2/`,
`source-probe/two-free-cold/`, `candidate-finite/` and `candidate-parent/`.
Scripts and command/input receipts accompany those outputs. All classification
and guarded diagnostic process groups drained; each used one CPU, a 150 GB
process memory ceiling and a 150 GB host reserve. No owner generation or production
checkpoint restore was used for those diagnostics.

The bounded checkpoint extraction read the 36-byte frontier inventory, a
positioned 97-byte geometry record at byte 25,200,887,633, and the final 64 KiB of
record segment 38. The 1237-byte frontier frame starts at byte 2,177,180,124;
eleven following frames tile the segment's EOF. Its diagnostic body was
decoded in source-defined bincode order with all 1128 bytes consumed. Frame
SHA256 is `8e4d50e5692de8e16ec0385462cfca6ca0dbcac9ab054b56366ba016fae6d7e9`.
Complete large checkpoint hashes were not reread; this extraction is diagnosis,
not full checkpoint authentication or a mathematical certificate.
