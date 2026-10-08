# Scalar input stages and checkpoint-preserving continuation

`examples/python/prepare_rank_campaign.py` prepares a **new** campaign using
existing saved rules. It neither generates IBPs nor starts a solver.

```bash
cd /common/dev/rustred
nix develop /common/dev/rustred --command python -B \
  examples/python/prepare_rank_campaign.py \
  --source-campaign campaigns/five-loop-a1-banana485-20261004 \
  --destination campaigns/my-five-loop-r0-d9 \
  --max-numerator-rank 0 --max-power-difference 9
```

The destination must not exist. The original campaign and its checkpoints are
not modified. Ordinary `nix develop /common/dev/rustred` uses the tracked flake;
do not use `path:/common/dev/rustred`, which copies ignored campaign data too.

## What rank zero means

For physical propagator/scalar-product indices `n_i`, the native definitions
are `A = sum(max(n_i, 0))`, `R = sum(max(-n_i, 0))`, and
`D = sum(n_i) = A - R`. Numerator rank R counts scalar-product powers, not
their Lorentz degree `2R`. For example `(2,1,0,-1)` has A=3, R=1, D=2.
The mass dimension of an L-loop integral in spacetime dimension d is
`L*d - 2*D`; uppercase D is not spacetime dimension d.

Each original required starting query is intersected with the requested R
and optional D caps. Its coordinate bounds, maximum A, minimum D and any
tighter original maximum D remain unchanged. At R=0, D=A: R=0,D≤9 therefore
means scalar starts with total positive power at most nine, subject to all
original restrictions. It is not a request for arbitrary positive powers.

Only a proven disjoint D interval (`original min D > capped max D`) is
omitted: the native interval format rejects min>max. Its exact query ID is
recorded in `omitted_disjoint_power_difference_query_ids`; this is an empty
intersection, not an auxiliary-role demotion. Other empty geometry stays in
the input for native admission. The full original source document is retained
as `rank-source-queries.json`, including omitted rows. In the current five-loop
source, D≤9 selects 67 of 116 required rows and omits the 49 fixed-D=10 rows;
raising D to ten restores those rows.
For this initial R=0,D≤9 stage, all retained rows have D=A=9. Of the 67
retained rows, 26 are empty because their sectors require 10–12 active
propagators, each with positive power. The other 41 have nonempty scalar
domains (637 starting integer tuples in this particular input). These counts
do not bound the descendant worklist and are not a closure claim. Raising
R at fixed D allows larger A=D+R; raising D at fixed R=0 admits scalar inputs
with more positive propagator power. In particular, this first stage is not
the set of every undotted five-loop top-level scalar integral.

By default auxiliary starting requests are omitted; `--include-auxiliary`
retains them with the same rank cap. All saved owner rules, routes and repair
overlays are retained in either mode. In particular, omitting auxiliary
starting requests does not remove those owners' usable reduction rules.
Some auxiliary requests have unbounded positive powers even at rank zero.

The rank limit applies only to starting inputs. Successors required to reduce
them are not clipped: reduction may encounter domains with higher or genuinely
unbounded numerator rank. The dashboard reports these encountered bounds.
The same rule applies to D: descendants are not restricted by the starting
D cap.

The preparation receipt records the original and restricted query hashes,
retained/removed IDs, and changed caps. It makes no closure claim. Closure
requires discharging the resulting reachable obligations in the native engine.

## Freeze and launch

Build the native CLI once, then use the normal production launcher to freeze
the executable and resource policy into the new campaign. For example:

```bash
nix develop /common/dev/rustred --command \
  cargo build --release --locked -p rustred-app --bin rustred
nix develop /common/dev/rustred --command python -B \
  examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/my-five-loop-r0-d9 \
  --executable target/release/rustred \
  --workers 32 --cpus 64-95 \
  --max-memory-bytes 600000000000 \
  --host-memory-reserve-bytes 150000000000 \
  --publication-policy epoch --epoch-inspector-lookup snapshot \
  --epoch-rolling --epoch-dispatch fifo \
  --epoch-publication-order oldest-prefix \
  --epoch-cut-size 16 --epoch-window 76 \
  --inspection-workers 31 --g2-residual-anchors union \
  --transfer-unreserved-lookahead 256 \
  --checkpoint-interval-seconds 3600 \
  --frontier-policy stop --no-auto-rescue
```

