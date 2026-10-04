//! Finite polynomial-weight endpoint constraints, not a module/syzygy solver.
//! Symbolica owns restriction, coefficient grouping, and field elimination.
//! The ordinary Span and unchanged original-source/chart checker own authority.
use super::project::{self, *};
use rustred::{
    algebra::{Coefficient, IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext},
    identity::IndexShift,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use symbolica::{
    domains::{SelfRing, rational_polynomial::FromNumeratorAndDenominator},
    prelude::Z,
    tensors::sparse::{LuLMode, SparseRowReducer},
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    ActivationFaces,
    WholeColumns,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub mode: Mode,
    pub polynomial_axes: Vec<usize>,
    pub protected_axes: Vec<usize>,
    pub weight_monomials: Vec<Vec<u16>>,
    pub max_total_degree: usize,
    pub max_unknowns: usize,
    pub max_constraints: usize,
    pub max_nonzeros: usize,
    pub max_activation_faces: usize,
}

pub fn config(r: &Value) -> super::Result<Option<Config>> {
    let Some(value) = r.get("boundary_polynomial") else {
        return Ok(None);
    };
    let config: Config = super::checked(serde_json::from_value(value.clone()))?;
    let mask = r["owner_mask"]
        .as_str()
        .ok_or("boundary owner mask required")?;
    let fixed = super::fixed(r)?;
    super::checked(config.validate(mask.len()))?;
    for &axis in &config.polynomial_axes {
        super::require(
            !fixed.iter().any(|&(a, _)| a == axis),
            "polynomial axis is fixed",
        )?;
    }
    for axis in 0..mask.len() {
        super::require(
            config.polynomial_axes.contains(&axis) || fixed.iter().any(|&(a, _)| a == axis),
            "every unlisted index must be explicitly fixed",
        )?;
    }
    for &axis in &config.protected_axes {
        super::require(
            mask.as_bytes()[axis] == b'0'
                && r["chart"]["lower"][axis].as_u64() == Some(0)
                && r["chart"]["upper"][axis].is_null(),
            "protected axes require the full inactive nonpositive orthant",
        )?;
    }
    super::require(
        super::number(r, "max_refinements")? == 0
            && r.get("modular_nomination").is_none()
            && !super::reconstructed::enabled(r)
            && !super::direct_l::enabled(r)?,
        "boundary polynomial mode is one exact bounded attempt without other projection backends",
    )?;
    Ok(Some(config))
}

impl Config {
    fn validate(&self, arity: usize) -> Result<()> {
        require(
            !self.polynomial_axes.is_empty()
                && !self.protected_axes.is_empty()
                && self.polynomial_axes.windows(2).all(|p| p[0] < p[1])
                && self.protected_axes.windows(2).all(|p| p[0] < p[1])
                && self.polynomial_axes.iter().all(|&a| a < arity)
                && self
                    .protected_axes
                    .iter()
                    .all(|a| self.polynomial_axes.contains(a)),
            "boundary axes must be sorted, unique, in range and protected within polynomial axes",
        )?;
        require(
            self.max_unknowns > 0
                && self.max_constraints > 0
                && self.max_nonzeros > 0
                && self.max_activation_faces > 0
                && !self.weight_monomials.is_empty(),
            "boundary limits and monomial list must be nonempty",
        )?;
        bound(
            self.weight_monomials.len(),
            self.max_unknowns,
            "weight monomials",
        )?;
        let mut seen = BTreeSet::new();
        for powers in &self.weight_monomials {
            require(
                powers.len() == self.polynomial_axes.len() && seen.insert(powers),
                "monomial arity/duplicate",
            )?;
            let degree = powers
                .iter()
                .try_fold(0usize, |sum, &p| add(sum, usize::from(p)))?;
            bound(degree, self.max_total_degree, "weight degree")?;
        }
        Ok(())
    }
}

// These are linear-constraint labels, never synthetic integral-shift columns.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Constraint {
    Global(IndexShift, Vec<u16>),
    Face(IndexShift, usize, i64, Vec<u16>),
}
struct BasisRow {
    source: usize,
    monomial: IndexedCoefficient,
    constraints: BTreeMap<Constraint, IndexedCoefficient>,
    target: BTreeMap<Vec<u16>, IndexedCoefficient>,
}

#[derive(Default)]
struct Admission {
    entries: usize,
    terms: usize,
}

