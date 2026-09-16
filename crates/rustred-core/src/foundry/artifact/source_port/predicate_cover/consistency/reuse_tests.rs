use super::super::{CacheLimits, WorkBudget};
use super::*;
use crate::algebra::CoefficientContext;

fn atoms(equations: &[CoefficientPolynomial]) -> Vec<Atom<'_>> {
    equations
        .iter()
        .map(|equation| Atom {
            indices: &[1, 2, 3],
            equation,
        })
        .collect()
}

#[test]
fn false_prefix_growth_and_reordering_reuse_only_completed_native_work() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations =
        ["a-b", "b-c", "a-c", "2*a-2*b"].map(|s| context.coefficient_fixture(s).numerator);
    let atoms = atoms(&equations);
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let mut cache = RestrictionCache::new(&[false; 3], &atoms);
    let mut direct = RestrictionCache::new(&[false; 3], &atoms);
    direct.limits.boxes = 0;
    for assignment in [
        [Some(true), Some(false), None, None],
        [Some(true), Some(false), Some(false), None],
        [Some(true), None, Some(false), None],
        [Some(true), Some(false), Some(false), Some(false)],
    ] {
        assert_eq!(
            cache.contradicts(&cell, &assignment),
            direct.contradicts(&cell, &assignment)
        );
    }
    assert_eq!(cache.statistics.base_calls, 1);
    assert_eq!(cache.statistics.extension_calls, 3);
    assert_eq!(cache.statistics.true_materializations, 3);
    assert_eq!(cache.statistics.false_materializations, 3);
    assert_eq!(cache.statistics.implication_replaces, 0);
    assert_eq!(cache.statistics.atom_slots, 4);
    assert_eq!(cache.statistics.coordinate_cells, 6);
    let before = cache.statistics;
    cache.budget.remaining = 0;
    let assignments = [Some(true), Some(false), Some(false), Some(false)];
    assert_eq!(
        cache.contradicts_implications_using(
            &cell,
            &assignments,
            &[(2, false), (0, true), (1, false), (3, false)],
            |_, _, _| panic!("all-hit call must not invoke native algebra")
        ),
        Ok(true)
    );
    assert_eq!(cache.statistics.native_work, before.native_work);
    assert_eq!(
        cache.statistics.true_materializations,
        before.true_materializations
    );
    assert_eq!(
        cache.statistics.false_materializations,
        before.false_materializations
    );
    assert_eq!(cache.statistics.base_calls, before.base_calls);
    assert_eq!(cache.statistics.extension_calls, before.extension_calls);
    assert_eq!(cache.statistics.native_panics, 0);
}

#[test]
fn dropping_or_adding_a_true_atom_never_reuses_another_true_basis() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a-b", "b-c", "a-c"].map(|s| context.coefficient_fixture(s).numerator);
    let atoms = atoms(&equations);
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let mut cache = RestrictionCache::new(&[false; 3], &atoms);
    for (assignment, expected) in [
        ([Some(true), Some(true), Some(false)], true),
        ([Some(true), None, Some(false)], false),
        ([None, Some(true), Some(false)], false),
        ([Some(true), Some(true), Some(false)], true),
    ] {
        assert_eq!(cache.contradicts(&cell, &assignment), Ok(expected));
    }
    assert_eq!(cache.statistics.base_calls, 3);
    assert_eq!(cache.statistics.extension_calls, 3);
    assert_eq!(cache.statistics.base_hits, 1);
    assert_eq!(cache.implications.len(), 3);
}

