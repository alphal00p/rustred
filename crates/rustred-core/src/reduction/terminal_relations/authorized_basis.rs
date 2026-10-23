//! Independent basis of caller-authorized rows through the existing native
//! sparse reducer. This adapter performs no source or normalization search.

use super::*;
use std::collections::HashSet;

/// Exact arbitrary-column basis and one shared domain for the whole span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalRelationEquationBasis {
    pub equations: Vec<TerminalRelationRow>,
    pub nonzero_conditions: Vec<Coefficient>,
}

impl TerminalRelationSession {
    /// Keep only an independent basis of explicitly authorized equations.
    /// All columns, including unresolved auxiliaries, remain ordinary keys.
    /// The caller retains the original equation authority; this is only its
    /// exact row span, with the union of input domains and native pivot guards.
    pub fn independent_equation_basis(
        family: Arc<IntegralFamily>,
        equations: &[TerminalEquation],
        limits: TerminalRelationLimits,
    ) -> Result<TerminalRelationEquationBasis, TerminalRelationError> {
        Self::independent_equation_basis_with_control(
            family,
            equations,
            limits,
            &AtomicBool::new(false),
            |_, _| {},
        )
    }

    pub fn independent_equation_basis_with_control(
        family: Arc<IntegralFamily>,
        equations: &[TerminalEquation],
        limits: TerminalRelationLimits,
        cancel: &AtomicBool,
        progress: impl Fn(usize, usize),
    ) -> Result<TerminalRelationEquationBasis, TerminalRelationError> {
        let checkpoint = || {
            if cancel.load(Ordering::Relaxed) {
                Err(invalid("authorized equation basis cancelled"))
            } else {
                Ok(())
            }
        };
        checkpoint()?;
        check("authorized equations", equations.len(), limits.max_rows)?;
        let context = family.coefficient_context();
        let mut keys = BTreeSet::new();
        let mut conditions = Vec::new();
        let mut seen = HashSet::new();
        let mut symbolic = false;
        let mut input_nonzeros = 0usize;
        for equation in equations {
            checkpoint()?;
            input_nonzeros = input_nonzeros
                .checked_add(equation.terms.len())
                .ok_or_else(|| invalid("authorized equation nonzero count overflow"))?;
            check(
                "authorized equation nonzeros",
                input_nonzeros,
                limits.max_nonzeros,
            )?;
            for (key, value) in &equation.terms {
                if key.powers().len() != family.denominator_count() {
                    return Err(invalid("authorized equation arity differs from family"));
                }
                context
                    .validate_with_limits(value, limits.algebra.exact_algebra)
                    .map_err(algebra)?;
                if value.is_zero() {
                    continue;
                }
                keys.insert(key.clone());
                symbolic |= !value.is_constant();
                if !value.denominator.is_constant() {
                    retain(
                        context,
                        value.denominator.clone().into(),
                        &mut conditions,
                        &mut seen,
                        limits,
                    )?;
                }
            }
            for condition in &equation.nonzero_conditions {
                retain(
                    context,
                    condition.clone(),
                    &mut conditions,
                    &mut seen,
                    limits,
                )?;
            }
        }
        check(
            "authorized equation columns",
            keys.len(),
            limits.max_columns,
        )?;
        let mut columns: Vec<_> = keys.into_iter().collect();
        elimination::sort_keys(&mut columns);
        let width = u32::try_from(columns.len()).map_err(invalid)?;
        let offsets: BTreeMap<_, _> = columns
            .iter()
            .enumerate()
            .map(|(i, key)| (key.clone(), i as u32))
            .collect();
        // Constant-coefficient symmetry rows need no factor ledger. For the
        // generic path, native L retains the actual pre-normalization pivot;
        // its numerator is required to authorize division by that pivot.
        let mode = if symbolic {
            LuLMode::Full
        } else {
            LuLMode::None
        };
        let mut reducer = SparseRowReducer::new(width, RationalPolynomialField::new(Z), mode);
        for (index, equation) in equations.iter().enumerate() {
            checkpoint()?;
            check(
                "authorized basis rows",
                reducer.u().nrows() as usize + 1,
                limits.max_rows,
            )?;
            let retained = reducer
                .u()
                .nvalues()
                .checked_add(reducer.l().nvalues())
                .and_then(|n| n.checked_add(columns.len()))
                .and_then(|n| {
                    if symbolic {
                        n.checked_add(reducer.u().nrows() as usize + 1)
                    } else {
                        Some(n)
                    }
                })
                .ok_or_else(|| invalid("authorized basis retained nonzero count overflow"))?;
            check(
                "authorized basis stored nonzeros",
                retained,
                limits.max_nonzeros,
            )?;
            let mut terms: Vec<_> = equation
                .terms
                .iter()
                .filter(|(_, c)| !c.is_zero())
                .map(|(key, value)| (offsets[key], value.clone()))
                .collect();
            terms.sort_by_key(|(id, _)| *id);
            let ids: Vec<_> = terms.iter().map(|(id, _)| *id).collect();
            let values: Vec<_> = terms.into_iter().map(|(_, value)| value).collect();
            if reducer.add_row(&values, &ids).is_some() && symbolic {
                let pivot = reducer
                    .l()
                    .values()
                    .last()
                    .ok_or_else(|| invalid("native authorized basis omitted pivot ledger"))?;
                context
                    .validate_with_limits(pivot, limits.algebra.exact_algebra)
                    .map_err(algebra)?;
                if !pivot.numerator.is_constant() {
                    retain(
                        context,
                        pivot.numerator.clone().into(),
                        &mut conditions,
                        &mut seen,
                        limits,
                    )?;
                }
                if !pivot.denominator.is_constant() {
                    retain(
                        context,
                        pivot.denominator.clone().into(),
                        &mut conditions,
                        &mut seen,
                        limits,
                    )?;
                }
            }
            progress(index + 1, reducer.u().nrows() as usize);
        }
        checkpoint()?;
        let native = reducer.u();
        let mut output = Vec::new();
        for span in native.row_ptrs().windows(2) {
            let mut row = BTreeMap::new();
            for (&column, value) in native.col_idcs()[span[0]..span[1]]
                .iter()
                .zip(&native.values()[span[0]..span[1]])
            {
                context
                    .validate_with_limits(value, limits.algebra.exact_algebra)
                    .map_err(algebra)?;
                if !value.denominator.is_constant() {
                    retain(
                        context,
                        value.denominator.clone().into(),
                        &mut conditions,
                        &mut seen,
                        limits,
                    )?;
                }
                row.insert(columns[column as usize].clone(), value.clone());
            }
            output.push(row);
        }
        Ok(TerminalRelationEquationBasis {
            equations: output,
            nonzero_conditions: conditions,
        })
    }
}

