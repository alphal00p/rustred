//! Compact target syntax reaches the existing generic tensor/lowering stack.
//!
//! The cancellation pairs are concrete validation inputs only.  Production
//! translation is configured solely by the authenticated family declarations
//! and contains no topology or loop-count dispatch.

use std::collections::BTreeMap;
use std::mem::size_of;

use rustred::*;
use symbolica::atom::AtomCore;

type LoweringSnapshot =
    BTreeMap<(TensorCovariantStructure, MetricPairing, ConcreteIntegralKey), Coefficient>;

fn lowered(power: i64, numerator: &str) -> LoweredSymbolicaProjectV1 {
    let source = format!(
        "I(loops(k),externals(),dimension(d),\
         prop(D1,k^2-m2,{power}),numerator({numerator}))"
    );
    SymbolicaIntegralInputCompiler::new(SymbolicaIntegralInputLimits::default())
        .unwrap()
        .compile_str(&source)
        .unwrap()
        .into_lowered(SymbolicaProjectLoweringLimits::default())
        .unwrap()
}

fn compile(
    project: &LoweredSymbolicaProjectV1,
) -> (SymbolicaTargetNumeratorCompiler, CompiledSymbolicaTargetV1) {
    let compiler = SymbolicaTargetNumeratorCompiler::try_new(
        project,
        SymbolicaTargetNumeratorLimits::default(),
    )
    .unwrap();
    let compiled = compiler.compile(project).unwrap();
    compiled.verify_replay(&compiler, project).unwrap();
    (compiler, compiled)
}

fn snapshot(lowering: &AuthenticatedVacuumCovariantTensorPolynomialLowering) -> LoweringSnapshot {
    let mut output = BTreeMap::new();
    for (covariant, reduction) in lowering.lowerings() {
        for (metrics, terms) in reduction.structures() {
            for (integral, coefficient) in terms {
                assert!(
                    output
                        .insert(
                            (covariant.clone(), metrics.clone(), integral.clone()),
                            coefficient.coefficient().clone(),
                        )
                        .is_none()
                );
            }
        }
    }
    output
}

fn independently_observed_retained_payload_bytes(
    project: &LoweredSymbolicaProjectV1,
    compiled: &CompiledSymbolicaTargetV1,
) -> usize {
    let tensor = compiled.tensor();
    let mut bytes = compiled.family_fingerprint().len();
    bytes += tensor.family_fingerprint().len();
    bytes += compiled.integral().powers().len() * size_of::<i64>();
    bytes += compiled.source_numerator().as_view().get_byte_size();
    bytes += compiled.translated_numerator().as_view().get_byte_size();
    bytes += tensor.source().as_view().get_byte_size();
    for position in 0..project.family().loop_count() {
        let vector = LoopVector::new(u16::try_from(position).unwrap());
        bytes += tensor.loop_atom(vector).unwrap().as_view().get_byte_size();
    }
    for term in tensor.terms() {
        bytes += term.weight().as_view().get_byte_size();
    }
    for allocation in tensor.index_allocations() {
        bytes += allocation.atom().as_view().get_byte_size();
    }
    for allocation in tensor.spectator_allocations() {
        bytes += allocation.atom().as_view().get_byte_size();
    }
    bytes
}

fn compile_and_snapshot(
    power: i64,
    numerator: &str,
) -> (
    LoweredSymbolicaProjectV1,
    CompiledSymbolicaTargetV1,
    LoweringSnapshot,
) {
    let project = lowered(power, numerator);
    let (_, compiled) = compile(&project);
    assert_eq!(compiled.integral().powers(), &[power]);
    let lowering = compiled.project_and_lower(&project).unwrap();
    lowering.verify(project.family()).unwrap();
    let snapshot = snapshot(&lowering);
    (project, compiled, snapshot)
}

fn assert_cancellation(
    numerator_power: i64,
    numerator: &str,
    explicit_power: i64,
    explicit_numerator: &str,
) {
    let (left_project, _, left) = compile_and_snapshot(numerator_power, numerator);
    let (right_project, _, right) = compile_and_snapshot(explicit_power, explicit_numerator);
    assert_eq!(
        left_project.family().fingerprint_ref(),
        right_project.family().fingerprint_ref(),
        "concrete target syntax must not specialize the family"
    );
    assert_eq!(left, right, "exact pre-IBP lowering does not close");
}

#[test]
fn compact_scalar_product_cancels_one_denominator() {
    assert_cancellation(4, "sp(k,k)-m2", 3, "1");
}