The second command prepares and displays the native invocation but does not
launch it. Start manually with the same launcher and campaign directory plus
`--start`. After a clean interruption, use `--resume --start` on that same
campaign. Ctrl+C requests an orderly checkpoint save; wait for its completion.
The memory guard may lower the admitted memory budget to preserve host headroom.

## Continue with the actual saved results

Once a CP6 checkpoint has completed original input admission, it can be
extended whether the current stage is complete or merely cleanly paused.
Stop the supervisor cooperatively and wait for native save/exit; never edit
the frozen query file or checkpoint. The continuation helper checks process
identities and holds the native checkpoint lock while preparing the append.

```bash
# Preview only: preserve D≤9 and add the cumulative R≤1 physical request.
nix develop /common/dev/rustred --command python -B \
  examples/python/extend_rank_campaign.py \
  --campaign-directory campaigns/my-five-loop-r0-d9 \
  --extend-rank 1 --dry-run

# Write the immutable required-query amendment; do not start native work.
nix develop /common/dev/rustred --command python -B \
  examples/python/extend_rank_campaign.py \
  --campaign-directory campaigns/my-five-loop-r0-d9 \
  --extend-rank 1
```

The printed command resumes the **same** frozen executable, rules, resource
policy and checkpoint. Add `--start` to the helper to invoke that resume
explicitly; otherwise no executable is probed or solver started. The regular
production launcher already supplies the entire `amendments/amendment-*.json`
chain on `--resume`. Repeating the same prepared-but-unapplied stage is safe;
apply/save it before preparing the next stage.
The helper uses a campaign's saved Python steering script when present,
otherwise the current sibling production launcher; both enforce the frozen
`bin/steering.json` policy and executable receipt.

Use `--extend-rank 2` to keep the previous D cap, or
`--extend-rank 1 --max-power-difference 10` to raise D without raising R.
Both caps are independently nondecreasing, and at least one must increase.
`--unbounded-power-difference` removes only the stage D cap; original bounds
still apply. Each stage is rederived from the full original query attachment,
so omitted fixed-D=10 rows return when allowed. Each row has a new unique ID
and an explicit **required** role. No helpers are added, promoted, superseded
or geometrically changed; original required rows remain immutable.

The native append-only scope-extension schema is
`rustred.owner-domain-scope-extension.json.v1`. Native resume authenticates the
original request and complete byte-digest amendment chain before applying it.
The helper only observes bounded CP6 JSON metadata and checks input hashes,
stage geometry and cursor structure; it does not replace native authentication.
Its current supported chain consists solely of its own rank/D extensions;
mixed manual/rescue amendments are explicitly rejected, not silently rewritten.

This retains actual processed domain records, dependency edges, closed anchors
and pending obligations from earlier stages, not just the original rule files.
Overlapping targets can reuse native exact/containment matches. It does not
promise zero reinspection for every overlap, generate new IBP rules, or produce
a new master-coefficient table for every lattice point.

The root-prefix closure bar still refers to the original admitted root prefix;
it is not a completion percentage for the enlarged required-query inventory.
The stage receipt reports the cumulative required-row count, including earlier
stages. Neither this count nor the live `required_queries_resolved` flag is a
closure certificate. Before claiming a stage complete, run the native cold
verifier against the latest resumed run's request and the same checkpoint:

```bash
CAMPAIGN=campaigns/my-five-loop-r0-d9
nix develop /common/dev/rustred --command "$CAMPAIGN/bin/rustred-FROZEN_SHA256" \
  walk-verify-closure --command "$CAMPAIGN/runs/RESUMED_RUN/request.json" \
  --checkpoint "$CAMPAIGN/checkpoints/main" --no-result --require-closure \
  --reinspect all --certification-scope physics-queries --reference-levers off \
  --threads 1 --output "$CAMPAIGN/runs/RESUMED_RUN/closure-verification.json"
```

Require PASS for the full cumulative required-query inventory, the exact
expected amendment chain, no unadmitted required rows and every required row
independently verified. Use `all-roots` when all auxiliary roots must also
close. Do not interpret automatic scope selection or the old prefix alone as
evidence that appended physical targets were verified. Replace
`bin/rustred-FROZEN_SHA256` with the `file` value from `bin/executable.json`,
and `RESUMED_RUN` with the run whose request includes the complete chain.
The older optional `audit_owner_domain_walk.py` Python audit does not yet
recognize required-scope extensions; its refusal is not a native closure
failure, and it must not be substituted for the native cold verifier above.

An earlier helper-free 116-query scalar control closed and cold-verified before
this change; that is prior evidence, not a completion guarantee for this new
R=0,D≤9 run or any enlarged stage. A fresh independent campaign is still
possible with `prepare_rank_campaign.py`, but does not reuse a prior ledger;
use continuation when retaining results is intended.

## Inspect rules, terminals and normalized candidate masters

`walk-inventory` reads an existing campaign without launching or resuming it.
CP6 saves coverage records and event counts, not the identities of every
selected rule and terminal. The command therefore cold-loads the saved owners
and re-inspects the saved domains once. Expect owner preparation time, not
just a quick checkpoint-header read. It generates no new IBPs and changes no
owner, input, checkpoint or numerical catalog.

```bash
cd /common/dev/rustred
env RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 \
  OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1 \
  SYMBOLICA_HIDE_BANNER=1 \
  target/release/rustred walk-inventory \
  --campaign-directory campaigns/five-loop-r0-d9-20261007 \
  --threads 4 --normalize-terminals \
  --output campaigns/five-loop-r0-d9-20261007/inventory.json

jq '{complete, installed_rules: .installed.rules,
     encountered_rules: .encountered.rules,
     encountered_terminals: .encountered.terminals,
     normalized_candidate_masters: .normalization.canonical_terminals}' \
  campaigns/five-loop-r0-d9-20261007/inventory.json
```

The existing Symbolica license is inherited. Build `target/release/rustred`
with the release command above if necessary. No Nix environment is needed
merely to run that built executable on this machine. The report destination
must not exist unless `--force` is specified; even that flag cannot overwrite
checkpoint or input data. Progress goes to stderr; omit `--output` for JSON
on stdout. The command follows the published `active-run.json`, not whichever
directory happens to sort last. To inspect a specific saved request, use
`--command PATH/TO/request.json --checkpoint PATH/TO/checkpoints/main` instead.

The report separates three different counts:

1. `installed`: rules and terminal declarations in the installed owner batches,
   including preferred programs and overlays. Declarations can repeat across
   batches. These counts describe available programs, not their use.
2. `encountered`: distinct owner/batch/rule identities and integral keys found
   while re-inspecting the saved symbolic covers. Partial and G2-reused records
   contribute their actual inspected residuals, not their entire enclosing
   domain. Previously inspected helper or quarantined history is counted too;
   never-inspected abandoned records are excluded. Conditional successors and
   routing covers can conservatively include terminals not needed by a
   particular concrete starting integral.
3. `normalization.canonical_terminals`: representatives remaining after the
   existing exact native vacuum normalization of that encountered key union.
   Structural aliases, eligible quadratic-numerator identities and proved zeros
   are applied; unsupported shapes are retained and reported. This is a
   **nonminimal candidate-master count**, not a proof of independence or a
   decomposition of all products into connected master factors.

