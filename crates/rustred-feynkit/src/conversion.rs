//! Preserve native Symbolica atoms across Feynkit's public Python boundary.

use std::collections::{BTreeMap, BTreeSet};

use pyo3::{exceptions::PyValueError, prelude::*};
use rustred::{
    algebra::{Coefficient, CoefficientContext},
    family::{AffineDenominator, IntegralFamily},
};
use symbolica::{api::python::PythonExpression, prelude::*};

pub(crate) struct FamilyConversion {
    pub family: IntegralFamily,
    pub original_parameters: BTreeMap<Atom, Atom>,
}

/// Momenta that no IBP parameter may mention.
struct Momenta {
    loops: Vec<Momentum>,
    external: Vec<Momentum>,
    edge: Option<Momentum>,
}

/// A momentum as FeynKit writes it: a symbol `k`, or a labeled call `K(0)`,
/// which Spenso spells `K(0, mink(d))` inside scalar products.
struct Momentum {
    head: Symbol,
    labels: Vec<Atom>,
}

impl Momentum {
    fn new(momentum: &PythonExpression) -> PyResult<Self> {
        match momentum.expr.as_view() {
            AtomView::Var(variable) => Ok(Self {
                head: variable.get_symbol(),
                labels: Vec::new(),
            }),
            AtomView::Fun(function) => Ok(Self {
                head: function.get_symbol(),
                labels: function.iter().map(|label| label.to_owned()).collect(),
            }),
            _ => Err(PyValueError::new_err(
                "family momenta must be symbols or labeled calls",
            )),
        }
    }

    /// Whether `value` mentions this momentum, bare or as a call with its labels.
    fn occurs_in(&self, value: AtomView<'_>) -> bool {
        let mut found = false;
        value.visitor(&mut |view| {
            found |= match view {
                AtomView::Var(variable) => {
                    self.labels.is_empty() && variable.get_symbol() == self.head
                }
                AtomView::Fun(function) => {
                    function.get_symbol() == self.head
                        && function.get_nargs() >= self.labels.len()
                        && function
                            .iter()
                            .zip(&self.labels)
                            .all(|(argument, label)| argument == label.as_view())
                }
                _ => false,
            };
            !found
        });
        found
    }
}

