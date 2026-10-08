# Cross-family terminal collection: four-loop evidence, 2026-10-08

The subsequent [saved-campaign integration](saved_campaign_terminal_collection_2026-10-08.md)
now supplies native collection publication, cold application and the ordinary
`saved_campaign.py refine` command. The library-only scope statements below
describe the preceding experimental milestone, not the current public interface.

## Result and scope

The complete packaged four-loop output inventories currently give this finite,
autonomously justified sequence:

| Stage | Count | Meaning and authority |
| --- | ---: | --- |
| Packaged outputs | 74 | Sum of four family-local declared inventories |
| Completed ordinary refinement | 65 | Family-local remaining terminals after exact normalization and finite ordinary IBP |
| Cross-family scalar/dotted aliases | 22 | Global family-tagged representatives after 43 verified full-U parameter permutations |
| Diagonal ordinary-IBP collection | 20 | Two additional exact combinations, reproduced by the public production API with full native source replay |
| Historical FMFT catalogue coordinates | 19 | Formal `PR` basis used for independent offline comparisons, not a proved lower bound |

The relevant historical FMFT basis has **19 coordinates, not 16**. Neither 65,
22, nor 20 is a claim of physical master independence. The 74 and 65 counts are
family-local labels; the 22 and 20 counts are global collection coordinates. The
22-to-20 step is genuinely additional linear reduction, whereas 65-to-22 removes
duplicate labels. No oracle coefficients enter either autonomous computation.

`VacuumFamilyAliasPlan::prepare` and `apply` provide the public unit-alias view.
The separately implemented `VacuumDiagonalCollectionPlan` now reproduces the
22-to-20 result, with checked flat maps and retained original-source evidence.
The scratch and production measurements below are distinct receipts. There is
no collection artifact format, campaign mutation,
automatic cross-family publication, or topology-specific kernel dispatch in this
slice. The original four family inventories remain intact.

## Inputs and matched conventions

The source receipt is
`TMP/terminal-compression-20261008/packaged-four-loop-summary.json`.
Every source session is complete and was cold-decoded with the same standard
Symanzik resource recipe used to construct its normalization plan.

| Parent | Packaged output keys | Ordinary remaining | Circuit-assisted remaining |
| --- | ---: | ---: | ---: |
| H | 22 | 19 | 19 |
| FG | 16 | 15 | 15 |
| BMW | 17 | 15 | 15 |
| X | 19 | 16 | 16 |
| Sum | 74 | 65 | 65 |

The source sessions are
`TMP/terminal-compression-20261008/catalog-{h,fg,bmw,x}-wide-ordinary-measured.rrbin`.
Their original family-local controls checked all 74 raw keys, including cold
replay and remaining-set-only output. The ordinary controls used signed-L1 depth
zero. Adding the earlier circuit provider did not improve these four counts.

The oracle comparison uses the matching native V6 caches at
`TMP/vakint-codec-migration-20261005/raw-new/four_loop/{parent}.rrcat.bin`.
Exact family fingerprints and key arities are checked before lookup. All 65
remaining keys have no numerator indices: 51 have no dots, 10 have one dot, and
4 have two dots. Thus the scalar/dotted proof domain covers this particular
inventory without discarding numerator information.

Conventions are Minkowski denominators `D=q^2-m^2`, `d=4-2*ep`, equal masses, and
unit mass after oracle evaluation. Fresh Vakint calls use its
`FMFTandMATAD` loop normalization with master expansion and substitution disabled.
This common normalization suffices for comparisons within the four-loop oracle
catalogue; it is not silently installed as a RustRed family convention.

`IntegralFamily` does not encode external loop-measure normalization. Cross-family
unit aliases mean equality in a common formal measure. Callers must retain any
family-dependent prefactors and original source/analytic-domain conditions. The
aliases preserve loop count and every positive exponent, hence total power and
common-mass homogeneity. For a unit-mass scalar vacuum value, restoration carries
the factor `(m^2)^(L*d/2-sum(a))` in this convention. Diagonal equations mix dot
degrees: their unit-mass coefficients must not be applied unchanged to separately
mass-restored values; the corresponding raised-dot terms carry `m^2` in the
dimensionful Euler identity.

