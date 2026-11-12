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

## Version 4 (M5 batch 5)

`crates/rustred-core/data/p/p-isp-preferences.v4.json` replaces v3 (P_PRESENTATION_VERSION 4):

- 57 generic-lane presentations (41 4L + 16 3L) with explicit-row ISPs (`{"rows": [[left, right], ..]}`,
  `IspPreference::Rows`: arbitrary massless-shaped momentum products `left.right` over the node's
  L loop momenta and `q`), chosen by route-basis-2's `basis_opt2` (= `basis_opt` with a
  free-row pool, the massless filter and inherited parent slot forms; model opt_all2). Recorded
  in the p_integral_dev workspace as `SCRATCHPAD/route-basis/data/pref_rows_all2.json`
  (= `p-isp-preferences.v4.json` of route-basis-2, notes/route-basis.md session 2).
- One entry for the integer Custom node 151 (`p_node_0105...0204`, bebe, owner
  L4T08-d261f9c2d25e2dac): basis b3 (the removed J1 lines kept as squares; ISPs `k3.(k1+q)`,
  `(k3-k4).q`, `k3.q`). With an entry an integer Custom node is presented in its own family
  (the top's sector momenta); its payload must be generated in that basis
  (`artefacts/m5/bebe-basis/README.md` in the workspace: R1 vote order with k4, k1 last +
  Astra's CA264 portfolio, 351 rules).

Measured on the batch-2/batch-4 bases (1 thread, 18 4L cells, values identical): rows alone -18.1 %
edges vs v3 (route-basis-2), bebe in b3 alone -33.4 % edges vs batch 4 (bebe-basis); the batch-5
18-cell matrix measures both together (notes/m5-batch5.md).
