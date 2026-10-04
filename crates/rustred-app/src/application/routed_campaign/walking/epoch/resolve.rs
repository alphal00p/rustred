//! Native event sink. The unchanged default ships every distinct Admit to P2.
//! Private lockstep S4 can resolve stored targets through the existing immutable
//! index; it retains full canonical images and independent P2 verification.
//! No self-first, Local, MRU or rolling-refresh policy changes are introduced.
use super::super::{
    diagnostics::OptionalRefusals,
    inspection::{Effect, Event, Finished, NativeStats},
    queue::{CompactDomain, Query},
};
use super::job::{BreakReason, ErrorKind, Job, JobResult, LookupReport, Miss, NativeKind, Scope};
use super::snapshot::Snapshot;
use super::verify::QueryImage;
use rustred::solver::DomainPowerSummary;
use std::collections::HashMap;
use std::ops::ControlFlow;

#[cfg(test)]
thread_local! {
    /// Test seam (`consumer_stop_parity`): break with this reason at this
    /// emitted-event count, on the thread that runs the job (W1: inline).
    pub(super) static FORCED_BREAK: std::cell::Cell<Option<(u64, BreakReason)>> =
        const { std::cell::Cell::new(None) };
}

fn forced_break(emitted: u64) -> Option<BreakReason> {
    #[cfg(test)]
    {
        FORCED_BREAK
            .with(|f| f.get())
            .filter(|&(at, _)| at == emitted)
            .map(|(_, reason)| reason)
    }
    #[cfg(not(test))]
    {
        let _ = emitted;
        None
    }
}

pub(super) struct Resolver<'a, const N: usize> {
    emitted: u64,
    accepted: u64,
    successors: u64,
    conditional: u64,
    known_reuse: u64,
    job_duplicates: u64,
    /// Admit ordinal (0-based over Admit events of this job).
    admits: u32,
    misses: Vec<Miss<N>>,
    /// digest -> indices into `misses` (confirmed by image equality).
    exact: HashMap<u64, Vec<u32>>,
    frontiers: Vec<Vec<u8>>,
    refusals: OptionalRefusals,
    break_reason: BreakReason,
    snapshot: Option<&'a Snapshot<N>>,
    lookup: Option<LookupReport>,
}

impl<'a, const N: usize> Resolver<'a, N> {
    pub fn new() -> Self {
        Self {
            emitted: 0,
            accepted: 0,
            successors: 0,
            conditional: 0,
            known_reuse: 0,
            job_duplicates: 0,
            admits: 0,
            misses: Vec::new(),
            exact: HashMap::new(),
            frontiers: Vec::new(),
            refusals: OptionalRefusals::default(),
            break_reason: BreakReason::None,
            snapshot: None,
            lookup: None,
        }
    }

    pub fn with_snapshot(snapshot: &'a Snapshot<N>) -> Self {
        let mut resolver = Self::new();
        resolver.snapshot = Some(snapshot);
        resolver.lookup = Some(LookupReport {
            version: snapshot.version,
            published_len: snapshot.published_len as u32,
            lookup: Default::default(),
            verify: Default::default(),
            seconds: 0.0,
        });
        resolver
    }

    fn stop(&mut self, reason: BreakReason) -> ControlFlow<()> {
        if self.break_reason == BreakReason::None {
            self.break_reason = reason;
        }
        ControlFlow::Break(())
    }

    /// `(emitted, accepted)` so far (kept for a panic while assembling).
    pub fn prefix(&self) -> (u64, u64) {
        (self.emitted, self.accepted)
    }

