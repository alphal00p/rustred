use super::*;
use rustred::{
    algebra::{CoefficientContext, IndexedCoefficientContext},
    identity::{IntegralShift, ParametricIbpGenerator, TranslatedSourceRequest},
};
use serde_json::json;

fn context(arity: usize) -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::try_new(["d"]).unwrap(),
        "boundary-polynomial-test",
        arity,
    )
    .unwrap()
}

// Obtain actual native shift handles, never forge the private physical-key type.
fn shift(n: i64) -> IndexShift {
    static SHIFTS: std::sync::OnceLock<BTreeMap<i64, IndexShift>> = std::sync::OnceLock::new();
    SHIFTS.get_or_init(|| {
        let base = CoefficientContext::try_new(["d"]).unwrap();
        let family = rustred::family::IntegralFamily::new(
            "boundary-shift-fixture",
            vec!["q".into()],
            Vec::new(),
            base.clone(),
            base.parameter("d").unwrap(),
            vec![rustred::family::AffineDenominator::new(
                base.integer(-1),
                vec![base.one()],
            )],
            Vec::new(),
            vec![base.zero()],
        )
        .unwrap();
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let prepared = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..prepared.len()).map(|i| prepared.generate(i)).collect();
        let completed = prepared.complete(rows).unwrap();
        let batch = generator
            .translate_selected_completed_source_rows(
                &completed,
                (-2..=2).map(|offset| {
                    TranslatedSourceRequest::new(0, IntegralShift::try_new([offset]).unwrap())
                }),
                Default::default(),
            )
            .unwrap();
        batch
            .sources()
            .iter()
            .flat_map(|source| {
                source
                    .terms()
                    .keys()
                    .map(|key| (key.values()[0], key.clone()))
            })
            .collect()
    })[&n]
        .clone()
}

fn limits() -> Limits {
    Limits {
        arithmetic: Default::default(),
        rows: 64,
        columns: 4096,
        nonzeros: 100000,
        coefficient_terms: 1000000,
        operations: 1000000,
        guards: 100000,
    }
}

fn config(mode: Mode) -> Config {
    Config {
        mode,
        polynomial_axes: vec![0],
        protected_axes: vec![0],
        weight_monomials: vec![vec![0]],
        max_total_degree: 1,
        max_unknowns: 144,
        max_constraints: 8192,
        max_nonzeros: 100000,
        max_activation_faces: 64,
        primitive_original_weights: false,
        target_template: None,
    }
}

// Algebra-only ordinary-unit frame fixture: native source proof is separately
// mandatory in the executable. Each selected row binds a unique RowId+offset.
fn ordinary_frame(
    c: &IndexedCoefficientContext,
    rows: Vec<Row>,
    guards: Vec<Guard>,
) -> super::super::source::Span {
    use super::super::source::{SourceBinding, Span, SpanProvenance};
    Span {
        provenance: SpanProvenance::Ordinary,
        bindings: (0..rows.len())
            .map(|i| SourceBinding {
                row: rustred::identity::RowId::OrdinaryIbp {
                    contraction_momentum: 0,
                    differentiated_loop: 0,
                },
                offset: IntegralShift::try_new(vec![i as i64; c.index_count()]).unwrap(),
            })
            .collect(),
        weights: (0..rows.len())
            .map(|i| Weights::from([(i, c.one())]))
            .collect(),
        images: rows.clone(),
        originals: rows,
        guards,
    }
}

fn primitive_config() -> Config {
    let mut config = config(Mode::ActivationFaces);
    // Deliberately put a nonprimitive representative first. This is a unit
    // counterexample, not a new source/monomial order for the real pilot.
    config.weight_monomials = vec![vec![1], vec![0]];
    config.primitive_original_weights = true;
    config
}

#[path = "template_tests.rs"]
mod template_tests;

#[test]
fn primitive_true_original_weight_multiple_rescues_only_new_circuit() {
    let c = context(1);
    let span = ordinary_frame(
        &c,
        vec![Row::from([
            (shift(0), c.one()),
            (shift(1), c.index(0).unwrap()),
        ])],
        vec![],
    );
    let mut cfg = primitive_config();
    cfg.primitive_original_weights = false;
    let off = project_original(
        &c,
        &span,
        &shift(0),
        &BTreeSet::new(),
        Default::default(),
        limits(),
        &cfg,
    )
    .err()
    .unwrap();
    assert!(
        off.to_string()
            .contains("excludes an entire activation face")
    );
    cfg.primitive_original_weights = true;
    let found = proposal(
        project_original(
            &c,
            &span,
            &shift(0),
            &BTreeSet::new(),
            Default::default(),
            limits(),
            &cfg,
        )
        .unwrap(),
    );
    assert_eq!(found.weights, Weights::from([(0, c.one())]));
    assert_eq!(found.image, span.originals[0]);
    verify_faces(
        &c,
        &found.image,
        &cfg,
        Default::default(),
        &mut found.guards.clone(),
        limits(),
        &mut 0,
    )
    .unwrap();
}

