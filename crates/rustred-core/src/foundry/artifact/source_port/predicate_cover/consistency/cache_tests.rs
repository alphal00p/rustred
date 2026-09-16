use super::*;
use crate::algebra::CoefficientContext;
use crate::foundry::artifact::source_port::predicate_cover::{
    Clause, PredicateCoverError, PredicateCoverLimits, TraversalWork, check_valuations,
};

fn disable_cache(cache: &mut RestrictionCache<'_>) {
    cache.limits.boxes = 0;
}

#[test]
fn explicit_work_allowance_is_shared_across_boxes_and_zero_never_calls_native() {
    let context = CoefficientContext::new(["d", "a", "b"]);
    let equation = context.coefficient_fixture("a-b").numerator;
    let atoms = [Atom {
        indices: &[1, 2],
        equation: &equation,
    }];
    let first = LatticeBox::try_new([0, 0], [Some(0); 2]).unwrap();
    let second = LatticeBox::try_new([1, 1], [Some(1); 2]).unwrap();
    let cost = polynomial_work(&atoms[0], 2).unwrap();
    let mut cache = RestrictionCache::with_work_limit(&[false; 2], &atoms, 2 * cost - 1);
    assert_eq!(cache.contradicts(&first, &[Some(true)]), Ok(false));
    assert_eq!(cache.budget.remaining, cost - 1);
    assert_eq!(
        cache.contradicts(&second, &[Some(true)]),
        Err(WorkExhausted)
    );
    assert_eq!(cache.statistics.classification_materializations, 1);
    // An earlier proved cache entry remains reusable after exhaustion.
    assert_eq!(cache.contradicts(&first, &[Some(false)]), Ok(true));
    assert_eq!(cache.statistics.native_work, cost);

    let mut zero = RestrictionCache::with_work_limit(&[false; 2], &atoms, 0);
    assert_eq!(zero.contradicts(&first, &[None]), Ok(false));
    assert_eq!(zero.contradicts(&first, &[Some(true)]), Err(WorkExhausted));
    assert_eq!(zero.statistics.classification_materializations, 0);
    assert_eq!(zero.statistics.native_work, 0);
}

#[test]
fn zero_and_nonzero_restrictions_are_reused_across_opposite_assignments() {
    let context = CoefficientContext::new(["d", "a", "b"]);
    let equation = context.coefficient_fixture("a-b").numerator;
    let atoms = [Atom {
        indices: &[1, 2],
        equation: &equation,
    }];
    let cell = LatticeBox::try_new([0, 0], [Some(0); 2]).unwrap();
    let mut cache = RestrictionCache::new(&[false; 2], &atoms);
    assert_eq!(cache.contradicts(&cell, &[Some(true)]), Ok(false));
    let charged = cache.statistics.native_work;
    assert!(charged > 0);
    cache.budget.remaining = 0;
    assert_eq!(cache.contradicts(&cell, &[Some(false)]), Ok(true));
    assert_eq!(cache.contradicts(&cell, &[Some(true)]), Ok(false));
    assert_eq!(cache.statistics.hits, 2);
    assert_eq!(cache.statistics.misses, 1);
    assert_eq!(cache.statistics.native_work, charged);
    assert_eq!(cache.entries[&cell][0], Some(RestrictionTruth::Zero));

    let mut cache = RestrictionCache::new(&[true, false], &atoms);
    assert_eq!(cache.contradicts(&cell, &[Some(false)]), Ok(false));
    let charged = cache.statistics.native_work;
    cache.budget.remaining = 0;
    assert_eq!(cache.contradicts(&cell, &[Some(true)]), Ok(true));
    assert_eq!(cache.statistics.hits, 1);
    assert_eq!(cache.statistics.native_work, charged);
    assert_eq!(
        cache.entries[&cell][0],
        Some(RestrictionTruth::NonzeroConstant)
    );
}

#[test]
fn unknown_is_cached_but_exact_finite_and_infinite_box_keys_do_not_alias() {
    let context = CoefficientContext::new(["d", "a", "b"]);
    let equation = context.coefficient_fixture("a-b").numerator;
    let atoms = [Atom {
        indices: &[1, 2],
        equation: &equation,
    }];
    let mut cache = RestrictionCache::new(&[false; 2], &atoms);
    for upper in [None, Some(1)] {
        let variable = LatticeBox::try_new([0, 0], [Some(0), upper]).unwrap();
        assert_eq!(cache.contradicts(&variable, &[Some(false)]), Ok(false));
        assert_eq!(cache.contradicts(&variable, &[Some(true)]), Ok(false));
        assert_eq!(cache.entries[&variable][0], Some(RestrictionTruth::Unknown));
    }
    let zero = LatticeBox::try_new([0, 0], [Some(0); 2]).unwrap();
    assert_eq!(cache.contradicts(&zero, &[Some(false)]), Ok(true));
    let nonzero = LatticeBox::try_new([0, 1], [Some(0), Some(1)]).unwrap();
    assert_eq!(cache.contradicts(&nonzero, &[Some(true)]), Ok(true));
    assert_eq!(cache.entries.len(), 4);
    assert_eq!(cache.statistics.misses, 4);
    assert_eq!(cache.statistics.hits, 2);
}

