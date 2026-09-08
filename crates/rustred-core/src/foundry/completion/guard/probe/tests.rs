use symbolica::domains::finite_field::Zp64;

use crate::algebra::{
    CoefficientContext, IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, GuardPredicateAuthority,
    StratumRegistryLimits,
};
use crate::sector::{Mask, SectorMonotoneDomain};

use super::{
    ExactGuardPredicateCatalog, ExactGuardPredicateCatalogLimits, ExactGuardProbeError,
    ExactGuardProbeLimits, ExactGuardProbeWitness,
};

const FAMILY: &str = "rustred.test.exact-guard-probe.v1";

fn make_context(scope: &str) -> IndexedCoefficientContext {
    let base = CoefficientContext::new(["d"]);
    IndexedCoefficientContext::try_new(&base, scope, 2).unwrap()
}

fn polynomial(
    context: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
) -> IndexedPolynomial {
    context
        .numerator_condition_with_limits(value, Default::default())
        .unwrap()
}

fn guarded_stratum(
    context: &IndexedCoefficientContext,
    guard: &IndexedPolynomial,
    branch: GuardBranch,
) -> DecoratedStratum {
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true, true]).unwrap(),
        &[0, 0],
        &[vec![0, 0]],
    )
    .unwrap();
    let identity = GuardBranchIdentity::try_from_indexed_polynomial(
        context,
        guard,
        branch,
        Default::default(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    DecoratedStratum::try_new(
        FAMILY,
        context.fingerprint(),
        domain,
        [identity],
        StratumRegistryLimits::default(),
    )
    .unwrap()
}

fn affine_guard(context: &IndexedCoefficientContext) -> IndexedPolynomial {
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let n0_minus_one = context
        .sub(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let n1_minus_one = context
        .sub(&context.index(1).unwrap(), &context.one())
        .unwrap();
    let value = context
        .add(&context.mul(&d, &n0_minus_one).unwrap(), &n1_minus_one)
        .unwrap();
    polynomial(context, &value)
}

fn make_catalog(
    context: &IndexedCoefficientContext,
    guard: IndexedPolynomial,
) -> ExactGuardPredicateCatalog {
    ExactGuardPredicateCatalog::try_from_pulled_back(
        context,
        [guard],
        ExactGuardPredicateCatalogLimits::default(),
    )
    .unwrap()
}

#[test]
fn exact_zero_and_nonzero_branches_are_decided_before_base_sampling() {
    let context = make_context("guard-probe-branches");
    let guard = affine_guard(&context);
    let catalog = make_catalog(&context, guard.clone());
    let zero = guarded_stratum(&context, &guard, GuardBranch::Zero);
    let nonzero = guarded_stratum(&context, &guard, GuardBranch::NonZero);

    let zero_witness = ExactGuardProbeWitness::try_new(
        &context,
        &zero,
        &[37],
        &[1, 1],
        &catalog,
        ExactGuardProbeLimits::default(),
    )
    .unwrap();
    assert_eq!(zero_witness.guard_count(), 1);

    let nonzero_witness = ExactGuardProbeWitness::try_new(
        &context,
        &nonzero,
        &[37],
        &[2, 1],
        &catalog,
        ExactGuardProbeLimits::default(),
    )
    .unwrap();
    assert_eq!(nonzero_witness.guard_count(), 1);

    assert_eq!(
        ExactGuardProbeWitness::try_new(
            &context,
            &zero,
            &[37],
            &[2, 1],
            &catalog,
            ExactGuardProbeLimits::default(),
        )
        .unwrap_err(),
        ExactGuardProbeError::PredicateBranchMismatch {
            guard_ordinal: 0,
            required: GuardBranch::Zero,
            actual: GuardBranch::NonZero,
        }
    );
    assert_eq!(
        ExactGuardProbeWitness::try_new(
            &context,
            &nonzero,
            &[37],
            &[1, 1],
            &catalog,
            ExactGuardProbeLimits::default(),
        )
        .unwrap_err(),
        ExactGuardProbeError::PredicateBranchMismatch {
            guard_ordinal: 0,
            required: GuardBranch::NonZero,
            actual: GuardBranch::Zero,
        }
    );
}

#[test]
fn generic_nonzero_branch_rejects_an_accidental_base_root() {
    let context = make_context("guard-probe-base-wall");
    let guard = affine_guard(&context);
    let catalog = make_catalog(&context, guard.clone());
    let nonzero = guarded_stratum(&context, &guard, GuardBranch::NonZero);

    assert_eq!(
        ExactGuardProbeWitness::try_new(
            &context,
            &nonzero,
            &[0],
            &[2, 1],
            &catalog,
            ExactGuardProbeLimits::default(),
        )
        .unwrap_err(),
        ExactGuardProbeError::BaseSampleOnNonzeroGuardWall { guard_ordinal: 0 }
    );
}

#[test]
fn one_exact_witness_is_reused_across_primes_and_rejects_only_divisors() {
    let context = make_context("guard-probe-unlucky-prime");
    let value = context
        .add(&context.index(0).unwrap(), &context.integer(98))
        .unwrap();
    let guard = polynomial(&context, &value);
    let catalog = make_catalog(&context, guard.clone());
    let nonzero = guarded_stratum(&context, &guard, GuardBranch::NonZero);
    let witness = ExactGuardProbeWitness::try_new(
        &context,
        &nonzero,
        &[37],
        &[3, 1],
        &catalog,
        ExactGuardProbeLimits::default(),
    )
    .unwrap();

    assert_eq!(
        witness.try_validate_modulus(&Zp64::new(101)).unwrap_err(),
        ExactGuardProbeError::UnluckyGuardPrime {
            guard_ordinal: 0,
            modulus: 101,
        }
    );
    witness.try_validate_modulus(&Zp64::new(103)).unwrap();
}

#[test]
fn external_or_missing_predicate_payloads_fail_closed() {
    let context = make_context("guard-probe-payload-authority");
    let guard = affine_guard(&context);
    let catalog = make_catalog(&context, guard.clone());
    let domain = guarded_stratum(&context, &guard, GuardBranch::Zero)
        .domain()
        .clone();
    let external = GuardBranchIdentity::try_new(
        "external-only-proof-label",
        GuardBranch::Zero,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let external_stratum = DecoratedStratum::try_new(
        FAMILY,
        context.fingerprint(),
        domain,
        [external],
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert_eq!(
        ExactGuardProbeWitness::try_new(
            &context,
            &external_stratum,
            &[37],
            &[1, 1],
            &catalog,
            ExactGuardProbeLimits::default(),
        )
        .unwrap_err(),
        ExactGuardProbeError::UnsupportedPredicateAuthority {
            guard_ordinal: 0,
            authority: GuardPredicateAuthority::BoundExternalProof,
        }
    );

    let other_value = context
        .add(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let other_catalog = make_catalog(&context, polynomial(&context, &other_value));
    let exact_stratum = guarded_stratum(&context, &guard, GuardBranch::Zero);
    assert_eq!(
        ExactGuardProbeWitness::try_new(
            &context,
            &exact_stratum,
            &[37],
            &[1, 1],
            &other_catalog,
            ExactGuardProbeLimits::default(),
        )
        .unwrap_err(),
        ExactGuardProbeError::MissingPredicatePayload { guard_ordinal: 0 }
    );
}

#[test]
fn witness_scope_is_bound_to_context_stratum_and_both_exact_points() {
    let context = make_context("guard-probe-scope");
    let guard = affine_guard(&context);
    let catalog = make_catalog(&context, guard.clone());
    let nonzero = guarded_stratum(&context, &guard, GuardBranch::NonZero);
    let witness = ExactGuardProbeWitness::try_new(
        &context,
        &nonzero,
        &[37],
        &[2, 1],
        &catalog,
        ExactGuardProbeLimits::default(),
    )
    .unwrap();
    witness
        .try_validate_scope(&context, &nonzero, &[37], &[2, 1])
        .unwrap();
    assert_eq!(
        witness
            .try_validate_scope(&context, &nonzero, &[41], &[2, 1])
            .unwrap_err(),
        ExactGuardProbeError::WitnessBasePointMismatch
    );
    assert_eq!(
        witness
            .try_validate_scope(&context, &nonzero, &[37], &[3, 1])
            .unwrap_err(),
        ExactGuardProbeError::WitnessIndexPointMismatch
    );

    let zero = guarded_stratum(&context, &guard, GuardBranch::Zero);
    assert_eq!(
        witness
            .try_validate_scope(&context, &zero, &[37], &[2, 1])
            .unwrap_err(),
        ExactGuardProbeError::WitnessStratumMismatch
    );
    let foreign = make_context("guard-probe-scope-foreign");
    assert_eq!(
        witness
            .try_validate_scope(&foreign, &nonzero, &[37], &[2, 1])
            .unwrap_err(),
        ExactGuardProbeError::WitnessContextMismatch
    );
}

#[test]
fn aggregate_probe_limits_apply_before_exact_specialization() {
    let context = make_context("guard-probe-limits");
    let guard = affine_guard(&context);
    let catalog = make_catalog(&context, guard.clone());
    let nonzero = guarded_stratum(&context, &guard, GuardBranch::NonZero);
    let mut limits = ExactGuardProbeLimits::default();
    limits.max_guards = 0;
    assert_eq!(
        ExactGuardProbeWitness::try_new(&context, &nonzero, &[37], &[2, 1], &catalog, limits,)
            .unwrap_err(),
        ExactGuardProbeError::ResourceLimit {
            resource: "decorated-stratum guards",
            requested: 1,
            limit: 0,
        }
    );
}
