//! Initial admission only: no input-row continuation or native execution.
use super::super::super::queue::{CompactSummary, Signature};
use super::super::{Query, admit_initial_with};
use super::*;
use serde_json::json;

#[test]
fn encountered_rank_is_identical_when_rebuilding_epoch_store() {
    let mut finite = domain(APPLY, [0, 0], [None, Some(7)]);
    finite.rank = None;
    let mut unbounded = domain(OTHER, [0, 0], [None, None]);
    unbounded.rank = None;
    let state = state_with(&[finite, unbounded]);
    assert_eq!(state.store.encountered_rank.json()["status"], "unbounded");
    assert_eq!(state.store.encountered_rank.json()["maximum"], 7);
    let mut restored = Store::new();
    for (&image, &summary) in state.store.domains.iter().zip(&state.store.summaries) {
        restored.try_reserve(1).unwrap();
        restored
            .push_restored(image, summary, image.digest().0)
            .unwrap();
    }
    assert_eq!(
        restored.encountered_rank.json(),
        state.store.encountered_rank.json()
    );
}

#[derive(PartialEq, Eq)]
struct Authority {
    images: Vec<CompactDomain<2>>,
    summaries: Vec<CompactSummary<2>>,
    rest: Value,
}

// Reservation capacity and lookup/verify work are deliberately excluded:
// failed preflight may retain capacity and has already performed lookups.
fn authority(state: &EpochState<2>) -> Authority {
    let mut buckets = state
        .store
        .bucket_of
        .iter()
        .map(|(&key, &id)| (key, id))
        .collect::<Vec<_>>();
    buckets.sort_unstable();
    let members = state
        .store
        .buckets
        .iter()
        .map(|bucket| {
            json!({"ids":bucket.index.ids(), "orthant":bucket.orthant,
            "live":bucket.index.storage().live})
        })
        .collect::<Vec<_>>();
    let exact = state
        .store
        .domains
        .iter()
        .map(|image| {
            state
                .store
                .exact
                .get(image.digest().0, image, &state.store.domains)
        })
        .collect::<Vec<_>>();
    Authority {
        images: state.store.domains.clone(),
        summaries: state.store.summaries.clone(),
        rest: json!({
            "exact":exact,"exact_len":state.store.exact.len(),
            "overflow_len":state.store.exact.overflow_len(),
            "buckets":buckets,"members":members,
            "max_rank":state.store.max_finite_rank,"unbounded":state.store.unbounded_rank_domains,
            "ledger":state.ledger.words(),"counts":state.ledger.counts().0,
            "nodes":state.nodes,"live":state.live,"poisoned":state.poisoned,
            "p0":state.p0,"k":state.k,"inflight":state.in_flight.len(),
            "frontiers":state.frontier_counts,"walk":state.counters,
            "edges":state.edges.log(),"edge_digest":state.edges.edge_digest(),
            "record_digest":state.edges.records_digest(),"anchors":state.anchors.len(),
            "closure":state.tracker.json(state.store.len(), state.p0 as usize),
        }),
    }
}

// The former successful admission sequence, using the same real geometry,
// index and verifier. It is a differential oracle, not an alternate solver.
fn old_success(state: &mut EpochState<2>, domain: &Domain<2>) -> u32 {
    let image = image(domain);
    let q = QueryImage::new(image).unwrap();
    let query = Query::new(q.core.clone(), domain.phase);
    if let Some((id, _, _)) = state
        .store
        .lookup(
            &q,
            &query,
            state.store.len(),
            &mut state.lookup,
            &mut state.verify,
        )
        .unwrap()
    {
        return id;
    }
    assert!(state.store.len() < state.id_cap());
    let retire = state
        .store
        .contained_live(&q, &query, &mut state.lookup)
        .unwrap();
    state.reserve_ids(1).unwrap();
    state.store.reserve_exact(&[q.digest]).unwrap();
    let id = state.store.push(image, query.compact, q.digest).unwrap();
    state.admit_id(id);
    let mut expected = 0;
    for &old in &retire {
        let old_q = QueryImage::new(state.store.domains[old as usize]).unwrap();
        verify(
            Container::Stored {
                id,
                domains: &state.store.domains,
                published_len: id as usize + 1,
            },
            &old_q,
            &mut state.verify,
        )
        .unwrap();
        if state.is_live(old) {
            expected += 1;
            state.set_live(old, false);
        }
    }
    assert_eq!(
        state.store.index_survivor(id, &query, &retire).unwrap(),
        expected
    );
    id
}