#[test]
fn primitive_endpoint_common_factor_is_not_original_weight_divisibility() {
    let c = context(1);
    let n = c.index(0).unwrap();
    let n2 = c.mul_with_limits(&n, &n, limits().arithmetic).unwrap();
    let span = ordinary_frame(&c, vec![Row::from([(shift(0), n), (shift(1), n2)])], vec![]);
    let mut cfg = primitive_config();
    cfg.weight_monomials = vec![vec![0]];
    let error = project_original(
        &c,
        &span,
        &shift(0),
        &BTreeSet::new(),
        Default::default(),
        limits(),
        &cfg,
    )
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("excludes an entire activation face")
    );
    // Even removing a TRUE common original-weight factor leaves a genuine
    // target zero here. The unchanged new-pivot/face test must still reject.
    cfg.weight_monomials = vec![vec![1], vec![0]];
    let error = project_original(
        &c,
        &span,
        &shift(0),
        &BTreeSet::new(),
        Default::default(),
        limits(),
        &cfg,
    )
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("excludes an entire activation face")
    );
}

#[test]
fn primitive_preserves_incoming_conditions_and_refuses_index_poles() {
    let c = context(1);
    let n = c.index(0).unwrap();
    let guard = Guard {
        polynomial: c
            .numerator_condition_with_limits(&n, limits().arithmetic)
            .unwrap(),
        origin: "genuine original n pole condition".into(),
    };
    let span = ordinary_frame(
        &c,
        vec![Row::from([(shift(0), c.one()), (shift(1), n.clone())])],
        vec![guard],
    );
    let error = project_original(
        &c,
        &span,
        &shift(0),
        &BTreeSet::new(),
        Default::default(),
        limits(),
        &primitive_config(),
    )
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("excludes an entire activation face")
    );
    let mut weights = Weights::from([(
        0,
        c.div_with_limits(&c.one(), &n, limits().arithmetic)
            .unwrap(),
    )]);
    let error = primitive_weights(
        &c,
        &mut weights,
        &primitive_config(),
        &mut Admission::default(),
        &mut 0,
        limits(),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("index-dependent source denominators")
    );
}

#[test]
fn primitive_requires_original_binding_and_preserves_selected_permutation() {
    let c = context(1);
    let row = Row::from([(shift(0), c.one()), (shift(1), c.index(0).unwrap())]);
    let mut span = ordinary_frame(
        &c,
        vec![Row::from([(shift(-1), c.one())]), row.clone()],
        vec![],
    );
    span.images = vec![row];
    span.weights = vec![Weights::from([(1, c.one())])];
    let found = proposal(
        project_original(
            &c,
            &span,
            &shift(0),
            &BTreeSet::new(),
            Default::default(),
            limits(),
            &primitive_config(),
        )
        .unwrap(),
    );
    let (contributions, _) = span.compose(&c, &found, limits()).unwrap();
    assert_eq!(contributions.len(), 1);
    assert_eq!(contributions[0].offset, span.bindings[1].offset);
    assert_eq!(contributions[0].weight, c.one());
    assert!(
        project(
            &c,
            &span.images,
            &shift(0),
            &BTreeSet::new(),
            &[],
            Default::default(),
            limits(),
            &primitive_config()
        )
        .is_err()
    );
    span.provenance = super::super::source::SpanProvenance::Weighted;
    assert!(
        project_original(
            &c,
            &span,
            &shift(0),
            &BTreeSet::new(),
            Default::default(),
            limits(),
            &primitive_config()
        )
        .is_err()
    );
    span.provenance = super::super::source::SpanProvenance::Ordinary;
    span.bindings[1] = span.bindings[0].clone();
    assert!(
        project_original(
            &c,
            &span,
            &shift(0),
            &BTreeSet::new(),
            Default::default(),
            limits(),
            &primitive_config()
        )
        .is_err()
    );
}

