# RustRed command-line interface

`rustred derive` is the human-facing entry point for RustRed's generic
LiteRed-like derivation layer. It constructs an exact affine integral family
with Symbolica and emits the complete fully parametric ordinary IBP and
Lorentz-invariance identities. It does not choose sectors, masters, special
recurrences, or reduce a concrete target.

The CLI never invokes FORM. The RustRed crate and binary use Symbolica's GMP
backend; there is no `no_gmp` mode.

## Quick start

The smallest input is a single Symbolica expression:

```text
I(
  name(tadpole),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1)
)
```

Run it from a file or standard input:

```console
rustred derive --input INPUT.symbolica \
  --input-format symbolica --output one_loop.derive.toml --n-cores 4

rustred derive --input-format symbolica < INPUT.symbolica
```

`--n-cores N` is a positive invocation-wide worker-core budget. The current raw
one-family `derive` command uses one private bounded Rayon pool for independent
ordinary IBP rows, then—after an ordinary/LI barrier—independent LI rows and
relation materialization. `--n-cores 1` stays entirely on the coordinator and
is the deterministic serial reference. For `N > 1`, this current command
constructs exactly `N` worker threads; it has no `--max-memory` policy and does
not derive a smaller effective width. Values greater than one require a
Symbolica license. The RustRed-owned scheduler never reads or mutates Rayon's
global pool, and worker count does not enter the semantic output. Vendored
restricted/unlicensed Symbolica
currently initializes its own one-thread global fallback, while the licensed
multicore path does not use it. Integration tests require
byte-identical output for `N=1`, `N=2`, and `N=4`. The separate implemented
`campaign run-waves --n-cores N` option bounds sibling workers within one
atomic K6 wave; it is not a generic multi-topology execution width.

`parameters(d,m2)` is intentionally absent: RustRed infers family scalars after
excluding declared family identifiers, momenta, and propagator IDs. The
optional `parameters(...)` clause is only an advanced strict allowlist. It can
also disambiguate a numerator-only scalar from tensor syntax—for example, in
`c*vec(k,mu)`, an explicit list can identify `c` as scalar without treating the
Lorentz index `mu` as one. The current `derive` command retains numerators as
unprocessed target metadata and therefore does not guess that distinction.

RustRed retains an explicitly declared allowlist and its source order in the
canonical `I(...)` and in `provenance.input_parameters`. Only scalars actually
discovered in dimension, propagators, power shifts, and external Gram values
enter the operational coefficient field. That active subset is sorted in
`family.parameters`, so declaration order and numerator-only extras cannot
change a family fingerprint or specialize a parametric IBP.

## Input modes

All three modes normalize into the same syntax-authenticated DTO and then use
the same affine lowering and parametric relation generator. There is no
topology or loop-count dispatch in this path.

### Raw Symbolica

Select raw mode with `--input-format symbolica`, or let `auto` recognize an
input whose first expression is `I(...)`, `rustred::I(...)`, or Symbolica's
fully explicit `rustred::{}::I(...)` spelling.

The v1 grammar is:

```text
I(
  name(family_name),                 # optional
  loops(k1,k2,...),                  # required, nonempty
  externals(p1,p2,...),              # required, may be empty
  parameters(d,m2,...),              # optional; inferred when absent
  dimension(d),                      # required
  prop(D1, denominator_expression, target_power),
  prop(D2, denominator_expression, target_power),
  ...,
  power_shift(D1, shift_expression), # optional, at most once per propagator
  gram(p1,p1, scalar_expression),    # one upper-triangular entry per pair
  gram(p1,p2, scalar_expression),
  ...,
  numerator(expression)              # optional concrete target metadata
)
```

Clause order is immaterial. Unknown clauses, repeated singleton clauses,
ambiguous patterns, undeclared identifiers, missing Gram entries, noninteger
target powers, and conflicting labels are errors. `prop` rows must form a
complete affine scalar-product basis: for `L` loops and `E` external momenta,
there are exactly `L(L+1)/2 + L E` propagators.

`target_power` and `numerator(...)` describe a concrete validation/reduction
target. `derive` retains them in its output with disposition
`not_processed_by_derive`; they do not specialize the universal IBP rows.

