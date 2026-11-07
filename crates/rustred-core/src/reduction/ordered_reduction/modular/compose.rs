//! Modular evaluation of a terminal composition plan (`composition.rs`).
//!
//! [`Program`] holds every distinct plan coefficient in dense form;
//! [`Program::reduce`] reduces it modulo one prime ([`PrimeProgram`]), which
//! evaluates the composed outputs at a batch of points of that prime from the
//! terminal values there. A prime that reduces a coefficient denominator to
//! the zero polynomial is unusable; a point that is a pole of a coefficient
//! is a bad point (dropped like a pole of a step coefficient).
//!
//! [`compose_images`] turns the per-prime images in `Z_p(d)` of the raw
//! terminals (`ModularPolynomial`) into images of the composed outputs: it
//! evaluates the terminal images and the plan at fresh points of the prime
//! and interpolates every output (Thiele, accepted after `COMPOSE_CHECKS`
//! consecutive agreeing fresh points, as the sampled arithmetic accepts its
//! per-prime interpolations).
use std::collections::{HashMap, HashSet};

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::{Ring, RingOps};

use super::error;
use super::field::{self, DenseRational, Fe, Poly, batch_invert, horner, reduce};
use super::lift::{Image, image_from};
use super::sampled::Thiele;
use crate::algebra::Coefficient;
use crate::reduction::ReductionError;
use crate::reduction::ordered_reduction::composition::OrderedCompositionPlan;

/// Consecutive agreeing fresh points before a composed output's per-prime
/// interpolation (`ModularPolynomial`) is accepted.
pub(super) const COMPOSE_CHECKS: usize = 3;
/// First batch of points of a `ModularPolynomial` composition (doubling).
const COMPOSE_FIRST_POINTS: usize = 16;
/// Upper bound on the points of one `ModularPolynomial` composition (fail
/// closed beyond it).
pub(super) const COMPOSE_MAX_POINTS: usize = 1 << 16;
/// Seed salt of the composition points of `ModularPolynomial`.
const COMPOSE_POINTS_SALT: u64 = 0x636f_6d70_6f73;

struct Stage {
    /// Per input: `(output, coefficient index)`.
    rows: Vec<Vec<(u32, u32)>>,
    width: usize,
}

/// A composition plan with its distinct coefficients in dense form.
pub(super) struct Program {
    coefficients: Vec<DenseRational>,
    stages: Vec<Stage>,
    inputs: usize,
    outputs: usize,
    /// Plan entries (statistics).
    pub entries: usize,
}

impl Program {
    pub fn new(plan: &OrderedCompositionPlan<'_>, inputs: usize) -> Self {
        let mut index: HashMap<&Coefficient, u32> = HashMap::new();
        let mut coefficients = vec![];
        let mut stages = Vec::with_capacity(plan.stages.len());
        let mut entries = 0;
        for stage in &plan.stages {
            let rows = stage
                .rows
                .iter()
                .map(|row| {
                    entries += row.len();
                    row.iter()
                        .map(|&(output, coefficient)| {
                            let next = coefficients.len() as u32;
                            let reference = *index.entry(coefficient).or_insert_with(|| {
                                coefficients.push(DenseRational::from_coefficient(coefficient));
                                next
                            });
                            (output, reference)
                        })
                        .collect()
                })
                .collect();
            stages.push(Stage {
                rows,
                width: stage.width,
            });
        }
        Self {
            coefficients,
            outputs: plan.outputs(),
            stages,
            inputs,
            entries,
        }
    }

    pub fn outputs(&self) -> usize {
        self.outputs
    }

    pub fn distinct_coefficients(&self) -> usize {
        self.coefficients.len()
    }

    /// The program modulo `field`; `None` when the prime reduces a
    /// coefficient denominator to the zero polynomial (unusable prime).
    pub fn reduce(&self, field: &Zp64) -> Option<PrimeProgram<'_>> {
        let mut numerators = Vec::with_capacity(self.coefficients.len());
        let mut denominators = Vec::with_capacity(self.coefficients.len());
        for coefficient in &self.coefficients {
            let mut denominator: Poly = coefficient
                .denominator
                .iter()
                .map(|c| reduce(field, c))
                .collect();
            field::trim(field, &mut denominator);
            if denominator.is_empty() {
                return None;
            }
            let mut numerator: Poly = coefficient
                .numerator
                .iter()
                .map(|c| reduce(field, c))
                .collect();
            field::trim(field, &mut numerator);
            numerators.push(numerator);
            denominators.push(denominator);
        }
        Some(PrimeProgram {
            program: self,
            field: field.clone(),
            numerators,
            denominators,
        })
    }
}

