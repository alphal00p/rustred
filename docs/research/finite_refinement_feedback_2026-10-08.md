# Finite refinement feedback: implementation and measured impact

## Conclusion

Finite feedback is implemented in the Rust collection engine and enabled by
default for **new explicit `refine` operations only**. It combines retained
finite IBP rows after exact eligible support identifications, without adding
new source seeds or changing the symbolic campaign walker.

The frozen five-loop inventory improves **607 → 601 candidate masters**. The
four individual four-loop parents and their combined inventory do not improve
further. The tradeoff is additional one-off refinement time, memory, proof
payload size and cold validation cost—not faster campaign closure. Previously
proved maps survive extension, but do not accelerate the domain walker.

The measured five-loop chain is:

`829 raw → 651 normalized → 608 ordinary/circuit → 607 diagonal → 601 feedback`.

All six newly eliminated outputs are scalar/dotted keys. The **253
numerator-bearing outputs remain 253**. These counts are nonminimal terminal
inventories, not a claim about the number of independent five-loop masters or
unrestricted family closure. No production campaign or Vakint package changed.

## What the algorithm adds

Separate elimination can leave useful relations hidden behind unresolved
auxiliaries. For example, retained rows `X + A = 0` and `Y + B = 0`, together
with a proved support identification `A = B`, give `X − Y = 0`. Re-substituting
terminal-only maps without these retained rows would miss this cancellation.

The implementation:

1. Snapshots every compatible finite session's retained upper-triangular rows
   and pending rebuild rows, plus normalization bridges and prior identities.
2. Uses family-qualified integral keys and exact full-U aliases on eligible
   support columns, including auxiliaries. Unsupported keys remain distinct;
   numerator/external columns are not discarded or mislabeled as scalar aliases.
3. Places auxiliaries before targets and preserves the native local column
   ordering where possible. Symbolica performs exact sparse forward elimination.
4. Back-substitutes only the terminal-pivot block, avoiding unnecessary expansion
   of the entire auxiliary triangular system. Every emitted relation replays
   against the **complete** aliased matrix and its source weights.
5. Stores chronological proof layers, guards, provider bindings and composed
   flat maps. Cold loading and rebind authenticate retained-row membership;
   unchanged final terminal maps cannot conceal a changed auxiliary rowspace.

The pinned Symbolica 3.0.0 API and existing RustRed use were inspected before
implementation: `SparseRowReducer::{add_row,u,l,pivots,
from_upper_triangular_matrix,back_substitute}` and native sparse multiplication
supply the algebra. No custom CAS, elimination or reconstruction kernel was added.
Native collection schema 2 retains narrow ingestion of schema-1 no-feedback
inputs. Diagonal and feedback resource limits are separate: the existing
diagonal column limit is not an appropriate limit for a 71k-column retained
finite system. Limits fail explicitly; they never truncate a relation.

Core code is under
`crates/rustred-core/src/reduction/terminal_relations/collection/saved/feedback/`.
The Rust entry points are `prepare_with_finite_feedback` and
`refine_with_finite_feedback`, with cancellable variants. Application/CLI and
Python wrappers steer these primitives rather than implementing algebra.

## Matched release measurements

Two matched pairs, alternating off/on then on/off, used the same optimized
binary and immutable sources. Each native process was pinned to physical CPU52
with one worker and all nested compute pools capped at one. No production cores
(64–95) were used. Compilation is excluded; the full-process boundary includes
input cold validation, refinement and writing the new publication. The
preparation sub-timer covers terminal collection, not the whole command.

Values below are two-run medians (also averages for two observations). This is
a reproducibility check, not enough sampling for a statistical speedup claim.

| Input | Raw keys | Remaining off → on | Full process off → on (s) | Collection preparation off → on (s) | Peak RSS off → on (MiB) |
|---|---:|---:|---:|---:|---:|
| H | 22 | 19 → 19 | 0.214 → 0.277 | 0.038 → 0.101 | 12.0 → 15.0 |
| FG | 16 | 15 → 15 | 0.137 → 0.196 | 0.021 → 0.059 | 9.0 → 15.0 |
| BMW | 17 | 15 → 15 | 0.150 → 0.202 | 0.022 → 0.067 | 9.0 → 15.0 |
| X | 19 | 16 → 16 | 0.240 → 0.309 | 0.046 → 0.119 | 12.0 → 15.0 |
| Combined four-loop | 74 | 20 → 20 | 0.846 → 1.136 | 0.128 → 0.416 | 18.1 → 39.1 |
| Frozen five-loop | 829 | **607 → 601** | **21.389 → 34.757** | **2.298 → 16.447** | **338.9 → 797.9** |

Five-loop full times range from 20.805–21.973 s off and 34.114–35.401 s on.
Median CPU times are 21.195 → 34.460 s; combined four-loop CPU times are
0.820 → 1.100 s. Thus six fewer candidates cost approximately **13.37 s extra**
on this inventory, a 62.5% process-time increase and a 0.99% terminal reduction.
This is a modest algebraic gain, not a performance win.

The retained five-loop feedback contains 16,662 rows, 71,339 columns (70,732
auxiliary), 1,212 aliases, six output equations, 394,691 sparse-factor nonzeros
and 1,764 replay operations. Its native collection payload grows from
150,099 to 6,291,109 bytes; these are collection bytes, not total package bytes.
No-gain four-loop trials retain no feedback layer, so their off/on payloads
are identical. Feedback counters describe **retained** proof work, not all
attempted scratch work. `passthrough_terminals = 0` in the five-loop result
does not mean that all numerators were eliminated.

