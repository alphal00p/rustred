# Independent five-loop R6/D10 campaign

## Prepared scope and status

The separate campaign is staged at
`/shared/codex-2/rustred/campaigns/five-loop-minimal-r6-d10`.
It has **not been launched**. THE_ONE's production inputs, amendments and
checkpoint are untouched. Saved owner rules, routes and overlays are reused;
neither IBPs nor a traversal ledger have been regenerated or copied.

The request is the original massless **Feynman-gauge** QCD input-template union,
intersected with scalar negative-index degree R<=6 and signed power sum D<=10.
The original templates have D=9 and D=10 bands, so the actual starting scope is
**9<=D<=10**, not every abstract integral with D<=10. It contains 116 required
domains over all 67 owners and no auxiliary helper starts:

- 41 connected physical classes, each with its D9 and D10 request;
- 18 factorized classes, each with its nested template restricted to D9--10;
- 8 convenience classes, each with conservative D9 and D10 requests.

A=sum positive powers, R=sum negative-index magnitudes, D=A-R. Consequently
R<=6 and D<=10 imply A<=16. Native projection already derives this inequality;
there is no additional independent global A cap. Dots and merged equal-momentum
propagators count with their full powers. All descendants retain their own
translated/routed bounds and are **not clipped** to the starting caps.

## Audit of the retained topology bounds

The user explicitly keeps original graph inputs, without prior numerator--
denominator cancellations. For these connected inputs the retained D10 bounds
are A<=16-V4min and R<=6-V4min; D9 uses A<=14-V4min and R<=5-V4min.

For an original connected L-loop QCD graph, incidence and Euler counting give
I=3L+E-3-V4. With Nb external boson/ghost fields and Nf external fermions, the
highest UV Taylor order is delta=4-Nb-3Nf/2. Since each derivative raises total
quadratic denominator power by at most one,

    A0 <= I+delta = 3L+1-Nf/2-V4 <=16-V4   (L=5).

The homogeneous kinetic/interaction coefficient has A0-R0=10, hence
R0<=6-V4. External fermions only tighten the conservative common bound.
Removing external legs and suppressing resulting degree-two vertices cannot
increase the original quartic count. The classifier enumerates cubic/quartic
vacuum skeletons **before** merging series classes, minimizes the quartic count
over matching realizations, and therefore uses V4min<=V4_original. Repeated
momenta do not invalidate the argument: original propagator powers are summed.

Full regeneration of that five-loop classification reproduces the committed
fixture. The 41 connected classes have V4min distribution
{0:16, 1:5, 2:13, 3:4, 4:3}. The 8 convenience classes use the wider V4=0
envelope. Native momentum-routing witnesses remain required; graph matching
alone is not the application authority.

The 18 factorized templates' older upper bounds A<=20..23 and R<=11..14 are
redundant under the new global caps. They are historical conservative allowances,
not a claim that the soft-CT construction requires those maxima. Lower-loop
factor reduction, including tensor contractions between factors, remains a
separate integration responsibility.

These are bounds for original massless renormalizable QCD inputs in Feynman
gauge, not a claim for massive on-shell counterterms, longitudinal covariant-
gauge powers, irrelevant operators, or arbitrary post-cancellation starting
representations. Internal pinches are allowed without applying these root caps.
The audit's literal-mask pinch counterchecks are preserved as negative evidence
in `TMP/r6-d10-original-topology-bound-audit.md`; they are not physical QCD
counterexamples, because a removed slot may represent several original factors.

Physical homogeneous numerator bounds and admission after RustRed's internal
shifted-denominator lowering are distinct. This preparation does not claim to
complete the deferred Vakint ingress validation or prove unrestricted five-loop
closure. The campaign itself must still discharge every required obligation.

## Preparation, resources and launch

The preparer now supports explicit recovery of a rank-scoped campaign's frozen
original templates. It verifies the source-template attachment and scope receipt
against the immutable base; it does not silently derive from THE_ONE's old
R0/D9 base or copy its amendments. The reproducible preparation command was:

```bash
python -B examples/python/prepare_rank_campaign.py \
  --source-campaign /shared/codex-2/rustred/campaigns/five-loop-rank-ladder-THE-ONE \
  --source-original-queries \
  --destination /shared/codex-2/rustred/campaigns/five-loop-minimal-r6-d10 \
  --max-numerator-rank 6 --max-power-difference 10
```

