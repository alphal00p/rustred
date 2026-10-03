//! Optional native [F,target] reduction with one native L-transpose solve.
//! Only accepted physical rows are mapped back to original inputs. No custom
//! elimination, back substitution, inverse, coefficient parser or source proof.
use super::project::{self, *};
use rustred::{algebra::IndexedCoefficientContext, identity::IndexShift};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use symbolica::{
    domains::SelfRing,
    prelude::Z,
    tensors::sparse::{LuLMode, SparseRowReducer, SparseVector},
};

pub fn enabled(r: &Value) -> super::Result<bool> {
    match r.get("projection_backend") {
        None => Ok(false),
        Some(Value::String(v)) if v == "augmented" => Ok(false),
        Some(Value::String(v)) if v == "direct-l" => Ok(true),
        _ => Err("projection_backend must be augmented or direct-l".into()),
    }
}

/// Native accepted A=L*U. The target U row is recovered by L^T*w=e_target.
/// The native L row number is NOT the input ordinal: empty/full-rank inputs
/// append no L row, whereas a nonempty dependent input can append one.
struct Accepted {
    input: usize,
    l_row: usize,
}

fn recover(
    c: &IndexedCoefficientContext,
    variables: &project::native_variables::Variables,
    reducer: &SparseRowReducer<Field>,
    accepted: &[Accepted],
    target_basis: usize,
    retained_terms: &mut usize,
    limits: Limits,
) -> Result<Weights> {
    let rank = accepted.len();
    require(
        rank == reducer.u().nrows() as usize && target_basis < rank,
        "accepted basis/target rank mismatch",
    )?;
    let mut triplets = Vec::new();
    for (basis, binding) in accepted.iter().enumerate() {
        let start = reducer.l().row_ptrs()[binding.l_row];
        let end = reducer.l().row_ptrs()[binding.l_row + 1];
        for at in start..end {
            let column = reducer.l().col_idcs()[at] as usize;
            require(column <= basis, "native accepted L is not triangular")?;
            bound(
                add(
                    add(reducer.u().nvalues(), reducer.l().nvalues())?,
                    add(triplets.len(), 1)?,
                )?,
                limits.nonzeros,
                "retained accepted L transpose entries",
            )?;
            triplets.push((
                column as u32,
                basis as u32,
                reducer.l().values()[at].clone(),
            ));
        }
    }
    // Native from_triplets requires strict row/column order, not L visitation.
    triplets.sort_by_key(|(row, column, _)| (*row, *column));
    require(
        triplets
            .windows(2)
            .all(|p| (p[0].0, p[0].1) < (p[1].0, p[1].1)),
        "duplicate accepted L transpose entry",
    )?;
    let n = u32::try_from(rank).map_err(|_| Error::Budget("native accepted rank"))?;
    let transpose = native(|| Matrix::from_triplets(n, n, triplets, Field::new(Z)))?;
    variables.matrix_bound(c, &transpose, limits)?;
    variables.admit_values(c, transpose.values(), retained_terms, limits)?;
    // Charge the cloned native solve input BEFORE allocating it. Native solve
    // scratch is additionally constrained by the outer process/time guard.
    bound(
        add(
            add(reducer.u().nvalues(), reducer.l().nvalues())?,
            add(add(transpose.nvalues(), transpose.nvalues())?, 1)?,
        )?,
        limits.nonzeros,
        "retained reduction plus native solve input",
    )?;
    bound(
        add(rank, 1)?,
        limits.columns,
        "native solve augmented columns",
    )?;
    variables.admit_values(c, transpose.values(), retained_terms, limits)?;
    let one = variables.map(c.one().raw())?;
    variables.admit_values(c, std::slice::from_ref(&one), retained_terms, limits)?;
    // The solve internally performs native forward/back substitution. Since
    // this is triangular, its pivots are exactly the retained L diagonals.
    super::progress::event("direct_l_recovery_start", || {
        json!({"rank":rank,
        "l_transpose_nonzeros":transpose.nvalues()})
    });
    let unit = SparseVector::from_csr(n, vec![one], vec![target_basis as u32], Field::new(Z));
    let solution = native(|| transpose.clone().solve(unit))?
        .map_err(|e| Error::Invalid(format!("native L transpose solve failed: {e}")))?;
    require(solution.len() == n, "native solve vector shape differs")?;
    // SparseVector intentionally exposes no raw getters. Its public append_col
    // bridge preserves native typed entries without a string round trip.
    let mut weights_column = Matrix::new(n, 0, Field::new(Z));
    native(|| weights_column.append_col(solution))?;
    variables.matrix_bound(c, &weights_column, limits)?;
    variables.admit_values(c, weights_column.values(), retained_terms, limits)?;
    bound(
        add(
            add(reducer.u().nvalues(), reducer.l().nvalues())?,
            add(transpose.nvalues(), weights_column.nvalues())?,
        )?,
        limits.nonzeros,
        "retained reduction/recovery nonzeros",
    )?;
    let equation = native(|| &transpose * &weights_column)?;
    variables.matrix_bound(c, &equation, limits)?;
    variables.admit_values(c, equation.values(), retained_terms, limits)?;
    bound(
        add(
            add(
                add(reducer.u().nvalues(), reducer.l().nvalues())?,
                transpose.nvalues(),
            )?,
            add(weights_column.nvalues(), equation.nvalues())?,
        )?,
        limits.nonzeros,
        "retained recovery/replay nonzeros",
    )?;
    require(
        equation.nrows() == n
            && equation.ncols() == 1
            && equation.nvalues() == 1
            && equation.row_ptrs()[target_basis] == 0
            && equation.row_ptrs()[target_basis + 1] == 1
            && equation.col_idcs()[0] == 0
            && equation.values()[0].is_one(),
        "native triangular solution replay failed",
    )?;
    let mut weights = Weights::new();
    for (basis, binding) in accepted.iter().enumerate() {
        for at in weights_column.row_ptrs()[basis]..weights_column.row_ptrs()[basis + 1] {
            require(weights_column.col_idcs()[at] == 0, "weight column changed")?;
            let value = variables.admit(c, &weights_column.values()[at], limits)?;
            // Common replay retains this weight's denominator BEFORE the
            // source product, exactly as in the default augmented backend.
            require(
                !value.is_zero(),
                "native solve retained explicit zero weight",
            )?;
            weights.insert(binding.input, value);
        }
    }
    super::progress::event(
        "direct_l_recovery_finished",
        || json!({"rank":rank,"weights":weights.len()}),
    );
    Ok(weights)
}

