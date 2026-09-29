use super::*;
use crate::application::routed_campaign::walking::queue::Domain;

fn image(phase: Phase, point: u64) -> CompactDomain<1> {
    CompactDomain::try_from_domain(&Domain {
        phase,
        owner: [true],
        lower: vec![point],
        upper: vec![Some(point)],
        rank: Some(1),
        powers: Default::default(),
    })
    .unwrap()
}

#[test]
fn measured_cost_growth_and_reuse_each_change_priority() {
    let images = [image(Phase::Apply, 0), image(Phase::Route, 1)];
    for (cost, growth, reuse) in [(2.0, 0, 0), (1.0, 2, 0), (1.0, 0, 2)] {
        let mut saved = Saved::default();
        saved.observe(&images[0], cost, growth, 0);
        saved.observe(&images[1], 1.0, 0, reuse);
        saved.candidates = vec![0, 1];
        assert_eq!(saved.choose(&images), Some(1));
    }
    let mut unobserved = Saved::default();
    unobserved.candidates = vec![0, 1];
    assert_eq!(unobserved.choose(&images), Some(0), "deterministic ID tie");
}

#[test]
fn oldest_slot_receives_every_eighth_selection_even_when_expensive() {
    let images: Vec<_> = (0..64)
        .map(|id| image(if id == 0 { Phase::Apply } else { Phase::Route }, id))
        .collect();
    let mut saved = Saved::default();
    saved.observe(&images[0], 1000.0, 1_000_000, 0);
    saved.observe(&images[1], 0.001, 0, 1_000_000);
    saved.candidates = (0..64).collect();
    for expected in 1..=7 {
        assert_eq!(saved.choose(&images), Some(expected));
    }
    assert_eq!(saved.choose(&images), Some(0));
    assert_eq!(saved.next, 0);
    assert!(saved.valid(64));
}

#[test]
fn saved_state_round_trip_preserves_next_choices_and_observations() {
    let images: Vec<_> = (0..20)
        .map(|id| {
            image(
                if id % 2 == 0 {
                    Phase::Apply
                } else {
                    Phase::Route
                },
                id,
            )
        })
        .collect();
    let mut saved = Saved::default();
    saved.observe(&images[0], 3.0, 11, 2);
    saved.observe(&images[1], 0.2, 2, 7);
    saved.candidates = (0..20).collect();
    for _ in 0..5 {
        saved.choose(&images).unwrap();
    }
    let encoded = serde_json::to_vec(&saved).unwrap();
    let mut decoded: Saved = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(saved, decoded);
    assert!(decoded.valid(20));
    while !saved.candidates.is_empty() {
        assert_eq!(saved.choose(&images), decoded.choose(&images));
    }
}

#[test]
fn checkpoint_hint_shape_is_bounded_and_rejects_omissions_or_corruption() {
    let base = serde_json::to_value(Saved::default()).unwrap();
    let mut oversized = base.clone();
    oversized["candidates"] = serde_json::json!((0..65).collect::<Vec<_>>());
    assert!(serde_json::from_value::<Saved>(oversized).is_err());
    for (field, value) in [
        ("next", serde_json::json!(8)),
        ("version", serde_json::json!(0)),
        ("candidates", serde_json::json!([1, 1])),
        ("candidates", serde_json::json!([2, 1])),
        ("candidates", serde_json::json!([10])),
    ] {
        let mut forged = base.clone();
        forged[field] = value;
        let decoded: Saved = serde_json::from_value(forged).unwrap();
        assert!(!decoded.valid(10));
    }
    let mut missing = base;
    missing.as_object_mut().unwrap().remove("buckets");
    assert!(serde_json::from_value::<Saved>(missing).is_err());
}

#[test]
fn observations_reject_nonfinite_and_saturate_without_score_overflow() {
    let mut bucket = Bucket::default();
    for seconds in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        bucket.observe(seconds, u64::MAX, u64::MAX);
        assert_eq!(bucket, Bucket::default());
    }
    bucket.observe(f64::MAX, u64::MAX, u64::MAX);
    assert!(bucket.valid());
    assert_eq!(bucket.cost_us, MAX_COST_US);
    assert_eq!(bucket.growth, MAX_COUNT);
    assert_eq!(bucket.reuse, MAX_COUNT);
    let (num, den) = bucket.ratio();
    assert!(num.checked_mul(den).is_some());
    bucket.observe(0.0, 0, 0);
    assert!(bucket.valid());
    assert_eq!(bucket.samples, 2);
}