#[test]
fn true_atoms_specialized_to_zero_remain_in_the_complete_key_and_slot_accounting() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a", "b-c", "2*b-2*c"].map(|s| context.coefficient_fixture(s).numerator);
    let atoms = atoms(&equations);
    let cell = LatticeBox::try_new([0; 3], [Some(0), None, None]).unwrap();
    let mut cache = RestrictionCache::new(&[false; 3], &atoms);
    assert_eq!(
        cache.contradicts(&cell, &[None, Some(true), Some(false)]),
        Ok(true)
    );
    assert_eq!(
        cache.contradicts(&cell, &[Some(true), Some(true), Some(false)]),
        Ok(true)
    );
    assert_eq!(cache.implications.len(), 2);
    assert_eq!(cache.entries.len(), 1);
    assert_eq!(cache.statistics.atom_slots, 3 + (1 + 1) + (2 + 1));
    assert_eq!(cache.statistics.coordinate_cells, 3 * 6);
    assert_eq!(cache.statistics.base_calls, 2);
    let before = cache.statistics;
    cache.budget.remaining = 0;
    assert_eq!(
        cache.contradicts(&cell, &[Some(true), Some(true), Some(false)]),
        Ok(true)
    );
    assert_eq!(
        cache.statistics.classification_materializations,
        before.classification_materializations
    );
    assert_eq!(
        cache.statistics.true_materializations,
        before.true_materializations
    );
    assert_eq!(
        cache.statistics.false_materializations,
        before.false_materializations
    );
    assert_eq!(
        cache.statistics.implication_replaces,
        before.implication_replaces
    );
}

#[test]
fn exact_lower_and_finite_or_infinite_upper_endpoints_keep_separate_scalar_entries() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a-b", "2*a-2*b"].map(|s| context.coefficient_fixture(s).numerator);
    let atoms = atoms(&equations);
    let mut cache = RestrictionCache::new(&[false; 3], &atoms);
    let cells = [
        LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
        LatticeBox::try_new([1, 0, 0], [None; 3]).unwrap(),
        LatticeBox::try_new([0; 3], [Some(1), None, None]).unwrap(),
        LatticeBox::try_new([0; 3], [None, Some(1), None]).unwrap(),
    ];
    for cell in &cells {
        assert_eq!(
            cache.contradicts(cell, &[Some(true), Some(false)]),
            Ok(true)
        );
    }
    assert_eq!(cache.statistics.base_calls, cells.len());
    assert_eq!(cache.statistics.extension_calls, cells.len());
    cache.budget.remaining = 0;
    for cell in &cells {
        assert_eq!(
            cache.contradicts(cell, &[Some(true), Some(false)]),
            Ok(true)
        );
    }
}

#[test]
fn scalar_results_remain_bound_to_sector_atom_order_and_native_variable_context() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a+b", "b", "2*b"].map(|s| context.coefficient_fixture(s).numerator);
    let atoms = atoms(&equations);
    let cell = LatticeBox::try_new([0; 3], [Some(0), None, None]).unwrap();
    let mut negative = RestrictionCache::new(&[false; 3], &atoms);
    assert_eq!(
        negative.contradicts(&cell, &[Some(true), Some(false), None]),
        Ok(true)
    );
    let mut positive = RestrictionCache::new(&[true, false, false], &atoms);
    assert_eq!(
        positive.contradicts(&cell, &[Some(true), Some(false), None]),
        Ok(false)
    );
    assert_eq!(negative.statistics.base_calls, 1);
    assert_eq!(positive.statistics.base_calls, 1);
    // The same ordinal assignments name different polynomials after reordering.
    let reordered = [
        Atom {
            indices: &[1, 2, 3],
            equation: &equations[1],
        },
        Atom {
            indices: &[1, 2, 3],
            equation: &equations[2],
        },
        Atom {
            indices: &[1, 2, 3],
            equation: &equations[0],
        },
    ];
    let mut reordered = RestrictionCache::new(&[true, false, false], &reordered);
    assert_eq!(
        reordered.contradicts(&cell, &[Some(true), Some(false), None]),
        Ok(true)
    );
    assert_eq!(reordered.statistics.base_calls, 1);
    // A different original-variable order needs its own immutable context.
    let remapped_context = CoefficientContext::new(["q", "c", "b", "a"]);
    let remapped_equations =
        ["a+b", "b"].map(|s| remapped_context.coefficient_fixture(s).numerator);
    let remapped_atoms = remapped_equations
        .iter()
        .map(|equation| Atom {
            indices: &[3, 2, 1],
            equation,
        })
        .collect::<Vec<_>>();
    let mut remapped = RestrictionCache::new(&[false; 3], &remapped_atoms);
    assert_eq!(
        remapped.contradicts(&cell, &[Some(true), Some(false)]),
        Ok(true)
    );
    assert_eq!(remapped.statistics.base_calls, 1);
    negative.budget.remaining = 0;
    assert_eq!(
        negative.contradicts(&cell, &[Some(true), Some(false), None]),
        Ok(true)
    );
    assert_eq!(negative.statistics.base_calls, 1);
}

