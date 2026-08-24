#![cfg(feature = "legacy-authored-oracles")]

use std::collections::{BTreeMap, BTreeSet};

use rustred::{
    CoefficientContext, FOUR_LOOP_BOUNDARY_HALO_BLOCKER_OCCURRENCES,
    FOUR_LOOP_BOUNDARY_HALO_FORMULA_DISPATCHES, FOUR_LOOP_BOUNDARY_HALO_SIGNED_LINE_DISPATCHES,
    FOUR_LOOP_BOUNDARY_HALO_UNIQUE_WITNESS_PLANS, FOUR_LOOP_CORNER_SHELL_GLOBAL_COLUMN_BOUND,
    FOUR_LOOP_CORNER_SHELL_RAW_ROWS, FourLoopBoundaryHaloCensusKey, FourLoopCornerColumnId,
    FourLoopCornerRawRowId, FourLoopCornerShellCertificate, FourLoopCornerShellConfig,
    FourLoopCornerShellError, FourLoopCornerShellStatus, FourLoopGenuineCornerType,
    FourLoopReferenceTopology, MassiveVacuumMaster, MasterProduct,
};

#[test]
fn native_160_row_corner_shell_is_a_replayable_complete_certificate() {
    // Structural limits are preflighted before any classifier or Symbolica
    // coefficient work.  Exercise that typed path in the same consolidated
    // test as the production certificate so this expensive shell builds once.
    let mut insufficient = FourLoopCornerShellConfig::default();
    insufficient.max_raw_rows = FOUR_LOOP_CORNER_SHELL_RAW_ROWS - 1;
    assert!(matches!(
        FourLoopCornerShellCertificate::build(insufficient),
        Err(FourLoopCornerShellError::ResourceLimit {
            resource: "native raw rows",
            requested: 160,
            limit: 159,
        })
    ));
    let mut insufficient_halo = FourLoopCornerShellConfig::default();
    insufficient_halo.boundary_halo.max_blocker_occurrences =
        FOUR_LOOP_BOUNDARY_HALO_BLOCKER_OCCURRENCES - 1;
    assert!(matches!(
        FourLoopCornerShellCertificate::build(insufficient_halo),
        Err(FourLoopCornerShellError::BoundaryHalo(
            rustred::FourLoopBoundaryHaloError::ResourceLimit {
                resource: "blocker occurrences",
                requested: 234,
                limit: 233,
            }
        ))
    ));

    let mut insufficient_formula_degree = FourLoopCornerShellConfig::default();
    insufficient_formula_degree
        .boundary_halo
        .max_coefficient_degree = 1;
    assert!(matches!(
        FourLoopCornerShellCertificate::build(insufficient_formula_degree),
        Err(FourLoopCornerShellError::BoundaryHalo(
            rustred::FourLoopBoundaryHaloError::ResourceLimit {
                resource: "configured coefficient exponent degree",
                requested: 2,
                limit: 1,
            }
        ))
    ));

    let certificate =
        FourLoopCornerShellCertificate::build(FourLoopCornerShellConfig::default()).unwrap();

    let expected_ids = FourLoopGenuineCornerType::ALL
        .into_iter()
        .flat_map(|corner_type| {
            (0_u8..4).flat_map(move |differentiated_loop| {
                (0_u8..4).map(move |contraction_loop| {
                    FourLoopCornerRawRowId::new(corner_type, differentiated_loop, contraction_loop)
                })
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(certificate.raw_row_ids(), expected_ids);
    assert_eq!(
        certificate.raw_row_ids().len(),
        FOUR_LOOP_CORNER_SHELL_RAW_ROWS
    );
    assert_eq!(
        certificate.normalized_rows().len() + certificate.blocked_rows().len(),
        FOUR_LOOP_CORNER_SHELL_RAW_ROWS
    );
    assert_eq!(
        certificate
            .raw_row_ids()
            .iter()
            .filter(|id| id.corner_type().reference_topology() == rustred::FourLoopTopology::H)
            .count(),
        144
    );
    assert_eq!(
        certificate
            .raw_row_ids()
            .iter()
            .filter(|id| id.corner_type().reference_topology() == rustred::FourLoopTopology::X)
            .count(),
        16
    );

    assert_eq!(certificate.status(), FourLoopCornerShellStatus::Complete);
    assert!(certificate.is_complete());
    assert!(certificate.blocked_rows().is_empty());
    assert!(certificate.blocker_census().is_empty());
    assert_eq!(certificate.blocker_term_count(), 0);
    assert_eq!(
        certificate.normalized_rows().len(),
        FOUR_LOOP_CORNER_SHELL_RAW_ROWS
    );
    let normalized_ids = certificate
        .normalized_rows()
        .iter()
        .map(|row| row.raw_id())
        .collect::<BTreeSet<_>>();
    assert_eq!(normalized_ids.len(), FOUR_LOOP_CORNER_SHELL_RAW_ROWS);

    let mut independently_counted = BTreeMap::<FourLoopBoundaryHaloCensusKey, usize>::new();
    for row in certificate.preclosure_blocked_rows() {
        assert!(!row.unsupported_boundary_halo().is_empty());
        for blocker in row.unsupported_boundary_halo() {
            assert_eq!(blocker.witness().product().unwrap(), *blocker.product());
            assert!(blocker.dot_degree() + blocker.numerator_degree() > 0);
            assert!(!blocker.coefficient().is_zero());
            assert!(blocker.stable_key().starts_with(
                "rustred-equal-mass-euclidean-four-loop-unsupported-boundary-halo-v1:"
            ));
            *independently_counted
                .entry(FourLoopBoundaryHaloCensusKey::from_blocker(blocker))
                .or_insert(0) += 1;
        }
    }
    assert_eq!(
        certificate.preclosure_blocker_census(),
        &independently_counted
    );
    assert_eq!(
        certificate.preclosure_blocker_term_count(),
        certificate
            .preclosure_blocked_rows()
            .iter()
            .map(|row| row.unsupported_boundary_halo().len())
            .sum::<usize>()
    );
    assert_eq!(certificate.preclosure_blocker_row_count(), 95);
    assert_eq!(certificate.preclosure_blocker_term_count(), 234);
    assert_eq!(certificate.boundary_halo_closures().len(), 234);
    assert!(
        certificate
            .preclosure_blocker_census()
            .keys()
            .all(|key| key.topology() == FourLoopReferenceTopology::H
                && key.dot_degree() == 1
                && key.numerator_degree() == 0)
    );
    let halo_stats = certificate.boundary_halo_stats();
    assert_eq!(
        halo_stats.blocker_occurrences(),
        FOUR_LOOP_BOUNDARY_HALO_BLOCKER_OCCURRENCES
    );
    assert_eq!(
        halo_stats.unique_witness_plans(),
        FOUR_LOOP_BOUNDARY_HALO_UNIQUE_WITNESS_PLANS
    );
    assert_eq!(
        halo_stats.signed_line_dispatches(),
        FOUR_LOOP_BOUNDARY_HALO_SIGNED_LINE_DISPATCHES
    );
    assert_eq!(
        halo_stats.formula_dispatches(),
        FOUR_LOOP_BOUNDARY_HALO_FORMULA_DISPATCHES
    );

    // Independently reconstruct every fixed direct-formula substitution from
    // the retained witness component owning the dot. This exhausts all 234
    // occurrences without relying on the reducer's private formula builder.
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mut covered_dispatches = BTreeSet::new();
    for closure in certificate.boundary_halo_closures() {
        let dotted_position = closure
            .blocker()
            .integral()
            .powers()
            .iter()
            .position(|power| *power == 2)
            .unwrap();
        let owner = closure
            .blocker()
            .witness()
            .components()
            .iter()
            .position(|component| {
                component
                    .signed_line_matches()
                    .iter()
                    .any(|line| line.physical_position() == dotted_position)
            })
            .unwrap();
        let component = &closure.blocker().witness().components()[owner];
        let position = component
            .signed_line_matches()
            .iter()
            .find(|line| line.physical_position() == dotted_position)
            .unwrap()
            .reference_position();
        assert_eq!(closure.dotted_component(), component.master());
        assert_eq!(closure.compact_reference_position(), position);
        covered_dispatches.insert((component.master(), position));

        let unaffected = MasterProduct::try_from_factors(
            closure
                .blocker()
                .witness()
                .components()
                .iter()
                .enumerate()
                .filter_map(|(index, other)| (index != owner).then_some(other.master())),
        )
        .unwrap();
        let factor = MasterProduct::from_factor;
        let local = match (component.master(), position) {
            (MassiveVacuumMaster::T1, 0) => vec![(
                factor(MassiveVacuumMaster::T1),
                coefficients.parse("(2-d)/2").unwrap(),
            )],
            (MassiveVacuumMaster::S2, 0..=2) => vec![(
                factor(MassiveVacuumMaster::S2),
                coefficients.parse("(3-d)/3").unwrap(),
            )],
            (MassiveVacuumMaster::B4, 0..=3) => vec![(
                factor(MassiveVacuumMaster::B4),
                coefficients.parse("(8-3*d)/8").unwrap(),
            )],
            (MassiveVacuumMaster::F5, 0) => vec![
                (
                    factor(MassiveVacuumMaster::B4),
                    coefficients.parse("(8-3*d)/6").unwrap(),
                ),
                (
                    MasterProduct::try_from_factors([
                        MassiveVacuumMaster::T1,
                        MassiveVacuumMaster::S2,
                    ])
                    .unwrap(),
                    coefficients.parse("2*(d-2)/3").unwrap(),
                ),
                (
                    factor(MassiveVacuumMaster::F5),
                    coefficients.parse("(6-d)/6").unwrap(),
                ),
            ],
            (MassiveVacuumMaster::F5, 1..=4) => vec![
                (
                    factor(MassiveVacuumMaster::B4),
                    coefficients.parse("(3*d-8)/24").unwrap(),
                ),
                (
                    MasterProduct::try_from_factors([
                        MassiveVacuumMaster::T1,
                        MassiveVacuumMaster::S2,
                    ])
                    .unwrap(),
                    coefficients.parse("(2-d)/6").unwrap(),
                ),
                (
                    factor(MassiveVacuumMaster::F5),
                    coefficients.parse("(3-d)/3").unwrap(),
                ),
            ],
            (MassiveVacuumMaster::M6, 0..=5) => vec![(
                factor(MassiveVacuumMaster::M6),
                coefficients.parse("(4-d)/4").unwrap(),
            )],
            other => panic!("unexpected direct-formula dispatch {other:?}"),
        };
        let expected = local
            .into_iter()
            .map(|(product, coefficient)| {
                (unaffected.checked_multiply(&product).unwrap(), coefficient)
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(closure.mass_normalized_output(), &expected);
    }
    assert!(covered_dispatches.contains(&(MassiveVacuumMaster::T1, 0)));
    assert!(covered_dispatches.contains(&(MassiveVacuumMaster::B4, 0)));
    assert!(covered_dispatches.contains(&(MassiveVacuumMaster::F5, 0)));
    assert!(
        covered_dispatches
            .iter()
            .any(|(master, position)| *master == MassiveVacuumMaster::F5 && *position > 0)
    );
    assert!(
        covered_dispatches
            .iter()
            .any(|(master, _)| *master == MassiveVacuumMaster::M6)
    );

    let all_columns = certificate
        .normalized_rows()
        .iter()
        .flat_map(|row| row.entries().keys().cloned())
        .collect::<BTreeSet<_>>();
    assert!(all_columns.len() <= FOUR_LOOP_CORNER_SHELL_GLOBAL_COLUMN_BOUND);
    let stable_keys = all_columns
        .iter()
        .map(FourLoopCornerColumnId::stable_key)
        .collect::<BTreeSet<_>>();
    assert_eq!(stable_keys.len(), all_columns.len());
    assert!(all_columns.iter().all(|column| match column {
        FourLoopCornerColumnId::Product(_) => column.stable_key().contains(":product:"),
        FourLoopCornerColumnId::Genuine { powers, .. } => {
            powers.len() == 10 && column.stable_key().contains(":genuine:")
        }
    }));

    let pivot_columns = certificate
        .pivots()
        .iter()
        .map(|rule| rule.pivot().clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(certificate.rank(), pivot_columns.len());
    assert!(
        certificate
            .pivots()
            .windows(2)
            .all(|pair| pair[0].pivot() > pair[1].pivot())
    );
    assert!(certificate.pivots().iter().all(|rule| {
        !rule.source_row_weights().is_empty()
            && rule.rhs().keys().all(|column| column < rule.pivot())
            && rule
                .source_row_weights()
                .keys()
                .all(|raw_id| normalized_ids.contains(raw_id))
    }));
    assert!(
        certificate
            .free_unresolved_columns()
            .iter()
            .all(|column| all_columns.contains(column) && !pivot_columns.contains(column))
    );
    assert!(
        certificate.normalization_contributions()
            <= FourLoopCornerShellConfig::default().max_normalization_contributions
    );
    assert!(
        certificate.elimination_updates()
            <= FourLoopCornerShellConfig::default().max_elimination_updates
    );

    // The constructor already performs this replay.  Re-run it independently
    // through the public certificate surface to catch provenance corruption.
    certificate.replay().unwrap();

    eprintln!(
        "four-loop corner shell: complete_rows={}, blocked_rows={}, preclosure_blocker_terms={}, preclosure_census_buckets={}, rank={}, free_unresolved={}, normalization_contributions={}, elimination_updates={}",
        certificate.normalized_rows().len(),
        certificate.blocked_rows().len(),
        certificate.preclosure_blocker_term_count(),
        certificate.preclosure_blocker_census().len(),
        certificate.rank(),
        certificate.free_unresolved_columns().len(),
        certificate.normalization_contributions(),
        certificate.elimination_updates(),
    );
    eprintln!(
        "four-loop preclosure blocker census: {:#?}",
        certificate.preclosure_blocker_census()
    );
}
