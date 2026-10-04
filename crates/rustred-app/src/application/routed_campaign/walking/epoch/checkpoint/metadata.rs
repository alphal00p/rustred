//! Borrowed metadata for the private writer. Query rows are streamed
//! individually; the full query document is bound once before the save path.
use super::super::super::{
    OwnerDomainWalkEpochPublicationOrder, OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest,
};
use super::super::ledger6::Tag;
use super::super::merge::StopReason;
use super::super::state::WalkCounters;
use super::super::store::LookupCounters;
use super::super::verify::VerifyCounters;
use super::{Digest, MEMBERSHIP_WORDS, MergeBoundary, Stream, invalid};
use crate::application::routed_campaign::matching::input::Query;
use serde::Serialize;
use serde_json::Value;
use std::io::{self, Write};

/// Already validated input identity; construct once before any memory-stop
/// save. Callers pass the complete parsed query slice used by the run, never
/// the prefix admitted so far. Restore must reparse the bound request and
/// verify this inventory before admitting any work.
pub(super) struct Identity<'a> {
    request: String,
    owners: &'a [String],
    owners_digest: [u8; 32],
    queries: &'a [Query],
    domain_limit: usize,
    event_limit: usize,
    frontier_limit: usize,
    route_domain_overcover: bool,
    g2: &'static str,
    epoch_rolling: bool,
    epoch_cut_size: usize,
    epoch_publication_order: OwnerDomainWalkEpochPublicationOrder,
    requested_window: Option<usize>,
    epoch_result_escrow_jobs: usize,
    epoch_result_escrow_bytes: Option<usize>,
    adaptive: bool,
    // Ephemeral capability gate only: never serialized into legacy scalars.
    // Finite replay can be read cold, but its one-attempt runtime account is
    // deliberately fresh-only in this first implementation.
    finite_replay: bool,
    preparation: Preparation,
    amendments: Vec<super::super::super::rescue::Parsed>,
}

impl<'a> Identity<'a> {
    pub fn new(
        request: &OwnerDomainWalkRequest,
        owners: &'a [String],
        queries: &'a [Query],
    ) -> io::Result<Self> {
        if request.publication_policy != OwnerDomainWalkPublicationPolicy::Epoch {
            return Err(invalid(
                "internal epoch identity requires epoch publication",
            ));
        }
        super::super::admit_extensions(request)
            .map_err(|error| io::Error::other(error.to_string()))?;
        if owners.iter().any(|digest| {
            digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }) {
            return Err(invalid("epoch owner digest is not canonical blake3"));
        }
        let mut stream = Stream::new(io::sink());
        serde_json::to_writer(&mut stream, owners).map_err(io::Error::other)?;
        let (_, owners_digest) = stream.finish()?;
        let amendments = request
            .amendments
            .iter()
            .map(|amendment| {
                super::super::super::rescue::parse(
                    amendment,
                    queries.first().map_or(0, |q| q.owner.len()),
                )
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
            })
            .collect::<io::Result<Vec<_>>>()?;
        let binding = super::super::super::checkpoint::epoch_request_binding(request);
        super::super::super::rescue::check_chain(&[], &amendments, &binding, queries)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        Ok(Self {
            request: binding,
            owners,
            owners_digest: owners_digest.blake3,
            queries,
            domain_limit: request.max_domains,
            event_limit: request.max_events,
            frontier_limit: request.max_frontiers,
            route_domain_overcover: request.route_domain_overcover,
            g2: request.g2_residual_anchors.name(),
            epoch_rolling: request.epoch_rolling,
            epoch_cut_size: request
                .effective_epoch_cut_size()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?,
            epoch_publication_order: request.epoch_publication_order,
            requested_window: request.epoch_window,
            epoch_result_escrow_jobs: request.epoch_result_escrow_jobs,
            epoch_result_escrow_bytes: request.epoch_result_escrow_bytes,
            adaptive: request.epoch_dispatch == crate::OwnerDomainWalkEpochDispatchPolicy::Adaptive,
            finite_replay: request.finite_replay.is_some(),
            preparation: Preparation::from_request(request),
            amendments,
        })
    }

