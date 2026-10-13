# Master-integral evaluation handoff

The detailed, authoritative handoff for extending the one-through-four-loop
epsilon expansions and evaluating the saved five-loop terminal basis is in
[Vakint's MASTERS.md](https://github.com/alphal00p/gammaloop/blob/vakint_rustred/crates/vakint/MASTERS.md).

Use RustRed `main` with GammaLoop **`vakint_rustred`**, not the older reference
checkout or the separate `feynkit` branch. The handoff covers exact master
definitions, existing MATAD/FMFT evaluations, available epsilon orders,
normalization/precision requirements, terminal extraction commands and the
external generation-54 five-loop application package. That package is not
committed to either repository.

The numerical task is separate from IBP generation and the running THE_ONE
campaign. In particular, 905 retained five-loop keys are a potentially
nonminimal basis, and finite-order master data can be insufficient when their
reduction coefficients contain epsilon poles.
