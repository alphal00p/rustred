use super::*;
use crate::algebra::CoefficientContext;

fn context() -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["projected_a", "projected_b"]),
        "projected-source-weight-tests",
        2,
    )
    .unwrap()
}
fn key(x: i64) -> IndexShift {
    IndexShift::try_new([x, 0], 2).unwrap()
}
fn row(values: &[(i64, IndexedCoefficient)]) -> BTreeMap<IndexShift, IndexedCoefficient> {
    values.iter().map(|(x, v)| (key(*x), v.clone())).collect()
}
fn limits() -> ProjectedSourceWeightLimits {
    ProjectedSourceWeightLimits {
        arithmetic: ExactAlgebraLimits::default(),
        max_rows: 100,
        max_columns: 100,
        max_input_nonzeros: 1000,
        max_coefficient_terms: 10000,
        max_degree: 8,
        max_probes: 2000,
        max_attempts: 3,
        max_primes: 4,
        max_cached_images: 2000,
        max_cached_values: 200000,
        max_weight_slots: 100,
    }
}
fn run(
    c: &IndexedCoefficientContext,
    rows: &[BTreeMap<IndexShift, IndexedCoefficient>],
    f: &[i64],
) -> ProjectedSourceWeightProposal {
    reconstruct_projected_source_weights(
        c,
        rows,
        &key(0),
        &f.iter().map(|x| key(*x)).collect(),
        limits(),
        |_| {},
    )
    .unwrap()
}
fn multiply(
    c: &IndexedCoefficientContext,
    rows: &[BTreeMap<IndexShift, IndexedCoefficient>],
    weights: &BTreeMap<usize, IndexedCoefficient>,
) -> BTreeMap<IndexShift, IndexedCoefficient> {
    let mut image = BTreeMap::new();
    for (i, w) in weights {
        for (key, a) in &rows[*i] {
            let value = c.mul_with_limits(w, a, limits().arithmetic).unwrap();
            let value = c
                .add_with_limits(
                    &image.remove(key).unwrap_or_else(|| c.zero()),
                    &value,
                    limits().arithmetic,
                )
                .unwrap();
            if !value.is_zero() {
                image.insert(key.clone(), value);
            }
        }
    }
    image
}

#[test]
fn genuine_weight_pole_is_returned_but_computational_pivot_is_not_a_guard() {
    let c = context();
    let f = c.index(0).unwrap();
    let first = row(&[(1, f.clone())]);
    let genuine = vec![first.clone(), row(&[(1, c.one()), (0, c.one())])];
    let a = run(&c, &genuine, &[1]);
    let expected = c
        .neg_with_limits(
            &c.div_with_limits(&c.one(), &f, limits().arithmetic)
                .unwrap(),
            limits().arithmetic,
        )
        .unwrap();
    assert_eq!(a.weights, BTreeMap::from([(0, expected), (1, c.one())]));
    assert_eq!(a.image, row(&[(0, c.one())]));
    assert_eq!(multiply(&c, &genuine, &a.weights), a.image);
    let canceled = vec![first, row(&[(0, c.one())])];
    let b = run(&c, &canceled, &[1]);
    assert_eq!(b.weights, BTreeMap::from([(1, c.one())]));
    assert_eq!(b.prefix_rows, 2); // slot 0 was reconstructed as exactly zero
    assert_eq!(b.image, row(&[(0, c.one())]));
    assert_eq!(multiply(&c, &canceled, &b.weights), b.image);
}

#[test]
fn independent_coupled_variables_and_all_lower_columns_are_reconstructed() {
    let c = context();
    let n = c.index(0).unwrap();
    let m = c.index(1).unwrap();
    let sum = c.add_with_limits(&n, &m, limits().arithmetic).unwrap();
    let product = c.mul_with_limits(&n, &m, limits().arithmetic).unwrap();
    let rows = vec![
        row(&[(2, n.clone()), (-2, m.clone())]),
        row(&[(2, c.one()), (0, sum.clone()), (-1, product)]),
    ];
    let p = run(&c, &rows, &[2, 3]); // key3 is a structurally absent full-F obligation
    assert_eq!(p.prefix_rows, 2);
    assert_eq!(p.original_variables, 4);
    assert_eq!(p.active_variables, 2);
    assert_eq!(p.image.len(), 3); // target and BOTH lower columns, not sampled omissions
    assert_eq!(p.image[&key(0)], c.one());
    assert_eq!(multiply(&c, &rows, &p.weights), p.image);
    for value in p.weights.values().chain(p.image.values()) {
        c.validate_with_limits(value, limits().arithmetic).unwrap();
        assert_eq!(value.raw().get_variables(), c.one().raw().get_variables());
    }
    let expected = c
        .div_with_limits(&c.one(), &sum, limits().arithmetic)
        .unwrap();
    assert_eq!(p.weights[&1], expected);
}