#[test]
fn sector_maps_and_atom_order_are_bound_to_separate_contexts() {
    let context = CoefficientContext::new(["d", "a", "b"]);
    let zero = context.coefficient_fixture("a-2*b").numerator;
    let nonzero = context.coefficient_fixture("a-b").numerator;
    let cell = LatticeBox::try_new([2, 1], [Some(2), Some(1)]).unwrap();
    let atoms = [
        Atom {
            indices: &[1, 2],
            equation: &zero,
        },
        Atom {
            indices: &[1, 2],
            equation: &nonzero,
        },
    ];
    let mut original = RestrictionCache::new(&[false; 2], &atoms);
    assert_eq!(original.contradicts(&cell, &[Some(false), None]), Ok(true));
    assert_eq!(original.contradicts(&cell, &[None, Some(false)]), Ok(false));
    assert_eq!(original.statistics.misses, 2);

    let mut opposite_sector = RestrictionCache::new(&[true, false], &atoms);
    assert_eq!(
        opposite_sector.contradicts(&cell, &[Some(false), None]),
        Ok(false)
    );
    assert_eq!(opposite_sector.statistics.misses, 1);
    let reordered = [
        Atom {
            indices: &[1, 2],
            equation: &nonzero,
        },
        Atom {
            indices: &[1, 2],
            equation: &zero,
        },
    ];
    let mut reordered = RestrictionCache::new(&[false; 2], &reordered);
    assert_eq!(
        reordered.contradicts(&cell, &[Some(false), None]),
        Ok(false)
    );
    assert_eq!(reordered.contradicts(&cell, &[None, Some(false)]), Ok(true));
    let remapped = [Atom {
        indices: &[2, 1],
        equation: &zero,
    }];
    let mut remapped = RestrictionCache::new(&[false; 2], &remapped);
    assert_eq!(remapped.contradicts(&cell, &[Some(false)]), Ok(false));
    assert_eq!(remapped.statistics.misses, 1);
    assert_eq!(original.contradicts(&cell, &[Some(false), None]), Ok(true));
    assert_eq!(original.statistics.hits, 1);
}

#[test]
fn each_memory_cap_uses_deterministic_uncached_fallback_without_eviction() {
    let context = CoefficientContext::new(["d", "a", "b"]);
    let equation = context.coefficient_fixture("a-b").numerator;
    let atoms = [Atom {
        indices: &[1, 2],
        equation: &equation,
    }];
    let first = LatticeBox::try_new([0; 2], [Some(0); 2]).unwrap();
    let second = LatticeBox::try_new([1; 2], [Some(1); 2]).unwrap();
    let limits = CacheLimits::default();
    for limits in [
        CacheLimits { boxes: 1, ..limits },
        CacheLimits {
            atom_slots: 1,
            ..limits
        },
        CacheLimits {
            coordinate_cells: 4,
            ..limits
        },
    ] {
        let mut cached = RestrictionCache::new(&[false; 2], &atoms);
        cached.limits = limits;
        let mut direct = RestrictionCache::new(&[false; 2], &atoms);
        disable_cache(&mut direct);
        for cell in [&first, &second, &second, &first] {
            for literal in [Some(true), Some(false)] {
                assert_eq!(
                    cached.contradicts(cell, &[literal]),
                    direct.contradicts(cell, &[literal]),
                );
            }
        }
        assert_eq!(cached.entries.len(), 1);
        assert_eq!(cached.statistics.atom_slots, 1);
        assert_eq!(cached.statistics.coordinate_cells, 4);
        assert_eq!(cached.statistics.hits, 3);
        assert_eq!(cached.statistics.misses, 5);
        assert_eq!(cached.statistics.uncached_results, 4);
        assert!(cached.entries.contains_key(&first));
        assert!(!cached.entries.contains_key(&second));
    }
}

