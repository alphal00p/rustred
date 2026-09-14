# Additional SpIRed reference fixtures

Date: 2026-09-14. Two additional supplied workloads now match the unchanged
C++ reference using independently generated Rust rules. This is `solveSector`
parity, not a certified family-closing artifact or complete PM-suite acceptance.

Subsequent checkpoint: the previously failing full `fam1_12` and `fam1_111`
runs now pass with integrated affine search and native joint-ideal normalization.
See [the affine-case results](spired_affine_results.md). Their failure descriptions
below are retained as historical evidence of what motivated that implementation.

## Exact results

| Workload | Requested sectors | Exact sector rules | Concrete residuals |
| --- | ---: | ---: | ---: |
| `bc4PMRad1` | 38 | 856 | 16 |
| `fam_cosmo` | 1 (`10101`) | 15 | 4 |

All exact RHS coefficients, coordinate guards, and sector signs match C++.
An independent native C++ binary reader verifies every residual integral key,
not merely the number of residuals. Rule files, preliminary-rule files, and
residual outputs are byte-identical between requested one and six workers.
The cosmology fixture selects one sector, so its actual worker count is one
in both runs; that check is not a multicore speedup measurement.

`bc4PMRad1` retains the noninteger offset `ep2` in its first denominator power.
Integer-coordinate sector signs and source seeding follow the original
reference conventions. `fam_cosmo` keeps `d,M1,M2,M3,s` symbolic, with `s=p²`.
Its comparison uses an explicit reference-notation alias `dot[p,p]` to `s`,
only after Rust generation. No reference equation, numerical substitution,
stored trace, or oracle ordering enters either search.

Source generation gives nine ordinary/LI rows for `bc4PMRad1` and six for
`fam_cosmo`. Neither input has removed cuts. All family constructors remain
example data outside the topology-generic solver.

## Repeated release comparison

Seven fresh-process pairs per setting alternate Rust/C++ launch order. Both
executables use the same prefixes of physical CPUs `2,3,4,5,6,8` on the shared
AMD EPYC 9754 host. Oracle comparison is disabled. Every output is checked
afterward against the independently validated baseline; all 70 timed runs
exit successfully and all 3,290 checked output files agree byte-for-byte.

| Workload | Workers | Rust end-to-end median | C++ end-to-end median | Rust generation + output median |
| --- | ---: | ---: | ---: | ---: |
| `bc4PMRad1` | 1 | 736.2 ms | 1804.9 ms | 718.9 ms |
| `bc4PMRad1` | 2 | 412.9 ms | 929.2 ms | 398.1 ms |
| `bc4PMRad1` | 4 | 220.5 ms | 409.3 ms | 206.2 ms |
| `bc4PMRad1` | 6 | 179.9 ms | 272.0 ms | 165.3 ms |
| `fam_cosmo` | 1 | 47.9 ms | 90.3 ms | 35.3 ms |

Rust's `bc4PMRad1` process speedup is 4.09× at six workers. Median paired
Rust/C++ wall ratios are 0.402, 0.441, 0.540, and 0.674; the cosmology ratio
is 0.529. These are empirical comparisons, not a guarantee for every run.
The six-worker Rust run at repeat three takes 0.792 s and is retained in the
sample. Its summed numerical phase rises to 2.080 s and symbolic phase to
1.108 s, with unchanged rules/cases/rows. The cause of this phase-specific
variance is unresolved; it must not simply be attributed to host load.

End-to-end wall is measured with a monotonic clock around the identical
GNU-time/affinity/timeout launch wrapper, including its small overhead.
Compilation is excluded; Rust uses `--release --locked`, and C++ uses the
unchanged GCC 14.4 `-O3 -DNDEBUG` executable. The original C++ `bc4PMRad1`
program has no separate campaign timer. C++ additionally writes compressed
binary rule databases, whereas Rust writes text and statistics, so output
formats are not identical. The machine is shared, not exclusively reserved;
the apparent C++ speedup above six is not an algorithmic scaling claim.

Median process CPU times for `bc4PMRad1` at 1/2/4/6 workers are
0.71/0.76/0.71/0.86 s Rust versus 1.77/1.79/1.54/1.41 s C++.
Median peak RSS ranges from 6,192–9,264 KiB Rust and 9,256–12,368 KiB C++.
These are process measurements, not an allocation or general memory-scaling
proof. Native initialization/source preparation is separate from campaign
time and varies between the correctness run and paired processes.

## Phase observations

One release serial correctness run of `bc4PMRad1` spends 520.379 ms in summed
sector solving: 324.713 ms in symbolic search (62%), 170.091 ms in numerical
search (33%), 12.492 ms in exception extraction, and 5.892 ms in geometry.
Preconditioning adds 4.617 ms and writing adds 12.641 ms. It visits 850 symbolic
cases and 22 numerical cases, generating 19,424 symbolic rows. Six numerical
cases reduce; 16 remain residuals.