fn same_signature_point(x: u64) -> Domain<2> {
    domain([true, true], [x, 100 - x], [Some(x), Some(100 - x)])
}

#[test]
fn prepared_initial_success_matches_old_ids_counters_and_bucket_order() {
    let mut route = boxed([2, 0], [2, 0]);
    route.phase = Phase::Route;
    let mut orthant = domain(APPLY, [0, 0], [None, None]);
    let mut inputs = vec![
        boxed([2, 0], [2, 0]),
        boxed([4, 0], [4, 0]),
        domain(OTHER, [0, 3], [Some(0), Some(3)]),
        route,
        boxed([1, 0], [5, 0]), // Retires both old Apply IDs, but not other buckets.
        boxed([3, 0], [3, 0]), // Live containment hit.
        boxed([2, 0], [2, 0]), // Exact hit still resolves the retired historical ID.
        orthant.clone(),
    ];
    orthant.rank = Some(200);
    inputs.push(orthant.clone());
    orthant.rank = Some(50);
    inputs.push(orthant.clone()); // Dominant-orthant hit, not an exact duplicate.
    orthant.rank = None;
    inputs.push(orthant);
    // Fill a real same-signature block, then allocate the next one.
    inputs.extend((1..=33).map(same_signature_point));
    let mut prepared = EpochState::new(10_000, usize::MAX, usize::MAX);
    let mut old = EpochState::new(10_000, usize::MAX, usize::MAX);
    for (row, input) in inputs.iter().enumerate() {
        assert_eq!(
            admit_initial(&mut prepared, input).unwrap(),
            old_success(&mut old, input),
            "row {row}"
        );
        assert!(
            authority(&prepared) == authority(&old),
            "authority differs at row {row}"
        );
        assert_eq!(
            prepared.verify, old.verify,
            "verification counters at row {row}"
        );
        assert_eq!(
            serde_json::to_value(prepared.lookup).unwrap(),
            serde_json::to_value(old.lookup).unwrap(),
            "lookup counters at row {row}"
        );
        assert_eq!(
            prepared.store.storage_json(),
            old.store.storage_json(),
            "successful storage accounting at row {row}"
        );
    }
    assert!(prepared.lookup.exact_hits > 0);
    assert!(prepared.lookup.contained_hits > 0);
    assert!(prepared.lookup.orthant_hits > 0);
    assert!(!prepared.is_live(0));
    assert_eq!(prepared.tag(0), Tag::Pending); // Initial IDs never transfer/alias.
}

