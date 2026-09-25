//! Ticket-local accepted prefixes; no native result/event reservoir lives here.
//!
//! The existing State fields are the mounted context used by common admission.
//! Switching tickets moves, never clones, its diagnostics and replay state.
//! Snapshots include that mounted context and all parked contexts atomically.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Serialize, Deserialize)]
pub(in super::super) struct Context {
    details: Vec<Value>,
    refusals: OptionalRefusals,
    replay: Option<replay::Replay>,
}

#[derive(Default, Serialize, Deserialize)]
pub(in super::super) struct Streams {
    pub active: Option<Ticket>,
    // A sequence avoids JSON object-key restrictions for structured tickets.
    pub parked: Vec<(Ticket, Context)>,
    pub initial_published: usize,
}

impl<const N: usize> State<N> {
    pub(in super::super) fn add_ready_progress(&self, out: &mut Value) {
        if !self.ready() {
            return;
        }
        out["publication_policy"] = json!("ready_ticket_stream");
        out["ready_stream_contexts"] =
            json!(usize::from(self.streams.active.is_some()) + self.streams.parked.len());
        out["ready_accepted_source_prefixes"] = json!(
            usize::from(
                self.replay
                    .as_ref()
                    .is_some_and(|r| r.accepted_events() > 0)
            ) + self
                .streams
                .parked
                .iter()
                .filter(|(_, c)| c.replay.as_ref().is_some_and(|r| r.accepted_events() > 0))
                .count()
        );
        out["ready_published_holes"] = json!(self.published_count() - self.queue.next);
        out["ready_prefix_tracking_scope"] = json!("checkpoint-enabled native streams only");
    }
    /// Failure/cancellation without a checkpoint still retains every accepted
    /// source-local diagnostic, never a fictional completed native inspection.
    pub(super) fn retain_ready_leftovers(&mut self, leftovers: &mut Vec<(usize, Finished)>) {
        let mut returns: BTreeMap<_, _> = leftovers.drain(..).collect();
        if let Some(ticket) = self.streams.active.take() {
            let context = self.take_context();
            self.streams.parked.push((ticket, context));
        }
        for (ticket, context) in std::mem::take(&mut self.streams.parked) {
            let finished = returns.remove(&ticket.parent);
            let mut record = json!({"id":ticket.parent,"physical_part":ticket.part,
                "committed":false,"partial_publisher":true,
                "partial_native_statistics_unavailable":finished.is_none()});
            record["frontiers"] = Value::Array(context.details);
            record["optional_refusals"] = Value::Array(context.refusals.records);
            if let Some(finished) = finished {
                record["stats"] = native_stats(finished.stats);
                record["error"] = json!(finished.error);
                record["seconds"] = json!(finished.seconds);
            }
            self.uncommitted.push(record);
        }
        for (id, finished) in returns {
            self.uncommitted.push(json!({"id":id,"committed":false,
                "stats":native_stats(finished.stats),"error":finished.error,"seconds":finished.seconds}));
        }
    }
    pub(in super::super) fn ready(&self) -> bool {
        self.queue
            .delegation
            .as_ref()
            .is_some_and(|ledger| ledger.is_ready())
    }
    pub(in super::super) fn published_count(&self) -> usize {
        self.queue
            .delegation
            .as_ref()
            .map_or(self.queue.next, |l| l.published_count())
    }
    pub(in super::super) fn initial_published(&self) -> usize {
        if self.ready() {
            self.streams.initial_published
        } else {
            self.queue.next.min(self.initial_domain_count)
        }
    }
    pub(in super::super) fn pending_descendants(&self) -> usize {
        self.queue
            .domains
            .len()
            .saturating_sub(self.initial_domain_count)
            .saturating_sub(
                self.published_count()
                    .saturating_sub(self.initial_published()),
            )
    }
    fn take_context(&mut self) -> Context {
        Context {
            details: std::mem::take(&mut self.details),
            refusals: std::mem::take(&mut self.refusals),
            replay: self.replay.take(),
        }
    }
    fn mount_context(&mut self, context: Context) {
        self.details = context.details;
        self.refusals = context.refusals;
        self.replay = context.replay;
    }
    pub(super) fn activate_stream(
        &mut self,
        ticket: Ticket,
        checkpointing: bool,
    ) -> Result<(), &'static str> {
        if !self.ready() || self.streams.active == Some(ticket) {
            return Ok(());
        }
        if ticket.part.is_some() {
            return Err("Ready subdivision is unsupported");
        }
        if self.streams.active.is_some() {
            self.streams
                .parked
                .try_reserve(1)
                .map_err(|_| "stream context allocation")?;
        }
        if let Some(previous) = self.streams.active.take() {
            let context = self.take_context();
            self.streams.parked.push((previous, context));
        }
        let context = self
            .streams
            .parked
            .iter()
            .position(|(key, _)| *key == ticket)
            .map(|index| self.streams.parked.swap_remove(index).1)
            .unwrap_or_else(|| Context {
                replay: checkpointing.then(replay::Replay::default),
                ..Default::default()
            });
        self.mount_context(context);
        self.streams.active = Some(ticket);
        Ok(())
    }
    pub(super) fn complete_stream(&mut self, ticket: Ticket) {
        if self.ready() {
            debug_assert_eq!(self.streams.active, Some(ticket));
            self.streams.active = None;
            self.replay = None;
        }
    }
    /// Only after ledger restore has validated and normalized Started->Reserved.
    pub(in super::super) fn validate_restored_streams(&self) -> Result<(), String> {
        if !self.ready() {
            if self.streams.active.is_some()
                || !self.streams.parked.is_empty()
                || self.streams.initial_published != 0
            {
                return Err("ordered checkpoint contains ready stream contexts".into());
            }
            return Ok(());
        }
        let ledger = self.queue.delegation.as_ref().expect("ready ledger");
        if self.streams.active.is_some() && self.replay.is_none()
            || self
                .streams
                .parked
                .iter()
                .any(|(_, context)| context.replay.is_none())
        {
            return Err("ready checkpoint stream has no replay prefix".into());
        }
        let mut keys = BTreeMap::new();
        for ticket in self
            .streams
            .active
            .iter()
            .chain(self.streams.parked.iter().map(|(key, _)| key))
        {
            if ticket.part.is_some()
                || ticket.parent >= ledger.len()
                || ledger.is_published(ticket.parent)
                || ledger.delegated_to(ticket.parent).is_some()
                || !ledger.can_dispatch(ticket.parent)
                || keys.insert(*ticket, ()).is_some()
            {
                return Err("invalid or duplicate ready checkpoint stream".into());
            }
        }
        if keys.len() > ledger.lookahead().get()
            || self.streams.initial_published > self.initial_domain_count
        {
            return Err("ready checkpoint context/count bound".into());
        }
        let actual_initial = (0..self.initial_domain_count)
            .filter(|&id| ledger.is_published(id))
            .count();
        if actual_initial != self.streams.initial_published {
            return Err("ready initial publication count mismatch".into());
        }
        if self.streams.active.is_none()
            && (!self.details.is_empty()
                || !self.refusals.records.is_empty()
                || self.replay.is_some())
        {
            return Err("unowned ready checkpoint prefix".into());
        }
        let mut ids = Vec::new();
        ids.try_reserve_exact(ledger.len())
            .map_err(|_| "checkpoint record-ID allocation")?;
        ids.resize(ledger.len(), false);
        let mut native = 0;
        let mut accepted = self.replay.as_ref().map_or(0, |r| r.accepted_events());
        for (_, context) in &self.streams.parked {
            accepted = accepted
                .checked_add(
                    context
                        .replay
                        .as_ref()
                        .expect("validated replay")
                        .accepted_events(),
                )
                .ok_or("checkpoint accepted-events overflow")?;
        }
        for record in &self.records {
            let id = record["id"]
                .as_u64()
                .and_then(|id| usize::try_from(id).ok())
                .ok_or("ready checkpoint record has no valid ID")?;
            if id >= ledger.len() || !ledger.is_published(id) || ids[id] {
                return Err("ready checkpoint record is duplicate or unpublished".into());
            }
            ids[id] = true;
            let delegated = record["record_kind"] == "delegated_not_inspected";
            if delegated != ledger.delegated_to(id).is_some() {
                return Err("ready checkpoint record responsibility mismatch".into());
            }
            native += usize::from(!delegated);
            if !delegated {
                let events = record["accepted_events"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or("ready checkpoint native record has no accepted-events count")?;
                accepted = accepted
                    .checked_add(events)
                    .ok_or("checkpoint accepted-events overflow")?;
            }
        }
        if self.records.len() != ledger.published_count()
            || native != self.native_records
            || native != ledger.native_publications()
        {
            return Err("ready checkpoint records/publications disagree".into());
        }
        if accepted != self.events {
            return Err("ready checkpoint accepted-prefix accounting mismatch".into());
        }
        Ok(())
    }
}
