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
    mut store: Store<N>,
    live: &[u64],
) -> io::Result<Store<N>> {
    let count = store.len();
    if generation == 0
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
    // Reuse the established index insertion, but never recompute a retire
    // set: every persisted live ID survives, even if another contains it.
    for id in 0..count {
        if live[id / 64] & (1 << (id % 64)) == 0 {
            continue;
        }
        let image = store.domains[id];
        let image_query = QueryImage::new(image).map_err(invalid)?;
        let query = Query::new(image_query.core, image.phase());
        if store
            .index_survivor(id as u32, &query, &[])
            .map_err(io::Error::other)?
            != 0
        {
            return Err(invalid("epoch restore unexpectedly retired a live ID"));
        }
    }
    // index_survivor updated orthants for the live subset. Replace those
    // slots by the exact same rule over the full historical insertion order.
    for bucket in &mut store.buckets {
        bucket.orthant = None;
    }
    for (id, image) in store.domains.iter().enumerate() {
        if !image.is_full_orthant() {
            continue;
        }
        let bucket = &mut store.buckets[store.bucket_of[&bucket_key(image)] as usize];
        if bucket
            .orthant
            .is_none_or(|old| rank_contains(image.rank(), store.domains[old as usize].rank()))
        {
            bucket.orthant = Some(id as u32);
        }
    }
    for bucket in &store.buckets {
        let saved = reader.u32()?;
        if saved != bucket.orthant.unwrap_or(u32::MAX) {
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
}
