//! The verify chokepoint (A1, W2.0 protocol §5.3). Every positive of the
//! epoch engine (an edge target, a transfer, an anchor) is a `Verified`
//! token, and `Verified` has a private constructor: only `verify` (one
//! container contains q) and `verify_cover` (G2' anchors: q is covered by
//! its residual plus the anchors' lent scopes, exactly) make one.
//! Phase and owner are compared explicitly, because the native summary
//! predicate compares owner but not phase and accepts an EMPTY candidate
//! before the owner test. Index lanes and stored summaries are prefilters
//! only; the authority is raw inclusion or the native predicate recomputed
//! on the canonical image.
use super::super::queue::CompactDomain;
use super::anchors::{AnchorRecord, union_cover};
use rustred::solver::DomainPowerSummary;

/// The query side of a containment test: the canonical image, its native
/// summary and its digest, all derived from the one image (F1).
#[derive(Clone)]
pub(super) struct QueryImage<const N: usize> {
    pub image: CompactDomain<N>,
    pub core: DomainPowerSummary<N>,
    pub digest: u64,
}

impl<const N: usize> QueryImage<N> {
    pub fn new(image: CompactDomain<N>) -> Result<Self, &'static str> {
        let core = image
            .try_native_summary()
            .map_err(|_| "query image has no native summary")?;
        Ok(Self {
            digest: image.digest().0,
            image,
            core,
        })
    }
}

/// Where a container lives. `Stored` reads a prefix of the canonical arena;
/// `JobMiss` is an earlier miss of the same inspection (inspector only, S4);
/// `Planned` is a survivor of the cut being merged, before ID assignment.
pub(super) enum Container<'a, const N: usize> {
    Stored {
        id: u32,
        domains: &'a [CompactDomain<N>],
        published_len: usize,
    },
    /// Same global ID binding as Stored, backed by immutable shared pages.
    /// The verifier itself obtains the image at ID; callers cannot provide an
    /// unrelated image alongside an otherwise plausible stored ID.
    Snapshot {
        id: u32,
        domains: &'a super::snapshot::shared::Pages<CompactDomain<N>>,
        published_len: usize,
    },
    #[allow(dead_code)] // inspector-side Local resolution lands in S4
    JobMiss {
        ordinal: u32,
        current: u32,
        image: &'a CompactDomain<N>,
    },
    Planned {
        pos: u32,
        survivors: usize,
        image: &'a CompactDomain<N>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ContainerRef {
    Stored(u32),
    JobMiss(u32),
    Planned(u32),
}

/// Proof that `container ⊇ q` held when the token was made. Private fields:
/// constructed only by `verify`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Verified {
    container: ContainerRef,
    q_digest: u64,
}

/// A token whose container is a final ID (P3 maps `Planned` positions to
/// IDs through the provisional-position bijection; no containment decision).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VerifiedId {
    id: u32,
    q_digest: u64,
}

impl Verified {
    /// `Stored(id)` stays `id`; `Planned(pos)` becomes `first_new + pos`.
    /// None for a `JobMiss` token (never shipped across the merge).
    pub fn into_id(self, first_new: u32) -> Option<VerifiedId> {
        let id = match self.container {
            ContainerRef::Stored(id) => id,
            ContainerRef::Planned(pos) => first_new.checked_add(pos)?,
            ContainerRef::JobMiss(_) => return None,
        };
        Some(VerifiedId {
            id,
            q_digest: self.q_digest,
        })
    }
}