Normalization is explicit (`--normalize-terminals`). It uses the one common
family already validated across the owners. It does not alter rule application
order or import relations from FORM/FMFT. `rule_page`, `terminal_page` and
`normalized_terminal_page` are bounded: select `--rules-start`,
`--terminals-start`, `--normalized-terminals-start`, and `--page-size`
(default25, at most1000).
An incomplete or failed census is reported as such and exits nonzero;
normalization is not presented as the complete campaign basis in that case.

Measured on October7,2026 with the optimized CLI:

| Saved scope | Installed rules | Encountered rules | Encountered terminals | Normalized representatives |
| --- | ---: | ---: | ---: | ---: |
| Combined four-loop,58-query control | 523 | 429 | 28 | 20 |
| Five-loop R=0,D≤9,67-query stage | 9,982 | 1,180 | 196 | 196 |

Both inventories pass complete cold reinspection. The five-loop normalizer
reports38 unsupported numerator shapes and14 cases whose projected positive
outputs are unbound in the selected terminal set; these remain in the output.
No further identities are inferred. All referenced five-loop checkpoint files
have identical checksums before and after inspection. These are scoped cover
inventories, not unrestricted-loop master counts. Local receipts are under
`TMP/walk-inventory-20261007/` and are not distributed as source.

### Further Laporta-style compression

Structural normalization is not IBP minimization. A separate optional
post-processing pass could generate ordinary IBPs near these finite terminals,
apply the saved rules where applicable, and eliminate the remaining auxiliary
integrals with Symbolica to obtain terminal-only relations. Uncovered auxiliary
integrals must remain columns; they cannot be dropped just because they lie
outside the initial rank/D request. Verified relations could then be stored as
an exact terminal-substitution sidecar, without regenerating the campaign.

RustRed already exposes `solver::bridge::solve_laporta` for finite targets.
Earlier [finite-terminal experiments](research/finite_terminal_relations.md)
found nine additional four-loop identities after applying structural aliases
to the generated auxiliary columns too. Those results demonstrate the mechanism,
not a measured reduction of this five-loop terminal set. `walk-inventory`
remains read-only. The optional second phase below runs a finite ordinary-IBP
search. A bounded search finding no additional relation does not establish a
minimal master basis.

## Publish every completed scope; refine manually

The current common wrapper publishes a portable artifact after every successful
solve or scope extension, without automatically searching master relations:

```bash
python -B examples/python/saved_campaign.py run --campaign /path/to/campaign
python -B examples/python/saved_campaign.py inspect --campaign /path/to/campaign
python -B examples/python/saved_campaign.py extend --campaign /path/to/campaign \
  --rank 1 --max-power-difference 10
```

To publish an already completed checkpoint without permission to solve an
incomplete scope, use `publish` instead of `run`. The wrapper defaults to the
current optimized native binary; `--executable /path/to/rustred` selects a
publisher/refiner separately from the original frozen solve engine.

Refinement is explicitly requested and consumes the published artifact:

```bash
python -B examples/python/saved_campaign.py refine --campaign /path/to/campaign \
  --seed-depth 0
```

The old persisted opt-in never authorizes future automatic refinement.
`artifacts/latest.json` selects the latest completed package; each earlier
scope package remains intact. A paused refinement does not replace usable
output. Repeat `run` after interruption of solving/publication, or explicitly
repeat `refine` after interruption of refinement. See
[campaign_master_reduction.md](campaign_master_reduction.md) for the full
command surface and portability/claim boundaries.

### Details of explicitly requested terminal refinement

Use `refine` (or low-level `--refine-masters`) only after publication. It
loads the published finite terminal inventory, generates ordinary IBPs around
those keys, and uses Symbolica's
exact sparse elimination to find terminal-only relations. Generated auxiliary
integrals remain matrix columns; they are never discarded for exceeding the
starting R/D bounds. Exact structural aliases on generated columns can expose
additional relations. This initial lane does **not** back-substitute through
the routed owner trace engine, which is not a coefficient reducer.

