//! Independent checks of the reference relations supplied on 2026-10-09.
//! Reference rows are queries, never admitted identities.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use symbolica::domains::finite_field::{ToFiniteField, Zp64};
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::domains::{Field, Ring};
use symbolica::prelude::{Integer, Z};
use symbolica::tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::solver::{Integral, IntegralOrder, Seed};

use super::reference_routing::{
    ReferenceKey, bubble_reflections, canonical_contours, canonicalize, is_scaleless,
    symmetry_images,
};
use super::reference_spatial::spatial_sources;

type TaggedKey = ([i8; 3], ReferenceKey);
type Row = BTreeMap<TaggedKey, Coefficient>;

struct Reference {
    name: &'static str,
    terms: Vec<([i8; 3], ReferenceKey, &'static str)>,
}

fn key(a: [i16; 6], energy: [i16; 3]) -> ReferenceKey {
    [
        a[0], a[1], a[2], a[3], a[4], a[5], -energy[0], -energy[1], -energy[2],
    ]
}

fn references() -> Vec<Reference> {
    let f = [1, 1, 1];
    let b = [0, 0, 1];
    let m = [1, 1, 0];
    vec![
        Reference {
            name: "R2a-zero",
            terms: vec![(f, key([1, 0, 0, 1, 1, 1], [0; 3]), "1")],
        },
        Reference {
            name: "R2b-zero",
            terms: vec![(f, key([0, 0, 1, 1, 1, 1], [0; 3]), "1")],
        },
        Reference {
            name: "R3a-zero",
            terms: vec![(f, key([1, 0, 1, 1, 1, 0], [0; 3]), "1")],
        },
        Reference {
            name: "R3b-zero",
            terms: vec![(f, key([1, 0, 1, 0, 1, 1], [0; 3]), "1")],
        },
        // The four factorizations are tested by routing and the independently
        // integrated two-loop relation in reference_conversion.rs.
        Reference {
            name: "R5-negative-index",
            terms: vec![
                (f, key([1, -1, 1, 1, 1, 1], [0; 3]), "1"),
                (b, key([1, 1, 0, 0, 1, 1], [0; 3]), "1/2"),
            ],
        },
        Reference {
            name: "R6-three-fermionic-contours",
            terms: vec![
                (f, key([1, 1, 1, 1, 1, -1], [0; 3]), "1"),
                (b, key([1, 1, 0, 0, 1, 1], [0; 3]), "-(1/(d-2)-2/(d-4)-5)/4"),
                (b, key([2, 1, 0, 0, 1, 1], [0, 2, 0]), "(d-3)/((d-2)*(d-4))"),
                (
                    f,
                    key([2, 1, 0, 0, 1, 1], [0, 2, 0]),
                    "-(d-5)/((d-2)*(d-4))",
                ),
                (
                    b,
                    key([2, 1, 0, 0, 1, 1], [1, 1, 0]),
                    "-2/(d-4)-8/(d-3)+22/(d-2)",
                ),
                (
                    b,
                    key([2, 1, 0, 0, 1, 1], [2, 0, 0]),
                    "2/(d-4)+4/(d-3)-8/(d-2)",
                ),
                (b, key([2, 2, 0, 0, 1, 1], [3, 1, 0]), "8/((d-3)*(d-2))"),
                (b, key([3, 1, 0, 0, 1, 1], [3, 1, 0]), "32/((d-3)*(d-2))"),
                (b, key([3, 1, 0, 0, 1, 1], [4, 0, 0]), "-16/((d-3)*(d-2))"),
                (f, key([2, 1, 1, 0, 0, 0], [0; 3]), "-(d-1)/((d-3)*(d-2))"),
            ],
        },
        Reference {
            name: "R7-two-fermionic-contours",
            terms: vec![
                (m, key([1, 1, -1, 1, 1, 1], [0; 3]), "1"),
                (
                    m,
                    key([1, 1, 0, 0, 1, 1], [0; 3]),
                    "-(2/(d-4)+2/(d-3)+9/(d-2)-7)/2",
                ),
                (m, key([2, 1, 0, 0, 1, 1], [0, 2, 0]), "4/((d-4)*(d-2))"),
                (
                    m,
                    key([2, 1, 0, 0, 1, 1], [1, 1, 0]),
                    "-4/(d-4)-16/(d-3)+44/(d-2)",
                ),
                (m, key([2, 2, 0, 0, 1, 1], [3, 1, 0]), "16/((d-3)*(d-2))"),
                (m, key([3, 1, 0, 0, 1, 1], [3, 1, 0]), "64/((d-3)*(d-2))"),
                (m, key([3, 1, 0, 0, 1, 1], [4, 0, 0]), "-32/((d-3)*(d-2))"),
                (f, key([2, 1, 1, 0, 0, 0], [0; 3]), "-4/((d-4)*(d-3))"),
                (f, key([2, 2, 1, 0, 0, 0], [1, 1, 0]), "4/((d-3)*(d-2))"),
                (
                    f,
                    key([2, 2, 1, 0, 0, 0], [1, 0, 1]),
                    "-8/((d-4)*(d-3)*(d-2))",
                ),
            ],
        },
    ]
}

fn accumulate(row: &mut Row, point: TaggedKey, value: Coefficient) {
    if value.is_zero() {
        return;
    }
    match row.entry(point) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(value);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let sum = entry.get() + &value;
            if sum.is_zero() {
                entry.remove();
            } else {
                *entry.get_mut() = sum;
            }
        }
    }
}