fn work(charged: &mut usize, count: usize, limits: Limits) -> Result<()> {
    *charged = add(*charged, count)?;
    bound(*charged, limits.operations, "boundary aggregate operations")
}

fn register_label<T: Ord>(
    key: T,
    labels: &mut BTreeSet<T>,
    count: &mut usize,
    config: &Config,
) -> Result<()> {
    if !labels.contains(&key) {
        let next = add(*count, 1)?;
        bound(next, config.max_constraints, "constraint and target labels")?;
        labels.insert(key);
        *count = next;
    }
    Ok(())
}

fn index_positions(c: &IndexedCoefficientContext) -> std::ops::Range<usize> {
    let first = c.base().parameter_names().len();
    first..first + c.index_count()
}

fn scalar(c: &IndexedCoefficientContext, value: &IndexedCoefficient, limits: Limits) -> Result<()> {
    c.validate_with_limits(value, limits.arithmetic)?;
    require(
        index_positions(c)
            .all(|p| !value.raw().numerator.contains(p) && !value.raw().denominator.contains(p)),
        "linear system coefficient escaped the base-parameter field",
    )
}

fn polynomial_input(
    c: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
    config: &Config,
    limits: Limits,
) -> Result<()> {
    c.validate_with_limits(value, limits.arithmetic)?;
    let base = c.base().parameter_names().len();
    require(
        index_positions(c).all(|p| !value.raw().denominator.contains(p)),
        "boundary polynomial mode refuses index-dependent source denominators",
    )?;
    require(
        (0..c.index_count()).all(|axis| {
            config.polynomial_axes.contains(&axis) || !value.raw().numerator.contains(base + axis)
        }),
        "source retains an undeclared polynomial index",
    )
}

/// Exact native grouping with the authenticated full variable map retained.
fn split(
    c: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
    config: &Config,
    limits: Limits,
    admission: &mut Admission,
    operations: &mut usize,
) -> Result<BTreeMap<Vec<u16>, IndexedCoefficient>> {
    polynomial_input(c, value, config, limits)?;
    bound(
        value.raw().numerator.nterms(),
        limits.coefficient_terms,
        "coefficient grouping input",
    )?;
    // A native grouping has at most one output per input term. Admit that
    // upper bound before requesting its temporary HashMap allocation.
    let upper = value.raw().numerator.nterms();
    bound(upper, config.max_constraints, "native grouping upper bound")?;
    bound(
        add(admission.entries, upper)?,
        config.max_nonzeros.min(limits.nonzeros),
        "grouping entry admission",
    )?;
    work(operations, upper, limits)?;
    let positions = index_positions(c).collect::<Vec<_>>();
    let groups = native(|| {
        value
            .raw()
            .numerator
            .to_multivariate_polynomial_list(&positions, true)
    })?;
    bound(groups.len(), config.max_constraints, "coefficient groups")?;
    let mut result = BTreeMap::new();
    for (powers, numerator) in groups {
        let raw = native(|| {
            Coefficient::from_num_den(numerator, value.raw().denominator.clone(), &Z, true)
        })?;
        let coefficient = c.admit_native_result_with_limits(raw, limits.arithmetic)?;
        scalar(c, &coefficient, limits)?;
        if !coefficient.is_zero() {
            let entries = add(admission.entries, 1)?;
            let terms = add(
                admission.terms,
                add(
                    coefficient.raw().numerator.nterms(),
                    coefficient.raw().denominator.nterms(),
                )?,
            )?;
            bound(
                entries,
                config.max_nonzeros.min(limits.nonzeros),
                "assembled constraint nonzeros",
            )?;
            bound(
                terms,
                limits.coefficient_terms,
                "assembled coefficient terms",
            )?;
            admission.entries = entries;
            admission.terms = terms;
            require(
                result.insert(powers.to_vec(), coefficient).is_none(),
                "duplicate native coefficient group",
            )?;
        }
    }
    Ok(result)
}

fn faces(
    shift: &IndexShift,
    config: &Config,
    charged: &mut usize,
    limits: Limits,
) -> Result<Vec<(usize, i64)>> {
    let mut result = Vec::new();
    for &axis in &config.protected_axes {
        let positive = shift.values()[axis].max(0);
        let count = usize::try_from(positive).map_err(|_| Error::Budget("activation faces"))?;
        *charged = add(*charged, count)?;
        bound(
            *charged,
            config.max_activation_faces.min(limits.operations),
            "activation faces",
        )?;
        result.extend((0..positive).map(|k| (axis, -k)));
    }
    Ok(result)
}

