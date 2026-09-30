//! Anchors (W2.0 protocol §7; orchestrator decision 10(a): G2' residual
//! anchors in union form). The record format carries every anchor kind from
//! day one, although S2 produces only InitialDBand records (today's
//! `--reuse-initial-d-bands`). G2Native / G2Residual records are W4 (G2');
//! their validator rules, their exact cover and the P1 path (gated by a G2'
//! flag that S2 never sets) are implemented and unit-tested here so that the
//! format, the P1 re-check and the restore/audit rules exist before the
//! planner does.
//!
//! Record kinds (the node's kind):
//! - `InitialDBand` (0): the node inspected its D < cut slice; one anchor
//!   < P0 lends its full domain for the D >= cut slice.
//! - `G2Native` (1): a G2' record whose anchors are all plain Natives (the
//!   falsifier's arm "n").
//! - `G2Residual` (2): a G2' record in union form (arm "u", decision 10(a)):
//!   anchors may be plain Natives, G2' natives and InitialDBand natives.
//!
//! Both G2' kinds carry their inspected residual as a union of pieces (a
//! coordinate box intersected with a power-difference band `D in [lo, hi]`,
//! the form of the production G2' lane's residual band); an empty union is
//! the planner's full cover.
//!
//! Rules (validators, P1 and restore/export):
//! - R1: no anchor record on a node < P0 (IDs < P0 are inspected whole).
//! - R2 InitialDBand: exactly one anchor, anchor < P0 <= node, same bucket,
//!   the anchor itself has no anchor record, no stamp, lends its full domain.
//! - R3 G2': every anchor stamped with its merge epoch, `stamp ==
//!   ledger[anchor].merge_epoch <= dispatch_version < merge epoch of the
//!   node`; the anchor is a merged Native; the lent scope is derived from the
//!   anchor, never chosen: a plain Native or a G2' native lends its full
//!   domain, an InitialDBand native lends ONLY its inspected low-D slice
//!   (its D >= cut slice is delegated to an anchor < P0 that may merge later);
//!   `G2Native` records admit plain Native anchors only.
//! - Cover: `Q ⊆ residual ∪ lent scopes` exactly, by the oracle's
//!   `lattice::Cell::covered_by_union` (undecided is refused).
//! - Every anchor edge is present in the node's run (restore/export; P1's
//!   tokens put them there in P3).
//!
//! Anchor links strictly decrease the merge epoch (G2') or end at an
//! anchor-free ID < P0 (InitialDBand), and a delegated slice is never lent
//! again: responsibility is well-founded.
use super::super::queue::CompactDomain;
use super::super::verify_closure::lattice::Cell;
use super::ledger6::{Entry6, Ledger6};

/// Version of the anchors section layout (§11.7 as amended by the S2 note,
/// disposition D19: u32 counts, a lent scope per anchor, union-form residual).
pub(super) const ANCHORS_VERSION: u16 = 2;
/// Region budget of the exact union cover (as the closure verifier).
pub(super) const COVER_REGIONS: u64 = super::super::g2::EPOCH_COVER_REGIONS;
pub(super) const MAX_ANCHORS: usize = super::super::g2::POINT_CAP;
pub(super) const MAX_RESIDUAL_PIECES: usize = 1 << 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum AnchorKind {
    InitialDBand = 0,
    G2Native = 1,
    G2Residual = 2,
}

impl AnchorKind {
    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => AnchorKind::InitialDBand,
            1 => AnchorKind::G2Native,
            2 => AnchorKind::G2Residual,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            AnchorKind::InitialDBand => "initial_d_band",
            AnchorKind::G2Native => "g2_native",
            AnchorKind::G2Residual => "g2_residual",
        }
    }
    pub fn is_g2(self) -> bool {
        !matches!(self, AnchorKind::InitialDBand)
    }
}

/// The scope an anchor lends to the node's cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Lent {
    /// The anchor's full domain (a plain or G2' Native; an ID < P0).
    Full = 0,
    /// Only the anchor's inspected D < cut slice (an InitialDBand native).
    LowSlice = 1,
}

impl Lent {
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Lent::Full),
            1 => Some(Lent::LowSlice),
            _ => None,
        }
    }
}