fn route(
    row: &mut Row,
    contours: [i8; 3],
    point: ReferenceKey,
    value: &Coefficient,
    c: &CoefficientContext,
) {
    if is_scaleless(contours, point).unwrap() {
        return;
    }
    for (point, factor) in canonicalize(contours, point).unwrap() {
        accumulate(
            row,
            (canonical_contours(contours).unwrap(), point),
            value * &c.integer(factor),
        );
    }
}

fn neighbours(point: ReferenceKey) -> BTreeSet<ReferenceKey> {
    let mut points = BTreeSet::from([point]);
    for up in 0..6 {
        for down in 0..6 {
            let mut next = point;
            next[up] += 1;
            next[down] -= 1;
            if next[..6].iter().all(|n| (-1..=3).contains(n)) {
                points.insert(next);
            }
        }
    }
    for up in 0..3 {
        for down in 0..3 {
            let mut next = point;
            next[6 + up] += 1;
            next[6 + down] -= 1;
            if next[6..].iter().all(|n| (-4..=0).contains(n)) {
                points.insert(next);
            }
        }
    }
    points
}

/// Recover weights on the original independent source rows from A = L U and
/// target = q U. Native L columns use accepted-U order; native L row positions
/// also include dependent inputs and therefore need the separate embedding.
fn original_source_weights<F: Field>(
    lower: &SparseMatrix<F>,
    accepted_l_rows: &[usize],
    query_l_row: usize,
) -> Vec<F::Element> {
    let field = lower.field();
    let count = accepted_l_rows.len();
    assert_eq!(lower.ncols() as usize, count);
    let mut pending = vec![field.zero(); count];
    for position in lower.row_ptrs()[query_l_row]..lower.row_ptrs()[query_l_row + 1] {
        pending[lower.col_idcs()[position] as usize] = lower.values()[position].clone();
    }
    let mut weights = vec![field.zero(); count];
    for source in (0..count).rev() {
        let row = accepted_l_rows[source];
        let range = lower.row_ptrs()[row]..lower.row_ptrs()[row + 1];
        let diagonal = range
            .clone()
            .find(|position| lower.col_idcs()[*position] as usize == source)
            .expect("native accepted L diagonal");
        let weight = field.div(&pending[source], &lower.values()[diagonal]);
        if field.is_zero(&weight) {
            continue;
        }
        for position in range {
            let column = lower.col_idcs()[position] as usize;
            if column == source {
                continue;
            }
            assert!(column < source);
            pending[column] = field.sub(
                &pending[column],
                &field.mul(&weight, &lower.values()[position]),
            );
        }
        weights[source] = weight;
    }
    weights
}

/// Two bounded support proposals from one modular specialization. Neither is
/// guaranteed to retain every source needed by the unspecialized exact query.
fn modular_source_supports<F: Field>(
    lower: &SparseMatrix<F>,
    accepted_l_rows: &[usize],
    retained: &[usize],
    query_l_row: usize,
) -> (Vec<usize>, Vec<usize>) {
    let mut pending = lower.col_idcs()
        [lower.row_ptrs()[query_l_row]..lower.row_ptrs()[query_l_row + 1]]
        .iter()
        .map(|n| *n as usize)
        .collect::<Vec<_>>();
    let mut needed = BTreeSet::new();
    while let Some(row) = pending.pop() {
        if !needed.insert(row) {
            continue;
        }
        let source_l_row = accepted_l_rows[row];
        pending.extend(
            lower.col_idcs()[lower.row_ptrs()[source_l_row]..lower.row_ptrs()[source_l_row + 1]]
                .iter()
                .map(|n| *n as usize)
                .filter(|n| *n != row),
        );
    }
    let trace: Vec<_> = needed.into_iter().map(|row| retained[row]).collect();
    let weights = original_source_weights(lower, accepted_l_rows, query_l_row);
    let weighted_trace = weights
        .iter()
        .enumerate()
        .filter(|(_, weight)| !lower.field().is_zero(weight))
        .map(|(source, _)| retained[source])
        .collect::<Vec<_>>();
    assert!(
        weighted_trace
            .iter()
            .all(|source| trace.binary_search(source).is_ok())
    );
    (trace, weighted_trace)
}

