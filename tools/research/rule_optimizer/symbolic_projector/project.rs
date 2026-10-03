//! Bounded [F,target,I] orchestration of Symbolica's native sparse reducer.
//! This returns algebraic proposals, never source or rule authority.
use rustred::algebra::{
    ExactAlgebraLimits, IndexedAlgebraError, IndexedCoefficient, IndexedCoefficientContext,
    IndexedPolynomial,
};
use rustred::identity::IndexShift;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};
use symbolica::{
    domains::{SelfRing, rational_polynomial::RationalPolynomialField},
    prelude::{IntegerRing, Z},
    tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer},
};

pub type Row = BTreeMap<IndexShift, IndexedCoefficient>;
pub type Weights = BTreeMap<usize, IndexedCoefficient>;
pub(super) type Field = RationalPolynomialField<IntegerRing, u16>;
pub(super) type Matrix = SparseMatrix<Field>;
pub type Result<T> = std::result::Result<T, Error>;

#[path = "native_variables.rs"]
pub(super) mod native_variables;

#[derive(Debug)]
pub enum Error {
    Invalid(String),
    Budget(&'static str),
    Algebra(IndexedAlgebraError),
    NativePanic,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(s) => f.write_str(s),
            Self::Budget(s) => write!(f, "budget exhausted: {s}"),
            Self::Algebra(e) => e.fmt(f),
            Self::NativePanic => f.write_str("native Symbolica panic"),
        }
    }
}
impl From<IndexedAlgebraError> for Error {
    fn from(e: IndexedAlgebraError) -> Self {
        Self::Algebra(e)
    }
}
pub fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(message.into()))
    }
}
pub fn bound(n: usize, max: usize, resource: &'static str) -> Result<()> {
    if n <= max {
        Ok(())
    } else {
        Err(Error::Budget(resource))
    }
}
pub(super) fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(Error::Budget("count overflow"))
}
pub(super) fn native<T>(f: impl FnOnce() -> T) -> Result<T> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|_| Error::NativePanic)
}

#[derive(Clone, Copy)]
pub struct Limits {
    pub arithmetic: ExactAlgebraLimits,
    pub rows: usize,
    pub columns: usize,
    pub nonzeros: usize,
    pub coefficient_terms: usize,
    pub operations: usize,
    pub guards: usize,
}
#[derive(Clone, Debug)]
pub struct Guard {
    pub polynomial: IndexedPolynomial,
    pub origin: String,
}
pub fn retain(
    c: &IndexedCoefficientContext,
    guards: &mut Vec<Guard>,
    polynomial: IndexedPolynomial,
    origin: impl Into<String>,
    limits: Limits,
) -> Result<()> {
    c.validate_polynomial_with_limits(&polynomial, limits.arithmetic)?;
    require(
        !polynomial.is_zero(),
        "required pre-cancellation guard is identically zero",
    )?;
    // Native predicate considers ALL base/index variables. Only authenticated
    // nonzero constants are tautologies; parameter-only conditions stay live.
    if polynomial.is_nonzero_constant() {
        return Ok(());
    }
    bound(add(guards.len(), 1)?, limits.guards, "retained conditions")?;
    guards.push(Guard {
        polynomial,
        origin: origin.into(),
    });
    Ok(())
}
pub fn denominator(
    c: &IndexedCoefficientContext,
    guards: &mut Vec<Guard>,
    value: &IndexedCoefficient,
    origin: impl Into<String>,
    limits: Limits,
) -> Result<()> {
    retain(
        c,
        guards,
        c.denominator_condition_with_limits(value, limits.arithmetic)?,
        origin,
        limits,
    )
}
pub fn add_term(
    c: &IndexedCoefficientContext,
    row: &mut Row,
    shift: IndexShift,
    value: IndexedCoefficient,
    guards: &mut Vec<Guard>,
    limits: Limits,
) -> Result<()> {
    denominator(c, guards, &value, "term before coalescing", limits)?;
    let value = if let Some(old) = row.remove(&shift) {
        denominator(c, guards, &old, "accumulator before coalescing", limits)?;
        c.add_with_limits(&old, &value, limits.arithmetic)?
    } else {
        value
    };
    denominator(c, guards, &value, "coalesced term", limits)?;
    if !value.is_zero() {
        row.insert(shift, value);
    }
    bound(row.len(), limits.columns, "full image columns")
}
pub(super) fn matrix_bound(
    c: &IndexedCoefficientContext,
    m: &Matrix,
    limits: Limits,
) -> Result<()> {
    bound(m.nvalues(), limits.nonzeros, "native matrix nonzeros")?;
    let mut terms = 0;
    for value in m.values() {
        let value = c.admit_native_result_with_limits(value.clone(), limits.arithmetic)?;
        terms = add(
            terms,
            add(
                value.raw().numerator.nterms(),
                value.raw().denominator.nterms(),
            )?,
        )?;
        bound(
            terms,
            limits.coefficient_terms,
            "native matrix coefficient terms",
        )?;
    }
    Ok(())
}
pub(super) fn admit_values(
    c: &IndexedCoefficientContext,
    values: &[rustred::algebra::Coefficient],
    retained_terms: &mut usize,
    limits: Limits,
) -> Result<Vec<IndexedCoefficient>> {
    let mut admitted = Vec::with_capacity(values.len());
    for raw in values {
        let value = c.admit_native_result_with_limits(raw.clone(), limits.arithmetic)?;
        *retained_terms = add(
            *retained_terms,
            add(raw.numerator.nterms(), raw.denominator.nterms())?,
        )?;
        bound(
            *retained_terms,
            limits.coefficient_terms,
            "retained native U/L coefficient terms",
        )?;
        admitted.push(value);
    }
    Ok(admitted)
}
fn frame(
    c: &IndexedCoefficientContext,
    rows: &[Row],
    columns: &[IndexShift],
    limits: Limits,
) -> Result<Matrix> {
    bound(rows.len(), limits.rows, "frame rows")?;
    bound(columns.len(), limits.columns, "frame columns")?;
    let mut matrix = Matrix::new(
        0,
        u32::try_from(columns.len()).map_err(|_| Error::Budget("native columns"))?,
        Field::new(Z),
    );
    for row in rows {
        let mut values = Vec::new();
        let mut ids = Vec::new();
        bound(
            add(matrix.nvalues(), row.len())?,
            limits.nonzeros,
            "frame input nonzeros",
        )?;
        for (shift, value) in row {
            c.validate_with_limits(value, limits.arithmetic)?;
            require(!value.is_zero(), "explicit zero in sparse image")?;
            ids.push(
                columns
                    .binary_search(shift)
                    .map_err(|_| Error::Invalid("unregistered shift".into()))?
                    as u32,
            );
            values.push(value.raw().clone());
        }
        native(|| matrix.add_row(values, ids))?;
    }
    matrix_bound(c, &matrix, limits)?;
    Ok(matrix)
}