#[test]
fn primitive_exact_quotients_must_remain_in_sparse_declared_ansatz() {
    let c = context(1);
    let mut cfg = primitive_config();
    cfg.weight_monomials = vec![vec![1]];
    let before = Weights::from([(0, c.index(0).unwrap())]);
    let mut weights = before.clone();
    let error = primitive_weights(
        &c,
        &mut weights,
        &cfg,
        &mut Admission::default(),
        &mut 0,
        limits(),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("escaped the declared monomial ansatz")
    );
    assert_eq!(weights, before);
}

#[test]
fn primitive_retained_budget_includes_preexisting_reducer_payload() {
    let c = context(1);
    let mut bounded = limits();
    bounded.coefficient_terms = 10;
    let before = Weights::from([(0, c.index(0).unwrap())]);
    let mut standalone = before.clone();
    primitive_after_reducer(
        &c,
        &mut standalone,
        &primitive_config(),
        &mut Admission::default(),
        0,
        &mut 0,
        bounded,
    )
    .unwrap();
    let mut cumulative = before.clone();
    let error = primitive_after_reducer(
        &c,
        &mut cumulative,
        &primitive_config(),
        &mut Admission::default(),
        9,
        &mut 0,
        bounded,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("primitive retained coefficient terms")
    );
    assert_eq!(cumulative, before);
}

#[test]
fn primitive_native_gcd_divides_each_original_weight_and_authenticates_maps() {
    let c = context(1);
    let n = c.index(0).unwrap();
    let mut weights = Weights::from([
        (0, n.clone()),
        (
            1,
            c.mul_with_limits(&c.integer(2), &n, limits().arithmetic)
                .unwrap(),
        ),
    ]);
    let g = primitive_weights(
        &c,
        &mut weights,
        &primitive_config(),
        &mut Admission::default(),
        &mut 0,
        limits(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(g, n);
    assert_eq!(weights, Weights::from([(0, c.one()), (1, c.integer(2))]));
    let other = IndexedCoefficientContext::try_new(
        &CoefficientContext::try_new(["d"]).unwrap(),
        "foreign-primitive",
        1,
    )
    .unwrap();
    let mut foreign = Weights::from([(0, other.index(0).unwrap())]);
    assert!(
        primitive_weights(
            &c,
            &mut foreign,
            &primitive_config(),
            &mut Admission::default(),
            &mut 0,
            limits()
        )
        .is_err()
    );
    for cap in [0, 2, 4] {
        let mut bounded = limits();
        bounded.operations = cap;
        let mut weights = Weights::from([(0, c.index(0).unwrap()), (1, c.index(0).unwrap())]);
        assert!(
            primitive_weights(
                &c,
                &mut weights,
                &primitive_config(),
                &mut Admission::default(),
                &mut 0,
                bounded
            )
            .is_err()
        );
    }
    let mut bounded = limits();
    bounded.coefficient_terms = 1;
    let mut weights = Weights::from([(0, c.index(0).unwrap())]);
    assert!(
        primitive_weights(
            &c,
            &mut weights,
            &primitive_config(),
            &mut Admission::default(),
            &mut 0,
            bounded
        )
        .is_err()
    );
}

#[test]
fn primitive_explicit_off_preserves_full_export_report_and_bytes() {
    let (bytes, mut request) = super::super::tests::tadpole();
    request["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    request["max_refinements"] = json!(0);
    // This old mode-off request has no boundary configuration at all.
    let (mut old, old_bytes) = super::super::run::<1>(&bytes, &request, true).unwrap();
    let (mut repeated, repeated_bytes) = super::super::run::<1>(&bytes, &request, true).unwrap();
    old.as_object_mut().unwrap().remove("seconds");
    repeated.as_object_mut().unwrap().remove("seconds");
    assert_eq!(old, repeated);
    assert_eq!(old_bytes, repeated_bytes);
    // Serde omission and explicit false preserve the boundary config semantics.
    let mut cfg = primitive_config();
    cfg.primitive_original_weights = false;
    let c = context(1);
    let span = ordinary_frame(
        &c,
        vec![Row::from([
            (shift(0), c.one()),
            (shift(1), c.index(0).unwrap()),
        ])],
        vec![],
    );
    let a = project(
        &c,
        &span.images,
        &shift(0),
        &BTreeSet::new(),
        &[],
        Default::default(),
        limits(),
        &cfg,
    )
    .err()
    .unwrap()
    .to_string();
    let b = project_original(
        &c,
        &span,
        &shift(0),
        &BTreeSet::new(),
        Default::default(),
        limits(),
        &cfg,
    )
    .err()
    .unwrap()
    .to_string();
    assert_eq!(a, b);
    assert!(
        diagnostic(&"x".repeat(5000))["truncated"]
            .as_bool()
            .unwrap()
    );
}

fn phi2(c: &IndexedCoefficientContext) -> IndexedCoefficient {
    let n = c.index(0).unwrap();
    c.mul_with_limits(
        &n,
        &c.add_with_limits(&n, &c.one(), limits().arithmetic)
            .unwrap(),
        limits().arithmetic,
    )
    .unwrap()
}

fn proposal(result: Projection) -> Proposal {
    match result {
        Projection::Target(value) => value,
        _ => panic!("expected target"),
    }
}

#[test]
fn two_activation_faces_are_not_a_whole_column_ban() {
    let c = context(1);
    let coefficient = phi2(&c);
    let rows = vec![Row::from([
        (shift(0), c.one()),
        (shift(2), coefficient.clone()),
    ])];
    let faces = proposal(
        project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::new(),
            &[],
            Default::default(),
            limits(),
            &config(Mode::ActivationFaces),
        )
        .unwrap(),
    );
    assert_eq!(faces.image[&shift(2)], coefficient);
    for n in [0, -1] {
        assert!(
            c.specialize_fixed_indices(&coefficient, &[(0, n)], Default::default())
                .unwrap()
                .0
                .is_zero()
        );
    }
    assert!(
        !c.specialize_fixed_indices(&coefficient, &[(0, -2)], Default::default())
            .unwrap()
            .0
            .is_zero()
    );
    assert!(matches!(
        project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::new(),
            &[],
            Default::default(),
            limits(),
            &config(Mode::WholeColumns)
        )
        .unwrap(),
        Projection::NoTarget { .. }
    ));
}

#[test]
fn constraints_apply_to_the_collected_endpoint_not_each_source() {
    let c = context(1);
    let minus_one = c.neg_with_limits(&c.one(), limits().arithmetic).unwrap();
    let n_minus_one = c
        .add_with_limits(&c.index(0).unwrap(), &minus_one, limits().arithmetic)
        .unwrap();
    let rows = vec![
        Row::from([(shift(0), c.one()), (shift(1), c.one())]),
        Row::from([(shift(1), n_minus_one)]),
    ];
    let found = proposal(
        project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::new(),
            &[],
            Default::default(),
            limits(),
            &config(Mode::ActivationFaces),
        )
        .unwrap(),
    );
    assert_eq!(found.weights.len(), 2);
    assert_eq!(found.image[&shift(1)], c.index(0).unwrap());
    let mut guards = found.guards.clone();
    assert_eq!(
        project::replay(&c, &rows, &found.weights, &mut guards, limits()).unwrap(),
        found.image
    );
    assert!(matches!(
        project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::new(),
            &[],
            Default::default(),
            limits(),
            &config(Mode::WholeColumns)
        )
        .unwrap(),
        Projection::NoTarget { .. }
    ));
}