/// A program reduced modulo one prime.
pub(super) struct PrimeProgram<'p> {
    program: &'p Program,
    field: Zp64,
    numerators: Vec<Poly>,
    denominators: Vec<Poly>,
}

impl PrimeProgram<'_> {
    /// Values of the composed outputs at `points` (output-major,
    /// `out[o * width + j]`) from the values of the plan's inputs there
    /// (`input(i)[j]`). Points that are poles of a coefficient are marked in
    /// `good`; their values are meaningless.
    pub fn apply<'v>(
        &self,
        points: &[Fe],
        input: impl Fn(usize) -> &'v [Fe],
        good: &mut [bool],
    ) -> Vec<Fe> {
        let f = &self.field;
        let width = points.len();
        let distinct = self.numerators.len();
        // Coefficient table, coefficient-major.
        let mut values = vec![f.zero(); distinct * width];
        let mut inverses = vec![f.zero(); distinct * width];
        for c in 0..distinct {
            let row = c * width;
            let (numerator, denominator) = (&self.numerators[c], &self.denominators[c]);
            if numerator.len() <= 1 && denominator.len() <= 1 {
                let n = numerator.first().copied().unwrap_or(f.zero());
                let d = denominator[0];
                values[row..row + width].fill(n);
                inverses[row..row + width].fill(d);
                continue;
            }
            for (j, x) in points.iter().enumerate() {
                values[row + j] = horner(f, numerator, *x);
                inverses[row + j] = horner(f, denominator, *x);
            }
        }
        batch_invert(f, &mut inverses, |position| good[position % width] = false);
        for (value, inverse) in values.iter_mut().zip(&inverses) {
            *value = f.mul(&*value, inverse);
        }
        let mut current: Option<Vec<Fe>> = None;
        for stage in &self.program.stages {
            let mut next = vec![f.zero(); stage.width * width];
            for (i, row) in stage.rows.iter().enumerate() {
                if row.is_empty() {
                    continue;
                }
                let source: &[Fe] = match &current {
                    None => input(i),
                    Some(previous) => &previous[i * width..(i + 1) * width],
                };
                for &(output, c) in row {
                    let coefficient = &values[c as usize * width..(c as usize + 1) * width];
                    let target = &mut next[output as usize * width..(output as usize + 1) * width];
                    for ((t, s), k) in target.iter_mut().zip(source).zip(coefficient) {
                        *t = f.add(&*t, &f.mul(s, k));
                    }
                }
            }
            current = Some(next);
        }
        current.unwrap_or_default()
    }
}

/// Points of one `ModularPolynomial` composition: drawn, and bad (poles of a
/// terminal image or of a plan coefficient, dropped).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ComposePoints {
    pub drawn: usize,
    pub bad: usize,
}

/// The point generator of `compose_images` at `prime`.
fn compose_rng(seed: u64, prime: u64) -> StdRng {
    StdRng::seed_from_u64(seed ^ COMPOSE_POINTS_SALT ^ prime.rotate_left(23))
}

