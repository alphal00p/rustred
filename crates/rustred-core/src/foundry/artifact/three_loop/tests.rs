use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::family::presentation::{
    AuxiliaryDenominator, CommonMassScale, DenominatorRole, FamilyConventions, FamilyPresentation,
    MetricConvention, MomentumCombination, MomentumRouting, PhysicalPropagator,
    PropagatorConvention,
};
use crate::family::{IntegralKey, invert_symbolic_matrix};
use crate::foundry::artifact::ZeroTerminalProof;
use crate::foundry::completion::{
    CompletePhysicalContractionGoal, FamilyCoverageError, FamilyCoverageLimits,
};
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};
use crate::sector::Mask;

use super::family::canonical_family;
use super::manifest::{
    FULL_RANK_ORBITS, VAKINT_CLASSES, VAKINT_SOURCE_REVISION, VAKINT_TOPOLOGIES_BLOB, ZERO_ORBITS,
};
use super::momentum_rank::{EDGE_MOMENTA, active_momentum_rank};
use super::symmetry::canonical_s4;
use super::terminal_authority::derive_k6_terminal_authority;

fn canonical_presentation(auxiliary_slots: &[usize]) -> FamilyPresentation {
    let family = canonical_family().unwrap();
    let context = family.coefficient_context();
    let roles = EDGE_MOMENTA
        .iter()
        .enumerate()
        .map(|(slot, momentum)| {
            if auxiliary_slots.contains(&slot) {
                DenominatorRole::Auxiliary(AuxiliaryDenominator::new(format!("ISP{}", slot + 1)))
            } else {
                DenominatorRole::Physical(PhysicalPropagator::new(
                    format!("D{}", slot + 1),
                    MomentumCombination::new(
                        momentum
                            .iter()
                            .map(|&coefficient| context.integer(coefficient))
                            .collect(),
                        Vec::new(),
                    ),
                    context.one(),
                ))
            }
        })
        .collect();
    let routing = MomentumRouting::new(
        vec!["k1".to_owned(), "k2".to_owned(), "k3".to_owned()],
        Vec::new(),
        (0..3)
            .map(|row| {
                (0..3)
                    .map(|column| {
                        if row == column {
                            context.one()
                        } else {
                            context.zero()
                        }
                    })
                    .collect()
            })
            .collect(),
        vec![Vec::new(), Vec::new(), Vec::new()],
        Vec::new(),
    );
    let conventions = FamilyConventions::new(
        MetricConvention::Euclidean,
        PropagatorConvention::MOMENTUM_SQUARED_MINUS_MASS_SQUARED,
    );
    let common_scale = CommonMassScale::new(context.one());
    FamilyPresentation::try_new(family, roles, routing, conventions, Some(common_scale)).unwrap()
}

#[test]
fn pressure_family_owns_the_exact_nine_ordinary_sources() {
    let family = canonical_family().unwrap();
    assert_eq!(family.loop_count(), 3);
    assert_eq!(family.external_count(), 0);
    assert_eq!(family.denominator_count(), 6);
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    assert_eq!(prepared.len(), 9);
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let actual = prepared
        .complete(rows)
        .unwrap()
        .into_relations()
        .iter()
        .map(|relation| relation.row_id().stable_string())
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            "ordinary-ibp:0:0",
            "ordinary-ibp:0:1",
            "ordinary-ibp:0:2",
            "ordinary-ibp:1:0",
            "ordinary-ibp:1:1",
            "ordinary-ibp:1:2",
            "ordinary-ibp:2:0",
            "ordinary-ibp:2:1",
            "ordinary-ibp:2:2",
        ]
    );
}

#[test]
fn exact_s4_action_partitions_all_sectors_into_zero_and_full_rank_orbits() {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    assert_eq!(canonicalizer.generator_count(), 2);
    assert_eq!(canonicalizer.group_order(), 24);
    let mut orbits = BTreeMap::<Vec<i64>, BTreeSet<Vec<i64>>>::new();
    for bits in 0_u64..64 {
        let powers = (0..6)
            .map(|slot| i64::from(((bits >> slot) & 1) != 0))
            .collect::<Vec<_>>();
        let canonical = canonicalizer
            .canonicalize(&IntegralKey::try_new(powers.clone()).unwrap())
            .unwrap()
            .canonical()
            .powers()
            .to_vec();
        orbits.entry(canonical).or_default().insert(powers);
    }
    assert_eq!(orbits.len(), 11);
    let registered = ZERO_ORBITS
        .iter()
        .chain(FULL_RANK_ORBITS.iter())
        .map(|orbit| orbit.representative.to_vec())
        .collect::<BTreeSet<_>>();
    assert_eq!(registered.len(), 11, "the orbit manifest has duplicates");
    assert_eq!(
        orbits.keys().cloned().collect::<BTreeSet<_>>(),
        registered,
        "the orbit manifest is not the complete canonical partition"
    );
    for (expected_zero, orbit) in ZERO_ORBITS
        .iter()
        .map(|orbit| (true, orbit))
        .chain(FULL_RANK_ORBITS.iter().map(|orbit| (false, orbit)))
    {
        let members = orbits.get(orbit.representative.as_slice()).unwrap();
        assert_eq!(members.len(), orbit.size);
        // For this massive vacuum family, rank deficiency leaves an
        // unconstrained scaleless loop direction. Full rank only keeps the
        // orbit as a closure obligation; it is not used as an analytic
        // nonzero certificate. Exercise Symbolica's authenticated exact matrix
        // rank rather than a parallel CAS implementation.
        let sector = Mask::try_from_indices(&orbit.representative).unwrap();
        let rank = active_momentum_rank(&family, &sector).unwrap();
        assert_eq!(
            rank < family.loop_count(),
            expected_zero,
            "wrong active-momentum rank decision for {:?}",
            orbit.representative
        );
    }
    assert_eq!(
        ZERO_ORBITS.iter().map(|orbit| orbit.size).sum::<usize>(),
        26
    );
    assert_eq!(
        FULL_RANK_ORBITS
            .iter()
            .map(|orbit| orbit.size)
            .sum::<usize>(),
        38
    );
}

