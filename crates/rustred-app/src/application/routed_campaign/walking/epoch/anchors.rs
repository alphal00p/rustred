//! Anchors (W2.0 protocol §7; orchestrator decision 10(a): G2' residual
//! anchors in union form). The record format carries every anchor kind from
//! day one, although S2 produces only InitialDBand records (today's
//! `--reuse-initial-d-bands`). G2Native / G2Residual records are W4 (G2');
//! their validator rules are implemented here so that the format, the P1
//! re-check and the restore/audit rules exist before the planner does.
//!
//! Rules:
//! - R1: no anchor record on a node < P0 (IDs < P0 are inspected whole).
//! - R2 InitialDBand: anchor < P0 <= node, same bucket, the anchor itself has
//!   no anchors, no stamp; the anchor lends its full domain to the node's
//!   D >= cut slice.
//! - R3 G2Native / G2Residual: every anchor stamped with its merge epoch,
//!   `stamp == ledger[anchor].merge_epoch <= dispatch_version <
//!   ledger[node].merge_epoch`; the anchor is a merged Native (or a G2
//!   residual native); an InitialDBand native lends only its inspected low-D
//!   slice. Anchor links strictly decrease the merge epoch, InitialDBand links
//!   end at an anchor-free ID < P0: responsibility is well-founded.
use super::ledger6::{Entry6, Ledger6};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AnchorKind {
    InitialDBand = 0,
    G2Native = 1,
    G2Residual = 2,
}

impl AnchorKind {
    pub fn name(self) -> &'static str {
        match self {
            AnchorKind::InitialDBand => "initial_d_band",
            AnchorKind::G2Native => "g2_native",
            AnchorKind::G2Residual => "g2_residual",
        }
    }
}