#[test]
fn newly_present_unsupported_false_atom_is_admitted_before_a_warm_proof() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let foreign = CoefficientContext::new(["q", "x", "y", "z"]);
    let base = context.coefficient_fixture("a-b").numerator;
    let implied = context.coefficient_fixture("2*a-2*b").numerator;
    let foreign_polynomial = foreign.coefficient_fixture("x-y").numerator;
    let nonlinear = context.coefficient_fixture("a*b").numerator;
    let mut malformed = implied.clone();
    malformed.exponents.pop();
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    for (polynomial, indices) in [
        (&foreign_polynomial, &[1, 2, 3][..]),
        (&nonlinear, &[1, 2, 3][..]),
        (&malformed, &[1, 2, 3][..]),
        (&implied, &[2, 1, 3][..]),
    ] {
        let atoms = [
            Atom {
                indices,
                equation: polynomial,
            },
            Atom {
                indices: &[1, 2, 3],
                equation: &base,
            },
            Atom {
                indices: &[1, 2, 3],
                equation: &implied,
            },
        ];
        let mut cache = RestrictionCache::new(&[false; 3], &atoms);
        assert_eq!(
            cache.contradicts(&cell, &[None, Some(true), Some(false)]),
            Ok(true)
        );
        cache.budget.remaining = 0;
        assert_eq!(
            cache.contradicts(&cell, &[Some(false), Some(true), Some(false)]),
            Ok(false)
        );
        assert_eq!(cache.statistics.base_hits, 0);
        assert_eq!(cache.statistics.extension_hits, 0);
        assert_eq!(cache.statistics.base_calls, 1);
    }
}

#[test]
fn warmed_base_cannot_bypass_the_unresolved_equation_admission_cap() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a-b", "2*a-2*b"].map(|s| context.coefficient_fixture(s).numerator);
    let atoms = atoms(&equations);
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let mut cache = RestrictionCache::new(&[false; 3], &atoms);
    let assignment = [Some(true), Some(false)];
    assert_eq!(cache.contradicts(&cell, &assignment), Ok(true));
    let mut unresolved = vec![(1, false); MAX_EQUATIONS + 1];
    unresolved[0] = (0, true);
    assert_eq!(
        cache.contradicts_implications(&cell, &assignment, &unresolved),
        Err(WorkExhausted)
    );
    assert_eq!(cache.statistics.base_hits, 0);
    assert_eq!(
        cache.statistics.exhaustion.unwrap().local_cap,
        Some(LocalCap::Equations)
    );
}

