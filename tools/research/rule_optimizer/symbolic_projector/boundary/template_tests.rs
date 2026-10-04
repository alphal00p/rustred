use super::*;

fn template(numerator: &[(i64, u16, Vec<u16>)], arity: usize) -> TargetTemplate {
    TargetTemplate {
        base_parameters: vec!["d".into()],
        numerator: numerator
            .iter()
            .map(|(integer, d, indices)| TemplateTerm {
                integer: *integer,
                base_powers: vec![*d],
                index_powers: indices.clone(),
            })
            .collect(),
        denominator: vec![TemplateTerm {
            integer: 1,
            base_powers: vec![0],
            index_powers: vec![0; arity],
        }],
    }
}

fn compiled(
    c: &IndexedCoefficientContext,
    spec: &TargetTemplate,
    cfg: &Config,
) -> CompiledTemplate {
    compile_template(
        c,
        spec,
        cfg,
        &mut Admission::default(),
        &mut 0,
        &mut vec![],
        limits(),
    )
    .unwrap()
}

#[test]
fn target_template_skips_false_pivot_multiple_without_extra_unknowns() {
    let c = context(1);
    let n = c.index(0).unwrap();
    let rows = vec![
        Row::from([(shift(0), n.clone()), (shift(1), n.clone())]),
        Row::from([(shift(0), c.one()), (shift(1), n)]),
    ];
    let mut cfg = config(Mode::ActivationFaces);
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
    cfg.target_template = Some(template(&[(1, 0, vec![0])], 1));
    cfg.max_unknowns = 2; // exactly two ORIGINAL weight unknowns
    let p = proposal(
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
    assert_eq!(p.weights, Weights::from([(1, c.one())]));
    assert_eq!(p.prefix_rows, 2);
}

#[test]
fn target_template_union_includes_missing_monomials_in_both_directions() {
    let c = context(1);
    let n = c.index(0).unwrap();
    let one_plus_n = c
        .add_with_limits(&c.one(), &n, limits().arithmetic)
        .unwrap();
    let mut cfg = config(Mode::ActivationFaces);
    for (target, spec) in [
        (c.one(), template(&[(1, 0, vec![0]), (1, 0, vec![1])], 1)),
        (one_plus_n, template(&[(1, 0, vec![0])], 1)),
    ] {
        cfg.target_template = Some(spec);
        assert!(matches!(
            project(
                &c,
                &[Row::from([(shift(0), target)])],
                &shift(0),
                &BTreeSet::new(),
                &[],
                Default::default(),
                limits(),
                &cfg
            )
            .unwrap(),
            Projection::NoTarget { .. }
        ));
    }
}

#[test]
fn target_template_allows_index_dependent_shape_and_verifies_full_replay() {
    let c = context(1);
    let spec = template(&[(1, 1, vec![0]), (-1, 0, vec![1])], 1);
    let mut cfg = config(Mode::ActivationFaces);
    let t = compiled(&c, &spec, &cfg);
    let endpoint = c
        .mul_with_limits(&t.value, &c.index(0).unwrap(), limits().arithmetic)
        .unwrap();
    let rows = vec![Row::from([
        (shift(0), t.value.clone()),
        (shift(1), endpoint),
    ])];
    cfg.target_template = Some(template(&[(1, 0, vec![0])], 1));
    assert!(matches!(
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
        .unwrap(),
        Projection::NoTarget { .. }
    ));
    cfg.target_template = Some(spec);
    let p = proposal(
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
    assert_eq!(p.image[&shift(1)], c.index(0).unwrap());
    // Same canonical coefficient but a different other coefficient must fail.
    let false_target = c
        .add_with_limits(&t.value, &c.index(0).unwrap(), limits().arithmetic)
        .unwrap();
    assert!(
        verify_template(
            &c,
            &false_target,
            &t,
            &cfg,
            &mut Admission::default(),
            &mut 0,
            &mut vec![],
            limits()
        )
        .is_err()
    );
}

#[test]
fn target_template_rejects_zero_foreign_maps_and_index_denominators() {
    let c = context(1);
    let cfg = config(Mode::ActivationFaces);
    let mut foreign = template(&[(1, 0, vec![0])], 1);
    foreign.base_parameters[0] = "foreign".into();
    let mut wrong_arity = template(&[(1, 0, vec![0, 0])], 1);
    let mut pole = template(&[(1, 0, vec![0])], 1);
    pole.denominator[0].index_powers[0] = 1;
    let zero = template(&[(1, 0, vec![0]), (-1, 0, vec![0])], 1);
    let mut zero_den = template(&[(1, 0, vec![0])], 1);
    zero_den.denominator[0].integer = 0;
    for spec in [foreign, wrong_arity.clone(), pole, zero, zero_den] {
        assert!(
            compile_template(
                &c,
                &spec,
                &cfg,
                &mut Admission::default(),
                &mut 0,
                &mut vec![],
                limits()
            )
            .is_err()
        );
    }
    wrong_arity.numerator[0].index_powers = vec![0];
    let foreign_context = IndexedCoefficientContext::try_new(
        &CoefficientContext::try_new(["d"]).unwrap(),
        "other-template",
        1,
    )
    .unwrap();
    let mut t = compiled(&c, &wrong_arity, &cfg);
    t.value = foreign_context.one();
    assert!(
        verify_template(
            &c,
            &c.one(),
            &t,
            &cfg,
            &mut Admission::default(),
            &mut 0,
            &mut vec![],
            limits()
        )
        .is_err()
    );
}

#[test]
fn target_template_retains_raw_cancelled_denominator_and_genuine_source_pole() {
    let c = context(1);
    let mut cfg = config(Mode::ActivationFaces);
    let mut spec = template(&[(1, 1, vec![0])], 1);
    spec.denominator[0].base_powers[0] = 1; // d/d, raw d must survive
    let mut guards = vec![];
    let t = compile_template(
        &c,
        &spec,
        &cfg,
        &mut Admission::default(),
        &mut 0,
        &mut guards,
        limits(),
    )
    .unwrap();
    assert_eq!(t.value, c.one());
    assert!(
        guards
            .iter()
            .any(|g| g.origin == "target template raw denominator"
                && !g.polynomial.is_nonzero_constant())
    );
    cfg.target_template = Some(spec);
    let n = c.index(0).unwrap();
    guards.push(Guard {
        polynomial: c
            .numerator_condition_with_limits(&n, limits().arithmetic)
            .unwrap(),
        origin: "genuine source pole".into(),
    });
    let error = project(
        &c,
        &[Row::from([(shift(0), c.one()), (shift(1), n)])],
        &shift(0),
        &BTreeSet::new(),
        &guards,
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
fn target_template_is_rechecked_after_primitive_original_division() {
    let c = context(1);
    let mut cfg = primitive_config();
    cfg.target_template = Some(template(&[(1, 0, vec![1])], 1));
    let span = ordinary_frame(
        &c,
        vec![Row::from([
            (shift(0), c.one()),
            (shift(1), c.index(0).unwrap()),
        ])],
        vec![],
    );
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
            .contains("replayed template pivot is zero")
    );
}

#[test]
fn target_template_preflights_complete_union_and_cumulative_limits() {
    let c = context(1);
    let mut cfg = config(Mode::ActivationFaces);
    let spec = template(&[(1, 0, vec![0]), (1, 0, vec![2])], 1);
    cfg.target_template = Some(spec.clone());
    let rows = vec![Row::from([(shift(0), c.one())])];
    cfg.max_constraints = 2; // 1 source target + 1 template-only + 1 constraint
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
        .err()
        .unwrap()
        .to_string()
        .contains("complete template union")
    );
    cfg.max_constraints = 8192;
    cfg.max_nonzeros = 2;
    assert!(
        compile_template(
            &c,
            &spec,
            &cfg,
            &mut Admission::default(),
            &mut 0,
            &mut vec![],
            limits()
        )
        .is_err()
    );
    cfg.max_nonzeros = 100000;
    for (ops, terms) in [(0, 1000000), (1000000, 1)] {
        let mut limited = limits();
        limited.operations = ops;
        limited.coefficient_terms = terms;
        assert!(
            compile_template(
                &c,
                &spec,
                &cfg,
                &mut Admission::default(),
                &mut 0,
                &mut vec![],
                limited
            )
            .is_err()
        );
    }
    let mut charged = limits().operations;
    assert!(
        compile_template(
            &c,
            &spec,
            &cfg,
            &mut Admission::default(),
            &mut charged,
            &mut vec![],
            limits()
        )
        .is_err()
    );
    let t = compiled(&c, &template(&[(1, 0, vec![0])], 1), &cfg);
    let mut after_reducer = Admission {
        entries: 0,
        terms: limits().coefficient_terms - 1,
    };
    assert!(
        verify_template(
            &c,
            &c.one(),
            &t,
            &cfg,
            &mut after_reducer,
            &mut 0,
            &mut vec![],
            limits()
        )
        .err()
        .unwrap()
        .to_string()
        .contains("assembled coefficient terms")
    );
}

#[test]
fn target_template_absent_null_parse_and_mode_off_replay_is_deterministic() {
    let (bytes, mut request) = super::super::super::tests::tadpole();
    request["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    request["max_refinements"] = json!(0);
    let (mut before, before_bytes) = super::super::super::run::<1>(&bytes, &request, true).unwrap();
    let (mut after, after_bytes) = super::super::super::run::<1>(&bytes, &request, true).unwrap();
    before.as_object_mut().unwrap().remove("seconds");
    after.as_object_mut().unwrap().remove("seconds");
    assert_eq!(before, after);
    assert_eq!(before_bytes, after_bytes);
    let mut request = json!({"owner_mask":"0","chart":{"lower":[0],"upper":[null],"fixed":[]},
        "max_refinements":0,"boundary_polynomial":{"mode":"activation-faces","polynomial_axes":[0],"protected_axes":[0],
        "weight_monomials":[[0]],"max_total_degree":1,"max_unknowns":1,"max_constraints":8192,"max_nonzeros":100000,"max_activation_faces":64}});
    assert!(
        super::super::config(&request)
            .unwrap()
            .unwrap()
            .target_template
            .is_none()
    );
    request["boundary_polynomial"]["target_template"] = Value::Null;
    assert!(
        super::super::config(&request)
            .unwrap()
            .unwrap()
            .target_template
            .is_none()
    );
}

#[test]
fn primitive_membership_uses_full_native_base_plus_index_map() {
    let c = context(2);
    let n0 = c.index(0).unwrap();
    let n1 = c.index(1).unwrap();
    let product = c.mul_with_limits(&n0, &n1, limits().arithmetic).unwrap();
    let original = Weights::from([(0, n0.clone()), (1, product)]);
    let mut cfg = config(Mode::ActivationFaces);
    cfg.polynomial_axes = vec![0, 1];
    cfg.protected_axes = vec![0, 1];
    cfg.max_total_degree = 2;
    cfg.weight_monomials = vec![vec![0, 0], vec![1, 0], vec![1, 1]]; // NOT [0,1]
    assert!(
        primitive_weights(
            &c,
            &mut original.clone(),
            &cfg,
            &mut Admission::default(),
            &mut 0,
            limits()
        )
        .err()
        .unwrap()
        .to_string()
        .contains("escaped the declared monomial ansatz")
    );
    cfg.weight_monomials.push(vec![0, 1]);
    let mut valid = original;
    assert_eq!(
        primitive_weights(
            &c,
            &mut valid,
            &cfg,
            &mut Admission::default(),
            &mut 0,
            limits()
        )
        .unwrap(),
        Some(n0)
    );
    assert_eq!(valid, Weights::from([(0, c.one()), (1, n1)]));
}
