//! RustRed's native IBP solver inside the host Symbolica Python extension.
//!
//! The bridge reads Feynkit's existing public family API. Expressions remain
//! native atoms in the host kernel; no second extension or string parser is used.

mod conversion;

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use conversion::FamilyConversion;
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::PyBool,
};
use rustred::{
    family::IntegralFamily,
    identity::ParametricIbpGenerator,
    sector::CutConstraint,
    solver::bridge::{self, DynamicPower, DynamicSolution, DynamicSolveOptions},
};
use symbolica::{
    api::python::PythonExpression,
    atom::{NamespacedSymbol, SymbolBuilder},
    prelude::*,
};

fn value_error(error: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(error.to_string())
}

type ExpressionTerm = (Vec<PythonExpression>, PythonExpression);
type ConcreteTerm = (Vec<i64>, PythonExpression);

static NEXT_INDEX_SCOPE: AtomicU64 = AtomicU64::new(0);

// Match RustRed's Python input boundary: booleans are flags, not powers.
struct PythonPower(i64);

impl<'a, 'py> FromPyObject<'a, 'py> for PythonPower {
    type Error = PyErr;

    fn extract(value: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if value.is_instance_of::<PyBool>() {
            return Err(PyTypeError::new_err("integral powers cannot be bool"));
        }
        value.extract::<i64>().map(Self)
    }
}

impl PythonPower {
    fn compact(self) -> PyResult<i16> {
        i16::try_from(self.0).map_err(|_| {
            PyValueError::new_err("propagator power is outside the compact range -64..=63")
        })
    }
}

/// Whether an integral has a nonpositive power on a cut denominator.
fn outside_cut(cut: &[bool], powers: &[i64]) -> bool {
    powers
        .iter()
        .zip(cut)
        .any(|(&power, &cut)| cut && power <= 0)
}

/// Return ``(powers, coefficient)`` terms, or their sum over ``integral(*powers)``.
fn terms_to_python(
    py: Python<'_>,
    terms: Vec<ConcreteTerm>,
    integral: Option<Symbol>,
) -> PyResult<Py<PyAny>> {
    let Some(integral) = integral else {
        return Ok(terms.into_pyobject(py)?.unbind().into_any());
    };
    let expression = terms
        .into_iter()
        .fold(Atom::Zero, |sum, (powers, coefficient)| {
            sum + coefficient.expr
                * FunctionBuilder::new(integral)
                    .add_args(powers.into_iter().map(Atom::num))
                    .finish()
        });
    Ok(PythonExpression::from(expression)
        .into_pyobject(py)?
        .unbind()
        .into_any())
}

/// An exact RustRed solver for a complete Feynkit ``IntegralFamily``.
///
/// Denominator order, signs, masses, dimension and external Gram products are
/// retained. These may involve loop-independent invariants such as
/// ``dot(q, q)``, calls declared ``Scalar`` that stay opaque parameters.
/// Call ``family.complete()`` explicitly for missing numerator slots;
/// powers of those auxiliary denominators should normally be nonpositive.
/// Solver searches support up to twelve denominators and fixed powers in
/// ``-64..=63``. Applying a symbolic recurrence accepts signed 64-bit powers.
///
/// ``cut`` flags reverse-unitarity denominators in family order. Every
/// integral with a nonpositive power on a cut denominator vanishes; solving
/// and reducing return zero for it.
#[pyclass(name = "IBPFamily", module = "symbolica.community.hepkit", frozen)]
pub struct PyIbpFamily {
    family: Arc<IntegralFamily>,
    parameters: BTreeMap<Atom, Atom>,
    indices: Vec<Atom>,
    cuts: CutConstraint,
}

