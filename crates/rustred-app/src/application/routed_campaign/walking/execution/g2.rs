//! G2' glue of the single publisher: session setup (index rebuilt from the
//! ledger's G2' log, restored pins), the worker plan hand-over at commit, the
//! log row, the index append, checkpoint pins and the report.
use super::super::delegation::G2Row;
use super::super::g2::{self, Outcome, Plan, Store, kind};
use super::super::queue::{Domain, Phase};
use super::*;
use std::collections::BTreeSet;
use std::sync::Arc;

/// Progress-metadata key of the pinned plans (present only with G2').
pub(in super::super) const PROGRESS_G2_PINS: &str = "g2_pins";

/// The scope an anchor-eligible record lends (initial D band: its inspected
/// low slice D <= cut - 1; otherwise its whole domain).
pub(in super::super) fn lent_scope<const N: usize>(domain: &Domain<N>, row: &G2Row) -> Domain<N> {
    let mut scope = domain.clone();
    if row.kind == kind::INITIAL_D_BAND {
        let below = row.band.0.saturating_sub(1);
        scope.powers.max_power_difference = Some(
            scope
                .powers
                .max_power_difference
                .map_or(below, |d| d.min(below)),
        );
    }
    scope
}

impl<const N: usize> State<N> {
    /// Session start: the anchor store over every merged anchor-eligible
    /// record of the ledger's G2' log, plus the plans pinned by a restore.
    pub(super) fn g2_setup(&mut self) -> Result<(), String> {
        let ledger = self
            .queue
            .delegation
            .as_ref()
            .ok_or("G2' residual anchors require the responsibility ledger")?;
        let log = ledger
            .g2()
            .ok_or("G2' residual anchors: the ledger has no G2' log")?;
        let ordered_lag = (!ledger.is_ready()).then(|| ledger.lookahead().get() as u64);
        let store = Store::new(
            ordered_lag,
            self.initial_domain_count,
            ledger.published_count() as u64,
        );
        let mut entries = Vec::new();
        for row in &log.rows {
            let id = row.id as usize;
            if row.kind == kind::G2_FULL_COVER
                || !ledger.discharged_locally(id)
                || self.queue.is_quarantined(id)
            {
                continue;
            }
            let domain = self.queue.domain(id);
            if let Some(entry) = Store::entry(&lent_scope(&domain, row), id, row.stamp, row.kind) {
                entries.push((domain.owner, entry));
            }
        }
        store.bulk(entries);
        for (id, outcome) in std::mem::take(&mut self.g2_pins) {
            store.pin(id, outcome);
        }
        self.g2 = Some(Arc::new(store));
        Ok(())
    }

    /// Commit: the worker's plan for job `id` (Some only for a G2' record).
    pub(super) fn g2_take(&mut self, id: usize, scope: Option<g2::G2Scope>) -> Option<Plan> {
        let taken = self.g2.as_ref().and_then(|store| store.take(id));
        let scope = scope?;
        match taken.as_deref() {
            Some(Outcome::Planned(plan)) if plan.scope() == scope => Some(plan.clone()),
            _ => {
                self.error
                    .get_or_insert_with(|| "G2' plan hand-over mismatch".into());
                None
            }
        }
    }

    /// Commit, BEFORE `publish_native`: the G2' log row of a published Apply
    /// native. Returns its merge stamp and kind.
    pub(super) fn g2_log(
        &mut self,
        id: usize,
        plan: Option<&Plan>,
        partial: Option<super::super::initial_overlap::InitialOverlapScope>,
    ) -> Option<(u64, u8)> {
        if self.error.is_some() || self.queue.domains[id].phase() != Phase::Apply {
            return None;
        }
        let ledger = self.queue.delegation.as_mut()?;
        ledger.g2()?;
        let stamp = ledger.published_count() as u64;
        let (record_kind, snapshot, band, anchors): (u8, u64, (i64, i64), Vec<(u32, u64)>) =
            match (plan, partial) {
                (Some(plan), _) => (
                    if plan.residual.is_some() {
                        kind::G2_RESIDUAL
                    } else {
                        kind::G2_FULL_COVER
                    },
                    plan.snapshot,
                    plan.residual.unwrap_or((1, 0)),
                    plan.anchors.iter().map(|a| (a.id, a.stamp)).collect(),
                ),
                (None, Some(scope)) => {
                    (kind::INITIAL_D_BAND, 0, (scope.cut, scope.cut), Vec::new())
                }
                (None, None) => (kind::NATIVE, 0, (0, 0), Vec::new()),
            };
        match ledger.record_g2(id, record_kind, stamp, snapshot, band, &anchors) {
            Ok(()) => Some((stamp, record_kind)),
            Err(error) => {
                self.error.get_or_insert_with(|| error.to_string());
                None
            }
        }
    }