/// The scope descriptor: InitialDBand carries its D cut; a G2' residual
/// carries its residual boxes (`lower/upper` per axis, u16, 0xFFFF = +inf).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AnchorScope {
    DBandCut(i64),
    ResidualBoxes(Vec<(Vec<u16>, Vec<u16>)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AnchorRecord {
    pub node: u32,
    pub kind: AnchorKind,
    /// v0 of the node's job (the snapshot its planner read).
    pub dispatch_version: u64,
    pub scope: AnchorScope,
    /// `(anchor, stamp)`: InitialDBand anchors carry no stamp.
    pub anchors: Vec<(u32, Option<u64>)>,
}

#[derive(Default)]
pub(super) struct AnchorMap {
    /// Sorted by node (nodes are merged once, in merge order; sorted on
    /// export).
    records: Vec<AnchorRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AnchorViolation {
    NodeBelowP0,
    NoAnchors,
    InitialAnchorNotBelowP0,
    CrossBucket,
    AnchorHasAnchors,
    UnexpectedStamp,
    MissingStamp,
    StampAfterDispatch,
    StampMismatch,
    AnchorNotNative,
    NodeNotLaterThanAnchor,
    ScopeKind,
}

/// What the validator needs to know about the store and ledger.
pub(super) struct AnchorView<'a> {
    pub p0: u32,
    pub ledger: &'a Ledger6,
    pub same_bucket: &'a dyn Fn(u32, u32) -> bool,
    pub has_anchors: &'a dyn Fn(u32) -> bool,
}

impl AnchorRecord {
    /// R1-R3 on one record. `node_epoch` is the node's merge epoch (the P1
    /// check runs before P3 stamps it: pass the epoch it will receive).
    pub fn validate(&self, view: &AnchorView<'_>, node_epoch: u64) -> Result<(), AnchorViolation> {
        if self.node < view.p0 {
            return Err(AnchorViolation::NodeBelowP0);
        }
        if self.anchors.is_empty() {
            return Err(AnchorViolation::NoAnchors);
        }
        for &(anchor, stamp) in &self.anchors {
            if !(view.same_bucket)(anchor, self.node) {
                return Err(AnchorViolation::CrossBucket);
            }
            match self.kind {
                AnchorKind::InitialDBand => {
                    if !matches!(self.scope, AnchorScope::DBandCut(_)) {
                        return Err(AnchorViolation::ScopeKind);
                    }
                    if anchor >= view.p0 {
                        return Err(AnchorViolation::InitialAnchorNotBelowP0);
                    }
                    if stamp.is_some() {
                        return Err(AnchorViolation::UnexpectedStamp);
                    }
                    if (view.has_anchors)(anchor) {
                        return Err(AnchorViolation::AnchorHasAnchors);
                    }
                }
                AnchorKind::G2Native | AnchorKind::G2Residual => {
                    if self.kind == AnchorKind::G2Residual
                        && !matches!(self.scope, AnchorScope::ResidualBoxes(_))
                    {
                        return Err(AnchorViolation::ScopeKind);
                    }
                    let stamp = stamp.ok_or(AnchorViolation::MissingStamp)?;
                    if stamp > self.dispatch_version {
                        return Err(AnchorViolation::StampAfterDispatch);
                    }
                    if self.dispatch_version >= node_epoch {
                        return Err(AnchorViolation::NodeNotLaterThanAnchor);
                    }
                    match view.ledger.get(anchor) {
                        Ok(Entry6::Native { epoch, .. }) if epoch == stamp => {}
                        Ok(Entry6::Native { .. }) => return Err(AnchorViolation::StampMismatch),
                        _ => return Err(AnchorViolation::AnchorNotNative),
                    }
                }
            }
        }
        Ok(())
    }
}

impl AnchorMap {
    pub fn try_reserve(&mut self, n: usize) -> Result<(), &'static str> {
        self.records
            .try_reserve(n)
            .map_err(|_| "anchor map allocation")
    }
    pub fn push(&mut self, record: AnchorRecord) {
        self.records.push(record);
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn records(&self) -> &[AnchorRecord] {
        &self.records
    }
    /// The §11.7 layout: header `{version u16, count u64}`, records sorted by
    /// node: `node u32, kind u8, n_anchors u8, scope_len u16,
    /// dispatch_version u64`, `(anchor u32, stamp u64 or u64::MAX) x n`, then
    /// the scope bytes (InitialDBand: `cut i64`; G2Residual: boxes).
    pub fn encode(&self) -> Vec<u8> {
        let mut sorted: Vec<&AnchorRecord> = self.records.iter().collect();
        sorted.sort_by_key(|record| record.node);
        let mut out = Vec::new();
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&(sorted.len() as u64).to_le_bytes());
        for record in sorted {
            let scope = match &record.scope {
                AnchorScope::DBandCut(cut) => cut.to_le_bytes().to_vec(),
                AnchorScope::ResidualBoxes(boxes) => {
                    let mut bytes = Vec::new();
                    for (lower, upper) in boxes {
                        for v in lower.iter().chain(upper) {
                            bytes.extend_from_slice(&v.to_le_bytes());
                        }
                    }
                    bytes
                }
            };
            out.extend_from_slice(&record.node.to_le_bytes());
            out.push(record.kind as u8);
            out.push(record.anchors.len() as u8);
            out.extend_from_slice(&(scope.len() as u16).to_le_bytes());
            out.extend_from_slice(&record.dispatch_version.to_le_bytes());
            for &(anchor, stamp) in &record.anchors {
                out.extend_from_slice(&anchor.to_le_bytes());
                out.extend_from_slice(&stamp.unwrap_or(u64::MAX).to_le_bytes());
            }
            out.extend_from_slice(&scope);
        }
        out
    }
}

/// G2' planning source (§3.4, §7 R3): per bucket, `(id, scope kind)` of every
/// merged native with merge epoch <= v, appended in P4, read by the planner
/// from the snapshot, never from ledger6. Built only with a bound G2' flag;
/// W2 lands the type and the visibility rule, not the planner.
#[derive(Default)]
pub(super) struct MergedView {
    /// `(bucket, id, merge epoch, kind)` in merge order.
    entries: Vec<(u32, u32, u64, AnchorKind)>,
}

impl MergedView {
    #[cfg(test)]
    pub fn push(&mut self, bucket: u32, id: u32, epoch: u64, kind: AnchorKind) {
        self.entries.push((bucket, id, epoch, kind));
    }
    /// Anchors a job dispatched at `v0` may use: merged at an epoch <= v0.
    #[cfg(test)]
    pub fn visible(&self, bucket: u32, v0: u64) -> impl Iterator<Item = u32> + '_ {
        self.entries
            .iter()
            .filter(move |entry| entry.0 == bucket && entry.2 <= v0)
            .map(|entry| entry.1)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