#[test]
fn ordinary_even_momentum_power_cancels_twice() {
    let (_, scalar_product) = compile(&lowered(1, "sp(k,k)^2"));
    let (_, ordinary_power) = compile(&lowered(1, "k^4"));
    assert_eq!(
        scalar_product.translated_numerator(),
        ordinary_power.translated_numerator(),
        "k^(2r) and sp(k,k)^r must enter the identical tensor syntax"
    );
    assert_cancellation(5, "(k^2-m2)^2", 3, "1");
}

#[test]
fn compact_rank_two_denominator_factor_cancels() {
    assert_cancellation(
        4,
        "(sp(k,k)-m2)*vec(k,mu)*vec(k,nu)",
        3,
        "vec(k,mu)*vec(k,nu)",
    );
}

#[test]
fn compact_rank_four_denominator_factor_cancels() {
    assert_cancellation(
        4,
        "(sp(k,k)-m2)*vec(k,a)*vec(k,b)*vec(k,c)*vec(k,e)",
        3,
        "vec(k,a)*vec(k,b)*vec(k,c)*vec(k,e)",
    );
}

#[test]
fn compact_metric_contraction_matches_scalar_product() {
    assert_cancellation(4, "metric(alpha,beta)*vec(k,alpha)*vec(k,beta)-m2", 3, "1");
}

#[test]
fn project_and_lower_rejects_a_different_concrete_target_in_the_same_family() {
    let source = lowered(4, "sp(k,k)-m2");
    let (_, compiled) = compile(&source);

    let different_power = lowered(3, "sp(k,k)-m2");
    assert_eq!(
        source.family().fingerprint_ref(),
        different_power.family().fingerprint_ref()
    );
    assert!(matches!(
        compiled.project_and_lower(&different_power),
        Err(SymbolicaTargetNumeratorError::ConcreteTargetPowerMismatch {
            position: 0,
            expected: 4,
            actual: 3,
        })
    ));

    let different_numerator = lowered(4, "1");
    assert_eq!(
        source.family().fingerprint_ref(),
        different_numerator.family().fingerprint_ref()
    );
    assert!(matches!(
        compiled.project_and_lower(&different_numerator),
        Err(SymbolicaTargetNumeratorError::ConcreteTargetNumeratorMismatch)
    ));
}

#[test]
fn compact_bridge_is_loop_count_neutral() {
    let source = "I(loops(k1,k2),externals(),dimension(d),\
                  prop(D1,k1^2-m2,1),prop(D2,k2^2-m2,1),\
                  prop(D3,(k1-k2)^2-m2,1),\
                  numerator(sp(k2,k1)*vec(k1,mu)*vec(k2,nu)))";
    let project = SymbolicaIntegralInputCompiler::new(SymbolicaIntegralInputLimits::default())
        .unwrap()
        .compile_str(source)
        .unwrap()
        .into_lowered(SymbolicaProjectLoweringLimits::default())
        .unwrap();
    let (_, compiled) = compile(&project);
    assert_eq!(compiled.integral().powers(), &[1, 1, 1]);
    assert_eq!(compiled.tensor().terms().len(), 1);
    assert_eq!(compiled.stats().scalar_product_calls(), 1);
    assert_eq!(compiled.stats().indexed_vector_calls(), 2);
}

#[test]
fn odd_or_bare_momentum_is_rejected_in_scalar_context() {
    for (numerator, expected) in [
        ("k", "bare"),
        ("k^3", "odd"),
        ("sp(k,k)^-1", "negative tensor"),
    ] {
        let project = lowered(1, numerator);
        let compiler = SymbolicaTargetNumeratorCompiler::try_new(
            &project,
            SymbolicaTargetNumeratorLimits::default(),
        )
        .unwrap();
        let error = compiler.compile(&project).unwrap_err();
        match (expected, error) {
            ("bare", SymbolicaTargetNumeratorError::BareMomentum { .. })
            | ("odd", SymbolicaTargetNumeratorError::OddMomentumPower { .. })
            | ("negative tensor", SymbolicaTargetNumeratorError::NegativeTensorPower { .. }) => {}
            (_, other) => panic!("unexpected compact-target rejection: {other}"),
        }
    }
}

#[test]
fn scalar_rational_structure_is_preserved_but_opaque_functions_are_rejected() {
    let scalar = lowered(1, "(d+2)/(m2^2)");
    let (_, compiled) = compile(&scalar);
    // The existing tensor polynomial normalizer expands the scalar sum while
    // retaining both exact rational weights in the declared family field.
    assert_eq!(compiled.tensor().terms().len(), 2);
    compiled.project_and_lower(&scalar).unwrap();

    let opaque = lowered(1, "J(d)");
    let compiler = SymbolicaTargetNumeratorCompiler::try_new(
        &opaque,
        SymbolicaTargetNumeratorLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        compiler.compile(&opaque),
        Err(SymbolicaTargetNumeratorError::UnsupportedFunction { .. })
    ));
}