impl PyIbpFamily {
    fn solution(&self, solution: DynamicSolution) -> PyResult<PyIbpSolution> {
        let mut replacements = self.parameters.clone();
        for (internal, visible) in solution.index_variables.iter().zip(&self.indices) {
            let PolyVariable::Symbol(symbol) = internal else {
                return Err(value_error("solver returned a non-symbolic index variable"));
            };
            replacements.insert(Atom::var(*symbol), visible.clone());
        }
        let rename = |atom: Atom| FamilyConversion::rename(&atom, &replacements);
        let cut: Arc<[bool]> = self.cuts.required_active().active_bits().into();
        let rules = solution
            .rules
            .into_iter()
            .map(|rule| PyIbpRule {
                target: rule.target.clone(),
                rhs: rule
                    .rhs
                    .iter()
                    .map(|term| {
                        (
                            term.powers.clone(),
                            rename(term.coefficient.to_expression()),
                        )
                    })
                    .collect(),
                conditions: rule
                    .nonzero_conditions
                    .iter()
                    .map(|p| rename(p.to_expression()))
                    .collect(),
                exceptions: rule
                    .exceptions
                    .iter()
                    .map(|branch| branch.iter().map(|p| rename(p.to_expression())).collect())
                    .collect(),
                sector: rule.sector,
                indices: self.indices.clone(),
                cut: cut.clone(),
            })
            .collect();
        let stats = BTreeMap::from([
            ("sectors".to_owned(), solution.stats.sectors),
            ("seeds".to_owned(), solution.stats.seeds),
            ("rows".to_owned(), solution.stats.rows),
            (
                "exact_trace_rows".to_owned(),
                solution.stats.exact_trace_rows,
            ),
        ]);
        Ok(PyIbpSolution {
            rules,
            residuals: solution.residuals,
            stats,
            arity: self.indices.len(),
            cut,
        })
    }
}

#[pymethods]
impl PyIbpFamily {
    #[new]
    #[pyo3(signature = (family, *, name="F", cut=None))]
    fn new(family: &Bound<'_, PyAny>, name: &str, cut: Option<Vec<bool>>) -> PyResult<Self> {
        record_usage();
        let converted = FamilyConversion::from_feynkit(family, name)?;
        let denominators = converted.family.denominator_count();
        let cuts = match cut {
            Some(cut) if cut.len() != denominators => {
                return Err(PyValueError::new_err(format!(
                    "cut has {} coordinates; expected {denominators}",
                    cut.len()
                )));
            }
            Some(cut) => CutConstraint::try_new(cut),
            None => CutConstraint::none(denominators),
        }
        .map_err(value_error)?;
        // A physical parameter may itself be called n1. Give indices a private
        // per-family scope and check even deliberately pre-created collisions.
        let indices = loop {
            let scope = NEXT_INDEX_SCOPE.fetch_add(1, Ordering::Relaxed);
            let indices = (0..converted.family.denominator_count())
                .map(|i| {
                    let name = format!("rustred_feynkit::indices_{scope}::n{}", i + 1);
                    let symbol = SymbolBuilder::new(
                        NamespacedSymbol::try_parse(&name).expect("valid index name"),
                    )
                    .build()
                    .map_err(value_error)?;
                    Ok(Atom::var(symbol))
                })
                .collect::<PyResult<Vec<_>>>()?;
            // Invariant calls may hide a symbol in their arguments.
            if !indices.iter().any(|index| {
                let index = index.get_symbol().expect("index is a symbol");
                converted
                    .original_parameters
                    .values()
                    .any(|parameter| parameter.contains_symbol(index))
            }) {
                break indices;
            }
        };
        Ok(Self {
            family: Arc::new(converted.family),
            parameters: converted.original_parameters,
            indices,
            cuts,
        })
    }

    #[getter]
    fn index_symbols(&self) -> Vec<PythonExpression> {
        self.indices.iter().cloned().map(Into::into).collect()
    }

    #[getter]
    fn denominator_count(&self) -> usize {
        self.family.denominator_count()
    }

    /// Reverse-unitarity flags, one per denominator in family order.
    #[getter]
    fn cut(&self) -> Vec<bool> {
        self.cuts.required_active().active_bits().to_vec()
    }