/// A modular support is only a proposal. Solve A^T w = target^T with native
/// sparse elimination, then independently check w A against every query column.
fn replay_reference_trace(
    name: &str,
    stage: &str,
    rows: &[Row],
    columns: &[TaggedKey],
    target: &Row,
    trace: &[usize],
    context: &CoefficientContext,
) -> bool {
    let started = Instant::now();
    let count = trace.len();
    let rhs_column = u32::try_from(count).expect("source-weight column count fits u32");
    let mut equations: Vec<Vec<(u32, Coefficient)>> = vec![Vec::new(); columns.len()];
    for (source, ordinal) in trace.iter().enumerate() {
        for (point, coefficient) in &rows[*ordinal] {
            let column = columns.binary_search(point).unwrap();
            equations[column].push((source as u32, coefficient.clone()));
        }
    }
    let minus_one = context.integer(-1);
    for (point, coefficient) in target {
        let column = columns.binary_search(point).unwrap();
        equations[column].push((rhs_column, &minus_one * coefficient));
    }
    equations.retain(|row| !row.is_empty());
    // The new variables are source weights, not physical integral indices.
    // Start with short constant equations to determine cheap certificate
    // entries before admitting densely coupled symbolic constraints.
    equations.sort_by_key(|row| {
        (
            row.len(),
            row.iter()
                .filter(|(_, coefficient)| {
                    !coefficient.numerator.is_constant() || !coefficient.denominator.is_constant()
                })
                .count(),
        )
    });
    let equation_count = equations.len();
    let transpose_elapsed = started.elapsed();
    let elimination_started = Instant::now();
    let mut exact = SparseRowReducer::new(
        rhs_column
            .checked_add(1)
            .expect("augmented weight column fits u32"),
        RationalPolynomialField::new(Z),
        LuLMode::None,
    );
    let mut processed = 0;
    for equation in equations {
        let (ids, values): (Vec<_>, Vec<_>) = equation.into_iter().unzip();
        let pivot = exact.add_row_with_back_subs(&values, &ids);
        processed += 1;
        // A pivot in the augmented column is an inconsistent exact constraint.
        if pivot == Some(rhs_column) {
            eprintln!(
                "reference {name}: exact_stage={stage} exact_pass=false reason=inconsistent-transposed-equation equations={processed}/{equation_count} trace_rows={count} transpose={transpose_elapsed:?} elimination={:?}",
                elimination_started.elapsed()
            );
            return false;
        }
        let rank = exact.u().nrows() as usize;
        if processed % 512 == 0 || rank == count || processed == equation_count {
            eprintln!(
                "reference {name}: exact_stage={stage} equations={processed}/{equation_count} weight_rank={rank}/{count} elapsed={:?}",
                started.elapsed()
            );
        }
        if rank == count {
            // These equations determine every weight. The independent product
            // below checks ALL columns, including equations not processed here.
            break;
        }
    }
    let elimination_elapsed = elimination_started.elapsed();
    if exact.u().nrows() as usize != count {
        eprintln!(
            "reference {name}: exact_stage={stage} exact_pass=false reason=underdetermined-source-weights trace_rows={count} transpose={transpose_elapsed:?} elimination={elimination_elapsed:?}"
        );
        return false;
    }
    let weights_started = Instant::now();
    let mut weights = vec![context.zero(); count];
    let upper = exact.u();
    // Native pivots are normalized. Read the resulting triangular equations
    // backwards without performing another elimination or constructing L.
    for source in (0..count).rev() {
        let row =
            exact.pivots()[source].expect("full weight rank has every variable pivot") as usize;
        let mut weight = context.zero();
        for position in upper.row_ptrs()[row]..upper.row_ptrs()[row + 1] {
            let column = upper.col_idcs()[position] as usize;
            let coefficient = &upper.values()[position];
            if column == source {
                assert!(upper.field().is_one(coefficient));
            } else if column == count {
                weight = &weight - coefficient;
            } else {
                assert!(column > source);
                weight = &weight - &(coefficient * &weights[column]);
            }
        }
        weights[source] = weight;
    }
    let weights_elapsed = weights_started.elapsed();
    // Release the reduced system before multiplying the original source rows.
    drop(exact);
    let replay_started = Instant::now();
    let mut replay = target.clone();
    let mut nonzero_weights = 0;
    for (source, weight) in weights.iter().enumerate() {
        if weight.is_zero() {
            continue;
        }
        nonzero_weights += 1;
        for (point, coefficient) in &rows[trace[source]] {
            accumulate(&mut replay, *point, &minus_one * &(weight * coefficient));
        }
    }
    let replay_passed = replay.is_empty();
    eprintln!(
        "reference {name}: exact_stage={stage} exact_pass={replay_passed} trace_rows={count} nonzero_weights={nonzero_weights} equations={processed}/{equation_count} residual_columns={} transpose={transpose_elapsed:?} elimination={elimination_elapsed:?} weights={weights_elapsed:?} multiply_back={:?} total={:?}",
        replay.len(),
        replay_started.elapsed(),
        started.elapsed()
    );
    // Full rank of a subsystem is not full-system consistency. A residual can
    // indicate an omitted inconsistent constraint, so it is a support miss.
    replay_passed
}