/// Native complete product, including columns not selected for elimination.
pub fn replay(
    c: &IndexedCoefficientContext,
    rows: &[Row],
    weights: &Weights,
    guards: &mut Vec<Guard>,
    limits: Limits,
) -> Result<Row> {
    let mut output = replay_many(c, rows, std::slice::from_ref(weights), guards, limits)?;
    require(output.len() == 1, "one-row replay output shape differs")?;
    Ok(output.remove(0))
}

/// Construct W and A once and let Symbolica evaluate the complete W*A span.
pub fn replay_many(
    c: &IndexedCoefficientContext,
    rows: &[Row],
    weights: &[Weights],
    guards: &mut Vec<Guard>,
    limits: Limits,
) -> Result<Vec<Row>> {
    bound(weights.len(), limits.rows, "weighted image rows")?;
    let columns = rows
        .iter()
        .flat_map(|r| r.keys().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    bound(columns.len(), limits.columns, "full replay columns")?;
    let mut operations = 0;
    for row in weights {
        for (&i, weight) in row {
            require(i < rows.len(), "weight row outside frozen span")?;
            denominator(c, guards, weight, "span weight before full product", limits)?;
            for value in rows[i].values() {
                denominator(c, guards, value, "source term before full product", limits)?;
            }
            operations = add(operations, rows[i].len())?;
        }
    }
    bound(
        operations,
        limits.operations,
        "full matrix product operations",
    )?;
    let b = frame(c, rows, &columns, limits)?;
    let mut w = Matrix::new(
        0,
        u32::try_from(rows.len()).map_err(|_| Error::Budget("native rows"))?,
        Field::new(Z),
    );
    for row in weights {
        bound(
            add(w.nvalues(), row.len())?,
            limits.nonzeros,
            "span weight nonzeros",
        )?;
        native(|| {
            w.add_row(
                row.values().map(|v| v.raw().clone()).collect(),
                row.keys().map(|&i| i as u32).collect(),
            )
        })?;
    }
    matrix_bound(c, &w, limits)?;
    let result = native(|| &w * &b)?;
    matrix_bound(c, &result, limits)?;
    require(
        result.nrows() as usize == weights.len() && result.ncols() as usize == columns.len(),
        "full native replay shape changed",
    )?;
    let mut output = Vec::new();
    for row in 0..weights.len() {
        let mut image = Row::new();
        for at in result.row_ptrs()[row]..result.row_ptrs()[row + 1] {
            let value =
                c.admit_native_result_with_limits(result.values()[at].clone(), limits.arithmetic)?;
            denominator(c, guards, &value, "full native product result", limits)?;
            if !value.is_zero() {
                image.insert(columns[result.col_idcs()[at] as usize].clone(), value);
            }
        }
        output.push(image);
    }
    Ok(output)
}

pub struct Proposal {
    pub weights: Weights,
    pub image: Row,
    pub guards: Vec<Guard>,
    pub prefix_rows: usize,
}
pub enum Projection {
    Target(Proposal),
    NoTarget { guards: Vec<Guard>, rows: usize },
}

/// All input guards remain live. Every physical prefix pivot is retained
/// conservatively; provenance pivots do not normalize a physical equation.
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

/// Opt-in injective coefficient-map transport; physical columns and guards do
/// not change. Every native result is restored before indexed admission.
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
        .flat_map(|r| r.keys().cloned())
        .collect::<BTreeSet<_>>();
    require(!forbidden.contains(target), "target cannot be forbidden")?;
    // A selected source subbank may have no term in an original forbidden
    // column. Keep that column explicitly: its exact projection is zero.
    for shift in forbidden {
        require(
            shift.values().len() == c.index_count(),
            "forbidden shift arity differs",
        )?;
    }
    bound(universe.len(), limits.columns, "image shift columns")?;
    let width = add(add(forbidden.len(), 1)?, rows.len())?;
    bound(width, limits.columns, "augmented projection columns")?;
    super::progress::projection_start(rows.len(), forbidden.len(), width);
    let physical = forbidden.len() + 1;
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
    // Authenticate and retain ALL input rows before an early target prefix.
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
    bound(
        add(inputs, rows.len())?,
        limits.nonzeros,
        "projection inputs",
    )?;
    let variables = native_variables::Variables::new(c, rows, compact, limits)?;
    let native_width = u32::try_from(width).map_err(|_| Error::Budget("native augmented width"))?;
    let mut reducer = native(|| SparseRowReducer::new(native_width, Field::new(Z), LuLMode::Full))?;
    let f = forbidden.iter().cloned().collect::<Vec<_>>();
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
        ids.push((physical + ordinal) as u32);
        values.push(variables.map(c.one().raw())?);
        let pivot = native(|| reducer.add_row(&values, &ids))?
            .ok_or_else(|| Error::Invalid("identity-augmented source lost independence".into()))?
            as usize;
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
        // add_row (not add_row_with_back_subs) appends to U and L without
        // changing earlier rows. Authenticate only the two new rows once;
        // total retained terms/nonzeros stay cumulatively resource-bound.
        let (urow, uids, uvalues) = reducer
            .u()
            .last_row()
            .ok_or_else(|| Error::Invalid("native U row missing".into()))?;
        let (lrow, lcols, lvalues) = reducer
            .l()
            .last_row()
            .ok_or_else(|| Error::Invalid("native L row missing".into()))?;
        require(
            urow as usize == ordinal
                && lrow as usize == ordinal
                && lcols.last().copied() == Some(urow),
            "native append-only row chronology changed",
        )?;
        let uvalues = variables.admit_values(c, uvalues, &mut retained_terms, limits)?;
        let lvalues = variables.admit_values(c, lvalues, &mut retained_terms, limits)?;
        if pivot < physical {
            let scale = lvalues
                .last()
                .ok_or_else(|| Error::Invalid("native pivot absent".into()))?;
            retain(
                c,
                &mut guards,
                c.numerator_condition_with_limits(&scale, limits.arithmetic)?,
                format!("physical pivot numerator row {ordinal}"),
                limits,
            )?;
            denominator(
                c,
                &mut guards,
                &scale,
                format!("physical pivot denominator row {ordinal}"),
                limits,
            )?;
        }
        if pivot != forbidden.len() {
            continue;
        }
        let (ids, values) = (uids, &uvalues);
        require(
            ids.first().copied() == Some(forbidden.len() as u32)
                && values.first().is_some_and(|v| v.raw().is_one()),
            "native target is not normalized",
        )?;
        require(
            ids.iter().skip(1).all(|&i| i as usize >= physical),
            "native target retains forbidden column",
        )?;
        let mut weights = Weights::new();
        for (&id, value) in ids.iter().skip(1).zip(values.iter().skip(1)) {
            let i = id as usize - physical;
            require(i <= ordinal, "native provenance exceeds visited rows")?;
            weights.insert(i, value.clone());
        }
        let image = replay(c, rows, &weights, &mut guards, limits)?;
        require(
            image.get(target).is_some_and(|v| v.raw().is_one())
                && forbidden.iter().all(|s| !image.contains_key(s)),
            "full symbolic target/F replay failed",
        )?;
        super::progress::event("projection_target", || {
            serde_json::json!({"rows_visited":ordinal+1,
            "f_size":forbidden.len(),"weight_terms":weights.len(),"full_image_terms":image.len()})
        });
        return Ok(Projection::Target(Proposal {
            weights,
            image,
            guards,
            prefix_rows: ordinal + 1,
        }));
    }
    super::progress::event(
        "projection_miss",
        || serde_json::json!({"rows_visited":rows.len(),"f_size":forbidden.len()}),
    );
    Ok(Projection::NoTarget {
        guards,
        rows: rows.len(),
    })
}