pub fn project(
    c: &IndexedCoefficientContext,
    rows: &[Row],
    target: &IndexShift,
    forbidden: &BTreeSet<IndexShift>,
    input_guards: &[Guard],
    limits: Limits,
) -> Result<Projection> {
    project_with_compaction(c, rows, target, forbidden, input_guards, limits, false)
}

pub fn project_with_compaction(
    c: &IndexedCoefficientContext,
    rows: &[Row],
    target: &IndexShift,
    forbidden: &BTreeSet<IndexShift>,
    input_guards: &[Guard],
    limits: Limits,
    compact: bool,
) -> Result<Projection> {
    bound(rows.len(), limits.rows, "source-span rows")?;
    require(!rows.is_empty(), "empty symbolic source span")?;
    require(
        target.values().len() == c.index_count(),
        "target shift arity differs",
    )?;
    let universe = rows
        .iter()
        .flat_map(|row| row.keys().cloned())
        .collect::<BTreeSet<_>>();
    require(!forbidden.contains(target), "target cannot be forbidden")?;
    // Preserve full caller F even when a selected subbank has structural
    // zeros in some columns. Native reduction and replay still see all F.
    for shift in forbidden {
        require(
            shift.values().len() == c.index_count(),
            "forbidden shift arity differs",
        )?;
    }
    bound(universe.len(), limits.columns, "image shift columns")?;
    let width = add(forbidden.len(), 1)?;
    bound(width, limits.columns, "physical projection columns")?;
    super::progress::projection_start(rows.len(), forbidden.len(), width);
    let mut guards = Vec::new();
    for g in input_guards {
        retain(
            c,
            &mut guards,
            g.polynomial.clone(),
            g.origin.clone(),
            limits,
        )?;
    }
    let mut inputs = 0;
    let mut input_terms = 0;
    for row in rows {
        for (shift, value) in row {
            require(
                shift.values().len() == c.index_count(),
                "image shift arity differs",
            )?;
            require(!value.is_zero(), "explicit zero in source image")?;
            denominator(c, &mut guards, value, "admitted span input", limits)?;
            inputs = add(inputs, 1)?;
            input_terms = add(
                input_terms,
                add(
                    value.raw().numerator.nterms(),
                    value.raw().denominator.nterms(),
                )?,
            )?;
            bound(
                input_terms,
                limits.coefficient_terms,
                "admitted span coefficient terms",
            )?;
        }
    }
    bound(inputs, limits.nonzeros, "projection inputs")?;
    let variables = project::native_variables::Variables::new(c, rows, compact, limits)?;
    let native_width = u32::try_from(width).map_err(|_| Error::Budget("native physical width"))?;
    let mut reducer = native(|| SparseRowReducer::new(native_width, Field::new(Z), LuLMode::Full))?;
    let f = forbidden.iter().cloned().collect::<Vec<_>>();
    let mut accepted = Vec::new();
    let mut retained_terms = 0;
    for (ordinal, row) in rows.iter().enumerate() {
        let mut values = Vec::new();
        let mut ids = Vec::new();
        for (i, shift) in f.iter().enumerate() {
            if let Some(v) = row.get(shift) {
                ids.push(i as u32);
                values.push(variables.map(v.raw())?);
            }
        }
        if let Some(v) = row.get(target) {
            ids.push(forbidden.len() as u32);
            values.push(variables.map(v.raw())?);
        }
        let before_u = reducer.u().nrows() as usize;
        let before_l = reducer.l().nrows() as usize;
        let pivot = native(|| reducer.add_row(&values, &ids))?;
        let after_u = reducer.u().nrows() as usize;
        let after_l = reducer.l().nrows() as usize;
        bound(
            add(reducer.u().nvalues(), reducer.l().nvalues())?,
            limits.nonzeros,
            "native U/L nonzeros",
        )?;
        super::progress::rows(
            ordinal + 1,
            forbidden.len(),
            reducer.u().nvalues(),
            reducer.l().nvalues(),
        );
        require(
            after_l == before_l || after_l == add(before_l, 1)?,
            "native L append chronology changed",
        )?;
        let admitted_l = if after_l > before_l {
            let (_, _, raw) = reducer
                .l()
                .last_row()
                .ok_or_else(|| Error::Invalid("native L row missing".into()))?;
            variables.admit_values(c, raw, &mut retained_terms, limits)?
        } else {
            Vec::new()
        };
        let Some(pivot) = pivot else {
            require(after_u == before_u, "dependent row appended U")?;
            continue;
        };
        require(
            after_u == add(before_u, 1)? && after_l == add(before_l, 1)?,
            "accepted U/L append chronology changed",
        )?;
        let (urow, uids, uvalues) = reducer
            .u()
            .last_row()
            .ok_or_else(|| Error::Invalid("native U row missing".into()))?;
        let (lrow, lids, _) = reducer
            .l()
            .last_row()
            .ok_or_else(|| Error::Invalid("native L row missing".into()))?;
        require(
            urow as usize == before_u
                && lrow as usize == before_l
                && lids.last().copied() == Some(urow)
                && lids
                    .iter()
                    .take(lids.len().saturating_sub(1))
                    .all(|id| *id < urow),
            "accepted L dependency chronology changed",
        )?;
        let admitted_u = variables.admit_values(c, uvalues, &mut retained_terms, limits)?;
        let scale = admitted_l
            .last()
            .ok_or_else(|| Error::Invalid("native pivot absent".into()))?;
        retain(
            c,
            &mut guards,
            c.numerator_condition_with_limits(scale, limits.arithmetic)?,
            format!("physical pivot numerator row {ordinal}"),
            limits,
        )?;
        denominator(
            c,
            &mut guards,
            scale,
            format!("physical pivot denominator row {ordinal}"),
            limits,
        )?;
        accepted.push(Accepted {
            input: ordinal,
            l_row: before_l,
        });
        require(
            accepted.len() == after_u,
            "accepted input mapping rank mismatch",
        )?;
        if pivot as usize != forbidden.len() {
            continue;
        }
        require(
            uids == &[forbidden.len() as u32]
                && admitted_u.len() == 1
                && admitted_u[0].raw().is_one(),
            "native target is not normalized or retains F",
        )?;
        let weights = recover(
            c,
            &variables,
            &reducer,
            &accepted,
            before_u,
            &mut retained_terms,
            limits,
        )?;
        let image = project::replay(c, rows, &weights, &mut guards, limits)?;
        require(
            image.get(target).is_some_and(|v| v.raw().is_one())
                && forbidden.iter().all(|s| !image.contains_key(s)),
            "full symbolic target/F replay failed",
        )?;
        super::progress::event(
            "projection_target",
            || json!({"rows_visited":ordinal+1,"f_size":forbidden.len(),"weight_terms":weights.len(),"full_image_terms":image.len(),"backend":"direct-l"}),
        );
        return Ok(Projection::Target(Proposal {
            weights,
            image,
            guards,
            prefix_rows: ordinal + 1,
        }));
    }
    super::progress::event(
        "projection_miss",
        || json!({"rows_visited":rows.len(),"f_size":forbidden.len(),"backend":"direct-l"}),
    );
    Ok(Projection::NoTarget {
        guards,
        rows: rows.len(),
    })
}
