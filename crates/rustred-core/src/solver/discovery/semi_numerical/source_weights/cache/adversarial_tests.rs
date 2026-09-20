//! Independent eviction tests: memoization changes work, never oracle values.

use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};

use crate::algebra::CoefficientContext;
use crate::solver::{ExactRow, Integral, IntegralOrder, Term};

use super::super::super::super::{
    CoefficientVariableOrder, MaterializationEvent, SymbolicExactBackend,
    exact_materialize_using_with_observer,
};
use super::super::{FrameVariables, Limits, ProbeFrame, probe::WeightImage};
use super::{CacheKey, Fp, ImageCache, Slot};

fn limits(images: usize, values: usize) -> Limits {
    Limits {
        max_degree: 8,
        max_probes: 2000,
        max_attempts: 3,
        max_primes: 4,
        max_cached_images: images,
        max_cached_values: values,
        max_weight_slots: 1000,
    }
}

fn row(context: &CoefficientContext, entries: &[(i16, &str)]) -> ExactRow<3> {
    entries
        .iter()
        .map(|(shift, value)| Term {
            integral: Integral::symbolic([*shift, 0, 0]).unwrap(),
            coefficient: context.coefficient_fixture(value),
        })
        .collect()
}

fn frame(rows: &[ExactRow<3>], shifts: &[i16]) -> ProbeFrame {
    let variables = FrameVariables::try_new(rows, CoefficientVariableOrder::Original, &[]).unwrap();
    let columns = shifts
        .iter()
        .map(|shift| Integral::symbolic([*shift, 0, 0]).unwrap())
        .collect::<Vec<_>>();
    ProbeFrame::new(
        rows,
        &columns,
        &IntegralOrder::new([true; 3], [false; 3]),
        &variables,
    )
    .unwrap()
}

type Snapshot = (
    Vec<(u32, Fp)>,
    Vec<Option<u32>>,
    Vec<Fp>,
    Vec<usize>,
    Vec<usize>,
    usize,
    usize,
);

fn snapshot(image: &WeightImage) -> Snapshot {
    (
        image.row.clone(),
        image.pivots.clone(),
        image.weights.clone(),
        image.accepted_sources.clone(),
        image.accepted_l_rows.clone(),
        image.harder_rank,
        image.lower_nonzeros,
    )
}

fn key(field: &Zp64, point: u64) -> CacheKey {
    (field.get_prime(), vec![*field.to_element(point).inner()])
}

fn assert_invariants(cache: &ImageCache<'_>) {
    assert_eq!(cache.images.len(), cache.recency.len());
    assert!(cache.images.len() <= cache.limits.max_cached_images);
    assert!(cache.scalar_slots <= cache.limits.max_cached_values);
    assert_eq!(
        cache.scalar_slots,
        cache
            .images
            .values()
            .map(|entry| entry.charged_slots)
            .sum::<usize>()
    );
    for (age, shared_key) in &cache.recency {
        let (stored_key, entry) = cache.images.get_key_value(shared_key).unwrap();
        assert_eq!(*age, entry.last_access);
        assert!(std::sync::Arc::ptr_eq(stored_key, shared_key));
    }
}

#[test]
fn eviction_recomputation_matches_the_complete_large_cache_oracle() {
    let context = CoefficientContext::new(["x"]);
    let frame = frame(&[row(&context, &[(2, "1/x"), (1, "1")])], &[2, 1]);
    let field = Zp64::new(101);
    let mut tiny = ImageCache::new(&frame, 0, limits(1, 1000));
    let mut large = ImageCache::new(&frame, 0, limits(128, 10000));
    tiny.freeze(vec![Some(0)]);
    large.freeze(vec![Some(0)]);
    for point in [1, 2, 1, 0, 3, 1, 4, 0, 1] {
        let probe = [field.to_element(point)];
        let actual = tiny.image(&field, &probe).unwrap().map(snapshot);
        let expected = large.image(&field, &probe).unwrap().map(snapshot);
        assert_eq!(actual, expected);
        assert_invariants(&tiny);
        assert_invariants(&large);
        assert_eq!(tiny.chronology, Some(vec![Some(0)]));
        tiny.check_failure().unwrap();
    }
    assert!(large.images.len() > tiny.images.len());
}