#[test]
#[ignore = "bounded supplied-reference performance pilot; run explicitly with --ignored --nocapture"]
fn bounded_supplied_reference_probe_reports_outcomes() {
    let started = Instant::now();
    let sources = spatial_sources::<9>(3).unwrap();
    let c = &sources.coefficients;
    let shells = std::env::var("RUSTRED_REFERENCE_SHELLS")
        .ok()
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(1);
    assert!(
        (1..=3).contains(&shells),
        "reference pilot shell limit is 1..=3"
    );
    eprintln!("reference source construction: {:?}", started.elapsed());
    let filter = std::env::var("RUSTRED_REFERENCE_FILTER").unwrap_or_default();
    let mut verified = Vec::new();
    let mut unresolved = Vec::new();
    for reference in references() {
        if !reference.name.contains(&filter) {
            continue;
        }
        let before = Instant::now();
        let main_contours = canonical_contours(reference.terms[0].0).unwrap();
        let mut target = Row::new();
        for (contours, point, coefficient) in &reference.terms {
            let coefficient = c.coefficient_fixture(coefficient);
            if canonical_contours(*contours).unwrap() == main_contours {
                route(&mut target, *contours, *point, &coefficient, c);
            } else {
                // A factorized F*F*F product can be embedded in the ++B
                // family by old R = P-new R. This is a unimodular change of
                // the independent contours, not a symmetry of a connected
                // three-fermion graph. Expand (P0-R0)^alpha explicitly.
                assert_eq!(*contours, [1, 1, 1]);
                assert_eq!(main_contours, [1, 1, 0]);
                assert_eq!(&point[3..6], &[0, 0, 0]);
                let rank = -point[8];
                let mut choose = 1_i64;
                for k in 0..=rank {
                    let mut mapped = *point;
                    mapped[2] = 0;
                    mapped[4] = point[2];
                    mapped[6] = point[6] - rank + k;
                    mapped[8] = -k;
                    let factor = if k % 2 == 0 { choose } else { -choose };
                    route(
                        &mut target,
                        main_contours,
                        mapped,
                        &(&coefficient * &c.integer(factor)),
                        c,
                    );
                    if k < rank {
                        choose = choose * i64::from(rank - k) / i64::from(k + 1);
                    }
                }
            }
        }
        if target.is_empty() {
            eprintln!(
                "reference {}: exact routing/scaleless proof {:?}",
                reference.name,
                before.elapsed()
            );
            verified.push(reference.name);
            continue;
        }
        let mut seeds = BTreeSet::new();
        let mut frontier: BTreeSet<_> = target.keys().copied().collect();
        for _ in 0..shells {
            let mut next = BTreeSet::new();
            for (contours, point) in &frontier {
                next.extend(
                    neighbours(*point)
                        .into_iter()
                        .map(|point| (*contours, point)),
                );
                for source in &sources.rows {
                    for term in source {
                        let seed: ReferenceKey =
                            std::array::from_fn(|axis| point[axis] - term.integral[axis].value());
                        if seed[..6].iter().all(|n| (-1..=3).contains(n))
                            && seed[6..].iter().all(|n| (-4..=0).contains(n))
                        {
                            next.insert((*contours, seed));
                        }
                    }
                }
                for (reflected_contours, image) in bubble_reflections(*contours, *point).unwrap() {
                    for (child, _) in image {
                        if child[..6].iter().all(|n| (-1..=3).contains(n))
                            && child[6..].iter().all(|n| (-4..=0).contains(n))
                        {
                            for (routed, _) in canonicalize(reflected_contours, child).unwrap() {
                                next.insert((
                                    canonical_contours(reflected_contours).unwrap(),
                                    routed,
                                ));
                            }
                        }
                    }
                }
            }
            frontier = next.difference(&seeds).copied().collect();
            seeds.extend(next);
        }
        let mut rows = Vec::new();
        let order = IntegralOrder::new([true; 9], [false; 9]);
        for (contours, point) in &seeds {
            if is_scaleless(*contours, *point).unwrap() {
                continue;
            }
            let seed = Seed {
                integral: Integral::numeric(*point).unwrap(),
                shifts: [0; 9],
            };
            for source in &sources.rows {
                let instantiated = crate::solver::instantiate::instantiate(
                    source,
                    &seed,
                    &sources.index_variables,
                    &[None; 9],
                    &order,
                    &[],
                    None,
                )
                .unwrap();
                let mut row = Row::new();
                for term in instantiated {
                    route(
                        &mut row,
                        *contours,
                        term.integral.powers().map(|p| p.value()),
                        &term.coefficient,
                        c,
                    );
                }
                if !row.is_empty() {
                    rows.push(row);
                }
            }
            for image in symmetry_images(*contours, *point).unwrap() {
                let mut row = Row::new();
                route(&mut row, *contours, *point, &c.one(), c);
                for (child, factor) in image {
                    route(&mut row, *contours, child, &c.integer(-factor), c);
                }
                if !row.is_empty() {
                    rows.push(row);
                }
            }
            for (reflected_contours, image) in bubble_reflections(*contours, *point).unwrap() {
                let mut row = Row::new();
                route(&mut row, *contours, *point, &c.one(), c);
                for (child, factor) in image {
                    route(&mut row, reflected_contours, child, &c.integer(-factor), c);
                }
                if !row.is_empty() {
                    rows.push(row);
                }
            }
        }
        // Admit short identities (especially exact routing symmetries) before
        // long IBPs to avoid unnecessary rational-function fill during replay.
        // This stable ordering only changes elimination, never the row space.
        rows.sort_by_cached_key(|row| {
            (
                row.len(),
                row.values()
                    .filter(|value| {
                        !value.numerator.is_constant() || !value.denominator.is_constant()
                    })
                    .count(),
            )
        });
        let columns: Vec<_> = rows
            .iter()
            .flat_map(|row| row.keys())
            .chain(target.keys())
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let ids = |row: &Row| {
            row.keys()
                .map(|point| columns.binary_search(point).unwrap() as u32)
                .collect::<Vec<_>>()
        };
        let field = Zp64::new(2_147_483_647);
        let evaluation = (0..c.parameter_names().len())
            .map(|i| Integer::from(17 + i).to_finite_field(&field))
            .collect::<Vec<_>>();
        let evaluate = |value: &Coefficient| {
            let n = value.numerator.evaluate_with_coeff_map(
                |v| v.to_finite_field(&field),
                &evaluation,
                &field,
            );
            let d = value.denominator.evaluate_with_coeff_map(
                |v| v.to_finite_field(&field),
                &evaluation,
                &field,
            );
            field.div(&n, &d)
        };
        let generation_elapsed = before.elapsed();
        let modular_start = Instant::now();
        let mut modular =
            SparseRowReducer::new(columns.len() as u32 + 1, field.clone(), LuLMode::Full);
        let mut retained = Vec::new();
        let mut accepted_l_rows = Vec::new();
        for (ordinal, row) in rows.iter().enumerate() {
            if modular
                .add_row(&row.values().map(&evaluate).collect::<Vec<_>>(), &ids(row))
                .is_some()
            {
                retained.push(ordinal);
                accepted_l_rows.push(modular.l().nrows() as usize - 1);
            }
        }
        let modular_pass = modular
            .add_row(
                &target.values().map(&evaluate).collect::<Vec<_>>(),
                &ids(&target),
            )
            .is_none();
        let modular_elapsed = modular_start.elapsed();
        eprintln!(
            "reference {}: seeds={} rows={} columns={} rank={} generation={:?} modular={:?} modular_pass={}",
            reference.name,
            seeds.len(),
            rows.len(),
            columns.len(),
            retained.len(),
            generation_elapsed,
            modular_elapsed,
            modular_pass
        );
        if !modular_pass {
            unresolved.push(reference.name);
            continue;
        }
        // Follow native L-pattern dependencies of this dependent query. The
        // reference row is never retained as an identity for another query.
        // Pattern reachability retains ancestors whose contributions can cancel
        // in the final original-source combination. A finite-field backsolve
        // finds those cancellations cheaply. Zeros here only propose a smaller
        // exact problem; the larger pattern trace remains the fallback. An
        // unlucky specialization can omit necessary sources from BOTH supports.
        let weights_started = Instant::now();
        let (trace, weighted_trace) = modular_source_supports(
            modular.l(),
            &accepted_l_rows,
            &retained,
            modular.l().nrows() as usize - 1,
        );
        let weights_elapsed = weights_started.elapsed();
        drop(modular);
        eprintln!(
            "reference {}: exact_trace_rows={} modular_weight_support={} weight_backsolve={:?}",
            reference.name,
            trace.len(),
            weighted_trace.len(),
            weights_elapsed
        );
        let mut passed = replay_reference_trace(
            reference.name,
            "modular-weight-support",
            &rows,
            &columns,
            &target,
            &weighted_trace,
            c,
        );
        if !passed && weighted_trace != trace {
            passed = replay_reference_trace(
                reference.name,
                "pattern-fallback",
                &rows,
                &columns,
                &target,
                &trace,
                c,
            );
        }
        if !passed {
            eprintln!(
                "reference {}: exact-trace-unresolved modular_weight_support={} pattern_support={}; both bounded proposals missed exact membership",
                reference.name,
                weighted_trace.len(),
                trace.len()
            );
            unresolved.push(reference.name);
            continue;
        }
        verified.push(reference.name);
    }
    assert!(
        !verified.is_empty() || !unresolved.is_empty(),
        "filter matched no reference"
    );
    eprintln!(
        "reference outcomes: verified={verified:?} unresolved={unresolved:?} total={:?}",
        started.elapsed()
    );
    if std::env::var("RUSTRED_REFERENCE_REQUIRE_CLOSURE").as_deref() == Ok("1") {
        assert!(
            unresolved.is_empty(),
            "bounded reference search left unresolved queries: {unresolved:?}"
        );
    }
}

