# Saved-campaign terminal collection integration — 2026-10-08

## What changed

The existing `saved_campaign.py refine` workflow now invokes the previously
library-only full-U alias and diagonal ordinary-IBP collection. Mathematical
preparation, exact composition, native persistence and application remain in
Rust/Symbolica. Python only selects a published source and supervises the
native phase. No oracle coefficients or new CAS implementation were added.

The new `TerminalCollectionPlan` composes finite local reductions with exact
family-qualified collection maps. The application-aware public loader returns
`MasterReductionArtifact`; its `apply_terminal(family, key)` includes those
maps. The explicitly named `load_master_relation_session` returns only the
underlying finite search cursor. New manifest-v2 publications cannot silently
be consumed as old uncollected v1 packages. Existing v1 finite publications
remain valid inputs for a new refinement.

Additional `--collection-artifact` arguments contribute other families' finite
sessions. Their native inputs are copied into the resulting package, not kept
as external runtime dependencies. Primary prior collection proofs survive
scope extension; peers' separate collection overlays are not imported as an
arbitrary knowledge merge. See the full
[application and lifecycle contract](../campaign_master_reduction.md#native-terminal-collection-and-application).

## Release results

The frozen inventories are the same as in the preceding
[cross-family study](cross_family_terminal_collection_2026-10-08.md). These are
finite output inventories, not newly generated four-/five-loop closing rules.

| Quantity | Four-loop H/FG/BMW/X | Frozen five-loop inventory |
| --- | ---: | ---: |
| Original keys | 74 | 829 |
| After local normalization | 74 | 651 |
| After existing finite local IBPs | 65 | 608 |
| Scalar/dotted full-U classes | 22 | 355 |
| Numerator/other retained outputs outside scalar collection | 0 | 253 |
| New diagonal equations | 2 | 1 |
| Final output keys | **20** | **607** |
| Collection preparation | 0.128 s | 2.319 s |
| Native CLI process, load through publication | 0.901 s | 20.242 s |
| Process CPU time, user + system | 0.87 s | 20.00 s |
| Peak RSS | 15,380 KiB | 345,996 KiB |
| New native collection payload | 47,515 bytes | 150,098 bytes |
| Independent cold public-artifact load | 0.537 s | 17.724 s |
| 100 complete application passes | 0.000613 s | 0.009277 s |

Measurements use `cargo build --release --locked -p rustred-app --bin rustred
--features capacity-dispatch`; compilation (4m42s for this build) and fixture
packaging are excluded. Native CLI processes ran on physical CPU52 and CPU53,
one worker each, with nested compute pools capped at one. Independent read-only
verification used CPUs54 and55. The shared host had other users' builds; these
are single-run observations, not a statistically established speedup. The
application timing measures borrowed flat-map lookups, not a whole integral
reduction or numerical master evaluation. Five-loop cold time is largely the
existing finite-session load (15.318 s in the independent control).

The four-loop inputs are four immutable finite native sessions, wrapped as
portable finite-session fixtures without claiming campaign coverage. The
five-loop input is an existing standard-profile, circuit-assisted publication;
its assistance policy is retained in the comparison. Source files were checked
unchanged. A new source search or a different assistance/profile choice is a
different workload and need not have these terminal counts.

## Independent mathematical/application checks

The verifier cold-loads the public artifact and compares every original map
against independently composed original finite-session maps and the standalone
collection API. All **74/74** four-loop and **829/829** five-loop maps agree.
All outputs belong to the retained set; retained outputs also present in the
raw inventory apply as identities. Explicit inherited guards and common-mass
coefficient exponents are checked. All 253 five-loop numerator free keys remain
unchanged.

For four loops, 65 raw keys present in the previously authenticated FMFT census
also agree exactly with those formal master expressions under `d=4-2*ep`.
The other nine raw keys have exact predecessor-composition verification, not a
new direct FMFT check. There is no numerical five-loop oracle in this control.
Neither result proves minimality, master independence, or additional family
closure. Inherited finite-session identities retain their generic-parameter
authority; the collection does not invent exceptional-dimension certificates.

A real SIGINT during the five-loop `terminal_collection` phase left a paused
native checkpoint and no completed artifact. Resuming completed collection and
published 607 outputs. The previously completed ordinary search was retained.
The interrupted process took 18.01 s through its safe stopping boundary; the
resume took 18.19 s including cold input loading.
Independent reinspection confirmed all 829 maps exactly equal the continuous
run; finite cursor content and the collection payload are byte-identical.

The public Python lifecycle control also passed after the physical-arity fix:
publish a small genuine four-loop scope at R≤1,D≤4; run plain `refine`; repeat
it as a no-op; request R≤2,D≤5; observe the old artifact as stale for that new
request; solve and publish without new relation-source generation; explicitly
refine again; and inspect the final package. The old collection is retained
during extension. This is a workflow control, not the complete four-loop
performance workload above. Receipt: `workflow-validation.json` and its exact
per-command logs. Final optimized CLI SHA256:
`b4130da7a7853fd087b3fa0c75d7554ebb80f0652c5b085c40fa6dc947a65ba3`.

## Evidence and reproduction

Local untracked evidence is under `TMP/collection-workflow-20261008/`:

- `native_control.py`, `package_controls.rs`, source receipts, and native CLI
  `.receipt.json`, `.time`, `.events.jsonl` and stdout/stderr logs;
- `four-loop-output`, `five-loop-output`, `five-loop-interrupted`;
- `verify_application.rs`, fingerprint-resolved linker, and
  `{four,five}-application-audit.{json,time}` containing all exact output maps;
- `core-tests-v3.log`, `core-envelope-tests.log`, `app-master-tests-v2.log`,
  Python steering/table test logs and release compilation receipts.

Representative native invocation, using actual published finite packages:

```bash
target/release/rustred walk-master-reduce \
  --artifact /path/to/H-publication --directory /path/to/collection \
  --normalization-profile standard \
  --collection-artifact /path/to/FG-publication \
  --collection-artifact /path/to/BMW-publication \
  --collection-artifact /path/to/X-publication
```

The Python saved-campaign wrapper sets the required nested-pool environment
automatically. For direct native execution, set `RAYON_NUM_THREADS`,
`OMP_NUM_THREADS`, `OMP_THREAD_LIMIT`, `OPENBLAS_NUM_THREADS`, `MKL_NUM_THREADS`
and `BLIS_NUM_THREADS` to `1`. An initial scratch harness invocation without
`OMP_THREAD_LIMIT=1` failed preflight before creating an output; it is not a
mathematical failure or a successful timing.

The public fresh-campaign lifecycle gate additionally exposed an existing
capacity-dispatch inventory boundary issue: storage-width padded coordinates
were exported as physical integral indices. Its narrow correction validates
inactive zero padding and exports only the family's actual axes; the core
arity checks remain strict. The complete public run/refine/extend/inspect
receipt is now successful and is recorded in `CODEX_PROGRESS.md`.