## Oracle census and fresh checks

`TMP/critical-geometry-20261008/oracle/census-full-fixed.json` records all 65 keys,
their exact native Symbolica FMFT values, and coefficient vectors in both `ep`
and `d`. Exact equality gives 22 distinct expressions; quotienting by a nonzero
rational scalar still gives 22. There are 19 formal `PR` coordinates and all 19
unit coordinate vectors occur, so the formal projection matrix has rank 19.
This proves a statement about these catalogue expressions, not physical master
independence or completeness of IBP.

Three of the 22 classes are mixed rather than pure `PR` coordinates:

| Label | Representative | Support/dots | Additional occurrence |
| --- | --- | --- | --- |
| A, class 17 | H `[2,1,0,0,1,1,1,1,1,0]` | 7 / 1 | FG `[2,1,1,0,1,1,1,1,0,0]` |
| B, class 19 | BMW `[0,2,1,1,1,1,1,1,0,0]` | 7 / 1 | X `[0,0,2,1,1,1,1,1,1,0]` |
| C, class 18 | H `[2,1,0,1,1,1,1,1,1,0]` | 8 / 1 | None in these 65 keys |

A and B contain lower-sector combinations involving `PR3`, `PR4`, `PR4d`,
`PR5`, `PR7`, `PR9`, and `PR9d`; C is `-ep*PR11-PR11d`.
The full exact expressions, rather than decimal fits, are in the census JSON.

Seven selected values were reproduced through **fresh offline Vakint/FMFT calls**:
four H inputs covering classes 13, 15, 17, 18 and three BMW inputs covering
classes 13, 19, 20. All seven fresh expressions exactly match their native cache
entries after native Symbolica rational simplification. The fresh inputs are
literal integrals generated from the checked CSV family descriptors; no guessed
`PR` assignment is supplied to the evaluator. Their receipts are
`oracle/fresh-input-receipt.json`, `fresh-h-fixed.log`, `fresh-bmw.log`, and
`fresh-exact-comparison.json` beneath the critical-geometry TMP directory.
Those seven inputs cover six distinct projection classes: `PR9d` is checked in
two families. The `PR9` entry used below initially came from the authenticated
existing catalogue. A separate eighth fresh H call, at
`[0,1,1,0,1,1,1,1,1,0]`, then reproduced `vakint::{}::PR9` byte-for-byte in native
canonical Symbolica form, in 0.17 s wall and 12,396 KiB peak RSS. The supplemental
receipts are `fresh-pr9-input-receipt.json`, `fresh-h-pr9.log`,
`fresh-pr9-comparison.json`, and `fresh-pr9.time`; the original seven-input
receipts are unchanged. All seven distinct classes in the two diagnostic
identities therefore now have a fresh check (eight input integrals total).

The exact comparisons also establish the diagnostic identities

```text
4*A + 2*B + PR9d - (2*d-7)*PR9 = 0
C + PR11d - (d/2-2)*PR11 = 0.
```

These suggested trying generic diagonal ordinary IBP after global column
equivalence. They were not imported as equations. Timings were 0.41 s for the
native census, 0.74 s for four fresh H evaluations, and 0.43 s for three fresh
BMW evaluations, with peak RSS 9,216 / 12,388 / 12,396 KiB respectively.

## Autonomous full-U aliases: 65 to 22

The alias pilot deliberately opens only the four completed native sessions, not
the FMFT caches. Its preparation is generic in family geometry and terminal keys:

1. Require equal loop count, exact dimension coefficient, and the same ordered
   coefficient names **and native Symbolica variable map** across owners.
2. Independently validate active unit-mass integer momentum squares and full-rank
   support geometry using existing native primitives. External momenta, analytic
   power shifts, numerator indices, and unsupported active masses are not guessed
   away; such keys remain unaliased with explicit skip statistics.
3. Use the existing power-colored graph canonicalizer only to propose a parameter
   permutation. Replay equality of the **full** native U polynomials, including
   their overall scale, and match each positive exponent under that permutation.
4. Keep a direct edge to the smaller declared `(family fingerprint, key)`
   representative. Reversed input order must yield the same representatives.

