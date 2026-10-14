//! Exact stable membership shared by routed and fresh-root application.
use super::*;
use crate::algebra::CoefficientContext;
use std::hash::Hasher;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Default)]
struct Collisions(Arc<AtomicUsize>);
struct CollisionHasher(Arc<AtomicUsize>);
impl BuildHasher for Collisions {
    type Hasher = CollisionHasher;
    fn build_hasher(&self) -> Self::Hasher {
        CollisionHasher(self.0.clone())
    }
}
impl Hasher for CollisionHasher {
    fn write(&mut self, _: &[u8]) {}
    fn finish(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed);
        0
    }
}

fn legacy(output: &mut Vec<Coefficient>, input: &[Coefficient]) {
    for condition in input {
        if !output.contains(condition) {
            output.push(condition.clone());
        }
    }
}

#[test]
fn ordered_union_matches_legacy_across_overlapping_children_and_vec_growth() {
    let context = CoefficientContext::new(["d"]);
    let guards = (0..257)
        .map(|i| context.coefficient_fixture(&format!("d-{i}")))
        .collect::<Vec<_>>();
    let batches = [
        guards[..129].to_vec(),
        guards[73..].iter().rev().cloned().collect(),
        guards.clone(),
        guards.iter().rev().cloned().collect(),
    ];
    let mut expected = vec![guards[200].clone(), guards[3].clone()];
    let mut actual = expected.clone();
    for batch in &batches {
        legacy(&mut expected, batch);
    }
    {
        let mut union = GuardUnion::try_new(&mut actual).unwrap();
        for batch in &batches {
            union.extend(batch).unwrap();
        }
        assert_eq!(union.index.previous.len(), union.output.len());
        assert!(union.index.heads.len() <= union.output.len());
    }
    assert_eq!(actual, expected);
    assert_eq!(actual[..2], [guards[200].clone(), guards[3].clone()]);
}

#[test]
fn forced_hash_collisions_use_exact_equality_and_build_index_only_once() {
    let context = CoefficientContext::new(["d"]);
    let guards = (0..32)
        .map(|i| context.coefficient_fixture(&format!("d-{i}")))
        .collect::<Vec<_>>();
    let mut output = guards[..3].to_vec();
    let hashes = Collisions::default();
    {
        let mut union = GuardUnion::with_hasher(&mut output, hashes.clone()).unwrap();
        union.extend(&guards).unwrap();
        union.extend(&guards).unwrap();
        assert_eq!(union.index.heads.len(), 1);
        assert_eq!(union.index.previous.len(), 32);
        assert_eq!(hashes.0.load(Ordering::Relaxed), 3 + 32 + 32);
    }
    assert_eq!(output, guards);
}

#[test]
fn exact_zero_poles_signs_and_products_are_never_normalized_or_dropped() {
    let context = CoefficientContext::new(["d"]);
    let source = [
        "d-4",
        "1/(d-4)",
        "(d-4)*(d-3)",
        "4-d",
        "0",
        "1",
        "d-4",
        "1/(d-4)",
        "0",
    ]
    .map(|text| context.coefficient_fixture(text));
    let mut expected = Vec::new();
    legacy(&mut expected, &source);
    let mut output = Vec::new();
    GuardUnion::with_hasher(&mut output, Collisions::default())
        .unwrap()
        .extend(&source)
        .unwrap();
    assert_eq!(output, expected);
    assert_eq!(output.len(), 6);
    assert_eq!(output[4], context.zero());
    assert_eq!(output[1], context.coefficient_fixture("1/(d-4)"));
}

#[test]
fn existing_prefix_duplicates_and_first_seen_order_remain_unchanged() {
    let context = CoefficientContext::new(["d"]);
    let a = context.coefficient_fixture("d-4");
    let b = context.coefficient_fixture("d-3");
    let c = context.coefficient_fixture("d-2");
    let mut output = vec![b.clone(), a.clone(), b.clone()];
    {
        let mut union = GuardUnion::try_new(&mut output).unwrap();
        union
            .extend(&[a.clone(), c.clone(), b.clone(), c.clone()])
            .unwrap();
    }
    assert_eq!(output, [b.clone(), a, b, c]);
}

#[test]
fn payload_admission_runs_only_for_new_guards_and_failure_keeps_membership_reusable() {
    let context = CoefficientContext::new(["d"]);
    let a = context.coefficient_fixture("d-4");
    let b = context.coefficient_fixture("d-3");
    let mut output = Vec::new();
    let mut calls = 0;
    {
        let mut union = GuardUnion::try_new(&mut output).unwrap();
        union
            .extend_with(&[a.clone(), a.clone()], |_| {
                calls += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(calls, 1);
        let before = union.output.clone();
        assert!(
            union
                .extend_with(&[a.clone(), b.clone()], |_| {
                    calls += 1;
                    Err(ReductionError::CacheCoefficientTermLimit {
                        requested: 2,
                        limit: 1,
                    })
                })
                .is_err()
        );
        assert_eq!(*union.output, before);
        assert_eq!(union.index.previous.len(), 1);
        union
            .extend_with(&[b.clone(), a.clone()], |_| {
                calls += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(calls, 3);
    }
    assert_eq!(output, [a, b]);
}
