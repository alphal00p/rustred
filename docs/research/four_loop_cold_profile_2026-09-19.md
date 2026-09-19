# Four-loop cold loading and first-point application profile

Date: 19 September 2026. This diagnostic explains the difference between the
public backend's expensive first call and its much faster cached calls. It does
not generate new IBPs or change the reducer. The candidate programs are tested
programs, not unrestricted certified closing artifacts.

## Workload and boundaries

An optimized standalone Rust client links the same release RustRed, Symbolica,
and Vakint libraries as the public scalar benchmark committed to GammaLoop as
`c5db07983a83e3d9eed6331727aad40212030512`. It constructs the same H/X full-parent
input with the first physical propagator cubed, then asks Vakint's existing
matcher to route it. Both the input and matched index vector are exactly
`[3,1,1,1,1,1,1,1,1,0]`; the routing witness and family fingerprint are checked.
The target is checked not to be a declared terminal.

Three boundaries are measured separately:

1. Read the existing **uncompressed** saved candidate bundle into memory.
2. Call `load_candidate_bundle`, including TOML parsing, exact coefficient
   construction, family reconstruction, and normal reducer admission checks.
3. Call bare `CandidateReducer::reduce_unit_mass` with an initially empty point
   cache, then repeat the identical call five times.

No FORM, rule generation, master-catalog lookup, numerical evaluation, or new
certification is performed. Unlike the public Vakint backend, this diagnostic
does not include gzip inflation, numerator lowering, projection onto the FMFT
master basis, epsilon expansion, or numerical master substitution. Comparing
these numbers directly with public end-to-end FMFT timings would be misleading.

Each repeated decomposition is checked exactly equal to the initial result.
Each repeat performs precisely one cache hit, no new rules, and no coefficient
coalescing. No new public profiling or cache-control API was introduced.

## Measured results

| Quantity | H | X |
|---|---:|---:|
| Raw TOML bytes | 240,831,356 | 537,049,880 |
| Raw read wall time | 0.381895 s | 0.447804 s |
| Decode/load wall time | 19.418371 s | 50.901307 s |
| Decode/load process CPU | 19.24 s | 50.31 s |
| First point wall time | 13.614155 s | 79.563839 s |
| First point process CPU | 13.50 s | 78.79 s |
| Repeats, microseconds, in order | 85, 58, 49, 49, 49 | 141, 107, 96, 86, 79 |
| Repeat median | 49 µs | 96 µs |
| First-point rule applications | 27,496 | 88,134 |
| First-point internal cache hits | 5,586 | 15,065 |
| First-point coefficient coalescing additions | 1,235,725 | 7,592,596 |
| Cached integral count | 27,882 | 88,579 |
| Cached coefficient terms | 1,974,954 | 10,264,920 |
| Cached coefficient bytes | 94,292,876 | 486,504,902 |
| Output terminal coefficients | 340 | 437 |
| Declared terminals | 386 | 445 |

The X profiled process completed successfully in 133.49 seconds wall time,
using 120.12 seconds user plus 11.86 seconds system CPU. Peak RSS was
12,954,624 KiB, with no swapping. This includes initialization, loading,
application, repeats, and cleanup; it is not a peak-memory measurement specific
to one phase. Per-phase RSS values retained in the logs are only snapshots.

These are single diagnostic runs on a shared AMD EPYC 9754 host, not paired
statistical benchmarks. CPU affinity was 88–93, with nested compute pools
capped at one; the actual H/X computation was serial. Compilation and dependency
resolution were excluded. The client used optimized release dependencies and
`rustc -C opt-level=3`. H used DWARF sampling; X used lower-overhead flat
sampling. These differences and shared-host noise preclude interpreting the
H/X ratio as a controlled comparative benchmark. The very small repeat times
are below the process-CPU tick resolution, so only wall times are meaningful
there.

## X phase-isolated samples