#[test]
fn eviction_follows_access_order_not_hash_iteration_order() {
    let context = CoefficientContext::new(["x"]);
    let frame = frame(&[row(&context, &[(2, "x"), (1, "1")])], &[2, 1]);
    let field = Zp64::new(101);
    let mut first = ImageCache::new(&frame, 0, limits(2, 1000));
    let mut second = ImageCache::new(&frame, 0, limits(2, 1000));
    for point in [1, 2, 1, 3] {
        let probe = [field.to_element(point)];
        assert_eq!(
            first.image(&field, &probe).unwrap().map(snapshot),
            second.image(&field, &probe).unwrap().map(snapshot)
        );
        assert_eq!(first.recency, second.recency);
        assert_invariants(&first);
        assert_invariants(&second);
    }
    assert!(first.images.contains_key(&key(&field, 1)));
    assert!(first.images.contains_key(&key(&field, 3)));
    assert!(!first.images.contains_key(&key(&field, 2)));
}

#[test]
fn retained_value_capacity_evicts_even_when_image_count_has_room() {
    let context = CoefficientContext::new(["x"]);
    let frame = frame(&[row(&context, &[(2, "1/x"), (1, "1")])], &[2, 1]);
    let field = Zp64::new(101);
    let mut measure = ImageCache::new(&frame, 0, limits(10, 1000));
    measure.image(&field, &[field.to_element(1)]).unwrap();
    let one_image_capacity = measure.scalar_slots;
    let mut cache = ImageCache::new(&frame, 0, limits(10, one_image_capacity));
    for point in [1, 2, 0, 3, 0, 1] {
        let expected = measure
            .image(&field, &[field.to_element(point)])
            .unwrap()
            .map(snapshot);
        assert_eq!(
            cache
                .image(&field, &[field.to_element(point)])
                .unwrap()
                .map(snapshot),
            expected
        );
        assert_invariants(&cache);
        assert_eq!(cache.images.len(), 1);
    }
}

#[test]
fn frozen_chronology_survives_eviction_of_a_pre_freeze_exceptional_image() {
    let context = CoefficientContext::new(["a"]);
    let frame = frame(
        &[
            row(&context, &[(3, "a"), (2, "a")]),
            row(&context, &[(3, "1"), (2, "2"), (1, "1")]),
            row(&context, &[(2, "1"), (1, "2")]),
        ],
        &[3, 2, 1],
    );
    let field = Zp64::new(101);
    let mut cache = ImageCache::new(&frame, 1, limits(2, 1000));
    assert_eq!(
        cache
            .image(&field, &[field.to_element(0)])
            .unwrap()
            .unwrap()
            .pivots,
        [None, Some(0), Some(1)]
    );
    let chronology = cache
        .image(&field, &[field.to_element(2)])
        .unwrap()
        .unwrap()
        .pivots
        .clone();
    assert_eq!(chronology, [Some(0), Some(1)]);
    cache.freeze(chronology.clone());
    assert!(
        cache
            .image(&field, &[field.to_element(0)])
            .unwrap()
            .is_none()
    );
    cache.image(&field, &[field.to_element(3)]).unwrap();
    cache.image(&field, &[field.to_element(4)]).unwrap();
    assert!(!cache.images.contains_key(&key(&field, 0)));
    // Recomputing now uses the frozen shorter prefix. The absence remains an
    // unusable sample; neither cache history may authorize the other branch.
    assert!(
        cache
            .image(&field, &[field.to_element(0)])
            .unwrap()
            .is_none()
    );
    assert_eq!(cache.chronology, Some(chronology));
    assert_invariants(&cache);
    cache.check_failure().unwrap();
}