    /// One native event. The breaking event is emitted but not accepted.
    pub fn emit(&mut self, event: Event<N>) -> ControlFlow<()> {
        let count = event.count as u64;
        self.emitted += count;
        if self.break_reason != BreakReason::None {
            return ControlFlow::Break(());
        }
        if let Some(reason) = forced_break(self.emitted) {
            return self.stop(reason);
        }
        let (successor, conditional) = match event.effect {
            Effect::Count => (false, false),
            Effect::KnownReuse {
                successor,
                conditional,
            } => {
                // An earlier emission of this job already produced its edge.
                self.known_reuse += count;
                (successor, conditional)
            }
            Effect::PreAdmittedOrthantReuse { .. } => {
                // Impossible with empty initial orthants: a protocol break.
                return self.stop(BreakReason::Protocol);
            }
            Effect::Admit {
                domain,
                successor,
                conditional,
            } => {
                // F1 step 0: one image; its native summary must exist.
                let image = match CompactDomain::try_from_domain(&domain) {
                    Ok(image) => image,
                    Err(_) => return self.stop(BreakReason::ResolverRange),
                };
                // F1 applies even to duplicates: a raw positive is not a
                // substitute for a valid canonical native query summary.
                let started = self.snapshot.map(|_| std::time::Instant::now());
                let core = match DomainPowerSummary::try_new(
                    domain.owner,
                    &domain.lower,
                    &domain.upper,
                    domain.rank,
                    domain.powers,
                ) {
                    Ok(core) => core,
                    Err(_) => return self.stop(BreakReason::ResolverSummary),
                };
                // CompactDomain is injective on the checked range: both facts
                // derive from this same native event, without expanding two
                // fresh coordinate vectors merely to repeat the projection.
                let q = QueryImage {
                    image,
                    core,
                    digest: image.digest().0,
                };
                let digest = q.digest;
                let ordinal = self.admits;
                self.admits += 1;
                let known = self.exact.get(&digest).is_some_and(|slots| {
                    slots
                        .iter()
                        .any(|&slot| self.misses[slot as usize].image == image)
                });
                if known {
                    self.job_duplicates += count;
                } else {
                    let target =
                        if let (Some(snapshot), Some(work)) = (self.snapshot, &mut self.lookup) {
                            let query = Query::new(q.core.clone(), image.phase());
                            match snapshot.lookup(
                                &q,
                                &query,
                                snapshot.published_len,
                                &mut work.lookup,
                                &mut work.verify,
                            ) {
                                Ok(found) => found.map(|(id, _, _)| id),
                                Err(_) => return self.stop(BreakReason::Protocol),
                            }
                        } else {
                            None
                        };
                    if self.misses.try_reserve(1).is_err() {
                        return self.stop(BreakReason::Alloc);
                    }
                    self.exact
                        .entry(digest)
                        .or_default()
                        .push(self.misses.len() as u32);
                    self.misses.push(Miss {
                        ordinal,
                        digest,
                        image,
                        target,
                    });
                }
                if let (Some(work), Some(started)) = (&mut self.lookup, started) {
                    work.seconds += started.elapsed().as_secs_f64();
                }
                (successor, conditional)
            }
            Effect::Frontier {
                value,
                successor,
                conditional,
            } => {
                match serde_json::to_vec(&value) {
                    Ok(bytes) => self.frontiers.push(bytes),
                    Err(_) => return self.stop(BreakReason::ResolverDiagnostic),
                }
                (successor, conditional)
            }
            Effect::Optional(d) => {
                if self
                    .refusals
                    .record(
                        d.disposition,
                        d.rank,
                        d.powers,
                        &d.lower,
                        &d.upper,
                        &d.shift,
                        d.ordinal,
                        &rustred::algebra::IndexedAlgebraError::ResourceLimit {
                            resource: d.resource,
                            requested: d.requested,
                            limit: d.limit,
                        },
                    )
                    .is_err()
                {
                    return self.stop(BreakReason::ResolverDiagnostic);
                }
                (false, false)
            }
        };
        self.accepted += count;
        self.successors += if successor { count } else { 0 };
        self.conditional += if conditional { count } else { 0 };
        ControlFlow::Continue(())
    }

