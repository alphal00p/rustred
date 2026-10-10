//! Rebuild lookup state without inventing a different live antichain.
//! The historical dominant full orthant is independent of live membership.
use super::super::{Digest, invalid};
use super::{CheckedRead, Query, QueryImage, Store};
use crate::application::routed_campaign::walking::epoch::store::bucket_key;
use crate::application::routed_campaign::walking::queue::rank_contains;
use std::io::{self, Read};
use std::path::Path;

/// Consume a provisional arena from the domain decoder. This still does not
/// validate the ledger/records/roots or produce a dispatchable EpochState.
pub(super) fn rebuild<const N: usize>(
    directory: &Path,
    generation: u64,
    digest: &Digest,
    bucket_count: u64,
    store: Store<N>,
    live: &[u64],
) -> io::Result<Store<N>> {
    rebuild_with_arity(directory, generation, digest, bucket_count, store, live, N)
}

pub(super) fn rebuild_with_arity<const N: usize>(
    directory: &Path,
    generation: u64,
    digest: &Digest,
    bucket_count: u64,
    mut store: Store<N>,
    live: &[u64],
    wire_arity: usize,
) -> io::Result<Store<N>> {
    let count = store.len();
    if generation == 0
        || !crate::application::routed_campaign::storage::compatible_width(wire_arity, N)
        || count >= u32::MAX as usize
        || live.len() != count.div_ceil(64)
        || count % 64 != 0 && live.last().is_some_and(|word| word >> (count % 64) != 0)
        || bucket_count != store.buckets.len() as u64
        || store
            .buckets
            .iter()
            .any(|bucket| bucket.index.storage().rows != 0 || bucket.orthant.is_some())
    {
        return Err(invalid(
            "epoch lookup restore inventory or nonempty destination",
        ));
    }
    let mut reader = CheckedRead::open(
        directory,
        &format!("epoch-{generation:020}-orthants.part"),
        digest.bytes,
        digest.blake3,
    )?;
    let mut magic = [0; 8];
    reader.read_exact(&mut magic)?;
    if &magic != b"EPORTH01"
        || reader.u64()? != bucket_count
        || bucket_count.checked_mul(4) != Some(reader.remaining())
    {
        return Err(invalid("epoch orthant header/count/length differs"));
    }
    // Append each persisted live ID once to the empty indexes checked above.
    // No deletion creates empty rows, so there is no retirement/compaction
    // work: every persisted live ID survives, even if another contains it.
    for id in 0..count {
        if live[id / 64] & (1 << (id % 64)) == 0 {
            continue;
        }
        let image = store.domains[id];
        let image_query = QueryImage::new(image).map_err(invalid)?;
        let query = Query::new(image_query.core, image.phase());
        store
            .index_restored(id as u32, &query)
            .map_err(io::Error::other)?;
    }
    // index_restored updated orthants for the live subset. Replace those
    // slots by the exact same rule over the full historical insertion order.
    for bucket in &mut store.buckets {
        bucket.orthant = None;
    }
    let mut saved_history = Vec::new();
    saved_history
        .try_reserve_exact(store.buckets.len())
        .map_err(|_| invalid("epoch orthant history allocation"))?;
    saved_history.resize(store.buckets.len(), None);
    for (id, image) in store.domains.iter().enumerate() {
        let runtime_full = image.is_full_orthant();
        // The saved slots describe the original wire coordinate space. A
        // physical full orthant acquires fixed-zero capacity axes on restore,
        // so it no longer qualifies for the unchanged runtime-N shortcut.
        // Validate its original historical representative before rebuilding
        // runtime slots; subsequent saves then describe their own wire width.
        let saved_full = if wire_arity == N {
            runtime_full
        } else {
            let (lower, upper) = image.raw_bounds();
            wire_arity < N
                && image.powers().is_unconstrained()
                && lower.iter().all(|&value| value == 0)
                && upper[..wire_arity].iter().all(|&value| value == u16::MAX)
                && upper[wire_arity..].iter().all(|&value| value == 0)
                && image.owner()[wire_arity..].iter().all(|&active| !active)
        };
        // When narrowing, a full wire orthant would have open omitted axes
        // and was already refused by the authenticated domain decoder.
        if !runtime_full && !saved_full {
            continue;
        }
        let bucket_id = store.bucket_of[&bucket_key(image)] as usize;
        let bucket = &mut store.buckets[bucket_id];
        if runtime_full
            && bucket
                .orthant
                .is_none_or(|old| rank_contains(image.rank(), store.domains[old as usize].rank()))
        {
            bucket.orthant = Some(id as u32);
        }
        if saved_full
            && saved_history[bucket_id].is_none_or(|old: u32| {
                rank_contains(image.rank(), store.domains[old as usize].rank())
            })
        {
            saved_history[bucket_id] = Some(id as u32);
        }
    }
    for expected in saved_history {
        let saved = reader.u32()?;
        if saved != expected.unwrap_or(u32::MAX) {
            return Err(invalid("epoch saved dominant orthant differs from history"));
        }
    }
    reader.finish()?;
    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::super::super::super::super::queue::{CompactDomain, Domain, Phase, Signature};
    use super::super::super::tests::Directory;
    use super::*;
    use rustred::solver::DomainPowerBounds;
    use std::fs;

    fn arena() -> Store<2> {
        let mut store = Store::new();
        // Three historical full orthants in bucket 0: the final unbounded
        // one dominates both finite ranks. Its two predecessors are live in
        // this deliberately isolated lookup fixture. Bucket 1 has no full
        // orthant, testing the u32::MAX slot without a dense-ID assumption.
        for (owner, upper, rank) in [
            ([true, false], vec![None, None], Some(1)),
            ([true, false], vec![None, None], Some(2)),
            ([true, false], vec![None, None], None),
            ([false, true], vec![Some(1), Some(1)], Some(1)),
        ] {
            let domain = Domain {
                phase: Phase::Apply,
                owner,
                lower: vec![0, 0],
                upper,
                rank,
                powers: DomainPowerBounds::default(),
            };
            let image = CompactDomain::try_from_domain(&domain).unwrap();
            let query = QueryImage::new(image).unwrap();
            let compact = Query::new(query.core, image.phase()).compact;
            store.try_reserve(1).unwrap();
            store.exact.try_reserve_one(query.digest).unwrap();
            store.push(image, compact, query.digest).unwrap();
        }
        store
    }

    fn orthants(directory: &Path, values: &[u32], claimed: u64) -> Digest {
        let mut bytes = b"EPORTH01".to_vec();
        bytes.extend_from_slice(&claimed.to_le_bytes());
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        fs::write(
            directory.join("epoch-00000000000000000001-orthants.part"),
            &bytes,
        )
        .unwrap();
        Digest {
            bytes: bytes.len() as u64,
            blake3: *blake3::hash(&bytes).as_bytes(),
        }
    }

    fn append(store: &mut Store<2>, domain: Domain<2>) {
        let image = CompactDomain::try_from_domain(&domain).unwrap();
        let query = QueryImage::new(image).unwrap();
        let compact = Query::new(query.core, image.phase()).compact;
        store.try_reserve(1).unwrap();
        store.exact.try_reserve_one(query.digest).unwrap();
        store.push(image, compact, query.digest).unwrap();
    }

    fn ray(phase: Phase, owner: [bool; 2], offset: u64, rank: Option<u32>) -> Domain<2> {
        Domain {
            phase,
            owner,
            lower: vec![offset + 1, offset + 2],
            upper: vec![None, None],
            rank,
            powers: DomainPowerBounds::default(),
        }
    }

    #[test]
    fn append_only_restore_matches_empty_retirement_across_buckets_and_ranks() {
        use super::super::super::super::store::Hit;

        let mixed_arena = || {
            let mut store = arena();
            for offset in 0..192 {
                for (phase, owner) in [
                    (Phase::Apply, [true, false]),
                    (Phase::Apply, [false, true]),
                    (Phase::Route, [true, false]),
                ] {
                    append(
                        &mut store,
                        ray(phase, owner, offset, (offset % 3 == 0).then_some(1000)),
                    );
                }
            }
            store
        };
        let mut old = mixed_arena();
        let mut live = vec![0; old.len().div_ceil(64)];
        for id in 0..old.len() {
            // Preserve a sparse membership, including contained live IDs,
            // while the historical dominant full orthant (ID 2) is retired.
            if id == 2 || id >= 4 && (id % 7 == 2 || id % 11 == 3) {
                continue;
            }
            live[id / 64] |= 1 << (id % 64);
            let image = old.domains[id];
            let query = QueryImage::new(image).unwrap();
            let query = Query::new(query.core, image.phase());
            assert_eq!(old.index_survivor(id as u32, &query, &[]).unwrap(), 0);
        }
        old.buckets[0].orthant = Some(2);
        let directory = Directory::new();
        let digest = orthants(&directory.0, &[2, u32::MAX, u32::MAX], 3);
        let restored = rebuild(&directory.0, 1, &digest, 3, mixed_arena(), &live).unwrap();
        assert_eq!(restored.domains, old.domains);
        assert_eq!(restored.summaries, old.summaries);
        assert_eq!(restored.bucket_of, old.bucket_of);
        assert_eq!(restored.exact.len(), old.exact.len());
        assert_eq!(restored.max_finite_rank, old.max_finite_rank);
        assert_eq!(restored.unbounded_rank_domains, old.unbounded_rank_domains);
        let mut multiple_blocks = false;
        let mut multiple_signatures = false;
        for (actual, expected) in restored.buckets.iter().zip(&old.buckets) {
            assert_eq!(actual.orthant, expected.orthant);
            assert_eq!(actual.index.storage(), expected.index.storage());
            assert_eq!(actual.index.ids(), expected.index.ids());
            let layout = serde_json::to_value(&actual.index).unwrap();
            assert_eq!(layout, serde_json::to_value(&expected.index).unwrap());
            let groups = layout["groups"].as_array().unwrap();
            multiple_signatures |= groups.len() > 1;
            for group in groups {
                let blocks = group["blocks"].as_array().unwrap();
                assert!(!blocks.is_empty());
                multiple_blocks |= blocks.len() > 1;
                assert!(
                    blocks
                        .iter()
                        .all(|block| block["len"].as_u64().unwrap() > 0)
                );
            }
            assert_eq!(actual.index.work().groups_visited, 0);
            assert_eq!(actual.index.work().blocks_visited, 0);
            assert!(expected.index.work().groups_visited > 0);
        }
        assert!(multiple_blocks && multiple_signatures);
        for (id, image) in restored.domains.iter().enumerate() {
            let query = QueryImage::new(*image).unwrap();
            assert_eq!(
                restored.exact.get(query.digest, image, &restored.domains),
                Some(id as u32)
            );
            let bucket = &restored.buckets[restored.bucket_of[&bucket_key(image)] as usize];
            assert_eq!(
                bucket.index.is_live(Signature::of(&query.core), id),
                live[id / 64] & (1 << (id % 64)) != 0
            );
        }

        let mut queries = restored.domains.clone();
        for (phase, owner, lower) in [
            (Phase::Apply, [true, false], 600),
            (Phase::Apply, [false, true], 600),
            (Phase::Route, [true, false], 600),
            (Phase::Route, [true, false], 0),
            (Phase::Route, [true, true], 0),
        ] {
            queries.push(
                CompactDomain::try_from_domain(&Domain {
                    phase,
                    owner,
                    lower: vec![lower; 2],
                    upper: vec![Some(lower + 2); 2],
                    rank: Some(1000),
                    powers: DomainPowerBounds::default(),
                })
                .unwrap(),
            );
        }
        let mut kinds = [false; 4];
        for image in queries {
            let q = QueryImage::new(image).unwrap();
            let query = Query::new(q.core.clone(), image.phase());
            let evaluate = |store: &Store<2>| {
                let mut lookup = Default::default();
                let mut verify = Default::default();
                let result = store
                    .lookup(&q, &query, store.len(), &mut lookup, &mut verify)
                    .unwrap();
                let contained = store.contained_live(&q, &query, &mut lookup).unwrap();
                (result, contained, lookup, verify)
            };
            let actual = evaluate(&restored);
            assert_eq!(actual, evaluate(&old));
            kinds[match actual.0 {
                Some((_, _, Hit::Exact)) => 0,
                Some((_, _, Hit::Orthant)) => 1,
                Some((_, _, Hit::Contained)) => 2,
                None => 3,
            }] = true;
        }
        assert_eq!(kinds, [true; 4]);
    }

    #[test]
    fn append_only_restore_eliminates_growing_empty_retirement_scans() {
        let mut previous_blocks = 0;
        for count in [64, 128, 256] {
            let mut direct = Store::new();
            let mut old = Store::new();
            for id in 0..count {
                let domain = ray(Phase::Route, [true, false], id as u64, None);
                append(&mut direct, domain.clone());
                append(&mut old, domain);
                let image = direct.domains[id];
                let q = QueryImage::new(image).unwrap();
                let query = Query::new(q.core, image.phase());
                direct.index_restored(id as u32, &query).unwrap();
                assert_eq!(old.index_survivor(id as u32, &query, &[]).unwrap(), 0);
            }
            let actual = &direct.buckets[0].index;
            let expected = &old.buckets[0].index;
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
            assert_eq!(actual.storage(), expected.storage());
            assert_eq!(actual.work().groups_visited, 0);
            assert_eq!(actual.work().blocks_visited, 0);
            assert_eq!(expected.work().groups_visited, count - 1);
            let scanned = expected.work().blocks_visited;
            assert!(scanned > 3 * previous_blocks);
            previous_blocks = scanned;
        }
    }

    #[test]
    fn retired_historical_orthant_and_exact_live_subset_restore_independently() {
        let directory = Directory::new();
        let digest = orthants(&directory.0, &[2, u32::MAX], 2);
        // IDs 0 and 1 are both persisted live, although one contains the
        // other: restore must not compute a new antichain and retire ID 0.
        let store = rebuild(&directory.0, 1, &digest, 2, arena(), &[0b1011]).unwrap();
        assert_eq!(store.buckets[0].orthant, Some(2));
        assert_eq!(store.buckets[1].orthant, None);
        for (id, image) in store.domains.iter().enumerate() {
            let core = image.try_native_summary().unwrap();
            let bucket = &store.buckets[store.bucket_of[&bucket_key(image)] as usize];
            assert_eq!(bucket.index.is_live(Signature::of(&core), id), id != 2);
        }
        assert_eq!(
            store
                .buckets
                .iter()
                .map(|b| b.index.storage().live)
                .sum::<usize>(),
            3
        );
        assert!(
            rebuild(&directory.0, 1, &digest, 2, store, &[0b1011]).is_err(),
            "only an empty provisional index may be rebuilt"
        );
    }

    #[test]
    fn actual_writer_domains_live_and_orthants_feed_the_same_restore_contract() {
        use super::super::super::super::dispatch::Dispatch;
        use super::super::super::{MergeBoundary, Section};
        let directory = Directory::new();
        let mut state = super::super::super::tests::state(4);
        state.store = arena().into();
        state.store.unique_mut().unwrap().buckets[0].orthant = Some(2);
        state.live[0] = 0b1011;
        let dispatch = Dispatch::new();
        let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
        let domains = boundary
            .write_new_section(&directory.0, 1, Section::Domains)
            .unwrap();
        let live = boundary
            .write_new_section(&directory.0, 1, Section::Live)
            .unwrap();
        let (bytes, digest) =
            super::super::super::publication::write_orthants(&boundary, Vec::new()).unwrap();
        fs::write(
            directory.0.join("epoch-00000000000000000001-orthants.part"),
            bytes,
        )
        .unwrap();
        let arena = super::super::FixedSection::<2>::open(&directory.0, &domains, 4)
            .unwrap()
            .domains()
            .unwrap();
        let live = super::super::FixedSection::<2>::open(&directory.0, &live, 1)
            .unwrap()
            .live(4)
            .unwrap();
        let restored = rebuild(&directory.0, 1, &digest, 2, arena, &live).unwrap();
        assert_eq!(restored.domains, state.store.domains);
        assert_eq!(restored.buckets[0].orthant, Some(2));
        assert_eq!(
            restored
                .buckets
                .iter()
                .map(|bucket| bucket.index.storage().live)
                .sum::<usize>(),
            3
        );
    }

    #[test]
    fn sparse_orthant_shape_rank_history_and_live_padding_mutations_fail() {
        let directory = Directory::new();
        for slots in [[1, u32::MAX], [u32::MAX, u32::MAX], [2, 3], [2, 99]] {
            let digest = orthants(&directory.0, &slots, 2);
            assert!(rebuild(&directory.0, 1, &digest, 2, arena(), &[0b1011]).is_err());
        }
        for (values, claimed) in [(vec![2], 2), (vec![2, u32::MAX], u64::MAX)] {
            let digest = orthants(&directory.0, &values, claimed);
            assert!(rebuild(&directory.0, 1, &digest, 2, arena(), &[0b1011]).is_err());
        }
        let mut digest = orthants(&directory.0, &[2, u32::MAX], 2);
        assert!(rebuild(&directory.0, 1, &digest, 2, arena(), &[0b10000]).is_err());
        assert!(rebuild(&directory.0, 1, &digest, 2, arena(), &[]).is_err());
        digest.blake3[0] ^= 1;
        assert!(rebuild(&directory.0, 1, &digest, 2, arena(), &[0b1011]).is_err());
    }

    #[test]
    #[cfg(feature = "capacity-dispatch")]
    fn physical_orthant_history_is_authenticated_before_capacity_lookup_and_resave() {
        use super::super::super::super::{dispatch::Dispatch, state::EpochState, store::Hit};
        use super::super::super::{MergeBoundary, Section};
        use crate::application::routed_campaign::walking::{
            descendant_closure::Tracker, epoch::admit_initial,
        };

        let directory = Directory::new();
        let mut source = super::super::super::tests::state(4);
        source.store = arena().into();
        source.store.unique_mut().unwrap().buckets[0].orthant = Some(2);
        source.live[0] = 0b1011;
        let dispatch = Dispatch::new();
        let boundary = MergeBoundary::borrow(&source, &dispatch, 16).unwrap();
        let domains = boundary
            .write_new_section(&directory.0, 1, Section::Domains)
            .unwrap();
        let decode = || {
            super::super::FixedSection::<4>::open_with_arity(&directory.0, &domains, 4, 2)
                .unwrap()
                .domains()
                .unwrap()
        };
        // Even a valid full orthant with a weaker rank is not the historical
        // representative. Rehashing its slot cannot bypass this check.
        for slots in [[1, u32::MAX], [u32::MAX, u32::MAX], [2, 3]] {
            let digest = orthants(&directory.0, &slots, 2);
            assert!(
                rebuild_with_arity(&directory.0, 1, &digest, 2, decode(), &[0b1011], 2).is_err()
            );
        }
        let digest = orthants(&directory.0, &[2, u32::MAX], 2);
        let restored =
            rebuild_with_arity(&directory.0, 1, &digest, 2, decode(), &[0b1011], 2).unwrap();
        assert_eq!(
            restored.domains[2].raw_bounds(),
            (&[0, 0, 0, 0], &[u16::MAX, u16::MAX, 0, 0])
        );
        assert!(
            restored
                .buckets
                .iter()
                .all(|bucket| bucket.orthant.is_none())
        );
        let image = CompactDomain::try_from_domain(&Domain {
            phase: Phase::Apply,
            owner: [true, false, false, false],
            lower: vec![0; 4],
            upper: vec![Some(1), Some(1), Some(0), Some(0)],
            rank: Some(1),
            powers: Default::default(),
        })
        .unwrap();
        let q = QueryImage::new(image).unwrap();
        let query = Query::new(q.core.clone(), image.phase());
        let (id, _, hit) = restored
            .lookup(
                &q,
                &query,
                4,
                &mut Default::default(),
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(id, 0);
        assert!(matches!(hit, Hit::Contained));

        // The normal writer persists the runtime capacity predicate. A later
        // capacity-width reload must not expect old physical orthant slots.
        let mut capacity = EpochState::<4>::new(100_000, usize::MAX, usize::MAX);
        for image in &restored.domains {
            admit_initial(&mut capacity, &image.expand()).unwrap();
        }
        assert_eq!(capacity.watermark(), 4);
        capacity.p0 = 4;
        capacity.tracker = Tracker::new(4);
        capacity.store = restored.into();
        capacity.live[0] = 0b1011;
        let directory = Directory::new();
        let boundary = MergeBoundary::borrow(&capacity, &dispatch, 16).unwrap();
        let domains = boundary
            .write_new_section(&directory.0, 1, Section::Domains)
            .unwrap();
        let (bytes, digest) =
            super::super::super::publication::write_orthants(&boundary, Vec::new()).unwrap();
        fs::write(
            directory.0.join("epoch-00000000000000000001-orthants.part"),
            bytes,
        )
        .unwrap();
        let arena = super::super::FixedSection::<4>::open(&directory.0, &domains, 4)
            .unwrap()
            .domains()
            .unwrap();
        let again = rebuild(&directory.0, 1, &digest, 2, arena, &[0b1011]).unwrap();
        assert_eq!(again.domains, capacity.store.domains);
        assert!(again.buckets.iter().all(|bucket| bucket.orthant.is_none()));
    }
}