#[test]
fn target_pivot_cannot_cancel_the_required_boundary_factors() {
    let c = context(1);
    let phi = phi2(&c);
    let rows = vec![Row::from([(shift(0), phi.clone()), (shift(2), phi)])];
    let error = project(
        &c,
        &rows,
        &shift(0),
        &BTreeSet::new(),
        &[],
        Default::default(),
        limits(),
        &config(Mode::ActivationFaces),
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("normalized endpoint"), "{error}");
}

#[test]
fn source_poles_and_wholly_excluded_faces_are_not_boundary_proofs() {
    let c = context(1);
    let pole = c
        .div_with_limits(&c.one(), &c.index(0).unwrap(), limits().arithmetic)
        .unwrap();
    let rows = vec![Row::from([(shift(0), c.one()), (shift(1), pole)])];
    let error = project(
        &c,
        &rows,
        &shift(0),
        &BTreeSet::new(),
        &[],
        Default::default(),
        limits(),
        &config(Mode::ActivationFaces),
    )
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("index-dependent source denominators")
    );
    let rows = vec![Row::from([(shift(0), c.one()), (shift(2), phi2(&c))])];
    let guard = Guard {
        polynomial: c
            .numerator_condition_with_limits(&c.index(0).unwrap(), limits().arithmetic)
            .unwrap(),
        origin: "original source assumption".into(),
    };
    let error = project(
        &c,
        &rows,
        &shift(0),
        &BTreeSet::new(),
        &[guard],
        Default::default(),
        limits(),
        &config(Mode::ActivationFaces),
    )
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("excludes an entire activation face")
    );
}

