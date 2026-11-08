use symbolica::domains::finite_field::Zp64;
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::prelude::Integer;

use super::super::field::{self, DenseRational, Poly};
use super::super::{CONSTANT_FLAG, CoefRef, Op};
use super::*;
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};

/// The first three primes above `2^62` (the range of the lift's primes).
const LARGE: [u64; 3] = [
    4_611_686_018_427_388_039,
    4_611_686_018_427_388_073,
    4_611_686_018_427_388_081,
];

/// A straight-line trace written by hand, evaluated exactly alongside.
struct Program {
    context: CoefficientContext,
    constants: Vec<(Integer, Integer)>,
    polynomials: Vec<DenseRational>,
    builder: BasisBuilder,
    ops: Vec<Op>,
    weights: Vec<Coefficient>,
    terminals: Vec<Coefficient>,
    active: Coefficient,
}

impl Program {
    fn new() -> Self {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        Self {
            constants: vec![],
            polynomials: vec![],
            builder: BasisBuilder::default(),
            ops: vec![],
            weights: vec![],
            terminals: vec![],
            active: context.zero(),
            context,
        }
    }

    fn intern(&mut self, text: &str) -> (CoefRef, Coefficient) {
        let coefficient = self.context.coefficient_fixture(text);
        let dense = DenseRational::from_coefficient(&coefficient);
        let reference = if dense.is_constant() {
            self.constants.push((
                coefficient.numerator.get_constant(),
                coefficient.denominator.get_constant(),
            ));
            CoefRef((self.constants.len() - 1) as u32 | CONSTANT_FLAG)
        } else {
            self.builder.push(&coefficient).unwrap();
            self.polynomials.push(dense);
            CoefRef((self.polynomials.len() - 1) as u32)
        };
        (reference, coefficient)
    }

    fn add(&self, a: &Coefficient, b: &Coefficient) -> Coefficient {
        self.context
            .try_add(a, b, ExactAlgebraLimits::default())
            .unwrap()
    }

    fn mul(&self, a: &Coefficient, b: &Coefficient) -> Coefficient {
        self.context
            .try_mul(a, b, ExactAlgebraLimits::default())
            .unwrap()
    }

    fn slot(values: &mut Vec<Coefficient>, slot: usize, zero: &Coefficient) {
        if values.len() <= slot {
            values.resize(slot + 1, zero.clone());
        }
    }

    fn init(&mut self, slot: usize, text: &str) {
        let (reference, value) = self.intern(text);
        Self::slot(&mut self.weights, slot, &self.context.zero());
        self.weights[slot] = value;
        self.ops.push(Op::new(OpTag::Init, slot as u32, reference));
    }

    fn pop(&mut self, slot: usize) {
        self.active = std::mem::replace(&mut self.weights[slot], self.context.zero());
        self.ops
            .push(Op::new(OpTag::Pop, slot as u32, CoefRef(CONSTANT_FLAG)));
    }

    /// `New`/`Add` (or `TermNew`/`TermAdd` when `terminal`) of `active * c`.
    fn emit(&mut self, terminal: bool, slot: usize, text: &str) {
        let (reference, value) = self.intern(text);
        let product = self.mul(&self.active, &value);
        let zero = self.context.zero();
        let values = if terminal {
            &mut self.terminals
        } else {
            &mut self.weights
        };
        Self::slot(values, slot, &zero);
        let current = values[slot].clone();
        let fresh = current.is_zero();
        let sum = self.add(&current, &product);
        let values = if terminal {
            &mut self.terminals
        } else {
            &mut self.weights
        };
        values[slot] = sum;
        let tag = match (terminal, fresh) {
            (false, true) => OpTag::New,
            (false, false) => OpTag::Add,
            (true, true) => OpTag::TermNew,
            (true, false) => OpTag::TermAdd,
        };
        self.ops.push(Op::new(tag, slot as u32, reference));
    }

    /// Record an erasure; `exact` asserts that the erased value is zero.
    fn cancel(&mut self, terminal: bool, slot: usize, exact: bool) {
        let values = if terminal {
            &mut self.terminals
        } else {
            &mut self.weights
        };
        assert_eq!(values[slot].is_zero(), exact);
        values[slot] = self.context.zero();
        let tag = if terminal {
            OpTag::TermCancel
        } else {
            OpTag::Cancel
        };
        self.ops
            .push(Op::new(tag, slot as u32, CoefRef(CONSTANT_FLAG)));
    }

