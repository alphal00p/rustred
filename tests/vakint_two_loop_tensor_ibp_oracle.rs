//! Current-path FORM-free validation of Vakint's two-loop tensor fixture.
//!
//! The source tensor is copied from `vakint/tests/tensor_reduction_tests.rs`.
//! Its projector image is checked before scalar reduction. One automatic raw
//! family inventory then compiles the four unresolved sunset sectors from one
//! shared generated row span into sector-local persistent V3 sources. The
//! adaptive provider is configured at depth zero and must perform no work.
//! The expected coefficients below are an independent frozen alphaLoop oracle;
//! no topology-specific recurrence, pivot, or loop dispatch is available to
//! production RustRed code.

use std::collections::BTreeMap;
use std::sync::Arc;

use rustred::*;
use symbolica::{atom::Atom, try_parse};

const ORDERING: IntegralOrderingPolicy = IntegralOrderingPolicy::RustRedUnshiftedV1;
const CYLINDRICAL_THROUGH_DEPTH: usize = 1;

fn parse_atom(input: &str) -> Atom {
    try_parse!(
        input,
        default_namespace = "rustred_vakint_two_loop_atom_oracle"
    )
    .unwrap()
}

fn family() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    let zero = context.zero();
    let one = context.one();
    let minus_m2 = context.parse("-m2").unwrap();
    IntegralFamily::new(
        "vakint-two-loop-tensor-parametric-oracle",
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        // Vakint convention:
        // D1=k1.k1-m2, D2=k2.k2-m2, D3=(k1+k2)^2-m2.
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), context.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn key(powers: [i64; 3]) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