#[test]
fn two_index_grouping_and_face_intersections_use_exact_native_restriction() {
    let c = context(2);
    let mut cfg = config(Mode::ActivationFaces);
    cfg.polynomial_axes = vec![0, 1];
    cfg.protected_axes = vec![0, 1];
    cfg.weight_monomials = vec![vec![0, 0], vec![1, 0], vec![0, 1]];
    cfg.validate(2).unwrap();
    let product = c
        .mul_with_limits(
            &c.index(0).unwrap(),
            &c.index(1).unwrap(),
            limits().arithmetic,
        )
        .unwrap();
    let groups = split(
        &c,
        &product,
        &cfg,
        limits(),
        &mut Admission::default(),
        &mut 0,
    )
    .unwrap();
    assert_eq!(groups.len(), 1);
    assert!(groups.values().next().unwrap().raw().is_one());
    for fixed in [vec![(0, 0)], vec![(1, 0)], vec![(0, 0), (1, 0)]] {
        assert!(
            c.specialize_fixed_indices(&product, &fixed, Default::default())
                .unwrap()
                .0
                .is_zero()
        );
    }
    // No claim that (n0) and (n1) are comaximal, and no quotient/ideal routine.
    cfg.polynomial_axes = vec![0];
    cfg.protected_axes = vec![0];
    cfg.weight_monomials = vec![vec![0]];
    assert!(
        split(
            &c,
            &product,
            &cfg,
            limits(),
            &mut Admission::default(),
            &mut 0
        )
        .is_err()
    );
}

#[test]
fn declared_polynomial_weights_stay_over_the_base_field() {
    let c = context(1);
    let mut cfg = config(Mode::ActivationFaces);
    cfg.weight_monomials = vec![vec![0], vec![1]];
    let rows = vec![Row::from([
        (shift(0), c.one()),
        (shift(-1), c.index(0).unwrap()),
    ])];
    let found = proposal(
        project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::new(),
            &[],
            Default::default(),
            limits(),
            &cfg,
        )
        .unwrap(),
    );
    assert!(found.image[&shift(0)].raw().is_one());
    assert!(scalar(&c, &c.index(0).unwrap(), limits()).is_err());
    assert!(
        project(
            &c,
            &rows,
            &shift(-1),
            &BTreeSet::new(),
            &[],
            Default::default(),
            limits(),
            &cfg,
        )
        .err()
        .unwrap()
        .to_string()
        .contains("zero target shift")
    );
    cfg.max_unknowns = 1;
    assert!(
        project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::new(),
            &[],
            Default::default(),
            limits(),
            &cfg
        )
        .is_err()
    );
}

#[test]
fn bounded_degree_one_weights_find_a_target_missed_by_constants() {
    let c = context(1);
    let rows = vec![
        Row::from([
            (shift(0), c.one()),
            (shift(-1), c.index(0).unwrap()),
            (shift(2), phi2(&c)),
        ]),
        Row::from([(shift(-1), c.one())]),
    ];
    let forbidden = BTreeSet::from([shift(-1)]);
    let mut cfg = config(Mode::ActivationFaces);
    assert!(matches!(
        project(
            &c,
            &rows,
            &shift(0),
            &forbidden,
            &[],
            Default::default(),
            limits(),
            &cfg
        )
        .unwrap(),
        Projection::NoTarget { .. }
    ));
    cfg.weight_monomials = vec![vec![0], vec![1]];
    let found = proposal(
        project(
            &c,
            &rows,
            &shift(0),
            &forbidden,
            &[],
            Default::default(),
            limits(),
            &cfg,
        )
        .unwrap(),
    );
    assert_eq!(found.weights[&0], c.one());
    assert_eq!(
        found.weights[&1],
        c.neg_with_limits(&c.index(0).unwrap(), limits().arithmetic)
            .unwrap()
    );
    assert!(!found.image.contains_key(&shift(-1)));
    assert_eq!(found.image[&shift(2)], phi2(&c));
}