    /// The trace, its basis, and the exact terminal values.
    fn finish(self) -> (Trace, Basis, Vec<Coefficient>) {
        let trace = Trace {
            ops: self.ops,
            constants: self.constants,
            polynomials: self.polynomials,
            slots: self.weights.len(),
            terminal_slots: self.terminals.len(),
            phase_starts: Vec::new(),
        };
        (trace, self.builder.finish(), self.terminals)
    }
}

/// The reduced image of an exact coefficient modulo `prime`.
fn reduce(coefficient: &Coefficient, prime: u64) -> Image {
    let f = Zp64::new(prime);
    let dense = DenseRational::from_coefficient(coefficient);
    let mut numerator: Poly = dense
        .numerator
        .iter()
        .map(|c| field::reduce(&f, c))
        .collect();
    let mut denominator: Poly = dense
        .denominator
        .iter()
        .map(|c| field::reduce(&f, c))
        .collect();
    field::trim(&f, &mut numerator);
    field::trim(&f, &mut denominator);
    if numerator.is_empty() {
        return super::super::lift::image_from(&f, &[], &[f.one()]);
    }
    let gcd = field::gcd(&f, &numerator, &denominator);
    numerator = field::quot_rem(&f, &numerator, &gcd).0;
    denominator = field::quot_rem(&f, &denominator, &gcd).0;
    let lead = f.inv(denominator.last().unwrap());
    for c in numerator.iter_mut().chain(denominator.iter_mut()) {
        *c = f.mul(&*c, &lead);
    }
    super::super::lift::image_from(&f, &numerator, &denominator)
}

/// Replay with each representation and the given lane groups; returns the
/// images per prime (in `primes` order).
fn replay_images(
    trace: &Trace,
    basis: &Basis,
    outputs: &[usize],
    primes: &[u64],
    lanes: usize,
    denominators: PolynomialDenominators,
) -> Result<Vec<Vec<Image>>, ReductionError> {
    let mut images = vec![];
    for group in primes.chunks(lanes) {
        let tables: Vec<LaneTables> = group
            .iter()
            .map(|&prime| LaneTables::new(prime, trace, basis).expect("good prime"))
            .collect();
        let replayed = match denominators {
            PolynomialDenominators::Factorized => replay(
                trace,
                outputs,
                tables.len(),
                &mut Factorized::new(&tables, basis, 1 << 14),
            ),
            PolynomialDenominators::Dense => replay(
                trace,
                outputs,
                tables.len(),
                &mut Dense::new(&tables, basis, 1 << 14),
            ),
        }?;
        images.extend(replayed.images);
    }
    Ok(images)
}

/// Mixed linear, repeated and nonlinear denominators, polynomial numerators,
/// merges, erasures and terminals.
fn mixed_program() -> Program {
    let mut p = Program::new();
    p.init(0, "(d^2-3)/(d-1)");
    p.init(1, "5/7");
    p.pop(0);
    p.emit(false, 2, "(2*d+1)/((d+2)^2*(d^2+1))");
    p.emit(false, 3, "d^3-4*d+11");
    p.emit(true, 0, "1/(d-1)");
    p.emit(false, 1, "(d-1)/(d^3-2)");
    p.pop(1);
    p.emit(false, 2, "-3/(d^2-3)^2");
    p.emit(false, 3, "(d^2+1)/(d+2)");
    p.emit(true, 1, "7*d/(d^2+d+3)");
    p.pop(2);
    p.emit(false, 4, "(d+5)*(d-1)/(d^2+1)");
    p.emit(true, 0, "(d^2-3)^2");
    p.emit(true, 2, "(4*d^2-1)/(3*d+5)");
    p.pop(3);
    p.emit(false, 4, "-2/(d-1)^3");
    p.emit(true, 1, "d^4");
    p.emit(true, 2, "-(4*d^2-1)/(3*d+5)");
    p.pop(4);
    p.emit(true, 0, "(d^3-2)/(d-7)");
    p
}

#[test]
fn partial_fractions_match_exact_arithmetic() {
    let program = mixed_program();
    let (trace, basis, terminals) = program.finish();
    // d-1, d+2, d^2+1, d^3-2, d^2-3, d+2 (again), d^2+d+3, 3d+5, d-7.
    assert_eq!((basis.factor_count(), basis.linear_count()), (8, 4));
    let outputs: Vec<usize> = (0..terminals.len()).collect();
    let expected: Vec<Vec<Image>> = LARGE
        .iter()
        .map(|&prime| terminals.iter().map(|t| reduce(t, prime)).collect())
        .collect();
    for lanes in [1, 2, 3] {
        for denominators in [
            PolynomialDenominators::Factorized,
            PolynomialDenominators::Dense,
        ] {
            let images =
                replay_images(&trace, &basis, &outputs, &LARGE, lanes, denominators).unwrap();
            assert_eq!(images, expected, "{lanes} {denominators:?}");
        }
    }
}

