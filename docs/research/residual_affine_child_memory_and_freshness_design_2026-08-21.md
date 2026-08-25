# Residual affine child memory and freshness design

## Purpose

This document specifies the sealed child boundary required by the
source-neutral affine inventory.  It covers the integer-system, affine-branch,
and branch-guard layers.  Existing V1 certificates and public compile/replay
behavior remain unchanged.

The key audit result is that the current branch no-replay path is genuinely
Boolean-cover-no-replay, but the guard path is not transitively no-replay.
Guard plan construction calls integer-system `replay()`.  V2 therefore needs a
non-forgeable freshness seal produced beside a newly compiled integer system.

## Fresh integer-system seal

Add a non-`Clone`, crate-private result whose constructor and fields remain in
the integer-system module:

```rust
pub(crate) struct ResidualAffineIntegerSystemFreshCompilation {
    certificate: Arc<ResidualAffineIntegerSystemCertificate>,
    retained_owned_logical_bytes_upper_bound: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}
```

The fresh result is consumed exactly once.  Its consuming split returns the
exact retained certificate `Arc` together with a separately non-`Clone`
`ResidualAffineIntegerSystemFreshPlanAuthorization`; no borrowed or repeatable
certificate extractor exists.  Both values authenticate the same allocation,
and neither can be minted from an old or independently reconstructed V1
certificate.

The V2 branch compiler consumes the fresh result.  Its guarded outcome owns
the branch `Arc` and the single plan authorization, while proved-empty and
unsupported outcomes structurally cannot own an authorization.  This includes
an atom-unsupported branch which retains a diagnostic affine map.  Add a
private shared plan-building inner and a crate-private consuming entry point:

```rust
compile_residual_affine_composition_plan_from_fresh_integer_system(
    ResidualAffineIntegerSystemFreshPlanAuthorization,
    limits,
)
```

This entry point omits only the public path's integer-system replay.  Guard V2
compilation consumes the guarded branch authorization, verifies exact
integer-system, branch, and Boolean-cover allocations with `Arc::ptr_eq`, and
never returns raw owners.  Even a zero-guard branch consumes the authorization
and builds the authenticated plan once.  The V1 plan API retains its existing
replay.

## Logical-memory convention

Memory certificates count allocator-independent logical slots, never
`Vec::capacity()` or allocator/GMP capacity.  Let:

```text
W           = size_of::<usize>()
arc<T>      = 2W + align_of::<T>() - 1 + size_of::<T>()
slots<T>(n) = n * size_of::<T>()
gmp(N,B)    = ceil(B/8) + N*W + max(N-1,0)
```

For post-build recomputation, traverse actual `Integer::Large` values and add
`ceil(significant_bits/8) + W` per value.  Shared `Arc` payloads are excluded;
their inline handles are already part of the enclosing fixed structure.
The final `max(N-1,0)` is required because separately rounding each integer's
bit payload can exceed `ceil(sum(bits)/8)` by as much as one byte for every
integer after the first.

## Integer-system envelope

For configured limits:

```text
I  = max_input_rows
C  = max_canonical_rows
IC = max_input_components
IL = max_input_lineage_ordinals
LM = max_lineage_entries_materialized
A  = max_ambient_arity
RO = max_row_operations
OI = max_operation_integer_entries
M  = max_map_entries
BW = max_integer_bit_work
```

A conservative retained envelope is:

```text
IS_ret =
    arc<IntegerSystemCertificate>
  + (I+C) * sizeof<InputRow>
  + 3*IC * sizeof<Integer>
  + 2*IL*W
  + RO * sizeof<RowOperation>
  + C * sizeof<FinalRow>
  + 2*LM*W
  + 2*A*W
  + M * sizeof<Integer>
  + gmp(3*IC + OI + M + 3, BW)
```

With cumulative `E=max_allocation_entries_reserved`, integer-bit work `B`, and
`Q` the largest logical work-entry type:

```text
IS_peak = arc<IntegerSystemCertificate> + E*Q + gmp(B,B)
```

Persist an adjacent transient census on both success and `Unsupported`:

```rust
struct ResidualAffineIntegerSystemRawTransientCensus {
    allocation_entries_reserved: usize,
    state_entries_materialized: usize,
    integer_bit_work: usize,
    frontier_states_peak: usize,
}
```

The current V1 result discards replay/canonical overlap, live DFS state and
siblings, row-normalization overlap, extended-GCD temporaries, winning-state
plus final-map overlap, verification clones, and all budget counters on the
unsupported exit.  These must be included in the V2 adjacent census rather
than added to the frozen V1 payload.

## Branch envelope

For branch limits, use:

```text
Z  = max_zero_atoms
G  = max_nonzero_guards
S  = max_zero_atom_source_terms
E  = max_zero_atom_exponent_entries
R  = floor(max_potential_row_components / 2)
WB = max_potential_block_witnesses
XE = max_potential_block_exponent_entries
U  = max_unsupported_reasons
F  = max_retained_atom_context_fingerprint_bytes
B  = max_potential_retained_integer_bits
```