/// Images of the composed outputs at one prime from the images of the plan's
/// inputs there (`ModularPolynomial`); `Ok(None)` for an unusable prime.
pub(super) fn compose_images(
    program: &Program,
    prime: u64,
    inputs: &[Image],
    seed: u64,
) -> Result<(Option<Vec<Image>>, ComposePoints), ReductionError> {
    if inputs.len() != program.inputs {
        return Err(error("composition inputs differ from the plan"));
    }
    let f = Zp64::new(prime);
    let Some(reduced) = program.reduce(&f) else {
        return Ok((None, ComposePoints::default()));
    };
    let outputs = program.outputs;
    let fe = |values: &[u64]| -> Poly { values.iter().map(|v| f.to_element(*v)).collect() };
    let terminals: Vec<(Poly, Poly)> = inputs
        .iter()
        .map(|image| (fe(&image.numerator), fe(&image.denominator)))
        .collect();
    let mut states: Vec<Thiele> = (0..outputs).map(|_| Thiele::new(&f)).collect();
    let mut xs: Vec<Fe> = vec![];
    let mut ys: Vec<Vec<Fe>> = vec![vec![]; outputs];
    let mut rng = compose_rng(seed, prime);
    let mut used = HashSet::new();
    let mut drawn = 0usize;
    let mut bad = 0usize;
    let mut count = COMPOSE_FIRST_POINTS;
    let mut open: Vec<usize> = (0..outputs).collect();
    while !open.is_empty() {
        if drawn + count > COMPOSE_MAX_POINTS {
            return Err(error(format!(
                "terminal composition needs more than {COMPOSE_MAX_POINTS} points per prime"
            )));
        }
        let mut points = Vec::with_capacity(count);
        while points.len() < count {
            let value = rng.random_range(2..prime);
            if used.insert(value) {
                points.push(f.to_element(value));
            }
        }
        drawn += count;
        let width = points.len();
        let mut good = vec![true; width];
        // Terminal values at the points, terminal-major.
        let mut values = vec![f.zero(); terminals.len() * width];
        let mut inverses = vec![f.zero(); terminals.len() * width];
        for (t, (numerator, denominator)) in terminals.iter().enumerate() {
            for (j, x) in points.iter().enumerate() {
                values[t * width + j] = horner(&f, numerator, *x);
                inverses[t * width + j] = horner(&f, denominator, *x);
            }
        }
        batch_invert(&f, &mut inverses, |position| good[position % width] = false);
        for (value, inverse) in values.iter_mut().zip(&inverses) {
            *value = f.mul(&*value, inverse);
        }
        let composed = reduced.apply(&points, |t| &values[t * width..(t + 1) * width], &mut good);
        for (j, x) in points.iter().enumerate() {
            if !good[j] {
                bad += 1;
                continue;
            }
            xs.push(*x);
            for (o, ys) in ys.iter_mut().enumerate() {
                ys.push(composed[o * width + j]);
            }
        }
        open.retain(|&o| !states[o].feed(&f, &xs, &ys[o], COMPOSE_CHECKS));
        count = drawn;
    }
    let images = states
        .iter()
        .map(|state| {
            let (numerator, denominator) = state.result.clone().unwrap_or_default();
            image_from(&f, &numerator, &denominator)
        })
        .collect();
    Ok((Some(images), ComposePoints { drawn, bad }))
}

