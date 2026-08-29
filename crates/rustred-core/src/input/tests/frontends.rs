use super::*;

#[test]
fn omitted_parameters_are_inferred_only_from_family_scalars() {
    let normalized = compiler()
        .compile_compact(&one_loop_source(2, "vec(k,mu)*tensor_only"), None)
        .expect("compact one-loop family should normalize");
    assert_eq!(normalized.parameter_source(), ParameterSource::Inferred);
    assert_eq!(
        normalized.parameter_names(),
        &["d".to_owned(), "m2".to_owned()]
    );
    assert!(!normalized.parameter_names().iter().any(|name| name == "mu"));
    assert!(
        !normalized
            .parameter_names()
            .iter()
            .any(|name| name == "tensor_only")
    );
}

#[test]
fn hybrid_parameter_override_must_match_an_internal_declaration() {
    let compiler = compiler();
    let source = "I(loops(k),externals(),parameters(m2,d),dimension(d),prop(D1,k^2-m2,1))";
    let matching = compiler
        .compile_compact(source, Some(vec!["m2".to_owned(), "d".to_owned()]))
        .expect("identical ordered declarations should agree");
    assert_eq!(
        matching.parameter_names(),
        &["m2".to_owned(), "d".to_owned()]
    );
    assert_eq!(
        matching.operational_parameter_names(),
        &["d".to_owned(), "m2".to_owned()]
    );

    let conflict = compiler.compile_compact(source, Some(vec!["d".to_owned(), "m2".to_owned()]));
    assert!(matches!(conflict, Err(Error::ConflictingParameterOverride)));
}

#[test]
fn canonical_rendering_round_trips_once_from_fully_qualified_names() {
    let compiler = compiler();
    let normalized = compiler
        .compile_compact(
            "I(loops(k),externals(),parameters(d,m2,tensor_only),dimension(d),prop(D1,k^2-m2,1),numerator(tensor_only*vec(k,mu)))",
            None,
        )
        .expect("compact input should normalize");
    let canonical = normalized.canonical_string();
    assert!(canonical.contains("rustred::"));
    let round_trip = compiler
        .compile_compact(&canonical, None)
        .expect("fully-qualified canonical output must be valid raw input");
    assert_eq!(round_trip.canonical_atom(), normalized.canonical_atom());
    assert_eq!(round_trip.parameter_names(), normalized.parameter_names());
}

#[test]
fn raw_and_text_parts_share_canonical_family_identity() {
    let compiler = compiler();
    let raw = compiler
        .compile_compact(&one_loop_source(1, "1"), None)
        .expect("raw family should normalize");
    let explicit = compiler
        .compile_text(TextProject {
            name: None,
            parameters: None,
            loop_momenta: vec!["k".to_owned()],
            external_momenta: vec![],
            dimension: "d".to_owned(),
            propagators: vec![TextPropagator {
                id: "D1".to_owned(),
                expression: "k^2-m2".to_owned(),
                target_power: 1,
                power_shift: None,
            }],
            external_gram: vec![],
            numerator: None,
        })
        .expect("text fields should normalize");
    assert_eq!(raw.canonical_atom(), explicit.canonical_atom());
    let raw_family = raw
        .into_lowered(LoweringLimits::default())
        .expect("raw family should lower");
    let explicit_family = explicit
        .into_lowered(LoweringLimits::default())
        .expect("explicit family should lower");
    assert_eq!(
        raw_family.family().fingerprint_ref(),
        explicit_family.family().fingerprint_ref()
    );
    assert_eq!(
        raw_family.denominators()[0].source(),
        raw_family.normalized().propagators()[0].expression(),
    );
}

#[test]
fn outer_only_parameters_and_all_three_frontends_converge() {
    let compiler = compiler();
    let source = one_loop_source(1, "1");
    let raw = compiler
        .compile_compact(&source, None)
        .expect("raw inferred family should normalize");
    let hybrid = compiler
        .compile_compact(&source, Some(vec!["d".to_owned(), "m2".to_owned()]))
        .expect("an outer-only strict allowlist should normalize");
    let explicit = compiler
        .compile_text(TextProject {
            name: None,
            parameters: Some(vec!["d".to_owned(), "m2".to_owned()]),
            loop_momenta: vec!["k".to_owned()],
            external_momenta: vec![],
            dimension: "d".to_owned(),
            propagators: vec![TextPropagator {
                id: "D1".to_owned(),
                expression: "k^2-m2".to_owned(),
                target_power: 1,
                power_shift: None,
            }],
            external_gram: vec![],
            numerator: Some("1".to_owned()),
        })
        .expect("explicit fields should normalize");
    assert_eq!(raw.canonical_atom(), hybrid.canonical_atom());
    assert_eq!(raw.canonical_atom(), explicit.canonical_atom());
    assert_eq!(hybrid.parameter_names(), &["d".to_owned(), "m2".to_owned()]);

    let raw = raw
        .into_lowered(LoweringLimits::default())
        .expect("raw family should lower");
    let hybrid = hybrid
        .into_lowered(LoweringLimits::default())
        .expect("hybrid family should lower");
    let explicit = explicit
        .into_lowered(LoweringLimits::default())
        .expect("explicit family should lower");
    assert_eq!(
        raw.family().fingerprint_ref(),
        hybrid.family().fingerprint_ref()
    );
    assert_eq!(
        raw.family().fingerprint_ref(),
        explicit.family().fingerprint_ref()
    );
}

#[test]
fn numerator_only_declared_extra_does_not_specialize_the_family() {
    let compiler = compiler();
    let inferred_source = one_loop_source(1, "tensor_only*vec(k,mu)");
    let declared_source = "I(loops(k),externals(),parameters(d,m2,tensor_only),dimension(d),prop(D1,k^2-m2,1),numerator(tensor_only*vec(k,mu)))";
    let inferred = compiler
        .compile_compact(&inferred_source, None)
        .expect("numerator-only symbols must not be inferred");
    let declared = compiler
        .compile_compact(declared_source, None)
        .expect("a declared numerator-only extra should be retained");
    assert_eq!(
        declared.parameter_names(),
        &["d".to_owned(), "m2".to_owned(), "tensor_only".to_owned()]
    );
    assert_eq!(
        declared.operational_parameter_names(),
        &["d".to_owned(), "m2".to_owned()]
    );
    assert_ne!(inferred.canonical_atom(), declared.canonical_atom());
    assert_eq!(
        inferred.operational_parameter_names(),
        declared.operational_parameter_names()
    );

    let inferred = inferred
        .into_lowered(LoweringLimits::default())
        .expect("inferred family should lower");
    let declared = declared
        .into_lowered(LoweringLimits::default())
        .expect("declared family should lower");
    assert_eq!(
        inferred.family().fingerprint_ref(),
        declared.family().fingerprint_ref()
    );
}