`R` is halved because existing branch preflight charges exactly two retained
row copies.  The local retained envelope includes the branch control block and
certificate, fingerprints, zero recognitions, guard ordinals, copied source
coefficients/exponents, row integers, witnesses/exponents, atom fingerprint
bytes, unsupported reasons, and GMP payloads.  Add `IS_ret` when the branch
owns an integer system.

Temporary system input and atom scratch are bounded by:

```text
Input = Z*sizeof<IntegerSystemInputRow> + R*sizeof<Integer> + Z*W + gmp(R,B)

Atom =
    atom.max_primitive_row_components * sizeof<Option<usize>>
  + atom.max_primitive_row_components * sizeof<Integer>
  + atom.max_base_variables * sizeof<u16>
  + gmp(atom.max_integer_bit_work, atom.max_integer_bit_work)
```

Then:

```text
Branch_peak = max(
    Branch_ret,
    Branch_local_ret + Input + Atom,
    Branch_local_ret + Input + IS_peak
)
```

The adjacent branch census must preserve reserve counts, actual system-input
components/lineages/bits, maximum atom-attempt peak (including unsupported
attempts), partial common-row/witness state, and nested integer-system attempt
censes.

## Guard envelope

For `Q` retained entries, aggregate mapped/condition polynomial terms `T`,
exponents `E`, integer bits `B`, and origin bytes `O`:

```text
Guard_ret =
    GuardBundle + arc<GuardCore>
  + family_fingerprint.len
  + context_fingerprint.len
  + Q*sizeof<GuardCompositionEntry>
  + slots<Integer>(T)
  + slots<u16>(E)
  + gmp(T,B)
  + O
```

The polynomial aggregate counts both the mapped polynomial and its distinct
condition copy.  Origin bytes include the conservative tree-node charge.

For plan limits, retain the plan/core control blocks, variables, support bits,
full image polynomials, and pivot/free coordinates.  Add compact geometry and
exponent scratch to obtain `Plan_peak`.

V2 does **not** infer a byte bound for Symbolica's polynomial or expression
workspaces from operation counters. The production compositor uses only
public Symbolica APIs: the polynomial evaluator is the fast path, while an
ordering-sensitive preflight sends inputs beyond the vendored evaluator's
`u32` mixed-radix stride to simultaneous expression replacement, direct
Symbolica expansion, and polynomial conversion. Both paths are admitted by
checked term, operation, exponent, coefficient-width, and integer-bit-work
limits before Symbolica is called. Those limits are work policies, not a
native allocator or OOM guarantee.

The logical-memory certificate covers storage owned and retained by RustRed:
the authenticated plan, guard records, sparse polynomial payloads, provenance,
and the entry prefix/copy overlap. Symbolica's evaluator, replacement,
expression, and conversion transients are explicitly outside that ownership
census. The fallback does not allocate or retain an independently managed
RustRed replacement buffer: it supplies an iterator whose items are collected
inside the Symbolica API call. The former RustRed-owned weak-composition,
Cartesian-product, and radix-collection engine has been deleted; all affine
polynomial composition is delegated to Symbolica.

The sealed V2 guard peak is:

```text
Guard_peak = max(
    Guard_ret,
    Plan_peak,
    Plan_ret + EntryPrefix_peak
)
```

`EntryPrefix_peak` includes the outer guard allocation, every already retained
entry, and the mapped-polynomial/condition-copy overlap for the current entry.
For a zero-guard branch it reduces to the fixed outer allocation. The V2
freshness-seal path does not replay integer-system elimination; authentication
does independently recompute the structural plan census from the already
authenticated integer-system certificate.

Account for free/nonfree support lengths, compact geometry count/bits, plan
retained/peak, exponent scratch, plan plus retained-entry-prefix overlap, and
mapped-polynomial/condition-copy overlap. Persist only the authenticated
combined peak scalars. Do not use current polynomial or plan
allocator-capacity-based byte helpers for these logical censes, and do not
describe the logical census as a byte cap on Symbolica's internal workspace.

## Required narrow helpers

Each layer supplies:

```rust
pub(crate) fn memory_envelope_from_limits(...) -> Result<OwnedLogicalMemoryEnvelope, Error>;

impl Certificate {
    pub(crate) fn recompute_retained_owned_logical_bytes_upper_bound(...);
    pub(crate) fn recompute_payload_comparison_census(...);
}
```

Branch and guard already have local equal-pair comparison-census machinery;
expose authenticated scalar results, not raw payloads.  Nested integer-system
comparison remains separately bounded.

## Acceptance tests

- exact and one-below retained/peak/comparison/integer-bit limits per layer;
- unsupported integer-system search retains its transient census;
- a later non-associate atom attempt contributes partial scratch to peak;
- multi-guard symbolic composition has `peak > retained` and counts both
  polynomial copies;
- exact source/branch/system allocation accepted and independently equal
  allocation rejected;
- multiple ready terminals use one Boolean replay and zero cover/branch/system
  replays inside the sealed child path;
- tampering each adjacent retained, peak, or comparison scalar fails replay;
- checked overflow, partial-GMP-limb, and multi-Large aggregate-rounding tests;
  and
- all existing V1 compile/replay tests remain unchanged.
