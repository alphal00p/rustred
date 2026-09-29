//! G2' log of the responsibility ledger: one row per published Apply native
//! record, in merge (publication) order, with its merge stamp, its kind and,
//! for a G2' record, its snapshot stamp, residual D band and anchors.
//!
//! Every link is validated when it is recorded (the coordinator's re-check of
//! a worker plan) and again at restore: the anchor is a published Apply
//! native of the same (phase, owner) with 0 frontiers, of kind Native,
//! initial D band or G2' residual (never a G2' full cover), merged strictly
//! before the snapshot, and the snapshot is at most the record's own stamp.
//! Stamps strictly increase along the log, so anchor links are well-founded
//! (no responsibility cycle). Exact coverage `Q <= residual u anchors` is
//! re-checked at restore by the checkpoint (it needs the domains).
use super::ledger::{Ledger, Local, Responsibility};
use super::types::{Error, NativeOutcome};

pub(in super::super) const NONE: u32 = u32::MAX;

pub(in super::super) mod kind {
    pub use super::super::super::g2::kind::*;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Row {
    pub id: u32,
    pub kind: u8,
    pub stamp: u64,
    /// G2' kinds: the snapshot stamp the plan read; 0 otherwise.
    pub snapshot: u64,
    /// G2' residual: inspected D band [lo, hi]; initial D band: (cut, cut).
    pub band: (i64, i64),
    pub anchors_start: u64,
    pub anchors_len: u32,
}

#[derive(Debug, Default)]
pub(in super::super) struct G2Log {
    pub rows: Vec<Row>,
    pub anchors: Vec<u32>,
    /// IDs below this (the initial admissions) are never G2'-planned (R1).
    pub initial_count: usize,
}

impl G2Log {
    pub fn new(initial_count: usize) -> Self {
        Self {
            initial_count,
            ..Self::default()
        }
    }
    pub fn anchors_of(&self, row: &Row) -> &[u32] {
        let start = row.anchors_start as usize;
        &self.anchors[start..start + row.anchors_len as usize]
    }
    pub fn g2_records(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| matches!(r.kind, kind::G2_RESIDUAL | kind::G2_FULL_COVER))
            .count()
    }
}

impl<K: Copy + Eq> Ledger<K> {
    pub fn enable_g2(&mut self, initial_count: usize) {
        if self.g2.is_none() {
            self.g2 = Some(G2Log::new(initial_count));
        }
    }

    pub fn g2(&self) -> Option<&G2Log> {
        self.g2.as_ref()
    }

    /// G2' activation on a checkpoint written without it: a log of the
    /// published Apply natives only (no G2' kinds exist yet), in merge order
    /// with their true stamps (validated by `validate_g2`).
    pub fn backfill_g2(
        &mut self,
        initial_count: usize,
        rows: Vec<(u32, u8, u64, (i64, i64))>,
    ) -> Result<(), String> {
        if self.g2.is_some() {
            return Err("G2' activation on a ledger that already has a G2' log".into());
        }
        let mut log = G2Log::new(initial_count);
        log.rows
            .try_reserve_exact(rows.len())
            .map_err(|_| "G2' activation log allocation")?;
        for (id, kind, stamp, band) in rows {
            if !matches!(kind, kind::NATIVE | kind::INITIAL_D_BAND) {
                return Err("G2' activation back-fills only Native and initial-D-band rows".into());
            }
            log.rows.push(Row {
                id,
                kind,
                stamp,
                snapshot: 0,
                band,
                anchors_start: 0,
                anchors_len: 0,
            });
        }
        self.attach_g2(log)
    }

    /// Restore: attach a decoded log (validated by `validate_g2`).
    pub fn attach_g2(&mut self, log: G2Log) -> Result<(), String> {
        for (index, row) in log.rows.iter().enumerate() {
            let entry = self
                .entries
                .get_mut(row.id as usize)
                .ok_or("checkpoint G2' row beyond the ledger")?;
            if entry.g2_row != NONE {
                return Err("checkpoint G2' log lists a record twice".into());
            }
            entry.g2_row = u32::try_from(index).map_err(|_| "checkpoint G2' log exceeds u32")?;
        }
        self.g2 = Some(log);
        Ok(())
    }

