use super::*;
use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::AffineDenominator;

fn family(mass_squared: i64) -> IntegralFamily {
    let base = CoefficientContext::new(["d"]);
    IntegralFamily::new(
        "checked-original-producer-k1",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            base.integer(-mass_squared),
            vec![base.one()],
        )],
        Vec::new(),
        vec![base.zero()],
    )
    .unwrap()
}

fn descending(
    family: &IntegralFamily,
) -> (IndexedCoefficientContext, OriginalSourceCombinationRequest) {
    let generator = ParametricIbpGenerator::try_new(family).unwrap();
    let c = generator.context().clone();
    let n_minus_one = c.sub(&c.index(0).unwrap(), &c.one()).unwrap();
    let twice = c.mul(&c.integer(2), &n_minus_one).unwrap();
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let weight = c.div(&c.integer(-1), &twice).unwrap();
    let rhs = c.div(&c.sub(&d, &twice).unwrap(), &twice).unwrap();
    let request = OriginalSourceCombinationRequest {
        root_sector: Mask::try_new([true]).unwrap(),
        sector: Mask::try_new([true]).unwrap(),
        ordering: OrderingPolicy::SpiredUncutV1,
        lower: vec![1],
        upper: vec![None],
        fixed: Vec::new(),
        contributions: vec![OriginalSourceContribution {
            source_row: RowId::OrdinaryIbp {
                contraction_momentum: 0,
                differentiated_loop: 0,
            },
            offset: IntegralShift::try_new([-1]).unwrap(),
            weight,
        }],
        rhs: vec![(IndexShift::try_new([-1], 1).unwrap(), rhs)],
        retained_conditions: Vec::new(),
    };
    (c, request)
}

fn failure(family: &IntegralFamily, request: OriginalSourceCombinationRequest) -> String {
    check_original_source_combination(family, request, Default::default())
        .unwrap_err()
        .to_string()
}

#[test]
fn exact_unbounded_k1_identity_preserves_original_sources_and_scope() {
    let f = family(1);
    let (c, request) = descending(&f);
    let checked = check_original_source_combination(&f, request, Default::default()).unwrap();
    assert_eq!(checked.family_fingerprint(), f.fingerprint());
    assert_eq!(checked.context_fingerprint(), c.fingerprint());
    assert_eq!(checked.root_sector().active_bits(), [true]);
    assert_eq!(checked.sector().active_bits(), [true]);
    assert_eq!(checked.ordering(), &OrderingPolicy::SpiredUncutV1);
    assert_eq!(checked.requested_bounds(), (&[1][..], &[None][..]));
    assert_eq!(checked.cell_bounds(0), Some((&[1][..], &[None][..])));
    assert_eq!(checked.cell_bounds(1), None);
    assert_eq!(checked.cells().len(), 1);
    let cell = checked.cells().next().unwrap();
    assert_eq!(cell.sources().provenance().len(), 1);
    let p = cell.sources().provenance()[0].translated();
    assert_eq!(
        p.source_row(),
        &RowId::OrdinaryIbp {
            contraction_momentum: 0,
            differentiated_loop: 0
        }
    );
    assert_eq!(p.offset().values(), [-1]);
    assert_eq!(cell.rule().right_hand_side().len(), 1);
    assert!(!cell.rule().nonzero_guards().is_empty());
}

#[test]
fn checked_cells_reconstruct_the_exact_original_request_without_pivot_search() {
    let f = family(1);
    let (_, request) = descending(&f);
    let checked = check_original_source_combination(&f, request, Default::default()).unwrap();
    let cell = checked.cells().next().unwrap();
    let contributions = cell
        .rule()
        .source_combination()
        .iter()
        .map(|term| {
            let p = cell.sources().provenance()[term.source_ordinal()].translated();
            OriginalSourceContribution {
                source_row: p.source_row().clone(),
                offset: p.offset().clone(),
                weight: term.coefficient().clone(),
            }
        })
        .collect();
    let (lower, upper) = checked.cell_bounds(0).unwrap();
    let replay = OriginalSourceCombinationRequest {
        root_sector: checked.root_sector().clone(),
        sector: checked.sector().clone(),
        ordering: checked.ordering().clone(),
        lower: lower.to_vec(),
        upper: upper.to_vec(),
        fixed: cell.fixed_restrictions().to_vec(),
        contributions,
        rhs: cell
            .rule()
            .right_hand_side()
            .iter()
            .map(|t| (t.shift().clone(), t.coefficient().clone()))
            .collect(),
        retained_conditions: cell
            .rule()
            .nonzero_guards()
            .iter()
            .map(|g| g.polynomial().clone())
            .collect(),
    };
    let replayed = check_original_source_combination(&f, replay, Default::default()).unwrap();
    assert_eq!(replayed.requested_bounds(), checked.requested_bounds());
    let rhs = replayed.cells().next().unwrap().rule().right_hand_side();
    assert_eq!(rhs[0].shift(), cell.rule().right_hand_side()[0].shift());
    assert_eq!(
        rhs[0].coefficient(),
        cell.rule().right_hand_side()[0].coefficient()
    );
}

