//! Current-path Symbolica tensor acceptance against the frozen Vakint oracle.
//!
//! The massive one-loop tadpole is only a concrete validation family.  Scalar
//! rules are not supplied by this test: one generic family inventory and one
//! generated symbolic row span feed an anchor-free sector root, a V2 row
//! system, and a V3 persistent elimination.  The certified family provider's
//! adaptive fallback is present only because the public API requires it and is
//! bounded to search depth zero.

use std::collections::BTreeSet;
use std::sync::Arc;

use rustred::*;
use symbolica::{atom::Atom, try_parse};

const ORDERING: IntegralOrderingPolicy = IntegralOrderingPolicy::RustRedUnshiftedV1;
const CYLINDRICAL_THROUGH_DEPTH: usize = 1;

fn parse_atom(input: &str) -> Atom {
    try_parse!(
        input,
        default_namespace = "rustred_cylindrical_tensor_oracle"
    )
    .unwrap()
}

fn massive_tadpole() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2", "A", "B", "C"]);
    IntegralFamily::new(
        "generated-cylindrical-one-loop-symbolica-tensor-oracle",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        // Vakint convention: k.k = D1 + mUV^2.
        vec![AffineDenominator::new(
            context.parse("-m2").unwrap(),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn key(power: i64) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new([power]).unwrap()
}

fn coefficient_for_rendered_covariant<'a>(
    compiled: &CompiledSymbolicaTensorNumerator,
    result: &'a AuthenticatedVacuumCovariantTensorPolynomialParametricReduction,
    rendered_covariant: &str,
) -> &'a Coefficient {
    let target = parse_atom(rendered_covariant);
    let (_, terms) = result
        .scalar_reduction()
        .structures()
        .iter()
        .find(|(covariant, _)| compiled.render_covariant(covariant).unwrap() == target)
        .unwrap_or_else(|| panic!("missing reduced covariant {target}"));
    assert_eq!(terms.len(), 1);
    terms.get(&key(1)).unwrap().coefficient()
}

