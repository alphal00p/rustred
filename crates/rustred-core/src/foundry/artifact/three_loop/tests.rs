use std::collections::{BTreeMap, BTreeSet};

use crate::algebra::matrix::{SymbolicaCoefficientMatrixLimits, rank_of_coefficient_matrix};
use crate::family::{IntegralKey, invert_symbolic_matrix};
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};

use super::family::canonical_family;
use super::manifest::{
    FULL_RANK_ORBITS, VAKINT_CLASSES, VAKINT_SOURCE_REVISION, VAKINT_TOPOLOGIES_BLOB, ZERO_ORBITS,
};
use super::symmetry::canonical_s4;

const EDGE_MOMENTA: [[i64; 3]; 6] = [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
    [-1, 0, 1],
    [1, -1, 0],
    [0, 1, -1],
];

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
        let rows = EDGE_MOMENTA
            .iter()
            .zip(orbit.representative)
            .map(|(momentum, power)| {
                momentum
                    .iter()
                    .map(|&component| {
                        family
                            .coefficient_context()
                            .integer(if power == 0 { 0 } else { component })
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let (rank, _) = rank_of_coefficient_matrix(
            family.coefficient_context(),
            &rows,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
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