    pub(in super::super) fn finite_replay_enabled(&self) -> bool {
        self.finite_replay
    }

    /// Bind small authenticated scalar metadata before allocating restored
    /// arrays. Aggregate allowances may change between sessions, but the
    /// saved arena must fit both its saved and the requested domain limits.
    pub(super) fn validate_saved(&self, saved: &OwnedScalars, lockstep_b: usize) -> io::Result<()> {
        if saved.schema != 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsupported private epoch scalar version; fresh state required",
            ));
        }
        if saved.record_schema != super::super::records::wire::RECORD_SCHEMA {
            return Err(invalid("epoch binary record schema differs"));
        }
        // Helper count is an execution receipt, not P2 semantics. The ordered
        // fold is identical with zero or many helpers, so a checkpoint can be
        // resumed with another valid worker partition (including W=1).
        if saved.preparation.obligations != self.preparation.obligations
            || saved.preparation.retirements != self.preparation.retirements
        {
            return Err(invalid(
                "epoch preparation logical allowances differ from checkpoint",
            ));
        }
        super::super::super::rescue::check_chain(
            &saved.amendments,
            &self.amendments,
            &self.request,
            self.queries,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        let mut previous_domain = u64::from(saved.p0);
        let mut previous_generation = 0;
        let amended_queries = saved
            .amendments
            .iter()
            .try_fold(0usize, |sum, reference| {
                let count = usize::try_from(reference.queries).ok()?;
                if reference.first_input != (self.queries.len() + sum) as u64
                    || reference.first_domain < previous_domain
                    || reference.first_domain > u64::from(saved.watermark)
                    || reference.quarantined > reference.first_domain
                    || reference.resumed_generation == 0
                    || reference.resumed_generation < previous_generation
                {
                    return None;
                }
                previous_domain = reference.first_domain;
                previous_generation = reference.resumed_generation;
                sum.checked_add(count)
            })
            .ok_or_else(|| invalid("epoch amendment cursor inventory"))?;
        let watermark = u64::from(saved.watermark);
        // Request/capability refusals must not silently select an older, smaller
        // generation. Other decode/validation/I/O failures may use a fully
        // validated previous generation, with the latest failure reported.
        if saved.request != self.request
            || saved.owner_count != self.owners.len()
            || saved.owners_digest != self.owners_digest
            || saved.total_queries != self.queries.len()
            || saved.lockstep_b != lockstep_b
            || saved.epoch_rolling != self.epoch_rolling
            || saved.epoch_publication_order != self.epoch_publication_order
            || saved.epoch_result_escrow_jobs != self.epoch_result_escrow_jobs
            || saved.epoch_result_escrow_bytes != self.epoch_result_escrow_bytes
            || self.epoch_base_window(lockstep_b)? != saved.epoch_base_window
            || self
                .requested_window
                .is_some_and(|window| window != saved.epoch_base_window)
            || (if saved.epoch_rolling {
                saved.epoch_cut_size != self.epoch_cut_size
            } else {
                saved.epoch_cut_size != 0
            })
            || saved.watermark as usize > self.domain_limit
            || saved.g2 != self.g2
            || saved.adaptive_dispatch.is_some() != self.adaptive
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "epoch checkpoint/request binding or requested domain limit differs",
            ));
        }
        if saved.walk_semantics_version != super::super::EPOCH_WALK_SEMANTICS_VERSION
            || !(1..=4096).contains(&lockstep_b)
            || saved.k >= super::super::ledger6::EPOCH_LIMIT
            || saved.watermark == u32::MAX
            || saved.p0 > saved.watermark
            || saved.watermark as usize > saved.max_domains
            || saved
                .ledger_counts
                .iter()
                .try_fold(0u64, |sum, value| sum.checked_add(*value))
                != Some(watermark)
            || saved.processed_queries > saved.total_queries
            || saved.input_frontiers > saved.processed_queries + amended_queries
            || saved.quarantined > watermark
            || saved.abandoned_obligations > saved.quarantined
            || saved.amendments.is_empty()
                && (saved.quarantined != 0 || saved.abandoned_obligations != 0)
            || matches!(saved.initial_admission, Admission::Complete)
                && saved.processed_queries != saved.total_queries
            || matches!(saved.initial_admission, Admission::InProgress)
                && (saved.k != 0
                    || saved.p0 != saved.watermark
                    || saved.ledger_counts[Tag::Pending as usize] != watermark)
            || saved.g2 == "off" && saved.walk.g2_records != 0
            || saved.imported_prefix != 0
            || saved.engine_certification_void
            || saved.self_edges > saved.edges
            || saved.closure.initial != saved.p0 as usize
            || saved.closure.initial_closed > saved.p0 as usize
            || saved.closure.total_closed > saved.watermark as usize
            || saved.closure.inspected > saved.watermark as usize
            || saved.closure.snapshot_revision > saved.closure.revision
            || !saved.closure.refresh_seconds.is_finite()
            || saved.closure.refresh_seconds < 0.0
            || [&saved.records_digest, &saved.edge_digest]
                .iter()
                .any(|digest| {
                    digest.len() != 64
                        || !digest
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
        {
            return Err(invalid(
                "epoch scalar identity, admission or inventory differs",
            ));
        }
        let runs = [
            Tag::Native,
            Tag::NativeFrontier,
            Tag::NativeError,
            Tag::Alias,
            Tag::Abandoned,
        ]
        .into_iter()
        .try_fold(0u64, |sum, tag| {
            sum.checked_add(saved.ledger_counts[tag as usize])
        });
        if runs != Some(saved.edge_runs) {
            return Err(invalid("epoch scalar ledger/run count differs"));
        }
        if saved.stop_reason.as_deref().is_some_and(|reason| {
            !matches!(
                reason,
                "frontier_stop"
                    | "error_stop"
                    | "exhausted_stop"
                    | "domain_allowance"
                    | "event_allowance"
                    | "frontier_allowance"
                    | "capacity"
                    | "ram_guard"
                    | "paused"
                    | "drained_uncertified"
            )
        }) {
            return Err(invalid("epoch scalar stop reason"));
        }
        if saved.operational_stop.as_ref().is_some_and(|stop| {
            !stop.valid() || saved.stop_reason.as_deref() != Some(stop.kind().name())
        }) {
            return Err(invalid("epoch operational stop context differs"));
        }
        validate_failure(
            saved.admission_failure.as_ref(),
            saved.initial_admission,
            saved.processed_queries,
            saved.total_queries,
            saved.stop_reason.as_deref(),
            saved.operational_stop.is_some(),
        )?;
        Ok(())
    }

    pub(super) fn owners(&self) -> &[String] {
        self.owners
    }

    pub(super) fn queries(&self) -> &[Query] {
        self.queries
    }

    pub(super) fn amendments(&self) -> &[super::super::super::rescue::Parsed] {
        &self.amendments
    }

    pub(super) fn query_rows(&self, applied: usize) -> Vec<(&Query, Option<u64>)> {
        self.queries
            .iter()
            .map(|q| (q, None))
            .chain(self.amendments.iter().take(applied).flat_map(|amendment| {
                amendment
                    .queries
                    .iter()
                    .map(move |q| (q, Some(amendment.sequence)))
            }))
            .collect()
    }

    pub(super) fn route_domain_overcover(&self) -> bool {
        self.route_domain_overcover
    }

    pub(super) fn epoch_rolling(&self) -> bool {
        self.epoch_rolling
    }

    pub(super) fn epoch_cut_size(&self) -> usize {
        self.epoch_cut_size
    }

    pub(super) fn epoch_result_escrow_jobs(&self) -> usize {
        self.epoch_result_escrow_jobs
    }

    pub(super) fn epoch_result_escrow_bytes(&self) -> Option<usize> {
        self.epoch_result_escrow_bytes
    }

    pub(super) fn epoch_base_window(&self, total: usize) -> io::Result<usize> {
        total
            .checked_sub(self.epoch_result_escrow_jobs)
            .filter(|&base| base > 0 && total <= 4096)
            .ok_or_else(|| invalid("invalid Epoch base/escrow reservation bounds"))
    }

    pub(super) fn epoch_publication_order(&self) -> OwnerDomainWalkEpochPublicationOrder {
        self.epoch_publication_order
    }

    pub(super) fn limits(&self) -> (usize, usize, usize) {
        (self.domain_limit, self.event_limit, self.frontier_limit)
    }

    pub(super) fn adaptive_dispatch(&self) -> bool {
        self.adaptive
    }

    pub(super) fn preparation(&self) -> Preparation {
        self.preparation
    }
}

