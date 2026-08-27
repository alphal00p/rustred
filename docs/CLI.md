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
rustred derive --input examples/cli/one_loop.symbolica \
  --input-format symbolica --output one_loop.derive.toml --n-cores 4

rustred derive --input-format symbolica < examples/cli/one_loop.symbolica
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
byte-identical output for `N=1`, `N=2`, and `N=4`. The forthcoming
multi-topology campaign scheduler uses the same option to bound all
concurrently active topology/sector/case work.

`parameters(d,m2)` is intentionally absent. RustRed infers the scalar
parameters after excluding declared family identifiers, loop momenta,
external momenta, and propagator IDs. Add a `parameters(...)` clause only when
an exact allowlist is useful. Parameters used only by `numerator(...)` are not
inferred by `derive`; declare them explicitly, inside the expression or in
hybrid TOML, when later numerator processing will need them. RustRed retains
the complete declared allowlist and its source order in the canonical `I(...)`
and in `provenance.input_parameters`. Only the parameters actually discovered
in dimension, propagators, power shifts, and external Gram values enter the
operational coefficient field. That active subset is sorted in
`family.parameters`, so declaration order and numerator-only extras cannot
change a family fingerprint or specialize a parametric IBP. The numerator and
its declared-only symbols remain target metadata for later processing.

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

# Optional. If omitted, d and m2 are inferred. If parameters(...) also occurs
# inside I(...), the two ordered declarations must agree exactly.
parameters = ["d", "m2"]

[metadata]
description = "two-loop massive vacuum family"
campaign = "massive-vacuum-validation"
tags = ["vacuum", "two-loop"]
```

Metadata is a bounded, sorted table whose values are strings or string arrays.
It is reported as provenance but never enters the family or relation
fingerprint. `parameters` is a strict declared allowlist, not metadata: every
scalar discovered in a family-defining field must occur in it. Its complete
user-written order is retained in provenance and canonical input, while only
the discovered subset enters the sorted operational coefficient context.
Consequently, a declared parameter used solely by `numerator(...)` remains
available for later tensor processing without changing derived IBPs.

### Fully explicit compact TOML

The explicit form is useful for generated configurations and tooling while
still accepting concise Symbolica expressions instead of coefficient
matrices:

```toml
schema = "rustred.project.toml.v1"
parameters = ["d", "m2"] # optional; inference is also available here

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

Exit status categories are stable for v1:

| Status | Category |
|---:|---|
| 0 | success |
| 2 | command usage |
| 3 | input I/O |
| 4 | input schema, grammar, or lowering |
| 5 | parametric derivation |
| 6 | output serialization or size policy |
| 7 | output I/O |
| 8 | parallel-execution setup or Symbolica license policy |

Both input and output have finite byte limits. The output is fully rendered
and checked before any byte is written to stdout, so an error never leaves a
partial machine-readable document.

Before generation, the CLI also checks a topology-independent worst-case count
of raw relation-term additions, capped at 2,000,000 attempts. Before rendering,
it charges the packed normalized/source Atoms with a conservative canonical
render factor and censuses all exact family and generated-relation rational
polynomials: sparse numerator/denominator terms, dense exponent payload,
integer magnitudes, shifts, and guard provenance share the 256 MiB
retained/render budget. These conservative limits are CLI policy; library users
can select their own resource policy. `--relations ordinary` does not construct LI rows;
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
  stable typed provenance;
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

For example, [`examples/cli/campaign.toml`](../examples/cli/campaign.toml)
contains two independently named concrete targets expressed as ordinary
Symbolica `I(...)` strings. The compact v1 container is:

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

# Optional; the same strict declaration semantics as hybrid project TOML.
parameters = ["d", "m2"]

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
  < examples/cli/one_loop.symbolica
```

The output schema is `rustred.campaign-plan-output.toml.v1`. It is sorted by
stable mathematical keys, has the same 256 MiB conservative/final output
limit as `derive`, and explicitly reports:

```toml
status = "ok"
scope = "roots_only"

[phases]
root_ingress = "complete"
target_normalization = "not_started"
dependency_discovery = "not_started"
derivation = "not_started"
closure = "not_started"
publication = "not_started"
```

Thus a successful plan authenticates, lowers, deduplicates, and records only
the supplied declarations and their declared-power jobs. It does not normalize
targets, enumerate subsectors, discover dependencies, derive an IBP, claim
masters/closure, or publish replacement rules. Because dependency discovery
has not run, roots-only output contains no dependency counts.
`campaign plan` deliberately rejects `--n-cores` and `--max-memory`: neither
resource controls a roots-only metadata operation. They will first appear on
the future execution command:

```text
rustred campaign derive campaign.toml --n-cores 4 --max-memory 120GiB --resume work/
rustred campaign verify bundle/ --exact
rustred campaign inspect bundle/
```

The library contains the static multi-root plan, stateless core-plus-memory
wave selection, a move-only atomic admission authority, and low-level stable
wave/resident-transform execution primitives. The CLI still uses only the
static plan for roots-only ingress; those primitives are not yet connected to
a calibrated physical estimator, frontier coordinator, reducer execution
command, or checkpoint barrier. This command does not derive sector rules or
claim closure.

Multiple compact Symbolica family/integral expressions may supply the roots.
The future execution TOML will additionally carry campaign-wide policies and
resources; `--n-cores` and memory admission may change timing only. The
deterministic work-unit, closure, checkpoint, and multi-start bundle contracts
are specified in the
[parallel campaign foundry design](research/parallel_campaign_foundry_design_2026-08-26.md).

For a future six-loop run on a roughly 100-core, 1-TiB EPYC node,
`--n-cores 100` remains only a ceiling. Before building its pool, the campaign
will derive and report an effective execution width `E` with
`1 <= E <= --n-cores`. `E=1` runs on the coordinator without a worker pool;
`E>1` creates `E` workers, while the separate coordinator remains another
possible Symbolica Workspace owner. The fixed baseline therefore charges the
coordinator plus every possible worker (and any explicitly admitted inner
thread), not merely the currently busy reducer owners. If the `E=1` baseline
plus one minimum runnable task does not fit, the command returns a typed
memory-capacity pause before pool construction. The executor acquires a core
lease and conservative memory permits before cloning any retained reducer or
constructing another heavyweight task owner. It keeps the unadmitted ready
frontier compact and may deliberately leave cores idle to respect
`--max-memory`; operators should set that value below physical RAM to retain
headroom for the OS and Symbolica memory that its public API cannot census.
Future diagnostics record requested width, effective width, worker-thread
count, and estimator revision as physical run metadata excluded from semantic
hashes.
