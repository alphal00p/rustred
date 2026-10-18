# Six-loop scalar-vacuum generation study

This is an **independent, user-launched generation study**, not an extension of
either five-loop production campaign. Preparing the inputs does not run IBP
generation. It does not perform a closure walk, certification, master
minimization, or numerical evaluation.

## Inputs and scope

The topology preparation uses the existing HEPKit/FeynKit scalar `phi3` graph
generator and Symbolica graph machinery. Its raw six-loop census has 388 cubic
vacuum graphs with tadpoles, zero-flow edges and factorized cases retained at
generation. After merging repeated propagators, the classification identifies
66 nonfactorized classes **among those generated cubic graphs**, with 14 maximal
fifteen-propagator representatives. This is not a count of every class in their
full contraction lattice. Exact integral unimodular momentum-routing witnesses
check that these 66 cubic-derived classes occur beneath the 14 parents; graph
isomorphism alone is not the equivalence criterion. Consult the files in
`topology/` for the exact generation options and the classification/coverage
receipts, including factorized cases that are not separate generation parents.

Each saved family has 21 independent scalar-product coordinates: fifteen
physical propagators and six auxiliary coordinates. The manifest explicitly
maps these coordinate roles; the launcher does not infer that auxiliary slots
must be trailing. The current generated families happen to use trailing
auxiliary coordinates.

The full contraction census contains **435 nonzero graph-matroid classes**:
351 nonfactorized and 84 factorized. The default manifest assigns one selected
sector representative of each class to one of the 14 parents. Thus native
generation solves 435 selected sector jobs, not hundreds of thousands of
duplicate labelled parent subsectors. The original zero-sector/routing witnesses
remain in the topology receipts; this is not a heuristic pruning of unknown
classes. Four- and five-loop controls recover 16 and 67 corresponding classes.

The `--root-zero-census` option also avoids classifying irrelevant
auxiliary-positive sectors: each parent's initial zero-sector census explores
its physical `2^15` subsets, not all `2^21` coordinate masks. Add
`--full-downsets` during preparation to diagnose the unquotiented alternative:
every nonzero physical-root subsector is then generated under each parent.
That option deliberately repeats equivalent work and is not the default.

**Selected generation is not assembled full-downset closure.** Independent
jobs do not yet install cross-parent or cross-sector alias routing to feed
generated representative rules back into every labelled presentation. The
topology classification/witness data and generated rules are the inputs to that
later integration step, not a substitute for it.

`R=0` means no negative-index degree in the starting integrals. **Positive
propagator powers remain symbolic and unbounded.** It does not mean merely
evaluating one all-unit-power integral per graph. The default generation settings
are sparse-factorized exact algebra, numerical depth zero, finite-case policy
`search`, and the existing A1 source-visitation priority (fewer terms, then fewer
coefficient monomials). These are input-controlled choices, not six-loop solver
branches. No claim is made that A1 is optimal here.

## Launch

Run from the RustRed checkout. The supplied `bin/rustred` is a frozen optimized
generation build with the 21-coordinate dispatch enabled; it is not a replacement
for the five-loop campaign's executable. The ordinary Nix environment supplies
Python and the shared libraries. The Symbolica license is read from the
environment and is never written into the study.

For a later rebuild of this specialized generation executable (not needed when
using the supplied frozen binary):

```bash
RUSTRED_RUNTIME_ARITIES=1,21 CARGO_TARGET_DIR=TMP/six-loop-build \
  cargo build --release --locked -p rustred-app --bin rustred \
  --no-default-features --features native,cli,runtime-arity-selection
```

Then pass `TMP/six-loop-build/release/rustred` as `--executable` for a **new**
study. Each prepared study freezes its own executable. The selected dispatch
includes a small one-coordinate smoke control and six-loop generation; it does
not silently enable unrelated 21-coordinate legacy walking/application paths.

Choose CPU IDs that are actually free. For example, 32 total workers and four
parent slots give eight sector workers per running parent; each free slot takes
the next parent. The worker count must be divisible by the number of slots.
This favors within-parent capacity for the two largest selected groups; it is
an initial setup, not a measured optimal six-loop allocation.
Here `192-223` is only an example CPU allocation, not a reservation:

```bash
cd /shared/codex-2/rustred
nix develop

export RUSTRED_6L_RUN="$PWD/six_loop_study/runs/scalar-r0"
python -B six_loop_study/generate.py \
  --directory "$RUSTRED_6L_RUN" \
  --manifest six_loop_study/topology/manifest.json \
  --executable six_loop_study/bin/rustred \
  --rank 0 --workers 32 --cpus 192-223 --jobs 4 \
  --max-memory-bytes 600000000000 \
  --start
```

Omitting `--start` only freezes portable input files, executable, resource
settings and a plan. It prints the explicit command that starts the native work.
Output is placed exclusively beneath the new run directory. An existing
directory is never silently reset or overwritten.

There is no wall-time deadline, sector-count stop, or swap-growth stop in this
launcher. One aggregate process-tree RSS guard covers all concurrent jobs. With
the example 600 GB request, the default 5% margin requests stopping at 570 GB;
600 GB is the hard stop. Host available RAM is shown as telemetry but does not
lower the request. This does not reserve memory or override OS/cgroup limits.

Optional native count allowances are set to the largest representable count;
bundle/coefficient permissions derive from the selected RAM allowance rather
than a separate small fixed ceiling. They are permissions, not preallocations.
Exact validation, representational limits, checked arithmetic, real allocation
failures and filesystem capacity still apply. The parameter-search depth and
finite-case policy are mathematical/search choices, not wall-time limits.
These resource statements concern the default sparse-factorized lane. The
optional `semi-numerical` backend requires a reconstruction-enabled build and
currently has separate native reconstruction degree/probe/retry allowances;
this study does not silently relax those or select that backend by default.

## Monitor, interrupt and resume

The launcher uses the existing native preparation dashboard, with independent
parent rows, active cores, RSS/host availability, current native phases, and
completed-sector checkpoint counts. The producer writes `snapshot.json` and
`events.jsonl` beneath each `attempts/<timestamp>/` directory, so the same run can
be observed from another terminal without controlling the solver:

```bash
six_loop_study/bin/rustred preparation-monitor \
  --snapshot "$RUSTRED_6L_RUN/attempts/<timestamp>/snapshot.json"
```

`--once` prints a complete one-shot table; `--json` prints the structured
snapshot. `--no-dashboard` disables only the optional viewer, not the producer,
resource guard or progress files. The optional `--full-downsets` diagnostic
uses periodic plain progress instead: its actual nonzero job counts are known
only after native zero-sector preparation, so the selected-sector dashboard
would otherwise show a misleading denominator. Structured native snapshots
remain available in either mode.

Ctrl+C stops dispatch and terminates/drains the launcher's owned native process
groups. **Only completed sectors are checkpointed. An in-progress sector must
restart.** This is not in-sector algebra checkpointing, and a RAM stop has the
same limitation. Already completed parent bundles and completed-sector shards
are retained. Resume with the exact frozen rank and inputs:

```bash
python -B six_loop_study/generate.py \
  --directory "$RUSTRED_6L_RUN" --resume --start
```

The launcher checks frozen input/executable hashes and completed output receipts;
RustRed independently binds each resumed sector checkpoint to its family, root,
rank, order and solver policy. It refuses a checkpoint directory missing its
manifest instead of overwriting unknown partial data. Choose a new run directory
for different rank, solver, or CPU/resource settings. Reusing R0 sector shards
as if they had been generated at R1 is deliberately unsupported.

## Result and interpretation

Each completed parent produces:

```text
generation/<parent-id>/
  candidates.rrbin       native Symbolica binary candidate bundle
  generation.toml        generation counts and timings
  sectors/              completed-sector native checkpoints
  completed.json        input-scope-checked output hashes and counts
```

The final `summary.json` sums the parent rule, finite-residual and solved-sector
counts. The starting selected sectors are graph-matroid representatives, but
the resulting rule/terminal totals are **not a normalized cross-parent master
count**. Finite residuals are candidate terminal integrals, not an independent
minimal master basis.
Successful native generation is reported as `generated-uncertified-candidates`,
never as a proof of scalar-family closure. It provides reusable rules for the
later routing/application study; it is not yet a packaged Vakint six-loop backend.

Pure steering regression tests (no native algebra):

```bash
PYTHONPATH=six_loop_study:tools/research/runtime_order \
  python -B -m unittest test_generate test_generation_scheduler \
  test_generation_monitor test_pipeline
```
