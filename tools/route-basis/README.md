# route-basis: the ISP preference table of P_PRESENTATION_VERSION 3

`crates/rustred-core/data/p/p-isp-preferences.v3.json` (embedded as
`propagator::present::P_ISP_PREFERENCES_JSON`) is the output of this chooser. It gives
55 generic-lane presentations of scope 1-4 (Insertion/SeriesParallel, Triangle, Carpet;
no Custom or Scheme owner) edge-pair ISPs `p_a.p_b` and unit rows that minimise the
routed terms of the 4L reductions (numerator re-expression between presentations).
Values do not change; only the presentations, hence the routes, change.

## Cost model

`basis_opt.py`: terms of a route item = distinct monomials of `prod_j F_j(T k)^e_j` in the
owner's coordinates (exact; equals the engine's route terms with orbit canonicalisation
off, 0 mismatches on 1,991 routes). Search proxy `prod_j C(n_j + e_j - 1, e_j)`, inbound
and outbound, weighted by the measured route traffic (`routes.tsv` of census runs),
coordinate descent over the owners (2 passes). Lane contracts are enforced (ISPs linear
in the bubble/triangle loop, the carpet q-rule); pairs with `q` in both factors are not
offered (their constant `q^2` breaks the massless family admission,
`family/unit_scale.rs`).

## Reproduce v3

Inputs (recorded with SHA256SUMS in the p_integral_dev workspace,
`artefacts/m5/batch4/route-basis-inputs/`):

- LIB: the batch-2 1-4 package (`artefacts/m5/batch2/p-library-1-4`, rustred 83ef7a8c),
  only its `census.json` is read;
- three census runs of that package, 1 thread, `P_CENSUS=on P_CENSUS_SLOTS=1`
  (`p_path_census LIB benchmark_p_integrals.json <cell> <top> 1 modular_polynomial OUT`):
  P4L_NO3 L4_d2_r8, P4L_NO1 L4_d2_r8, P4L_NO2 L4_d0_r8 (files `slots.json`, `owners.tsv`,
  `routes.tsv`).

```
python3 tools/route-basis/basis_opt.py LIB OUT NO3_L4_d2_r8 NO1_L4_d2_r8 NO2_L4_d0_r8   # RB_TOP=40 RB_POOL=both RB_PASSES=2 (defaults)
python3 tools/route-basis/write_pref.py OUT/result.json NO3_L4_d2_r8/slots.json p-isp-preferences.json
```

(`mapslib.py` from `tools/m5-maps`.) The output is the v3 table up to JSON layout
(the repo file lists one node per line, sorted). A development override without a
rebuild: `RUSTRED_P_ISP_PREFERENCE=<json>` at package assembly (replaces the embedded
table; a package built with it is NOT a version-3 package).

## Measured (batch-2 package vs the same owners with the v3 table, 1 thread)

18 4L cells: values identical 18/18, edges -16..-54 % (sum -34.1 %), keys sum -21.8 %,
peak RSS sum -9.3 %; 75 1L-3L cells: values identical, edges -22.1 %.
Changing the table means a new file name and a `P_PRESENTATION_VERSION` bump.
