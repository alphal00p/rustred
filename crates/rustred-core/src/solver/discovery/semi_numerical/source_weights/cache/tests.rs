use symbolica::domains::finite_field::FiniteFieldCore;

use crate::algebra::CoefficientContext;
use crate::solver::{Integral, IntegralOrder, Term};

use super::super::super::super::CoefficientVariableOrder;
use super::super::super::FrameVariables;
use super::*;

fn frame() -> ProbeFrame {
    let context = CoefficientContext::new(["x"]);
    let columns = [
        Integral::symbolic([2, 0]).unwrap(),
        Integral::symbolic([1, 0]).unwrap(),
    ];
    let rows = vec![vec![
        Term {
            integral: columns[0],
            coefficient: context.coefficient_fixture("x"),
        },
        Term {
            integral: columns[1],
            coefficient: context.one(),
        },
    ]];
    let variables =
        FrameVariables::try_new(&rows, CoefficientVariableOrder::Original, &[]).unwrap();
    ProbeFrame::new(
        &rows,
        &columns,
        &IntegralOrder::new([true; 2], [false; 2]),
        &variables,
    )
    .unwrap()
}

fn limits(images: usize, values: usize) -> Limits {
    Limits {
        max_degree: 8,
        max_probes: 2000,
        max_attempts: 3,
        max_primes: 4,
        max_cached_images: images,
        max_cached_values: values,
        max_weight_slots: 100,
    }
}

fn key(field: &Zp64, value: u64) -> CacheKey {
    (field.get_prime(), vec![*field.to_element(value).inner()])
}

fn assert_accounting(cache: &ImageCache<'_>) {
    assert_eq!(cache.images.len(), cache.recency.len());
    assert_eq!(
        cache.scalar_slots,
        cache
            .images
            .values()
            .map(|entry| entry.charged_slots)
            .sum::<usize>()
    );
    assert!(cache.images.len() <= cache.limits.max_cached_images);
    assert!(cache.scalar_slots <= cache.limits.max_cached_values);
    for (key, entry) in &cache.images {
        assert_eq!(cache.recency.get(&entry.last_access), Some(key));
    }
}

#[test]
fn least_recent_hit_is_refreshed_before_capacity_eviction() {
    let frame = frame();
    let field = Zp64::new(101);
    let mut cache = ImageCache::new(&frame, 0, limits(2, 1000));
    for value in [1, 2, 1, 3] {
        assert!(
            cache
                .image(&field, &[field.to_element(value)])
                .unwrap()
                .is_some()
        );
        assert_accounting(&cache);
    }
    assert!(cache.images.contains_key(&key(&field, 1)));
    assert!(!cache.images.contains_key(&key(&field, 2)));
    assert!(cache.images.contains_key(&key(&field, 3)));
    assert_eq!(
        cache.recency.first_key_value().unwrap().1.as_ref(),
        &key(&field, 1)
    );
}

#[test]
fn evicted_point_is_recomputed_with_identical_native_image() {
    let frame = frame();
    let field = Zp64::new(101);
    let mut cache = ImageCache::new(&frame, 0, limits(1, 1000));
    let first = cache
        .image(&field, &[field.to_element(7)])
        .unwrap()
        .unwrap();
    let first_values = (
        first.row.clone(),
        first.weights.clone(),
        first.pivots.clone(),
    );
    cache.freeze(first_values.2.clone());
    assert!(
        cache
            .image(&field, &[field.to_element(11)])
            .unwrap()
            .is_some()
    );
    assert!(!cache.images.contains_key(&key(&field, 7)));
    let again = cache
        .image(&field, &[field.to_element(7)])
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            again.row.clone(),
            again.weights.clone(),
            again.pivots.clone()
        ),
        first_values
    );
    assert_eq!(cache.chronology.as_ref(), Some(&first_values.2));
    assert_accounting(&cache);
}

#[test]
fn retained_value_limit_evicts_independently_of_image_count() {
    let frame = frame();
    let field = Zp64::new(101);
    let mut cache = ImageCache::new(&frame, 0, limits(1000, 12));
    for value in 1..100 {
        assert!(
            cache
                .image(&field, &[field.to_element(value)])
                .unwrap()
                .is_some()
        );
        assert_eq!(cache.images.len(), 1);
        assert_eq!(cache.scalar_slots, 12);
        assert_accounting(&cache);
    }
}

#[test]
fn recency_counter_reset_preserves_mathematical_branch_and_bounds() {
    let frame = frame();
    let field = Zp64::new(101);
    let mut cache = ImageCache::new(&frame, 0, limits(2, 1000));
    let pivots = cache
        .image(&field, &[field.to_element(1)])
        .unwrap()
        .unwrap()
        .pivots
        .clone();
    cache.freeze(pivots.clone());
    cache.next_access = usize::MAX;
    assert!(
        cache
            .image(&field, &[field.to_element(2)])
            .unwrap()
            .is_some()
    );
    assert_eq!(cache.next_access, 1);
    assert_eq!(cache.chronology, Some(pivots));
    assert!(!cache.images.contains_key(&key(&field, 1)));
    assert_accounting(&cache);
}