#[test]
fn symbolica_rank_one_two_and_four_numerators_use_only_the_persistent_cylindrical_source() {
    let family = massive_tadpole();
    let coefficients = family.coefficient_context();
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let restrictions = SectorRestrictions::unrestricted(family.denominator_count()).unwrap();

    // Compile the shared generic family objects before choosing any concrete
    // tensor numerator or scalar power.
    let inventory = Arc::new(
        FamilySectorInventoryCompiler::compile(
            &family,
            restrictions.clone(),
            PowerShiftPolicy::FormalGeneric,
            ORDERING,
            FamilySectorInventoryLimits::default(),
        )
        .unwrap(),
    );
    let shared_row_span = Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            ParametricIbpConfig::default(),
            GeneratedSymbolicRowSpanConfig::default(),
        )
        .unwrap(),
    );
    assert_eq!(shared_row_span.rows().len(), 1, "L(L+E)=1 native IBP");

    let root = Arc::new(
        GeneratedCylindricalSectorRootStartCertificate::
            compile_with_replayed_inventory_and_row_span(
                &family,
                &context,
                Arc::clone(&inventory),
                SectorMask::try_new([true]).unwrap(),
                Arc::clone(&shared_row_span),
                CYLINDRICAL_THROUGH_DEPTH,
                GeneratedCylindricalSectorRootStartLimits::default(),
            )
            .unwrap(),
    );
    assert!(root.assignment().is_empty());
    assert!(Arc::ptr_eq(root.inventory_arc(), &inventory));
    assert!(Arc::ptr_eq(root.row_span_arc(), &shared_row_span));

    let rows = Arc::new(
        GeneratedCylindricalRowSystemCertificate::compile_from_sector_root(
            &family,
            &context,
            Arc::clone(&root),
            GeneratedCylindricalRowSystemLimits::default(),
        )
        .unwrap(),
    );
    assert_eq!(rows.schema(), GENERATED_CYLINDRICAL_ROW_SYSTEM_V2_SCHEMA);
    let persistent = Arc::new(
        GeneratedCylindricalPersistentEliminationCertificate::compile(
            &family,
            &context,
            Arc::clone(&rows),
            GeneratedCylindricalPersistentEliminationLimits::default(),
        )
        .unwrap(),
    );
    assert_eq!(
        persistent.schema(),
        GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA
    );
    persistent.replay(&family, &context).unwrap();
    assert_eq!(persistent.stats().elimination_builds(), 1);
    assert!(persistent.stats().pivot_rows() > 0);

    let generated = ParametricIbpGenerator::try_with_context(
        &family,
        context.clone(),
        ParametricIbpConfig::default(),
    )
    .unwrap()
    .generate()
    .unwrap();
    let canonical_rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    assert_eq!(canonical_rows.len(), 1);
    let symmetry_report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    assert!(symmetry_report.completion().is_exhaustive_within_bounds());

    let mut adaptive_limits = AdaptiveRuleSearchLimits::default();
    adaptive_limits.max_search_depth = 0;
    // Depth zero still contains the central scout point. Make all adaptive
    // work/output surfaces hostile so the strict proof-arm checks below are
    // backed by deterministic control-flow exclusion as well.
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
    let provider = CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_source(
        family.clone(),
        restrictions,
        symmetry_report.symmetries().iter().cloned(),
        adaptive,
        Arc::clone(&persistent),
        ORDERING,
        CertifiedFamilyRuleProviderLimits::default(),
    )
    .unwrap();
    assert_eq!(provider.adaptive().limits(), adaptive_limits);
    assert!(Arc::ptr_eq(
        provider.persistent_cylindrical_source().unwrap(),
        &persistent,
    ));
    let provider = MasterPolicyProvider::with_selected(provider, [key(1)]).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        coefficients,
        ORDERING,
        provider,
        ReductionEngineLimits::default(),
    );

    let compiler = SymbolicaTensorNumeratorCompiler::try_new(
        &family,
        SymbolicaTensorSyntax::vakint().unwrap(),
        [("k".to_owned(), parse_atom("vakint::k(3)"))],
        SymbolicaTensorNumeratorLimits::default(),
    )
    .unwrap();

    let rho = "user_space::mink4(4,33)";
    let odd_compiled = compiler
        .compile(parse_atom(&format!("rustred::B*vakint::k(3,{rho})*vakint::p(1,{rho})")).as_view())
        .unwrap();
    odd_compiled.verify_replay(&compiler).unwrap();
    assert_eq!(odd_compiled.terms().len(), 1);
    let odd_projection = odd_compiled
        .project(&family, GenericTensorPolynomialLimits::default())
        .unwrap();
    odd_projection.verify(&family).unwrap();
    assert!(
        odd_projection.numerator().is_zero(),
        "vacuum isotropy must remove the odd rank-one numerator before scalar IBP"
    );
    let odd_lowering = odd_projection.lower(&family, &key(4)).unwrap();
    let odd_result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant_polynomial(odd_lowering, &mut engine)
        .unwrap();
    odd_result.require_complete().unwrap();
    assert!(odd_result.scalar_reduction().is_zero());
    assert!(odd_result.scalar_reduction().scalar_witnesses().is_empty());
    odd_result.verify_with_engine(&family, &mut engine).unwrap();

    let mu = "user_space::mink4(4,11)";
    let nu = "user_space::mink4(4,22)";
    let a = "user_space::mink4(4,41)";
    let b = "user_space::mink4(4,42)";
    let c = "user_space::mink4(4,43)";
    let e = "user_space::mink4(4,44)";
    let even_compiled = compiler
        .compile(
            parse_atom(&format!(
                "rustred::A*vakint::k(3,{mu})*vakint::k(3,{nu})\
                 +rustred::C*vakint::k(3,{a})*vakint::k(3,{b})\
                  *vakint::k(3,{c})*vakint::k(3,{e})"
            ))
            .as_view(),
        )
        .unwrap();
    even_compiled.verify_replay(&compiler).unwrap();
    let even_projection = even_compiled
        .project(&family, GenericTensorPolynomialLimits::default())
        .unwrap();
    even_projection.verify(&family).unwrap();
    let even_lowering = even_projection.lower(&family, &key(4)).unwrap();
    let even_result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant_polynomial(even_lowering, &mut engine)
        .unwrap();
    even_result.require_complete().unwrap();
    even_result.verify(&family).unwrap();
    even_result
        .verify_with_engine(&family, &mut engine)
        .unwrap();

    assert_eq!(even_result.scalar_reduction().len(), 4);
    let rank_two = coefficients.parse("A*(d-4)*(d-2)/(48*m2^2)").unwrap();
    assert_eq!(
        coefficient_for_rendered_covariant(
            &even_compiled,
            &even_result,
            &format!("vakint::g({mu},{nu})"),
        ),
        &rank_two,
    );
    let rank_four = coefficients.parse("C*(d-2)/(48*m2)").unwrap();
    for pairing in [
        format!("vakint::g({a},{b})*vakint::g({c},{e})"),
        format!("vakint::g({a},{c})*vakint::g({b},{e})"),
        format!("vakint::g({a},{e})*vakint::g({b},{c})"),
    ] {
        assert_eq!(
            coefficient_for_rendered_covariant(&even_compiled, &even_result, &pairing),
            &rank_four,
        );
    }

    let scalar_witnesses = even_result.scalar_reduction().scalar_witnesses();
    assert_eq!(
        scalar_witnesses.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([key(2), key(3), key(4)])
    );
    let mut persistent_steps = 0_usize;
    for (source, witness) in scalar_witnesses {
        assert!(
            !witness.application_traces().is_empty(),
            "non-master scalar source {source:?} retained no proof"
        );
        let mut source_persistent_steps = 0_usize;
        for trace in witness.application_traces() {
            match trace {
                ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite) => match rewrite.proof() {
                    CertifiedConcreteRewriteProof::GeneratedCylindricalNumericQuotientElimination {
                        persistent_source,
                        ..
                    } => {
                        assert!(Arc::ptr_eq(persistent_source, &persistent));
                        persistent_steps += 1;
                        source_persistent_steps += 1;
                    }
                    CertifiedConcreteRewriteProof::Symmetry { .. } => {}
                    proof => panic!("scalar source {source:?} used a forbidden proof: {proof:?}"),
                },
                ConcreteRuleApplicationTrace::ProvedZero(_) => {}
                trace => panic!("scalar source {source:?} used a non-certified path: {trace:?}"),
            }
        }
        assert!(
            source_persistent_steps > 0,
            "nontrivial scalar source {source:?} never used the persistent source"
        );
    }
    assert!(persistent_steps > 0);
    assert_eq!(
        engine.provider().inner().adaptive().stats(),
        AdaptiveRuleSearchStats::default()
    );

    // The authenticated tensor result and each retained scalar proof must own
    // their complete replay graph, not borrow construction-time handles or a
    // live provider.
    drop(engine);
    drop(symmetry_report);
    drop(canonical_rows);
    drop(generated);
    drop(persistent);
    drop(rows);
    drop(root);
    drop(shared_row_span);
    drop(inventory);

    for witness in even_result.scalar_reduction().scalar_witnesses().values() {
        for trace in witness.application_traces() {
            match trace {
                ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite) => {
                    rewrite.replay(&family, &context, ORDERING).unwrap()
                }
                ConcreteRuleApplicationTrace::ProvedZero(zero) => zero.replay(&family).unwrap(),
                ConcreteRuleApplicationTrace::Parametric(_) => {
                    panic!("provider-free replay retained a parametric fallback")
                }
                ConcreteRuleApplicationTrace::ConditionalParametric(_) => {
                    panic!("provider-free replay retained a conditional fallback")
                }
            }
        }
    }
    odd_result.verify(&family).unwrap();
    even_result.verify(&family).unwrap();
    odd_compiled.verify_replay(&compiler).unwrap();
    even_compiled.verify_replay(&compiler).unwrap();
}