#[test]
fn full_l_support_cancels_ancestors_and_preserves_original_source_positions() {
    let field = Zp64::new(2_147_483_647);
    let values = |coefficients: &[i64]| {
        coefficients
            .iter()
            .map(|value| Integer::from(*value).to_finite_field(&field))
            .collect::<Vec<_>>()
    };
    let mut reducer = SparseRowReducer::new(4, field.clone(), LuLMode::Full);
    assert!(reducer.add_row(&values(&[1, 1]), &[0, 1]).is_some());
    // A dependent input contributes a native L row but no accepted source.
    assert!(reducer.add_row(&values(&[2, 2]), &[0, 1]).is_none());
    assert!(reducer.add_row(&values(&[1, 1]), &[0, 2]).is_some());
    assert!(reducer.add_row(&values(&[1, 2, 3]), &[0, 1, 2]).is_some());
    assert!(reducer.add_row(&values(&[1, 2, 3]), &[0, 1, 2]).is_none());
    let lower = reducer.l();
    let query = lower.nrows() as usize - 1;
    assert_eq!(lower.row_ptrs()[query + 1] - lower.row_ptrs()[query], 3);
    let weights = original_source_weights(lower, &[0, 2, 3], query);
    // All three accepted rows enter the elimination trace, but the query is
    // exactly the third original row. Ancestor weights cancel in the backsolve.
    assert_eq!(weights, values(&[0, 0, 1]));
}

