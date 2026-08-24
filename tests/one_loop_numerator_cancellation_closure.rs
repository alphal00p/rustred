//! Metamorphic closure under numerator/denominator cancellation.
//!
//! Every pair below represents the same one-loop integral in two ways:
//! a factor `D1 = k^2-m2` is retained in the Symbolica numerator on the left
//! and cancelled explicitly by lowering the concrete propagator power on the
//! right.  Both sides independently rebuild and replay the generic generated
//! IBP certificate.  FORM and topology-authored recurrences are not used.

use std::collections::{BTreeMap, BTreeSet};

use rustred::*;
use symbolica::{
    atom::{Atom, AtomCore},
    try_parse,
};

const ORDERING: IntegralOrderingPolicy = IntegralOrderingPolicy::RustRedUnshiftedV1;

type LoweringSnapshot =
    BTreeMap<(TensorCovariantStructure, MetricPairing, ConcreteIntegralKey), Coefficient>;
type OutputSnapshot = BTreeMap<(TensorCovariantStructure, ConcreteIntegralKey), Coefficient>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct SemanticGuardLoci {
    family: BTreeSet<String>,
    projection: BTreeSet<String>,
    weights: BTreeSet<String>,
    lowering: BTreeSet<String>,
    scalar: BTreeSet<String>,
    certified: BTreeSet<String>,
}

struct CaseResult {
    reduction: AuthenticatedVacuumCovariantTensorPolynomialParametricReduction,
    lowering: LoweringSnapshot,
    output: OutputSnapshot,
    guards: SemanticGuardLoci,
}

