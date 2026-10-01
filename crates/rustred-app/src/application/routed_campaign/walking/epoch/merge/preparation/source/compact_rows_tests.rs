use super::*;
use crate::application::routed_campaign::walking::queue::Phase;
use rustred::solver::DomainPowerBounds;

fn miss<const N: usize>(ordinal: usize, supplied: bool) -> Miss<N> {
    let image = CompactDomain::try_from_domain(&Domain {
        phase: Phase::Apply,
        owner: [true; N],
        lower: vec![ordinal as u64; N],
        upper: vec![Some(ordinal as u64); N],
        rank: None,
        powers: DomainPowerBounds::default(),
    })
    .unwrap();
    Miss {
        ordinal: ordinal as u32,
        digest: image.digest().0,
        image,
        target: supplied.then_some(0),
    }
}

fn resolution<const N: usize>(miss: &Miss<N>, stored: bool) -> MissResolution<N> {
    let q = QueryImage::new(miss.image).unwrap();
    if stored {
        let token = verify(
            Container::Stored {
                id: 0,
                domains: &[miss.image],
                published_len: 1,
            },
            &q,
            &mut VerifyCounters::default(),
        )
        .unwrap();
        MissResolution::Stored(token)
    } else {
        let query = Query::new(q.core.clone(), miss.image.phase());
        MissResolution::Candidate { q, query }
    }
}

fn roundtrip<const N: usize>(supplied: &[bool], resolved_stored: &[bool]) {
    assert_eq!(supplied.len(), resolved_stored.len());
    let misses: Vec<Miss<N>> = supplied
        .iter()
        .enumerate()
        .map(|(i, &supplied)| miss(i, supplied))
        .collect();
    let mut rows = ResolvedRows::new(&misses).unwrap();
    let candidate_bound = supplied.iter().filter(|&&stored| !stored).count();
    assert!(rows.tags.capacity() >= misses.len());
    assert!(rows.candidates.capacity() >= candidate_bound);
    if candidate_bound == 0 {
        assert_eq!(rows.candidates.capacity(), 0);
    }
    if misses.is_empty() {
        assert_eq!(rows.tags.capacity(), 0);
    }
    let capacities = (rows.tags.capacity(), rows.candidates.capacity());
    let pointers = (rows.tags.as_ptr(), rows.candidates.as_ptr());
    for (miss, &stored) in misses.iter().zip(resolved_stored) {
        assert!(miss.target.is_none() || stored);
        rows.push(resolution(miss, stored));
        assert_eq!(
            (rows.tags.capacity(), rows.candidates.capacity()),
            capacities
        );
        assert_eq!((rows.tags.as_ptr(), rows.candidates.as_ptr()), pointers);
    }
    assert_eq!(rows.tags.len(), misses.len());
    assert_eq!(
        rows.candidates.len(),
        resolved_stored.iter().filter(|&&stored| !stored).count()
    );
    let mut decoded = rows.into_iter();
    assert_eq!(decoded.len(), misses.len());
    for (position, (miss, &stored)) in misses.iter().zip(resolved_stored).enumerate() {
        let actual = decoded.next().expect("one resolution per input row");
        match (actual, resolution(miss, stored)) {
            (MissResolution::Stored(actual), MissResolution::Stored(expected)) => {
                assert_eq!(actual, expected);
            }
            (
                MissResolution::Candidate { q, query },
                MissResolution::Candidate {
                    q: expected_q,
                    query: expected_query,
                },
            ) => {
                assert_eq!(q.image, expected_q.image);
                assert_eq!(q.digest, expected_q.digest);
                assert_eq!(q.core, expected_q.core);
                assert_eq!(query.core, expected_query.core);
                assert_eq!(query.image(), expected_query.image());
                assert_eq!(query.bucket, expected_query.bucket);
            }
            _ => panic!("changed source-row classification at {position}"),
        }
        assert_eq!(decoded.len(), misses.len() - position - 1);
    }
    assert!(decoded.next().is_none());
}

#[test]
fn empty_stored_candidate_mixed_and_targetless_stored_roundtrip() {
    fn cases<const N: usize>() {
        roundtrip::<N>(&[], &[]);
        roundtrip::<N>(&[true], &[true]);
        roundtrip::<N>(&[false], &[false]);
        roundtrip::<N>(
            &[true, false, false, true, false, false],
            &[true, false, true, true, false, false],
        );
        // None is an allocation upper bound, not the predicted result.
        roundtrip::<N>(&[false; 4], &[true; 4]);
    }
    cases::<2>();
    cases::<10>();
    cases::<15>();
    cases::<32>();
}