    /// Commit, AFTER the record is published: index the record's lent scope
    /// (anchor-eligible kinds with no frontier) and advance the snapshot.
    pub(super) fn g2_index(&self, id: usize, logged: Option<(u64, u8)>, frontiers: usize) {
        let (Some(store), Some((stamp, record_kind))) = (&self.g2, logged) else {
            return;
        };
        if self.error.is_none()
            && frontiers == 0
            && record_kind != kind::G2_FULL_COVER
            && !self.queue.is_quarantined(id)
        {
            let domain = self.queue.domain(id);
            let row = G2Row {
                id: id as u32,
                kind: record_kind,
                stamp,
                snapshot: 0,
                band: self
                    .queue
                    .delegation
                    .as_ref()
                    .and_then(|l| l.g2_row(id))
                    .map_or((0, 0), |row| row.band),
                anchors_start: 0,
                anchors_len: 0,
            };
            store.append(
                &domain.owner,
                Store::entry(&lent_scope(&domain, &row), id, stamp, record_kind),
            );
        }
        store.published(stamp + 1);
    }

    /// Record block of a G2' record.
    pub(super) fn g2_record_json<const M: usize>(
        plan: &Plan,
        domain: &Domain<M>,
        merge_stamp: Option<u64>,
    ) -> Value {
        json!({"mode":"union","merge_stamp":merge_stamp,"snapshot_stamp":plan.snapshot,
            "residual_power_bounds":plan.residual.map(|(lo, hi)| power_bounds_json(g2::residual_powers(domain.powers, lo, hi))),
            "residual_d_band":plan.residual.map(|(lo, hi)| [lo, hi]),
            "residual_pieces":u8::from(plan.residual.is_some()),
            "anchors":plan.anchors.iter().map(|a| json!({"id":a.id,"stamp":a.stamp,
                "kind":kind::name(a.kind),"scope":kind::scope(a.kind)})).collect::<Vec<_>>(),
            "query_points":plan.points,"covered_points":plan.covered_points,
            "candidates":plan.candidates,"membership_tests":plan.membership_tests,
            "coordinates_and_rank_unchanged":true,
            "authority":"exact interval membership of every covered point in one listed anchor, confirmed by the native one-point DomainPowerSummary inclusion; anchors merged strictly before the snapshot stamp"})
    }

    /// Checkpoint save: the decisions of every unfinished stream that may hold
    /// accepted events (Ordered publisher, Ready active and parked tickets)
    /// plus pins not yet consumed. A resumed job re-runs its pinned decision,
    /// so its replayed prefix is the one accepted before the save.
    pub(super) fn g2_pins_json(&self) -> Option<Value> {
        let Some(store) = self.g2.as_ref() else {
            // A save between restore and session start keeps the restored pins.
            return (!self.g2_pins.is_empty()).then(|| {
                Value::Array(
                    self.g2_pins
                        .iter()
                        .map(|(id, outcome)| json!([id, outcome]))
                        .collect(),
                )
            });
        };
        let mut ids: BTreeSet<usize> = BTreeSet::new();
        ids.insert(self.queue.next);
        ids.extend(self.streams.active.map(|t| t.parent));
        ids.extend(self.streams.parked.iter().map(|(t, _)| t.parent));
        let mut pins: Vec<(usize, Arc<Outcome>)> = ids
            .into_iter()
            .filter_map(|id| store.peek(id).map(|outcome| (id, outcome)))
            .collect();
        for (id, outcome) in store.pending_pins() {
            if !pins.iter().any(|(pinned, _)| *pinned == id) {
                pins.push((id, outcome));
            }
        }
        pins.sort_by_key(|(id, _)| *id);
        Some(Value::Array(
            pins.into_iter()
                .map(|(id, outcome)| json!([id, *outcome]))
                .collect(),
        ))
    }