#[test]
fn shared_memory_caps_fall_back_without_evicting_any_classification_or_scalar_entry() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a-b", "2*a-2*b"].map(|s| context.coefficient_fixture(s).numerator);
    let atoms = atoms(&equations);
    let classification = LatticeBox::try_new([0; 3], [Some(0); 3]).unwrap();
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let defaults = CacheLimits::default();
    for limits in [
        CacheLimits {
            boxes: 1,
            ..defaults
        },
        CacheLimits {
            atom_slots: 2,
            ..defaults
        },
        CacheLimits {
            coordinate_cells: 6,
            ..defaults
        },
    ] {
        let mut cache = RestrictionCache::new(&[false; 3], &atoms);
        cache.limits = limits;
        assert_eq!(
            cache.contradicts(&classification, &[Some(true), None]),
            Ok(false)
        );
        for _ in 0..2 {
            assert_eq!(
                cache.contradicts(&cell, &[Some(true), Some(false)]),
                Ok(true)
            );
        }
        assert_eq!(cache.entries.len(), 1);
        assert!(cache.implications.is_empty());
        assert_eq!(cache.statistics.base_fallbacks, 2);
        assert_eq!(cache.statistics.extension_fallbacks, 2);
        assert_eq!(cache.statistics.base_calls, 2);
        assert_eq!(cache.statistics.atom_slots, 2);
        assert_eq!(cache.statistics.coordinate_cells, 6);
        cache.budget.remaining = 0;
        assert_eq!(
            cache.contradicts(&cell, &[Some(true), Some(false)]),
            Err(WorkExhausted)
        );
        assert!(cache.implications.is_empty());
    }
    // One true-key slot fits but the false-result slot does not.
    let mut cache = RestrictionCache::new(&[false; 3], &atoms);
    cache.limits.atom_slots = 1;
    for _ in 0..2 {
        assert_eq!(
            cache.contradicts(&cell, &[Some(true), Some(false)]),
            Ok(true)
        );
    }
    assert_eq!(cache.implications.len(), 1);
    assert_eq!(cache.statistics.base_calls, 1);
    assert_eq!(cache.statistics.extension_calls, 2);
    assert_eq!(cache.statistics.extension_fallbacks, 2);
    assert_eq!(cache.statistics.atom_slots, 1);
    // In the opposite insertion order, a scalar entry consumes the shared cap.
    cache.limits.boxes = 1;
    cache.limits.atom_slots = defaults.atom_slots;
    assert_eq!(
        cache.contradicts(&classification, &[Some(true), None]),
        Ok(false)
    );
    assert!(cache.entries.is_empty());
    assert_eq!(cache.statistics.uncached_results, 1);
}

#[test]
fn each_exhaustion_stage_records_only_accepted_work_and_completed_outcomes() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a-b", "b-c"].map(|s| context.coefficient_fixture(s).numerator);
    let atoms = atoms(&equations);
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let true_cost = polynomial_work(&atoms[0], 0).unwrap();
    let false_cost = polynomial_work(&atoms[1], 0).unwrap();
    let base_cost = 16;
    let extension_cost = 32;
    for (stage, accepted, requested, base_calls, has_base) in [
        (Stage::TrueMaterialization, 0, true_cost, 0, false),
        (Stage::BaseReduction, true_cost, base_cost, 0, false),
        (
            Stage::FalseMaterialization,
            true_cost + base_cost,
            false_cost,
            1,
            true,
        ),
        (
            Stage::ExtensionReduction,
            true_cost + base_cost + false_cost,
            extension_cost,
            1,
            true,
        ),
    ] {
        let mut cache = RestrictionCache::new(&[false; 3], &atoms);
        cache.budget.remaining = accepted + requested - 1;
        assert_eq!(
            cache.contradicts(&cell, &[Some(true), Some(false)]),
            Err(WorkExhausted)
        );
        assert_eq!(cache.budget.remaining, 0);
        assert_eq!(cache.statistics.native_work, accepted);
        assert_eq!(cache.statistics.base_calls, base_calls);
        assert_eq!(cache.statistics.extension_calls, 0);
        assert_eq!(!cache.implications.is_empty(), has_base);
        let failure = cache.statistics.exhaustion.unwrap();
        assert_eq!(failure.stage, stage);
        assert_eq!(failure.requested, Some(requested));
        assert_eq!(failure.remaining, requested - 1);
        assert_eq!(failure.shape.true_count, 1);
        assert_eq!(failure.shape.false_count, 1);
        assert_eq!(failure.shape.columns, 4);
        assert_eq!(
            cache.cached_extension(
                cache
                    .scalar_position(&cell, &[Some(true), Some(false)])
                    .ok(),
                1
            ),
            None
        );
        cache.budget = WorkBudget::default();
        assert_eq!(
            cache.contradicts(&cell, &[Some(true), Some(false)]),
            Ok(false)
        );
        assert_eq!(cache.statistics.base_calls, 1);
        assert_eq!(cache.statistics.extension_calls, 1);
        let work = cache.statistics.native_work;
        cache.budget.remaining = 0;
        assert_eq!(
            cache.contradicts(&cell, &[Some(true), Some(false)]),
            Ok(false)
        );
        assert_eq!(cache.statistics.native_work, work);
    }
}