Do not repeat preparation into the existing destination. Its input selection is
`90245dd3babf801915f786b273988d351e3d843487c25ee3671842853d0c3dc6`;
queries are `38a9f4bca9a2bc215c26a6721ef3bd311a170e3d6be3330937f1e06b6c9dc93d`.

Frozen settings:

- 32 physical CPUs, IDs96--127, disjoint from THE_ONE's64--95;
- requested RAM ceiling600,000,000,000 bytes, save/stop margin5%;
- host available-memory floor150,000,000,000 bytes;
- swap-growth-only stop disabled;
- hourly checkpoints, plus orderly interruption and RAM-stop checkpoints;
- Epoch snapshot inspectors, rolling oldest-prefix publication, cut16/window76,
  31 inspection workers, G2 union reuse, lookahead256;
- no runtime or cumulative-work deadline; unknown frontiers stop explicitly;
- automatic ordinary publication, no automatic master refinement, deep
  verification disabled by default.

The RAM ceiling is the user's requested maximum, not a reservation. At each
launch the effective ceiling is min(600GB, available RAM minus150GB). The
preparation preview admitted about567GB on the shared host; this is recalculated
at launch. No ZFS/host-wide setting was changed.

The campaign contains independent copies of the tested native solver
`814745116dd52749a6eb2272774503afc762d806823924cf4c35b8e732745ce7` and publisher
`fe3f1d9167e931d9179240a61202f0c6340252b93aae9a484133d0cfe1a46050`.
This is a steering/input change, not a new Rust engine or an unmeasured claimed
solver speedup. No Rust rebuild was needed. Publisher paths resolve inside the
new campaign, not through THE_ONE.

In the user's `5L_minimal_plus_incremental_later` tab of Zellij session `rustred`:

```bash
cd /shared/codex-2/rustred
export RUSTRED_CAMPAIGN="$PWD/campaigns/five-loop-minimal-r6-d10"
export TMPDIR="$PWD/TMP" TMP="$PWD/TMP" TEMP="$PWD/TMP"
nix develop --command python -B examples/python/saved_campaign.py run \
  --campaign "$RUSTRED_CAMPAIGN"
```

The same command resumes automatically when a checkpoint exists. Ctrl+C requests
an orderly save; wait for the stopped/checkpoint message before resuming. The
current launcher and dashboard are used; the native programs are campaign-frozen.
Do not use `nix develop path:...`, which can copy large untracked campaign data.

Monitor from another terminal, or inspect after publication:

```bash
nix develop --command python -B examples/python/campaign_monitor.py "$RUSTRED_CAMPAIGN"
nix develop --command python -B examples/python/saved_campaign.py inspect \
  --campaign "$RUSTRED_CAMPAIGN"
```

## Later extension

After stopping cleanly or completing, a rank extension appends new requests and
reuses the checkpoint; it does not overwrite an earlier publication:

```bash
nix develop --command python -B examples/python/saved_campaign.py extend \
  --campaign "$RUSTRED_CAMPAIGN" --rank 7
```

An extension retains the original topology-template ceilings. Here connected
templates are already fully admitted at globalR6, so rank7 enlarges the nested
factorized requests, not the justified connected-graph envelopes. Continuing to
rank11 at D10 recovers the broader current THE_ONE template scope. It does not
promise blanket rank11 coverage for every topology. Requesting deliberately
more general connected inputs would require an explicit scope amendment rather
than silently overriding the physical-template bounds. The derived A cap grows
with the rank/D limits; no independent A16 constraint is frozen.

## Validation evidence

- 26 preparation/extension tests pass, including restoring D10 rows omitted
  from a source's initial D9 base, immutable source preservation, source-binding
  mutations, duplicate/missing attachments and append-only extension.
- The broader141-test Python staging, phase/publication and public-wrapper
  integration suite passes in13.637s.
- All48 distinct planner/checker tests pass. Full five-loop classification
  regeneration:85.074s wall/84.663s CPU, CPU18; subsequent fast tests1.785s.
- Existing optimized native CP6 test passes actual R0->R1->R2 extension,
  interrupted partial admission, cold replay and one-shot equivalence:1.27s.
- The exact Nix/public-wrapper launch command was exercised with `--dry-run`;
  no native five-loop campaign was invoked.
- Independent preparation/code and mathematical audits completed separately.

Local receipts live under `TMP/minimal-r6d10-preparation-20261014/`,
`TMP/minimal-r6d10-classification-20261014/`,
`TMP/minimal-r6d10-native-extension-audit-20261014/` and
`TMP/minimal-r6d10-independent-delivery-audit-20261014.json`.
Campaign binaries, rules and outputs remain untracked.
