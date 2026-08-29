use super::super::{IndexShift, IndexSpace, ParametricRelationError};

#[test]
fn fallible_index_construction_preserves_checked_semantics() {
    let space = IndexSpace::try_new(2).unwrap();
    assert_eq!(space.try_zero().unwrap().values(), &[0, 0]);
    assert_eq!(space.unit(1, -1).unwrap().values(), &[0, -1]);
    assert_eq!(
        IndexShift::try_new([2, -3], 2)
            .unwrap()
            .checked_add(&IndexShift::try_new([-1, 5], 2).unwrap())
            .unwrap()
            .values(),
        &[1, 2]
    );
    assert!(matches!(
        IndexShift::try_new([i64::MAX], 1)
            .unwrap()
            .checked_add(&IndexShift::try_new([1], 1).unwrap()),
        Err(ParametricRelationError::IndexOverflow { position: 0 })
    ));
    // This is rejected by Vec's capacity arithmetic without attempting a
    // material allocation, exercising the checked internal allocation path.
    assert!(matches!(
        IndexSpace::try_new(usize::MAX).unwrap().try_zero(),
        Err(ParametricRelationError::AllocationFailure {
            resource: "zero index-shift components",
            requested: usize::MAX,
        })
    ));
}

#[test]
fn overlong_index_shift_is_rejected_without_draining_the_iterator() {
    struct PanicIfPolledAfterFirstExtra {
        polls: usize,
    }

    impl Iterator for PanicIfPolledAfterFirstExtra {
        type Item = i64;

        fn next(&mut self) -> Option<Self::Item> {
            self.polls += 1;
            match self.polls {
                1 => Some(7),
                2 => Some(11),
                _ => panic!("an overlong shift iterator was drained after arity was known"),
            }
        }
    }

    assert_eq!(
        IndexShift::try_new(PanicIfPolledAfterFirstExtra { polls: 0 }, 1),
        Err(ParametricRelationError::WrongArity {
            expected: 1,
            actual: 2,
        })
    );
}
