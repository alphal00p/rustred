use super::*;

#[test]
fn family_name_retention_moves_owned_buffers_and_fallibly_copies_borrowed_names() {
    let owned = String::from("owned-family-name");
    let owned_pointer = owned.as_ptr();
    let retained_owned = retain_family_name(Cow::Owned(owned)).unwrap();
    assert_eq!(retained_owned.as_ptr(), owned_pointer);

    let borrowed = "borrowed-family-name";
    let retained_borrowed = retain_family_name(Cow::Borrowed(borrowed)).unwrap();
    assert_eq!(retained_borrowed, borrowed);
    assert_ne!(retained_borrowed.as_ptr(), borrowed.as_ptr());
}

#[test]
fn proportional_family_limits_and_identity_strings_precede_label_sets() {
    let context = CoefficientContext::new(["d"]);
    let scalar_limit_first = IntegralFamily::new_with_limits(
        "duplicate-loop-labels",
        vec!["k".into(), "k".into()],
        Vec::new(),
        context.clone(),
        context.one(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        IntegralFamilyLimits {
            max_scalar_products: 0,
            ..IntegralFamilyLimits::default()
        },
    );
    assert!(matches!(
        scalar_limit_first,
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family scalar products",
            requested: 3,
            limit: 0,
        })
    ));

    let oversized_borrowed_name = "borrowed-family-name";
    let name_limit_first = IntegralFamily::new_with_limits(
        oversized_borrowed_name,
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.one(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        IntegralFamilyLimits {
            max_fingerprint_bytes: 4,
            ..IntegralFamilyLimits::default()
        },
    );
    assert!(matches!(
        name_limit_first,
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family fingerprint bytes",
            requested,
            limit: 4,
        }) if requested == oversized_borrowed_name.len()
    ));

    let oversized_label = "loop-label-too-long";
    let label_limit_first = IntegralFamily::new_with_limits(
        "x",
        vec![oversized_label.into()],
        Vec::new(),
        context.clone(),
        context.one(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        IntegralFamilyLimits {
            max_fingerprint_bytes: 4,
            ..IntegralFamilyLimits::default()
        },
    );
    assert!(matches!(
        label_limit_first,
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family fingerprint string bytes",
            requested,
            limit: 4,
        }) if requested == oversized_label.len()
    ));

    // Symbolica parameter labels are identifiers.  Keep this fixture
    // oversized for the RustRed fingerprint limit without failing in the
    // coefficient-context parser before that boundary is reached.
    let oversized_parameter = "parameter_label_too_long";
    let parameter_context = CoefficientContext::new([oversized_parameter]);
    let parameter_limit_first = IntegralFamily::new_with_limits(
        "x",
        vec!["k".into()],
        Vec::new(),
        parameter_context.clone(),
        parameter_context.one(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        IntegralFamilyLimits {
            max_fingerprint_bytes: 4,
            ..IntegralFamilyLimits::default()
        },
    );
    assert!(matches!(
        parameter_limit_first,
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family fingerprint string bytes",
            requested,
            limit: 4,
        }) if requested == oversized_parameter.len()
    ));
}

#[test]
fn coordinates_are_loop_loop_then_loop_external() {
    let context = CoefficientContext::new(["d"]);
    let family = IntegralFamily::new(
        "two-loop-two-leg",
        vec!["k1".into(), "k2".into()],
        vec!["p1".into(), "p2".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        identity_denominators(&context, 7),
        vec![
            vec![context.one(), context.zero()],
            vec![context.zero(), context.one()],
        ],
        vec![context.zero(); 7],
    )
    .unwrap();

    assert_eq!(
        family.coordinates(),
        &[
            ScalarProductCoordinate::LoopLoop { left: 0, right: 0 },
            ScalarProductCoordinate::LoopLoop { left: 0, right: 1 },
            ScalarProductCoordinate::LoopLoop { left: 1, right: 1 },
            ScalarProductCoordinate::LoopExternal {
                loop_index: 0,
                external_index: 0,
            },
            ScalarProductCoordinate::LoopExternal {
                loop_index: 0,
                external_index: 1,
            },
            ScalarProductCoordinate::LoopExternal {
                loop_index: 1,
                external_index: 0,
            },
            ScalarProductCoordinate::LoopExternal {
                loop_index: 1,
                external_index: 1,
            },
        ]
    );
    assert_eq!(
        family.contraction_momenta(),
        &[
            ContractionMomentum::Loop(0),
            ContractionMomentum::Loop(1),
            ContractionMomentum::External(0),
            ContractionMomentum::External(1),
        ]
    );
    assert_eq!(
        family
            .coordinate_index(ScalarProductCoordinate::LoopExternal {
                loop_index: 1,
                external_index: 0,
            })
            .unwrap(),
        5
    );
    family.verify_exact_replay().unwrap();
}