The cosmology sector spends 26.048 ms solving: 13.040 ms in symbolic search,
11.782 ms in numerical search, 0.957 ms in exceptions, and 0.115 ms in geometry.
It visits 14 symbolic and five numerical cases, generating 374 symbolic rows.
One numerical case reduces; four remain residuals. These are phase counters,
not a primitive-level profile attributing time to native GCD, GPLU, or replay.

Oracle validation is outside all generation counters. It is included in the
correctness processes' wall time and requires retaining their solutions; those
process times and peak RSS must not be presented as oracle-free production
measurements. The paired timing study uses separate oracle-free runs.

Across the seven paired serial runs, median summed `bc4PMRad1` solving is
694.5 ms: 420.9 ms symbolic search (61%), 238.3 ms numerical search (34%),
16.7 ms exceptions, and 7.9 ms geometry. Median cosmology solving is 34.3 ms,
split chiefly between 17.9 ms symbolic and 15.0 ms numerical search. These
are separate observations from the initial correctness-run profile above;
no samples are mixed to construct a faster total.

## Historical full-manifest failures before affine integration

The unchanged `fam1_12` attempt finishes 38/40 sectors and writes 963 rules.
The missing sectors are `110000111` and `110011100`, whose C++ outputs have
68 and 74 rules. The first failure is the genuine coupled face `n8=n9`, with
the cuts fixed to one, `n3..n6=0`, and `n7=1` (C++ one-based indices).
The full diagnostic process takes 0.54 s and exits with a typed geometry error.
It is not a completed 1,105-rule timing comparison.

The unchanged `fam1_111` attempt finishes 119/132 selected sectors and writes
8,384 rules. Its first reported error is sector `111000010100011` on `n9=n11`.
Twelve missing sectors coincide with the reference's coupled-case census;
the additional sector `111110100010011` contains a coordinate conjunction
arising from joint polynomial constraints (diagnosed below). All produced-sector rule counts agree with C++,
but these partial outputs have not undergone the full native equation audit.
The 20.31 s diagnostic exits with an error and no success summary; it is not
comparable to the complete 105.43 s C++ run producing 10,333 rules.

At that checkpoint, the standalone `AffineCase` service supplied canonical
integral charts, exact containment, integer-divisibility pruning, and tangent
matching, but was not connected to sector search. The planned integration had
to translate sources before chart substitution, keep transverse source seeds,
and omit the integer-empty `2*n5-2*n6=1` reference rule with an exact proof.
That integration now passes the full comparisons described in the newer
[affine report](spired_affine_results.md). Genuine congruence charts remain
typed unsupported.

### The thirteenth sector needs joint polynomial normalization

An isolated run reproduces the extra failure in `111110100010011`. With
`a=n4` and `b=n14` in C++ notation, the exact exceptional conjunction is

```text
E1 = -32 -38b -14b² +33a +39ab +6ab² +3a² +3a²b = 0
E2 = -31 -19b -2b² +34a +16ab +2a² = 0
E3 = -7 -b +8a = 0.
```

The linear equation gives `b=8a-7`. Substitution gives
`E2=2(a-1)(a-2)` and `E1=2(a-1)(204a²-433a+226)`.
The second candidate `a=2,b=9` fails `E1` (value 352), leaving only
`a=b=1`, exactly the coordinate conjunction in C++. This is an exact
algebraic observation which, at that checkpoint, was not implemented by the queue.

The earlier geometry checked equations individually and could not perform
joint normalization. Native Symbolica `GroebnerBasis::new` reduced-ideal
normalization was identified as the generic cold-path cure, exposing `a-1,b-1`
before coordinate admission without custom elimination or bounded enumeration.
Its subsequent implementation tests and full-sector replay now pass; those
results are recorded separately from this historical diagnostic.

## Validation and local evidence at that earlier checkpoint

- 122 focused solver tests pass, including 12 affine-geometry and two new
  per-job configuration tests.
- 29 example tests pass, including exact family recipes, source counts,
  reference aliases, and all 16 explicit `fam1_112` ordering inputs.
- Release K1 and K3 top-sector checks pass, as do all 617 `vac3` and 802
  `fam1_11` equations at serial/requested-six workers. The top-sector K3
  check is not a replacement for its earlier full-family validation.
- Independent implementation, mathematical, and output audits pass.
- `cargo check --locked --workspace --all-targets` passes. The existing
  unused-method warning and unused shared-helper warnings in the single-case
  diagnostic example remain nonfatal.

Ignored local evidence directories:

- `target/spired-next-fixtures.o86wsq`: Rust outputs, run script, phase data,
  independent output audit, and paired timing study.
- `target/spired-pm-remaining.XyEHkz`: unchanged C++ outputs, build/run
  provenance, and exact requested-manifest census.
- `target/spired-pm-residuals.lIrUEC`: native binary residual readers and
  exact sector/integral-key lists.

Reference source and binary outputs remain uncommitted. Reproduction inputs
and commands are documented in the [Rust examples](../examples/rust/README.md).