#[test]
fn unsupported_maps_and_equations_keep_the_original_conservative_short_circuit() {
    let context = CoefficientContext::new(["d", "a", "b"]);
    let malformed = context.coefficient_fixture("a*b").numerator;
    let valid = context.coefficient_fixture("a-b").numerator;
    let atoms = [
        Atom {
            indices: &[1, 2],
            equation: &malformed,
        },
        Atom {
            indices: &[1, 2],
            equation: &valid,
        },
    ];
    let cell = LatticeBox::try_new([0; 2], [Some(0); 2]).unwrap();
    let mut cache = RestrictionCache::new(&[false; 2], &atoms);
    for _ in 0..2 {
        assert_eq!(cache.contradicts(&cell, &[Some(false); 2]), Ok(false));
    }
    assert_eq!(cache.statistics.misses, 1);
    assert_eq!(cache.statistics.hits, 1);
    assert_eq!(cache.entries[&cell][0], Some(RestrictionTruth::Unsupported));
    assert_eq!(cache.entries[&cell][1], None);
    let foreign = [Atom {
        indices: &[0, 2],
        equation: &valid,
    }];
    let mut foreign = RestrictionCache::new(&[false; 2], &foreign);
    assert_eq!(foreign.contradicts(&cell, &[Some(false)]), Ok(false));
    assert_eq!(foreign.contradicts(&cell, &[Some(false)]), Ok(false));
    assert_eq!(
        foreign.entries[&cell][0],
        Some(RestrictionTruth::Unsupported)
    );
}

#[test]
fn exhausted_work_is_never_cached_as_unknown_or_as_a_proof() {
    let context = CoefficientContext::new(["d", "a", "b"]);
    let equation = context.coefficient_fixture("a-b").numerator;
    let atoms = [Atom {
        indices: &[1, 2],
        equation: &equation,
    }];
    let cell = LatticeBox::try_new([0; 2], [Some(0); 2]).unwrap();
    let mut cache = RestrictionCache::new(&[false; 2], &atoms);
    cache.budget.remaining = 1;
    for literal in [Some(false), Some(true)] {
        assert_eq!(cache.contradicts(&cell, &[literal]), Err(WorkExhausted));
    }
    assert_eq!(cache.statistics.hits, 0);
    assert_eq!(cache.statistics.native_work, 0);
    assert!(cache.entries.is_empty());
    assert_eq!(cache.budget.remaining, 0);
}

#[test]
fn repeated_native_work_is_measurably_eliminated_without_increasing_its_budget() {
    let context = CoefficientContext::new(["d", "a", "b"]);
    let equation = context.coefficient_fixture("a-b").numerator;
    let atoms = [Atom {
        indices: &[1, 2],
        equation: &equation,
    }];
    let cell = LatticeBox::try_new([0; 2], [Some(0); 2]).unwrap();
    let exact_cost = (equation.exponents.len() + equation.coefficients.len()) * 3;
    let mut cached = RestrictionCache::new(&[false; 2], &atoms);
    cached.budget.remaining = exact_cost;
    for _ in 0..1_000 {
        assert_eq!(cached.contradicts(&cell, &[Some(true)]), Ok(false));
    }
    assert_eq!(cached.statistics.misses, 1);
    assert_eq!(cached.statistics.hits, 999);
    assert_eq!(cached.statistics.native_work, exact_cost);
    assert_eq!(cached.budget.remaining, 0);
    let mut direct = RestrictionCache::new(&[false; 2], &atoms);
    disable_cache(&mut direct);
    direct.budget.remaining = exact_cost;
    assert_eq!(direct.contradicts(&cell, &[Some(true)]), Ok(false));
    assert_eq!(direct.contradicts(&cell, &[Some(true)]), Err(WorkExhausted));
    assert_eq!(direct.statistics.native_work, exact_cost);
    eprintln!(
        "restriction-cache synthetic workload: {:?}",
        cached.statistics
    );
}

#[test]
fn cache_and_direct_traversal_preserve_the_same_genuine_uncovered_witness() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equation = context.coefficient_fixture("a-b").numerator;
    let atoms = [Atom {
        indices: &[1, 2, 3],
        equation: &equation,
    }];
    let clauses = [Clause {
        domain: LatticeBox::try_new([1, 0, 0], [None; 3]).unwrap(),
        literals: vec![(0, true)],
    }];
    let sector = [false; 3];
    let mut cached = TraversalWork::new(&sector, &atoms);
    let mut direct = TraversalWork::new(&sector, &atoms);
    disable_cache(&mut direct.consistency);
    let mut assignments = [Some(false)];
    let mut direct_assignments = assignments;
    let limits = PredicateCoverLimits::default();
    let cached_result = check_valuations(
        &sector,
        &atoms,
        &clauses,
        &[],
        &mut assignments,
        &mut cached,
        limits,
    );
    let direct_result = check_valuations(
        &sector,
        &atoms,
        &clauses,
        &[],
        &mut direct_assignments,
        &mut direct,
        limits,
    );
    assert!(matches!(
        cached_result,
        Err(PredicateCoverError::Uncovered { .. })
    ));
    assert_eq!(cached_result, direct_result);
    assert_eq!(assignments, direct_assignments);
    assert_eq!(cached.nodes, direct.nodes);
}