    /// Generate the ordinary ``L*(L+E)`` symbolic-index IBP zero equations.
    /// Each row is a list of ``(powers, coefficient)`` terms, summing to zero.
    fn ibp_identities(&self) -> PyResult<Vec<Vec<ExpressionTerm>>> {
        let generator = ParametricIbpGenerator::try_new(&self.family).map_err(value_error)?;
        let batch = generator.prepare_ordinary_ibp().map_err(value_error)?;
        let rows = (0..batch.len()).map(|i| batch.generate(i)).collect();
        let relations = batch.complete(rows).map_err(value_error)?.into_relations();
        let mut replacements = self.parameters.clone();
        for (i, visible) in self.indices.iter().enumerate() {
            replacements.insert(
                generator
                    .context()
                    .index(i)
                    .map_err(value_error)?
                    .to_expression(),
                visible.clone(),
            );
        }
        Ok(relations
            .into_iter()
            .map(|relation| {
                relation
                    .terms()
                    .iter()
                    .map(|(shift, coefficient)| {
                        let powers = shift
                            .values()
                            .iter()
                            .zip(&self.indices)
                            .map(|(shift, index)| (index + Atom::num(*shift)).into())
                            .collect();
                        let coefficient =
                            FamilyConversion::rename(&coefficient.to_expression(), &replacements)
                                .into();
                        (powers, coefficient)
                    })
                    .collect()
            })
            .collect())
    }

    /// Discover a reusable recurrence on a sector, retaining exceptional loci.
    /// ``None`` in fixed leaves an index symbolic; integers fix absolute powers.
    /// In a cut family, discovery drops terms that leave the cut through a fixed
    /// cut index. Terms that leave it through a symbolic cut index stay in
    /// ``rule.terms``; ``apply`` and ``reduce`` drop them as zero.
    #[pyo3(signature = (sector, *, fixed=None, max_depth=2, include_lorentz=false))]
    fn solve_parametric(
        &self,
        py: Python<'_>,
        sector: Vec<bool>,
        fixed: Option<Vec<Option<PythonPower>>>,
        max_depth: u32,
        include_lorentz: bool,
    ) -> PyResult<PyIbpSolution> {
        let fixed = match fixed {
            Some(fixed) => fixed
                .into_iter()
                .map(|power| power.map(PythonPower::compact).transpose())
                .collect::<PyResult<_>>()?,
            None => vec![None; self.family.denominator_count()],
        };
        let result = py
            .detach(|| {
                bridge::solve_parametric(
                    &self.family,
                    &self.cuts,
                    &sector,
                    &fixed,
                    DynamicSolveOptions {
                        max_depth,
                        include_lorentz,
                        ..Default::default()
                    },
                )
            })
            .map_err(value_error)?;
        self.solution(result)
    }

    /// Run exact finite-target Laporta elimination with a bounded seed search.
    /// ``residuals`` are the unresolved basis at this depth, not certified masters.
    #[pyo3(signature = (targets, *, max_depth=2, include_lorentz=false, max_targets=1024))]
    fn reduce_laporta(
        &self,
        py: Python<'_>,
        targets: Vec<Vec<PythonPower>>,
        max_depth: u32,
        include_lorentz: bool,
        max_targets: usize,
    ) -> PyResult<PyIbpSolution> {
        let targets = targets
            .into_iter()
            .map(|target| {
                target
                    .into_iter()
                    .map(PythonPower::compact)
                    .collect::<PyResult<Vec<_>>>()
            })
            .collect::<PyResult<Vec<_>>>()?;
        let result = py
            .detach(|| {
                bridge::solve_laporta(
                    &self.family,
                    &self.cuts,
                    &targets,
                    DynamicSolveOptions {
                        max_depth,
                        include_lorentz,
                        max_targets,
                    },
                )
            })
            .map_err(value_error)?;
        self.solution(result)
    }

    fn __repr__(&self) -> String {
        let cut = self.cut();
        let cut = if cut.contains(&true) {
            let flags: Vec<_> = cut
                .iter()
                .map(|&cut| if cut { "True" } else { "False" })
                .collect();
            format!(", cut=[{}]", flags.join(", "))
        } else {
            String::new()
        };
        format!(
            "IBPFamily(name={:?}, loops={}, external_momenta={}, denominators={}{cut})",
            self.family.name(),
            self.family.loop_count(),
            self.family.external_count(),
            self.family.denominator_count()
        )
    }
}

/// One exact solved identity with explicit generic-domain conditions.
/// ``exceptions`` is a disjunction of branches; every polynomial in a branch
/// vanishing makes that branch exceptional and forbids the rule.
#[pyclass(
    name = "IBPRule",
    module = "symbolica.community.hepkit",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyIbpRule {
    target: Vec<DynamicPower>,
    rhs: Vec<(Vec<DynamicPower>, Atom)>,
    conditions: Vec<Atom>,
    exceptions: Vec<Vec<Atom>>,
    sector: Vec<bool>,
    indices: Vec<Atom>,
    cut: Arc<[bool]>,
}