### Hybrid TOML

Hybrid mode embeds the same compact `I(...)` expression and keeps document
metadata outside it:

```toml
schema = "rustred.project.toml.v1"

integral = """
I(
  name(sunset),
  loops(k1,k2),
  externals(),
  dimension(d),
  prop(D1,k1^2-m2,1),
  prop(D2,k2^2-m2,1),
  prop(D3,(k1+k2)^2-m2,1)
)
"""

[metadata]
description = "two-loop massive vacuum family"
campaign = "massive-vacuum-validation"
tags = ["vacuum", "two-loop"]
```

Metadata is a bounded, sorted table whose values are strings or string arrays.
It is reported as provenance but never enters the family or relation
fingerprint. If supplied, `parameters` is a strict declared allowlist, not
metadata: every scalar discovered in a family-defining field must occur in it.
Its complete user-written order is retained in provenance and canonical input,
while only the discovered subset enters the sorted operational coefficient
context.
Consequently, a declared parameter used solely by `numerator(...)` remains
available for later tensor processing without changing derived IBPs.

### Fully explicit compact TOML

The explicit form is useful for generated configurations and tooling while
still accepting concise Symbolica expressions instead of coefficient
matrices:

```toml
schema = "rustred.project.toml.v1"

[family]
name = "sunset"
loop_momenta = ["k1", "k2"]
external_momenta = []
dimension = "d"

[[family.denominators]]
id = "D1"
expression = "k1^2-m2"

[[family.denominators]]
id = "D2"
expression = "k2^2-m2"

[[family.denominators]]
id = "D3"
expression = "(k1+k2)^2-m2"

[kinematics]
external_gram = []

[target]
powers = [1, 1, 1]
numerator = "1"

[metadata]
description = "explicit form of the same family"
```

`power_shift = "..."` is optional on each denominator. For external
kinematics, `external_gram` is a full symmetric matrix of Symbolica strings.
Its mirrored entries must use identical strings after surrounding whitespace
is removed; the sparse form below avoids duplicating off-diagonal entries.
As an alternative, omit the matrix and provide sparse upper-triangular tables:

```toml
[[kinematics.gram]]
left = "p"
right = "p"
value = "s"
```

The explicit and hybrid fields are mutually exclusive. The TOML reader uses
strict schemas and rejects unknown fields rather than silently ignoring a
misspelling.

## Command contract

```text
rustred derive [OPTIONS]

--input <PATH|->          input path or standard input [default: -]
--output <PATH|->         output path or standard output [default: -]
--input-format <FORMAT>   auto, toml, or symbolica [default: auto]
--relations <SELECTION>   all, ordinary, or li [default: all]
--n-cores <COUNT>         maximum worker cores for parallel stages [default: 1]
--force                   atomically replace an existing output file
```

For values above one, `COUNT` may not exceed the logical cores reported as
available to the process, including operating-system/container restrictions.
This prevents an accidental request from creating an unbounded OS-thread
storm; a 100-core request is admitted on a node exposing at least 100 logical
cores.

Successful standard output contains only the complete TOML document.
Diagnostics go to standard error. File output is staged, synchronized, and
installed atomically in the destination directory; without `--force`, an
existing destination is never replaced.

The current command reports these exit-status categories; pre-alpha interfaces
and schemas may still change during the repository reset:

| Status | Category |
|---:|---|
| 0 | success |
| 2 | command usage |
| 3 | input I/O |
| 4 | input schema, grammar, lowering, or resource limit |
| 5 | parametric derivation |
| 6 | output serialization or size policy |
| 7 | output I/O |
| 8 | parallel-execution setup or Symbolica license policy |
| 70 | internal application invariant |

Both input and output have finite byte limits. The output is fully rendered
and checked before any byte is written to stdout, so an error never leaves a
partial machine-readable document.

Before generation, the shared application service checks a topology-independent
worst-case count of raw relation-term additions, capped at 2,000,000 attempts.
Before rendering,
it charges the packed normalized/source Atoms with a conservative canonical
render factor and censuses all exact family and generated-relation rational
polynomials: sparse numerator/denominator terms, dense exponent payload,
integer magnitudes, shifts, and condition sources share the 256 MiB
retained/render budget. These conservative limits are shared application policy
for both the CLI and the existing Python API; lower-level core-library users can
select their own resource policy. `--relations ordinary` does not construct LI rows;
`--relations li` constructs only the authenticated ordinary source rows needed
internally by LiteRed's LI construction and emits only LI rows.