The default finite seed depth is zero: ordinary IBPs at each normalized
terminal. `--master-seed-depth N` requests a signed-L1 neighbourhood around
those seeds. At five loops each seed supplies 25 ordinary IBPs; increasing
the depth can increase work sharply. Finishing the selected search is labelled
`completed_nonminimal`, not proof of independent masters or an unrestricted
five-loop family solve.

An existing campaign's solve binary can stay frozen. Select a new postprocessing
binary independently with `--master-reduction-executable`, which is copied and
frozen under the campaign's separate `master-reduction/` directory. For example,
after building the current release:

```bash
nix develop /common/dev/rustred --command python -B \
  examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/five-loop-r0-d9-20261007 \
  --resume --refine-masters \
  --master-reduction-executable target/release/rustred --start
```

Executable and seed preferences persist, but permission to refine does not.
An R/D extension solves and publishes the larger scope without refinement.
Use the current helper, not an older copied Python snapshot.

The dashboard explicitly switches to **Master refinement**, with finite work,
raw/normalized/remaining terminal counts, sparse rows/nonzeros, CPU/RAM and
checkpoint status. Ctrl+C requests an orderly stop; the current atomic native
row finishes before the cursor and matrix are saved. The active Symbolica row
cannot be asynchronously interrupted safely. A resume continues the saved
refinement session when `refine` is explicitly repeated, rather than rerunning the completed campaign. Scheduled
checkpoints use the campaign interval; a machine crash may redo work since the
last durable checkpoint. Earlier R/D-stage packages remain available, and
compatible later stages reuse saved independent rows instead of regenerating
their completed source equations.

The final phase directory is a portable **symbolic package**: native owner
programs, routing and exact starting-scope metadata, plus a native Symbolica
terminal-relation state. Relative runtime members remain valid after moving
the entire directory. This supplies symbolic ingredients for later Vakint
integration; numerical five-loop master values and a routed coefficient
application adapter are not supplied by this step. CP6 coverage verification
is not upgraded to a global inter-owner termination or source-certification
claim.

Inspect either a paused phase or the final package without loading its large
owner programs:

```bash
target/release/rustred master-inspect --artifact /path/to/phase-directory
target/release/rustred master-inspect --artifact /path/to/phase-directory --format table
target/release/rustred master-inspect --artifact /path/to/phase-directory --format json
```

Interactive terminals get an aligned coloured table; `NO_COLOR` disables
colour. Redirected automatic output is JSON. The table distinguishes installed
rule records from observed-cover rules, the current scope's terminals from the
retained search inventory, normalized candidates and the remaining nonminimal
basis. It also displays starting R/D caps (within the recorded domains), search
depth and checkpoint state. Native Rust callers cold-load the composed
`MasterReductionArtifact` with `load_master_reduction` and call
`apply_terminal(family, key)` for exact collected substitutions. The explicitly
named `load_master_relation_session` exposes only the finite search cursor,
without the cross-family collection layer. See the
[collection contract](campaign_master_reduction.md#native-terminal-collection-and-application)
for peer inputs, native persistence, retained guards and scope-extension reuse.
No numerical evaluation or assertion of master minimality is implied.

## Reading the corrected monitor

- `Completed-scan D/C` is new discoveries divided by new recursive closures
  between the **last two completed scans**, with the elapsed interval and
  scan age. It remains visible when stale; it is not a closure ETA. The
  trailing-hour observation is shown separately.
- `Next closure scan` reports throttle eligibility. Once due, the coordinator
  must reach a monitoring boundary. It is not a promise of an exact start time.
- `Checkpoint size` is the last saved generation's referenced payload bytes,
  including sealed record segments, not the size of all old files in its folder.
- `Max scheduled rank` is a bound derived from every admitted domain's geometry,
  including restored domains. Truly unbounded domains are labelled as such;
  absence of an explicit rank cap alone does not imply infinity.

New builds read pre-update checkpoints, but old binaries do not understand the
new persisted scan-history field. Keep a pre-update checkpoint for rollback
to an old executable. Scope-extension resumes require a binary supporting the
new required-query schema; freeze that binary before starting R0.