impl PyIbpRule {
    fn expressions(&self, powers: &[DynamicPower]) -> Vec<PythonExpression> {
        powers
            .iter()
            .zip(&self.indices)
            .map(|(power, index)| {
                if power.symbolic {
                    (index + Atom::num(i64::from(power.value))).into()
                } else {
                    Atom::num(i64::from(power.value)).into()
                }
            })
            .collect()
    }

    fn instantiate(
        &self,
        py: Python<'_>,
        powers: &[i64],
        integral: Option<Symbol>,
    ) -> PyResult<Option<Py<PyAny>>> {
        if powers.len() != self.target.len() {
            return Err(PyValueError::new_err(
                "one integer power is required per denominator",
            ));
        }
        if powers
            .iter()
            .zip(&self.sector)
            .any(|(power, active)| (*power > 0) != *active)
        {
            return Ok(None);
        }
        let mut replacements = BTreeMap::new();
        let mut offsets = Vec::new();
        for ((target, index), power) in self.target.iter().zip(&self.indices).zip(powers) {
            if !target.symbolic && *power != i64::from(target.value) {
                return Ok(None);
            }
            let offset = power
                .checked_sub(i64::from(target.value))
                .ok_or_else(|| PyValueError::new_err("integral power overflow"))?;
            offsets.push(offset);
            if target.symbolic {
                replacements.insert(index.clone(), Atom::num(offset));
            }
        }
        let specialize = |atom: &Atom| FamilyConversion::rename(atom, &replacements).expand();
        if self
            .conditions
            .iter()
            .any(|condition| specialize(condition).is_zero())
            || self.exceptions.iter().any(|branch| {
                branch
                    .iter()
                    .all(|condition| specialize(condition).is_zero())
            })
        {
            return Ok(None);
        }
        let mut terms = Vec::<ConcreteTerm>::new();
        for (term, coefficient) in &self.rhs {
            let result = term
                .iter()
                .zip(&offsets)
                .map(|(power, offset)| {
                    if power.symbolic {
                        offset
                            .checked_add(i64::from(power.value))
                            .ok_or_else(|| PyValueError::new_err("integral power overflow"))
                    } else {
                        Ok(i64::from(power.value))
                    }
                })
                .collect::<PyResult<Vec<_>>>()?;
            // Cut-zero integrals vanish; skip them before specializing.
            if !outside_cut(&self.cut, &result) {
                terms.push((result, specialize(coefficient).into()));
            }
        }
        terms_to_python(py, terms, integral).map(Some)
    }
}

#[pymethods]
impl PyIbpRule {
    #[getter]
    fn target(&self) -> Vec<PythonExpression> {
        self.expressions(&self.target)
    }

    /// Symbolic right-hand side. In a cut family it can include integrals that
    /// leave the cut at particular indices; ``apply`` and ``reduce`` drop those.
    #[getter]
    fn terms(&self) -> Vec<ExpressionTerm> {
        self.rhs
            .iter()
            .map(|(powers, coefficient)| (self.expressions(powers), coefficient.clone().into()))
            .collect()
    }

    #[getter]
    fn nonzero_conditions(&self) -> Vec<PythonExpression> {
        self.conditions.iter().cloned().map(Into::into).collect()
    }

    #[getter]
    fn exceptions(&self) -> Vec<Vec<PythonExpression>> {
        self.exceptions
            .iter()
            .map(|branch| branch.iter().cloned().map(Into::into).collect())
            .collect()
    }

    #[getter]
    fn sector(&self) -> Vec<bool> {
        self.sector.clone()
    }