#[test]
fn transposed_certificate_checks_columns_after_the_first_full_rank_subsystem() {
    let context = CoefficientContext::new(["d"]);
    let first = ([0, 0, 1], key([1, 0, 0, 0, 0, 0], [0; 3]));
    let second = ([0, 0, 1], key([2, 0, 0, 0, 0, 0], [0; 3]));
    let source = Row::from([(first, context.one()), (second, context.one())]);
    let inconsistent_target = Row::from([(first, context.one()), (second, context.integer(2))]);
    // The first transposed equation fixes the only weight to 1. Elimination
    // stops at full rank before the equally short second equation, which would
    // require weight 2. Only full original-source multiplication detects this.
    assert!(!replay_reference_trace(
        "late-inconsistent-column-regression",
        "transposed-subsystem",
        std::slice::from_ref(&source),
        &[first, second],
        &inconsistent_target,
        &[0],
        &context,
    ));
    assert!(replay_reference_trace(
        "late-inconsistent-column-regression",
        "consistent-control",
        std::slice::from_ref(&source),
        &[first, second],
        &source,
        &[0],
        &context,
    ));
}

#[test]
fn transposed_certificate_recovers_symbolic_weights_with_nonunit_pivots() {
    let context = CoefficientContext::new(["d"]);
    let first = ([0, 0, 1], key([1, 0, 0, 0, 0, 0], [0; 3]));
    let second = ([0, 0, 1], key([2, 0, 0, 0, 0, 0], [0; 3]));
    let rows = vec![
        Row::from([
            (first, context.coefficient_fixture("d")),
            (second, context.one()),
        ]),
        Row::from([(first, context.one()), (second, context.integer(2))]),
    ];
    // This target is ((d+1)/2)*source_0 + 3*source_1. Solving the
    // transposed system introduces a nonconstant pivot before cancellation.
    let target = Row::from([
        (first, context.coefficient_fixture("d*(d+1)/2+3")),
        (second, context.coefficient_fixture("(d+1)/2+6")),
    ]);
    assert!(replay_reference_trace(
        "symbolic-certificate-regression",
        "transposed-symbolic",
        &rows,
        &[first, second],
        &target,
        &[0, 1],
        &context,
    ));
}