/// Resolved helper reservation and complete logical scratch allowances.
/// Neither count is a cumulative-work or mathematical coverage limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Preparation {
    pub helpers: usize,
    pub obligations: usize,
    pub retirements: usize,
}
impl Preparation {
    pub fn from_request(request: &OwnerDomainWalkRequest) -> Self {
        let (obligations, retirements) = request
            .epoch_preparation_limits()
            .expect("validated preparation limits");
        Self {
            helpers: super::super::super::worker_budget::WorkerBudget::for_request(request).helpers,
            obligations,
            retirements,
        }
    }
    pub fn engine(
        self,
    ) -> Result<super::super::merge::preparation::Engine, super::super::merge::preparation::Error>
    {
        super::super::merge::preparation::Engine::new(
            self.helpers,
            super::super::merge::preparation::Limits {
                obligations: self.obligations,
                retirements: self.retirements,
            },
        )
    }
}

#[derive(Clone, Copy, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Admission {
    /// Preparation is complete, but some input queries are not yet admitted.
    InProgress,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AdmissionFailureKind {
    DomainAllowance,
    FrontierAllowance,
    Allocation,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AdmissionFailure {
    pub query_index: usize,
    pub kind: AdmissionFailureKind,
    pub message: String,
}

impl AdmissionFailure {
    fn valid_message(&self) -> bool {
        !self.message.is_empty()
            && self.message.len() <= 4096
            && match self.kind {
                AdmissionFailureKind::DomainAllowance => {
                    self.message == "scheduled domain allowance"
                }
                AdmissionFailureKind::FrontierAllowance => {
                    self.message == "retained frontier allowance"
                }
                AdmissionFailureKind::Allocation => self.message.contains("allocation"),
                AdmissionFailureKind::Refused => self.message.starts_with("input: "),
            }
    }
    pub fn reason(&self) -> StopReason {
        match self.kind {
            AdmissionFailureKind::DomainAllowance => StopReason::DomainAllowance,
            AdmissionFailureKind::FrontierAllowance => StopReason::FrontierAllowance,
            AdmissionFailureKind::Allocation => StopReason::RamGuard,
            AdmissionFailureKind::Refused => StopReason::ErrorStop,
        }
    }
    pub fn from_error(index: usize, error: super::super::AdmissionError) -> io::Result<Self> {
        use super::super::AdmissionError as E;
        let kind = match &error {
            E::DomainCap => AdmissionFailureKind::DomainAllowance,
            E::FrontierCap => AdmissionFailureKind::FrontierAllowance,
            E::Alloc(_) => AdmissionFailureKind::Allocation,
            E::Refused(_) => AdmissionFailureKind::Refused,
            E::Internal(message) => return Err(io::Error::other(message.clone())),
        };
        let (message, _) = error.stop();
        let failure = Self {
            query_index: index,
            kind,
            message,
        };
        if !failure.valid_message() {
            return Err(invalid(
                "epoch admission diagnostic exceeds bounded metadata",
            ));
        }
        Ok(failure)
    }
}

pub(super) fn validate_failure(
    failure: Option<&AdmissionFailure>,
    admission: Admission,
    processed: usize,
    total: usize,
    stop: Option<&str>,
    operational: bool,
) -> io::Result<()> {
    if let Some(failure) = failure {
        if !matches!(admission, Admission::InProgress)
            || failure.query_index != processed
            || processed >= total
            || !failure.valid_message()
            || (!operational && stop != Some(failure.reason().name()))
        {
            return Err(invalid("epoch admission failure binding differs"));
        }
    } else if matches!(admission, Admission::InProgress)
        && (matches!(
            stop,
            Some("error_stop" | "domain_allowance" | "frontier_allowance")
        ) || stop == Some("ram_guard") && !operational)
    {
        return Err(invalid(
            "epoch admission error stop is missing its diagnostic",
        ));
    }
    Ok(())
}

/// Every reference is held for the same save; no root/frontier array clone.
pub(super) struct Inputs<'a> {
    pub identity: &'a Identity<'a>,
    pub admission: Admission,
    pub rows: &'a [Value],
    pub frontiers: &'a [Value],
    pub stop: Option<StopReason>,
    pub operational_stop: Option<&'a super::stop::Stop>,
    pub admission_failure: Option<&'a AdmissionFailure>,
}

