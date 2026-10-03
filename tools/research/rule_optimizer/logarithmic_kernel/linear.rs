//! Native sparse reducers and products; no elimination or back-substitution here.
use super::*;
use std::cmp::Ordering;

fn bounded_matrix(m: &Matrix, r: &Value) -> Result<()> {
    admit(r, "max_reducer_nonzeros", m.nvalues())?;
    let mut terms = 0;
    for c in m.values() {
        terms = add(terms, add(c.numerator.nterms(), c.denominator.nterms())?)?;
        admit(r, "max_coefficient_terms", terms)?;
    }
    Ok(())
}
fn reducer_bound(s: &SparseRowReducer<Field>, r: &Value) -> Result<()> {
    admit(
        r,
        "max_reducer_nonzeros",
        add(s.u().nvalues(), s.l().nvalues())?,
    )?;
    bounded_matrix(s.u(), r)?;
    bounded_matrix(s.l(), r)
}
fn pivot_guard(s: &SparseRowReducer<Field>, row: usize) -> Result<Value> {
    let (_, _, v) = s.l().last_row().ok_or("native L row missing")?;
    let c = v.last().ok_or("native pivot scale missing")?;
    require(
        !c.numerator.is_zero() && !c.denominator.is_zero(),
        "zero pivot scale",
    )?;
    Ok(
        json!({"input_row":row,"numerator_nonzero":c.numerator.to_string(),"denominator_nonzero":c.denominator.to_string()}),
    )
}
pub(super) fn product(a: &Matrix, b: &Matrix, r: &Value) -> Result<Matrix> {
    require(
        a.ncols() == b.nrows(),
        "native matrix product shape mismatch",
    )?;
    // Exact count of scalar-product contributions before native coalescing.
    let mut operations = 0;
    for &col in a.col_idcs() {
        let i = col as usize;
        operations = add(operations, b.row_ptrs()[i + 1] - b.row_ptrs()[i])?;
    }
    admit(r, "max_exact_operations", operations)?;
    admit(r, "max_product_terms", operations)?;
    let result = a * b;
    bounded_matrix(&result, r)?;
    Ok(result)
}
pub(super) fn certify_kernel(weights: &Matrix, constraints: &Matrix, r: &Value) -> Result<()> {
    let replay = product(weights, constraints, r)?;
    require(
        replay.values().iter().all(|v| v.is_zero()),
        "kernel weights fail full native constraint replay",
    )
}
pub(super) struct Kernel {
    pub weights: Matrix,
    pub report: Value,
}
pub(super) fn left_kernel(c: &Matrix, one: &Coefficient, r: &Value) -> Result<Kernel> {
    let width = add(c.ncols() as usize, c.nrows() as usize)?;
    admit(r, "max_augmented_columns", width)?;
    admit(
        r,
        "max_input_nonzeros",
        add(c.nvalues(), c.nrows() as usize)?,
    )?;
    let mut reducer = SparseRowReducer::new(checked(u32::try_from(width))?, field(), LuLMode::Full);
    let mut weights = Matrix::new(0, c.nrows(), field());
    let mut guards = Vec::new();
    let mut rank = 0;
    for row in 0..c.nrows() as usize {
        let range = c.row_ptrs()[row]..c.row_ptrs()[row + 1];
        let mut ids = c.col_idcs()[range.clone()].to_vec();
        let mut values = c.values()[range].to_vec();
        ids.push(c.ncols() + row as u32);
        values.push(one.clone());
        let pivot = reducer
            .add_row(&values, &ids)
            .ok_or("identity-augmented row lost independence")?;
        reducer_bound(&reducer, r)?;
        guards.push(pivot_guard(&reducer, row)?);
        admit(r, "max_conditions", guards.len())?;
        if pivot < c.ncols() {
            rank += 1;
            continue;
        }
        let (_, ids, values) = reducer.u().last_row().ok_or("native U row missing")?;
        require(
            ids.iter().all(|&id| id >= c.ncols()),
            "native kernel row retains constraint column",
        )?;
        let out_ids = ids.iter().map(|&id| id - c.ncols()).collect();
        admit(r, "max_kernel_vectors", weights.nrows() as usize + 1)?;
        weights.add_row(values.to_vec(), out_ids);
        bounded_matrix(&weights, r)?;
    }
    require(
        rank + weights.nrows() as usize == c.nrows() as usize,
        "native rank-nullity mismatch",
    )?;
    certify_kernel(&weights, c, r)?;
    Ok(Kernel {
        report: json!({"constraint_rank":rank,"kernel_dimension":weights.nrows(),"coefficient_field":"K",
        "full_constraint_replay_zero":true,"all_native_kernel_rows_retained":true,"pivot_guards_before_cancellation":guards,
        "native_reducer_nonzeros":reducer.u().nvalues()+reducer.l().nvalues()}),
        weights,
    })
}
pub(super) fn rank(frame: &Matrix, r: &Value) -> Result<Value> {
    admit(r, "max_augmented_columns", frame.ncols() as usize)?;
    let mut reducer = SparseRowReducer::new(frame.ncols(), field(), LuLMode::Full);
    let mut guards = Vec::new();
    for row in 0..frame.nrows() as usize {
        let range = frame.row_ptrs()[row]..frame.row_ptrs()[row + 1];
        let pivot = reducer.add_row(&frame.values()[range.clone()], &frame.col_idcs()[range]);
        reducer_bound(&reducer, r)?;
        if pivot.is_some() {
            guards.push(pivot_guard(&reducer, row)?);
            admit(r, "max_conditions", guards.len())?;
        }
    }
    Ok(json!({"rank":reducer.u().nrows(),"pivot_guards_for_rank_only":guards}))
}
pub(super) fn full_tail<const N: usize>(
    row: &Matrix,
    columns: &[Integral<N>],
    target: &Integral<N>,
    order: &IntegralOrder<N>,
) -> Result<Value> {
    require(
        row.nrows() == 1 && row.ncols() as usize == columns.len(),
        "full replay shape mismatch",
    )?;
    let mut target_seen = false;
    let mut tail = Vec::new();
    for (&col, value) in row.col_idcs().iter().zip(row.values()) {
        if value.is_zero() {
            continue;
        }
        let k = &columns[col as usize];
        if k == target {
            require(value.is_one(), "full replay target not one")?;
            target_seen = true;
        } else {
            require(
                order.compare(k, target) == Ordering::Greater,
                "full replay retains a forbidden non-lower key",
            )?;
            tail.push(json!({"key":powers(k),"coefficient":coefficient(&-value.clone())}));
        }
    }
    require(target_seen, "full replay target absent")?;
    Ok(json!(tail))
}
pub(super) struct PointSolution {
    pub weights: Option<Matrix>,
    pub report: Value,
}
pub(super) fn target_pivot<const N: usize>(
    frame: &Matrix,
    columns: &[Integral<N>],
    target: &Integral<N>,
    order: &IntegralOrder<N>,
    one: &Coefficient,
    r: &Value,
) -> Result<PointSolution> {
    let bad = columns
        .iter()
        .enumerate()
        .filter_map(|(i, k)| {
            (k != target && order.compare(k, target) != Ordering::Greater).then_some(i)
        })
        .collect::<Vec<_>>();
    let tc = columns
        .iter()
        .position(|k| k == target)
        .ok_or("target column absent")?;
    let projected_target = checked(u32::try_from(bad.len()))?;
    let identity = projected_target.checked_add(1).ok_or("width overflow")?;
    let width = add(identity as usize, frame.nrows() as usize)?;
    admit(r, "max_augmented_columns", width)?;
    let mut reducer = SparseRowReducer::new(checked(u32::try_from(width))?, field(), LuLMode::Full);
    let mut guards = Vec::new();
    for row in 0..frame.nrows() as usize {
        let mut entries = BTreeMap::new();
        for at in frame.row_ptrs()[row]..frame.row_ptrs()[row + 1] {
            let col = frame.col_idcs()[at] as usize;
            let mapped = if col == tc {
                Some(projected_target)
            } else {
                bad.binary_search(&col).ok().map(|i| i as u32)
            };
            if let Some(id) = mapped {
                entries.insert(id, frame.values()[at].clone());
            }
        }
        entries.insert(identity + row as u32, one.clone());
        let pivot = reducer
            .add_row(
                &entries.values().cloned().collect::<Vec<_>>(),
                &entries.keys().copied().collect::<Vec<_>>(),
            )
            .ok_or("point identity row dependent")?;
        reducer_bound(&reducer, r)?;
        guards.push(pivot_guard(&reducer, row)?);
        admit(r, "max_conditions", guards.len())?;
        if pivot != projected_target {
            continue;
        }
        let (_, ids, values) = reducer.u().last_row().ok_or("point native U row missing")?;
        require(
            ids.first() == Some(&projected_target) && values.first().is_some_and(|v| v.is_one()),
            "point target pivot not normalized",
        )?;
        require(
            ids.iter().skip(1).all(|&id| id >= identity),
            "uneliminated point F column",
        )?;
        let mut weights = Matrix::new(0, frame.nrows(), field());
        weights.add_row(
            values.iter().skip(1).cloned().collect(),
            ids.iter().skip(1).map(|&id| id - identity).collect(),
        );
        let replay = product(&weights, frame, r)?;
        let tail = full_tail(&replay, columns, target, order)?;
        return Ok(PointSolution {
            weights: Some(weights),
            report: json!({"status":"EXACT_FINITE_LOGARITHMIC_SOURCE_COMBINATION","first_target_prefix":row+1,"forbidden_columns":bad.len(),
            "pivot_guards_before_cancellation":guards,"all_F_zero":true,"target_one":true,"same_saved_order_strict_descent":true,"rhs":tail}),
        });
    }
    Ok(PointSolution {
        weights: None,
        report: json!({"status":"NO_TARGET_PIVOT_IN_FIXED_DEGREE_KERNEL","forbidden_columns":bad.len(),
        "pivot_guards_before_cancellation":guards,"finite_degree_negative_only":true,"master_or_irreducibility_claim":false}),
    })
}