#[test]
fn traversal_work_and_observed_byte_limits_are_exact_at_the_boundary() {
    let project = lowered(4, "(sp(k,k)-m2)*vec(k,mu)*vec(k,nu)");
    let (_, baseline) = compile(&project);
    let stats = baseline.stats();
    let independently_observed = independently_observed_retained_payload_bytes(&project, &baseline);
    assert_eq!(
        stats.observed_retained_payload_bytes(),
        independently_observed,
        "retained census must count both independently owned fingerprint Strings"
    );

    let mut exact = SymbolicaTargetNumeratorLimits::default();
    exact.max_input_nodes = stats.input_nodes();
    exact.max_translation_operations = stats.translation_operations();
    exact.max_observed_input_atom_bytes = stats.observed_input_atom_bytes();
    exact.max_observed_translated_atom_bytes = stats.observed_translated_atom_bytes();
    exact.max_observed_retained_payload_bytes = independently_observed;
    SymbolicaTargetNumeratorCompiler::try_new(&project, exact)
        .unwrap()
        .compile(&project)
        .unwrap();

    let mut too_few_nodes = exact;
    too_few_nodes.max_input_nodes = stats.input_nodes() - 1;
    assert!(matches!(
        SymbolicaTargetNumeratorCompiler::try_new(&project, too_few_nodes)
            .unwrap()
            .compile(&project),
        Err(SymbolicaTargetNumeratorError::ResourceLimit {
            resource: "compact target input",
            ..
        })
    ));

    let mut too_little_work = exact;
    too_little_work.max_translation_operations = stats.translation_operations() - 1;
    assert!(matches!(
        SymbolicaTargetNumeratorCompiler::try_new(&project, too_little_work)
            .unwrap()
            .compile(&project),
        Err(SymbolicaTargetNumeratorError::WorkLimit {
            resource: "compact target translation operations",
            ..
        })
    ));

    let mut too_few_input_bytes = exact;
    too_few_input_bytes.max_observed_input_atom_bytes = stats.observed_input_atom_bytes() - 1;
    assert!(matches!(
        SymbolicaTargetNumeratorCompiler::try_new(&project, too_few_input_bytes)
            .unwrap()
            .compile(&project),
        Err(SymbolicaTargetNumeratorError::ResourceLimit {
            resource: "observed compact target input Atom bytes",
            ..
        })
    ));

    let mut too_few_translated_bytes = exact;
    too_few_translated_bytes.max_observed_translated_atom_bytes =
        stats.observed_translated_atom_bytes() - 1;
    assert!(matches!(
        SymbolicaTargetNumeratorCompiler::try_new(&project, too_few_translated_bytes)
            .unwrap()
            .compile(&project),
        Err(SymbolicaTargetNumeratorError::ResourceLimit {
            resource: "observed translated target Atom bytes",
            ..
        })
    ));

    let mut too_few_retained_bytes = exact;
    too_few_retained_bytes.max_observed_retained_payload_bytes = independently_observed - 1;
    assert!(matches!(
        SymbolicaTargetNumeratorCompiler::try_new(&project, too_few_retained_bytes)
            .unwrap()
            .compile(&project),
        Err(SymbolicaTargetNumeratorError::ResourceLimit {
            resource: "compiled compact target observed retained payload bytes",
            ..
        })
    ));
}

#[test]
fn constructor_limits_declared_momenta_before_identity_allocation() {
    let project = lowered(1, "1");
    let mut exact = SymbolicaTargetNumeratorLimits::default();
    exact.max_loop_momenta = 1;
    exact.max_denominators = 1;
    exact.max_total_momentum_label_bytes = 1;
    SymbolicaTargetNumeratorCompiler::try_new(&project, exact).unwrap();

    let mut too_few = exact;
    too_few.max_loop_momenta = 0;
    assert!(matches!(
        SymbolicaTargetNumeratorCompiler::try_new(&project, too_few),
        Err(SymbolicaTargetNumeratorError::ResourceLimit {
            resource: "compact target loop momenta",
            ..
        })
    ));

    let mut too_few_denominators = exact;
    too_few_denominators.max_denominators = 0;
    assert!(matches!(
        SymbolicaTargetNumeratorCompiler::try_new(&project, too_few_denominators),
        Err(SymbolicaTargetNumeratorError::ResourceLimit {
            resource: "compact target denominators",
            ..
        })
    ));

    let mut too_few_label_bytes = exact;
    too_few_label_bytes.max_total_momentum_label_bytes = 0;
    assert!(matches!(
        SymbolicaTargetNumeratorCompiler::try_new(&project, too_few_label_bytes),
        Err(SymbolicaTargetNumeratorError::ResourceLimit {
            resource: "compact momentum label bytes",
            ..
        })
    ));
}
