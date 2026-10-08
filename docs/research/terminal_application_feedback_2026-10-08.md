# Terminal identities during application versus campaign discovery

## Current-build control

Existing weighted terminal normalization already has an application seam:
load a candidate bundle, install its sealed normalization plan before any
cache use, then call `CandidateReducer::reduce_unit_mass`. The raw terminal
declarations and source guards remain unchanged. Normalization is applied at
leaves, so subsequent coefficient accumulation and ancestor cache entries can
be smaller.

On October8 we reran the independently audited saved-program client at
`TMP/terminal-weighted-application.fs03hf/client.rs`, unchanged, against the
current frozen optimized core/application libraries. Both tests use the
nonterminal target `[2,1,1,1,1,1,1,1,1,0]`. This is a **repeat of an existing
feature**, not a measured benefit of the new cross-family implementation.

| Target | First application U-only / weighted | Whole process U-only / weighted | Output terms | Rule applications, both | Retained coefficient bytes U-only / weighted |
|---|---:|---:|---:|---:|---:|
| H D1² | 3.351 / 2.728s | 4.94 / 4.40s | 46 / 15 | 26,956 | 29,950,048 / 22,715,736 |
| X D1² | 16.896 / 13.053s | 20.09 / 16.35s | 59 / 14 | 82,637 | 123,085,704 / 88,775,062 |

Every weighted map matches exact postcomposition of the separately saved
U-only result, including coefficient contexts and common-mass power
telescoping. All five subsequent same-point cache hits per process match
exactly. Applied-rule and cached-integral counts are unchanged; coefficient
coalescing additions fall418,514→312,147 and2,139,884→1,535,210. This is
consistent with cheaper coefficient arithmetic, not fewer traversed integrals.

Weighted cold preparation costs0.342/0.492s, versus0.135/0.170s for U-only.
Native bundle loads are1.07–1.08s(H) and2.04–2.07s(X), separately recorded.
Whole-process CPU is4.90→4.37s and19.95→16.24s. GNU-time peakRSS is
497,664→497,672KiB and1,099,352→1,056,816KiB. H does not show an RSS gain.
Each pair is one fresh process per mode on a shared host, not a statistical
speedup claim. Tiny terminal-only workloads can lose to the preparation cost,
as the earlier control demonstrated.

## Reproduction boundary

Inputs are the unchanged current native programs and sidecars under
`TMP/vakint-codec-migration-20261005/raw-new/four_loop/`.
Evidence: `TMP/critical-geometry-20261008/application-{h,x}-{u,weighted}`
with `.log`, `.err`, `.time`, and `.output` suffixes. All stderr files are
empty and all process exit codes are zero. The wrapper SHA256 is
`c71533a35ec165b3c41dbb801070edb0caa48c97f059453f9cf41343891fd24e`.
Core SHA256 is `031f8dca8e66f885d2726699b556b8b3077c0bc90bb122412a03ddaa13a9afb5`;
app SHA256 is `1f92307397354c71b32798c80594b10b8f1f89933864ef4f7dd999e84b6f9026`.

The scratch client is compiled at opt-level1 against optimized libraries;
generic code instantiated in the client may inherit that setting. Treat this
as a matched application diagnostic, not a full production release benchmark.
No compilation or rule generation enters the timings. The processes use
affinity56–59, one Rayon/OpenMP/OpenBLAS thread, and clock ticks100Hz.

```bash
nix develop --command bash -c 'taskset -c 56-59 rustc --edition=2024 -C opt-level=1 \
  TMP/terminal-weighted-application.fs03hf/client.rs \
  -L dependency=target/release/deps \
  --extern rustred=target/release/deps/librustred-6639687e357815eb.rlib \
  --extern rustred_app=target/release/deps/librustred_app-e7fc8d24fcf06a2c.rlib \
  -o TMP/critical-geometry-20261008/application-control'
```

The client arguments are `MODE BUNDLE SIDECAR TARGET recurrence|terminal
NEW_OUTPUT CONTROL_OUTPUT`. Run `u` first, then `weighted` with the first
output as control. Use fresh output directories; the client refuses reuse.

## Actual campaign implications

The routed campaign currently solves reachability/coverage obligations. Its
Apply path discards RHS coefficients after local coalescing and terminates at
the raw declared terminal. Replacing a terminal by a finite exact combination
there does **not** retroactively shrink its preceding domain traversal. It
must not silently discharge unfinished coverage or be reported as a faster
campaign merely because the printed terminal count falls.

The near-term useful seam is immutable normalization during coefficient
application or terminal publication. A finite relation bank should be prepared
once, not regenerated on every leaf. Cross-family maps must keep family-tagged
keys and a common loop-measure convention; raw integer index vectors alone
are not identities.

For generation itself, the promising separate step is to identify equivalent
columns while building a finite exact source system, or transport a genuinely
parametric rule through a verified family map. These retain all auxiliary
columns, source conditions and closure obligations. A relation found at fixed
integer powers cannot simply be promoted to a recurrence at arbitrary powers.
The cross-family ordinary-source pilot tests that finite-system mechanism
separately; its successful implementation and final65→20control are documented
in `cross_family_terminal_collection_2026-10-08.md`. The same generic code finds
one additional exact equation on the frozen five-loop scalar subset, giving
608→607combined labels when numerator keys remain unchanged. These outcomes
do not establish a gain in campaign generation time. No user campaign is
modified by these diagnostics.

Inspection of generation's ordinary-source constructor found no explicit
diagonal/Euler prefix. However, existing sector preconditioning already forms
linear combinations of the ordinary rows, so the compact sum may occur
implicitly. Before adding any generation mechanism, a follow-up should check
the prepared basis, then compare prioritizing an existing proportional row or
adding a provenance-backed diagonal prefix with the ordinary fallback intact.
The existing preconditioner provenance maps back to original IBPs and can be
exactly replayed; a `RowId::Derived` label alone is not authority. This is an
untested cost-reduction hypothesis, not an additional mathematical relation
space or a reason to restart the running campaign.