    /// The G2' row of a record, if any.
    pub fn g2_row(&self, id: usize) -> Option<&Row> {
        let log = self.g2.as_ref()?;
        let index = self.entries.get(id)?.g2_row;
        (index != NONE).then(|| &log.rows[index as usize])
    }

    /// Coordinator, immediately BEFORE `publish_native(id, Completed)`: log a
    /// published Apply native record. `stamp` must be the current publication
    /// count. G2' kinds carry their plan; every link is re-checked here.
    #[allow(clippy::too_many_arguments)]
    pub fn record_g2(
        &mut self,
        id: usize,
        record_kind: u8,
        stamp: u64,
        snapshot: u64,
        band: (i64, i64),
        anchors: &[(u32, u64)],
    ) -> Result<(), Error> {
        let Some(log) = self.g2.as_ref() else {
            return Err(Error::InvalidG2Anchor);
        };
        if stamp != self.published_count() as u64
            || id >= self.entries.len()
            || self.entries[id].g2_row != NONE
            || self.entries[id].responsibility != Responsibility::Local(Local::Started)
            || log.rows.last().is_some_and(|last| last.stamp >= stamp)
        {
            return Err(Error::InvalidG2Anchor);
        }
        let g2 = matches!(record_kind, kind::G2_RESIDUAL | kind::G2_FULL_COVER);
        if g2 {
            if id < log.initial_count
                || snapshot > stamp
                || anchors.is_empty()
                || (record_kind == kind::G2_RESIDUAL) != (band.0 <= band.1)
                || self.entries[id].initial_anchor.is_some()
            {
                return Err(Error::InvalidG2Anchor);
            }
            let mut previous: Option<u64> = None;
            for &(anchor, anchor_stamp) in anchors {
                let row = self.valid_anchor(id, anchor)?;
                if row.stamp != anchor_stamp
                    || row.stamp >= snapshot
                    || previous.is_some_and(|p| p >= row.stamp)
                {
                    return Err(Error::InvalidG2Anchor);
                }
                previous = Some(row.stamp);
            }
        } else if !anchors.is_empty()
            || snapshot != 0
            || (record_kind == kind::INITIAL_D_BAND) != self.entries[id].initial_anchor.is_some()
            || !matches!(record_kind, kind::NATIVE | kind::INITIAL_D_BAND)
        {
            return Err(Error::InvalidG2Anchor);
        }
        let log = self.g2.as_mut().expect("checked");
        let index = u32::try_from(log.rows.len()).map_err(|_| Error::Capacity)?;
        log.anchors
            .try_reserve(anchors.len())
            .map_err(|_| Error::Allocation)?;
        log.rows.try_reserve(1).map_err(|_| Error::Allocation)?;
        let anchors_start = log.anchors.len() as u64;
        log.anchors
            .extend(anchors.iter().map(|&(anchor, _)| anchor));
        log.rows.push(Row {
            id: u32::try_from(id).map_err(|_| Error::InvalidId)?,
            kind: record_kind,
            stamp,
            snapshot,
            band,
            anchors_start,
            anchors_len: anchors.len() as u32,
        });
        self.entries[id].g2_row = index;
        Ok(())
    }

    /// A published native (Completed, with or without retained frontiers).
    pub fn native_published(&self, id: usize) -> bool {
        self.entries.get(id).is_some_and(|e| {
            matches!(
                e.responsibility,
                Responsibility::Local(Local::Published(NativeOutcome::Completed { .. }))
            )
        })
    }

    pub fn has_initial_anchor(&self, id: usize) -> bool {
        self.entries
            .get(id)
            .is_some_and(|e| e.initial_anchor.is_some())
    }

    /// A published native with no retained frontier (an anchor may lend it).
    pub fn discharged_locally(&self, id: usize) -> bool {
        self.entries.get(id).is_some_and(|e| {
            e.responsibility
                == Responsibility::Local(Local::Published(NativeOutcome::Completed {
                    unresolved_frontiers: 0,
                }))
        })
    }

