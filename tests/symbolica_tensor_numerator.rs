use rustred::*;
use symbolica::prelude::*;

fn parse_atom(input: &str) -> Atom {
    try_parse!(input, default_namespace = "rustred_tensor_boundary_test").unwrap()
}

fn one_loop_family(name: &str, parameters: &[&str]) -> IntegralFamily {
    let context = CoefficientContext::new(parameters.iter().copied());
    IntegralFamily::new(
        name,
        vec!["ell".to_owned()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.parse("-m2").unwrap(),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn two_loop_identity_family(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    let denominators = (0..3)
        .map(|row| {
            AffineDenominator::new(
                context.zero(),
                (0..3)
                    .map(|column| {
                        if row == column {
                            context.one()
                        } else {
                            context.zero()
                        }
                    })
                    .collect(),
            )
        })
        .collect();
    IntegralFamily::new(
        name,
        vec!["left".to_owned(), "right".to_owned()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![context.zero(); 3],
    )
    .unwrap()
}

fn make_compiler(
    family: &IntegralFamily,
    loop_map: impl IntoIterator<Item = (&'static str, &'static str)>,
    limits: SymbolicaTensorNumeratorLimits,
) -> SymbolicaTensorNumeratorCompiler {
    let syntax = SymbolicaTensorSyntax::vakint().unwrap();
    SymbolicaTensorNumeratorCompiler::try_new(
        family,
        syntax,
        loop_map
            .into_iter()
            .map(|(name, atom)| (name.to_owned(), parse_atom(atom))),
        limits,
    )
    .unwrap()
}

fn compile_project_render(
    family: &IntegralFamily,
    compiler: &SymbolicaTensorNumeratorCompiler,
    source: &str,
) -> (CompiledSymbolicaTensorNumerator, Atom) {
    let source = parse_atom(source);
    let compiled = compiler.compile(source.as_view()).unwrap();
    let projection = compiled
        .project(family, GenericTensorPolynomialLimits::default())
        .unwrap();
    projection.verify(family).unwrap();
    let rendered = compiled.render_projected(projection.numerator()).unwrap();
    (compiled, rendered)
}

#[test]
fn exact_atom_boundary_reproduces_vakint_one_loop_a_and_b() {
    let family = one_loop_family("symbolica-atom-vakint-a-b", &["d", "m2"]);
    let compiler = make_compiler(
        &family,
        [("ell", "vakint::k(1)")],
        SymbolicaTensorNumeratorLimits::default(),
    );

    let (a, rendered_a) = compile_project_render(
        &family,
        &compiler,
        "vakint::k(1,1)*vakint::k(1,2)+vakint::k(1,3)*vakint::p(1,3)",
    );
    assert_eq!(
        rendered_a,
        parse_atom("rustred::d^-1*vakint::dot(vakint::k(1),vakint::k(1))*vakint::g(1,2)")
    );
    a.verify_replay(&compiler).unwrap();

    let (b, rendered_b) = compile_project_render(
        &family,
        &compiler,
        "(vakint::k(1,1)*vakint::k(1,2))^2*vakint::g(1,2)\
         +vakint::k(1,3)*vakint::p(1,3)\
         +vakint::k(1,1)*vakint::k(1,2)*vakint::p(2,1)*vakint::p(3,2)",
    );
    assert_eq!(
        rendered_b,
        parse_atom(
            "vakint::dot(vakint::k(1),vakint::k(1))^2*vakint::g(1,2)\
             +rustred::d^-1*vakint::dot(vakint::k(1),vakint::k(1))\
              *vakint::dot(vakint::p(2),vakint::p(3))"
        )
    );
    assert_eq!(b.terms().len(), 3);
    b.verify_replay(&compiler).unwrap();
}

#[test]
fn arbitrary_declared_weights_empty_monomials_and_decorated_indices_round_trip() {
    let family = one_loop_family("symbolica-atom-weighted-decorated", &["d", "m2", "A", "B"]);
    let compiler = make_compiler(
        &family,
        [("ell", "vakint::k(1)")],
        SymbolicaTensorNumeratorLimits::default(),
    );
    let source = "rustred::A*vakint::k(1,user_space::mink4(4,11))\
         *vakint::p(1,user_space::mink4(4,11))\
         *vakint::k(1,user_space::mink4(4,12))\
         *vakint::p(1,user_space::mink4(4,12))+rustred::B";
    let (compiled, rendered) = compile_project_render(&family, &compiler, source);
    assert_eq!(
        rendered,
        parse_atom(
            "rustred::B+rustred::A*rustred::d^-1\
             *vakint::dot(vakint::k(1),vakint::k(1))\
             *vakint::dot(vakint::p(1),vakint::p(1))"
        )
    );
    assert!(compiled.index_allocations().iter().any(|allocation| {
        allocation.atom() == &parse_atom("user_space::mink4(4,11)")
            && allocation.origin() == &SymbolicaIndexAllocationOrigin::Input
    }));
    assert!(compiled.terms().iter().any(|term| {
        term.weight() == &parse_atom("rustred::B")
            && term.monomial().loop_vectors().is_empty()
            && term.monomial().spectator_vectors().is_empty()
            && term.monomial().metrics().is_empty()
    }));
    compiled.verify_replay(&compiler).unwrap();

    let (decorated, rendered) = compile_project_render(
        &family,
        &compiler,
        "vakint::k(1,user_space::mink4(4,33))\
         *vakint::k(1,user_space::mink4(4,44))\
         *vakint::p(user_space::leg(7),user_space::mink4(4,33))\
         *vakint::p(user_space::leg(8),user_space::mink4(4,44))",
    );
    assert_eq!(
        rendered,
        parse_atom(
            "rustred::d^-1*vakint::dot(vakint::k(1),vakint::k(1))\
             *vakint::dot(vakint::p(user_space::leg(7)),\
                          vakint::p(user_space::leg(8)))"
        )
    );
    assert!(
        decorated.spectator_allocations().iter().any(|allocation| {
            allocation.atom() == &parse_atom("vakint::p(user_space::leg(7))")
        })
    );
    decorated.verify_replay(&compiler).unwrap();
}

#[test]
fn opaque_weights_are_retained_but_never_widen_the_family_field() {
    let family = one_loop_family("symbolica-atom-opaque-weight", &["d", "m2"]);
    let compiler = make_compiler(
        &family,
        [("ell", "vakint::k(1)")],
        SymbolicaTensorNumeratorLimits::default(),
    );
    let source = parse_atom("user_space::B");
    let compiled = compiler.compile(source.as_view()).unwrap();
    assert_eq!(compiled.terms().len(), 1);
    assert_eq!(compiled.terms()[0].weight(), &source);
    assert!(compiled.terms()[0].monomial().loop_vectors().is_empty());
    assert!(matches!(
        compiled.try_weighted_sources(&family),
        Err(SymbolicaTensorNumeratorError::DeferredWeight {
            source_term: 0,
            weight
        }) if weight == source
    ));

    let functional =
        parse_atom("user_space::opaque(user_space::tag)^-1*vakint::k(1,7)*vakint::k(1,8)");
    let compiled = compiler.compile(functional.as_view()).unwrap();
    assert_eq!(
        compiled.terms()[0].weight(),
        &parse_atom("user_space::opaque(user_space::tag)^-1")
    );
    assert!(matches!(
        compiled.try_weighted_sources(&family),
        Err(SymbolicaTensorNumeratorError::DeferredWeight { .. })
    ));
}

#[test]
fn dummy_indices_are_globally_collision_free_and_the_transcript_replays() {
    let family = one_loop_family("symbolica-atom-dummy-collision", &["d", "m2"]);
    let compiler = make_compiler(
        &family,
        [("ell", "vakint::k(1)")],
        SymbolicaTensorNumeratorLimits::default(),
    );
    let source = parse_atom(
        "vakint::dot(vakint::k(1),vakint::p(9))\
         *vakint::k(1,rustred::tensor_dummy_index(0))",
    );
    let compiled = compiler.compile(source.as_view()).unwrap();
    assert_eq!(compiled.stats().fresh_dummy_attempts, 2);
    assert!(compiled.index_allocations().iter().any(|allocation| {
        allocation.atom() == &parse_atom("rustred::tensor_dummy_index(0)")
            && allocation.origin() == &SymbolicaIndexAllocationOrigin::Input
    }));
    assert!(compiled.index_allocations().iter().any(|allocation| {
        allocation.atom() == &parse_atom("rustred::tensor_dummy_index(1)")
            && matches!(
                allocation.origin(),
                SymbolicaIndexAllocationOrigin::LoopSpectatorDot { .. }
            )
    }));
    let projection = compiled
        .project(&family, GenericTensorPolynomialLimits::default())
        .unwrap();
    assert_eq!(
        compiled.render_projected(projection.numerator()).unwrap(),
        parse_atom(
            "rustred::d^-1*vakint::dot(vakint::k(1),vakint::k(1))\
             *vakint::p(9,rustred::tensor_dummy_index(0))"
        )
    );
    compiled.verify_replay(&compiler).unwrap();
}

#[test]
fn user_indices_in_every_normalized_summand_precede_all_private_allocations() {
    let family = one_loop_family("symbolica-atom-global-index-prescan", &["d", "m2"]);
    let compiler = make_compiler(
        &family,
        [("ell", "vakint::k(1)")],
        SymbolicaTensorNumeratorLimits::default(),
    );
    let source = parse_atom(
        "vakint::dot(vakint::k(1),vakint::p(7))*vakint::k(1,user_space::mu)\
         +vakint::k(1,rustred::tensor_dummy_index(0))\
          *vakint::k(1,rustred::tensor_dummy_index(1))\
         +vakint::k(1,user_space::nu)^2",
    );
    let compiled = compiler.compile(source.as_view()).unwrap();
    assert_eq!(compiled.stats().fresh_dummy_attempts, 3);
    let first_generated = compiled
        .index_allocations()
        .iter()
        .position(|allocation| {
            matches!(
                allocation.origin(),
                SymbolicaIndexAllocationOrigin::LoopSpectatorDot { .. }
            )
        })
        .unwrap();
    assert!(
        compiled.index_allocations()[..first_generated]
            .iter()
            .all(|allocation| allocation.origin() == &SymbolicaIndexAllocationOrigin::Input)
    );
    assert!(
        compiled.index_allocations()[first_generated..]
            .iter()
            .all(|allocation| matches!(
                allocation.origin(),
                SymbolicaIndexAllocationOrigin::LoopSpectatorDot { .. }
            ))
    );
    assert!(compiled.index_allocations().iter().any(|allocation| {
        allocation.atom() == &parse_atom("rustred::tensor_dummy_index(2)")
            && matches!(
                allocation.origin(),
                SymbolicaIndexAllocationOrigin::LoopSpectatorDot { .. }
            )
    }));
    compiled.verify_replay(&compiler).unwrap();
}

#[test]
fn loop_identity_map_is_exact_order_independent_and_rendered_without_cascades() {
    let family = two_loop_identity_family("symbolica-atom-simultaneous-loop-map");
    let compiler = make_compiler(
        &family,
        [("right", "vakint::k(17)"), ("left", "vakint::k(3)")],
        SymbolicaTensorNumeratorLimits::default(),
    );
    let (compiled, rendered) = compile_project_render(
        &family,
        &compiler,
        "vakint::k(17,user_space::mu)*vakint::k(3,user_space::nu)",
    );
    assert_eq!(
        rendered,
        parse_atom(
            "rustred::d^-1*vakint::g(user_space::mu,user_space::nu)\
             *vakint::dot(vakint::k(3),vakint::k(17))"
        )
    );
    assert_eq!(
        compiled.loop_atom(LoopVector::new(0)),
        Some(&parse_atom("vakint::k(3)"))
    );
    assert_eq!(
        compiled.loop_atom(LoopVector::new(1)),
        Some(&parse_atom("vakint::k(17)"))
    );
}

#[test]
fn tensor_normalization_and_reserved_syntax_are_explicitly_bounded() {
    let family = one_loop_family("symbolica-atom-resource-bounds", &["d", "m2"]);
    let limits = SymbolicaTensorNumeratorLimits {
        max_power: 1,
        ..SymbolicaTensorNumeratorLimits::default()
    };
    let compiler = make_compiler(&family, [("ell", "vakint::k(1)")], limits);
    let powered = parse_atom("(vakint::k(1,1)*vakint::k(1,2))^2");
    assert!(matches!(
        compiler.compile(powered.as_view()),
        Err(SymbolicaTensorNumeratorError::ResourceLimit {
            resource: "tensor power",
            requested: 2,
            limit: 1,
        })
    ));

    let compiler = make_compiler(
        &family,
        [("ell", "vakint::k(1)")],
        SymbolicaTensorNumeratorLimits::default(),
    );
    let nested = parse_atom("user_space::wrapper(vakint::k(1,1))");
    assert!(matches!(
        compiler.compile(nested.as_view()),
        Err(SymbolicaTensorNumeratorError::UnsupportedReservedFactor { .. })
    ));
    let reciprocal = parse_atom("vakint::k(1,1)^-1");
    assert!(matches!(
        compiler.compile(reciprocal.as_view()),
        Err(SymbolicaTensorNumeratorError::UnsupportedTensorPower { .. })
    ));
}

#[test]
fn preflight_precedence_canonical_cancellation_and_foreign_coordinates_are_safe() {
    let family = one_loop_family("symbolica-atom-adversarial-boundary", &["d", "m2"]);
    let limits = SymbolicaTensorNumeratorLimits {
        max_expanded_factor_entries: 1,
        max_normalization_operations: 6,
        ..SymbolicaTensorNumeratorLimits::default()
    };
    let bounded = make_compiler(&family, [("ell", "vakint::k(1)")], limits);
    let expanding = parse_atom("(vakint::k(1,1)+vakint::k(1,2))^2");
    assert!(matches!(
        bounded.compile(expanding.as_view()),
        Err(SymbolicaTensorNumeratorError::ResourceLimit {
            resource: "expanded tensor factor entries",
            requested: 2,
            limit: 1,
        })
    ));

    let compiler = make_compiler(
        &family,
        [("ell", "vakint::k(1)")],
        SymbolicaTensorNumeratorLimits::default(),
    );
    // The Atom API is canonical: an exact cancellation has already erased the
    // reserved subtree before this boundary receives the view.
    let cancelled = parse_atom(
        "user_space::wrapper(vakint::k(1,99))\
         -user_space::wrapper(vakint::k(1,99))",
    );
    assert_eq!(cancelled, Atom::num(0));
    let compiled = compiler.compile(cancelled.as_view()).unwrap();
    assert_eq!(compiled.terms().len(), 1);
    assert_eq!(compiled.terms()[0].weight(), &Atom::num(0));
    let uncancelled = parse_atom("user_space::wrapper(vakint::k(1,99))");
    assert!(matches!(
        compiler.compile(uncancelled.as_view()),
        Err(SymbolicaTensorNumeratorError::UnsupportedReservedFactor { .. })
    ));

    let compiled = compiler.compile(Atom::num(1).as_view()).unwrap();
    let foreign_scalar = GenericScalarProductMonomial::try_from_factors([(
        ScalarProductCoordinate::LoopExternal {
            loop_index: 0,
            external_index: 0,
        },
        1,
    )])
    .unwrap();
    let foreign = GenericCovariantTensorNumerator::try_new_with_limit(
        [GenericCovariantTensorTerm::new(
            family.coefficient_context().one(),
            TensorCovariantStructure::new(
                MetricPairing::empty(),
                Vec::new(),
                SpectatorScalarProductMonomial::one(),
            ),
            foreign_scalar,
        )],
        1,
    )
    .unwrap();
    assert!(matches!(
        compiled.render_projected(&foreign),
        Err(
            SymbolicaTensorNumeratorError::UnsupportedRenderedScalarProduct {
                coordinate: ScalarProductCoordinate::LoopExternal {
                    loop_index: 0,
                    external_index: 0,
                }
            }
        )
    ));

    let tight_renderer = make_compiler(
        &family,
        [("ell", "vakint::k(1)")],
        SymbolicaTensorNumeratorLimits {
            max_render_factor_entries: 1,
            ..SymbolicaTensorNumeratorLimits::default()
        },
    );
    let source = parse_atom("vakint::k(1,user_space::mu)*vakint::k(1,user_space::nu)");
    let compiled = tight_renderer.compile(source.as_view()).unwrap();
    let oversized_covariant = TensorCovariantStructure::new(
        MetricPairing::new([
            Metric::new(LorentzIndex::new(0), LorentzIndex::new(1)),
            Metric::new(LorentzIndex::new(0), LorentzIndex::new(1)),
        ]),
        Vec::new(),
        SpectatorScalarProductMonomial::one(),
    );
    assert!(matches!(
        compiled.render_covariant(&oversized_covariant),
        Err(SymbolicaTensorNumeratorError::ResourceLimit {
            resource: "rendered covariant factor entries",
            requested: 2,
            limit: 1,
        })
    ));
}