fn restriction(
    c: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
    face: (usize, i64),
    arithmetic: IndexedAlgebraLimits,
    guards: &mut Vec<Guard>,
    limits: Limits,
    operations: &mut usize,
) -> Result<IndexedCoefficient> {
    work(operations, 1, limits)?;
    let (restricted, witness) = c.specialize_fixed_indices(value, &[face], arithmetic)?;
    retain(
        c,
        guards,
        witness,
        "boundary restriction pre-cancellation denominator",
        limits,
    )?;
    Ok(restricted)
}

/// Check after normalization; a raw boundary factor or a discarded face is not enough.
fn verify_faces(
    c: &IndexedCoefficientContext,
    image: &Row,
    config: &Config,
    arithmetic: IndexedAlgebraLimits,
    guards: &mut Vec<Guard>,
    limits: Limits,
    operations: &mut usize,
) -> Result<()> {
    let mut count = 0;
    for (shift, value) in image {
        for face in faces(shift, config, &mut count, limits)? {
            let restricted = restriction(c, value, face, arithmetic, guards, limits, operations)?;
            require(
                restricted.is_zero(),
                "normalized endpoint does not vanish on an activation face",
            )?;
            for guard in guards.iter() {
                work(operations, 1, limits)?;
                let on_face =
                    c.specialize_fixed_polynomial(&guard.polynomial, &[face], arithmetic)?;
                require(
                    !on_face.is_zero(),
                    "a retained pole/pivot/source condition excludes an entire activation face",
                )?;
            }
        }
    }
    Ok(())
}