## Output schema

The output schema is `rustred.derive-output.toml.v1`. It includes:

- RustRed and Symbolica producer versions and the canonical expression-format
  identifier;
- detected input mode, canonical normalized `I(...)`, parameter provenance
  including source and operational orders, and external metadata;
- the family fingerprint, exact parameter order, dimension, momentum order,
  and abstract index names;
- typed scalar-product coordinates and every denominator's canonical source,
  normalized expression, constant, and full affine coefficient row;
- the external Gram matrix and all generic-domain nonzero conditions with
  their deterministic `sources` collections;
- generated/emitted row counts;
- every selected relation with a typed row ID, its ordered integer shift
  vectors, canonical Symbolica coefficients, and exceptional nonzero
  conditions.

The equation convention is recorded literally in each document:

```text
sum(term.coefficient * I(n + term.shift) for term in relation.terms) = 0
```

All authoritative expression strings use Symbolica's fully qualified
`AtomCore::to_canonical_string()` representation. They are independent of
symbol registration order and can be parsed back to the same expression.

## Multi-topology campaign planning

The current `derive` command emits raw generic IBP/LI relations for one family;
it does not solve sectors or publish a closed replacement-rule bundle.
`campaign plan` now provides the deliberately smaller roots-only ingress:

```text
rustred campaign plan --input campaign.toml --output campaign.plan.toml
```

The compact v1 container can contain independently named concrete targets
expressed as ordinary Symbolica `I(...)` strings:

```toml
schema = "rustred.campaign-input.toml.v1"

[[roots]]
id = "tadpole-scalar"
integral = """
I(
  name(tadpole),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1)
)
"""

[roots.metadata]
purpose = "scalar validation root"
```

Every `integral` value is passed whole to the existing Symbolica input
compiler; the campaign layer does not split strings or define another
expression grammar. Parameter inference, optional `parameters`, target powers,
numerators, and affine-family lowering therefore have exactly the same input
meaning as under `derive`. The roots-only command retains the numerator but
does not tensor-reduce, scalar-lower, or cancel it against propagators.

Generated configurations may instead use the same existing project schema and
fields under a per-root `project` prefix. This cleanly reuses both its hybrid
`integral` and fully explicit `family` forms:

```toml
[[roots]]
id = "generated-root"

[roots.project]
schema = "rustred.project.toml.v1"

[roots.project.family]
name = "tadpole"
loop_momenta = ["k"]
external_momenta = []
dimension = "d"

[[roots.project.family.denominators]]
id = "D1"
expression = "k^2-m2"

[roots.project.target]
powers = [1]
numerator = "1"
```

The root must choose exactly one of `integral` and `project`. Metadata and
parameters belong beside `integral` in compact mode or inside the nested
project in project mode. Root IDs are unique ingress labels. Families with
identical exact representations and identical `(family, declared-power sector,
ordering)` jobs are interned even when they came from different roots or input
modes. The `declared_power_sector` is derived only from the signs of the
declared target powers. It is deliberately not called a target or normalized sector: numerator
lowering or denominator cancellation can change the eventual concrete support.

A one-root raw Symbolica convenience is also available:

```console
rustred campaign plan --input-format symbolica --root-id tadpole \
  < INPUT.symbolica
```

The output schema is `rustred.campaign-plan-output.toml.v1`. It is sorted by
stable mathematical keys, has the same 256 MiB conservative/final output
limit as `derive`, and explicitly reports `status = "ok"` and
`scope = "roots_only"`.

Thus a successful plan authenticates, lowers, deduplicates, and records only
the supplied declarations and their declared-power jobs. It does not normalize
targets, enumerate subsectors, discover dependencies, derive an IBP, claim
masters/closure, or publish replacement rules. It contains no fictional status
records for those unimplemented operations and no dependency counts.
`campaign plan` deliberately rejects `--n-cores` and `--max-memory`: neither
resource controls a roots-only metadata operation. The K6-specific execution
commands below consume their own strict foundry configuration; they do not
consume or upgrade this roots-only plan into a closure claim.

