use crate::identity::row::RowId;

use super::super::{ParametricIbpError, ParametricIbpGenerator};
use super::support::coordinate_family;

#[test]
fn prepared_li_batch_is_empty_with_fewer_than_two_externals() {
    for externals in [0, 1] {
        let family = coordinate_family(
            &format!("li-empty-source-barrier-e{externals}"),
            2,
            externals,
        );
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let source_batch = generator.prepare_external_ibp_sources().unwrap();
        let source_rows = (0..source_batch.len())
            .map(|ordinal| source_batch.generate(ordinal))
            .collect();
        let sources = source_batch.complete(source_rows).unwrap();
        assert_eq!(
            generator
                .prepare_lorentz_invariance(&sources)
                .unwrap()
                .len(),
            0
        );
    }
}

#[test]
fn source_completion_seals_layout_scope_and_order_without_generator_identity() {
    let family = coordinate_family("sealed-source-validation", 1, 2);
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let equivalent_generator = ParametricIbpGenerator::try_new(&family).unwrap();

    // A separately prepared generator with the same semantic scope is a
    // valid source; pointer identity is deliberately irrelevant.
    let target = generator.prepare_ordinary_ibp().unwrap();
    let equivalent = equivalent_generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..equivalent.len())
        .map(|ordinal| equivalent.generate(ordinal))
        .collect();
    let completed = target.complete(rows).unwrap();
    assert!(generator.prepare_lorentz_invariance(&completed).is_ok());

    let short = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..short.len() - 1)
        .map(|ordinal| short.generate(ordinal))
        .collect();
    assert!(matches!(
        short.complete(rows),
        Err(ParametricIbpError::WrongSourceRowCount {
            batch: "ordinary IBP source",
            expected: 3,
            actual: 2,
        })
    ));

    let reordered = generator.prepare_ordinary_ibp().unwrap();
    let mut rows = (0..reordered.len())
        .map(|ordinal| reordered.generate(ordinal))
        .collect::<Vec<_>>();
    rows.swap(0, 1);
    assert!(matches!(
        reordered.complete(rows),
        Err(ParametricIbpError::SourceRowOrdinalMismatch {
            batch: "ordinary IBP source",
            position: 0,
            actual: 1,
        })
    ));

    let wrong_layout = generator.prepare_ordinary_ibp().unwrap();
    let ordinary_source = equivalent_generator.prepare_ordinary_ibp().unwrap();
    let external_source = equivalent_generator.prepare_external_ibp_sources().unwrap();
    let mut rows = (0..ordinary_source.len())
        .map(|ordinal| ordinary_source.generate(ordinal))
        .collect::<Vec<_>>();
    rows[0] = external_source.generate(0);
    assert!(matches!(
        wrong_layout.complete(rows),
        Err(ParametricIbpError::SourceRowLayoutMismatch {
            position: 0,
            expected: "ordinary IBP source",
            actual: "external-contraction IBP source",
        })
    ));

    let foreign_family = coordinate_family("foreign-li-source", 1, 2);
    let foreign_generator = ParametricIbpGenerator::try_new(&foreign_family).unwrap();
    let target = generator.prepare_ordinary_ibp().unwrap();
    let foreign_batch = foreign_generator.prepare_ordinary_ibp().unwrap();
    let foreign_rows = (0..foreign_batch.len())
        .map(|ordinal| foreign_batch.generate(ordinal))
        .collect();
    assert!(matches!(
        target.complete(foreign_rows),
        Err(ParametricIbpError::SourceRowScopeMismatch {
            batch: "ordinary IBP source",
            position: 0,
        })
    ));

    let foreign_batch = foreign_generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..foreign_batch.len())
        .map(|ordinal| foreign_batch.generate(ordinal))
        .collect();
    let foreign_completed = foreign_batch.complete(rows).unwrap();
    assert!(matches!(
        generator.prepare_lorentz_invariance(&foreign_completed),
        Err(ParametricIbpError::CompletedSourceScopeMismatch)
    ));
}

#[test]
fn li_only_source_batch_contains_exactly_external_contractions() {
    let family = coordinate_family("dense-external-sources", 2, 3);
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_external_ibp_sources().unwrap();
    assert_eq!(batch.len(), 6);
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let sources = batch.complete(rows).unwrap();
    let li_batch = generator.prepare_lorentz_invariance(&sources).unwrap();
    let li_rows = (0..li_batch.len())
        .map(|ordinal| li_batch.generate(ordinal))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    drop(li_batch);
    let source_rows = sources.into_relations();
    assert_eq!(
        source_rows
            .iter()
            .map(|row| row.row_id().clone())
            .collect::<Vec<_>>(),
        vec![
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
    assert_eq!(li_rows.len(), 3);
    assert_eq!(
        li_rows
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
}