#[test]
fn changed_weight_rhs_offset_or_family_cannot_authenticate_the_old_formula() {
    let f = family(1);
    let (c, request) = descending(&f);
    let mut wrong = request.clone();
    wrong.contributions[0].weight = c
        .mul(&c.integer(2), &wrong.contributions[0].weight)
        .unwrap();
    assert!(failure(&f, wrong).contains("residual"));
    let mut wrong = request.clone();
    wrong.rhs[0].1 = c
        .neg_with_limits(&wrong.rhs[0].1, Default::default())
        .unwrap();
    assert!(failure(&f, wrong).contains("residual"));
    let mut wrong = request.clone();
    wrong.contributions[0].offset = IntegralShift::try_new([0]).unwrap();
    assert!(failure(&f, wrong).contains("residual"));
    assert!(check_original_source_combination(&family(2), request, Default::default()).is_err());
}

#[test]
fn forged_nonordinary_or_out_of_family_row_ids_are_rejected() {
    let f = family(1);
    let (_, request) = descending(&f);
    for id in [
        RowId::OrdinaryIbp {
            contraction_momentum: 1,
            differentiated_loop: 0,
        },
        RowId::OrdinaryIbp {
            contraction_momentum: 0,
            differentiated_loop: 1,
        },
        RowId::Derived {
            label: Arc::from("forged ordinary row"),
        },
    ] {
        let mut wrong = request.clone();
        wrong.contributions[0].source_row = id;
        assert!(failure(&f, wrong).contains("RowId is absent"));
    }
}

#[test]
fn duplicate_weights_cannot_cancel_away_a_singular_denominator() {
    let f = family(1);
    let (c, mut request) = descending(&f);
    let pole = c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap();
    let singular = c.div(&c.one(), &pole).unwrap();
    let mut extra = request.contributions[0].clone();
    request.contributions[0].weight = c.add(&request.contributions[0].weight, &singular).unwrap();
    extra.weight = c.neg_with_limits(&singular, Default::default()).unwrap();
    request.contributions.push(extra);
    assert!(failure(&f, request).contains("duplicate original"));
}

#[test]
fn original_weight_pole_and_retained_guard_must_miss_the_entire_domain() {
    let f = family(1);
    let (c, request) = descending(&f);
    let mut pole = request.clone();
    pole.lower[0] = 0; // n=1 is the original normalization pole.
    assert!(failure(&f, pole).contains("guard"));
    let mut extra = request;
    extra.retained_conditions.push(
        c.numerator_condition_with_limits(
            &c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap(),
            Default::default(),
        )
        .unwrap(),
    );
    assert!(failure(&f, extra.clone()).contains("guard"));
    extra.lower[0] = 2; // n>=3 excludes the pole exactly, not by a sample.
    assert!(check_original_source_combination(&f, extra, Default::default()).is_ok());
}

#[test]
fn fixed_faces_cannot_be_widened_and_empty_rhs_is_not_a_new_terminal() {
    let f = family(1);
    let (_, mut request) = descending(&f);
    request.fixed = vec![FixedIndexRestriction::new(0, 2)];
    request.upper = vec![Some(1)];
    assert!(check_original_source_combination(&f, request.clone(), Default::default()).is_ok());
    let mut wrong = request.clone();
    wrong.upper[0] = None;
    assert!(failure(&f, wrong).contains("fixed coordinate does not hold"));
    request.rhs.clear();
    assert!(failure(&f, request).contains("nonempty RHS"));
}

