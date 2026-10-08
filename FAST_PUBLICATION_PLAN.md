# Fast saved-campaign publication and optional deep verification

## Requested outcome

Make independent deep verification optional and disabled by default, and remove
the measured low-hanging publication bottlenecks. Preserve accurate encountered
rules/terminal inventories, checkpoint binding, truthful assurance labels and
the explicit master-refinement workflow. Do not change production artifacts or
restart campaigns without a separate request.

## Diagnosis

THE_ONE R≤4, D≤9 publication completed with 335/335 queries verified and 938 raw
terminal candidates. Full publication took 14,135 seconds, including 10,332 s
serial structural checks and 2,286 s native reinspection. It read 60,115,005 domains
and 596,470,375 edges. A 15 s low-frequency profile of the serial stage found
Region::nonempty 31.4%, Region::with 15.56%, Region::minus 4.24%, memmove 15.28%, plus
substantial allocation overhead. That deep verifier certifies coinductive
dependency coverage, not strict descent or termination.

## Delegation and implementation

1. `finite_feedback_core`: separate ordinary inventory extraction from the deep
   graph verifier. Preserve exact encountered inventory; do not replace it with
   every declared payload terminal. Bind request/owners/checkpoint, require the
   requested saved scope complete, and report ordinary trusted-scope publication
   distinctly from independent verification. Optimize unnecessary Route/successor
   replay and contention if the existing matcher supports a classification-only
   census. Old checkpoints without stored inventories need an honest census
   fallback, not invented inventory statistics.
2. `finite_feedback_app`: Rust/CLI/Python default-off deep-verification option,
   mode-bound resume and no-op decisions, clear inspector/dashboard assurance and
   progress fields. Explicit deep must not silently reuse fast-only publication.
   Refinement remains separately requested and must not trigger graph verification.
3. `collection_workflow_audit`: remove avoidable allocations/work in exact lattice
   union checks with differential tests. Afterwards independently audit the other
   lanes. Its own mathematical optimization is reviewed by root/core, not itself.
4. Root: coordinate shared files/builds, integrate, profile release controls,
   verify public lifecycle and unchanged production inputs, document and deliver.

## Acceptance

- Accurate identical encountered rule/terminal sets on small and four-loop
  controls between ordinary and deep publication; compare bounded five-loop
  controls and a representative saved THE_ONE checkpoint when practical.
- Explicit independent-validation status, no misleading PASS/certified claim
  on the ordinary path, no termination/minimality claims on either path.
- Corrupt/mismatched/incomplete checkpoints fail safely; cancellation and
  resume cannot publish partial inventories. Previously completed artifacts
  remain inspectable during a new attempt.
- Deep verifier mutation/containment tests remain valid; differential lattice
  tests include empty cells, unbounded axes, intersections and resource budgets.
- Default-off/explicit-on CLI and Python tests, inspector progress and resume
  mode changes. Release before/after measurements exclude compilation and use
  matching inputs/resources; no fresh full campaign is needed.
- Separate implementation and independent audit, formatting/focused tests,
  scoped commit and push. Preserve unrelated HEPKit/notebook changes and all
  private/untracked campaign/reference material.

An incremental online inventory is desirable but not a prerequisite for the
first low-risk improvement if adding it would expand into a new checkpoint
architecture. Record any remaining census/load cost explicitly rather than
promising instant publication.

## Full-size gate: measured context-cache bottleneck

The first corrected full THE_ONE census was stopped cleanly in its scratch
directory after 64,783 of 13,105,672 jobs. It is censored, not a successful
publication timing. A 10-second 99 Hz user-CPU profile put 95.30% of samples in
RustRed's thread-owned Symbolica context helper: linear cache lookup and full
cleanup scans dominated, rather than mathematical matching. Add a narrow
follow-up fix: pointer-indexed source/local identities, Weak-backed pointer
lifetime safety, idempotent already-local reuse, and bounded incremental
cleanup. Keep Symbolica's existing context-cloning operations; no CAS kernel,
rule changes, new reconstruction or wider solver work. Core implements;
the separate auditor reviews ownership/cleanup/aliasing and tests. Validate
release cache controls and saved-campaign controls before rerunning THE_ONE.

## Completion receipt

Implementation pushed in `e39b072f` and `d68b3eea`; optimized release built and
independently audited. The corrected full THE_ONE scratch publication completes
successfully in 1,508.13 s on 16 workers with 50.39 GiB peak RSS. It enumerates
13,105,672 unique Apply scopes and reproduces all 938 raw terminal keys, their
normalization payload, 6,851 encountered-rule count and classification-event
counts. Native bytes outside the seed-order vector are identical to the old
publication. Individual encountered rule IDs were not saved by the old artifact
and are not claimed to have been compared. Production inputs/pointers unchanged.

Final four-loop fast/deep controls take 2.429/8.158 s; scalar five-loop controls
137.128/137.481 s. Exact output and Python publish/refine/upgrade/extend lifecycle
checks pass. Large-input loading/preparation and the remaining census are still
real costs: no incremental inventory architecture was added. The full comparison,
measurement boundaries, audits and known unrelated test failures are recorded
in `docs/research/fast_publication_2026-10-08.md`.