#[test]
fn generated_two_loop_rules_reduce_the_complete_vakint_tensor_source() {
    let family = family();
    let coefficients = family.coefficient_context();
    let parametric_context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let restrictions = SectorRestrictions::unrestricted(family.denominator_count()).unwrap();

    let tensor_syntax = SymbolicaTensorSyntax::vakint().unwrap();
    let tensor_compiler = SymbolicaTensorNumeratorCompiler::try_new(
        &family,
        tensor_syntax,
        [
            ("k1".to_owned(), parse_atom("vakint::k(1)")),
            ("k2".to_owned(), parse_atom("vakint::k(2)")),
        ],
        SymbolicaTensorNumeratorLimits::default(),
    )
    .unwrap();

    // Compile the complete raw unresolved family queue before selecting a
    // concrete numerator or integral power. The equal-mass sunset is only a
    // validation family supplied through the generic IntegralFamily API.
    let source_set = GeneratedCylindricalFamilySourceSetCompiler::compile(
        &family,
        &parametric_context,
        restrictions.clone(),
        PowerShiftPolicy::FormalGeneric,
        ORDERING,
        ParametricIbpConfig::default(),
        GeneratedSymbolicRowSpanConfig::default(),
        CYLINDRICAL_THROUGH_DEPTH,
        GeneratedCylindricalFamilySourceSetLimits::default(),
    )
    .unwrap();
    source_set.replay(&family, &parametric_context).unwrap();

    let expected_solve_order = [
        SectorMask::try_new([false, true, true]).unwrap(),
        SectorMask::try_new([true, false, true]).unwrap(),
        SectorMask::try_new([true, true, false]).unwrap(),
        SectorMask::try_new([true, true, true]).unwrap(),
    ];
    assert_eq!(source_set.solve_order(), expected_solve_order);
    let inventory = Arc::clone(source_set.inventory_arc());
    let shared_row_span = Arc::clone(
        source_set
            .row_span_arc()
            .expect("a nonempty family solve order has one generated row span"),
    );
    assert_eq!(shared_row_span.rows().len(), 4, "L(L+E)=4 native IBPs");
    let sources = source_set.persistent_sources().to_vec();
    assert_eq!(sources.len(), expected_solve_order.len());
    let source_by_sector = expected_solve_order
        .iter()
        .cloned()
        .zip(sources.iter().cloned())
        .collect::<BTreeMap<_, _>>();
    for source in &sources {
        assert_eq!(
            source.schema(),
            GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA
        );
        let start = source.row_system().start();
        let root = start
            .sector_root_start()
            .expect("automatic V1 family sources are raw sector roots");
        assert!(start.assignment().is_empty());
        assert!(Arc::ptr_eq(root.inventory_arc(), &inventory));
        assert!(Arc::ptr_eq(root.row_span_arc(), &shared_row_span));
    }

    let generated = ParametricIbpGenerator::try_with_context(
        &family,
        parametric_context.clone(),
        ParametricIbpConfig::default(),
    )
    .unwrap()
    .generate()
    .unwrap();
    let canonical_rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    assert_eq!(canonical_rows.len(), 4, "L(L+E)=4 generated ordinary IBPs");
    let symmetries = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    assert!(symmetries.completion().is_exhaustive_within_bounds());
    assert_eq!(symmetries.symmetries().len(), 6);

    let mut adaptive_limits = AdaptiveRuleSearchLimits::default();
    adaptive_limits.max_search_depth = 0;
    // Depth zero still contains the central scout point. Make every adaptive
    // work/output surface zero as well, so entering either ordinary fallback
    // path is a deterministic resource error rather than invisible work.
    adaptive_limits.max_enumerated_offsets_per_integral = 0;
    adaptive_limits.max_offset_enumeration_steps_per_layer = 0;
    adaptive_limits.max_offset_components_per_integral = 0;
    adaptive_limits.max_scout_points_per_integral = 0;
    adaptive_limits.max_pivot_candidates_per_integral = 0;
    adaptive_limits.max_cached_decisions = 0;
    adaptive_limits.elimination.max_source_rows = 0;
    adaptive_limits.elimination.max_columns = 0;
    adaptive_limits.elimination.max_pivots = 0;
    adaptive_limits.rule.max_rhs_terms = 0;
    adaptive_limits.rule.max_source_rows_for_replay = 0;
    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &canonical_rows,
        ORDERING,
        adaptive_limits,
    )
    .unwrap();
    let provider = CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_sources(
        family.clone(),
        restrictions,
        symmetries.symmetries().iter().cloned(),
        adaptive,
        source_set.persistent_sources().iter().cloned(),
        ORDERING,
        CertifiedFamilyRuleProviderLimits::default(),
    )
    .unwrap();
    assert_eq!(provider.persistent_cylindrical_sources().len(), 4);
    assert_eq!(provider.adaptive().limits(), adaptive_limits);
    let provider =
        MasterPolicyProvider::with_selected(provider, [key([1, 1, 1]), key([0, 1, 1])]).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        coefficients,
        ORDERING,
        provider,
        ReductionEngineLimits::default(),
    );

    let numerator = parse_atom(
        "(vakint::k(1,1)*vakint::k(2,2))^2*vakint::g(1,2)\
         +vakint::k(2,3)*vakint::p(1,3)\
         +vakint::k(1,1)*vakint::k(2,2)*vakint::p(2,1)*vakint::p(3,2)",
    );
    let compiled = tensor_compiler.compile(numerator.as_view()).unwrap();
    compiled.verify_replay(&tensor_compiler).unwrap();
    assert_eq!(compiled.terms().len(), 3);

    let projection = compiled
        .project(&family, GenericTensorPolynomialLimits::default())
        .unwrap();
    projection.verify(&family).unwrap();

    let odd_source = compiled
        .terms()
        .iter()
        .position(|term| {
            term.monomial().loop_vectors().len() == 1
                && term.monomial().spectator_vectors().len() == 1
        })
        .unwrap();
    assert!(
        projection
            .source_projection(odd_source)
            .unwrap()
            .numerator()
            .is_zero(),
        "the odd-rank Vakint summand must project to zero"
    );

    assert_eq!(
        compiled.render_projected(projection.numerator()).unwrap(),
        parse_atom(
            "vakint::dot(vakint::k(1),vakint::k(1))\
             *vakint::dot(vakint::k(2),vakint::k(2))*vakint::g(1,2)\
             +rustred::d^-1*vakint::dot(vakint::k(1),vakint::k(2))\
              *vakint::dot(vakint::p(2),vakint::p(3))"
        )
    );

    let metric_atom = parse_atom("vakint::g(1,2)");
    let spectator_atom = parse_atom("vakint::dot(vakint::p(2),vakint::p(3))");
    let metric = projection
        .numerator()
        .terms()
        .iter()
        .find(|term| compiled.render_covariant(term.covariant()).unwrap() == metric_atom)
        .unwrap()
        .covariant()
        .clone();
    let spectator = projection
        .numerator()
        .terms()
        .iter()
        .find(|term| compiled.render_covariant(term.covariant()).unwrap() == spectator_atom)
        .unwrap()
        .covariant()
        .clone();
    assert_eq!(projection.numerator().terms().len(), 2);
    assert!(projection.numerator().terms().iter().any(|term| {
        term.covariant() == &metric
            && term
                .loop_scalar_products()
                .exponent(ScalarProductCoordinate::LoopLoop { left: 0, right: 0 })
                == 1
            && term
                .loop_scalar_products()
                .exponent(ScalarProductCoordinate::LoopLoop { left: 1, right: 1 })
                == 1
            && term.coefficient() == &coefficients.one()
    }));
    assert!(projection.numerator().terms().iter().any(|term| {
        term.covariant() == &spectator
            && term
                .loop_scalar_products()
                .exponent(ScalarProductCoordinate::LoopLoop { left: 0, right: 1 })
                == 1
            && term.coefficient() == &coefficients.parse("1/d").unwrap()
    }));

    let lowering = projection.lower(&family, &key([1, 2, 1])).unwrap();
    lowering.verify(&family).unwrap();

    let result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant_polynomial(lowering, &mut engine)
        .unwrap();
    result.require_complete().unwrap();

    // Frozen alphaLoop result after tensor lowering and scalar IBPs, with the
    // two masters deliberately left unsubstituted:
    //   g12 [d/2 J011 + d*m2/3 J111] - (p2.p3)/6 J111.
    assert_eq!(result.scalar_reduction().len(), 3);
    assert_eq!(
        result
            .scalar_reduction()
            .term(&metric, &key([0, 1, 1]))
            .unwrap()
            .coefficient(),
        &coefficients.parse("d/2").unwrap()
    );
    assert_eq!(
        result
            .scalar_reduction()
            .term(&metric, &key([1, 1, 1]))
            .unwrap()
            .coefficient(),
        &coefficients.parse("d*m2/3").unwrap()
    );
    assert_eq!(
        result
            .scalar_reduction()
            .term(&spectator, &key([1, 1, 1]))
            .unwrap()
            .coefficient(),
        &coefficients.parse("-1/6").unwrap()
    );
    assert!(
        result
            .scalar_reduction()
            .term(&spectator, &key([0, 1, 1]))
            .is_none()
    );
    assert_eq!(
        result.scalar_certified_domains().len(),
        result.scalar_reduction().scalar_witnesses().len()
    );
    let mut persistent_steps = 0_usize;
    let mut symmetry_steps = 0_usize;
    let mut tensor_zero_steps = 0_usize;
    for (source, witness) in result.scalar_reduction().scalar_witnesses() {
        assert_eq!(
            result.scalar_certified_domain(source),
            Some(witness.certified_domain())
        );
        if witness.application_traces().is_empty() {
            assert!(
                source == &key([0, 1, 1]) || source == &key([1, 1, 1]),
                "only explicitly selected masters may have an empty proof trace: {source:?}"
            );
            assert_eq!(
                witness.terms(),
                &BTreeMap::from([(source.clone(), coefficients.one())])
            );
            continue;
        }
        for trace in witness.application_traces() {
            match trace {
                ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite) => match rewrite.proof() {
                    CertifiedConcreteRewriteProof::GeneratedCylindricalNumericQuotientElimination {
                        persistent_source,
                        ..
                    } => {
                        let sector = SectorMask::try_from_indices(rewrite.source().powers()).unwrap();
                        let expected_source = source_by_sector.get(&sector).unwrap_or_else(|| {
                            panic!(
                                "rewrite source {:?} has no automatic family source",
                                rewrite.source()
                            )
                        });
                        assert!(Arc::ptr_eq(expected_source, persistent_source));
                        assert_eq!(persistent_source.row_system().start().sector(), &sector);
                        persistent_steps += 1;
                    }
                    CertifiedConcreteRewriteProof::Symmetry { path } => {
                        assert!(!path.is_empty());
                        symmetry_steps += 1;
                    }
                    proof => panic!(
                        "scalar source {source:?} used a forbidden adaptive proof: {proof:?}"
                    ),
                },
                ConcreteRuleApplicationTrace::ProvedZero(proof) => {
                    proof.replay(&family).unwrap();
                    tensor_zero_steps += 1;
                }
                ConcreteRuleApplicationTrace::Parametric(_) => {
                    panic!("scalar source {source:?} retained a parametric fallback")
                }
                ConcreteRuleApplicationTrace::ConditionalParametric(_) => {
                    panic!("scalar source {source:?} retained a conditional fallback")
                }
            }
        }
    }
    assert!(
        persistent_steps > 0,
        "tensor reduction used no persistent V3 source"
    );
    assert!(
        symmetry_steps > 0,
        "tensor reduction exercised no exact S3 transport"
    );

    // Exercise the only remaining admissible trace arm explicitly. A single
    // sunset line leaves an unconstrained loop integration and is analytically
    // zero; no selected-master or search-exhaustion convention is involved.
    let zero_result = engine.reduce(&key([0, 0, 1])).unwrap();
    zero_result.require_complete().unwrap();
    assert!(zero_result.terms().is_empty());
    assert!(zero_result.terminal_statuses().is_empty());
    let mut retained_zero_proofs = Vec::new();
    for trace in zero_result.application_traces() {
        match trace {
            ConcreteRuleApplicationTrace::ProvedZero(proof) => {
                assert_eq!(proof.source(), &key([0, 0, 1]));
                proof.replay(&family).unwrap();
                retained_zero_proofs.push(proof.clone());
            }
            trace => panic!("single-line zero used an unexpected proof arm: {trace:?}"),
        }
    }
    assert!(!retained_zero_proofs.is_empty());
    assert_eq!(
        tensor_zero_steps, 0,
        "the nonzero tensor fixture unexpectedly hit zero"
    );

    let adaptive_stats = engine.provider().inner().adaptive().stats();
    assert_eq!(adaptive_stats, AdaptiveRuleSearchStats::default());
    result.verify_with_engine(&family, &mut engine).unwrap();

    // Provider-free replay must reconstruct every generated specialization,
    // quotient witness, and exact elimination after the provider and every
    // external source-set/build handle have been destroyed.
    drop(engine);
    drop(source_by_sector);
    drop(sources);
    drop(shared_row_span);
    drop(inventory);
    drop(source_set);
    drop(symmetries);
    drop(canonical_rows);
    drop(generated);

    for witness in result.scalar_reduction().scalar_witnesses().values() {
        for trace in witness.application_traces() {
            match trace {
                ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite) => rewrite
                    .replay(&family, &parametric_context, ORDERING)
                    .unwrap(),
                ConcreteRuleApplicationTrace::ProvedZero(proof) => proof.replay(&family).unwrap(),
                ConcreteRuleApplicationTrace::Parametric(_) => {
                    panic!("provider-free replay retained a parametric fallback")
                }
                ConcreteRuleApplicationTrace::ConditionalParametric(_) => {
                    panic!("provider-free replay retained a conditional fallback")
                }
            }
        }
    }
    for proof in retained_zero_proofs {
        proof.replay(&family).unwrap();
    }
    result.verify(&family).unwrap();
}
