#set page(paper: "a4", margin: 18mm)
#set text(size: 10pt)
#set par(justify: true)
= RustRed linear-combination representation study
ValentinHirschi — 2026-10-28

Do not adopt this checked point-update LC migration. Exact results and guards are preserved, and 4L peak RSS improves, but representative timings do not show balanced gains: FG improves, H has a small paired regression, complete 1L–3L request ranges overlap, and 5L cold medians are higher. Production retains BTreeMap; no dependency pin or PR is needed for this result.

== Tested change and method

Native c2ca815 and official Symbolica 9cd5767 are the baselines. Both routed map arms use official 9cd; the routed LC arm uses the private a0866fd extension. Both replay executables use a0866fd to compile all four backends. The candidate adds the source-reviewed private checked-update/reservation API a0866fd to Symbolica and uses LC sorted-vector storage for sparse caches, closed master combination, routed entry collection, and root input/output accumulation. Existing public and persistence map boundaries remain, with conversion cost inside request timing. Factorized caches and symbolic coefficient-polynomial internals remain outside this change. This tests binary-search point updates, not bulk merging of sorted child expansions.

All coefficient validation, fallible arithmetic, resource admission and coalescing order use the existing CoefficientContext operations. LC's infallible ring arithmetic is never used for coefficient addition. Integral order, zero removal, foreign-context rejection and full original ordered nonzero guards are retained. The extra API provides transactional fallible occupied-key replacement and explicit reservation; seven API tests, five adapter tests and four principal/root differential tests passed.

Matching campaign builds use rustc 1.98.1, optimization 3, fat LTO, one codegen unit, no debug assertions/overflow checks, CPU 196 and one algebra thread. Three fresh processes per arm each perform three inner repeats. Each process is summarized first; reported medians and ranges then use the three independent process medians. Ranges are observed ranges, not confidence intervals. Routed arm order rotates between pristine c2, modified Native map control and LC. A separate replay compares original, entry, borrowed-get_mut maps and checked LC within each executable.

== Complete guarded requests

Values are milliseconds for 1L–3L and seconds for routed 4L/5L: median [minimum, maximum] of process medians. The low-loop map control includes the shared borrow/iterator changes; routed pristine c2 independently checks that control boundary.

#table(columns: (1fr, 2fr, 2fr), [Request (ms)], [Map control], [LC],
[1L], [0.151 [0.146, 0.161]], [0.148 [0.146, 0.154]],
[2L], [0.479 [0.434, 0.487]], [0.505 [0.453, 0.550]],
[3L], [29.913 [29.853, 30.562]], [30.349 [29.628, 32.136]],
)

#table(columns: (1.2fr, 1.7fr, 1.7fr, 1.7fr), [Routed phase (s)], [Pristine c2], [Map control], [LC],
[4L FG], [0.339 [0.324, 0.348]], [0.338 [0.333, 0.339]], [0.318 [0.317, 0.327]],
[4L H], [4.505 [4.473, 4.548]], [4.577 [4.536, 4.694]], [4.559 [4.520, 4.596]],
[5L cold roots], [4.128 [4.020, 4.429]], [4.128 [4.058, 4.879]], [4.237 [4.136, 4.523]],
[5L cold batch], [3.940 [3.936, 4.216]], [3.923 [3.916, 4.716]], [4.245 [3.964, 4.384]],
[5L warm roots], [0.586 [0.574, 0.662]], [0.549 [0.542, 0.795]], [0.580 [0.580, 0.761]],
[5L warm batch], [0.287 [0.266, 0.355]], [0.256 [0.248, 0.298]], [0.266 [0.261, 0.269]],
)

FG uses 3,839 rules and 22,906 coalescing additions, returning 15 masters and 465 ordered guards. H uses 27,496 rules and 336,860 additions, returning 21 masters and 1,620 guards. FG's matched LC/pristine cold ratios are 0.935–0.981; H's are 1.010–1.012. Thus overlapping arm ranges do not erase the paired directions. The map-control comparison is neutral for H. Warm 4L requests are approximately 0.08–0.21 ms.

