//! Numeric replay of a recorded trace at one batch of points modulo one prime.
use symbolica::domains::finite_field::Zp64;
use symbolica::domains::{Ring, RingOps};

use super::field::{Fe, batch_invert, horner, reduce};
use super::{OpTag, Trace};

pub(super) enum ReplayError {
    /// The prime divides a constant coefficient's denominator.
    BadPrime,
    /// A key erased as exactly zero has a nonzero value at a valid point.
    ZeroDecision,
}

pub(super) struct ReplayBatch {
    /// Terminal-major values: `values[t * points + j]`.
    pub values: Vec<Fe>,
    /// Columns where every coefficient was finite.
    pub good: Vec<bool>,
    pub cancel_checks: usize,
}

/// Coefficient values for one batch.
struct Table {
    constants: Vec<Fe>,
    /// Polynomial-major: `polynomials[c * points + j]`.
    polynomials: Vec<Fe>,
}

fn evaluate_table(
    trace: &Trace,
    field: &Zp64,
    points: &[Fe],
    good: &mut [bool],
) -> Result<Table, ReplayError> {
    let width = points.len();
    let mut numerators = Vec::with_capacity(trace.constants.len());
    let mut denominators = Vec::with_capacity(trace.constants.len());
    for (numerator, denominator) in &trace.constants {
        numerators.push(reduce(field, numerator));
        denominators.push(reduce(field, denominator));
    }
    let mut bad_prime = false;
    batch_invert(field, &mut denominators, |_| bad_prime = true);
    if bad_prime {
        return Err(ReplayError::BadPrime);
    }
    let constants = numerators
        .iter()
        .zip(&denominators)
        .map(|(n, d)| field.mul(n, d))
        .collect();
    let mut values = vec![field.zero(); trace.polynomials.len() * width];
    let mut inverses = vec![field.zero(); trace.polynomials.len() * width];
    let mut reduced = Vec::new();
    for (index, rational) in trace.polynomials.iter().enumerate() {
        let row = index * width;
        reduced.clear();
        reduced.extend(rational.numerator.iter().map(|c| reduce(field, c)));
        for (j, x) in points.iter().enumerate() {
            values[row + j] = horner(field, &reduced, *x);
        }
        reduced.clear();
        reduced.extend(rational.denominator.iter().map(|c| reduce(field, c)));
        for (j, x) in points.iter().enumerate() {
            inverses[row + j] = horner(field, &reduced, *x);
        }
    }
    batch_invert(field, &mut inverses, |position| {
        good[position % width] = false
    });
    for (value, inverse) in values.iter_mut().zip(&inverses) {
        *value = field.mul(&*value, inverse);
    }
    Ok(Table {
        constants,
        polynomials: values,
    })
}

/// Evaluate the trace at `points` (distinct, Montgomery form) modulo `field`.
pub(super) fn replay(
    trace: &Trace,
    field: &Zp64,
    points: &[Fe],
) -> Result<ReplayBatch, ReplayError> {
    let width = points.len();
    let mut good = vec![true; width];
    let table = evaluate_table(trace, field, points, &mut good)?;
    let zero = field.zero();
    let one = field.one();
    let mut weights = vec![zero; trace.slots * width];
    let mut terminals = vec![zero; trace.terminal_slots * width];
    let mut active = vec![zero; width];
    let unit = vec![one; width];
    let mut cancel_checks = 0;
    for op in &trace.ops {
        let tag = op.tag();
        let slot = op.slot();
        match tag {
            OpTag::Pop => {
                active.copy_from_slice(&weights[slot * width..(slot + 1) * width]);
                continue;
            }
            OpTag::Cancel | OpTag::TermCancel => {
                let target = if tag == OpTag::Cancel {
                    &weights[slot * width..(slot + 1) * width]
                } else {
                    &terminals[slot * width..(slot + 1) * width]
                };
                cancel_checks += 1;
                if target
                    .iter()
                    .zip(&good)
                    .any(|(value, good)| *good && !field.is_zero(value))
                {
                    return Err(ReplayError::ZeroDecision);
                }
                continue;
            }
            _ => {}
        }
        let source: &[Fe] = if tag == OpTag::Init { &unit } else { &active };
        let destination = match tag {
            OpTag::Init | OpTag::New | OpTag::Add => &mut weights[slot * width..(slot + 1) * width],
            _ => &mut terminals[slot * width..(slot + 1) * width],
        };
        let assign = matches!(tag, OpTag::Init | OpTag::New | OpTag::TermNew);
        let reference = op.coefficient;
        if reference.is_constant() {
            let c = table.constants[reference.index()];
            if assign {
                for (d, s) in destination.iter_mut().zip(source) {
                    *d = field.mul(s, &c);
                }
            } else {
                for (d, s) in destination.iter_mut().zip(source) {
                    *d = field.add(&*d, &field.mul(s, &c));
                }
            }
        } else {
            let row = reference.index() * width;
            let c = &table.polynomials[row..row + width];
            if assign {
                for ((d, s), c) in destination.iter_mut().zip(source).zip(c) {
                    *d = field.mul(s, c);
                }
            } else {
                for ((d, s), c) in destination.iter_mut().zip(source).zip(c) {
                    *d = field.add(&*d, &field.mul(s, c));
                }
            }
        }
    }
    Ok(ReplayBatch {
        values: terminals,
        good,
        cancel_checks,
    })
}