impl FamilyConversion {
    pub fn from_feynkit(source: &Bound<'_, PyAny>, name: &str) -> PyResult<Self> {
        if !source.getattr("is_independent")?.extract::<bool>()? {
            return Err(PyValueError::new_err(
                "dependent propagators must be partial-fractioned before IBP reduction",
            ));
        }
        if !source.getattr("is_complete")?.extract::<bool>()? {
            return Err(PyValueError::new_err(
                "incomplete scalar-product basis; call family.complete() before IBP reduction",
            ));
        }
        let loops = source
            .getattr("loop_momenta")?
            .extract::<Vec<PythonExpression>>()?;
        let external = source
            .getattr("external_momenta")?
            .extract::<Vec<PythonExpression>>()?;
        let kin = source.getattr("kinematics")?;
        let dimension = kin
            .getattr("dimension")?
            .extract::<PythonExpression>()?
            .expr;
        let product = |left: &PythonExpression, right: &PythonExpression| {
            Ok::<Atom, PyErr>(
                kin.call_method1("scalar_product", (left.clone(), right.clone()))?
                    .extract::<PythonExpression>()?
                    .expr,
            )
        };
        // RustRed orders all loop-loop entries before loop-external entries.
        // Feynkit's scalar_products getter interleaves these two groups.
        let mut products = Vec::new();
        for (i, left) in loops.iter().enumerate() {
            for right in &loops[i..] {
                products.push(product(left, right)?);
            }
        }
        for left in &loops {
            for right in &external {
                products.push(product(left, right)?);
            }
        }
        let gram = external
            .iter()
            .map(|left| {
                external
                    .iter()
                    .map(|right| product(left, right))
                    .collect::<PyResult<Vec<_>>>()
            })
            .collect::<PyResult<Vec<_>>>()?;
        let denominators = source
            .getattr("denominators")?
            .extract::<Vec<PythonExpression>>()?;
        let mut affine = Vec::new();
        for denominator in denominators {
            let mut row = vec![Atom::new(); products.len() + 1];
            for (monomial, coefficient) in denominator.expr.coefficient_list::<i32>(&products) {
                let index = if monomial.is_one() {
                    0
                } else {
                    products
                        .iter()
                        .position(|p| p == &monomial)
                        .map(|i| i + 1)
                        .ok_or_else(|| {
                            PyValueError::new_err(
                                "denominator is not affine in loop scalar products",
                            )
                        })?
                };
                row[index] = &row[index] + coefficient;
            }
            affine.push(row);
        }
        let momenta = |momenta: &[PythonExpression]| {
            momenta
                .iter()
                .map(Momentum::new)
                .collect::<PyResult<Vec<_>>>()
        };
        // FeynKit's routed edge momenta Q(e) may depend on any loop momentum.
        let module = kin.get_type().getattr("__module__")?.extract::<String>()?;
        let edge = match source.py().import(module.as_str())?.getattr("Symbols") {
            Ok(symbols) => Some(Momentum::new(
                &symbols.call_method0("edge_momentum")?.extract()?,
            )?),
            Err(_) => None,
        };
        let momenta = Momenta {
            loops: momenta(&loops)?,
            external: momenta(&external)?,
            edge,
        };
        let mut symbols = BTreeSet::new();
        for value in std::iter::once(&dimension)
            .chain(gram.iter().flatten())
            .chain(affine.iter().flatten())
        {
            Self::scalar_symbols(value.as_view(), &momenta, &mut symbols)?;
        }
        let names = (0..symbols.len())
            .map(|i| format!("feynkit_parameter_{i}"))
            .collect::<Vec<_>>();
        let context = CoefficientContext::try_new(names.clone()).map_err(super::value_error)?;
        let forward = symbols
            .into_iter()
            .zip(names.iter().map(|name| {
                context
                    .parameter(name)
                    .expect("declared bridge parameter")
                    .to_expression()
            }))
            .collect::<BTreeMap<_, _>>();
        let coefficient = |value: &Atom| -> PyResult<Coefficient> {
            let renamed = Self::rename(value, &forward);
            let result = renamed
                .try_to_rational_polynomial(&Q, &Z, Some(context.one().get_variables().clone()))
                .map_err(|error| {
                    PyValueError::new_err(format!(
                        "IBP coefficient {value} must be a rational function of scalar symbols and invariants: {error}"
                    ))
                })?;
            if !context.contains(&result) {
                return Err(PyValueError::new_err(format!(
                    "undeclared scalar dependence in IBP coefficient {value}"
                )));
            }
            Ok(result)
        };
        let denominators = affine
            .iter()
            .map(|row| {
                Ok(AffineDenominator::new(
                    coefficient(&row[0])?,
                    row[1..].iter().map(&coefficient).collect::<PyResult<_>>()?,
                ))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let gram = gram
            .iter()
            .map(|row| row.iter().map(&coefficient).collect::<PyResult<Vec<_>>>())
            .collect::<PyResult<_>>()?;
        let dimension = coefficient(&dimension)?;
        let shifts = vec![context.zero(); denominators.len()];
        let family = IntegralFamily::new(
            name,
            (0..loops.len()).map(|i| format!("loop_{i}")).collect(),
            (0..external.len())
                .map(|i| format!("external_{i}"))
                .collect(),
            context,
            dimension,
            denominators,
            gram,
            shifts,
        )
        .map_err(super::value_error)?;
        Ok(Self {
            family,
            original_parameters: forward
                .into_iter()
                .map(|(original, internal)| (internal, original))
                .collect(),
        })
    }

    /// Collect the parameters: scalar symbols and opaque invariant calls.
    ///
    /// A call such as the tensor invariant `dot(q, q)` is one opaque parameter
    /// when its head is declared `Scalar`, is not a Symbolica built-in (whose
    /// relations, such as `log(s*t) = log(s) + log(t)`, the solver cannot see),
    /// keeps no factor inside a linear head, and mentions none of the family's
    /// momenta or FeynKit's routed edge momenta. Its arguments, such as the `d`
    /// in `mink(d)`, are not parameters. Only these syntactic checks are made:
    /// any other vector inside an invariant must itself be loop-independent.
    fn scalar_symbols(
        value: AtomView<'_>,
        momenta: &Momenta,
        symbols: &mut BTreeSet<Atom>,
    ) -> PyResult<()> {
        match value {
            AtomView::Var(_) | AtomView::Fun(_) => {
                let mentions =
                    |momenta: &[Momentum]| momenta.iter().any(|momentum| momentum.occurs_in(value));
                if mentions(&momenta.loops) {
                    return Err(PyValueError::new_err(format!(
                        "IBP coefficient {value} depends on a loop momentum; masses, invariants, and the dimension must be loop-independent"
                    )));
                }
                if mentions(&momenta.external) {
                    return Err(PyValueError::new_err(format!(
                        "IBP coefficient {value} contains an external momentum; assign all external scalar products in Kinematics to values free of the family's momenta"
                    )));
                }
                if mentions(momenta.edge.as_slice()) {
                    return Err(PyValueError::new_err(format!(
                        "IBP coefficient {value} contains a routed edge momentum, which may depend on loop momenta; write it through the family's momenta first"
                    )));
                }
                if let AtomView::Fun(function) = value {
                    let head = function.get_symbol();
                    if head.is_builtin() {
                        return Err(PyValueError::new_err(format!(
                            "IBP coefficient {value} calls a Symbolica built-in, whose relations the solver cannot use; introduce a symbol for it"
                        )));
                    }
                    if !head.is_scalar() {
                        return Err(PyValueError::new_err(format!(
                            "IBP coefficients must be rational functions of scalar symbols and of invariants such as dot(q, q), whose function is declared Scalar; {value} is not"
                        )));
                    }
                    if head.is_linear()
                        && function.iter().any(|arg| matches!(arg, AtomView::Mul(_)))
                    {
                        return Err(PyValueError::new_err(format!(
                            "IBP invariant {value} keeps a factor or an uncontracted tensor inside a linear function; contract or simplify the tensor expression, or factor scalar coefficients out"
                        )));
                    }
                }
                symbols.insert(value.to_owned());
            }
            AtomView::Add(sum) => {
                for child in sum.iter() {
                    Self::scalar_symbols(child, momenta, symbols)?;
                }
            }
            AtomView::Mul(product) => {
                for child in product.iter() {
                    Self::scalar_symbols(child, momenta, symbols)?;
                }
            }
            AtomView::Pow(power) => {
                let (base, exponent) = power.get_base_exp();
                if !matches!(exponent, AtomView::Num(number) if number.get_coeff_view().is_integer())
                {
                    return Err(PyValueError::new_err(format!(
                        "IBP coefficient {value} has a non-integer or symbolic exponent; introduce a symbol for roots, exponentials, or symbolic powers"
                    )));
                }
                Self::scalar_symbols(base, momenta, symbols)?;
            }
            AtomView::Num(_) => {}
        }
        Ok(())
    }

    /// Replace whole atoms top-down. An opaque invariant is therefore replaced
    /// before its arguments are visited, so a parameter such as the dimension
    /// is never substituted inside it.
    pub fn rename(value: &Atom, replacements: &BTreeMap<Atom, Atom>) -> Atom {
        value.replace_map(|view, _, output| {
            if let Some(replacement) = replacements.get(&view.to_owned()) {
                **output = replacement.clone();
            }
        })
    }
}