fn retain(
    context: &crate::algebra::CoefficientContext,
    value: Coefficient,
    conditions: &mut Vec<Coefficient>,
    seen: &mut HashSet<Coefficient>,
    limits: TerminalRelationLimits,
) -> Result<(), TerminalRelationError> {
    context
        .validate_with_limits(&value, limits.algebra.exact_algebra)
        .map_err(algebra)?;
    if value.is_zero() {
        return Err(invalid("authorized equation nonzero condition vanishes"));
    }
    if seen.insert(value.clone()) {
        check(
            "authorized basis nonzero conditions",
            conditions.len() + 1,
            limits.max_nonzeros,
        )?;
        conditions.push(value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::family::AffineDenominator;

    #[test]
    fn basis_retains_dependent_input_domains_and_native_pivot_guard_once() {
        let context = CoefficientContext::new(["d"]);
        let family = Arc::new(
            IntegralFamily::new(
                "authorized-basis",
                vec!["k".into()],
                vec![],
                context.clone(),
                context.parameter("d").unwrap(),
                vec![AffineDenominator::new(
                    context.integer(-1),
                    vec![context.integer(1)],
                )],
                vec![],
                vec![context.zero()],
            )
            .unwrap(),
        );
        let key = IntegralKey::try_new([1]).unwrap();
        let d = context.parameter("d").unwrap();
        let limits = TerminalRelationLimits::default();
        let pivot = context
            .try_sub(&d, &context.integer(2), limits.algebra.exact_algebra)
            .unwrap();
        let guard = context
            .try_sub(&d, &context.integer(3), limits.algebra.exact_algebra)
            .unwrap();
        let equations = vec![
            TerminalEquation {
                terms: BTreeMap::from([(key.clone(), pivot.clone())]),
                nonzero_conditions: Vec::new(),
            },
            TerminalEquation {
                terms: BTreeMap::from([(key.clone(), context.integer(2))]),
                nonzero_conditions: vec![guard.clone(), guard.clone()],
            },
        ];
        let basis =
            TerminalRelationSession::independent_equation_basis(family.clone(), &equations, limits)
                .unwrap();
        assert_eq!(
            basis.equations,
            vec![BTreeMap::from([(key, context.one())])]
        );
        assert_eq!(basis.nonzero_conditions, vec![guard, pivot]);
        assert!(
            TerminalRelationSession::independent_equation_basis_with_control(
                family,
                &equations,
                limits,
                &AtomicBool::new(true),
                |_, _| {}
            )
            .is_err()
        );
    }

    #[test]
    fn many_rows_share_one_guard_envelope_and_keep_auxiliary_columns() {
        let context = CoefficientContext::new(["d"]);
        let family = Arc::new(
            IntegralFamily::new(
                "authorized-basis-shared",
                vec!["k".into()],
                vec![],
                context.clone(),
                context.parameter("d").unwrap(),
                vec![AffineDenominator::new(
                    context.integer(-1),
                    vec![context.integer(1)],
                )],
                vec![],
                vec![context.zero()],
            )
            .unwrap(),
        );
        let guards: Vec<_> = (1..=64)
            .map(|i| {
                context
                    .try_sub(
                        &context.parameter("d").unwrap(),
                        &context.integer(i),
                        crate::algebra::ExactAlgebraLimits::unbounded_resources(),
                    )
                    .unwrap()
            })
            .collect();
        let mut equations = vec![TerminalEquation {
            terms: BTreeMap::new(),
            nonzero_conditions: guards.clone(),
        }];
        for power in 1..=64 {
            equations.push(TerminalEquation {
                terms: BTreeMap::from([
                    (IntegralKey::try_new([power]).unwrap(), context.one()),
                    (IntegralKey::try_new([-power]).unwrap(), context.integer(-1)),
                ]),
                nonzero_conditions: Vec::new(),
            });
        }
        let basis = TerminalRelationSession::independent_equation_basis(
            family,
            &equations,
            TerminalRelationLimits::default(),
        )
        .unwrap();
        assert_eq!(basis.equations.len(), 64);
        assert_eq!(basis.nonzero_conditions, guards);
        assert!(
            basis
                .equations
                .iter()
                .all(|row| row.len() == 2 && row.keys().any(|key| key.powers()[0] < 0))
        );
    }

    #[test]
    fn eliminated_symbolic_pivot_and_dependent_input_pole_remain_guarded() {
        let context = CoefficientContext::new(["d"]);
        let family = Arc::new(
            IntegralFamily::new(
                "authorized-basis-eliminated-pivot",
                vec!["k".into()],
                vec![],
                context.clone(),
                context.parameter("d").unwrap(),
                vec![AffineDenominator::new(
                    context.integer(-1),
                    vec![context.integer(1)],
                )],
                vec![],
                vec![context.zero()],
            )
            .unwrap(),
        );
        let limits = TerminalRelationLimits::default();
        let exact = limits.algebra.exact_algebra;
        let d = context.parameter("d").unwrap();
        let square = context.try_mul(&d, &d, exact).unwrap();
        let pivot = context.try_sub(&square, &d, exact).unwrap();
        let pole = context.try_sub(&d, &context.integer(3), exact).unwrap();
        let mut keys = vec![
            IntegralKey::try_new([1]).unwrap(),
            IntegralKey::try_new([-1]).unwrap(),
        ];
        elimination::sort_keys(&mut keys);
        let equation = |left, right| TerminalEquation {
            terms: BTreeMap::from([(keys[0].clone(), left), (keys[1].clone(), right)]),
            nonzero_conditions: Vec::new(),
        };
        let equations = vec![
            equation(context.one(), d.clone()),
            equation(context.one(), square),
            equation(
                context.try_div(&context.one(), &pole, exact).unwrap(),
                context.try_div(&d, &pole, exact).unwrap(),
            ),
        ];
        let basis = TerminalRelationSession::independent_equation_basis(family, &equations, limits)
            .unwrap();
        assert_eq!(basis.equations.len(), 2);
        assert_eq!(basis.nonzero_conditions, vec![pole, pivot]);
        let columns: BTreeSet<_> = basis
            .equations
            .iter()
            .flat_map(|row| row.keys().cloned())
            .collect();
        assert_eq!(columns, keys.into_iter().collect());
    }
}