#[test]
fn complete_k6_physical_downset_plans_all_sixty_four_masks_and_eleven_s4_orbits() {
    let presentation = canonical_presentation(&[]);
    let family = presentation.family();
    let canonicalizer = canonical_s4(&family).unwrap();
    let goal = CompletePhysicalContractionGoal::try_new(&presentation).unwrap();
    let plan = goal
        .try_plan(&canonicalizer, FamilyCoverageLimits::default())
        .unwrap();
    let repeated = goal
        .try_plan(&canonicalizer, FamilyCoverageLimits::default())
        .unwrap();

    assert_eq!(plan, repeated);
    assert_eq!(goal.physical_slot_count(), 6);
    assert_eq!(goal.maximal_sector().active_bits(), &[true; 6]);
    assert_eq!(plan.family_fingerprint(), family.fingerprint());
    assert_eq!(plan.maximal_sector(), goal.maximal_sector());
    assert_eq!(plan.raw_sector_count(), 64);
    assert_eq!(plan.required_orbits().len(), 11);
    assert_eq!(
        plan.required_orbits()
            .iter()
            .map(|orbit| orbit.raw_sector_count())
            .sum::<usize>(),
        64
    );

    let planned = plan
        .required_orbits()
        .iter()
        .map(|orbit| (orbit.corner().powers().to_vec(), orbit.raw_sector_count()))
        .collect::<BTreeMap<_, _>>();
    let expected = ZERO_ORBITS
        .iter()
        .chain(FULL_RANK_ORBITS.iter())
        .map(|orbit| (orbit.representative.to_vec(), orbit.size))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(planned, expected);
    assert_eq!(planned.get(&vec![0, 0, 1, 1, 0, 1]), Some(&4));

    assert_eq!(
        plan.required_orbits()
            .iter()
            .map(|orbit| orbit.corner().powers().to_vec())
            .collect::<Vec<_>>(),
        [
            vec![0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 1],
            vec![0, 0, 0, 0, 1, 1],
            vec![0, 0, 1, 0, 1, 0],
            vec![0, 0, 0, 1, 1, 1],
            vec![0, 0, 1, 0, 1, 1],
            vec![0, 0, 1, 1, 0, 1],
            vec![0, 0, 1, 1, 1, 1],
            vec![0, 1, 1, 1, 1, 0],
            vec![0, 1, 1, 1, 1, 1],
            vec![1, 1, 1, 1, 1, 1],
        ]
    );
    assert!(plan.required_orbits().windows(2).all(|pair| {
        let left_active = pair[0].sector().active_count();
        let right_active = pair[1].sector().active_count();
        left_active < right_active
            || (left_active == right_active && pair[0].corner() < pair[1].corner())
    }));
}

#[test]
fn five_vakint_matcher_roots_are_not_a_complete_full_rank_sector_manifest() {
    let presentation = canonical_presentation(&[]);
    let family = presentation.family();
    let canonicalizer = canonical_s4(&family).unwrap();
    let goal = CompletePhysicalContractionGoal::try_new(&presentation).unwrap();
    let plan = goal
        .try_plan(&canonicalizer, FamilyCoverageLimits::default())
        .unwrap();

    let matcher_roots = VAKINT_CLASSES
        .iter()
        .map(|witness| witness.canonical_sector.to_vec())
        .collect::<BTreeSet<_>>();
    let full_rank_plan = FULL_RANK_ORBITS
        .iter()
        .map(|orbit| orbit.representative.to_vec())
        .collect::<BTreeSet<_>>();
    let planned = plan
        .required_orbits()
        .iter()
        .map(|orbit| orbit.corner().powers().to_vec())
        .collect::<BTreeSet<_>>();

    assert_eq!(matcher_roots.len(), 5);
    assert_eq!(full_rank_plan.len(), 6);
    assert!(matcher_roots.is_subset(&planned));
    assert_eq!(
        full_rank_plan
            .difference(&matcher_roots)
            .cloned()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([vec![0, 0, 1, 1, 0, 1]])
    );
}

