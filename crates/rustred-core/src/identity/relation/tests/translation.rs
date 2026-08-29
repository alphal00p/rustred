use crate::algebra::{CoefficientContext, IndexedAlgebraLimits, IndexedCoefficientContext};
use crate::identity::row::RowId;

use super::super::{Builder, IndexShift, IndexSpace, RelationLimits};

#[test]
fn translation_moves_keys_and_coefficient_indices_together() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "relation-translate", 2).unwrap();
    let space = IndexSpace::try_new(2).unwrap();
    let mut relation = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: "source".into(),
        },
        &context,
    );
    relation
        .add_term(
            &context,
            space.try_zero().unwrap(),
            context.index(0).unwrap(),
            RelationLimits::default(),
        )
        .unwrap();
    let relation = relation.finish();
    let translation = IndexShift::try_new([2, -1], 2).unwrap();
    let translated = relation
        .translated(
            &context,
            &translation,
            RowId::Derived {
                label: "translated".into(),
            },
            RelationLimits::default(),
        )
        .unwrap();
    let (shift, coefficient) = translated.terms().first_key_value().unwrap();
    assert_eq!(shift.values(), &[2, -1]);
    let (coefficient, denominator_nonzero) = context
        .specialize(coefficient, &[3, 7], IndexedAlgebraLimits::default())
        .unwrap();
    assert_eq!(coefficient, base.integer(5));
    assert!(denominator_nonzero.is_none());
}

#[test]
fn translation_composes_exactly() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "relation-compose", 2).unwrap();
    let space = IndexSpace::try_new(2).unwrap();
    let mut source = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: "source".into(),
        },
        &context,
    );
    source
        .add_term(
            &context,
            space.unit(1, 1).unwrap(),
            context.index(0).unwrap(),
            RelationLimits::default(),
        )
        .unwrap();
    let source = source.finish();
    let s = IndexShift::try_new([1, -2], 2).unwrap();
    let t = IndexShift::try_new([-4, 3], 2).unwrap();
    let st = s.checked_add(&t).unwrap();
    let sequential = source
        .translated(
            &context,
            &s,
            RowId::Derived { label: "s".into() },
            RelationLimits::default(),
        )
        .unwrap()
        .translated(
            &context,
            &t,
            RowId::Derived { label: "st".into() },
            RelationLimits::default(),
        )
        .unwrap();
    let direct = source
        .translated(
            &context,
            &st,
            RowId::Derived { label: "st".into() },
            RelationLimits::default(),
        )
        .unwrap();
    assert_eq!(sequential.terms(), direct.terms());
    assert_eq!(sequential.nonzero_conditions(), direct.nonzero_conditions());
}