#[test]
fn unlucky_specialization_can_lose_both_bounded_exact_supports() {
    let context = CoefficientContext::new(["d"]);
    let first = ([0, 0, 1], key([1, 0, 0, 0, 0, 0], [0; 3]));
    let second = ([0, 0, 1], key([2, 0, 0, 0, 0, 0], [0; 3]));
    let rows = vec![
        Row::from([(first, context.one())]),
        Row::from([(second, context.one())]),
    ];
    let target = Row::from([
        (first, context.one()),
        (second, context.coefficient_fixture("d-17")),
    ]);
    let field = Zp64::new(2_147_483_647);
    let values = |coefficients: &[i64]| {
        coefficients
            .iter()
            .map(|value| Integer::from(*value).to_finite_field(&field))
            .collect::<Vec<_>>()
    };
    let mut modular = SparseRowReducer::new(3, field.clone(), LuLMode::Full);
    assert!(modular.add_row(&values(&[1]), &[0]).is_some());
    assert!(modular.add_row(&values(&[1]), &[1]).is_some());
    // This is the actual target specialization at d=17, including its zero.
    assert!(modular.add_row(&values(&[1, 0]), &[0, 1]).is_none());
    let (pattern_trace, weighted_trace) = modular_source_supports(
        modular.l(),
        &[0, 1],
        &[0, 1],
        modular.l().nrows() as usize - 1,
    );
    assert_eq!(pattern_trace, vec![0]);
    assert_eq!(weighted_trace, vec![0]);
    // Both production support proposals omit source 1. Their exact misses
    // must remain unresolved at this bound, not disprove the reference.
    assert!(!replay_reference_trace(
        "specialized-zero-regression",
        "modular-weight-support",
        &rows,
        &[first, second],
        &target,
        &weighted_trace,
        &context,
    ));
    assert!(!replay_reference_trace(
        "specialized-zero-regression",
        "actual-pattern-support",
        &rows,
        &[first, second],
        &target,
        &pattern_trace,
        &context,
    ));
    // An independent diagnostic full-row check proves that the query is true.
    // The bounded production probe intentionally does not trigger this larger
    // problem automatically after a support miss.
    assert!(replay_reference_trace(
        "specialized-zero-regression",
        "diagnostic-full-source-check",
        &rows,
        &[first, second],
        &target,
        &[0, 1],
        &context,
    ));
}