fn family(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        // D1 = k^2-m2, hence k^2 = D1+m2.
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

fn parse_atom(input: &str) -> Atom {
    try_parse!(
        input,
        default_namespace = "rustred_one_loop_cancellation_closure"
    )
    .unwrap()
}

fn coefficient_polynomial_locus(polynomial: &CoefficientPolynomial) -> String {
    polynomial.to_expression().to_canonical_string()
}

fn snapshot_lowering(
    lowering: &AuthenticatedVacuumCovariantTensorPolynomialLowering,
) -> LoweringSnapshot {
    let mut snapshot = BTreeMap::new();
    for (covariant, reduction) in lowering.lowerings() {
        for (metrics, terms) in reduction.structures() {
            for (integral, coefficient) in terms {
                assert!(
                    snapshot
                        .insert(
                            (covariant.clone(), metrics.clone(), integral.clone()),
                            coefficient.coefficient().clone(),
                        )
                        .is_none(),
                    "a lowering snapshot key must be unique"
                );
            }
        }
    }
    snapshot
}

fn snapshot_output(reduction: &CovariantTensorParametricReductionResult) -> OutputSnapshot {
    let mut snapshot = BTreeMap::new();
    for (covariant, terms) in reduction.structures() {
        for (integral, coefficient) in terms {
            assert!(
                snapshot
                    .insert(
                        (covariant.clone(), integral.clone()),
                        coefficient.coefficient().clone(),
                    )
                    .is_none(),
                "a reduced-output snapshot key must be unique"
            );
        }
    }
    snapshot
}

fn semantic_guard_loci(
    family: &IntegralFamily,
    reduction: &AuthenticatedVacuumCovariantTensorPolynomialParametricReduction,
) -> SemanticGuardLoci {
    let mut loci = SemanticGuardLoci::default();
    loci.family.extend(
        family
            .domain()
            .conditions()
            .map(|condition| condition.polynomial().to_expression().to_canonical_string()),
    );
    for source in reduction.projection().source_projections() {
        loci.projection.extend(
            source
                .domain()
                .projection_nonzero_conditions()
                .iter()
                .map(|condition| condition.polynomial().to_expression().to_canonical_string()),
        );
    }
    loci.weights.extend(
        reduction
            .projection()
            .weight_nonzero_conditions()
            .iter()
            .map(|condition| condition.polynomial().to_expression().to_canonical_string()),
    );
    for lowering in reduction.authenticated_lowering().lowerings().values() {
        loci.lowering.extend(
            lowering
                .coefficient_nonzero_conditions()
                .iter()
                .map(|condition| condition.polynomial().to_expression().to_canonical_string()),
        );
    }
    loci.scalar
        .extend(reduction.scalar_guards().iter().map(|guard| {
            guard
                .condition()
                .polynomial()
                .to_expression()
                .to_canonical_string()
        }));
    for (_, conditions) in reduction.scalar_certified_domains() {
        loci.certified.extend(
            conditions
                .iter()
                .map(|condition| coefficient_polynomial_locus(condition.polynomial())),
        );
    }
    loci
}

fn reduce_independently(family: &IntegralFamily, source: &str, power: i64) -> CaseResult {
    // Construct syntax before parsing so Symbolica registers the conventional
    // symmetric/linear Vakint heads with their authenticated attributes.
    let compiler = SymbolicaTensorNumeratorCompiler::try_new(
        family,
        SymbolicaTensorSyntax::vakint().unwrap(),
        [("k".to_owned(), parse_atom("vakint::k(3)"))],
        SymbolicaTensorNumeratorLimits::default(),
    )
    .unwrap();
    let compiled = compiler.compile(parse_atom(source).as_view()).unwrap();
    compiled.verify_replay(&compiler).unwrap();
    let projection = compiled
        .project(family, GenericTensorPolynomialLimits::default())
        .unwrap();
    projection.verify(family).unwrap();
    let lowering = projection.lower(family, &key(power)).unwrap();
    lowering.verify(family).unwrap();
    let lowering_snapshot = snapshot_lowering(&lowering);

    // This complete generator/discovery/provider stack is rebuilt for every
    // side of every metamorphic equality.
    let generated = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .generate()
        .unwrap();
    let parametric_context = generated.context().clone();
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        family,
        &parametric_context,
        SectorMask::try_new([true]).unwrap(),
        ORDERING,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    discovery.replay(family, &parametric_context).unwrap();
    let sector_provider = ParametricSectorRuleProvider::try_new(
        family,
        &parametric_context,
        [discovery.coverage().clone()],
        ParametricSectorRuleProviderLimits::default(),
    )
    .unwrap();
    let master_provider = MasterPolicyProvider::with_selected(sector_provider, [key(1)]).unwrap();
    let provider = CertifiedZeroSectorRuleProvider::try_unrestricted(
        family,
        PowerShiftPolicy::FormalGeneric,
        master_provider,
        CertifiedRewriteLimits::default(),
    )
    .unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        ORDERING,
        provider,
        ReductionEngineLimits::default(),
    );
    let reduction = TensorParametricReductionComposer::new(family)
        .reduce_authenticated_covariant_polynomial(lowering, &mut engine)
        .unwrap();
    reduction.require_complete().unwrap();
    reduction.verify(family).unwrap();
    reduction.verify_with_engine(family, &mut engine).unwrap();
    let output = snapshot_output(reduction.scalar_reduction());
    let guards = semantic_guard_loci(family, &reduction);
    CaseResult {
        reduction,
        lowering: lowering_snapshot,
        output,
        guards,
    }
}

fn assert_equivalent(left: &CaseResult, right: &CaseResult) {
    assert_eq!(
        left.lowering, right.lowering,
        "exact scalar-product lowering must close before IBP"
    );
    assert_eq!(
        left.output, right.output,
        "unreplaced-master output differs"
    );
    assert_eq!(left.guards, right.guards, "semantic guard loci differ");
    assert_eq!(
        left.reduction.scalar_reduction().terminal_statuses(),
        right.reduction.scalar_reduction().terminal_statuses(),
        "terminal classifications differ"
    );
    assert_eq!(
        left.reduction.scalar_reduction().selected_masters(),
        right.reduction.scalar_reduction().selected_masters(),
        "selected master leaves differ"
    );
    assert_eq!(
        left.reduction
            .scalar_reduction()
            .certified_masters()
            .keys()
            .collect::<BTreeSet<_>>(),
        right
            .reduction
            .scalar_reduction()
            .certified_masters()
            .keys()
            .collect::<BTreeSet<_>>(),
        "certified master leaves differ"
    );
    assert_eq!(
        left.reduction.scalar_reduction().uncovered_leaves(),
        right.reduction.scalar_reduction().uncovered_leaves(),
        "uncovered leaves differ"
    );
}