Sampling used `perf record -B -N -m 64 -F 99 -e cpu-clock:u --clockid
CLOCK_MONOTONIC`, without escalation. The client prints Linux monotonic-clock
phase boundaries; `perf report --time START,END --no-children --no-inline
--call-graph none` restricts the report to the corresponding interval.

There are **3,763 load samples and 7,166 first-point application samples**, with
zero reported lost samples. Percentages are exclusive self samples, not
inclusive call-tree costs. The sampled event covers userspace CPU only, not
kernel time or sleeping/I/O.

Largest loader self symbols:

| Symbol | Self samples |
|---|---:|
| `_int_malloc` | 12.04% |
| Symbolica `Token::parse_with_atom_info` | 8.50% |
| TOML `array::on_array` | 6.09% |
| TOML lexer `into_vec` | 2.95% |
| Symbolica `AtomView::normalize` | 2.82% |

Largest first-point application self symbols:

| Symbol | Self samples |
|---|---:|
| RustRed `validate_polynomial_on_map` | 3.52% |
| Numerica integer comparison | 3.29% |
| Symbolica `heap_division_packed_exp` | 3.15% |
| Numerica integer GCD | 2.93% |
| `memmove` | 2.80% |
| `cfree` | 2.80% |
| `malloc` | 2.64% |
| `_int_malloc` | 2.32% |
| Numerica `IntegerRing::try_div` | 2.29% |
| Symbolica polynomial content | 1.65% |
| Symbolica polynomial division degrees | 1.65% |
| Symbolica polynomial GCD | 1.62% |

The application long tail contains further exact arithmetic, polynomial GCD,
multiplication, cloning, and allocation routines. These percentages do not
establish a single dominant call stack: this is a flat profile. The counters
independently demonstrate millions of coefficient combinations across hundreds
of output terminal keys.

## Interpretation and next work

The observations support two distinct improvements:

- Native Symbolica binary coefficient I/O can remove the current text parsing,
  expanded-expression conversion, and much of the intermediate allocation from
  initialization. This is a loading improvement, not an application speedup.
- Canonical terminal handling can potentially reduce the number and size of
  exact coefficients propagated through the recurrence DAG. The current X
  reduction retains 10.26 million cached coefficient terms before mapping its
  437 output terminal keys onto Vakint's much smaller master basis.

Neither observation proves the speedup of a future implementation. In
particular, binary I/O alone cannot remove the measured 79.6-second X
first-point application cost. Conversely, the existing point cache is already
very fast; the public warm backend timings also include projection, epsilon
expansion, and evaluation outside this diagnostic's boundary.

This profiling work introduces no alternate rule-application backend. The
current method is retained while binary I/O and terminal deduplication are
developed and measured separately.

## Evidence and collection caveats

Local, untracked evidence is under `TMP/four-loop-cold-diagnostic/`:

- `client.rs`, `build-flat.log`, and `client-flat`: diagnostic source/build.
- `h.log`: complete H phase counters and successful exact repeated checks.
- `x.flat.log`, `x.flat.time`, and `x.flat.perf.log`: successful X results.
- `x.flat.perf.data`: complete flat sample recording.
- `x.apply.profile.txt` and `x.load.profile.txt`: symbols with at least 0.5%.
- `x.apply.profile-all.txt` and `x.load.profile-all.txt`: complete tables and
  individual sample counts.

H's initial DWARF recording completed the mathematical workload, but profiler
postprocessing was unexpectedly expensive. Only that postprocessing was
interrupted after the client had printed all successful comparisons. Its
recording is retained as `h.dwarf-slow-postprocess.perf.data`; the profiler
wrapper's wall time must not be used as an H solver timing.

The earlier `x.perf.*` files describe failed profiler launches while the prior
DWARF profiler still held mapping resources. No X reduction ran in those
attempts. The completed X run is explicitly the `x.flat.*` set, which exited
successfully. No permission escalation or system-wide profiler setting change
was used.
