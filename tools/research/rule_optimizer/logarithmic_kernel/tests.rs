use super::*;
use rustred::family::AffineDenominator;

fn request() -> Value {
    let mut r = json!({"schema":SCHEMA,"target":[2],"owner_mask":"1","family_fingerprint":"fixture",
        "expected_order":"rustred.spired-uncut-sector-order.v1","protected_axes":[0],"recenter":[0],
        "max_weight_degree":1,"compute_parametric_image_rank":true,"limits":{},
        "owner_load_limits":{"max_bundle_bytes":1073741824,"max_total_input_bytes":2147483648_u64,
        "max_total_coefficient_bytes":2147483648_u64,"max_collection_entries":32000000,
        "max_total_symbolica_state_bytes":16777216,"max_zero_sector_visits":1000000,"max_coefficient_bytes":16777216}});
    for name in [
        "max_owner_bytes",
        "max_denominators",
        "max_ordinary_rows",
        "max_unknowns",
        "max_constraint_columns",
        "max_augmented_columns",
        "max_input_nonzeros",
        "max_reducer_nonzeros",
        "max_kernel_vectors",
        "max_polynomial_operations",
        "max_polynomial_terms",
        "max_coordinate_cells",
        "max_translated_terms",
        "max_conditions",
        "max_product_terms",
        "max_exact_operations",
        "max_physical_columns",
        "max_coefficient_terms",
        "max_report_bytes",
    ] {
        r["limits"][name] = json!(10_000_000);
    }
    r
}

#[test]
fn explicit_owner_ingress_policy_propagates_without_touching_algebra_limits() {
    let mut r = request();
    let names = [
        "max_bundle_bytes",
        "max_total_input_bytes",
        "max_total_coefficient_bytes",
        "max_collection_entries",
        "max_total_symbolica_state_bytes",
        "max_zero_sector_visits",
        "max_coefficient_bytes",
    ];
    for (i, name) in names.iter().enumerate() {
        r["owner_load_limits"][name] = json!(1000 + i);
    }
    let p = owner_load_limits(&r).unwrap();
    assert_eq!(
        [
            p.bundle.max_bundle_bytes,
            p.max_total_input_bytes,
            p.bundle.max_total_coefficient_bytes,
            p.bundle.max_collection_entries,
            p.max_total_symbolica_state_bytes,
            p.max_zero_sector_visits,
            p.bundle.max_coefficient_bytes
        ],
        [1000, 1001, 1002, 1003, 1004, 1005, 1006]
    );
    assert_eq!(
        p.bundle.exact_algebra,
        CandidateOwnerLoadLimits::default().bundle.exact_algebra
    );
    r["owner_load_limits"]["max_bundle_bytes"] = json!(0);
    assert!(owner_load_limits(&r).is_err());
    r["owner_load_limits"]["max_bundle_bytes"] = json!(rustred_app::MAX_CANDIDATE_BUNDLE_BYTES + 1);
    assert!(owner_load_limits(&r).is_err());
    r["owner_load_limits"]["max_bundle_bytes"] = json!(1000);
    r["owner_load_limits"]["invented"] = json!(1);
    assert!(owner_load_limits(&r).is_err());
}

#[test]
fn explicit_degree_and_protected_scope_cannot_silently_expand() {
    assert_eq!(validate(&request()).unwrap(), 1);
    for (field, value) in [
        ("max_weight_degree", json!(2)),
        ("protected_axes", json!([])),
        ("protected_axes", json!([0, 0])),
        ("recenter", json!([])),
        ("owner_mask", json!("0")),
        ("compute_parametric_image_rank", Value::Null),
    ] {
        let mut r = request();
        r[field] = value;
        assert!(validate(&r).is_err(), "{field}");
    }
}