#[test]
fn linear_and_nonlinear_poles_cancel_exactly() {
    let mut p = Program::new();
    p.init(0, "1");
    p.pop(0);
    // A double pole that cancels to a simple one, a nonlinear pole that
    // cancels completely, and a merged frontier weight that cancels exactly.
    p.emit(true, 0, "1/(d-1)^2");
    p.emit(true, 0, "-1/(d-1)^2");
    p.emit(true, 0, "3/(d-1)");
    p.emit(true, 1, "(d+1)/(d^2+2)");
    p.emit(true, 1, "-(d+1)/(d^2+2)+2");
    p.emit(false, 1, "(d^2+5)/((d-3)*(d^2+2))");
    p.emit(false, 1, "-(d^2+5)/((d-3)*(d^2+2))");
    p.cancel(false, 1, true);
    p.emit(false, 2, "1/(d^2+2)^2");
    p.pop(2);
    p.emit(true, 2, "(d^2+2)^2");
    let (trace, basis, terminals) = p.finish();
    let outputs = [0, 1, 2];
    for &prime in &LARGE {
        let images = replay_images(
            &trace,
            &basis,
            &outputs,
            &[prime],
            1,
            PolynomialDenominators::Factorized,
        )
        .unwrap()
        .pop()
        .unwrap();
        let expected: Vec<Image> = terminals.iter().map(|t| reduce(t, prime)).collect();
        assert_eq!(images, expected);
        // 3/(d-1), 2 and 1.
        assert_eq!(images[0].degrees(), Some((0, 1)));
        assert_eq!(images[1].degrees(), Some((0, 0)));
        assert_eq!(images[2], reduce(&terminals[2], prime));
        assert_eq!(images[2].numerator, vec![1]);
    }
}

#[test]
fn a_wrong_recorded_erasure_fails_closed() {
    let mut p = Program::new();
    p.init(0, "1");
    p.pop(0);
    p.emit(false, 1, "1/(d-1)");
    p.emit(false, 1, "-1/(d+1)");
    p.cancel(false, 1, false);
    p.emit(true, 0, "1");
    let (trace, basis, _) = p.finish();
    for denominators in [
        PolynomialDenominators::Factorized,
        PolynomialDenominators::Dense,
    ] {
        let error = replay_images(&trace, &basis, &[0], &LARGE, 3, denominators)
            .expect_err("nonzero erasure");
        assert!(error.to_string().contains("nonzero"), "{error}");
    }
}

#[test]
fn bad_primes_are_rejected() {
    let cases: [(&[&str], u64); 6] = [
        // A constant coefficient denominator.
        (&["1/7"], 7),
        // A leading coefficient of a basis factor.
        (&["1/(7*d+1)"], 7),
        // Two linear factors with the same root.
        (&["1/(d-1)", "1/(d-12)"], 11),
        // A nonlinear factor vanishing at a linear root: 2^2 + 1 = 5.
        (&["1/(d^2+1)", "1/(d-2)"], 5),
        // Two nonlinear factors sharing the root 3 modulo 5.
        (&["1/(d^2+1)", "1/(d^2+d+3)"], 5),
        // The content of a polynomial coefficient's denominator.
        (&["d/(7*d-7)"], 7),
    ];
    for (coefficients, prime) in cases {
        let mut p = Program::new();
        p.init(0, "1");
        p.pop(0);
        for (slot, text) in coefficients.iter().enumerate() {
            p.emit(true, slot, text);
        }
        let (trace, basis, _) = p.finish();
        assert!(
            LaneTables::new(prime, &trace, &basis).is_none(),
            "{coefficients:?} mod {prime}"
        );
        // Good primes of the same size are accepted.
        assert!(LaneTables::new(1_000_003, &trace, &basis).is_some());
    }
}