/// The first `count` points `compose_images` draws at `prime` (test hook).
#[cfg(test)]
pub(super) fn first_compose_points(seed: u64, prime: u64, count: usize) -> Vec<u64> {
    let mut rng = compose_rng(seed, prime);
    let mut used = HashSet::new();
    let mut points = Vec::with_capacity(count);
    while points.len() < count {
        let value = rng.random_range(2..prime);
        if used.insert(value) {
            points.push(value);
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::reduction::ordered_reduction::composition::OrderedCompositionStage;
    use std::sync::Arc;
    use symbolica::domains::Field;

    fn image(prime: u64, function: &DenseRational) -> Image {
        let f = Zp64::new(prime);
        let reduce_all = |values: &[symbolica::prelude::Integer]| -> Poly {
            let mut poly: Poly = values.iter().map(|c| reduce(&f, c)).collect();
            field::trim(&f, &mut poly);
            poly
        };
        let mut numerator = reduce_all(&function.numerator);
        let mut denominator = reduce_all(&function.denominator);
        let g = field::gcd(&f, &numerator, &denominator);
        numerator = field::quot_rem(&f, &numerator, &g).0;
        denominator = field::quot_rem(&f, &denominator, &g).0;
        let lead = f.inv(denominator.last().unwrap());
        for c in numerator.iter_mut().chain(denominator.iter_mut()) {
            *c = f.mul(&*c, &lead);
        }
        field::trim(&f, &mut numerator);
        image_from(&f, &numerator, &denominator)
    }

    /// Two terminals through two stages: the composed images equal the
    /// images of the exactly composed functions.
    #[test]
    fn composed_images_equal_the_images_of_the_exact_composition() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let c = |text: &str| context.coefficient_fixture(text);
        let terminals = [c("(d^3-7*d+1)/((d-4)*(2*d-7))"), c("(5*d+3)/(d-4)")];
        let a = c("(d-4)/(d+1)");
        let b = c("-(2*d-7)/(3*d-11)");
        let e = c("1");
        let g = c("(d^2+1)/7");
        let plan = OrderedCompositionPlan {
            stages: vec![
                OrderedCompositionStage {
                    rows: vec![vec![(0, &a), (1, &b)], vec![(1, &e)]],
                    width: 2,
                },
                OrderedCompositionStage {
                    rows: vec![vec![(0, &g)], vec![(0, &e), (1, &a)]],
                    width: 2,
                },
            ],
            labels: Arc::new(()),
        };
        plan.validate(2).unwrap();
        // Exact composition.
        let mul = |x: &Coefficient, y: &Coefficient| x * y;
        let mid0 = mul(&terminals[0], &a);
        let mid1 = &mul(&terminals[0], &b) + &terminals[1];
        let out0 = &mul(&mid0, &g) + &mid1;
        let out1 = mul(&mid1, &a);
        let program = Program::new(&plan, 2);
        assert_eq!(program.distinct_coefficients(), 4);
        for prime in [1_000_000_007u64, 4_611_686_018_427_388_039] {
            let inputs: Vec<Image> = terminals
                .iter()
                .map(|t| image(prime, &DenseRational::from_coefficient(t)))
                .collect();
            let (composed, points) = compose_images(&program, prime, &inputs, 7).unwrap();
            let composed = composed.expect("usable prime");
            assert!(points.drawn >= COMPOSE_FIRST_POINTS);
            assert_eq!(points.bad, 0);
            assert_eq!(
                composed[0],
                image(prime, &DenseRational::from_coefficient(&out0))
            );
            assert_eq!(
                composed[1],
                image(prime, &DenseRational::from_coefficient(&out1))
            );
        }
    }

    /// A pole of a terminal image (at the first composition point) and of a
    /// plan coefficient (at the second) are bad points, dropped; the composed
    /// image is still the image of the exact composition.
    #[test]
    fn poles_at_composition_points_are_dropped() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let c = |text: &str| context.coefficient_fixture(text);
        let (prime, seed) = (1_000_000_007u64, 11);
        let poles = first_compose_points(seed, prime, 2);
        let terminals = [c(&format!("(d^2+3)/(d-{})", poles[0])), c("(2*d+1)/(d+4)")];
        let a = c(&format!("(d+1)/(d-{})", poles[1]));
        let b = c("3");
        let plan = OrderedCompositionPlan {
            stages: vec![OrderedCompositionStage {
                rows: vec![vec![(0, &a)], vec![(0, &b)]],
                width: 1,
            }],
            labels: Arc::new(()),
        };
        plan.validate(2).unwrap();
        let exact = &(&terminals[0] * &a) + &(&terminals[1] * &b);
        let program = Program::new(&plan, 2);
        let inputs: Vec<Image> = terminals
            .iter()
            .map(|t| image(prime, &DenseRational::from_coefficient(t)))
            .collect();
        let (composed, points) = compose_images(&program, prime, &inputs, seed).unwrap();
        assert_eq!(points.bad, 2);
        assert_eq!(
            composed.expect("usable prime")[0],
            image(prime, &DenseRational::from_coefficient(&exact))
        );
    }

    #[test]
    fn a_prime_dividing_a_coefficient_denominator_is_unusable() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let c = context.coefficient_fixture("d/1000000007");
        let plan = OrderedCompositionPlan {
            stages: vec![OrderedCompositionStage {
                rows: vec![vec![(0, &c)]],
                width: 1,
            }],
            labels: Arc::new(()),
        };
        let program = Program::new(&plan, 1);
        assert!(program.reduce(&Zp64::new(1_000_000_007)).is_none());
        assert!(program.reduce(&Zp64::new(1_000_000_009)).is_some());
    }
}