#[test]
fn selective_protection_accepts_only_sorted_unique_nonempty_active_subsets() {
    let mut r = request();
    r["target"] = json!([2, 0, 1]);
    r["owner_mask"] = json!("101");
    r["recenter"] = json!([0, 0, 0]);
    for axes in [json!([0]), json!([2]), json!([0, 2])] {
        r["protected_axes"] = axes;
        assert_eq!(validate(&r).unwrap(), 3);
        let scope = protection_scope(&r);
        assert_eq!(scope["active_axes"], json!([0, 2]));
        assert_eq!(scope["protected_axes"], r["protected_axes"]);
        assert_eq!(
            scope["all_active_axes_protected"],
            r["protected_axes"] == json!([0, 2])
        );
        assert_eq!(scope["kernel_vector_subset_selection"], false);
    }
    for axes in [
        json!([]),
        json!([0, 0]),
        json!([2, 0]),
        json!([1]),
        json!([0, 1, 2]),
        json!([3]),
        json!([-1]),
    ] {
        r["protected_axes"] = axes;
        assert!(validate(&r).is_err(), "{}", r["protected_axes"]);
    }
}

#[test]
fn native_augmented_kernel_is_replayed_against_every_original_column() {
    let c = CoefficientContext::try_new(["d"]).unwrap();
    let r = request();
    let mut a = Matrix::new(0, 2, field());
    a.add_row(vec![c.one()], vec![0]);
    a.add_row(vec![c.integer(2)], vec![0]);
    a.add_row(vec![c.one()], vec![1]);
    let kernel = linear::left_kernel(&a, &c.one(), &r).unwrap();
    assert_eq!(kernel.report["constraint_rank"], 2);
    assert_eq!(kernel.weights.nrows(), 1);
    linear::certify_kernel(&kernel.weights, &a, &r).unwrap();
    let mut changed = Matrix::new(0, 2, field());
    changed.add_row(vec![c.one()], vec![0]);
    changed.add_row(vec![c.integer(3)], vec![0]);
    changed.add_row(vec![c.one()], vec![1]);
    assert!(linear::certify_kernel(&kernel.weights, &changed, &r).is_err());
    let mut capped = r.clone();
    capped["limits"]["max_augmented_columns"] = json!(4);
    assert!(linear::left_kernel(&a, &c.one(), &capped).is_err());
}

