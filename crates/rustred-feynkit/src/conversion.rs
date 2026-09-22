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
        let mut symbols = BTreeSet::new();
        for value in std::iter::once(&dimension)
            .chain(gram.iter().flatten())
            .chain(affine.iter().flatten())
        {
            Self::scalar_symbols(value.as_view(), &mut symbols)?;
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
                        "IBP coefficients must be rational functions of scalar symbols: {error}"
                    ))
                })?;
            if !context.contains(&result) {
                return Err(PyValueError::new_err(
                    "undeclared scalar dependence in IBP coefficient",
                ));
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

    fn scalar_symbols(value: AtomView<'_>, symbols: &mut BTreeSet<Atom>) -> PyResult<()> {
        match value {
            AtomView::Var(_) => {
                symbols.insert(value.to_owned());
            }
            AtomView::Add(sum) => {
                for child in sum.iter() {
                    Self::scalar_symbols(child, symbols)?;
                }
            }
            AtomView::Mul(product) => {
                for child in product.iter() {
                    Self::scalar_symbols(child, symbols)?;
                }
            }
            AtomView::Pow(power) => {
                for child in power.iter() {
                    Self::scalar_symbols(child, symbols)?;
                }
            }
            AtomView::Num(_) => {}
            AtomView::Fun(_) => {
                return Err(PyValueError::new_err(
                    "IBP coefficients must be rational functions of scalar symbols; assign all external scalar products in Kinematics",
                ));
            }
        }
        Ok(())
    }

    pub fn rename(value: &Atom, replacements: &BTreeMap<Atom, Atom>) -> Atom {
        value.replace_map(|view, _, output| {
            if let Some(replacement) = replacements.get(&view.to_owned()) {
                **output = replacement.clone();
            }
        })
    }
}