    /// Substitute concrete indices, rejecting an incompatible sector or exception.
    /// Conditions remaining symbolic in kinematic parameters must still be nonzero.
    /// Return ``(powers, coefficient)`` terms by default. Pass a bare Symbolica
    /// symbol as ``integral`` to return the sum ``coefficient * integral(*powers)``.
    #[pyo3(signature = (powers, *, integral=None))]
    fn apply(
        &self,
        py: Python<'_>,
        powers: Vec<PythonPower>,
        integral: Option<&PythonExpression>,
    ) -> PyResult<Py<PyAny>> {
        let integral = integral
            .map(|head| {
                let AtomView::Var(symbol) = head.expr.as_view() else {
                    return Err(PyValueError::new_err(
                        "integral must be a bare Symbolica symbol",
                    ));
                };
                Ok(symbol.get_symbol())
            })
            .transpose()?;
        let powers = powers.into_iter().map(|power| power.0).collect::<Vec<_>>();
        self.instantiate(py, &powers, integral)?.ok_or_else(|| {
            PyValueError::new_err(
                "rule does not apply to these powers or lies on an exceptional locus",
            )
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "IBPRule(terms={}, conditions={}, exception_branches={})",
            self.rhs.len(),
            self.conditions.len(),
            self.exceptions.len()
        )
    }
}

/// Rules and unresolved integrals from a finite IBP search.
#[pyclass(name = "IBPSolution", module = "symbolica.community.hepkit", frozen)]
pub struct PyIbpSolution {
    rules: Vec<PyIbpRule>,
    residuals: Vec<Vec<i16>>,
    stats: BTreeMap<String, usize>,
    arity: usize,
    cut: Arc<[bool]>,
}

#[pymethods]
impl PyIbpSolution {
    #[getter]
    fn rules(&self) -> Vec<PyIbpRule> {
        self.rules.clone()
    }

    #[getter]
    fn residuals(&self) -> Vec<Vec<i16>> {
        self.residuals.clone()
    }

    #[getter]
    fn stats(&self) -> BTreeMap<String, usize> {
        self.stats.clone()
    }

    /// Apply the first valid solved rule; leave unresolved integrals unchanged.
    /// Laporta rules have already been back-substituted through solved targets.
    /// Parametric rules perform one recurrence step per call.
    /// Return ``(powers, coefficient)`` terms by default. Pass a bare Symbolica
    /// symbol as ``integral`` to return the sum ``coefficient * integral(*powers)``.
    /// Unresolved integrals remain explicit, and symbolic rule conditions still apply.
    /// In a cut family, integrals outside the cut reduce to zero, requested or not.
    #[pyo3(signature = (powers, *, integral=None))]
    fn reduce(
        &self,
        py: Python<'_>,
        powers: Vec<PythonPower>,
        integral: Option<&PythonExpression>,
    ) -> PyResult<Py<PyAny>> {
        let integral = integral
            .map(|head| {
                let AtomView::Var(symbol) = head.expr.as_view() else {
                    return Err(PyValueError::new_err(
                        "integral must be a bare Symbolica symbol",
                    ));
                };
                Ok(symbol.get_symbol())
            })
            .transpose()?;
        let powers = powers.into_iter().map(|power| power.0).collect::<Vec<_>>();
        if powers.len() != self.arity {
            return Err(PyValueError::new_err(
                "one integer power is required per denominator",
            ));
        }
        if outside_cut(&self.cut, &powers) {
            return terms_to_python(py, Vec::new(), integral);
        }
        for rule in &self.rules {
            if let Some(terms) = rule.instantiate(py, &powers, integral)? {
                return Ok(terms);
            }
        }
        terms_to_python(py, vec![(powers, Atom::num(1).into())], integral)
    }

    fn __repr__(&self) -> String {
        format!(
            "IBPSolution(rules={}, residuals={})",
            self.rules.len(),
            self.residuals.len()
        )
    }
}

/// Register the bridge in the same module and Symbolica kernel as Feynkit.
pub fn register_hep_module(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyIbpFamily>()?;
    module.add_class::<PyIbpRule>()?;
    module.add_class::<PyIbpSolution>()?;
    Ok(())
}

static CITATIONS_USED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[inline]
pub(crate) fn record_usage() {
    use std::sync::atomic::Ordering;
    if !CITATIONS_USED.load(Ordering::Relaxed) {
        CITATIONS_USED.store(true, Ordering::Relaxed);
    }
}

/// Whether this package has performed an operation in this process.
pub fn was_used() -> bool {
    CITATIONS_USED.load(std::sync::atomic::Ordering::Relaxed)
}
