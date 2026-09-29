//! Cross-section geometry and local state checks. This is still provisional:
//! record bodies, query roots, closure reconstruction and durable session
//! allocation must pass before any runnable EpochState can be constructed.
use super::super::super::anchors::{AnchorKind, AnchorMap, AnchorRecord, AnchorView, union_cover};
use super::super::super::ledger6::{EPOCH_LIMIT, Entry6, Ledger6, MAX_ATTEMPTS, MAX_GUARD, Tag};
use super::super::super::state::{
    NODE_ANCHORED, NODE_INSPECTED, NODE_RESIDUAL, NODE_SEALED, WalkCounters,
};
use super::super::super::store::{Store, bucket_key};
use super::super::super::verify::{Container, QueryImage, VerifyCounters, verify};
use super::super::invalid;
use super::EdgeStore;
use std::collections::BTreeMap;
use std::io;

// Only the owned, unpublished node buffer is used as scratch. Section decoding
// already rejects this bit; validate checks again before touching any flag.
const RUN_SEEN: u8 = 128;

pub(super) struct View<'a, const N: usize> {
    pub store: &'a Store<N>,
    pub ledger: &'a Ledger6,
    pub nodes: &'a mut [u8],
    pub live: &'a [u64],
    pub edges: &'a mut EdgeStore,
    pub anchors: &'a AnchorMap,
    pub frontier_counts: &'a BTreeMap<u32, u32>,
    pub closure_flags: Option<&'a [u8]>,
    pub walk: &'a WalkCounters,
    pub k: u64,
    pub p0: u32,
    pub input_frontiers: usize,
    pub records_digest: &'a str,
}

fn entry(ledger: &Ledger6, id: u32) -> io::Result<Entry6> {
    ledger
        .get(id)
        .map_err(|_| invalid("epoch cross-state ledger word"))
}

fn merged(entry: Entry6) -> bool {
    matches!(
        entry,
        Entry6::Native { .. }
            | Entry6::NativeFrontier { .. }
            | Entry6::NativeError { .. }
            | Entry6::Alias { .. }
            | Entry6::Abandoned { .. }
    )
}