impl VerifiedId {
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn q_digest(&self) -> u64 {
        self.q_digest
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct VerifyCounters {
    pub calls: u64,
    pub accepted: u64,
    pub raw_inclusions: u64,
    /// Native predicate recomputed on the canonical image (§5.3 step 4).
    pub recomputes: u64,
    pub refused_range: u64,
    pub refused_bucket: u64,
    /// Exact union covers decided for G2' anchors (`verify_cover`).
    pub union_covers: u64,
}

impl VerifyCounters {
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({"calls":self.calls,"accepted":self.accepted,
            "raw_inclusions":self.raw_inclusions,"native_recomputes":self.recomputes,
            "refused_range":self.refused_range,"refused_phase_owner":self.refused_bucket,
            "union_covers":self.union_covers})
    }
    pub fn add(&mut self, other: &Self) {
        self.calls += other.calls;
        self.accepted += other.accepted;
        self.raw_inclusions += other.raw_inclusions;
        self.recomputes += other.recomputes;
        self.refused_range += other.refused_range;
        self.refused_bucket += other.refused_bucket;
        self.union_covers += other.union_covers;
    }
}

/// `container ⊇ q`, in four steps identical for every variant: range; phase
/// and owner equal (explicit); raw inclusion (sufficient); otherwise the
/// native predicate on the canonical image, recomputed.
pub(super) fn verify<const N: usize>(
    container: Container<'_, N>,
    q: &QueryImage<N>,
    counters: &mut VerifyCounters,
) -> Option<Verified> {
    counters.calls += 1;
    let (reference, image) = match container {
        Container::Stored {
            id,
            domains,
            published_len,
        } => {
            if (id as usize) >= published_len.min(domains.len()) {
                counters.refused_range += 1;
                return None;
            }
            (ContainerRef::Stored(id), &domains[id as usize])
        }
        Container::Snapshot {
            id,
            domains,
            published_len,
        } => {
            if (id as usize) >= published_len {
                counters.refused_range += 1;
                return None;
            }
            let Some(image) = domains.get(id as usize) else {
                counters.refused_range += 1;
                return None;
            };
            (ContainerRef::Stored(id), image)
        }
        Container::JobMiss {
            ordinal,
            current,
            image,
        } => {
            if ordinal >= current {
                counters.refused_range += 1;
                return None;
            }
            (ContainerRef::JobMiss(ordinal), image)
        }
        Container::Planned {
            pos,
            survivors,
            image,
        } => {
            if pos as usize >= survivors {
                counters.refused_range += 1;
                return None;
            }
            (ContainerRef::Planned(pos), image)
        }
    };
    if image.phase() != q.image.phase() || image.owner() != q.image.owner() {
        counters.refused_bucket += 1;
        return None;
    }
    let included = if image.contains(&q.image) {
        counters.raw_inclusions += 1;
        true
    } else {
        counters.recomputes += 1;
        match image.try_native_summary() {
            Ok(summary) => summary.contains(&q.core),
            Err(_) => false,
        }
    };
    if !included {
        return None;
    }
    counters.accepted += 1;
    Some(Verified {
        container: reference,
        q_digest: q.digest,
    })
}

/// One local evaluation of the exact union predicate. Its inputs stay borrowed
/// until token creation: it cannot be substituted for another record/arena or
/// serialized, cloned, or reused by a later cut. A negative or undecided result
/// is retained only to report the original validator verdict, never as proof.
/// Construction always runs the same bounded oracle as restore and export.
/// P1's cut callback reads only its immutably borrowed anchor map; the `Fn`
/// bound alone does not promise purity for arbitrary diagnostic/test callbacks.
pub(super) struct UnionCover<'a, const N: usize> {
    record: &'a AnchorRecord,
    node: &'a CompactDomain<N>,
    domains: &'a [CompactDomain<N>],
    published_len: usize,
    _cut_of: &'a dyn Fn(u32) -> Option<i64>,
    verdict: Option<bool>,
}

impl<'a, const N: usize> UnionCover<'a, N> {
    pub fn evaluate(
        record: &'a AnchorRecord,
        node: &'a CompactDomain<N>,
        domains: &'a [CompactDomain<N>],
        published_len: usize,
        cut_of: &'a dyn Fn(u32) -> Option<i64>,
    ) -> Self {
        Self {
            record,
            node,
            domains,
            published_len,
            _cut_of: cut_of,
            verdict: union_cover(node, domains, record, cut_of),
        }
    }

    pub fn verdict(&self) -> Option<bool> {
        self.verdict
    }
}

/// G2' anchor edges (§5.3 "anchor edges: exact cover predicate"): one token
/// per anchor of `record`, all naming the node's image `q`, iff every anchor
/// is a stored container in range (`Stored`, id < `published_len`) of q's
/// phase and owner (explicit) and `q ⊆ residual ∪ lent scopes` holds exactly
/// (`lattice::Cell::covered_by_union`, the oracle's predicate; undecided is
/// refused). Consume the one-use union evaluation from P1's structural
/// validator, after QueryImage construction in its original error order.
/// Counter positions and the range/bucket checks are unchanged; no caller can
/// supply a boolean in place of the bounded oracle's result.
pub(super) fn verify_cover<const N: usize>(
    record: &AnchorRecord,
    q: &QueryImage<N>,
    cover: UnionCover<'_, N>,
    counters: &mut VerifyCounters,
) -> Option<Vec<Verified>> {
    counters.calls += 1;
    if !std::ptr::eq(record, cover.record) || q.image != *cover.node {
        return None;
    }
    let domains = cover.domains;
    let published_len = cover.published_len;
    for a in &record.anchors {
        if (a.anchor as usize) >= published_len.min(domains.len()) {
            counters.refused_range += 1;
            return None;
        }
        let image = &domains[a.anchor as usize];
        if image.phase() != q.image.phase() || image.owner() != q.image.owner() {
            counters.refused_bucket += 1;
            return None;
        }
    }
    counters.union_covers += 1;
    if cover.verdict != Some(true) {
        return None;
    }
    counters.accepted += 1;
    Some(
        record
            .anchors
            .iter()
            .map(|a| Verified {
                container: ContainerRef::Stored(a.anchor),
                q_digest: q.digest,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