#[test]
fn original_ordered_layout_target_and_absent_forbidden_are_preserved() {
    let c = context();
    let n = c.index(0).unwrap();
    let rows = vec![row(&[(-5, n.clone()), (0, c.one()), (9, n)])];
    let mut events = Vec::new();
    let p = reconstruct_projected_source_weights(
        &c,
        &rows,
        &key(0),
        &BTreeSet::from([key(2)]),
        limits(),
        |e| events.push(e),
    )
    .unwrap();
    assert_eq!(p.image, rows[0]);
    assert!(matches!(
        events[0],
        ProjectedSourceWeightEvent::FramePrepared {
            rows: 1,
            columns: 4,
            target_column: 1,
            original_variables: 4,
            active_variables: 1
        }
    ));
    assert!(events.iter().any(|e| matches!(
        e,
        ProjectedSourceWeightEvent::ExactProductFinished { output_terms: 3 }
    )));
}

#[test]
fn every_post_hit_input_is_authenticated_and_input_poles_remain_caller_obligations() {
    let c = context();
    let n = c.index(0).unwrap();
    let g = c.index(1).unwrap();
    let one_over_g = c
        .div_with_limits(&c.one(), &g, limits().arithmetic)
        .unwrap();
    let mut rows = vec![row(&[(0, n)]), row(&[(-1, one_over_g.clone())])];
    let p = run(&c, &rows, &[]);
    assert_eq!(p.prefix_rows, 1);
    assert_eq!(p.weights.len(), 1);
    assert_eq!(rows[1][&key(-1)], one_over_g); // never discard this input condition inventory
    rows[1] = row(&[(-1, c.zero())]);
    assert!(matches!(
        reconstruct_projected_source_weights(
            &c,
            &rows,
            &key(0),
            &BTreeSet::new(),
            limits(),
            |_| {}
        ),
        Err(ProjectedSourceWeightError::Invalid(
            "explicit zero source entry"
        ))
    ));
    let foreign =
        IndexedCoefficientContext::try_new(&CoefficientContext::new(["foreign"]), "foreign", 2)
            .unwrap();
    rows[1] = row(&[(-1, foreign.one())]);
    assert!(matches!(
        reconstruct_projected_source_weights(
            &c,
            &rows,
            &key(0),
            &BTreeSet::new(),
            limits(),
            |_| {}
        ),
        Err(ProjectedSourceWeightError::Algebra(
            IndexedAlgebraError::WrongContext
        ))
    ));
    // Malformed native payloads cannot even obtain a sealed IndexedCoefficient.
    let mut malformed = c.one().raw().clone();
    malformed.denominator = c.zero().raw().numerator.clone();
    assert!(
        c.admit_native_result_with_limits(malformed, limits().arithmetic)
            .is_err()
    );
}

#[test]
fn target_shape_and_structural_zero_columns_are_budgeted() {
    let c = context();
    let rows = vec![row(&[(0, c.index(0).unwrap())])];
    for (target, f) in [
        (key(0), BTreeSet::from([key(0)])),
        (IndexShift::try_new([0], 1).unwrap(), BTreeSet::new()),
        (
            key(0),
            BTreeSet::from([IndexShift::try_new([1], 1).unwrap()]),
        ),
    ] {
        assert!(
            reconstruct_projected_source_weights(&c, &rows, &target, &f, limits(), |_| {}).is_err()
        );
    }
    let mut l = limits();
    l.max_columns = 1;
    assert!(matches!(
        reconstruct_projected_source_weights(
            &c,
            &rows,
            &key(0),
            &BTreeSet::from([key(2)]),
            l,
            |_| {}
        ),
        Err(ProjectedSourceWeightError::Budget("full physical columns"))
    ));
    let bad = vec![BTreeMap::from([(
        IndexShift::try_new([0], 1).unwrap(),
        c.one(),
    )])];
    assert!(
        reconstruct_projected_source_weights(&c, &bad, &key(0), &BTreeSet::new(), limits(), |_| {})
            .is_err()
    );
}