fn assert_single_master_coefficient(family: &IntegralFamily, result: &CaseResult, expected: &str) {
    assert_eq!(result.output.len(), 1);
    let ((_, integral), coefficient) = result.output.iter().next().unwrap();
    assert_eq!(integral, &key(1));
    assert_eq!(
        coefficient,
        &family.coefficient_context().parse(expected).unwrap()
    );
}

fn assert_master_coefficients(
    family: &IntegralFamily,
    result: &CaseResult,
    expected_terms: usize,
    expected: &str,
) {
    assert_eq!(result.output.len(), expected_terms);
    let expected = family.coefficient_context().parse(expected).unwrap();
    for ((_, integral), coefficient) in &result.output {
        assert_eq!(integral, &key(1));
        assert_eq!(coefficient, &expected);
    }
}

#[test]
fn scalar_denominator_factor_cancels_exactly_before_generated_ibp() {
    let family = family("one-loop-cancellation-scalar");
    let numerator = reduce_independently(
        &family,
        "vakint::dot(vakint::k(3),vakint::k(3))-rustred::m2",
        4,
    );
    let explicit = reduce_independently(&family, "1", 3);
    assert_equivalent(&numerator, &explicit);
    assert_single_master_coefficient(&family, &numerator, "(d-4)*(d-2)/(8*m2^2)");
}

#[test]
fn repeated_denominator_factor_cancels_with_the_correct_multiplicity() {
    let family = family("one-loop-cancellation-squared");
    let numerator = reduce_independently(
        &family,
        "(vakint::dot(vakint::k(3),vakint::k(3))-rustred::m2)^2",
        5,
    );
    let explicit = reduce_independently(&family, "1", 3);
    assert_equivalent(&numerator, &explicit);
    assert_single_master_coefficient(&family, &numerator, "(d-4)*(d-2)/(8*m2^2)");
}

#[test]
fn denominator_factor_cancels_in_a_free_rank_two_numerator() {
    let family = family("one-loop-cancellation-free-rank-two");
    let tensor = "vakint::k(3,user_space::mu)*vakint::k(3,user_space::nu)";
    let numerator = reduce_independently(
        &family,
        &format!("(vakint::dot(vakint::k(3),vakint::k(3))-rustred::m2)*{tensor}"),
        4,
    );
    let explicit = reduce_independently(&family, tensor, 3);
    assert_equivalent(&numerator, &explicit);
    // k(mu)k(nu) I(3) = g(mu,nu) (d-2)/(8 m2) I(1).
    assert_master_coefficients(&family, &numerator, 1, "(d-2)/(8*m2)");
}

#[test]
fn denominator_factor_cancels_in_a_free_rank_four_numerator() {
    let family = family("one-loop-cancellation-free-rank-four");
    let tensor = "vakint::k(3,user_space::a)*vakint::k(3,user_space::b)\
                  *vakint::k(3,user_space::c)*vakint::k(3,user_space::e)";
    let numerator = reduce_independently(
        &family,
        &format!("(vakint::dot(vakint::k(3),vakint::k(3))-rustred::m2)*{tensor}"),
        4,
    );
    let explicit = reduce_independently(&family, tensor, 3);
    assert_equivalent(&numerator, &explicit);
    // The rank-four vacuum projector produces all three metric pairings.
    assert_master_coefficients(&family, &numerator, 3, "1/8");
}

#[test]
fn metric_contracted_denominator_spelling_matches_explicit_cancellation() {
    let family = family("one-loop-cancellation-metric-spelling");
    let numerator = reduce_independently(
        &family,
        "vakint::g(user_space::alpha,user_space::beta)\
         *vakint::k(3,user_space::alpha)*vakint::k(3,user_space::beta)\
         -rustred::m2",
        4,
    );
    let explicit = reduce_independently(&family, "1", 3);
    assert_equivalent(&numerator, &explicit);
    assert_single_master_coefficient(&family, &numerator, "(d-4)*(d-2)/(8*m2^2)");
}