#[test]
fn config_is_explicit_generic_bounded_and_off_by_default() {
    assert!(super::config(&json!({})).unwrap().is_none());
    let mut request = json!({"owner_mask":"00","chart":{"lower":[0,0],"upper":[null,null],"fixed":[]},
        "max_refinements":0,"boundary_polynomial":{"mode":"activation-faces","polynomial_axes":[0,1],"protected_axes":[0,1],
        "weight_monomials":[[0,0],[1,0],[0,1]],"max_total_degree":1,"max_unknowns":144,"max_constraints":8192,"max_nonzeros":100000,"max_activation_faces":64}});
    assert!(
        !super::config(&request)
            .unwrap()
            .unwrap()
            .primitive_original_weights
    );
    request["boundary_polynomial"]["primitive_original_weights"] = json!(false);
    assert!(
        !super::config(&request)
            .unwrap()
            .unwrap()
            .primitive_original_weights
    );
    request["boundary_polynomial"]["primitive_original_weights"] = json!("true");
    assert!(super::config(&request).is_err());
    request["boundary_polynomial"]["primitive_original_weights"] = json!(true);
    assert!(
        super::config(&request)
            .unwrap()
            .unwrap()
            .primitive_original_weights
    );
    for invalid in [json!([0, 0]), json!([2]), json!([])] {
        request["boundary_polynomial"]["protected_axes"] = invalid;
        assert!(super::config(&request).is_err());
    }
    request["boundary_polynomial"]["protected_axes"] = json!([0, 1]);
    request["boundary_polynomial"]["weight_monomials"] = json!([[2, 0]]);
    assert!(super::config(&request).is_err());
    request["boundary_polynomial"]["weight_monomials"] = json!([[0, 0]]);
    request["max_refinements"] = json!(1);
    assert!(super::config(&request).is_err());
}

#[test]
fn foreign_context_is_rejected_before_coefficient_grouping() {
    let c = context(1);
    let foreign = IndexedCoefficientContext::try_new(
        &CoefficientContext::try_new(["d"]).unwrap(),
        "foreign-boundary-context",
        1,
    )
    .unwrap();
    let cfg = config(Mode::ActivationFaces);
    assert!(
        split(
            &c,
            &foreign.index(0).unwrap(),
            &cfg,
            limits(),
            &mut Admission::default(),
            &mut 0
        )
        .is_err()
    );
    assert!(scalar(&c, &foreign.one(), limits()).is_err());
}

#[test]
fn entry_label_face_and_operation_caps_fail_before_unbounded_work() {
    let c = context(1);
    let rows = vec![Row::from([(shift(0), c.one()), (shift(2), phi2(&c))])];
    let mut cfg = config(Mode::WholeColumns);
    cfg.max_constraints = 1;
    assert!(
        project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::new(),
            &[],
            Default::default(),
            limits(),
            &cfg
        )
        .is_err()
    );
    cfg = config(Mode::ActivationFaces);
    cfg.max_nonzeros = 1; // The identity entry already consumes this allowance.
    assert!(
        project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::new(),
            &[],
            Default::default(),
            limits(),
            &cfg
        )
        .is_err()
    );
    cfg = config(Mode::ActivationFaces);
    cfg.max_activation_faces = 1;
    assert!(
        project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::new(),
            &[],
            Default::default(),
            limits(),
            &cfg
        )
        .is_err()
    );

    let mut labels = BTreeSet::new();
    let mut count = 0;
    cfg.max_constraints = 1;
    register_label(0usize, &mut labels, &mut count, &cfg).unwrap();
    assert!(register_label(1usize, &mut labels, &mut count, &cfg).is_err());
    assert_eq!(labels, BTreeSet::from([0]));

    let image = rows[0].clone();
    cfg = config(Mode::ActivationFaces);
    let mut tight = limits();
    tight.operations = 2;
    let polynomial = c
        .numerator_condition_with_limits(
            &c.add_with_limits(&phi2(&c), &c.one(), tight.arithmetic)
                .unwrap(),
            tight.arithmetic,
        )
        .unwrap();
    let guard = Guard {
        polynomial,
        origin: "nonvanishing on both faces".into(),
    };
    let mut guards = vec![guard.clone(), guard];
    let error = verify_faces(
        &c,
        &image,
        &cfg,
        Default::default(),
        &mut guards,
        tight,
        &mut 0,
    )
    .err()
    .unwrap();
    assert!(
        error.to_string().contains("aggregate operations"),
        "{error}"
    );

    let mut admission = Admission::default();
    let mut operations = 0;
    tight.operations = 1;
    assert!(split(&c, &phi2(&c), &cfg, tight, &mut admission, &mut operations).is_err());
    assert_eq!(admission.entries, 0);
}
