use super::super::{CutConstraint, Error, Mask, Pattern, PatternSlot};

struct ImpossibleExactSizeHint<T> {
    value: Option<T>,
}

impl<T> ImpossibleExactSizeHint<T> {
    fn one(value: T) -> Self {
        Self { value: Some(value) }
    }
}

impl<T> Iterator for ImpossibleExactSizeHint<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.value.take()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, Some(usize::MAX))
    }
}

#[test]
fn fallible_foundation_allocations_are_typed_and_report_requested_entries() {
    assert!(matches!(
        Mask::try_new(ImpossibleExactSizeHint::one(true)),
        Err(Error::AllocationFailure {
            resource: "sector mask bits",
            requested: usize::MAX,
        })
    ));
    assert!(matches!(
        Pattern::try_new(ImpossibleExactSizeHint::one(PatternSlot::Any)),
        Err(Error::AllocationFailure {
            resource: "sector pattern slots",
            requested: usize::MAX,
        })
    ));
    assert!(matches!(
        CutConstraint::try_from_positions(usize::MAX, std::iter::empty()),
        Err(Error::AllocationFailure {
            resource: "cut active mask",
            requested: usize::MAX,
        })
    ));

    let allocation_error = Error::AllocationFailure {
        resource: "test payload",
        requested: 17,
    };
    assert_eq!(
        allocation_error.to_string(),
        "could not reserve 17 bounded entries for test payload"
    );
}