### Physical campaign preflight

The separate topology-free preflight accepts those physical controls today:

```console
rustred campaign preflight \
  --profile PROFILE.toml \
  --n-cores 100 \
  --max-memory 150GiB \
  --output campaign.preflight.toml
```

The profile schema is `rustred.campaign-execution-resource-profile.v1`.
There are deliberately no default byte estimates: it must explicitly provide
an estimator revision, an enclosing memory limit, all fixed components, and
the retained/transient envelope of a one-core minimum runnable task. Byte
strings accept only an unsigned integer followed by the case-sensitive binary
unit `B`, `KiB`, `MiB`, `GiB`, or `TiB`. Unknown TOML fields and arithmetic
overflow are rejected.

Output uses schema
`rustred.campaign-execution-preflight-output.toml.v1`. It reports either a
`ready` width or `paused_for_memory_capacity` with a typed shortfall. Both are
valid preflight outcomes and exit with status 0; invalid arguments exit 2 and
an invalid profile exits 4. Every unsigned output integer is a lossless decimal
string, identified by `unsigned_integer_encoding = "unsigned-decimal-string"`.

This command invokes only the pure width planner. It does not parse a topology,
initialize Symbolica or require a license, consume an accepted plan, construct
a worker pool, hydrate a reducer, or schedule campaign work. The inline test
profiles contain illustrative values, not named-host measurements.

### Bounded K6 foundry execution

Two implemented commands exercise the current K6 foundry without accepting
recurrence algebra from the caller:

```console
rustred campaign run \
  --config CONFIG.toml \
  --output run.report.toml \
  --measurements-output run.measurements.toml

rustred campaign run-waves \
  --config examples/k6_autonomous_campaign.toml \
  --output waves.report.toml \
  --measurements-output waves.measurements.toml \
  --artifact-output waves.rribp \
  --n-cores 4
```

Both commands consume schema `rustred.foundry-campaign-config.toml.v2`.
`mode = "autonomous"` admits no caller-authored hint object: RustRed selects
the proposal ordering, probes, coordinate priority, and itinerary. `mode =
"external-hints-only"` requires a typed `[hints]` object containing only
bounded search metadata. Neither form can encode an imported identity,
recurrence right-hand side, coefficient, source row, support, reduction, or
artifact payload. `campaign run` requires the single-sector fixed-point
itinerary and remains diagnostic-only. `campaign run-waves` requires the
full-rank atomic-wave itinerary and publishes same-rank siblings only as a
complete wave.

The deterministic semantic schemas are
`rustred.foundry-campaign-report.toml.v2` and
`rustred.foundry-wave-campaign-report.toml.v2`. Optional timing sidecars use
`rustred.foundry-campaign-measurements.toml.v1` and
`rustred.foundry-wave-campaign-measurements.toml.v1`; timings never enter the
semantic report. The stderr dashboard is terminal-only by default,
`--no-progress` disables it, and `--color auto|always|never` controls only that
presentation.

A completed process is not necessarily a closed campaign. An incomplete wave
report has `publication = "diagnostic_only"`, `outcome = "incomplete"`,
`artifact_installed = false`, and `durable_artifact_published = false`. It owns
no artifact bytes and does not touch `--artifact-output`; the report instead
contains the blocking wave and detached exact residual diagnostics for every
blocking sibling. Only a fully published wave chain is installed as a K6
artifact, deterministically encoded, decoded through the untrusted cold-load
boundary, replayed, and canonically re-encoded before artifact bytes become
available. Report, measurement, and artifact destinations must be distinct;
lexical aliases and aliases through existing symlinked parents are rejected
before any write, including with `--force`.

The supplied external-hint and autonomous example configurations are bounded
release inputs, not evidence of K6 closure. Run both from a release build after
each coherent foundry slice and retain their exact residual geometry, resource
stop, and execution time. K6 remains open until both lanes independently pass
the documented closure, replay, reload, and representative-reduction gates.

### Durable closing artifacts

Three fine-grained campaign operations expose the completed `K = 1` and
`K = 3` closing artifacts without introducing topology-name dispatch:

```console
rustred campaign generate \
  --family unit-mass-vacuum-k1 \
  --output one_loop.rr

rustred campaign inspect \
  --artifact one_loop.rr \
  --output one_loop.inspect.toml

rustred campaign reduce \
  --artifact one_loop.rr \
  --powers 3 \
  --output one_loop.I3.toml

rustred campaign generate \
  --family unit-mass-vacuum-k3 \
  --output two_loop_sunset.rr

rustred campaign reduce \
  --artifact two_loop_sunset.rr \
  --powers 2,2,1 \
  --output two_loop_sunset.I221.toml
```

`campaign generate` writes the deterministic binary artifact itself, not a
TOML proxy. The semantic family selectors are `unit-mass-vacuum-k1` and
`unit-mass-vacuum-k3`. There is no preset K6 selector: K6 bytes can originate
only from a completely published `campaign run-waves` result, and the current
bounded example campaigns do not close K6.
`campaign inspect` and `campaign reduce` require artifact bytes from a path or
from `--artifact -`; neither substitutes a hidden preset for those bytes. The
`K = 3` untrusted-load boundary cold-regenerates its registered derivation once
and requires a byte-exact comparison before returning the sealed owner; no
foundry work or authentication repeats in the reducer hot path. Generation
output uses the existing atomic file installer, so `--force` is required to
replace an existing file. Invalid or truncated bytes are rejected before any
requested TOML output file is created.

The equivalent stream operation is exact binary piping:

```console
rustred campaign generate --family unit-mass-vacuum-k1 \
  | rustred campaign inspect --artifact -
```

Inspection emits schema
`rustred.closing-artifact-inspect-output.toml.v1` after one bounded decode,
authentication, and exact replay. Reduction emits schema
`rustred.closing-artifact-reduce-output.toml.v1`, exact Symbolica-canonical
unit-mass coefficients keyed by master power vectors, and a separate decimal
string `common_mass_squared_power`. For `--powers 3`, the only master is `[1]`,
the coefficient is
`(-6*rustred::{}::d+8+rustred::{}::d^2)*1/8`, and the common-mass-squared
power is `-2`.

`--max-rule-applications N` is a nonnegative per-call ceiling, defaulting to
and capped at 1,000,000. Durable input is bounded at 256 MiB before decode and
then by the core codec's structural, string, coefficient, arity, and exact-
algebra limits. Successful decoding produces one sealed owner; recursive hot-
path application does not repeat cold authentication.

The core library contains a host-independent pre-pool effective-width planner,
checked resource values, bounded ordered execution, and the K6-specific
single-sector and atomic-wave foundry drivers. Roots-only family/sector/job
interning is application-owned. The width plan enforces
`M_operational < M_enclosing`, charges the coordinator and every possible
warmed worker plus one minimum runnable task, and returns a typed no-fit pause
without constructing a pool. The roots-only CLI remains separate; the resource
preflight exposes only the pure decision from an explicit profile. Named-host
calibration, task-specific estimator adapters, generic multi-family campaign
execution, and checkpointing remain unimplemented. The
fine-grained `K = 1` and `K = 3` artifact commands are separate from roots-only
planning and physical preflight; they do not claim three-loop closure or
Vakint integration.

`--n-cores` is always a ceiling. The planner derives an effective execution
width `E` with `1 <= E <= --n-cores`. `E=1` denotes inline coordinator
execution without a worker pool; `E>1` denotes `E` worker threads, while the
separate coordinator remains another possible Symbolica workspace owner. The
fixed baseline therefore charges the coordinator plus every possible worker
and any explicitly admitted inner thread. If the inline baseline plus one
minimum runnable task does not fit, preflight returns a typed memory-capacity
pause.

Operators should keep `--max-memory` below physical RAM to preserve headroom
for the OS and opaque Symbolica scratch that its public API cannot census. The
reported width, limits, fixed breakdown, and estimator revision are physical
metadata excluded from mathematical identities. The exact closure boundary,
remaining checkpoint work, and parallel-memory contracts are documented in
the [foundry design](foundry.md); the present K6 commands implement only the
bounded execution and conditional publication surfaces described above.