#[derive(Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scalars<W, L, V, S, C = super::stop::Stop> {
    pub schema: u32,
    pub request: S,
    pub owner_count: usize,
    pub owners_digest: [u8; 32],
    pub walk_semantics_version: u32,
    pub preparation: Preparation,
    pub record_schema: u32,
    pub lockstep_b: usize,
    pub epoch_base_window: usize,
    pub epoch_result_escrow_jobs: usize,
    #[serde(deserialize_with = "serde::Deserialize::deserialize")]
    pub epoch_result_escrow_bytes: Option<usize>,
    // Omitted for legacy lockstep CP6 bytes. Rolling binds the original cut
    // size independently of the persisted, worker-width-independent window.
    #[serde(default, skip_serializing_if = "is_false")]
    pub epoch_rolling: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub epoch_cut_size: usize,
    #[serde(
        default,
        skip_serializing_if = "OwnerDomainWalkEpochPublicationOrder::is_default"
    )]
    pub epoch_publication_order: OwnerDomainWalkEpochPublicationOrder,
    pub k: u64,
    pub watermark: u32,
    pub p0: u32,
    pub max_domains: usize,
    pub max_events: u64,
    pub max_frontiers: u64,
    pub ledger_counts: [u64; 8],
    pub walk: W,
    pub lookup: L,
    pub verify: V,
    pub closure: super::super::super::descendant_closure::Counters,
    pub edge_runs: u64,
    pub edges: u64,
    pub self_edges: u64,
    pub records_digest: String,
    pub edge_digest: String,
    pub initial_admission: Admission,
    pub total_queries: usize,
    pub processed_queries: usize,
    pub input_frontiers: usize,
    pub stop_reason: Option<S>,
    pub operational_stop: Option<C>,
    pub admission_failure: Option<AdmissionFailure>,
    pub amendments: Vec<super::super::super::rescue::AmendmentRef>,
    pub quarantined: u64,
    pub abandoned_obligations: u64,
    pub g2: S,
    pub imported_prefix: u64,
    pub engine_certification_void: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive_dispatch: Option<super::super::dispatch::adaptive::Saved>,
}

