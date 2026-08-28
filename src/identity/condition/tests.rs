use super::IdentityConditionSource;
use crate::identity::RowId;

#[test]
fn stable_string_pins_nested_row_source_encoding() {
    let source = IdentityConditionSource::RelationTranslation {
        source_row: RowId::Derived {
            label: "a:b".into(),
        },
        target_row: RowId::OrdinaryIbp {
            contraction_momentum: 3,
            differentiated_loop: 2,
        },
        offset: vec![-1, 2].into_boxed_slice(),
    };
    assert_eq!(
        source.stable_string(),
        "relation-translation:derived:3:a:b:ordinary-ibp:3:2:[-1,2]"
    );
}