Separate independent cold-load measurements give approximately 18.9 → 32.5 s
for five loops. After validation, 100 passes over all 829 map lookups take
about 16 ms; this is borrowed-map lookup timing, **not** full integral reduction.
Proof validation is not repeated on each application.

An initial off measurement overlapped our own lifecycle test on CPU52 and is
excluded. The entire table was rerun serially after that test completed, under
the `clean` evidence label. Other shared-host users remained active; their
uncontrolled contention is a limitation of the timing data.

## Correctness, lifecycle and independent audit

- 48 core terminal-relation tests, 34 app master-reduction tests, 10 CLI tests,
  two table tests, 64 Python workflow/wrapper tests and 33 dashboard tests pass.
- Independent code/math review covers a physical cross-family shared-auxiliary
  cancellation, numerator auxiliaries, guard preservation, guard/rowspace
  tampering, native source binding and schema ingestion.
- Independent cold application verifies all **74 four-loop and 829 five-loop
  original maps**. Composing every baseline map through the new map gives the
  new result exactly; all outputs are retained terminals and homogeneity
  exponents are preserved. Nine five-loop raw maps change.
- 65 four-loop original maps additionally match the existing exact FMFT census.
  The other nine receive composition checks only. No numerical five-loop oracle
  evaluation is claimed.
- Local finite source cursors, rows and columns do not advance during feedback.
  The two five-loop feedback payloads are byte-identical.
- The actual Python workflow passes publication at R≤1,D≤4, explicit refinement,
  unchanged-repeat no-op, inspection, amendment to R≤2,D≤5, stale-publication
  detection, extension publication without discovery, and explicit refinement
  again. This is a small lifecycle control, not a full four-loop scope benchmark.
- Ctrl+C during collection preserves a pre-collection checkpoint with the finite
  source search complete. Resume retries unfinished collection, not the campaign
  solve. The collection matrix itself is not an incremental checkpoint. A real
  five-loop SIGINT test saved safely in 9.29 s; resume produced the exact same
  native collection bytes as the uninterrupted run, with unchanged source rows
  and 601 outputs. The first harness attempt reused a create-new event-log path;
  changing to a fresh resume log fixed that test setup error, not product code.

Implementation lanes were `finite_feedback_core` and `finite_feedback_app`;
`collection_workflow_audit` independently audited code and results. Root ran
integration, matched measurements and public lifecycle checks. No findings were
resolved by weakening guards, dropping auxiliaries or changing asserted coverage.

## Use and reproducibility

For a new explicit refinement, feedback is enabled by default:

```bash
python -B examples/python/saved_campaign.py refine \
  --campaign "$RUSTRED_CAMPAIGN" --finite-feedback
python -B examples/python/saved_campaign.py inspect \
  --campaign "$RUSTRED_CAMPAIGN"
```

Use `--no-finite-feedback` to skip **new** feedback discovery. Already proved
maps are retained. The mode/recipe is part of the checkpoint identity; omitted
native resume flags inherit the recorded choice. Ordinary run, publication
and extension never start this stage implicitly. Additional compatible family
packages can be supplied through repeated `--collection-artifact` options.

For an isolated native comparison, refine the same immutable source twice,
using distinct output directories and distinct event files:

```bash
target/release/rustred walk-master-reduce \
  --artifact /path/to/source --directory /path/to/off \
  --normalization-profile standard --no-finite-feedback
target/release/rustred walk-master-reduce \
  --artifact /path/to/source --directory /path/to/on \
  --normalization-profile standard --finite-feedback
```

The five-loop comparison additionally uses `--circuit-symmetry-assistance` in
both modes to preserve the frozen source's chosen provider. Peer inventories,
provider modes, profiles and all other arguments must match.

Build: `cargo build --release --locked -j 8 -p rustred-app --bin rustred
--features capacity-dispatch` inside `nix develop`, CPU32–39/build-0 lock.
Measured binary SHA256:
`f24853278c7b1379a8d4d98e22126d2adee64f89a989e72bd59934c650d69595`.

Local evidence (deliberately not committed):

- `TMP/finite-feedback-20261008/compare.py --cases h fg bmw x combined five
  --repeats 2 --label clean`, run through `nix develop --command python -B`;
  `clean-summary.json`, per-run receipts, GNU-time files and native outputs.
- Four-loop sources: `TMP/collection-workflow-20261008/four-loop-inputs/family-*`;
  five-loop: `TMP/terminal-compression-20261008/five-loop-standard-v2/circuit`.
  Receipts retain unchanged source manifest/native hashes and exact commands.
- `TMP/finite-feedback-20261008/workflow-validation.json` and focused test logs.
- `TMP/finite-feedback-20261008/interruption-receipt.json`, scratch SIGINT/resume
  command, output checks and checkpoint timings.
- `TMP/finite-feedback-audit-20261008/{four,five,five-details}.json`, independent
  verifier and timing logs. `five-details.json` lists all six eliminated keys
  and their exact rational replacements (8–52 terms), including dimensions.

The five-loop collection payload SHA256 is
`8bb64a2660065defe1011475a82ac4a9ff6855a74dfa6af4eb5844bba961fdec`.
Measured outputs and private/reference materials remain untracked.

## Decision and limits

Keep feedback on for explicit `refine`: avoiding six expensive numerical-master
evaluations can justify approximately thirteen extra seconds once. Users needing
the cheapest refinement can switch discovery off. Negative four-loop results
are retained rather than suggesting that feedback always helps.

This delivery deliberately stops at finite feedback. It does not seed more
Laporta equations, find a minimal basis, certify previously unproved campaign
rules, calculate numerical masters, change Vakint or feed these identities into
live symbolic domain traversal. Extension reuses the proved output maps; only
another explicit refinement attempts additional finite cancellations.