For this equal-loop unshifted unit-mass scalar/dotted class, full-U equality also
matches the mass-dependent parameter polynomial and the power/Gamma factors.
This is parameter-integral authority; it does not claim a momentum routing map.
In particular, equal monomial support or primitive U content alone is not enough.

Actual result from `oracle/alias-census.json`:

- Raw keys 65; canonical keys 22; verified aliases 43; no skips.
- Supports analyzed 54; canonicalizations 65.
- Native source loading 0.373478310 s; preparation 0.052192596 s.
- Reversed-input second preparation and direct-representative verification
  0.052236636 s; complete process 0.48 s wall, peak RSS 9,216 KiB.
- Independent post-hoc comparison: every native alias joins one exact oracle
  class, and each of the 22 oracle classes has exactly one native representative.
- All 12 source native-session/cache/CSV hashes match their pre-pilot values.

The post-hoc comparison is a cross-check, not proof authority for any alias.
`oracle/alias-receipt.json` contains this boundary and the complete source hashes.

### Public Rust application from already loaded sessions

The following combines expressions in the remaining keys of already loaded
`TerminalRelationSession`s. Session decoding must first select the recorded
normalization profile/limits; do not decode a standard-profile normalization
sidecar with conservative limits. The caller supplies the pinned collection
preparation recipe and finite input expressions, with any external measure
factors already retained in their coefficients.

```rust
use std::collections::BTreeMap;
use rustred::algebra::{Coefficient, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::reduction::terminal_normalization::{
    VacuumFamilyAliasLimits, VacuumFamilyAliasPlan, VacuumIntegralKey,
};
use rustred::reduction::terminal_relations::TerminalRelationSession;

fn collection_view(
    sessions: &[TerminalRelationSession],
    expressions: &[BTreeMap<IntegralKey, Coefficient>],
    preparation_limits: VacuumFamilyAliasLimits,
    algebra_limits: ExactAlgebraLimits,
) -> Result<BTreeMap<VacuumIntegralKey, Coefficient>, Box<dyn std::error::Error>> {
    if sessions.len() != expressions.len()
        || sessions.iter().any(|session| !session.is_complete())
    {
        return Err("one expression per completed source session is required".into());
    }
    let inventories: Vec<_> = sessions.iter().map(|session| {
        (session.family_owner().clone(), session.remaining_terminals())
    }).collect();
    let plan = VacuumFamilyAliasPlan::prepare(&inventories, preparation_limits)?;
    let terms = sessions.iter().zip(expressions).flat_map(|(session, expression)| {
        expression.iter().map(move |(key, coefficient)| {
            (session.family_owner().as_ref(), key, coefficient)
        })
    });
    Ok(plan.apply(terms, algebra_limits)?)
}
```

In a long-lived caller, retain the immutable plan and call `apply` repeatedly.
Application performs direct lookup and native coefficient validation/addition,
coalesces equal representatives, and removes exact cancellations; it does not
repeat graph search or full-U proof. Unknown owners/keys are errors even with
zero coefficients, and foreign coefficient contexts are rejected. Algebra
limits bound each operation, not the length or cost of a caller-supplied stream.
No input session or declared inventory is changed. Applying this alias plan
alone gives 22 coordinates on this data, not the pilot's 20.

The diagonal production slice is available under
`rustred::reduction::terminal_relations::collection`, with public
types `VacuumDiagonalCollectionPlan`, `VacuumDiagonalCollectionLimits`,
`VacuumCollectionEquation`, and `GuardedVacuumReduction`. It prepares once
from the same authenticated finite inventories, retains
original specialized diagonal sources plus exact full-source replay, and returns
flat guarded reductions with checked common-mass power differences. Its final
tests and measured application are recorded below. No CLI or global resumable
artifact workflow is promised by that interface.

### Recorded preparation limits

The pilot used `VacuumFamilyAliasLimits` defaults except that its Symanzik recipe
was the then-current shared standard recipe. This is explicit opt-in steering;
the conservative global core defaults were not changed. To reproduce the recorded
run without relying on future defaults, pin the following values:

| Limit | Value | Scope |
| --- | ---: | --- |
| Families / declared terminals | 1,024 / 1,000,000 | Collection |
| Supports / canonicalizations | 4,096 / 16,384 | Collection, not reset per owner |
| Graph vertices / edges | 4,096 / 65,536 | One canonicalization input |
| Symanzik parameters / parameter exponent | 4,096 / 65,535 | Per family |
| Polynomial terms / exponent entries | 4,000,000 / 64,000,000 | Per family preparation |
| Term operations / determinant ring operations | 16,000,000 / 16,000,000 | Per family preparation |
| Determinant matrix entries / adjugate minors | 1,048,576 / 1,048,576 | Per family preparation |
| Nested exact algebra exponent / terms / term operations | 65,535 / 4,000,000 / 16,000,000 | Algebra operation |

Family/terminal overflow is a typed error. Unsupported geometry and bounded
preparation skips retain raw keys rather than create partial authority. These
are explicit preparation bounds, not a hard timeout or a bound on all internal
Symbolica scratch memory; graph canonicalization has no abort callback. Total
Symanzik work is indirectly bounded by the finite family cap, not by one shared
arithmetic counter.

## Autonomous diagonal ordinary-IBP pilot: 22 to 20

`TMP/critical-geometry-20261008/diagonal_collection.rs` takes the same four native
sessions. For each distinct positive support of their 65 keys, it constructs the
undotted corner and generates existing ordinary IBP. It sums the diagonal loop
derivatives, then prepares full-U aliases over the **entire generated column
inventory**, not just the original terminal keys. All columns outside the target
orbits remain auxiliary columns and are eliminated before terminal columns.

Native Symbolica sparse elimination carries an explicit source-identity block.
The resulting weights `W` are replayed against the complete column-quotiented
source matrix: `W * A = output`, with no projection dropping auxiliary terms.
Source guards, original denominators, and pivot conditions are retained. The
driver does not load FMFT data and does not assert exhaustive source closure.

The measured diagonal mode has 54 corner seeds, 216 native ordinary rows combined
into 54 diagonal sums, 389 input nonzeros, and 343 scalar aliases across the
expanded column inventory. The quotient has 50 columns: 28 auxiliary plus 22
target classes. It produces exactly two independent target equations and hence
20 remaining target classes. Retained reducer storage is 276 nonzeros, and the
final exact source replay uses 7 scalar-product contributions.

Timing: decode 0.380352090 s, source generation 0.007837307 s, expanded alias
preparation 0.120191456 s, reduction plus replay 0.001187853 s; total measured
process 0.52 s wall and 9,216 KiB peak RSS. Internal total is 0.509936966 s.
The scratch bounds are 8 families, 128 seeds, 2,048 native source rows, 10,000
columns, 200,000 input nonzeros, 1,000,000 reducer nonzeros, 2,000,000 final replay
contributions, 10,000 retained conditions, and an external 120-second timeout.

The two normalized rows require `4*d-14 != 0` and `4*d-16 != 0`. Their exact
FMFT comparison, independently evaluated after `d=4-2*ep`, vanishes for both
rows and reproduces the two diagnostically checked dot identities above. This is generic-d
authority on the recorded domain, not an assertion about specialized exceptional
dimensions. The report is `diagonal-collection.json`; the independent comparison
is `oracle/diagonal-oracle-comparison.json`.

The cheap generic mechanism is therefore enough for two of the three mixed
projection classes. One extra coordinate relative to the formal FMFT 19 remains;
that discrepancy motivates a separate bounded experiment, not oracle injection
or a claim that the remaining 20 are independent.

An all-ordinary-rows negative control used the same immutable scratch driver,
the same corners, a 120-second/2-GiB outer cap, and CPUs 56–59. It generated 864
ordinary sources, 10,587 input nonzeros, and 1,092 quotient columns (1,070
auxiliary), but still found only two terminal equations and 20 remaining classes.
Internal total was 0.847 s; evidence is `TMP/critical-geometry-20261008/all-collection*`.
This supports choosing the cheaper diagonal lane, not a claim of exhaustive
ordinary-IBP closure. The separate five-loop scalar-subset control's final result
is recorded in the production section below.