    /// An admissible anchor of `id`: its logged row.
    pub(in super::super) fn valid_anchor(&self, id: usize, anchor: u32) -> Result<Row, Error> {
        let anchor = anchor as usize;
        if id >= self.entries.len() || anchor == id || anchor >= self.entries.len() {
            return Err(Error::InvalidG2Anchor);
        }
        let source = &self.entries[anchor];
        let row = self.g2_row(anchor).copied().ok_or(Error::InvalidG2Anchor)?;
        if source.key != self.entries[id].key
            || source.responsibility
                != Responsibility::Local(Local::Published(NativeOutcome::Completed {
                    unresolved_frontiers: 0,
                }))
            || !matches!(
                row.kind,
                kind::NATIVE | kind::INITIAL_D_BAND | kind::G2_RESIDUAL
            )
        {
            return Err(Error::InvalidG2Anchor);
        }
        Ok(row)
    }

    /// Restore: the log against the ledger (structure, stamps, kinds, links).
    /// Every published Apply native has exactly one row; `apply(id)` tells
    /// whether an ID is an Apply domain.
    pub fn validate_g2(&self, apply: impl Fn(usize) -> bool) -> Result<(), String> {
        let Some(log) = self.g2.as_ref() else {
            return Ok(());
        };
        let published = self.published_count() as u64;
        let mut previous: Option<u64> = None;
        let mut expected_start = 0u64;
        for row in &log.rows {
            let id = row.id as usize;
            let entry = self
                .entries
                .get(id)
                .ok_or("checkpoint G2' row beyond the ledger")?;
            let Responsibility::Local(Local::Published(NativeOutcome::Completed { .. })) =
                entry.responsibility
            else {
                return Err("checkpoint G2' row of an unpublished record".into());
            };
            if !apply(id)
                || row.stamp >= published
                || previous.is_some_and(|p| p >= row.stamp)
                || row.anchors_start != expected_start
                || (!self.is_ready() && row.stamp != id as u64)
            {
                return Err("checkpoint G2' row order, stamp or phase is invalid".into());
            }
            previous = Some(row.stamp);
            expected_start += u64::from(row.anchors_len);
            let g2 = matches!(row.kind, kind::G2_RESIDUAL | kind::G2_FULL_COVER);
            if g2 {
                if id < log.initial_count
                    || row.snapshot > row.stamp
                    || row.anchors_len == 0
                    || (row.kind == kind::G2_RESIDUAL) != (row.band.0 <= row.band.1)
                    || entry.initial_anchor.is_some()
                {
                    return Err("checkpoint G2' record plan is invalid".into());
                }
                let mut last: Option<u64> = None;
                for &anchor in log.anchors_of(row) {
                    let anchor_row = self
                        .valid_anchor(id, anchor)
                        .map_err(|_| "checkpoint G2' anchor is not an admissible merged record")?;
                    if anchor_row.stamp >= row.snapshot
                        || last.is_some_and(|l| l >= anchor_row.stamp)
                    {
                        return Err(
                            "checkpoint G2' anchor was not merged before the snapshot".into()
                        );
                    }
                    last = Some(anchor_row.stamp);
                }
            } else if row.anchors_len != 0
                || row.snapshot != 0
                || !matches!(row.kind, kind::NATIVE | kind::INITIAL_D_BAND)
                || (row.kind == kind::INITIAL_D_BAND) != entry.initial_anchor.is_some()
            {
                return Err("checkpoint G2' row kind disagrees with the ledger".into());
            }
        }
        if expected_start != log.anchors.len() as u64 {
            return Err("checkpoint G2' anchor list length disagrees with its rows".into());
        }
        // Every published Apply native is logged.
        let natives = self
            .entries
            .iter()
            .enumerate()
            .filter(|(id, e)| {
                apply(*id)
                    && matches!(
                        e.responsibility,
                        Responsibility::Local(Local::Published(NativeOutcome::Completed { .. }))
                    )
            })
            .count();
        if natives != log.rows.len() {
            return Err("checkpoint G2' log misses a published Apply native".into());
        }
        Ok(())
    }
}