The 5L fixture is the frozen toy-plus-authenticated-ladder two-root control, not a standalone cold ladder measurement: cache is empty at root-sequence entry, toy runs first, and both roots then share the session. Batch gets a separate clear. Load, owner preparation, oracle construction, wire checks and report serialization are excluded from native call timers and reported separately. Every arm applied 9,965 rules and returned 16 batch masters with 140,311 ordered guards. LC/pristine paired cold ratios are 1.002–1.054 for roots and 1.006–1.079 for batch. All three paired cold estimates are slower, although arm ranges overlap. The warm-batch improvement against pristine is also present in the map control, so it does not establish an LC-specific benefit.

== Replay, memory and exactness

Within the map-storage replay executable, scale-plus-add medians in milliseconds are:
#table(columns: (1fr, 2fr, 2fr), [Trace], [Borrowed map], [Checked LC],
[1L], [0.978 [0.956, 1.097]], [0.843 [0.751, 0.904]],
[2L], [3.254 [3.253, 3.257]], [3.163 [3.158, 3.165]],
[3L], [16.914 [16.839, 18.102]], [17.269 [17.204, 19.072]],
)
Collection-only 3L regresses: borrowed map 11.350 [11.229, 11.422] ms versus LC 11.509 [11.451, 11.544] ms. Repeated-key synthetic controls also regress; iteration improvements at 1L/2L do not consistently extend to 3L. These neutral/regression cases and both production-storage replay cohorts remain in the complete metadata. Borrowed lookup removes avoidable occupied-key cloning, so gains over the original map alone cannot be attributed entirely to storage layout.

Uninstrumented lifetime peak RSS is neutral at approximately 6/9/231 MiB for 1L/2L/3L. At 4L, median RSS falls from approximately 268 to 259 MiB (FG), and 1,349 to 1,300 MiB (H). These are whole-process peaks including setup, oracle, wire and output work, not timed-call memory. The first 5L receipt alone is 2.23 GB; its process peaks near 7.3 GiB and takes 413 s despite roughly 4 s cold calls. Direct unbuffered JSON output is a measured harness-throughput limitation, outside the reduction timers.

No valid allocation-reduction claim is available. The Stage A allocation scope failed its unchanged diagnostic-clear condition; the old output did not identify the veto flag. Source review found no ledger defect, but Symbolica can create detached initialization workers, so untimed warmup alone does not prove global allocator quiescence. That is a possible contaminant, not the established cause. Stage B allocation builds remain unlaunched.

All accepted 1L–5L results agree in complete exact coefficients, unresolved sets, original ordered guards and work counters. All 27 routed processes passed; the 18-process replay also agrees in full trace bytes and inventories. Each routed process tests one fresh completed native record's encode/decode/reencode, binding rejection, truncation and corruption checks; full encoded bytes agree across arms. This does not claim every cache record was reencoded. Authenticated program artifacts were loaded; existing application-cache payloads were neither loaded nor rewritten. The real ladder input comes from the fresh authenticated lowering-only capture 71dcd042; no IBP/reduction/cache writes ran in that preparation. Final12 and held-out fixtures remain excluded.

The held-off heavy-job window has two disclosed incidental reads: a brief packaging ELF hash receipt was written at 03:26:29 UTC inside H's first map-control invocation, and one unexpected full read of the first 2.23 GB own 5L receipt on CPU 197. Internal timed-phase overlap and impact are unknown. Every sample is retained; no repeat or exclusion is based on performance. Full raw receipts, source/build identities, tool/profile evidence and qualification metadata remain in the workspace.

== Decision and limits

Keep the production map representation. The focused candidate is correct on the admitted checks, but the balanced measurements do not establish a representative speed advantage without regressions. The small memory gains do not satisfy the requested conditional speed criterion. Retain the measured prototype privately; no upstream migration or dependency update is proposed.

These results apply to the tested checked point-update migration and the admitted fixtures. They do not establish a general result about every LC design, a checked two-way bulk merge, physical q0/quadratic workloads, or Symbolica polynomial division. The latter's map frames are a separate algebra track.