The five-loop control's read-only census cold-loaded the frozen
`five-loop-standard-v2/circuit/state-0000000000000007.rrbin`: its 608 remaining
keys split into 355 nonnegative scalar/dotted keys and 253 numerator keys.
The 355 keys occupy 63 supports, with support-size counts
`{5:1, 6:6, 7:28, 8:41, 9:118, 10:129, 11:32}` and dot counts
`{0:63, 1:63, 2:90, 3:67, 4:47, 5:19, 6:6}`. The test keeps all 253
numerator keys untouched and adds no radius of new source seeds. There is no
reason to transfer the four-loop cross-family gain of 43 to this already
normalized single-family inventory. The typed census receipt is
`TMP/critical-geometry-20261008/five-loop608-scalar-census.json`, with source SHA256
`8e89e286fd3f89cb10cf1fc169138f307ec68635b406c57a8e806cd976415e8c`.

## Final production API measurements

The public `VacuumDiagonalCollectionPlan::prepare` reuses the finite source
generator and full-U aliases, retains every generated column, and eliminates
auxiliaries before requested target classes. Within the target block it prefers
higher positive power for elimination. This removes the scratch pilot's
avoidable dimension-dependent pivots: **both production controls add no nonzero
conditions**. That does not erase conditions belonging to any predecessor
reduction, nor assert pointwise convergence at all dimensions.

`sources()` exposes the original specialized ordinary rows, their `RowId`s,
corner seeds, and diagonal sums; `equations()` exposes exact target equations
and weights in those sums. `apply(family, key)` is an immutable checked lookup
of a flat `GuardedVacuumReduction`; every output belongs to
`remaining_terminals()`. `common_mass_squared_power(family, target, output)`
checks that the output occurs in that map and returns
`sum(output powers)-sum(target powers)`. The caller multiplies the corresponding
coefficient by this power of the common `m^2` when restoring dimensionful
integrals, retaining external measure factors separately.

For example, after constructing finite scalar `inventories` from the already
loaded sessions as above and choosing the pinned `preparation_limits` recipe:

```rust
use rustred::reduction::terminal_relations::collection::{
    VacuumDiagonalCollectionLimits, VacuumDiagonalCollectionPlan,
};

let diagonal_limits = VacuumDiagonalCollectionLimits {
    aliases: preparation_limits,
    ..Default::default()
};
let plan = VacuumDiagonalCollectionPlan::prepare(&inventories, diagonal_limits)?;
for (family, keys) in &inventories {
    for key in keys {
        let reduced = plan.apply(family, key)?;
        // Retain reduced.nonzero_conditions() AND predecessor conditions.
        for (output, coefficient) in reduced.terms() {
            let mass_squared_power = plan.common_mass_squared_power(family, key, output)?;
            // Keep the family-qualified output and external prefactors.
            // Dimensionful term: coefficient * (m^2)^mass_squared_power * I(output).
            let _ = (coefficient, mass_squared_power);
        }
    }
}
```

`VacuumDiagonalCollectionLimits::default()` otherwise inherits the conservative
core alias/Symanzik limits. The measured production controls explicitly selected
the standard Symanzik values in the recorded table above. This standalone API
does not infer an application profile from metadata; callers select both the
source decoding profile and the collection preparation recipe explicitly.

Unlike the alias-only plan's conservative skip behavior, this diagonal API
explicitly rejects requested numerator powers, external momenta, analytic
shifts, and active non-unit masses. A caller selecting a scalar subset must
retain the excluded numerator expressions separately; it must not pretend that
the subset describes the entire input.

| Measurement | Four-loop collection | Five-loop scalar subset |
| --- | ---: | ---: |
| Selected scalar/dotted keys | 65 | 355 |
| Global target classes before diagonal equations | 22 | 355 |
| Distinct corner seeds / native source rows | 54 / 216 | 63 / 315 |
| Columns / auxiliary columns | 50 / 28 | 515 / 160 |
| Exact terminal equations | 2 | 1 |
| Remaining scalar classes | 20 | 354 |
| Untouched numerator labels | 0 | 253 |
| Combined retained labels | 20 | 607 |
| First cold native load, seconds | 0.379305994 | 15.256862812 |
| First plan preparation, seconds | 0.123633213 | 2.117668078 |
| All 65 / 355 checked lookups, seconds | 0.000034660 | 0.000403451 |
| Independent second cold load, seconds | 0.362978298 | 15.117117273 |
| Second preparation, seconds | 0.123960865 | 2.130813268 |
| Full driver including cold repeat, seconds | 0.991228593 | 34.627116325 |
| Process wall / peak RSS KiB | 1.00 / 12,288 | 34.66 / 358,572 |