#[test]
fn exact_but_raising_ordinary_identity_fails_descent() {
    let f = family(1);
    let (c, mut request) = descending(&f);
    let twice_n = c.mul(&c.integer(2), &c.index(0).unwrap()).unwrap();
    let pivot = c
        .sub(
            &c.lift(&c.base().parameter("d").unwrap()).unwrap(),
            &twice_n,
        )
        .unwrap();
    request.contributions[0].offset = IntegralShift::try_new([0]).unwrap();
    request.contributions[0].weight = c.div(&c.one(), &pivot).unwrap();
    request.rhs = vec![(
        IndexShift::try_new([1], 1).unwrap(),
        c.div(&twice_n, &pivot).unwrap(),
    )];
    let error = check_original_source_combination(&f, request, Default::default()).unwrap_err();
    assert!(
        matches!(&error, SourcePortAuditError::UnprovedDescentObligation {
        term_ordinal: 0, shift, local_lower, local_upper, child_sector,
    } if shift == &[1] && local_lower == &[1] && local_upper == &[None] && child_sector == &[true])
    );
    let message = error.to_string();
    assert!(
        message.contains("not uniformly lower on local box [1]..[None]")
            && message.contains("shift=[1], child_sector=[true]"),
        "{message}"
    );
}

#[test]
fn root_scope_and_nonzero_successor_activation_are_separate_gates() {
    let f = family(1);
    let (_, mut request) = descending(&f);
    request.root_sector = Mask::try_new([false]).unwrap();
    assert!(failure(&f, request).contains("root/sector/domain"));
    // An inactive source can acquire a positive child even while the total
    // number of active axes drops. The root gate must not use support count.
    let piece = LatticeBox::try_new([0, 0], [Some(0), Some(0)]).unwrap();
    assert!(validate_root_successor(&[true, false], &piece, &[-1, 1]).is_err());
    let inactive = LatticeBox::try_new([0, 1], [Some(0), None]).unwrap();
    assert!(validate_root_successor(&[true, false], &inactive, &[-1, 1]).is_ok());
}

#[test]
fn foreign_coefficient_context_and_geometry_budgets_fail_closed() {
    let f = family(1);
    let (_, request) = descending(&f);
    let foreign_base = CoefficientContext::new(["d", "foreign"]);
    let foreign =
        IndexedCoefficientContext::try_new(&foreign_base, "foreign-proof-context", 1).unwrap();
    let mut wrong = request.clone();
    wrong.contributions[0].weight = foreign.one();
    assert!(check_original_source_combination(&f, wrong, Default::default()).is_err());
    let mut limits = OriginalSourceCombinationLimits::default();
    limits.geometry.max_requested_boxes = 0;
    assert!(matches!(
        check_original_source_combination(&f, request.clone(), limits),
        Err(SourcePortAuditError::ResourceBudgetExhausted { .. })
    ));
    let mut limits = OriginalSourceCombinationLimits::default();
    limits.rule.max_source_combination_terms = 0;
    assert!(matches!(
        check_original_source_combination(&f, request, limits),
        Err(SourcePortAuditError::ResourceBudgetExhausted { .. })
    ));
}

#[test]
fn prefix_order_cannot_borrow_a_sector_primary_certificate() {
    use rustred_order::{
        CompiledOrder, CoordinateGroups, DegreeRow, Direction, Limits, OrderDescriptor,
    };
    let f = family(1);
    let (_, mut request) = descending(&f);
    let descriptor = OrderDescriptor {
        pre_support_degree_rows: vec![DegreeRow {
            active: vec![1],
            inactive: vec![1],
        }],
        support_weights: vec![1],
        support_priority: vec![0],
        degree_rows: vec![DegreeRow {
            active: vec![1],
            inactive: vec![1],
        }],
        coordinate_priority: vec![0],
        coordinate_groups: CoordinateGroups::InactiveFirst,
        active_direction: Direction::Descending,
        inactive_direction: Direction::Ascending,
    };
    request.ordering = OrderingPolicy::try_programmed(
        CompiledOrder::compile(descriptor, Limits::default()).unwrap(),
    )
    .unwrap();
    assert!(failure(&f, request).contains("support-primary"));
}
