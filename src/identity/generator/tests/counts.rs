use crate::algebra::CoefficientContext;
use crate::family::IntegralFamily;
use crate::identity::row::RowId;

use super::super::{ParametricIbpError, ParametricIbpGenerator};
use super::support::{coordinate_family, identity_denominators};

#[test]
fn sentinel_topology_neutral_source_counts_cover_one_two_and_six_loops() {
    for (loops, ordinary_count) in [(1, 3), (2, 8)] {
        let family = coordinate_family(&format!("li-sentinel-l{loops}"), loops, 2);
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let ordinary_batch = generator.prepare_ordinary_ibp().unwrap();
        let ordinary_rows = (0..ordinary_batch.len())
            .map(|ordinal| ordinary_batch.generate(ordinal))
            .collect();
        let ordinary = ordinary_batch.complete(ordinary_rows).unwrap();
        let li_batch = generator.prepare_lorentz_invariance(&ordinary).unwrap();
        let lorentz_invariance = (0..li_batch.len())
            .map(|ordinal| li_batch.generate(ordinal))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        drop(li_batch);
        let ordinary = ordinary.into_relations();

        assert_eq!(ordinary.len(), ordinary_count);
        assert_eq!(lorentz_invariance.len(), 1);
        assert_eq!(
            lorentz_invariance[0].row_id(),
            &RowId::LorentzInvariance {
                first_external: 0,
                second_external: 1,
            }
        );
        assert_eq!(
            ordinary.len() + lorentz_invariance.len(),
            ordinary_count + 1
        );
    }

    let family = coordinate_family("ordinary-source-sentinel-l6-k21", 6, 0);
    assert_eq!(family.denominator_count(), 21);
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let row_results = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let rows = batch.complete(row_results).unwrap().into_relations();
    assert_eq!(rows.len(), 36);
    for (ordinal, row) in rows.iter().enumerate() {
        assert_eq!(
            row.row_id(),
            &RowId::OrdinaryIbp {
                contraction_momentum: ordinal / 6,
                differentiated_loop: ordinal % 6,
            }
        );
    }
}

#[test]
fn two_loop_rows_are_q_major_and_li_pairs_are_lexicographic() {
    let base = CoefficientContext::new(["d", "s00", "s01", "s02", "s11", "s12", "s22", "nu"]);
    let family = IntegralFamily::new(
        "two-loop-three-leg-structure",
        vec!["k0".into(), "k1".into()],
        vec!["p0".into(), "p1".into(), "p2".into()],
        base.clone(),
        base.parameter("d").unwrap(),
        identity_denominators(&base, vec![base.zero(); 9]),
        vec![
            vec![
                base.parameter("s00").unwrap(),
                base.parameter("s01").unwrap(),
                base.parameter("s02").unwrap(),
            ],
            vec![
                base.parameter("s01").unwrap(),
                base.parameter("s11").unwrap(),
                base.parameter("s12").unwrap(),
            ],
            vec![
                base.parameter("s02").unwrap(),
                base.parameter("s12").unwrap(),
                base.parameter("s22").unwrap(),
            ],
        ],
        vec![
            base.parameter("nu").unwrap(),
            base.zero(),
            base.zero(),
            base.zero(),
            base.zero(),
            base.zero(),
            base.zero(),
            base.zero(),
            base.zero(),
        ],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let ordinary_batch = generator.prepare_ordinary_ibp().unwrap();
    assert_eq!(ordinary_batch.len(), 10);
    assert!(matches!(
        ordinary_batch.generate(10),
        Err(ParametricIbpError::RowOrdinalOutOfRange {
            batch: "ordinary IBP source",
            ordinal: 10,
            rows: 10,
        })
    ));
    let ordinary_rows = (0..ordinary_batch.len())
        .map(|ordinal| ordinary_batch.generate(ordinal))
        .collect();
    let ordinary = ordinary_batch.complete(ordinary_rows).unwrap();
    let li_batch = generator.prepare_lorentz_invariance(&ordinary).unwrap();
    assert_eq!(li_batch.len(), 3);
    assert!(matches!(
        li_batch.generate(3),
        Err(ParametricIbpError::RowOrdinalOutOfRange {
            batch: "Lorentz-invariance",
            ordinal: 3,
            rows: 3,
        })
    ));
    let lorentz_invariance = (0..li_batch.len())
        .map(|ordinal| li_batch.generate(ordinal))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    drop(li_batch);
    let ordinary = ordinary.into_relations();

    let ids = ordinary
        .iter()
        .map(|row| row.row_id().clone())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            RowId::OrdinaryIbp {
                contraction_momentum: 0,
                differentiated_loop: 0,
            },
            RowId::OrdinaryIbp {
                contraction_momentum: 0,
                differentiated_loop: 1,
            },
            RowId::OrdinaryIbp {
                contraction_momentum: 1,
                differentiated_loop: 0,
            },
            RowId::OrdinaryIbp {
                contraction_momentum: 1,
                differentiated_loop: 1,
            },
            RowId::OrdinaryIbp {
                contraction_momentum: 2,
                differentiated_loop: 0,
            },
            RowId::OrdinaryIbp {
                contraction_momentum: 2,
                differentiated_loop: 1,
            },
            RowId::OrdinaryIbp {
                contraction_momentum: 3,
                differentiated_loop: 0,
            },
            RowId::OrdinaryIbp {
                contraction_momentum: 3,
                differentiated_loop: 1,
            },
            RowId::OrdinaryIbp {
                contraction_momentum: 4,
                differentiated_loop: 0,
            },
            RowId::OrdinaryIbp {
                contraction_momentum: 4,
                differentiated_loop: 1,
            },
        ]
    );
    assert_eq!(
        lorentz_invariance
            .iter()
            .map(|row| row.row_id().clone())
            .collect::<Vec<_>>(),
        vec![
            RowId::LorentzInvariance {
                first_external: 0,
                second_external: 1,
            },
            RowId::LorentzInvariance {
                first_external: 0,
                second_external: 2,
            },
            RowId::LorentzInvariance {
                first_external: 1,
                second_external: 2,
            },
        ]
    );
    assert_eq!(ordinary.len() + lorentz_invariance.len(), 13);
}
