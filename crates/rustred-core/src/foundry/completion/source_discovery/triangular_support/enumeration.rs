use crate::identity::{IntegralShift, TranslatedSourceRequest};
use crate::sector::Mask;

use super::resource::{checked_add, checked_mul, try_vec};
use super::{OFFSETS, TriangularSupportError};

pub(super) fn validate_axes(
    chart_axes: &[usize],
    arity: usize,
) -> Result<(), TriangularSupportError> {
    for (position, &axis) in chart_axes.iter().enumerate() {
        if axis >= arity {
            return Err(TriangularSupportError::AxisOutOfRange {
                position,
                axis,
                arity,
            });
        }
        if let Some(first_position) = chart_axes[..position]
            .iter()
            .position(|&existing| existing == axis)
        {
            return Err(TriangularSupportError::DuplicateAxis {
                first_position,
                duplicate_position: position,
                axis,
            });
        }
    }
    Ok(())
}

pub(super) fn count_offsets(
    axis_count: usize,
    degree: usize,
) -> Result<usize, TriangularSupportError> {
    if axis_count == 0 {
        return Ok(1);
    }
    let n = checked_add(OFFSETS, axis_count, degree)?;
    checked_binomial(n, axis_count.min(degree))
}

fn checked_binomial(n: usize, k: usize) -> Result<usize, TriangularSupportError> {
    let mut result = 1usize;
    for step in 1..=k {
        let mut numerator = n - k + step;
        let mut denominator = step;
        let common = gcd(result, denominator);
        result /= common;
        denominator /= common;
        let common = gcd(numerator, denominator);
        numerator /= common;
        denominator /= common;
        if denominator != 1 {
            return Err(TriangularSupportError::Invariant {
                detail: "exact binomial preflight left a nonunit denominator",
            });
        }
        result = checked_mul(OFFSETS, result, numerator)?;
    }
    Ok(result)
}

fn gcd(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

pub(super) fn enumerate_requests(
    requests: &mut Vec<TranslatedSourceRequest>,
    sector: &Mask,
    chart_axes: &[usize],
    source_degree_ceilings: &[usize],
    arity: usize,
) -> Result<(), TriangularSupportError> {
    let axis_count = chart_axes.len();
    let mut chart = try_vec("triangular-support chart coordinates", axis_count)?;
    chart.resize(axis_count, 0usize);
    let mut remaining = try_vec("triangular-support chart budgets", axis_count)?;
    remaining.resize(axis_count, 0usize);
    let mut next = try_vec("triangular-support chart cursors", axis_count)?;
    next.resize(axis_count, None::<usize>);
    let mut signed = try_vec("triangular-support signed offset", arity)?;
    signed.resize(arity, 0i64);

    for (source_ordinal, &degree) in source_degree_ceilings.iter().enumerate() {
        if axis_count == 0 {
            push_request(requests, source_ordinal, &signed, arity)?;
            continue;
        }
        for total_degree in 0..=degree {
            if axis_count == 1 {
                chart[0] = total_degree;
                push_chart_request(
                    requests,
                    source_ordinal,
                    sector,
                    chart_axes,
                    &chart,
                    &mut signed,
                    arity,
                )?;
                continue;
            }

            remaining[0] = total_degree;
            next.fill(None);
            next[0] = Some(0);
            let mut position = 0usize;
            loop {
                if position == axis_count - 1 {
                    chart[position] = remaining[position];
                    push_chart_request(
                        requests,
                        source_ordinal,
                        sector,
                        chart_axes,
                        &chart,
                        &mut signed,
                        arity,
                    )?;
                    position -= 1;
                    continue;
                }

                let Some(coordinate) = next[position] else {
                    if position == 0 {
                        break;
                    }
                    position -= 1;
                    continue;
                };
                next[position] = if coordinate == remaining[position] {
                    None
                } else {
                    Some(checked_add(
                        "triangular-support chart cursor",
                        coordinate,
                        1,
                    )?)
                };
                chart[position] = coordinate;
                remaining[position + 1] = remaining[position] - coordinate;
                position += 1;
                if position < axis_count - 1 {
                    next[position] = Some(0);
                }
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_chart_request(
    requests: &mut Vec<TranslatedSourceRequest>,
    source_ordinal: usize,
    sector: &Mask,
    chart_axes: &[usize],
    chart: &[usize],
    signed: &mut [i64],
    arity: usize,
) -> Result<(), TriangularSupportError> {
    for (&axis, &coordinate) in chart_axes.iter().zip(chart) {
        let coordinate = i64::try_from(coordinate).map_err(|_| {
            TriangularSupportError::DegreeNotRepresentable {
                source_ordinal,
                degree: coordinate,
            }
        })?;
        signed[axis] = if sector.active_bits()[axis] {
            coordinate
        } else {
            -coordinate
        };
    }
    push_request(requests, source_ordinal, signed, arity)
}

fn push_request(
    requests: &mut Vec<TranslatedSourceRequest>,
    source_ordinal: usize,
    signed: &[i64],
    arity: usize,
) -> Result<(), TriangularSupportError> {
    let offset = IntegralShift::try_new_with_component_limit(signed.iter().copied(), arity)
        .map_err(TriangularSupportError::Shift)?;
    requests.push(TranslatedSourceRequest::new(source_ordinal, offset));
    Ok(())
}
