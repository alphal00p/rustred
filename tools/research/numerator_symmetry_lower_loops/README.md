# Lower-loop numerator-symmetry validation

This is an isolated validation of the numerator-polynomial symmetry provider,
not a new IBP generation or a numerical-master calculation. All generated
evidence is written under a caller-supplied new `TMP/` directory. Existing
Vakint data and production campaigns are read-only inputs.

The frozen three-loop input is the complete packaged K6 artifact, not one
chosen subsector. The four-loop input is all four packaged H, FG, BMW and X
programs and their complete terminal-normalization sidecars: 1,155 raw keys
and 74 family-local normalized output keys. These 74 outputs are not a claim
of 74 independent masters; cross-family identifications are reported
separately. Earlier 19- or 20-terminal physics controls must not be substituted
for this full inventory.

The experiment compares an exact baseline with the same baseline plus the new
symmetry equations. It drains provider requests/equations only and asserts the
ordinary-source cursor remains at zero. Thus newly generated ordinary IBP rows
cannot be credited to the symmetry provider. Every emitted witness is replayed,
then cold-imported in the native provider codec. All original raw key maps are
checked after composition; outputs must be remaining terminals only.

Negative results are retained. In particular, K6 has only positive corner keys,
and the four-loop package already has a verified quadratic-numerator symmetry
normalizer. No improvement from a more general polynomial provider is therefore
an entirely possible outcome. A lower count in a different representative
convention is not by itself a new exact dependency.

No FMFT values, approximate zero tests, custom algebra or hard-coded reduction
relations are used. Source byte hashes are checked before and after the run.

Measured results and qualifications are in
[`docs/research/numerator_symmetry_lower_loop_results_2026-10-21.md`](../../../../docs/research/numerator_symmetry_lower_loop_results_2026-10-21.md).
In particular the rank-one probe inventory is an explicit experiment input,
not yet an implicit part of the production raw-inventory provider flag.

## Commands

Compile `study.rs` against the same optimized `rustred`, `rustred_app` and
`serde_json` libraries and native GMP/Blake3 paths used by the frozen CLI. The
helper itself uses `-C opt-level=3 -C lto=thin`; this does not rebuild the engine.
The evidence receipts pin its exact binary and source inputs.

```bash
python -B tools/research/numerator_symmetry_lower_loops/prepare.py \
  --package /path/to/vakint/crates/vakint/data/rustred \
  --output /path/to/new-evidence/inputs

python -B tools/research/numerator_symmetry_lower_loops/run.py \
  --binary /path/to/optimized-study \
  --inputs /path/to/new-evidence/inputs \
  --output /path/to/new-evidence/run1 \
  --cpus 240,241,242,243

# Repeat the same command with --output /path/to/new-evidence/run2, then:
python -B tools/research/numerator_symmetry_lower_loops/summarize.py \
  /path/to/new-evidence
```

Paths and CPUs are caller-selected. Every output directory must be new; source
programs and campaigns are never overwritten. The existing native normalization
preparation defaults are retained, while explicitly caller-controlled native
I/O, relation, and application grants are unbounded for these trusted fixtures.