pub(super) type OwnedScalars = Scalars<WalkCounters, LookupCounters, VerifyCounters, String>;

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

impl Inputs<'_> {
    pub fn validate<const N: usize>(&self, boundary: &MergeBoundary<'_, N>) -> io::Result<()> {
        let state = boundary.state;
        let base = self.identity.epoch_base_window(boundary.lockstep_b)?;
        if self
            .identity
            .requested_window
            .is_some_and(|requested| requested != base)
        {
            return Err(invalid(
                "epoch save base window differs from requested window",
            ));
        }
        if boundary.dispatch.adaptive.is_some() != self.identity.adaptive {
            return Err(invalid("epoch adaptive dispatch policy/state differs"));
        }
        if self.identity.g2 == "off" && (state.counters.g2_records != 0 || state.g2_store.is_some())
        {
            return Err(invalid("epoch G2 state is not bound to Union"));
        }
        match &state.rescue {
            Some(rescue)
                if !rescue.amendments.is_empty()
                    && state.store.rescue_duplicates
                    && super::super::rescue::valid(&rescue.abandoned, state.store.len())
                    && super::super::rescue::valid(&state.store.quarantine, state.store.len()) =>
            {
                super::super::super::rescue::check_chain(
                    &rescue.amendments,
                    &self.identity.amendments,
                    &self.identity.request,
                    self.identity.queries,
                )
                .map_err(io::Error::other)?;
                if rescue
                    .abandoned
                    .iter()
                    .zip(&state.store.quarantine)
                    .any(|(a, q)| a & !q != 0)
                {
                    return Err(invalid("epoch abandonment outside quarantine"));
                }
            }
            None if !state.store.rescue_duplicates && state.store.quarantine.is_empty() => {}
            _ => return Err(invalid("epoch rescue profile or bitset differs")),
        }
        if matches!(self.admission, Admission::InProgress) {
            super::super::dispatch::Dispatch::validate_admission(state, boundary.dispatch)
                .map_err(|_| {
                    invalid("epoch incomplete admission has issued or noninitial state")
                })?;
        }
        if self
            .operational_stop
            .is_some_and(|context| !context.valid() || self.stop != Some(context.kind()))
        {
            return Err(invalid("epoch save operational stop context differs"));
        }
        validate_failure(
            self.admission_failure,
            self.admission,
            self.rows.len().min(self.identity.queries.len()),
            self.identity.queries.len(),
            self.stop.map(StopReason::name),
            self.operational_stop.is_some(),
        )?;
        let queries = self
            .identity
            .query_rows(state.rescue.as_ref().map_or(0, |r| r.amendments.len()));
        if self.rows.len() > queries.len()
            || matches!(self.admission, Admission::Complete) && self.rows.len() != queries.len()
            || matches!(self.admission, Admission::InProgress)
                && (state.k != 0
                    || !state.in_flight.is_empty()
                    || state.p0 != state.watermark()
                    || state.ledger.counts().get(Tag::Pending) != u64::from(state.watermark()))
        {
            return Err(invalid("epoch input admission progress is inconsistent"));
        }
        let mut next_frontier = 0;
        for (row, (query, amendment)) in self.rows.iter().zip(queries) {
            let expected_role = if query.auxiliary {
                "auxiliary"
            } else {
                "required"
            };
            if row["id"].as_str() != Some(query.id.as_str())
                || row["role"].as_str() != Some(expected_role)
                || row["role_declared"].as_bool() != Some(query.role_declared)
                || row.get("amendment").and_then(Value::as_u64) != amendment
            {
                return Err(invalid("epoch query root order or exact role changed"));
            }
            match row.get("domain") {
                Some(Value::Number(number))
                    if number.as_u64().is_some_and(|id| {
                        id < u64::from(if amendment.is_some() {
                            state.watermark()
                        } else {
                            state.p0
                        })
                    }) => {}
                Some(Value::Null)
                    if row["source_validity_unresolved"] == true
                        && self
                            .frontiers
                            .get(next_frontier)
                            .is_some_and(|frontier| frontier["id"] == query.id) =>
                {
                    next_frontier += 1;
                }
                _ => {
                    return Err(invalid(
                        "epoch input root is missing or outside initial prefix",
                    ));
                }
            }
        }
        if next_frontier != self.frontiers.len() {
            return Err(invalid("epoch input frontier inventory differs"));
        }
        // Every initial ID was introduced by a row in the processed prefix.
        // A partial stop must not hide later admitted IDs behind a short map.
        let width = (MEMBERSHIP_WORDS * 64) as u64;
        let mut start = 0;
        while start < u64::from(state.p0) {
            let mut seen = [0u64; MEMBERSHIP_WORDS];
            let end = (start + width).min(u64::from(state.p0));
            for row in self.rows.iter().take(self.identity.queries.len()) {
                if let Some(id) = row["domain"]
                    .as_u64()
                    .filter(|id| *id >= start && *id < end)
                {
                    let local = (id - start) as usize;
                    seen[local / 64] |= 1 << (local % 64);
                }
            }
            if seen
                .iter()
                .map(|word| u64::from(word.count_ones()))
                .sum::<u64>()
                != end - start
            {
                return Err(invalid("epoch protected initial ID has no query row"));
            }
            start = end;
        }
        Ok(())
    }

    pub fn write_scalars<const N: usize, W: Write>(
        &self,
        boundary: &MergeBoundary<'_, N>,
        output: W,
    ) -> io::Result<(W, Digest)> {
        self.validate(boundary)?;
        let state = boundary.state;
        let scalars = Scalars {
            schema: 5,
            request: self.identity.request.as_str(),
            owner_count: self.identity.owners.len(),
            owners_digest: self.identity.owners_digest,
            walk_semantics_version: super::super::EPOCH_WALK_SEMANTICS_VERSION,
            preparation: self.identity.preparation,
            record_schema: super::super::records::wire::RECORD_SCHEMA,
            lockstep_b: boundary.lockstep_b,
            epoch_base_window: self.identity.epoch_base_window(boundary.lockstep_b)?,
            epoch_result_escrow_jobs: self.identity.epoch_result_escrow_jobs,
            epoch_result_escrow_bytes: self.identity.epoch_result_escrow_bytes,
            epoch_rolling: self.identity.epoch_rolling,
            epoch_cut_size: if self.identity.epoch_rolling {
                self.identity.epoch_cut_size
            } else {
                0
            },
            epoch_publication_order: self.identity.epoch_publication_order,
            k: state.k,
            watermark: state.watermark(),
            p0: state.p0,
            max_domains: state.max_domains,
            max_events: state.max_events,
            max_frontiers: state.max_frontiers,
            ledger_counts: state.ledger.counts().0,
            walk: &state.counters,
            lookup: &state.lookup,
            verify: &state.verify,
            closure: state.tracker.counters(),
            edge_runs: state.edges.runs(),
            edges: state.edges.edges(),
            self_edges: state.edges.self_edges(),
            records_digest: state.edges.records_digest(),
            edge_digest: state.edges.edge_digest(),
            initial_admission: self.admission,
            total_queries: self.identity.queries.len(),
            processed_queries: self.rows.len().min(self.identity.queries.len()),
            input_frontiers: self.frontiers.len(),
            stop_reason: self.stop.map(StopReason::name),
            operational_stop: self.operational_stop,
            admission_failure: self.admission_failure.cloned(),
            amendments: state
                .rescue
                .as_ref()
                .map_or_else(Vec::new, |r| r.amendments.clone()),
            quarantined: super::super::rescue::count(&state.store.quarantine),
            abandoned_obligations: state
                .rescue
                .as_ref()
                .map_or(0, |r| super::super::rescue::count(&r.abandoned)),
            g2: self.identity.g2,
            imported_prefix: 0,
            engine_certification_void: false,
            adaptive_dispatch: boundary.dispatch.adaptive.cloned(),
        };
        let mut stream = Stream::new(output);
        serde_json::to_writer(&mut stream, &scalars).map_err(io::Error::other)?;
        stream.finish()
    }

    pub fn write_rows<W: Write>(&self, output: W, frontiers: bool) -> io::Result<(W, Digest)> {
        let mut stream = Stream::new(output);
        for row in if frontiers { self.frontiers } else { self.rows } {
            serde_json::to_writer(&mut stream, row).map_err(io::Error::other)?;
            stream.write_all(b"\n")?;
        }
        stream.finish()
    }

    pub fn owner_count(&self) -> usize {
        self.identity.owners.len()
    }

    /// Variable owner inventory never enters the bounded scalar metadata or
    /// manifest. One immutable digest per line, with no topology-count cap.
    pub fn write_owners<W: Write>(&self, output: W) -> io::Result<(W, Digest)> {
        let mut stream = Stream::new(output);
        for owner in self.identity.owners {
            serde_json::to_writer(&mut stream, owner).map_err(io::Error::other)?;
            stream.write_all(b"\n")?;
        }
        stream.finish()
    }
}