#[test]
fn oversized_single_image_preserves_cached_state_and_poison_is_not_a_zero() {
    let context = CoefficientContext::new(["x"]);
    let frame = frame(&[row(&context, &[(2, "1"), (1, "x")])], &[2, 1]);
    let field = Zp64::new(101);
    let mut measure = ImageCache::new(&frame, 0, limits(10, 1000));
    let zero_tail = measure
        .image(&field, &[field.to_element(0)])
        .unwrap()
        .map(snapshot);
    let smaller_capacity = measure.scalar_slots;
    let mut cache = ImageCache::new(&frame, 0, limits(10, smaller_capacity));
    assert_eq!(
        cache
            .image(&field, &[field.to_element(0)])
            .unwrap()
            .map(snapshot),
        zero_tail
    );
    let before_slots = cache.scalar_slots;
    assert!(
        cache
            .coefficient(&field, &[field.to_element(1)], Slot::Target(0))
            .is_none()
    );
    assert!(cache.failure.is_some());
    assert_eq!(cache.scalar_slots, before_slots);
    assert_eq!(cache.images.len(), 1);
    assert!(cache.images.contains_key(&key(&field, 0)));
    let before_access = cache.next_access;
    assert!(
        cache
            .coefficient(&field, &[field.to_element(0)], Slot::Target(0))
            .is_none()
    );
    assert_eq!(cache.next_access, before_access);
    assert!(cache.check_failure().is_err());
    assert_invariants(&cache);
}

#[test]
fn access_counter_reset_drops_only_memoization_not_the_frozen_branch() {
    let context = CoefficientContext::new(["x"]);
    let frame = frame(&[row(&context, &[(2, "x"), (1, "1")])], &[2, 1]);
    let field = Zp64::new(101);
    let mut cache = ImageCache::new(&frame, 0, limits(2, 1000));
    cache.freeze(vec![Some(0)]);
    let first = cache
        .image(&field, &[field.to_element(1)])
        .unwrap()
        .map(snapshot);
    cache.next_access = usize::MAX;
    assert_eq!(
        cache
            .image(&field, &[field.to_element(1)])
            .unwrap()
            .map(snapshot),
        first
    );
    assert_eq!(cache.next_access, 1);
    assert_eq!(cache.recency.keys().copied().collect::<Vec<_>>(), [0]);
    assert_eq!(cache.chronology, Some(vec![Some(0)]));
    assert_invariants(&cache);
}

#[test]
fn one_image_cache_preserves_complete_exact_materializer_output() {
    let context = CoefficientContext::new(["unused", "a", "b"]);
    let rows = vec![
        row(&context, &[(3, "a+1"), (2, "1"), (1, "2")]),
        row(&context, &[(4, "b+2"), (2, "3"), (1, "4")]),
        row(&context, &[(4, "b+2"), (3, "a+1"), (2, "a+5"), (1, "b+9")]),
    ];
    let run = |backend| {
        let mut events = Vec::new();
        let result = exact_materialize_using_with_observer(
            &rows,
            &IntegralOrder::new([true; 3], [false; 3]),
            Integral::symbolic([2, 0, 0]).unwrap(),
            backend,
            CoefficientVariableOrder::Reverse,
            &[],
            |event| events.push(event),
        )
        .unwrap();
        (result, events)
    };
    let expected = run(SymbolicExactBackend::Sparse).0;
    assert_eq!(expected, row(&context, &[(2, "1"), (1, "(b+3)/(a+1)")]));
    for max_cached_images in [1, 1024] {
        let (actual, events) = run(SymbolicExactBackend::SemiNumericalSourceWeights {
            max_degree: 8,
            max_probes: 2000,
            max_attempts: 3,
            max_primes: 4,
            max_cached_images,
            max_cached_values: 10000,
            max_weight_slots: 1000,
        });
        assert_eq!(actual, expected);
        for term in actual {
            assert_eq!(term.coefficient.numerator.variables(), context.variables());
            assert_eq!(
                term.coefficient.denominator.variables(),
                context.variables()
            );
        }
        assert!(!events.iter().any(|event| matches!(
            event,
            MaterializationEvent::SemiNumericalExactReplayStarted { .. }
                | MaterializationEvent::SemiNumericalExactReplayFinished { .. }
                | MaterializationEvent::RowStarted { .. }
                | MaterializationEvent::RowFinished { .. }
        )));
    }
}
