# Four-loop ordering probe

This note records bounded ordering experiments on the H momentum family. The ordering is a solver input, not a
family or topology selector. The example driver now passes the same ordering
permutation to the source-port audit, so replay and publication use one
coherent ordering authority.

All runs in the first table used the release `spired-generate-four-loop-h` example,
six workers, and Symbolica-backed exact replay. **Correction:** its original
example helper retained symbolic common mass `m`, despite its unit-mass name.
These were not literal-unit-scale timings. The helper and canonical external
TOML now use literal mass squared one; the other C++ comparison examples keep
their intended symbolic scale. None of the
runs produced a durable artifact; publication remains fail-closed.

The rule counts and uncovered boxes below refer only to the first rejected
sector in each campaign, not the complete four-loop family census.

| coordinate permutation | result |
| --- | --- |
| natural (implicit) | 60/63 rules replayed and descended; 7 uncovered boxes; three affine target/exceptional diagnostics |
| reverse `9,8,7,6,5,4,3,2,1,0` | 68/70 replayed and descended; zero uncovered boxes; two affine target/exceptional diagnostics |
| rotate `1,2,3,4,5,6,7,8,9,0` | 55/60 replayed and descended; zero uncovered boxes; five affine diagnostics |
| even/odd `0,2,4,6,8,1,3,5,7,9` | solver stopped earlier on an unsupported nonlinear exceptional intersection |

The reverse ordering is therefore a useful candidate for the eventual bounded
portfolio, but it is not a closure result. The remaining two affine rules are
lower-dimensional ownership strata. They cannot be represented honestly as
rectangular boxes: adding their bounding boxes would over-cover points where the
affine equality is false. A complete four-loop artifact still needs the generic
affine application-domain and exact mixed-stratum coverage path described in
the four-loop closure plan unless another fully replayed subset proves coverage.
No H-specific rule is hard-coded here.

## Sufficient-subsystem follow-up

The source-port gate now discards an unsupported affine candidate only if the
remaining replayed, strictly descending coordinate cells and finite terminals
independently cover the complete mathematical sector. The discarded candidate
contributes neither an identity nor coverage. The ordinary artifact installer
and cold loader independently recheck that retained subsystem. A missing finite
point or infinite region prevents omission and retains the detailed affine
diagnostics. Tests cover affine targets, affine exceptional guards, both kinds
of gaps, durable reload, and reductions on the affine equality itself.

A new six-worker reverse-order release run, steered by Python and still using
the old symbolic-mass binary, took **111.000102 seconds** and passed the earlier
redundant-candidate obstruction. It stopped later at sector `0010011001`:
91/94 rules replayed and descended, one uncovered box, and three affine
candidates that therefore could not be discarded. No artifact was written.
This is progress to a later verified boundary, not four-loop closure. Subsequent
performance runs must use the corrected literal-unit-scale external input.

## Literal-unit-scale generic CLI run

Revision `a435599` was built with `cargo build -p rustred-app --bin rustred
--release --locked -j2`. Python steered the resulting executable directly:

```console
target/release/rustred family-close \
  --input examples/input/four_loop_h.toml --input-format toml \
  --permutation 9,8,7,6,5,4,3,2,1,0 --n-cores 6 \
  --output /tmp/rustred-family-close-release-2RvQXc/h.rr
```

This is a full-family generation/publication attempt, not a selected-sector
probe. The process completed in **162.272127 seconds wall time**, using
**726.054749 CPU-seconds** and **746,156 KiB peak RSS**. It exited with the
typed execution error (code 8), emitted no artifact bytes and created no output
artifact. These are single shared-host observations, not controlled scaling
benchmarks. Compilation is outside the process interval.

The first rejected sector is again `0010011001`: **91/94** candidate rules
replayed and descended, **one uncovered box**, and three unsupported affine
candidates (46 exceptional, 54 target, 67 target). Each retains the equality
`1 + n4 - 2*n7 = 0`; the last additionally fixes `n8 = 0`. These are the
first rejected sector's counts, not counts for the complete family. The
literal-unit input therefore confirms that the affine artifact extension is
needed for this ordering. The exact infinite domain and a bounded extension
design are recorded in
[the affine-domain study](research/four_loop_affine_artifact_extension.md).

Fresh-process release controls through the same CLI, with caller-supplied
families and one worker, passed generation, cold inspection and reduction:

| family | generation | cold inspection | cold reduction |
| --- | ---: | ---: | ---: |
| K1 | 15.333 ms | 4.556 ms | 4.998 ms |
| K3 | 19.431 ms | 8.918 ms | 9.111 ms |

Targets were `[3]` and `[2,2,1]`. All six subprocesses returned zero. These
small controls are not substitutes for four-loop closure or Gregor's benchmarks.