pub(super) fn validate<const N: usize>(view: View<'_, N>) -> io::Result<()> {
    let count = view.store.len();
    if count >= u32::MAX as usize
        || view.p0 as usize > count
        || view.k >= EPOCH_LIMIT
        || view.ledger.words().len() != count
        || view.nodes.len() != count
        || view.live.len() != count.div_ceil(64)
        || view.closure_flags.is_some_and(|flags| flags.len() != count)
        || view
            .nodes
            .iter()
            .any(|flag| flag & !(NODE_SEALED | NODE_INSPECTED | NODE_ANCHORED | NODE_RESIDUAL) != 0)
    {
        return Err(invalid("epoch cross-state shape or node flags"));
    }
    let domains = &view.store.domains;
    let mut initial_inspected = 0;
    let mut frontier_sum = view.input_frontiers as u64;
    let mut verify_counters = VerifyCounters::default();
    for (id, &flags) in view.nodes.iter().enumerate() {
        let id = id as u32;
        let current = entry(view.ledger, id)?;
        let sealed = matches!(current, Entry6::Native { .. } | Entry6::Alias { .. });
        let inspected = matches!(
            current,
            Entry6::Native { .. } | Entry6::NativeFrontier { .. }
        );
        let anchor = view.anchors.get(id);
        if (flags & NODE_SEALED != 0) != sealed
            || (flags & NODE_INSPECTED != 0) != inspected
            || (flags & NODE_ANCHORED != 0) != anchor.is_some()
            || (flags & NODE_RESIDUAL != 0) != anchor.is_some_and(|a| a.kind.is_g2())
            || view.closure_flags.is_some_and(|closure| {
                let saved = closure[id as usize];
                saved & !7 != 0 || saved & 3 != flags & 3 || saved & 4 != 0 && !sealed
            })
        {
            return Err(invalid("epoch ledger/node/closure local flags differ"));
        }
        initial_inspected += u64::from(id < view.p0 && inspected);
        let epoch = match current {
            Entry6::Native {
                epoch,
                residual,
                dband,
            } => {
                if residual != anchor.is_some_and(|a| a.kind.is_g2())
                    || dband != anchor.is_some_and(|a| a.kind == AnchorKind::InitialDBand)
                {
                    return Err(invalid("epoch native anchor flags differ"));
                }
                Some(epoch)
            }
            Entry6::NativeFrontier { epoch } | Entry6::Abandoned { epoch } => Some(epoch),
            Entry6::NativeError { epoch, err } => {
                if !(1..=8).contains(&err) {
                    return Err(invalid("epoch native error class"));
                }
                Some(epoch)
            }
            Entry6::Alias { to } => {
                if id < view.p0
                    || to <= id
                    || to as usize >= count
                    || view.live[id as usize / 64] >> (id % 64) & 1 != 0
                {
                    return Err(invalid(
                        "epoch alias protected, live or outside forward range",
                    ));
                }
                let query = QueryImage::new(domains[id as usize]).map_err(io::Error::other)?;
                if verify(
                    Container::Stored {
                        id: to,
                        domains,
                        published_len: count,
                    },
                    &query,
                    &mut verify_counters,
                )
                .is_none()
                {
                    return Err(invalid(
                        "epoch alias representative does not contain its source",
                    ));
                }
                None
            }
            Entry6::Pending(counters) | Entry6::Reserved(counters) => {
                if counters.attempts >= MAX_ATTEMPTS || counters.guard >= MAX_GUARD {
                    return Err(invalid("epoch unmerged retry counters exceed live bounds"));
                }
                None
            }
            Entry6::Exhausted(counters) => {
                if counters.attempts < MAX_ATTEMPTS && counters.guard < MAX_GUARD {
                    return Err(invalid("epoch exhausted counters below exhaustion bounds"));
                }
                None
            }
        };
        // Raw ledger layout permits zero, but actual P3 stamps k+1: no
        // merged native can predate the first completed merge.
        if epoch.is_some_and(|epoch| epoch == 0 || epoch > view.k) {
            return Err(invalid(
                "epoch native merge version outside completed state",
            ));
        }
        if anchor.is_some() && epoch.is_none() {
            return Err(invalid("epoch anchor belongs to an unmerged source"));
        }
        match (current, view.frontier_counts.get(&id)) {
            (Entry6::NativeFrontier { .. }, Some(&n)) if n != 0 => {
                frontier_sum = frontier_sum
                    .checked_add(u64::from(n))
                    .ok_or_else(|| invalid("epoch frontier aggregate overflow"))?;
            }
            (Entry6::NativeFrontier { .. }, _) | (_, Some(_)) => {
                return Err(invalid(
                    "epoch sparse frontier inventory differs from C4 ledger",
                ));
            }
            _ => {}
        }
        if let Some(record) = anchor {
            let epoch = epoch.expect("anchor requires a merged entry above");
            if domains[id as usize].phase() != super::super::super::super::queue::Phase::Apply
                || record.dispatch_version >= epoch
            {
                return Err(invalid(
                    "epoch anchor dispatch version is not before its merge",
                ));
            }
            let same_bucket = |a: u32, b: u32| {
                (a as usize) < count
                    && (b as usize) < count
                    && bucket_key(&domains[a as usize]) == bucket_key(&domains[b as usize])
            };
            let record_of = |node| {
                view.anchors
                    .get(node)
                    .map(|record| (record.kind, record.d_band().map(|(_, cut)| cut)))
            };
            let cut_of = |node| {
                view.anchors
                    .get(node)
                    .and_then(AnchorRecord::d_band)
                    .map(|(_, cut)| cut)
            };
            let cover = |record: &AnchorRecord| {
                union_cover(&domains[id as usize], domains, record, &cut_of)
            };
            let eligible =
                |id, _v0| super::super::super::g2::eligible(domains, view.ledger, view.anchors, id);
            record
                .validate(
                    &AnchorView {
                        p0: view.p0,
                        published_len: count,
                        arity: N,
                        ledger: view.ledger,
                        same_bucket: &same_bucket,
                        record_of: &record_of,
                        edges_of: None,
                        merged_view: Some(&eligible),
                        cover: &cover,
                    },
                    epoch,
                )
                .map_err(|_| invalid("epoch anchor geometry or provenance"))?;
        }
    }
    if view.frontier_counts.len() as u64 != view.ledger.counts().get(Tag::NativeFrontier)
        || view
            .anchors
            .records()
            .iter()
            .filter(|r| r.kind == AnchorKind::InitialDBand)
            .count() as u64
            != view.walk.partials
        || view
            .anchors
            .records()
            .iter()
            .filter(|r| r.kind.is_g2())
            .count() as u64
            != view.walk.g2_records
        || view
            .anchors
            .records()
            .iter()
            .any(|record| record.node as usize >= count)
    {
        return Err(invalid("epoch anchor or frontier inventory count differs"));
    }
    let counts = view.ledger.counts();
    if view.walk.natives
        != counts.get(Tag::Native) + counts.get(Tag::NativeFrontier) + counts.get(Tag::NativeError)
        || view.walk.completed != counts.get(Tag::Native) + counts.get(Tag::NativeFrontier)
        || view.walk.native_errors != counts.get(Tag::NativeError)
        || view.walk.aliases != counts.get(Tag::Alias)
        || view.walk.transfers != view.walk.aliases
        || view.walk.initial_inspected != initial_inspected
        || view.walk.merges != view.k
        || frontier_sum > view.walk.frontiers
        || counts.get(Tag::NativeError) == 0 && frontier_sum != view.walk.frontiers
    {
        return Err(invalid("epoch ledger-derived walk counters differ"));
    }
    // Input obligations and sparse C4 counts form only a lower bound on the
    // aggregate: C2 error prefixes may contribute too. Bodies prove the remainder.
    let result = (|| {
        let mut previous_epoch = 0;
        for (source, targets) in view.edges.run_iter() {
            let flags = view
                .nodes
                .get_mut(source as usize)
                .ok_or_else(|| invalid("epoch run source range"))?;
            if *flags & RUN_SEEN != 0 {
                return Err(invalid("epoch repeated run source"));
            }
            *flags |= RUN_SEEN;
            match entry(view.ledger, source)? {
                Entry6::Alias { to } if targets == [to] => {}
                Entry6::Native { epoch, .. }
                | Entry6::NativeFrontier { epoch }
                | Entry6::NativeError { epoch, .. }
                | Entry6::Abandoned { epoch }
                    if epoch >= previous_epoch =>
                {
                    previous_epoch = epoch;
                    if matches!(entry(view.ledger, source)?, Entry6::Abandoned { .. })
                        && !targets.is_empty()
                    {
                        return Err(invalid("abandoned obligation has dependency edges"));
                    }
                }
                _ => return Err(invalid("epoch run source tag, alias target or merge order")),
            }
            if let Some(record) = view.anchors.get(source)
                && record
                    .anchors
                    .iter()
                    .any(|anchor| targets.binary_search(&anchor.anchor).is_err())
            {
                return Err(invalid("epoch anchor dependency missing from run"));
            }
            if view.closure_flags.is_some_and(|flags| {
                flags[source as usize] & 4 != 0
                    && targets
                        .iter()
                        .any(|&target| flags[target as usize] & 4 == 0)
            }) {
                return Err(invalid(
                    "epoch closed source depends on an unclosed descendant",
                ));
            }
        }
        for (id, &flag) in view.nodes.iter().enumerate() {
            if (flag & RUN_SEEN != 0) != merged(entry(view.ledger, id as u32)?) {
                return Err(invalid("epoch merged ledger entry has no unique run"));
            }
        }
        Ok(())
    })();
    // Every ordinary error path clears scratch, even though failed provisional
    // state is discarded by assembly. No published/live state is touched.
    for flag in view.nodes {
        *flag &= !RUN_SEEN;
    }
    result?;
    view.edges
        .restore_records_digest(view.ledger, view.records_digest)
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests;