#[test]
fn point_solver_cancels_all_nonlower_columns_and_full_replay_rejects_mutants() {
    let c = CoefficientContext::try_new(["d"]).unwrap();
    let r = request();
    let target = Integral::numeric([2]).unwrap();
    let order = IntegralOrder::new([true], [false]);
    let columns = [
        Integral::numeric([3]).unwrap(),
        target,
        Integral::numeric([1]).unwrap(),
    ];
    let mut frame = Matrix::new(0, 3, field());
    frame.add_row(vec![c.one(), c.one()], vec![0, 1]);
    frame.add_row(vec![c.one(), c.integer(-1)], vec![0, 2]);
    let solved = linear::target_pivot(&frame, &columns, &target, &order, &c.one(), &r).unwrap();
    let weights = solved.weights.unwrap();
    let replay = linear::product(&weights, &frame, &r).unwrap();
    assert_eq!(
        linear::full_tail(&replay, &columns, &target, &order)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut mutant = Matrix::new(0, 3, field());
    mutant.add_row(vec![c.one(), c.one()], vec![0, 1]);
    assert!(linear::full_tail(&mutant, &columns, &target, &order).is_err());
    let mut lost = Matrix::new(0, 3, field());
    lost.add_row(vec![c.one()], vec![2]);
    assert!(linear::full_tail(&lost, &columns, &target, &order).is_err());
}

fn two_loop() -> IntegralFamily {
    let c = CoefficientContext::try_new(["d"]).unwrap();
    IntegralFamily::new(
        "joint-logarithmic-2L-toy",
        vec!["k".into(), "q".into()],
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        [[1, 0, 0], [0, 1, 0], [0, 0, 1]]
            .into_iter()
            .map(|a| {
                AffineDenominator::new(c.zero(), a.into_iter().map(|v| c.integer(v)).collect())
            })
            .collect(),
        vec![],
        vec![c.zero(); 3],
    )
    .unwrap()
}

#[test]
fn denominator_permutation_preserves_generic_constraint_kernel_dimension() {
    let family = two_loop();
    let permutation = [2, 0, 1];
    let permuted = IntegralFamily::new(
        "joint-logarithmic-permuted-2L-toy",
        family.loop_momenta().to_vec(),
        vec![],
        family.coefficient_context().clone(),
        family.dimension().clone(),
        permutation
            .iter()
            .map(|&i| family.denominators()[i].clone())
            .collect(),
        vec![],
        vec![family.coefficient_context().zero(); 3],
    )
    .unwrap();
    let mut dimensions = Vec::new();
    for f in [&family, &permuted] {
        let g = ParametricIbpGenerator::try_new(f).unwrap();
        let p = g.prepare_ordinary_ibp().unwrap();
        let count = p.len();
        let generated = (0..count).map(|i| p.generate(i)).collect();
        let c = p.complete(generated).unwrap();
        let inv = g
            .translate_selected_completed_source_rows(
                &c,
                (0..count).map(|i| {
                    TranslatedSourceRequest::new(i, IntegralShift::try_new([0, 0, 0]).unwrap())
                }),
                Default::default(),
            )
            .unwrap();
        let ids = inv
            .sources()
            .iter()
            .map(|s| (s.row_id().clone(), s.provenance().source_ordinal()))
            .collect();
        let mut r = request();
        r["target"] = json!([1, 1, 1]);
        r["owner_mask"] = json!("111");
        r["protected_axes"] = json!([0, 1, 2]);
        r["recenter"] = json!([0, 0, 0]);
        let pre = polynomial::preflight(f, &r).unwrap();
        let system = polynomial::assemble(f, &ids, &pre, &r).unwrap();
        let kernel =
            linear::left_kernel(&system.matrix, &f.coefficient_context().one(), &r).unwrap();
        dimensions.push(kernel.weights.nrows());
    }
    assert!(dimensions[0] > 0);
    assert_eq!(dimensions[0], dimensions[1]);
}

#[test]
fn native_joint_rotation_has_zero_full_parametric_image_not_a_useful_rule() {
    // D=(k.k,k.q,q.q). V_k=D2*k-D1*q; V_q=D3*k-D2*q.
    // Native assembly and original source replay establish the identity; the
    // test does not derive derivative coefficients or imitate elimination.
    let f = two_loop();
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let p = g.prepare_ordinary_ibp().unwrap();
    let count = p.len();
    let generated = (0..count).map(|i| p.generate(i)).collect();
    let c = p.complete(generated).unwrap();
    let inv = g
        .translate_selected_completed_source_rows(
            &c,
            (0..count).map(|i| {
                TranslatedSourceRequest::new(i, IntegralShift::try_new([0, 0, 0]).unwrap())
            }),
            Default::default(),
        )
        .unwrap();
    let ids = inv
        .sources()
        .iter()
        .map(|s| (s.row_id().clone(), s.provenance().source_ordinal()))
        .collect();
    let mut r = request();
    r["target"] = json!([1, 1, 1]);
    r["owner_mask"] = json!("111");
    r["protected_axes"] = json!([0, 1, 2]);
    r["recenter"] = json!([0, 0, 0]);
    let pre = polynomial::preflight(&f, &r).unwrap();
    let system = polynomial::assemble(&f, &ids, &pre, &r).unwrap();
    // Resolve q labels from the actual native family direction inventory.
    use rustred::family::ContractionMomentum;
    let k = f
        .contraction_momenta()
        .iter()
        .position(|q| *q == ContractionMomentum::Loop(0))
        .unwrap();
    let q = f
        .contraction_momenta()
        .iter()
        .position(|q| *q == ContractionMomentum::Loop(1))
        .unwrap();
    let wanted = [
        (k, 0, [0, 1, 0], 1),
        (q, 0, [1, 0, 0], -1),
        (k, 1, [0, 0, 1], 1),
        (q, 1, [0, 1, 0], -1),
    ];
    let mut chosen = BTreeMap::new();
    for (q, l, alpha, value) in wanted {
        let at = system
            .unknowns
            .iter()
            .position(|u| {
                u.row_id
                    == RowId::OrdinaryIbp {
                        contraction_momentum: q,
                        differentiated_loop: l,
                    }
                    && u.alpha == alpha
            })
            .unwrap();
        chosen.insert(at as u32, f.coefficient_context().integer(value));
    }
    let mut weights = Matrix::new(0, system.unknowns.len() as u32, field());
    weights.add_row(
        chosen.values().cloned().collect(),
        chosen.keys().copied().collect(),
    );
    linear::certify_kernel(&weights, &system.matrix, &r).unwrap();
    let requests = system
        .unknowns
        .iter()
        .map(|u| {
            TranslatedSourceRequest::new(
                u.ordinal,
                IntegralShift::try_new(u.alpha.iter().map(|&a| -i64::from(a))).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let batch = g
        .translate_selected_completed_source_rows(&c, requests.clone(), Default::default())
        .unwrap();
    let source_indices = requests
        .iter()
        .map(|q| {
            batch
                .sources()
                .iter()
                .position(|s| {
                    s.provenance().source_ordinal() == q.source_ordinal()
                        && s.provenance().offset() == q.offset()
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    let images = sources::images(&f, &g, &batch, &source_indices, &weights, &r).unwrap();
    assert_eq!(images.report["true_parametric_zero_images"], 1);
    assert!(images.rows[0].is_empty());
    assert_eq!(images.report["image_rank"]["rank"], 0);
    let shift = inv
        .sources()
        .iter()
        .flat_map(|s| s.terms().keys())
        .next()
        .unwrap();
    let context = g.context();
    let value = context
        .sub(&context.index(0).unwrap(), &context.integer(2))
        .unwrap();
    let mut point_only = BTreeMap::new();
    point_only.insert(shift.clone(), value);
    assert!(!point_only.is_empty());
    let mut guards = Vec::new();
    assert!(
        sources::specialize_row::<3>(
            &point_only,
            context,
            f.coefficient_context(),
            &[2, 1, 1],
            &mut guards,
            &r
        )
        .unwrap()
        .is_empty()
    );
    // A nonzero parametric image that vanishes at this point is not labeled
    // an identically-zero source image before specialization.
    assert!(!point_only.values().next().unwrap().is_zero());
}

#[test]
fn massive_tadpole_degree_one_has_target_and_degree_zero_does_not() {
    let source = r#"schema = "rustred.project.toml.v1"
[family]
name = "finite_logarithmic_k1"
loop_momenta = ["q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P"
expression = "q^2-1"
[target]
powers = [1]
"#;
    let bundle =
        rustred_app::family_candidates(rustred_app::FamilyCandidatesRequest::new(source)).unwrap();
    let mask = Mask::try_new([true]).unwrap();
    let (family, programs) = load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes: bundle.bundle(),
            owner_sector: &mask,
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let programs = Arc::new(programs);
    let bound = programs
        .bind_owner_search([true], Default::default())
        .unwrap();
    let mut r = request();
    r["family_fingerprint"] = json!(family.fingerprint());
    r["expected_order"] = json!(bound.owner_ordering().stable_id().to_string());
    let result = probe::<1>(bundle.bundle(), &r).unwrap();
    assert_eq!(result["coefficient_kernel_over_K"]["kernel_dimension"], 1);
    assert_eq!(
        result["point"]["result"]["status"],
        "EXACT_FINITE_LOGARITHMIC_SOURCE_COMBINATION"
    );
    assert_eq!(
        result["point"]["original_source_replay"]["all_original_columns_replayed"],
        true
    );
    r["compute_parametric_image_rank"] = json!(false);
    let skipped = probe::<1>(bundle.bundle(), &r).unwrap();
    assert_eq!(skipped["point"]["result"], result["point"]["result"]);
    assert_eq!(
        skipped["full_parametric_images"]["image_rank"]["rank"],
        Value::Null
    );
    r["max_weight_degree"] = json!(0);
    let zero = probe::<1>(bundle.bundle(), &r).unwrap();
    assert_eq!(zero["coefficient_kernel_over_K"]["kernel_dimension"], 0);
    assert_eq!(
        zero["point"]["result"]["status"],
        "NO_TARGET_PIVOT_IN_FIXED_DEGREE_KERNEL"
    );
    r["max_weight_degree"] = json!(1);
    r["target"] = json!([1]);
    let boundary = probe::<1>(bundle.bundle(), &r).unwrap();
    assert_eq!(
        boundary["point"]["result"]["status"],
        "NO_TARGET_PIVOT_IN_FIXED_DEGREE_KERNEL"
    );
    assert_eq!(boundary["zero_sector_terms_discarded"], false);
}