    pub(super) fn g2_restore_pins(&mut self, value: Option<Value>) -> Result<(), String> {
        let Some(value) = value else {
            return Ok(());
        };
        let pins: Vec<(usize, Outcome)> =
            serde_json::from_value(value).map_err(|e| format!("invalid G2' pins: {e}"))?;
        for (id, outcome) in &pins {
            if *id >= self.queue.domains.len() {
                return Err("G2' pin beyond the admitted domains".into());
            }
            if let Outcome::Planned(plan) = outcome
                && (plan.anchors.is_empty() || plan.residual.is_some_and(|(lo, hi)| lo > hi))
            {
                return Err("invalid G2' pinned plan".into());
            }
        }
        self.g2_pins = pins;
        Ok(())
    }

    /// Before rescue's reverse-taint pass, publish the dependencies of durable
    /// accepted decisions, even when their streams accepted zero callbacks.
    /// This changes neither the pinned plan nor its replay cursor. Ordinary
    /// G2-only resume keeps its historical path; rescue calls this before any
    /// amendment admission/save. Validate the entire set before adding edges.
    pub(in super::super) fn g2_restore_pin_dependencies(&mut self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        for (id, _) in &self.g2_pins {
            if *id >= self.queue.domains.len() || !ids.insert(*id) {
                return Err("rescue: invalid or duplicate G2' pin source".into());
            }
        }
        let planned = self
            .g2_pins
            .iter()
            .any(|(_, outcome)| matches!(outcome, Outcome::Planned(_)));
        if !planned {
            // Old activation checkpoints may retain inert Whole pins for
            // Route or already-published jobs. They lend no dependency.
            return Ok(());
        }
        let ledger = self
            .queue
            .delegation
            .as_ref()
            .ok_or("rescue: G2' pins need the ledger")?;
        let closure = self.closure.borrow();
        for (id, outcome) in &self.g2_pins {
            let Outcome::Planned(plan) = outcome else {
                continue;
            };
            if *id < self.initial_domain_count
                || self.queue.domains[*id].phase() != Phase::Apply
                || ledger.is_published(*id)
                || !matches!(closure.local_status(*id), Some((_, false)))
                || plan.anchors.is_empty()
                || plan.snapshot > ledger.published_count() as u64
                || plan.residual.is_some_and(|(lo, hi)| lo > hi)
            {
                return Err(
                    "rescue: G2' planned pin is not an unfinished unsealed Apply inspection".into(),
                );
            }
            let mut previous = None;
            for anchor in &plan.anchors {
                let row = ledger
                    .valid_anchor(*id, anchor.id)
                    .map_err(|e| format!("rescue: invalid G2' pinned anchor: {e}"))?;
                if row.kind != anchor.kind
                    || row.stamp != anchor.stamp
                    || row.stamp >= plan.snapshot
                    || previous.is_some_and(|stamp| stamp >= row.stamp)
                    || closure.local_status(anchor.id as usize).is_none()
                {
                    return Err(
                        "rescue: G2' pinned anchor kind, stamp or dependency endpoint mismatch"
                            .into(),
                    );
                }
                previous = Some(row.stamp);
            }
        }
        drop(closure);
        let mut closure = self.closure.borrow_mut();
        for (id, outcome) in &self.g2_pins {
            if let Outcome::Planned(plan) = outcome {
                for anchor in &plan.anchors {
                    closure.edge(*id, anchor.id as usize);
                    if closure.local_status(*id).is_none() {
                        return Err("rescue: could not retain G2' pinned dependency edges".into());
                    }
                }
            }
        }
        Ok(())
    }

    /// Durable pins count as accepted decisions for rescue's worker-view
    /// epochs/dead-cone preservation, including a zero-callback pin. At save,
    /// currently active decisions become durable together with their epoch.
    pub(super) fn g2_pinned_holders(&self) -> Vec<usize> {
        let mut ids: Vec<_> = self.g2_pins.iter().map(|(id, _)| *id).collect();
        if let Some(store) = &self.g2 {
            ids.extend(store.pending_pins().into_iter().map(|(id, _)| id));
            ids.extend(
                std::iter::once(self.queue.next)
                    .chain(self.streams.active.map(|t| t.parent))
                    .chain(self.streams.parked.iter().map(|(t, _)| t.parent))
                    .filter(|&id| store.peek(id).is_some()),
            );
        }
        ids.retain(|&id| {
            id < self.queue.domains.len()
                && self
                    .queue
                    .delegation
                    .as_ref()
                    .is_some_and(|ledger| !ledger.is_published(id))
        });
        ids
    }