/// This checks conversion coverage, separately from the reduction proofs
/// above. The output is a sum of occupied phase-space integrals of vacuum
/// kernels and their mixed energy derivatives; those kernels are not evaluated
/// here, and enumerating their recipes does not prove a reference equality.
#[test]
fn every_supplied_three_loop_term_has_an_explicit_formal_cut_conversion() {
    use super::reference_cut_plan::cut_plans;

    let started = Instant::now();
    let context = CoefficientContext::new(["d"]);
    let mut terms = Vec::new();
    for reference in references() {
        for (contours, point, _) in reference.terms {
            terms.push((reference.name, contours, point));
        }
    }
    // Include every supplied tadpole-times-sunset LHS. Their factorization
    // proof is independent of this cut enumeration and lives in routing tests.
    for (contours, powers) in [
        ([1, 1, 1], [1, 1, 1, 1, 0, 0]),
        ([1, 1, 1], [1, 1, 1, 0, 1, 0]),
        ([1, 1, 0], [1, 1, 1, 0, 1, 0]),
        ([1, 1, 0], [1, 1, 0, 1, 0, 1]),
    ] {
        terms.push(("R4-factorization", contours, key(powers, [0; 3])));
    }
    let mut plan_count = 0;
    let mut jet_count = 0;
    let mut imaginary_numerator_terms = 0;
    let mut negative_real_numerator_terms = 0;
    let mut surface_jets = 0;
    let mut differentiated_kernel_jets = 0;
    let mut mixed_surface_jets = 0;
    let mut mixed_kernel_jets = 0;
    let mut negative_cut_orientations = 0;
    let mut polynomial_kernels = 0;
    let mut raised_uncut_kernels = 0;
    let mut max_surface_order = 0;
    let mut max_kernel_derivative = 0;
    for &(label, contours, point) in &terms {
        let plans = cut_plans(&context, contours, point)
            .unwrap_or_else(|error| panic!("{label}, {contours:?}, {point:?}: {error}"));
        assert!(!plans.is_empty());
        assert_eq!(plans[0].cut_mask, 0);
        let original_powers = std::array::from_fn(|axis| point[axis]);
        assert_eq!(plans[0].kernel_powers, original_powers);
        plan_count += plans.len();
        for plan in plans {
            assert!(!plan.energy_numerator.is_empty());
            assert!(!plan.kernel_jets.is_empty());
            for leg in &plan.legs {
                assert!(point[leg.line] > 0, "a polynomial is not a cut propagator");
                assert_eq!(leg.original_power, point[leg.line] as u32);
                negative_cut_orientations += usize::from(leg.orientation == -1);
            }
            imaginary_numerator_terms += plan
                .energy_numerator
                .values()
                .filter(|(_, imag)| *imag != 0)
                .count();
            negative_real_numerator_terms += plan
                .energy_numerator
                .values()
                .filter(|(real, _)| *real < 0)
                .count();
            polynomial_kernels += usize::from(plan.kernel_powers.iter().any(|power| *power < 0));
            raised_uncut_kernels += usize::from(plan.kernel_powers.iter().any(|power| *power > 1));
            jet_count += plan.kernel_jets.len();
            for (jets, coefficient) in &plan.kernel_jets {
                assert!(!coefficient.is_zero());
                let surfaces = jets.iter().filter(|(_, _, surface)| *surface > 0).count();
                let derivatives = jets
                    .iter()
                    .filter(|(_, derivative, _)| *derivative > 0)
                    .count();
                surface_jets += usize::from(surfaces > 0);
                differentiated_kernel_jets += usize::from(derivatives > 0);
                mixed_surface_jets += usize::from(surfaces > 1);
                mixed_kernel_jets += usize::from(derivatives > 1);
                max_surface_order =
                    max_surface_order.max(jets.iter().map(|(_, _, b)| *b).max().unwrap());
                max_kernel_derivative =
                    max_kernel_derivative.max(jets.iter().map(|(_, r, _)| *r).max().unwrap());
            }
        }
    }
    assert_eq!(
        terms.len(),
        30,
        "all supplied D1 terms, including products and four factorizations"
    );
    assert!(plan_count > terms.len());
    assert!(jet_count > plan_count);
    assert!(imaginary_numerator_terms > 0 && negative_real_numerator_terms > 0);
    assert!(surface_jets > 0 && differentiated_kernel_jets > 0);
    assert!(mixed_surface_jets > 0 && mixed_kernel_jets > 0);
    assert!(negative_cut_orientations > 0);
    assert!(polynomial_kernels > 0 && raised_uncut_kernels > 0);
    assert_eq!(max_surface_order, 2);
    assert_eq!(max_kernel_derivative, 2);
    eprintln!(
        "supplied D1 formal cut conversion: reference_terms={} cut_plans={plan_count} full_kernel_jets={jet_count} surface_jets={surface_jets} differentiated_kernel_jets={differentiated_kernel_jets} mixed_surface_jets={mixed_surface_jets} mixed_kernel_jets={mixed_kernel_jets} imaginary_numerator_terms={imaginary_numerator_terms} max_surface_order={max_surface_order} max_kernel_derivative={max_kernel_derivative} elapsed={:?}; remaining vacuum kernels are unevaluated",
        terms.len(),
        started.elapsed()
    );
}