/// One piece of a union-form residual: the node's domain intersected with a
/// coordinate box (u16 per axis as in the compact image, `u16::MAX` = +inf
/// upper) and an inclusive power-difference band (None: unbounded).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Piece {
    pub d_lo: Option<i64>,
    pub d_hi: Option<i64>,
    pub lower: Vec<u16>,
    pub upper: Vec<u16>,
}

impl Piece {
    /// The whole node restricted to a D band (the production G2' residual).
    #[cfg(test)]
    pub fn band(arity: usize, d_lo: Option<i64>, d_hi: Option<i64>) -> Self {
        Self {
            d_lo,
            d_hi,
            lower: vec![0; arity],
            upper: vec![u16::MAX; arity],
        }
    }
}

/// The scope descriptor: InitialDBand carries its D cut (the node inspected
/// D < cut); a G2' record carries its inspected residual in union form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AnchorScope {
    DBandCut(i64),
    Residual(Vec<Piece>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AnchorRef {
    pub anchor: u32,
    /// The anchor's merge epoch (G2'); InitialDBand anchors carry none.
    pub stamp: Option<u64>,
    pub lent: Lent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AnchorRecord {
    pub node: u32,
    pub kind: AnchorKind,
    /// v0 of the node's job (the snapshot its planner read).
    pub dispatch_version: u64,
    pub scope: AnchorScope,
    pub anchors: Vec<AnchorRef>,
}

#[derive(Default)]
pub(super) struct AnchorMap {
    /// In merge order (nodes merge once; sorted by node on export).
    records: Vec<AnchorRecord>,
    /// node -> position in `records`.
    by_node: std::collections::HashMap<u32, u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AnchorViolation {
    NodeBelowP0,
    NoAnchors,
    DuplicateAnchor,
    AnchorOutOfRange,
    InitialDBandAnchorCount,
    InitialAnchorNotBelowP0,
    CrossBucket,
    AnchorHasAnchors,
    UnexpectedStamp,
    MissingStamp,
    StampAfterDispatch,
    StampMismatch,
    AnchorNotNative,
    MalformedAnchorEntry,
    NodeNotLaterThanAnchor,
    NotInMergedView,
    LentScopeMismatch,
    AnchorKindInadmissible,
    AnchorRecordMismatch,
    ScopeKind,
    ScopeArity,
    MissingAnchorEdge,
    UnionNotCovered,
    UnionUndecided,
}

/// What the validator needs to know about the store, the ledger and (at
/// restore/export) the saved edge runs.
pub(super) struct AnchorView<'a> {
    pub p0: u32,
    /// Anchors must be < this (P1: W_k; restore/export: W).
    pub published_len: usize,
    pub arity: usize,
    pub ledger: &'a Ledger6,
    pub same_bucket: &'a dyn Fn(u32, u32) -> bool,
    /// The anchor record of an ID, if any: its kind and, for InitialDBand,
    /// its cut.
    pub record_of: &'a dyn Fn(u32) -> Option<(AnchorKind, Option<i64>)>,
    /// The node's saved edge run (restore/export). None in P1: P3 appends
    /// the anchor edges from P1's tokens.
    pub edges_of: Option<&'a dyn Fn(u32) -> Option<Vec<u32>>>,
    /// G2' planner visibility (`MergedView` of S_{v0}); None when no view is
    /// kept (restore/export: the stamps are re-checked against ledger6).
    pub merged_view: Option<&'a dyn Fn(u32, u64) -> bool>,
    /// The exact cover `Q ⊆ residual ∪ lent scopes`.
    pub cover: &'a dyn Fn(&AnchorRecord) -> Option<bool>,
}

impl AnchorRecord {
    /// The InitialDBand anchor and cut, if this is an InitialDBand record.
    pub fn d_band(&self) -> Option<(u32, i64)> {
        match (self.kind, &self.scope, self.anchors.first()) {
            (AnchorKind::InitialDBand, AnchorScope::DBandCut(cut), Some(anchor)) => {
                Some((anchor.anchor, *cut))
            }
            _ => None,
        }
    }

    /// R1-R3, the lent-scope rule, edge presence and the exact cover on one
    /// record. `node_epoch` is the node's merge epoch (P1 runs before P3
    /// stamps it: pass the epoch it will receive).
    pub fn validate(&self, view: &AnchorView<'_>, node_epoch: u64) -> Result<(), AnchorViolation> {
        use AnchorViolation as V;
        if self.node < view.p0 {
            return Err(V::NodeBelowP0);
        }
        if self.anchors.is_empty() {
            return Err(V::NoAnchors);
        }
        let mut ids: Vec<u32> = self.anchors.iter().map(|a| a.anchor).collect();
        ids.sort_unstable();
        if ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(V::DuplicateAnchor);
        }
        match (&self.scope, self.kind) {
            (AnchorScope::DBandCut(_), AnchorKind::InitialDBand) => {
                if self.anchors.len() != 1 {
                    return Err(V::InitialDBandAnchorCount);
                }
            }
            (AnchorScope::Residual(pieces), AnchorKind::G2Native | AnchorKind::G2Residual) => {
                if pieces
                    .iter()
                    .any(|p| p.lower.len() != view.arity || p.upper.len() != view.arity)
                {
                    return Err(V::ScopeArity);
                }
            }
            _ => return Err(V::ScopeKind),
        }
        for a in &self.anchors {
            if a.anchor as usize >= view.published_len {
                return Err(V::AnchorOutOfRange);
            }
            if !(view.same_bucket)(a.anchor, self.node) {
                return Err(V::CrossBucket);
            }
            if self.kind == AnchorKind::InitialDBand {
                if a.anchor >= view.p0 {
                    return Err(V::InitialAnchorNotBelowP0);
                }
                if a.stamp.is_some() {
                    return Err(V::UnexpectedStamp);
                }
                if a.lent != Lent::Full {
                    return Err(V::LentScopeMismatch);
                }
                if (view.record_of)(a.anchor).is_some() {
                    return Err(V::AnchorHasAnchors);
                }
                continue;
            }
            let stamp = a.stamp.ok_or(V::MissingStamp)?;
            if stamp > self.dispatch_version {
                return Err(V::StampAfterDispatch);
            }
            if self.dispatch_version >= node_epoch {
                return Err(V::NodeNotLaterThanAnchor);
            }
            let (epoch, residual, dband) = match view.ledger.get(a.anchor) {
                Ok(Entry6::Native {
                    epoch,
                    residual,
                    dband,
                }) => (epoch, residual, dband),
                _ => return Err(V::AnchorNotNative),
            };
            if epoch != stamp {
                return Err(V::StampMismatch);
            }
            if residual && dband {
                return Err(V::MalformedAnchorEntry);
            }
            if let Some(visible) = view.merged_view
                && !visible(a.anchor, self.dispatch_version)
            {
                return Err(V::NotInMergedView);
            }
            // The lent scope follows from the anchor; it is never chosen.
            let expected = if dband { Lent::LowSlice } else { Lent::Full };
            if a.lent != expected {
                return Err(V::LentScopeMismatch);
            }
            if self.kind == AnchorKind::G2Native && (dband || residual) {
                return Err(V::AnchorKindInadmissible);
            }
            // ledger6 flags and the anchor's own record agree.
            let consistent = match (view.record_of)(a.anchor) {
                None => !dband && !residual,
                Some((AnchorKind::InitialDBand, Some(_))) => dband,
                Some((AnchorKind::InitialDBand, None)) => false,
                Some((AnchorKind::G2Native | AnchorKind::G2Residual, _)) => residual,
            };
            if !consistent {
                return Err(V::AnchorRecordMismatch);
            }
        }
        if let Some(edges_of) = view.edges_of {
            let run = edges_of(self.node).unwrap_or_default();
            if self
                .anchors
                .iter()
                .any(|a| run.binary_search(&a.anchor).is_err())
            {
                return Err(V::MissingAnchorEdge);
            }
        }
        match (view.cover)(self) {
            Some(true) => Ok(()),
            Some(false) => Err(V::UnionNotCovered),
            None => Err(V::UnionUndecided),
        }
    }
}

/// The lattice cell of a canonical image (the oracle's exact predicate).
pub(super) fn cell_of<const N: usize>(image: &CompactDomain<N>) -> Cell {
    Cell {
        owner: image.owner().to_vec(),
        lower: (0..N).map(|axis| image.lower(axis)).collect(),
        upper: (0..N).map(|axis| image.upper(axis)).collect(),
        rank: image.rank(),
        powers: image.powers(),
    }
}

/// The D < cut slice of a cell (what an InitialDBand native inspected).
pub(super) fn low_slice(mut cell: Cell, cut: i64) -> Option<Cell> {
    let high = cut.checked_sub(1)?;
    cell.powers.max_power_difference = Some(
        cell.powers
            .max_power_difference
            .map_or(high, |d| d.min(high)),
    );
    Some(cell)
}

/// `q ∩ piece` (box and D band).
fn piece_cell(q: &Cell, piece: &Piece) -> Option<Cell> {
    if piece.lower.len() != q.lower.len() || piece.upper.len() != q.upper.len() {
        return None;
    }
    let mut cell = q.clone();
    for axis in 0..cell.lower.len() {
        cell.lower[axis] = cell.lower[axis].max(u64::from(piece.lower[axis]));
        if piece.upper[axis] != u16::MAX {
            let high = u64::from(piece.upper[axis]);
            cell.upper[axis] = Some(cell.upper[axis].map_or(high, |u| u.min(high)));
        }
    }
    if let Some(lo) = piece.d_lo {
        cell.powers.min_power_difference =
            Some(cell.powers.min_power_difference.map_or(lo, |d| d.max(lo)));
    }
    if let Some(hi) = piece.d_hi {
        cell.powers.max_power_difference =
            Some(cell.powers.max_power_difference.map_or(hi, |d| d.min(hi)));
    }
    Some(cell)
}

/// Exact `Q ⊆ residual ∪ lent scopes` for one record (`Q` = the node's
/// image): the recorded residual (InitialDBand: the D < cut slice; G2': the
/// union of pieces) plus each anchor's lent scope (full domain, or the
/// anchor's own D < cut slice, `cut_of`). None: undecided or malformed
/// (refused by every caller).
pub(super) fn union_cover<const N: usize>(
    node: &CompactDomain<N>,
    domains: &[CompactDomain<N>],
    record: &AnchorRecord,
    cut_of: &dyn Fn(u32) -> Option<i64>,
) -> Option<bool> {
    let q = cell_of(node);
    let mut targets = Vec::with_capacity(record.anchors.len() + 1);
    match &record.scope {
        AnchorScope::DBandCut(cut) => targets.push(low_slice(q.clone(), *cut)?),
        AnchorScope::Residual(pieces) => {
            if pieces.len() > MAX_RESIDUAL_PIECES {
                return None;
            }
            for piece in pieces {
                targets.push(piece_cell(&q, piece)?);
            }
        }
    }
    for a in &record.anchors {
        let cell = cell_of(domains.get(a.anchor as usize)?);
        targets.push(match a.lent {
            Lent::Full => cell,
            Lent::LowSlice => low_slice(cell, cut_of(a.anchor)?)?,
        });
    }
    let refs: Vec<&Cell> = targets.iter().collect();
    q.covered_by_union(&refs, COVER_REGIONS)
}

fn put_opt_i64(out: &mut Vec<u8>, value: Option<i64>) {
    out.push(u8::from(value.is_some()));
    out.extend_from_slice(&value.unwrap_or(0).to_le_bytes());
}

pub(super) fn scope_bytes(scope: &AnchorScope) -> Result<Vec<u8>, String> {
    Ok(match scope {
        AnchorScope::DBandCut(cut) => cut.to_le_bytes().to_vec(),
        AnchorScope::Residual(pieces) => {
            if pieces.len() > MAX_RESIDUAL_PIECES {
                return Err("anchor residual piece bound".into());
            }
            let mut bytes = Vec::new();
            let n = u32::try_from(pieces.len()).map_err(|_| "anchor residual: too many pieces")?;
            bytes.extend_from_slice(&n.to_le_bytes());
            for piece in pieces {
                if piece.lower.len() != piece.upper.len() || piece.lower.len() > 32 {
                    return Err("anchor residual piece arity".into());
                }
                put_opt_i64(&mut bytes, piece.d_lo);
                put_opt_i64(&mut bytes, piece.d_hi);
                for v in piece.lower.iter().chain(&piece.upper) {
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
            bytes
        }
    })
}

impl AnchorMap {
    pub fn try_reserve(&mut self, n: usize) -> Result<(), &'static str> {
        self.records
            .try_reserve(n)
            .map_err(|_| "anchor map allocation")?;
        self.by_node
            .try_reserve(n)
            .map_err(|_| "anchor map allocation")
    }
    /// Append a validated record (P3). A second record for one node is an
    /// engine inconsistency (C5 in P3).
    pub fn push(&mut self, record: AnchorRecord) -> Result<(), String> {
        let position = u32::try_from(self.records.len()).map_err(|_| "anchor map beyond u32")?;
        if self.by_node.insert(record.node, position).is_some() {
            return Err(format!("a second anchor record for node {}", record.node));
        }
        self.records.push(record);
        Ok(())
    }
    pub fn get(&self, node: u32) -> Option<&AnchorRecord> {
        self.by_node
            .get(&node)
            .map(|&position| &self.records[position as usize])
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn records(&self) -> &[AnchorRecord] {
        &self.records
    }
    /// The anchors section, layout v2 (amended §11.7): header `{version u16,
    /// count u64}`; records sorted by node: `node u32, kind u8, reserved u8
    /// (0), reserved u16 (0), n_anchors u32, scope_len u32, dispatch_version
    /// u64`, then `(anchor u32, lent u8, 3 zero bytes, stamp u64 or
    /// u64::MAX) x n`, then the scope bytes (InitialDBand: `cut i64`; G2':
    /// `n_pieces u32` then per piece `d_lo (u8 flag, i64), d_hi (u8 flag,
    /// i64), lower u16 x N, upper u16 x N`). Every count is a checked
    /// conversion: a record that does not fit is refused, never truncated.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let mut sorted: Vec<&AnchorRecord> = self.records.iter().collect();
        sorted.sort_by_key(|record| record.node);
        if sorted.windows(2).any(|pair| pair[0].node == pair[1].node) {
            return Err("anchor map: two records for one node".into());
        }
        let mut out = Vec::new();
        out.extend_from_slice(&ANCHORS_VERSION.to_le_bytes());
        out.extend_from_slice(&(sorted.len() as u64).to_le_bytes());
        for record in sorted {
            let scope = scope_bytes(&record.scope)?;
            let n = u32::try_from(record.anchors.len())
                .map_err(|_| format!("anchor record {}: too many anchors", record.node))?;
            let scope_len = u32::try_from(scope.len())
                .map_err(|_| format!("anchor record {}: scope too large", record.node))?;
            out.extend_from_slice(&record.node.to_le_bytes());
            out.push(record.kind as u8);
            out.push(0);
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&n.to_le_bytes());
            out.extend_from_slice(&scope_len.to_le_bytes());
            out.extend_from_slice(&record.dispatch_version.to_le_bytes());
            for a in &record.anchors {
                if a.stamp == Some(u64::MAX) {
                    return Err(format!("anchor record {}: stamp u64::MAX", record.node));
                }
                out.extend_from_slice(&a.anchor.to_le_bytes());
                out.push(a.lent as u8);
                out.extend_from_slice(&[0; 3]);
                out.extend_from_slice(&a.stamp.unwrap_or(u64::MAX).to_le_bytes());
            }
            out.extend_from_slice(&scope);
        }
        Ok(out)
    }

    /// Decode layout v2 (restore side of S3; round-trip tested). Refuses
    /// unknown kinds and lent codes, non-zero reserved bytes, unsorted
    /// nodes, non-canonical scope bytes and trailing bytes.
    pub fn decode(bytes: &[u8], arity: usize) -> Result<Vec<AnchorRecord>, String> {
        let mut r = super::job::Reader::new(bytes);
        let e = |m: &'static str| m.to_string();
        if r.u16().map_err(e)? != ANCHORS_VERSION {
            return Err("anchors version".into());
        }
        let count = r.u64().map_err(e)?;
        if arity > 32 || count > bytes.len() as u64 / 24 {
            return Err("anchor record count/arity exceeds bytes".into());
        }
        let mut records = Vec::new();
        let mut last: Option<u32> = None;
        for _ in 0..count {
            let node = r.u32().map_err(e)?;
            if last.is_some_and(|l| l >= node) {
                return Err("anchor records not sorted by node".into());
            }
            last = Some(node);
            let kind = AnchorKind::from_code(r.u8().map_err(e)?).ok_or("anchor kind")?;
            if r.u8().map_err(e)? != 0 || r.u16().map_err(e)? != 0 {
                return Err("anchor reserved bytes".into());
            }
            let n = r.u32().map_err(e)?;
            let scope_len = r.u32().map_err(e)? as usize;
            let dispatch_version = r.u64().map_err(e)?;
            if n == 0
                || n as usize > MAX_ANCHORS
                || scope_len > 4 + MAX_RESIDUAL_PIECES * (18 + 4 * arity)
                || (n as usize)
                    .checked_mul(16)
                    .and_then(|n| n.checked_add(scope_len))
                    .is_none_or(|n| n > bytes.len())
            {
                return Err("anchor counts exceed bounded record".into());
            }
            let mut anchors = Vec::new();
            for _ in 0..n {
                let anchor = r.u32().map_err(e)?;
                let lent = Lent::from_code(r.u8().map_err(e)?).ok_or("anchor lent scope")?;
                if r.u8().map_err(e)? != 0 || r.u16().map_err(e)? != 0 {
                    return Err("anchor padding".into());
                }
                let stamp = r.u64().map_err(e)?;
                anchors.push(AnchorRef {
                    anchor,
                    stamp: (stamp != u64::MAX).then_some(stamp),
                    lent,
                });
            }
            let scope = match kind {
                AnchorKind::InitialDBand => {
                    if scope_len != 8 {
                        return Err("InitialDBand scope length".into());
                    }
                    AnchorScope::DBandCut(r.i64().map_err(e)?)
                }
                AnchorKind::G2Native | AnchorKind::G2Residual => {
                    let pieces = r.u32().map_err(e)? as usize;
                    if pieces > MAX_RESIDUAL_PIECES || scope_len != 4 + pieces * (18 + 4 * arity) {
                        return Err("G2' scope length".into());
                    }
                    let mut out = Vec::new();
                    for _ in 0..pieces {
                        let d_lo = r.opt_i64().map_err(e)?;
                        let d_hi = r.opt_i64().map_err(e)?;
                        let lower = (0..arity)
                            .map(|_| r.u16())
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(e)?;
                        let upper = (0..arity)
                            .map(|_| r.u16())
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(e)?;
                        out.push(Piece {
                            d_lo,
                            d_hi,
                            lower,
                            upper,
                        });
                    }
                    AnchorScope::Residual(out)
                }
            };
            records.push(AnchorRecord {
                node,
                kind,
                dispatch_version,
                scope,
                anchors,
            });
        }
        r.finish().map_err(e)?;
        Ok(records)
    }
}

/// G2' planning source (§3.4, §7 R3): per bucket, `(id, lent scope)` of
/// every merged native with merge epoch <= v, appended in merge order, read
/// by the planner from the snapshot, never from ledger6. Built only with a
/// bound G2' flag (never in S2); W2 lands the type, the visibility rule and
/// P1's presence check.
#[derive(Default)]
pub(super) struct MergedView {
    /// `(bucket, id, merge epoch, lent scope)` in merge order.
    entries: Vec<(u32, u32, u64, Lent)>,
    by_id: std::collections::HashMap<u32, u64>,
}

impl MergedView {
    pub fn push(&mut self, bucket: u32, id: u32, epoch: u64, lent: Lent) {
        self.entries.push((bucket, id, epoch, lent));
        self.by_id.insert(id, epoch);
    }
    /// Anchors a job dispatched at `v0` may use: merged at an epoch <= v0.
    #[cfg(test)]
    pub fn visible(&self, bucket: u32, v0: u64) -> impl Iterator<Item = u32> + '_ {
        self.entries
            .iter()
            .filter(move |entry| entry.0 == bucket && entry.2 <= v0)
            .map(|entry| entry.1)
    }
    /// P1's presence check: `id` is in the view at version `v0`.
    pub fn contains(&self, id: u32, v0: u64) -> bool {
        self.by_id.get(&id).is_some_and(|&epoch| epoch <= v0)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