    /// G2' activation on a resumed checkpoint written without G2': back-fill
    /// the log from the ledger and the record order (a record's merge stamp is
    /// its position in the record stream; an initial-D-band cut is read from
    /// its record), validate it, and pin every stream that may hold accepted
    /// events to a whole inspection (its accepted prefix came from one).
    pub(in super::super) fn g2_backfill(&mut self) -> Result<Value, String> {
        #[derive(serde::Deserialize)]
        struct Cut {
            cut: i64,
        }
        #[derive(serde::Deserialize)]
        struct Probe {
            id: usize,
            #[serde(default)]
            initial_overlap: Option<Cut>,
        }
        let started = std::time::Instant::now();
        let ledger = self
            .queue
            .delegation
            .as_ref()
            .ok_or("G2' activation requires the responsibility ledger")?;
        let total = self.records.borrow().total();
        if total != ledger.published_count() {
            return Err("G2' activation: records and publications disagree".into());
        }
        let mut rows = Vec::new();
        let mut position = 0u64;
        let files = match &*self.records.borrow() {
            super::records::RecordSink::Sidecar(sidecar) => sidecar.files(),
            super::records::RecordSink::Memory(_) => {
                return Err("G2' activation requires a record sidecar".into());
            }
        };
        files.for_each_line(|line| {
            let probe: Probe = serde_json::from_slice(line)
                .map_err(|e| format!("G2' activation: invalid record line: {e}"))?;
            let stamp = position;
            position += 1;
            let id = probe.id;
            if self
                .queue
                .domains
                .get(id)
                .is_none_or(|d| d.phase() != Phase::Apply)
                || !ledger.native_published(id)
            {
                return Ok(());
            }
            let row = if ledger.has_initial_anchor(id) {
                let cut = probe
                    .initial_overlap
                    .ok_or("G2' activation: initial-D-band record without its cut")?
                    .cut;
                (id as u32, kind::INITIAL_D_BAND, stamp, (cut, cut))
            } else {
                (id as u32, kind::NATIVE, stamp, (0, 0))
            };
            rows.try_reserve(1)
                .map_err(|_| "G2' activation log allocation".to_owned())?;
            rows.push(row);
            Ok(())
        })?;
        if position as usize != total {
            return Err("G2' activation: record stream shorter than its count".into());
        }
        let backfilled = rows.len();
        let initial_count = self.initial_domain_count;
        let ledger = self.queue.delegation.as_mut().expect("checked");
        ledger.backfill_g2(initial_count, rows)?;
        let domains = &self.queue.domains;
        ledger.validate_g2(|id| domains.get(id).is_some_and(|d| d.phase() == Phase::Apply))?;
        let mut pinned: BTreeSet<usize> = BTreeSet::new();
        pinned.insert(self.queue.next);
        pinned.extend(self.streams.active.map(|t| t.parent));
        pinned.extend(self.streams.parked.iter().map(|(t, _)| t.parent));
        pinned.retain(|&id| id < self.queue.domains.len());
        self.g2_pins = pinned.iter().map(|&id| (id, Outcome::Whole)).collect();
        Ok(
            json!({"event":"g2_activated","operation":"owner_domain_walk",
            "published_records":total,"backfilled_apply_natives":backfilled,
            "streams_pinned_whole":pinned.len(),"seconds":started.elapsed().as_secs_f64(),
            "family_closure_claim":false}),
        )
    }

    /// Final and paused reports.
    pub(in super::super) fn add_g2_report(&self, document: &mut Value) {
        let Some(store) = &self.g2 else {
            return;
        };
        let mut report = store.report();
        if let Some(log) = self.queue.delegation.as_ref().and_then(|l| l.g2()) {
            report["logged_apply_natives"] = json!(log.rows.len());
            report["logged_g2_records"] = json!(log.g2_records());
            report["logged_anchor_links"] = json!(log.anchors.len());
        }
        document["g2_residual_anchors"] = report;
        document["g2_index_telemetry"] = store.telemetry();
    }
}