#[test]
fn every_initial_reservation_refusal_keeps_authority_and_allows_exact_retry() {
    let first = QueryImage::new(image(&same_signature_point(1))).unwrap();
    let last = QueryImage::new(image(&same_signature_point(33))).unwrap();
    assert_eq!(Signature::of(&first.core), Signature::of(&last.core));
    let narrow = domain([true, true], [65_000, 65_000], [Some(65_000), Some(65_000)]);
    let narrow_q = QueryImage::new(image(&narrow)).unwrap();
    let extrema = narrow_q.core.extrema().unwrap();
    let mut wide = domain([true, true], [64_000, 64_000], [None, None]);
    // Derive physical-power bounds from the native summary (active powers
    // include the +1 offset), not from raw coordinate sums.
    wide.powers.max_positive_power = Some(u64::try_from(extrema.positive_power().0).unwrap());
    wide.powers.min_power_difference =
        Some(i64::try_from(extrema.power_difference().0.unwrap()).unwrap());
    let wide_q = QueryImage::new(image(&wide)).unwrap();
    assert_eq!(Signature::of(&narrow_q.core), Signature::of(&wide_q.core));
    assert_eq!(
        wide_q.core.extrema().unwrap().upper(),
        &[Some(66_000), Some(66_000)]
    );
    let cases = [
        ("first bucket", vec![], boxed([2, 0], [2, 0])),
        (
            "new bucket",
            vec![boxed([2, 0], [2, 0])],
            domain(OTHER, [0, 3], [Some(0), Some(3)]),
        ),
        (
            "existing new group",
            vec![boxed([2, 0], [2, 0])],
            boxed([4, 0], [4, 0]),
        ),
        (
            "retirements",
            vec![boxed([2, 0], [2, 0]), boxed([4, 0], [4, 0])],
            boxed([1, 0], [5, 0]),
        ),
        (
            "existing tail",
            vec![same_signature_point(1)],
            same_signature_point(2),
        ),
        (
            "existing full block",
            (1..=32).map(same_signature_point).collect(),
            same_signature_point(33),
        ),
        ("existing tail envelope promotion", vec![narrow], wide),
    ];
    for (name, prefix, candidate) in cases {
        let mut count = 0;
        admit_initial_with(&mut state_with(&prefix), &candidate, || {
            count += 1;
            Ok(())
        })
        .unwrap();
        assert!(
            count >= 3,
            "{name}: includes capacity and final-precommit boundaries"
        );
        let mut expected = state_with(&prefix);
        let expected_id = old_success(&mut expected, &candidate);
        for fail_at in 0..count {
            let mut state = state_with(&prefix);
            let before = authority(&state);
            let mut step = 0;
            let error = admit_initial_with(&mut state, &candidate, || {
                let fail = step == fail_at;
                step += 1;
                if fail {
                    Err("injected initial admission allocation")
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert!(
                matches!(&error, AdmissionError::Alloc(_)),
                "{name}: {error:?}"
            );
            assert!(matches!(error.stop().1, Ok(StopReason::RamGuard)));
            assert!(
                authority(&state) == before,
                "{name}: authority changed at reservation {fail_at}"
            );
            let candidate_image = image(&candidate);
            assert_eq!(
                state.store.exact.get(
                    candidate_image.digest().0,
                    &candidate_image,
                    &state.store.domains
                ),
                None
            );
            assert_eq!(admit_initial(&mut state, &candidate).unwrap(), expected_id);
            assert!(
                authority(&state) == authority(&expected),
                "{name}: retry differs at reservation {fail_at}"
            );
        }
    }
}

#[test]
fn planned_retirement_refusal_precedes_initial_authority_mutation() {
    let mut state = state_with(&[boxed([2, 0], [2, 0])]);
    // Keep the real summary/index at point 2 but corrupt its raw authority
    // to point 9: reverse lookup still proposes ID 0, Planned verification
    // must reject it. No hook replaces the actual verifier.
    state.store.unique_mut().unwrap().domains[0] = image(&boxed([9, 0], [9, 0]));
    let before = authority(&state);
    let error = admit_initial(&mut state, &boxed([1, 0], [5, 0])).unwrap_err();
    assert!(
        matches!(&error, AdmissionError::Internal(message) if message.contains("planned image (verify)"))
    );
    assert!(error.stop().1.is_err());
    assert!(authority(&state) == before);
    assert!(!state.poisoned); // No authority commit was entered.
}

#[test]
fn initial_admission_refuses_existing_poison_even_for_an_exact_hit() {
    let original = boxed([2, 0], [2, 0]);
    let mut state = state_with(&[original.clone()]);
    state.poisoned = true;
    let before = authority(&state);
    for input in [original, boxed([1, 0], [5, 0])] {
        let error = admit_initial_with(&mut state, &input, || {
            panic!("poison must refuse before preparation")
        })
        .unwrap_err();
        assert!(matches!(error, AdmissionError::Internal(_)));
        assert!(authority(&state) == before);
    }
}

#[test]
fn initial_postcommit_mismatch_remains_poisoned_and_is_never_a_ram_stop() {
    let mut state = state_with(&[boxed([2, 0], [2, 0])]);
    // Deliberately violate index/live agreement to exercise the impossible
    // postcommit invariant, not to model a resumable allocation failure.
    state.set_live(0, false);
    let candidate = boxed([1, 0], [5, 0]);
    let error = admit_initial(&mut state, &candidate).unwrap_err();
    assert!(
        matches!(&error, AdmissionError::Internal(message) if message.contains("retirement mismatch"))
    );
    assert!(error.stop().1.is_err());
    assert!(state.poisoned);
    assert_eq!(state.store.len(), 2);
    let refused = authority(&state);
    assert!(matches!(
        admit_initial(&mut state, &candidate),
        Err(AdmissionError::Internal(_))
    ));
    assert!(authority(&state) == refused);
}