pub fn project(
    c: &IndexedCoefficientContext,
    rows: &[Row],
    target: &IndexShift,
    forbidden: &BTreeSet<IndexShift>,
    input_guards: &[Guard],
    arithmetic: IndexedAlgebraLimits,
    limits: Limits,
    config: &Config,
) -> Result<Projection> {
    config.validate(c.index_count())?;
    require(
        !rows.is_empty() && target.values().len() == c.index_count() && !forbidden.contains(target),
        "boundary source/target shape",
    )?;
    require(
        target.values().iter().all(|&value| value == 0),
        "boundary activation faces require zero target shift",
    )?;
    require(
        forbidden
            .iter()
            .all(|shift| shift.values().len() == c.index_count()),
        "forbidden shift arity",
    )?;
    bound(rows.len(), limits.rows, "boundary original rows")?;
    let unknowns = rows
        .len()
        .checked_mul(config.weight_monomials.len())
        .ok_or(Error::Budget("weight unknowns"))?;
    bound(unknowns, config.max_unknowns, "weight unknowns")?;
    let mut guards = Vec::new();
    for guard in input_guards {
        retain(
            c,
            &mut guards,
            guard.polynomial.clone(),
            guard.origin.clone(),
            limits,
        )?;
    }
    let mut universe = BTreeSet::new();
    let mut operations = 0usize;
    for row in rows {
        for (shift, value) in row {
            require(
                shift.values().len() == c.index_count(),
                "source shift arity",
            )?;
            polynomial_input(c, value, config, limits)?;
            denominator(c, &mut guards, value, "boundary original input", limits)?;
            universe.insert(shift.clone());
        }
    }
    bound(
        universe.len(),
        limits.columns,
        "boundary full image columns",
    )?;
    let mut face_count = 0;
    let face_map = universe
        .iter()
        .map(|s| Ok((s.clone(), faces(s, config, &mut face_count, limits)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut monomials = Vec::new();
    for powers in &config.weight_monomials {
        let mut value = c.one();
        for (&axis, &power) in config.polynomial_axes.iter().zip(powers) {
            let index = c.index(axis)?;
            for _ in 0..power {
                work(&mut operations, 1, limits)?;
                value = c.mul_with_limits(&value, &index, limits.arithmetic)?;
            }
        }
        monomials.push(value);
    }
    let mut basis = Vec::new();
    let mut labels = BTreeSet::new();
    let mut target_labels = BTreeSet::new();
    let mut admission = Admission {
        entries: unknowns,
        terms: 0,
    };
    bound(
        admission.entries,
        config.max_nonzeros.min(limits.nonzeros),
        "identity entry admission",
    )?;
    let mut label_count = 0usize;
    for (source, row) in rows.iter().enumerate() {
        for monomial in &monomials {
            let mut b = BasisRow {
                source,
                monomial: monomial.clone(),
                constraints: BTreeMap::new(),
                target: BTreeMap::new(),
            };
            for (shift, value) in row {
                work(&mut operations, 1, limits)?;
                let product = c.mul_with_limits(monomial, value, limits.arithmetic)?;
                let fs = &face_map[shift];
                if forbidden.contains(shift)
                    || (config.mode == Mode::WholeColumns && !fs.is_empty())
                {
                    for (powers, coefficient) in
                        split(c, &product, config, limits, &mut admission, &mut operations)?
                    {
                        let key = Constraint::Global(shift.clone(), powers);
                        register_label(key.clone(), &mut labels, &mut label_count, config)?;
                        require(
                            b.constraints.insert(key, coefficient).is_none(),
                            "duplicate global constraint",
                        )?;
                    }
                } else if config.mode == Mode::ActivationFaces {
                    for &face in fs {
                        let restricted = restriction(
                            c,
                            &product,
                            face,
                            arithmetic,
                            &mut guards,
                            limits,
                            &mut operations,
                        )?;
                        for (powers, coefficient) in split(
                            c,
                            &restricted,
                            config,
                            limits,
                            &mut admission,
                            &mut operations,
                        )? {
                            let key = Constraint::Face(shift.clone(), face.0, face.1, powers);
                            register_label(key.clone(), &mut labels, &mut label_count, config)?;
                            require(
                                b.constraints.insert(key, coefficient).is_none(),
                                "duplicate face constraint",
                            )?;
                        }
                    }
                }
                if shift == target {
                    for (key, coefficient) in
                        split(c, &product, config, limits, &mut admission, &mut operations)?
                    {
                        register_label(key.clone(), &mut target_labels, &mut label_count, config)?;
                        require(
                            b.target.insert(key, coefficient).is_none(),
                            "duplicate target coefficient",
                        )?;
                    }
                }
            }
            basis.push(b);
        }
    }
    if target_labels.is_empty() {
        return Ok(Projection::NoTarget {
            guards,
            rows: basis.len(),
        });
    }
    let labels = labels.into_iter().collect::<Vec<_>>();
    let target_labels = target_labels.into_iter().collect::<Vec<_>>();
    let physical = add(labels.len(), target_labels.len())?;
    let width = add(physical, unknowns)?;
    bound(width, limits.columns, "boundary augmented columns")?;
    bound(
        admission.entries,
        config.max_nonzeros.min(limits.nonzeros),
        "boundary augmented input nonzeros",
    )?;
    let native_width = u32::try_from(width).map_err(|_| Error::Budget("boundary native width"))?;
    let mut reducer = native(|| SparseRowReducer::new(native_width, Field::new(Z), LuLMode::Full))?;
    let mut retained_terms = admission.terms;
    super::progress::event("boundary_constraints_ready", || {
        serde_json::json!({
        "unknowns":unknowns,"constraints":labels.len(),"target_coefficient_columns":target_labels.len(),
        "activation_faces":face_count,"coefficient_field":"base parameters only", "mode":format!("{:?}",config.mode)})
    });
    for (ordinal, b) in basis.iter().enumerate() {
        let mut entries = BTreeMap::new();
        for (key, coefficient) in &b.constraints {
            entries.insert(
                labels.binary_search(key).expect("collected constraint"),
                coefficient.clone(),
            );
        }
        for (key, coefficient) in &b.target {
            entries.insert(
                labels.len()
                    + target_labels
                        .binary_search(key)
                        .expect("collected target coefficient"),
                coefficient.clone(),
            );
        }
        entries.insert(physical + ordinal, c.one());
        let ids = entries.keys().map(|&i| i as u32).collect::<Vec<_>>();
        let values = entries
            .values()
            .map(|c| c.raw().clone())
            .collect::<Vec<_>>();
        let pivot = native(|| reducer.add_row(&values, &ids))?.ok_or_else(|| {
            Error::Invalid("identity-augmented boundary row lost independence".into())
        })? as usize;
        bound(
            add(reducer.u().nvalues(), reducer.l().nvalues())?,
            config.max_nonzeros.min(limits.nonzeros),
            "boundary U/L nonzeros",
        )?;
        let (urow, ids, raw) = reducer
            .u()
            .last_row()
            .ok_or_else(|| Error::Invalid("missing boundary U row".into()))?;
        let (lrow, _, lraw) = reducer
            .l()
            .last_row()
            .ok_or_else(|| Error::Invalid("missing boundary L row".into()))?;
        require(
            urow as usize == ordinal && lrow as usize == ordinal,
            "boundary row chronology",
        )?;
        let values = project::admit_values(c, raw, &mut retained_terms, limits)?;
        let lvalues = project::admit_values(c, lraw, &mut retained_terms, limits)?;
        for value in values.iter().chain(&lvalues) {
            scalar(c, value, limits)?;
            denominator(
                c,
                &mut guards,
                value,
                "boundary native field result",
                limits,
            )?;
        }
        if pivot < physical {
            let scale = lvalues
                .last()
                .ok_or_else(|| Error::Invalid("missing boundary pivot".into()))?;
            retain(
                c,
                &mut guards,
                c.numerator_condition_with_limits(scale, limits.arithmetic)?,
                "boundary field pivot numerator",
                limits,
            )?;
        }
        if !(labels.len()..physical).contains(&pivot) {
            continue;
        }
        require(
            ids.iter().all(|&i| i as usize >= labels.len()),
            "boundary target retains a constraint",
        )?;
        let mut weights = Weights::new();
        for (&id, scalar_weight) in ids
            .iter()
            .zip(&values)
            .filter(|(id, _)| **id as usize >= physical)
        {
            let index = id as usize - physical;
            require(index <= ordinal, "boundary provenance beyond visited basis")?;
            let source = basis[index].source;
            work(&mut operations, 1, limits)?;
            let weight =
                c.mul_with_limits(scalar_weight, &basis[index].monomial, limits.arithmetic)?;
            let weight = if let Some(previous) = weights.remove(&source) {
                work(&mut operations, 1, limits)?;
                c.add_with_limits(&previous, &weight, limits.arithmetic)?
            } else {
                weight
            };
            denominator(
                c,
                &mut guards,
                &weight,
                "boundary reconstructed polynomial weight",
                limits,
            )?;
            if !weight.is_zero() {
                weights.insert(source, weight);
            }
        }
        // This is the first target-bearing dependency in declared basis order.
        // A later normalization/guard/proof refusal rejects this candidate only;
        // it does not establish absence of another useful kernel combination.
        let raw_image = project::replay(c, rows, &weights, &mut guards, limits)?;
        let target_value = raw_image
            .get(target)
            .ok_or_else(|| Error::Invalid("boundary target vanished in full replay".into()))?;
        require(
            !target_value.is_zero() && forbidden.iter().all(|s| !raw_image.contains_key(s)),
            "boundary full target/F replay",
        )?;
        retain(
            c,
            &mut guards,
            c.numerator_condition_with_limits(target_value, limits.arithmetic)?,
            "boundary target normalization pivot",
            limits,
        )?;
        denominator(
            c,
            &mut guards,
            target_value,
            "boundary target before normalization",
            limits,
        )?;
        for weight in weights.values_mut() {
            work(&mut operations, 1, limits)?;
            *weight = c.div_with_limits(weight, target_value, limits.arithmetic)?;
            denominator(
                c,
                &mut guards,
                weight,
                "boundary normalized source weight",
                limits,
            )?;
        }
        let image = project::replay(c, rows, &weights, &mut guards, limits)?;
        require(
            image.get(target).is_some_and(|v| v.raw().is_one())
                && forbidden.iter().all(|s| !image.contains_key(s)),
            "boundary normalized full replay",
        )?;
        if config.mode == Mode::WholeColumns {
            require(
                image
                    .keys()
                    .all(|s| face_map.get(s).is_none_or(Vec::is_empty)),
                "global-zero control retained a protected endpoint",
            )?;
        }
        verify_faces(
            c,
            &image,
            config,
            arithmetic,
            &mut guards,
            limits,
            &mut operations,
        )?;
        return Ok(Projection::Target(Proposal {
            weights,
            image,
            guards,
            prefix_rows: ordinal + 1,
        }));
    }
    Ok(Projection::NoTarget {
        guards,
        rows: basis.len(),
    })
}

#[cfg(test)]
#[path = "boundary/tests.rs"]
mod tests;