#[test]
fn explicit_input_and_reconstruction_allowances_refuse_without_fallback() {
    let c = context();
    let rows = vec![row(&[(0, c.index(0).unwrap())])];
    let mut cases = Vec::new();
    let mut l = limits();
    l.max_rows = 0;
    cases.push(l);
    let mut l = limits();
    l.max_input_nonzeros = 0;
    cases.push(l);
    let mut l = limits();
    l.max_coefficient_terms = 1;
    cases.push(l);
    let mut l = limits();
    l.max_cached_values = 1;
    cases.push(l);
    let mut l = limits();
    l.max_cached_images = 0;
    cases.push(l);
    let mut l = limits();
    l.max_weight_slots = 0;
    cases.push(l);
    let mut l = limits();
    l.max_primes = 1;
    cases.push(l);
    let mut l = limits();
    l.max_probes = 0;
    cases.push(l);
    let mut l = limits();
    l.max_degree = 0;
    cases.push(l);
    let mut l = limits();
    l.max_attempts = 0;
    cases.push(l);
    for l in cases {
        assert!(
            reconstruct_projected_source_weights(&c, &rows, &key(0), &BTreeSet::new(), l, |_| {})
                .is_err()
        );
    }
    let mut l = limits();
    l.max_coefficient_terms = 2; // input fits; returned unit/weight do not
    assert!(matches!(
        reconstruct_projected_source_weights(&c, &rows, &key(0), &BTreeSet::new(), l, |_| {}),
        Err(ProjectedSourceWeightError::Budget(
            "cumulative coefficient terms"
        ))
    ));
}

#[test]
fn dependent_empty_miss_and_constant_frames_are_explicit_native_eligibility_limits() {
    let c = context();
    let n = c.index(0).unwrap();
    let harder = row(&[(1, n.clone())]);
    for rows in [
        vec![BTreeMap::new(), row(&[(0, n.clone())])],
        vec![harder.clone(), harder.clone(), row(&[(0, n.clone())])],
        vec![row(&[(-1, n.clone())]), row(&[(0, n.clone())])],
        vec![row(&[(1, n), (0, c.one())])],
        vec![row(&[(0, c.one())])],
    ] {
        assert!(
            reconstruct_projected_source_weights(
                &c,
                &rows,
                &key(0),
                &BTreeSet::from([key(1)]),
                limits(),
                |_| {}
            )
            .is_err()
        );
    }
}

#[test]
fn exact_full_product_rejects_mutated_weight_or_missing_lower_tail() {
    let c = context();
    let rows = vec![row(&[(0, c.index(0).unwrap()), (-1, c.index(1).unwrap())])];
    let p = run(&c, &rows, &[]);
    let variables = FrameVariables::from_coefficients(
        c.one().raw().get_variables().clone(),
        rows.iter()
            .flat_map(|r| r.values().map(IndexedCoefficient::raw)),
    )
    .unwrap();
    let columns = [key(0), key(-1)];
    let frame = ProbeFrame::from_ordered_rows(
        vec![
            columns
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    (
                        i as u32,
                        variables.map_coefficient(rows[0][k].raw()).unwrap(),
                    )
                })
                .collect(),
        ],
        2,
    )
    .unwrap();
    let weight = variables.map_coefficient(p.weights[&0].raw()).unwrap();
    let target: Vec<_> = columns
        .iter()
        .enumerate()
        .map(|(i, k)| {
            (
                i as u32,
                variables.map_coefficient(p.image[k].raw()).unwrap(),
            )
        })
        .collect();
    assert!(
        super::super::validation::validate_product(
            &frame,
            1,
            0,
            &[weight.clone()],
            &target,
            &variables
        )
        .is_ok()
    );
    assert!(
        super::super::validation::validate_product(
            &frame,
            1,
            0,
            &[weight],
            &target[..1],
            &variables
        )
        .is_err()
    );
    let bad = variables.map_coefficient(c.one().raw()).unwrap();
    assert!(
        super::super::validation::validate_product(&frame, 1, 0, &[bad], &target, &variables)
            .is_err()
    );
}
