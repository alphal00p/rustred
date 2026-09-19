# Bounded finite-terminal IBP experiments

These September 19, 2026 experiments test whether a small, independently
generated ordinary-IBP system supplies additional identities among the
terminals left after [exact parameter equivalence](vacuum_parametric_terminal_equivalence.md).
They do not regenerate the saved sector programs, use FMFT relations as input,
certify those programs or try to prove a minimal master basis. Production
behavior is unchanged.

## Remaining targets and exact method

The shipped equivalence plan reduces 1,155 family-local declared keys to 179:

| Parent | Retained representatives | Positive-power representatives | Numerator-bearing keys |
| --- | ---: | ---: | ---: |
| H | 52 | 22 | 30 |
| FG | 26 | 16 | 10 |
| BMW | 37 | 17 | 20 |
| X | 64 | 19 | 45 |
| Total | 179 | 74 | 105 |

Each of the 105 numerator keys has five unit denominator powers, one index
equal to minus one, and zero elsewhere. They are not high-rank numerator
examples. The count is family-local, not a number of independent masters.

For each seed, generate the 16 ordinary four-loop vacuum IBPs with the existing
generic source generator. Specialize integer indices, retaining dimension as
an exact symbolic parameter. Keep the full homogeneous identity. Normalize
only keys already covered by the independently verified parameter-alias plan;
all other keys remain auxiliary integral columns. In the zero-pruned variant,
remove an integral only when the existing exact sector analyzer returns a
sealed `ProvedZero` certificate. Inconclusive results remain columns.

Put auxiliary columns first and terminal columns last, with the saved integral
ordering within each block. Symbolica's native sparse reducer performs exact
forward elimination over rational polynomials. A terminal-block pivot would
exhibit an identity among the retained terminals. Thus the objective is a
small finite relation, not a new parametric closure search.

Independently regenerate the source matrix and check `L * U = A` exactly; these
experiments perform no back substitution. Also check that the L rows corresponding to independent
source insertions form an invertible triangular matrix. Both conditions matter:
the first alone only proves one containment between row spaces. Retain source
and pivot conditions, coefficient contexts and the separately verified
parameter/zero equalities. Native Symbolica owns all algebra and elimination.

Any proposed identity must descend among existing terminals and pass exact
offline-catalog comparison only **after** its independent construction. No
catalog expression selects a seed, equation, column or pivot. No weighted
terminal replacement is installed merely because a sampled value agrees.

## Completed radius-zero experiments

Radius zero means the canonical terminal keys themselves are the only seeds.
All generated rows independently pivot in the auxiliary block in both runs.
The terminal-only row space is therefore zero for these specific source pools.

| Parent | Rows / exact rank | Auxiliary columns, original | Auxiliary columns, proved-zero pruning | Additional terminal identities |
| --- | ---: | ---: | ---: | ---: |
| H | 832 | 1,702 | 1,680 | 0 |
| FG | 416 | 953 | 926 | 0 |
| BMW | 592 | 1,357 | 1,333 | 0 |
| X | 1,024 | 2,312 | 2,289 | 0 |

The original exact GPLU intervals were respectively 34.953, 18.000, 30.860 and
69.812 ms; regenerated-source replay took 150.428, 77.216, 111.285 and
316.488 ms. Loading, plan preparation and evidence I/O are excluded from those
phase timers. Whole processes took 2.00, 0.86, 1.40 and 3.87 s.

The second run proves 9/9/8/8 zero supports and removes 108/90/62/83 row terms
in H/FG/BMW/X. Zero analysis and pruning take 19.628/11.771/13.648/22.806 ms;
GPLU takes 32.793/17.900/28.872/54.518 ms. No declared terminal is proved zero,
and rank remains unchanged. Every process exits successfully with exact replay
and the selected-L invertibility check passing; no budget is exhausted.
Independent audits cover both variants.

## Completed FG radius-one experiment

The next bounded test adds signed unit shifts around FG's 26 canonical seeds,
deduplicates the result and respects the input's inactive-ISP restrictions.
Its terminal block still consists of those 26 original representatives, not
the larger seed set.

| Quantity | Completed observation |
| --- | ---: |
| Deduplicated seeds | 397 |
| Ordinary source rows | 6,352 |
| Rows zero after exact simplification | 144 |
| Remaining rows / exact rank | 6,208 / 5,690 |
| Auxiliary / terminal columns | 8,321 / 26 |
| Proved-zero supports / removed row terms | 59 / 4,521 |
| U / L nonzeros | 163,537 / 150,990 |
| Additional terminal identities | 0 |
| Exact GPLU / regenerated replay | 14.427 / 19.040 s |
| Whole-process wall / user / system | 35.01 / 34.53 / 0.21 s |
| Peak RSS reported by GNU time | 205,824 KiB |

Exact regenerated-source replay and the selected-L invertibility check pass.
The rank and relation statements are over `Q(d)`. Nonconstant pivot guards are
retained in the evidence; this is not a proof at each exceptional dimension.
The process finishes below its explicit 120-second, 10,000-row, 20,000-column
and two-million-L/U-nonzero bounds. These bounds govern this scratch diagnostic,
not the uncapped C++ comparison or a general per-operation memory guarantee.
No larger-radius campaign is implied by this result.

## Interpretation and next bounded check

These misses do **not** establish master independence or show that a generated
parametric rule is wrong. New sources or other exact integral equalities can
change the row space. In particular, these runs normalize only already-known
terminal keys. Newly generated positive-power auxiliary keys have not yet been
quotiented by parameter equivalence, so equivalent auxiliary integrals can
remain distinct columns.

The next small test applies the existing exact parameter-equivalence service
to the union of generated FG radius-zero columns and original terminals before
elimination. A quotient class containing an original terminal must remain a
terminal class even if its least representative happens to be a newly generated
auxiliary key. Retain the exact binding to an original terminal; do not invent
new masters or silently discard the requested terminal subspace. This test is
separate from a deeper source search and must pass the same source replay.

A finite nonminimal basis remains acceptable. A production weighted-terminal
expansion service is deferred until a bounded experiment actually finds useful
relations at an acceptable cost. The existing exact unit-weight normalization
and all four-loop numerical acceptance results remain valid independently.

## Reproducibility

All runs use optimized clients linked against the measured RustRed/Symbolica
release libraries, CPUs 74–79 and one compute worker; compilation is outside
every reported timer. Saved programs, orderings and terminal sets are unchanged.
This is a shared-host diagnostic, not a confidence-bound benchmark.

Source clients, inputs and executable hashes, source rows, exact A/L/U matrices,
proof logs, resource measurements and audits are retained locally in:

- `TMP/terminal-finite-relations.smcJg7/` (radius zero);
- `TMP/terminal-finite-zero.M7UXVf/` (radius zero plus proved zeros);
- `TMP/terminal-finite-radius1.MfBs8n/` (FG radius one).

These reference/evidence directories are not distributed as RustRed source.