#[test]
fn native_errors_and_panics_are_uncached_and_keep_false_ordinal_short_circuit_order() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a-b", "b-c", "2*a-2*b"].map(|s| context.coefficient_fixture(s).numerator);
    let atoms = atoms(&equations);
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let assignment = [Some(true), Some(false), Some(false)];
    let unresolved = [(0, true), (1, false), (2, false)];
    for panic in [false, true] {
        for fail_at in [1, 2] {
            let mut cache = RestrictionCache::new(&[false; 3], &atoms);
            let mut calls = 0;
            assert_eq!(
                cache.contradicts_implications_using(
                    &cell,
                    &assignment,
                    &unresolved,
                    |fixed, equations, indices| {
                        calls += 1;
                        if calls == fail_at {
                            if panic {
                                panic!("injected native failure");
                            }
                            return Err(());
                        }
                        native_rank(fixed, equations, indices)
                    }
                ),
                Ok(false)
            );
            assert_eq!(calls, fail_at);
            assert_eq!(cache.statistics.native_errors, usize::from(!panic));
            assert_eq!(cache.statistics.native_panics, usize::from(panic));
            assert_eq!(cache.implications.len(), usize::from(fail_at == 2));
            assert_eq!(
                cache.cached_extension(cache.scalar_position(&cell, &assignment).ok(), 1),
                None
            );
            // A later cached proof may not bypass a new earlier failing query.
            assert_eq!(
                cache.contradicts(&cell, &[Some(true), None, Some(false)]),
                Ok(true)
            );
            assert_eq!(
                cache.contradicts_implications_using(&cell, &assignment, &unresolved, |_, _, _| {
                    Err(())
                }),
                Ok(false)
            );
            assert_eq!(cache.statistics.extension_hits, 0);
            assert_eq!(cache.contradicts(&cell, &assignment), Ok(true));
            cache.budget.remaining = 0;
            assert_eq!(cache.contradicts(&cell, &assignment), Ok(true));
        }
    }
}

#[test]
fn inconsistent_extensions_are_successful_nonproofs_but_inconsistent_bases_are_reusable() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a", "a-1"].map(|s| context.coefficient_fixture(s).numerator);
    let atoms = atoms(&equations);
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let mut cache = RestrictionCache::new(&[false; 3], &atoms);
    assert_eq!(
        cache.contradicts(&cell, &[Some(true), Some(false)]),
        Ok(false)
    );
    assert_eq!(cache.statistics.inconsistent_extensions, 1);
    assert_eq!(cache.contradicts(&cell, &[Some(true); 2]), Ok(true));
    assert_eq!(cache.statistics.inconsistent_bases, 1);
    cache.budget.remaining = 0;
    assert_eq!(
        cache.contradicts(&cell, &[Some(true), Some(false)]),
        Ok(false)
    );
    assert_eq!(cache.contradicts(&cell, &[Some(true); 2]), Ok(true));
    assert_eq!(cache.statistics.base_calls, 2);
    assert_eq!(cache.statistics.extension_calls, 1);
}
