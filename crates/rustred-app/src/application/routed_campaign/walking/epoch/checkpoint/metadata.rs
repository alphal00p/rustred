//! Borrowed metadata for the private writer. Query rows are streamed
//! individually; the full query document is bound once before the save path.
use super::super::super::{OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest};
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
        Ok(Self {
            request: super::super::super::checkpoint::epoch_request_binding(request),
            owners,
            owners_digest: owners_digest.blake3,
            queries,
        })
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Admission {
    /// Preparation is complete, but some input queries are not yet admitted.
    InProgress,
    Complete,
}

/// Every reference is held for the same save; no root/frontier array clone.
pub(super) struct Inputs<'a> {
    pub identity: &'a Identity<'a>,
    pub admission: Admission,
    pub rows: &'a [Value],
    pub frontiers: &'a [Value],
    pub stop: Option<StopReason>,
}

#[derive(Serialize)]
struct Scalars<'a> {
    schema: u32,
    request: &'a str,
    owner_count: usize,
    owners_digest: [u8; 32],
    walk_semantics_version: u32,
    lockstep_b: usize,
    k: u64,
    watermark: u32,
    p0: u32,
    max_domains: usize,
    max_events: u64,
    max_frontiers: u64,
    ledger_counts: [u64; 7],
    walk: &'a WalkCounters,
    lookup: &'a LookupCounters,
    verify: &'a VerifyCounters,
    closure: super::super::super::descendant_closure::Counters,
    edge_runs: u64,
    edges: u64,
    self_edges: u64,
    records_digest: String,
    edge_digest: String,
    initial_admission: Admission,
    total_queries: usize,
    processed_queries: usize,
    input_frontiers: usize,
    stop_reason: Option<&'static str>,
    // Reserved provenance fields: this implementation admits none of these.
    amendments: [(); 0],
    quarantined: [(); 0],
    abandoned_obligations: [(); 0],
    g2: &'static str,
    imported_prefix: u64,
    engine_certification_void: bool,
}

impl Inputs<'_> {
    pub fn validate<const N: usize>(&self, boundary: &MergeBoundary<'_, N>) -> io::Result<()> {
        let state = boundary.state;
        if self.rows.len() > self.identity.queries.len()
            || matches!(self.admission, Admission::Complete)
                && self.rows.len() != self.identity.queries.len()
            || matches!(self.admission, Admission::InProgress)
                && (state.k != 0
                    || !state.in_flight.is_empty()
                    || state.p0 != state.watermark()
                    || state.ledger.counts().get(Tag::Pending) != u64::from(state.watermark()))
        {
            return Err(invalid("epoch input admission progress is inconsistent"));
        }
        let mut next_frontier = 0;
        for (row, query) in self.rows.iter().zip(self.identity.queries) {
            let expected_role = if query.auxiliary {
                "auxiliary"
            } else {
                "required"
            };
            if row["id"].as_str() != Some(query.id.as_str())
                || row["role"].as_str() != Some(expected_role)
                || row["role_declared"].as_bool() != Some(query.role_declared)
            {
                return Err(invalid("epoch query root order or exact role changed"));
            }
            match row.get("domain") {
                Some(Value::Number(number))
                    if number.as_u64().is_some_and(|id| id < u64::from(state.p0)) => {}
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
            for row in self.rows {
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
            schema: 1,
            request: &self.identity.request,
            owner_count: self.identity.owners.len(),
            owners_digest: self.identity.owners_digest,
            walk_semantics_version: super::super::EPOCH_WALK_SEMANTICS_VERSION,
            lockstep_b: boundary.lockstep_b,
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
            processed_queries: self.rows.len(),
            input_frontiers: self.frontiers.len(),
            stop_reason: self.stop.map(StopReason::name),
            amendments: [],
            quarantined: [],
            abandoned_obligations: [],
            g2: "off",
            imported_prefix: 0,
            engine_certification_void: false,
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