Both runs use CPUs 52–55, one Rayon thread, a 120-second timeout, and a 2-GiB
outer virtual-memory limit. Cold loading and re-preparing after reversed owner
input order reproduce the exact guarded maps, equations, and source weights.
The four-loop maps contain 72 output terms; the five-loop maps contain 358.
The five-loop total is dominated by **two approximately 15-second native loads**;
it must not be reported as 34.6 seconds of algebraic preparation. This modest
608-to-607 application is independently source-replayed, but has no five-loop
FMFT oracle comparison and no master-independence claim.

Preparation defaults bound 128 corner seeds, 2,048 source rows, 200,000 source
terms, 10,000 columns, 1,000,000 reducer nonzeros, 2,000,000 replay operations,
100,000 flat-map terms, and 10,000 conditions. The 2,000,000 coefficient-term
limit counts numerator/denominator terms in one retained matrix at phase
boundaries; it is not a whole-plan or peak-memory cap. Alias scopes remain as
documented above. These pilots opt into the recorded standard Symanzik recipe.

The actual production four-loop equations and **all 65 exported flat maps** were
independently compared against the authenticated FMFT census using native
Symbolica after `d=4-2*ep`: both equations vanish and every map agrees exactly.
All 72 exported common-mass exponents were checked independently too. Evidence:
`TMP/critical-geometry-20261008/oracle/production-four-loop-comparison.json`;
the checker is `oracle/verify_production.rs`. This external comparison is not
the native authority: production already replays its ordinary-source weights
against every column after authenticated full-U aliases.