#[test]
fn lanes_are_independent_including_unlucky_primes() {
    // Modulo 13 the weight 13*d + 1 drops to a constant and 26/(d-1) to zero:
    // an unlucky prime for these terminals, which must still evaluate to the
    // exact reductions, whatever the other lanes of its job.
    let mut p = Program::new();
    p.init(0, "13*d+1");
    p.pop(0);
    p.emit(false, 1, "1/(d-2)");
    p.emit(true, 0, "26/(d-1)");
    p.emit(true, 0, "1/(d+3)");
    p.pop(1);
    p.emit(true, 1, "(d-5)/(d^2+3)");
    p.emit(true, 2, "13");
    let (trace, basis, terminals) = p.finish();
    let outputs = [0, 1, 2];
    let primes = [13, 1_000_003, LARGE[0], 17, LARGE[1]];
    let expected: Vec<Vec<Image>> = primes
        .iter()
        .map(|&prime| terminals.iter().map(|t| reduce(t, prime)).collect())
        .collect();
    assert_ne!(expected[0][0].degrees(), expected[1][0].degrees());
    assert!(expected[0][2].numerator.is_empty());
    for lanes in [1, 2, 5] {
        for denominators in [
            PolynomialDenominators::Factorized,
            PolynomialDenominators::Dense,
        ] {
            let images =
                replay_images(&trace, &basis, &outputs, &primes, lanes, denominators).unwrap();
            assert_eq!(images, expected, "{lanes} {denominators:?}");
        }
    }
}

#[test]
fn jobs_fill_the_threads_within_the_lane_bound() {
    assert_eq!(job_sizes(0, 4, 4), Vec::<usize>::new());
    assert_eq!(job_sizes(3, 16, 4), vec![1, 1, 1]);
    assert_eq!(job_sizes(16, 16, 4), vec![1; 16]);
    assert_eq!(job_sizes(10, 4, 4), vec![3, 3, 2, 2]);
    assert_eq!(job_sizes(32, 16, 4), vec![2; 16]);
    assert_eq!(job_sizes(9, 1, 4), vec![3, 3, 3]);
    assert_eq!(job_sizes(5, 1, 1), vec![1; 5]);
}

#[test]
fn degrees_beyond_the_bound_fail_closed() {
    let mut p = Program::new();
    p.init(0, "1");
    p.pop(0);
    p.emit(true, 0, "1/(d-1)^3");
    let (trace, basis, _) = p.finish();
    let tables = vec![LaneTables::new(LARGE[0], &trace, &basis).unwrap()];
    for (max_degree, ok) in [(3, true), (2, false)] {
        let result = replay(
            &trace,
            &[0],
            1,
            &mut Factorized::new(&tables, &basis, max_degree),
        );
        assert_eq!(result.is_ok(), ok, "{max_degree}");
        let result = replay(
            &trace,
            &[0],
            1,
            &mut Dense::new(&tables, &basis, max_degree),
        );
        assert_eq!(result.is_ok(), ok, "{max_degree}");
    }
}

/// A refused checkpoint inside the operation loop stops a polynomial replay
/// (both representations) with the refusal; an unrefused replay returns the
/// exact images.
#[test]
fn a_polynomial_replay_stops_at_a_refused_checkpoint_inside_its_operations() {
    use crate::reduction::stopping::{Stopping, is_stop};
    let mut p = Program::new();
    p.init(0, "(d+3)/(d-1)");
    p.pop(0);
    for k in 0..5 * CHECKPOINT_OPS {
        p.emit(true, k % 3, &format!("(d+{k})/(d^2+{})", k % 4 + 1));
    }
    let (trace, basis, exact) = p.finish();
    let outputs = [0, 1, 2];
    let expected: Vec<Image> = exact.iter().map(|c| reduce(c, LARGE[0])).collect();
    for denominators in [
        PolynomialDenominators::Factorized,
        PolynomialDenominators::Dense,
    ] {
        let clean = Stopping::armed(None);
        let images = clean
            .run(|| replay_images(&trace, &basis, &outputs, &LARGE[..1], 1, denominators))
            .unwrap();
        assert_eq!(images, vec![expected.clone()]);
        let checkpoints = clean.seen();
        assert!(
            checkpoints >= trace.ops.len() / CHECKPOINT_OPS,
            "{checkpoints}"
        );
        for stop_at in 1..=checkpoints {
            let stopping = Stopping::armed(Some(stop_at));
            let error = stopping
                .run(|| replay_images(&trace, &basis, &outputs, &LARGE[..1], 1, denominators))
                .unwrap_err();
            assert!(is_stop(&error), "{stop_at}: {error}");
        }
    }
}