    /// The result of a returned native call.
    pub fn finish(self, job: &Job<N>, finished: Finished) -> JobResult<N> {
        let (kind, stats_json, stats_events, optional, route, scope, truncated) =
            match finished.stats {
                NativeStats::Apply(stats) | NativeStats::ApplyPartial(stats, _) => (
                    if matches!(finished.stats, NativeStats::ApplyPartial(..)) {
                        NativeKind::ApplyPartial
                    } else {
                        NativeKind::Apply
                    },
                    super::super::stats_json(stats),
                    stats.events as u64,
                    [
                        stats.optional_coefficient_refusals as u64,
                        stats.optional_original_refusals as u64,
                        stats.optional_coalesced_refusals as u64,
                    ],
                    (0, 0),
                    finished.initial_overlap_scope().map(|scope| Scope {
                        anchor: scope.anchor_id as u32,
                        cut: scope.cut,
                        residual: scope.residual_powers,
                    }),
                    self.refusals.truncated(stats),
                ),
                NativeStats::Route(stats) => (
                    NativeKind::Route,
                    super::super::execution::native_stats(finished.stats),
                    stats.events as u64,
                    [0; 3],
                    (
                        stats.masks_examined as u64,
                        stats.joint_support_masks_pruned as u64,
                    ),
                    None,
                    false,
                ),
                NativeStats::ApplyG2(_, _) => {
                    // Legacy G2 publication stamps/plans are not epoch
                    // scopes. Reject even an otherwise successful result
                    // as C5, never as full coverage or a retryable panic.
                    return JobResult {
                        seq: job.seq,
                        parent: job.parent,
                        v0: job.v0,
                        kind: NativeKind::G2Residual,
                        error_kind: ErrorKind::Other,
                        break_reason: BreakReason::Protocol,
                        panic: false,
                        emitted: self.emitted,
                        accepted: self.accepted,
                        stats_events: u64::MAX,
                        successors: self.successors,
                        conditional: self.conditional,
                        known_reuse: self.known_reuse,
                        job_duplicates: self.job_duplicates,
                        optional: [0; 3],
                        route_masks: 0,
                        route_joint_pruned: 0,
                        seconds: finished.seconds,
                        stats_json: Vec::new(),
                        error: Some("legacy G2 result is unsupported by epoch".into()),
                        frontiers: Vec::new(),
                        refusals: Vec::new(),
                        refusals_truncated: false,
                        scope: None,
                        g2: None,
                        finite_replay: None,
                        lookup: self.lookup,
                        misses: Vec::new(),
                    };
                }
            };
        JobResult {
            seq: job.seq,
            parent: job.parent,
            v0: job.v0,
            kind,
            error_kind: ErrorKind::of(finished.error_kind),
            break_reason: self.break_reason,
            panic: false,
            emitted: self.emitted,
            accepted: self.accepted,
            stats_events,
            successors: self.successors,
            conditional: self.conditional,
            known_reuse: self.known_reuse,
            job_duplicates: self.job_duplicates,
            optional,
            route_masks: route.0,
            route_joint_pruned: route.1,
            seconds: finished.seconds,
            stats_json: serde_json::to_vec(&stats_json).expect("stats serialize"),
            error: finished.error,
            frontiers: self.frontiers,
            refusals: self
                .refusals
                .records
                .iter()
                .map(|value| serde_json::to_vec(value).expect("refusal serializes"))
                .collect(),
            refusals_truncated: truncated,
            scope,
            g2: None,
            finite_replay: None,
            lookup: self.lookup,
            misses: self.misses,
        }
    }

    /// A caught panic (C3): the emitted prefix survives, stats do not.
    pub fn finish_panic(self, job: &Job<N>, seconds: f64) -> JobResult<N> {
        JobResult {
            seq: job.seq,
            parent: job.parent,
            v0: job.v0,
            kind: match job.image.phase() {
                super::super::queue::Phase::Apply => NativeKind::Apply,
                super::super::queue::Phase::Route => NativeKind::Route,
            },
            error_kind: ErrorKind::Other,
            break_reason: self.break_reason,
            panic: true,
            emitted: self.emitted,
            accepted: self.accepted,
            stats_events: u64::MAX,
            successors: self.successors,
            conditional: self.conditional,
            known_reuse: self.known_reuse,
            job_duplicates: self.job_duplicates,
            optional: [0; 3],
            route_masks: 0,
            route_joint_pruned: 0,
            seconds,
            stats_json: Vec::new(),
            error: Some("symbolic inspection panicked; native stats unavailable".into()),
            frontiers: Vec::new(),
            refusals: Vec::new(),
            refusals_truncated: false,
            scope: None,
            g2: None,
            finite_replay: None,
            lookup: self.lookup,
            misses: Vec::new(),
        }
    }
}