#[test]
fn auxiliary_isp_slots_are_inactive_in_the_maximal_physical_sector() {
    let presentation = canonical_presentation(&[5]);
    let family = presentation.family();
    let canonicalizer = canonical_s4(&family).unwrap();
    let goal = CompletePhysicalContractionGoal::try_new(&presentation).unwrap();

    assert_eq!(goal.physical_slot_count(), 5);
    assert_eq!(
        goal.maximal_sector().active_bits(),
        &[true, true, true, true, true, false]
    );
    assert!(matches!(
        goal.try_plan(&canonicalizer, FamilyCoverageLimits::default()),
        Err(FamilyCoverageError::SlotRolesNotSymmetryInvariant { .. })
    ));
}

#[test]
fn sealed_terminal_authority_exactly_owns_zero_and_factorized_k6_regions() {
    let authority = derive_k6_terminal_authority().unwrap();
    assert!(Arc::ptr_eq(
        &authority,
        &derive_k6_terminal_authority().unwrap()
    ));
    assert_eq!(
        authority.authority_id(),
        "rustred.test.three-loop-k6-terminal-authority.v1"
    );
    assert_eq!(authority.arity(), 6);
    assert_eq!(authority.family().denominator_count(), 6);
    assert_eq!(authority.dependencies().len(), 2);
    assert_eq!(authority.zero_sectors().len(), 26);
    assert_eq!(authority.factorization_rules().len(), 3);
    assert_eq!(
        authority
            .factorization_rules()
            .iter()
            .map(|rule| rule.application_domain().sector().clone())
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    assert_eq!(authority.parent_terminals().len(), 3);
    assert_eq!(
        authority
            .factorization_rules()
            .iter()
            .map(|rule| rule.master_embeddings().len())
            .collect::<Vec<_>>(),
        [2, 1, 1]
    );
    assert_eq!(
        authority
            .zero_sectors()
            .iter()
            .filter(|terminal| { terminal.proof() == ZeroTerminalProof::ScalelessVacuumPolynomial })
            .count(),
        1
    );

    let zero_representatives = ZERO_ORBITS
        .iter()
        .map(|orbit| orbit.representative)
        .collect::<BTreeSet<_>>();
    for bits in 0_u64..64 {
        let powers: [i64; 6] = std::array::from_fn(|slot| i64::from(((bits >> slot) & 1) != 0));
        let key = IntegralKey::try_new(powers).unwrap();
        let canonical = authority
            .canonicalizer()
            .unwrap()
            .canonicalize(&key)
            .unwrap();
        assert_eq!(
            authority.is_zero_terminal(&key),
            zero_representatives.contains(canonical.canonical().powers()),
            "wrong sealed zero ownership for {powers:?}"
        );
    }
}

#[test]
fn frozen_vakint_class_snapshot_keeps_p_slots_and_unimodular_forced_bases_exact() {
    assert_eq!(VAKINT_SOURCE_REVISION.len(), 40);
    assert_eq!(VAKINT_TOPOLOGIES_BLOB.len(), 40);
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let context = family.coefficient_context();
    for witness in VAKINT_CLASSES {
        let raw_sector = witness.powers_by_slot([1; 6]);
        let canonical = canonicalizer
            .canonicalize(&IntegralKey::try_new(raw_sector).unwrap())
            .unwrap();
        assert_eq!(
            canonical.canonical().powers(),
            witness.canonical_sector,
            "{} has a stale sector route",
            witness.label
        );

        let distinct = witness.powers_by_slot([11, 12, 13, 14, 15, 16]);
        for (slot, &power) in distinct.iter().enumerate() {
            assert_eq!(
                power,
                if witness.active_slots[slot] {
                    11 + i64::try_from(slot).unwrap()
                } else {
                    0
                },
                "{} did not preserve propagator slot {}",
                witness.label,
                slot + 1
            );
        }

        let matrix = witness
            .routing_rows
            .chunks_exact(3)
            .map(|row| {
                row.iter()
                    .map(|&entry| context.integer(entry))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let (_, determinant) =
            invert_symbolic_matrix(context, &matrix, family.construction_limits()).unwrap();
        assert!(
            determinant == context.one() || determinant == context.integer(-1),
            "{} has a non-unimodular forced basis",
            witness.label
        );

        let mut selected_slots = BTreeSet::new();
        for row in witness.routing_rows.chunks_exact(3) {
            let matching_slot = EDGE_MOMENTA.iter().enumerate().find_map(|(slot, edge)| {
                let direct = row == edge;
                let reversed = row.iter().zip(edge).all(|(&left, &right)| left == -right);
                (witness.active_slots[slot] && (direct || reversed)).then_some(slot)
            });
            assert!(
                matching_slot.is_some_and(|slot| selected_slots.insert(slot)),
                "{} forced basis is not made of distinct active propagators",
                witness.label
            );
        }
    }

    let covered = VAKINT_CLASSES
        .iter()
        .map(|witness| witness.canonical_sector)
        .collect::<BTreeSet<_>>();
    assert_eq!(covered.len(), 5);
    assert!(
        !covered.contains(&[0, 0, 1, 1, 0, 1]),
        "the second spanning-tree orbit must remain an explicit extra artifact obligation"
    );
}
