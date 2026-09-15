//! Arithmetic verifier tests, not alternate source/rule constructors.
//! The maps below are test inputs only; the real caller must retain original
//! source authority and every pre-cancellation guard separately.

use crate::algebra::{CoefficientContext, IndexedCoefficient, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::source_port::geometry;
use crate::foundry::completion::LatticeBox;
use crate::identity::ParametricIbpGenerator;
use crate::sector::{Mask, zero};

use super::*;

const SECTOR: [bool; 3] = [true, true, false];

struct Fixture {
    context: IndexedCoefficientContext,
    zeros: Vec<[bool; 3]>,
    indices: [usize; 3],
}

impl Fixture {
    fn new() -> Self {
        let base = CoefficientContext::new(["d"]);
        let family = IntegralFamily::new(
            "refined-original-replay-sunset",
            vec!["k0".into(), "k1".into()],
            Vec::new(),
            base.clone(),
            base.parameter("d").unwrap(),
            [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
                .into_iter()
                .map(|row| {
                    AffineDenominator::new(
                        base.integer(-1),
                        row.into_iter().map(|value| base.integer(value)).collect(),
                    )
                })
                .collect(),
            Vec::new(),
            vec![base.zero(); 3],
        )
        .unwrap();
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let analyzer = zero::Analyzer::try_unrestricted(&family).unwrap();
        let zeros: Vec<_> = (0_u8..8)
            .map(|bits| std::array::from_fn(|axis| bits & (1 << axis) != 0))
            .filter(|mask| {
                matches!(
                    analyzer.analyze(&Mask::try_new(*mask).unwrap()).unwrap(),
                    zero::Decision::ProvedZero(_)
                )
            })
            .collect();
        assert_eq!(zeros.len(), 4);
        assert!(!zeros.contains(&SECTOR));
        assert!(!zeros.contains(&[true, true, true]));
        let indices = std::array::from_fn(|axis| context.base().parameter_names().len() + axis);
        Self {
            context,
            zeros,
            indices,
        }
    }

    fn original(
        &self,
        entries: impl IntoIterator<Item = ([i64; 3], IndexedCoefficient)>,
    ) -> OriginalCombination {
        let columns = entries
            .into_iter()
            .map(|(shift, coefficient)| (key(shift), coefficient))
            .collect::<BTreeMap<_, _>>();
        OriginalCombination {
            columns,
            guards: Vec::new(),
            // These tests exercise the arithmetic verifier directly and do
            // not claim that an original-source multiplication was performed.
            source_rows_multiplied: 0,
        }
    }

    fn verify(
        &self,
        original: &OriginalCombination,
        rhs: &[(IndexShift, IndexedCoefficient)],
        cell: &LatticeBox,
    ) -> Result<usize, SourcePortAuditError> {
        verify::<3>(&self.context, original, rhs, |shift, coefficient| {
            geometry::uniformly_zero_wide(
                shift,
                Some((coefficient.raw(), &self.indices)),
                std::slice::from_ref(cell),
                &SECTOR,
                &self.zeros,
            )
        })
    }

    fn base(&self) -> OriginalCombination {
        self.original([
            ([0, 0, 0], self.context.one()),
            ([-1, 0, 0], self.context.integer(-2)),
        ])
    }

    fn rhs(&self) -> Vec<(IndexShift, IndexedCoefficient)> {
        vec![(key([-1, 0, 0]), self.context.integer(2))]
    }
}

fn key(shift: [i64; 3]) -> IndexShift {
    IndexShift::try_new(shift, 3).unwrap()
}

fn cell(lower: [u64; 3], upper: [Option<u64>; 3]) -> LatticeBox {
    LatticeBox::try_new(lower, upper).unwrap()
}

#[test]
fn exact_full_residual_passes_and_desired_rhs_mutation_fails() {
    let f = Fixture::new();
    let domain = cell([1, 0, 0], [None; 3]);
    let original = f.base();
    assert_eq!(f.verify(&original, &f.rhs(), &domain).unwrap(), 2);
    let wrong_rhs = [(key([-1, 0, 0]), f.context.integer(3))];
    assert!(f.verify(&original, &wrong_rhs, &domain).is_err());
    assert_eq!(original.columns[&key([-1, 0, 0])], f.context.integer(-2));
}

#[test]
fn an_omitted_nonzero_physical_column_cannot_disappear_from_replay() {
    let f = Fixture::new();
    let domain = cell([1, 1, 0], [None; 3]);
    let mut original = f.base();
    original
        .columns
        .insert(key([0, -1, 0]), f.context.integer(3));
    let mut complete_rhs = f.rhs();
    complete_rhs.push((key([0, -1, 0]), f.context.integer(-3)));
    assert_eq!(f.verify(&original, &complete_rhs, &domain).unwrap(), 3);
    let failure = f.verify(&original, &f.rhs(), &domain).unwrap_err();
    assert!(failure.to_string().contains("[0, -1, 0]"));
    assert!(original.columns.contains_key(&key([0, -1, 0])));
}

#[test]
fn finite_activation_checks_both_minus_one_and_zero_not_a_single_sample() {
    let f = Fixture::new();
    let n = f.context.index(2).unwrap();
    let n_plus_one = f.context.add(&n, &f.context.one()).unwrap();
    let mut original = f.base();
    original
        .columns
        .insert(key([0, 0, 2]), f.context.mul(&n, &n_plus_one).unwrap());
    // Local x2=0..1 is physical n2=0,-1. The shifted third power
    // n2+2 is positive at BOTH points; no zero sector may erase it.
    let both = cell([1, 0, 0], [None, None, Some(1)]);
    assert_eq!(f.verify(&original, &f.rhs(), &both).unwrap(), 3);
    original.columns.insert(key([0, 0, 2]), n_plus_one);
    let only_minus_one = cell([1, 0, 1], [None, None, Some(1)]);
    assert!(f.verify(&original, &f.rhs(), &only_minus_one).is_ok());
    let failure = f.verify(&original, &f.rhs(), &both).unwrap_err();
    assert!(failure.to_string().contains("[0, 0, 2]"));
    let only_zero = cell([1, 0, 0], [None, None, Some(0)]);
    assert!(f.verify(&original, &f.rhs(), &only_zero).is_err());
}

#[test]
fn a_nonzero_target_cannot_be_replaced_by_a_zero_projection_or_rhs_entry() {
    let f = Fixture::new();
    let domain = cell([1, 0, 0], [None; 3]);
    let mut original = f.base();
    original.columns.remove(&key([0, 0, 0]));
    let failure = f.verify(&original, &f.rhs(), &domain).unwrap_err();
    assert!(failure.to_string().contains("[0, 0, 0]"));
    // Also reject a target placed in the RHS before the zero-product
    // callback; the callback is not a way to authorize self-reduction.
    let bad_rhs = [(key([0, 0, 0]), f.context.one())];
    assert!(
        verify::<3>(&f.context, &f.base(), &bad_rhs, |_, _| {
            panic!("a target-containing RHS must fail canonical admission first")
        })
        .is_err()
    );
}

#[test]
fn a_singular_denominator_is_not_a_zero_on_a_finite_activation_face() {
    let f = Fixture::new();
    let n = f.context.index(2).unwrap();
    let n_plus_one = f.context.add(&n, &f.context.one()).unwrap();
    let mut original = f.base();
    original
        .columns
        .insert(key([0, 0, 2]), f.context.div(&n, &n_plus_one).unwrap());
    // n/(n+1) really vanishes at n=0, but is undefined at n=-1.
    // This deliberately bypasses separate guard validation to ensure the
    // zero-product check itself cannot turn its pole into a valid zero.
    let only_zero = cell([1, 0, 0], [None, None, Some(0)]);
    assert!(f.verify(&original, &f.rhs(), &only_zero).is_ok());
    let both = cell([1, 0, 0], [None, None, Some(1)]);
    assert!(f.verify(&original, &f.rhs(), &both).is_err());
    let pole = cell([1, 0, 1], [None, None, Some(1)]);
    assert!(f.verify(&original, &f.rhs(), &pole).is_err());
}

#[test]
fn authenticated_zero_sectors_can_remove_only_the_actual_full_residual_product() {
    let f = Fixture::new();
    let domain = cell([0, 0, 0], [Some(0), Some(0), None]);
    // Both positive target powers are exactly1. Removing both leaves
    // the genuinely scaleless sector000, independently proved above.
    let mut original = f.original([
        ([0, 0, 0], f.context.one()),
        ([1, 0, 0], f.context.integer(-2)),
        ([-1, -1, 0], f.context.integer(7)),
    ]);
    let rhs = [(key([1, 0, 0]), f.context.integer(2))];
    assert_eq!(f.verify(&original, &rhs, &domain).unwrap(), 3);
    assert!(original.columns.contains_key(&key([-1, -1, 0])));
    // A different physical product is nonzero, including its activating
    // point at n2=0. Its unrelated coefficient cannot borrow the zero proof.
    original.columns.remove(&key([-1, -1, 0]));
    original
        .columns
        .insert(key([0, 0, 1]), f.context.integer(7));
    assert!(f.verify(&original, &rhs, &domain).is_err());
}