Final controls are `production-four-loop.json` and
`production-five-loop-scalar.json` beneath `TMP/critical-geometry-20261008`.
Their source sessions remain unchanged. The thin wrapper calls the production
API directly; it does not update a campaign, implement a CLI command, or create a
resumable collection artifact. See also
[the critical-geometry note](critical_geometry_terminal_diagnostics_2026-10-08.md#production-api-replay-and-scalar-subset-control)
for the wrapper commands and source-evidence details.

### Separate, untested source-prefix idea

Offering a compact parametric diagonal sum earlier to candidate generation is a
different proposal, not a measured benefit of this application API. Current
ordinary source preparation emits individual IBPs and optional LI rows; sector
preconditioning already makes polynomial combinations and may implicitly expose
the same sum. Existing source-order controls only reorder that prepared basis.
A bounded follow-up should first inspect whether it already contains the sum,
then compare priority-only or a provenance-backed optional sum prefix while
keeping the full ordinary fallback. Any composite prefix must map back to the
original diagonal `RowId`s and pass the existing full-source/domain replay,
not acquire authority from a `Derived` label. No campaign or such generation
experiment was run for this note.

## Reproduction commands, tests, and receipts

Commands run from the repository root. The TMP readers are research tools, not
installed product commands. Existing native inputs are read-only.

```bash
nix develop --command bash -c '
  export TMPDIR=/common/dev/rustred/TMP
  flock TMP/locks/build-0.lock taskset -c 32-39 \
    cargo test --locked -p rustred --lib terminal_normalization::parametric \
      --features capacity-dispatch -j8 -- --test-threads=4
'
nix develop --command bash -c '
  export TMPDIR=/common/dev/rustred/TMP
  flock TMP/locks/build-1.lock taskset -c 40-47 \
    cargo build --locked --release -p rustred --lib --features capacity-dispatch -j8
'
```

The initial collection run passed 21 focused parametric tests. Its existing
debug executable also passed all 65 `terminal_normalization` tests and all 20
`terminal_relations` tests without rebuilding. These include the older scale,
non-involutive permutation, numerator, mass, and preparation-bound regressions.
New collection tests cover cross-family dotted aliases, input-order determinism,
cold native replay, unknown keys/owners, exact context/dimension mismatches, and
collection-wide caps. Additional independent adversarial/application tests are
tracked separately from that initial 21-test receipt. The final implemented
collection slice passed **27 terminal-relation tests, 69 normalization tests,
and 418 candidate-reduction tests**, with zero failures (0.21 s, 15.69 s, and
4.95 s respectively). Final logs are `collection-relations-tests-v2.log`,
`collection-normalization-final-tests.log`, and
`collection-candidate-regressions.log` beneath the critical-geometry TMP directory.
The 69 normalization tests include the new application and independent
adversarial controls; the 27 relation tests include production collection
source replay, guard/resource failures, flat maps, and mass-power checks.

The core-only release build took 3m00s and produced
`target/release/deps/librustred-1c1049495d0c9e72.rlib`; the matching direct Symbolica
library is `libsymbolica-04d533078f6a1027.rlib`. These linkage suffixes identify
this receipt, not stable build interfaces. The alias reader links as follows:

```bash
nix develop --command bash -c '
  export TMPDIR=/common/dev/rustred/TMP
  flock TMP/locks/build-1.lock taskset -c 48-51 rustc --edition=2024 -C opt-level=1 \
    TMP/critical-geometry-20261008/oracle/alias_census.rs \
    --extern rustred=target/release/deps/librustred-1c1049495d0c9e72.rlib \
    --extern serde_json=target/release/deps/libserde_json-764c963f555eb7fe.rlib \
    -L dependency=target/release/deps \
    -L native=target/release/build/gmp-mpfr-sys-d5e4317be2c2f047/out/lib \
    -o TMP/critical-geometry-20261008/oracle/alias-census
  RAYON_NUM_THREADS=1 taskset -c 48-51 \
    TMP/critical-geometry-20261008/oracle/alias-census
'
timeout 120s taskset -c 52-55 env SYMBOLICA_HIDE_BANNER=1 RAYON_NUM_THREADS=1 \
  TMP/critical-geometry-20261008/diagonal_collection diagonal \
  TMP/terminal-compression-20261008/catalog-h-wide-ordinary-measured.rrbin \
  TMP/terminal-compression-20261008/catalog-fg-wide-ordinary-measured.rrbin \
  TMP/terminal-compression-20261008/catalog-bmw-wide-ordinary-measured.rrbin \
  TMP/terminal-compression-20261008/catalog-x-wide-ordinary-measured.rrbin
```

The later final production release build completed in **3m11s**; its log is
`collection-final-release-build-v2.log`. The production wrapper uses `opt-level=3`
and release core `librustred-c1b5cf29c3a1a26b.rlib`, SHA256
`d64d2ed874f1054502d745cb5b4fc22f9de9021a7e2d1a526ae5c48506b2c5ec`.
This is a different measured engine from the earlier alias-only thin driver;
the scratch and production timings must not be conflated. Its calls are:

```bash
timeout 120s taskset -c 52-55 env SYMBOLICA_HIDE_BANNER=1 RAYON_NUM_THREADS=1 \
  TMP/critical-geometry-20261008/production_collection scalar-only \
  TMP/terminal-compression-20261008/catalog-h-wide-ordinary-measured.rrbin \
  TMP/terminal-compression-20261008/catalog-fg-wide-ordinary-measured.rrbin \
  TMP/terminal-compression-20261008/catalog-bmw-wide-ordinary-measured.rrbin \
  TMP/terminal-compression-20261008/catalog-x-wide-ordinary-measured.rrbin
timeout 120s taskset -c 52-55 env SYMBOLICA_HIDE_BANNER=1 RAYON_NUM_THREADS=1 \
  TMP/critical-geometry-20261008/production_collection scalar-subset \
  TMP/terminal-compression-20261008/five-loop-standard-v2/circuit/state-0000000000000007.rrbin
```

The measured commands also set `ulimit -v 2097152` in their parent shell, as
recorded in the linked critical-geometry note. To repeat the independent exact
four-loop comparison, build `oracle/verify_production.rs` with the matching
native Symbolica and serde_json libraries, then run `oracle/verify-production`;
it reads the frozen census and actual exported production equations/maps only.

For the fresh oracle checks, generate the fixed selected TSVs using
`oracle/prepare_fresh.py`, then run the prebuilt ignored offline test once with
`fresh-h.tsv` and once with `fresh-bmw.tsv`:

```bash
nix develop --command bash -c '
  export TMPDIR=/common/dev/rustred/TMP
  export LD_LIBRARY_PATH=/nix/store/iyz6ihfqdw0dsz1ds04wqh7af1csfrg3-flint-3.6.0/lib
  export FORM_PATH=/common/dev/rustred/FOR_REFERENCE_ONLY_DO_NOT_PUSH/form5-hep/install/bin/form
  export VAKINT_4L_CANDIDATE_ORACLE_FORM_PATH="$FORM_PATH"
  export VAKINT_4L_CANDIDATE_TERMINAL_INPUTS=/common/dev/rustred/TMP/critical-geometry-20261008/oracle/fresh-h.tsv
  taskset -c 48-51 \
    FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop/target/release/deps/experimental_rustred_4l_tests-bad36e260dac1fe0 \
      offline_fmft_candidate_terminal_catalog --exact --ignored --test-threads=1 --nocapture
'
```

The matching FORM is 5.0.1 (`Jul22 v5.0.1-30-gc9e09c3`). The literal Nix paths
are local runtime receipts, not portable installation advice. Initial failed
attempts were retained: missing time binary, missing standalone macro crate
namespace, an incorrectly unqualified Symbolica-name scan, and missing FLINT
runtime path. The corrected `*-fixed` receipts above supersede them; in
particular, the stale `census-v2.json` is not the final formal-rank evidence.

Selected SHA256 receipts:

| Artifact | SHA256 |
| --- | --- |
| Native oracle census reader `oracle/census-full` | `1832c6934f2ef6c986f388ccfb9e17ad651de2a40874c64e268bef128f0c050f` |
| Corrected oracle census JSON | `3e8e4b11dd1b3dea56170d3386b94a9cea612b1de73d45f4f08fb06f40c1c364` |
| Fresh FMFT test executable | `a1475c9c38570249fa08f5482770f5cf8cfb11fcd793715de3dd4f5b0de15e92` |
| Supplemental fresh PR9 log | `14e65ad614dcb5ba9c3eea82ab8c22ef71bc5dcdebae216e215a5a22135ef6c8` |
| FORM executable | `da2afd34a62587046e3db21cbfba4bb7e7dada50a23f433088ad2e44ab031163` |
| Autonomous `oracle/alias-census` executable | `a1c91a576be1d3a54857bd4a307bf1ec27366137e3a5dcfca63e3cc52987c643` |
| Alias census JSON | `234676e55dda5d3b844323da62fd6218a3e45ee77f8a53c05a819c8bacbc835a` |
| Autonomous `diagonal_collection` executable | `96c40c061dbfa3c3d787ab9516672b6fe770b5b2a24699b239c8e48fa4036c0e` |
| Diagonal pilot JSON | `28c0711d61dbd58d94f54f52ed8ad0311b3b2967ee330cb0bc7832afb413510d` |
| Production wrapper executable | `72483a11b6a150199a9dbf5d66a37578454cc13c0c7f1cabf807fae4e45ad932` |
| Production four-loop JSON | `a2cda2d30c76d51c2c0a3e71ecbaf440e1f054f8f7e2a55976eda778f1ab94e9` |
| Production five-loop scalar JSON | `63a0cb5ff9bdf126c35144221d0f1e9fcc2af3e8617184ae35369d89147dfaca` |
| Independent production FMFT comparison JSON | `7a617fdf179ba8f1af2223d4278c4bc3762088aa736591e929472cb3ce8e95c1` |
| Independent production comparison executable | `8715d9fee133c72b704dcdb85375878c2ae74745bea466100a307bb61cb4205e` |

Full source hashes, warm/cold checks, native alias witnesses, source row origins,
and exact replay conditions remain in the named receipts. None of these runs
modified Vakint assets or a user campaign.
