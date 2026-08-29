use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::identity::condition::ParametricNonZeroCondition;
use crate::identity::row::RowId;

use super::super::{Builder, IndexSpace, RelationLimits};

pub(super) fn actual_input_denominator_condition(
    scope: &str,
) -> (IndexedCoefficientContext, ParametricNonZeroCondition) {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, scope, 1).unwrap();
    let mut relation = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: scope.into(),
        },
        &context,
    );
    relation
        .add_term(
            &context,
            IndexSpace::try_new(1).unwrap().try_zero().unwrap(),
            context.lift(&base.coefficient_fixture("1/x")).unwrap(),
            RelationLimits::default(),
        )
        .unwrap();
    let relation = relation.finish();
    assert_eq!(relation.nonzero_conditions().len(), 1);
    (context, relation.nonzero_conditions()[0].clone())
}
