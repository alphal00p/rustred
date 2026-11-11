# T211 opt-in single-target numeric frontend: tested source reference

This is the exact historical frontend source, preserved byte-for-byte for the rr-consolidate engine owner. It is deliberately outside Cargo targets and examples. It is not an engine port, a current-core compilation claim, or a general 20-slot API test.

## Minimal change from r9

minimal-vs-r9.patch has two hunks:

1. With T211_FORCE_SINGLE_TARGET=1, a fully fixed Case remains in the ordinary pending-case queue instead of being collected into the bulk numerical union.
2. main accepts exactly20 slots and instantiates only run::<20>; the other r9 arity instantiations were removed to make this frontend build small.

The source-order dispatch already existed in r9. Once the fixed Case stays in the ordinary queue it reaches the existing solve_case_with_source_order_and_observer call, so source_order reverse is actually used for that target and the configured symbolic SparseTargetOnlyFactorized (TOF) backend applies. The old numerical union instead entered NumericalStarted with the numeric Sparse path. No new solver, IBP basis, seed constructor or backend ladder is implemented here. Default input behavior is unchanged when the opt-in variable is absent.

The pending-case path preserves the original family, mathematical order, original source basis and configured seed radius2. A failed/exhausted search remains an error; it is not interpreted as a residual/master or a knockout. Existing r9 case/exclusion/cover machinery is otherwise unchanged.

## Existing execution evidence, reused without replay

The tested opt0 build completed in27.92s against frozen rr-probept core rlib7d069f9a (full bindings in PROVENANCE.json). The earlier opt2 compiler attempt hit its181.56s guard; it remains historical.

At the fixed P input in PROVENANCE.json, the source-order reverse + single-target TOF path completed one rule with4136RHS and2597retained source descriptors, no exceptions/residuals/native descent failure. Exact materialization took6.831s; that is NOT the whole job time: the actual monitored job was39.28s (native total38.335s). The previous bulk numeric-union attempt was capped at120.99s. Shared-host timings are indicative.

The existing independent standard RS2/RS3 report passed at P61=2^61-1 and d=17/7,19/7,23/7, all26115columns, zero raw residuals3/3 and mutations rejected3/3. It recomputed the witness using30authenticated original IBP basis rows and2597retained descriptors, without native weights. This is one fixed relation, not whole-T211 closure or a master-count proof. No build, solve or verification was repeated for this source handoff.

## Integration boundary

rr-consolidate owns the engine/API port and its tests. Use the tiny patch and existing native call as the porting reference; do not add this full legacy harness as a production Cargo example. The source remains pinned to the old tested rlib interface, including r9 frontend machinery and the20-slot assertion. Current-core integration needs an explicit native option with the same default-off behavior and ordinary source/witness/guard/descent obligations.

No binary, family/table/certificate input, license content or live-worker file is included. Source and base hashes are recorded in PROVENANCE.json; the copied source retains exactly b14694d19d0c6d716c7129b40d653881c24cd4cc76d0cab95e872c3e42fe091e.