#[test]
fn full_block_stored_and_candidate_worst_case_do_not_reallocate() {
    roundtrip::<15>(&[true; MISS_BLOCK], &[true; MISS_BLOCK]);
    roundtrip::<15>(&[false; MISS_BLOCK], &[false; MISS_BLOCK]);
}

#[test]
fn dropping_a_partial_iterator_leaves_no_borrow_or_reused_payload() {
    let misses = [miss::<2>(0, false), miss(1, true), miss(2, false)];
    for consumed in 0..=misses.len() {
        let mut rows = ResolvedRows::new(&misses).unwrap();
        for miss in &misses {
            rows.push(resolution(miss, miss.target.is_some()));
        }
        let mut decoded = rows.into_iter();
        for _ in 0..consumed {
            assert!(decoded.next().is_some());
        }
        drop(decoded);
        // Input and independently prepared future blocks remain untouched.
        assert_eq!(misses[2].ordinal, 2);
    }
}

#[test]
#[should_panic(expected = "source candidate payload")]
fn missing_private_payload_never_silently_truncates() {
    let rows = ResolvedRows::<2> {
        tags: vec![None],
        candidates: Vec::new(),
    };
    let _ = rows.into_iter().next();
}

#[test]
fn skipping_and_last_keep_candidate_payloads_paired_with_their_tags() {
    let supplied = [false, true, false, false, true, false];
    let misses: Vec<Miss<2>> = supplied
        .into_iter()
        .enumerate()
        .map(|(ordinal, supplied)| miss(ordinal, supplied))
        .collect();
    let prepared = || {
        let mut rows = ResolvedRows::new(&misses).unwrap();
        for miss in &misses {
            rows.push(resolution(miss, miss.target.is_some()));
        }
        rows.into_iter()
    };
    let digest = |resolution: MissResolution<2>| match resolution {
        MissResolution::Stored(token) => token.into_id(0).unwrap().q_digest(),
        MissResolution::Candidate { q, .. } => q.digest,
    };
    for (position, miss) in misses.iter().enumerate() {
        assert_eq!(digest(prepared().nth(position).unwrap()), miss.digest);
        assert_eq!(
            digest(prepared().skip(position).next().unwrap()),
            miss.digest
        );
    }
    assert_eq!(
        digest(prepared().last().unwrap()),
        misses.last().unwrap().digest
    );
    assert!(prepared().nth(misses.len()).is_none());
    let mut partial = prepared();
    assert_eq!(digest(partial.next().unwrap()), misses[0].digest);
    assert_eq!(digest(partial.nth(1).unwrap()), misses[2].digest);
    assert_eq!(
        digest(partial.last().unwrap()),
        misses.last().unwrap().digest
    );
}

#[test]
fn compact_transport_layout_diagnostic() {
    fn report<const N: usize>() {
        for (name, supplied) in [
            ("stored", vec![true; MISS_BLOCK]),
            ("candidate", vec![false; MISS_BLOCK]),
        ] {
            let misses: Vec<Miss<N>> = supplied
                .into_iter()
                .enumerate()
                .map(|(i, supplied)| miss(i, supplied))
                .collect();
            let rows = ResolvedRows::new(&misses).unwrap();
            let tag_bytes = rows.tags.capacity() * std::mem::size_of::<Option<Verified>>();
            let payload_bytes =
                rows.candidates.capacity() * std::mem::size_of::<(QueryImage<N>, Query<N>)>();
            println!(
                "compact_source_rows {}",
                serde_json::json!({
                    "arity": N,
                    "input_class": name,
                    "scope": "reserved backing capacity only; excludes Vec headers, allocator overhead, touched bytes, RSS and speed",
                    "row_count": MISS_BLOCK,
                    "old_enum_stride": std::mem::size_of::<MissResolution<N>>(),
                    "tag_stride": std::mem::size_of::<Option<Verified>>(),
                    "payload_stride": std::mem::size_of::<(QueryImage<N>, Query<N>)>(),
                    "tag_bytes": tag_bytes,
                    "payload_bytes": payload_bytes,
                    "total_bytes": tag_bytes + payload_bytes,
                })
            );
        }
    }
    report::<2>();
    report::<10>();
    report::<15>();
    report::<32>();
}
