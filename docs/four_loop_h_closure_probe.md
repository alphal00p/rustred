# Four-loop H closure probe

This report records the first release attempt to turn the generic RustRed
source-port solver into a durable four-loop artifact. The family was supplied
as ordinary data (the first nine physical H propagators plus the auxiliary ISP
`D10` at power zero); no topology name is consulted by the solver.

```text
target/release/examples/spired-generate-four-loop-h \
  /tmp/rustred-four-loop-h-artifact.rr 6
```

The run classified all ten-index sectors with Symbolica's unrestricted zero
analyzer, shared the ordinary source system across six workers, and attempted
`SourcePortAudit::install_complete`. Publication correctly failed closed:

```text
sector [false, false, false, false, false, true, true, false, true, true]
60/63 replayed, 60/63 descending, 7 uncovered boxes
rule 41 stored guard geometry: non-coordinate exceptional ownership remains unsupported
rule 42 stored guard geometry: coupled affine ownership is not yet supported by the artifact bridge
rule 48 stored guard geometry: coupled affine ownership is not yet supported by the artifact bridge
```

This is not evidence that the IBP search itself is complete, nor is it an
artifact. It identifies the next generic implementation requirement: preserve
the affine exceptional cases produced by discovery, compile their integer
charts/ownership into the artifact verifier, and only then retry all sectors.
The core must remain family- and topology-neutral; the H constructor is merely
an example input while `examples/input/four_loop_h.toml` is the canonical
user-supplied representation.

The artifact bridge now reports these failures as typed
`UnsupportedAffineOwnership` diagnostics. Each diagnostic retains the sector,
the fixed-coordinate face, the exact coupled equations, and whether the
unsupported domain is a target or an exceptional branch. This is deliberately
diagnostic only: rectangular hulls and sampled affine points are not accepted
as coverage proofs, so publication remains fail-closed until an affine-aware
application-domain carrier and integer-lattice coverage proof are implemented.
