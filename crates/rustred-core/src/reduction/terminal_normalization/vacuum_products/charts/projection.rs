use std::collections::HashSet;
use std::fmt;
use symbolica::atom::{Atom, AtomCore, AtomView, FunctionBuilder, Symbol};

use super::super::{VacuumProductError as Error, algebra};
use super::VerifiedVacuumProductChart;
use crate::algebra::Coefficient;
use crate::family::{IntegralKey, ScalarProductCoordinate};
use crate::reduction::terminal_normalization::VacuumFactor;
use crate::scalar_numerator::{
    FamilyScalarNumeratorService, ScalarNumeratorLimits, WeightedScalarWork,
};

/// Scalar syntax in the verified canonical component coordinates. The native
/// dimension is d, never an epsilon-substituted adapter dimension. Vectors in
/// other components are tensor spectators, not denominator-dependent externals.
#[derive(Clone, Debug)]
pub struct VacuumTensorProjectionInput {
    pub numerator: Atom,
    pub dot_head: Symbol,
    pub component_loop_momenta: Vec<Vec<Atom>>,
    pub dimension: Atom,
}

/// Result from an existing tensor projector. All original projector poles must
/// be reported before expression simplification can cancel them. Only exact
/// native coefficient-field guards and final scalar coefficients are accepted.
#[derive(Clone, Debug)]
pub struct VacuumTensorProjection {
    pub numerator: Atom,
    pub nonzero_conditions: Vec<Atom>,
}

#[derive(Clone, Debug)]
pub struct VacuumProductProjection {
    pub terms: Vec<(Vec<VacuumFactor>, Coefficient)>,
    pub nonzero_conditions: Vec<Coefficient>,
}

impl VerifiedVacuumProductChart {
    /// Transport the complete original numerator, call the supplied existing
    /// tensor engine only for a cross-component coupling, and lower/coalesce its
    /// scalar result at unit mass. No tensor identity is implemented here.
    /// Residual cross-component ISP powers or non-native scalar spectators fail.
    /// Returned products can be passed directly to `VacuumProductReducer`.
    pub fn project<E: fmt::Display>(
        &self,
        dot_head: Symbol,
        loop_momenta: Vec<Atom>,
        limits: ScalarNumeratorLimits,
        mut projector: impl FnMut(&VacuumTensorProjectionInput) -> Result<VacuumTensorProjection, E>,
    ) -> Result<VacuumProductProjection, Error> {
        let service = FamilyScalarNumeratorService::try_new(
            &self.combined,
            dot_head,
            loop_momenta.clone(),
            limits,
        )
        .map_err(algebra)?;
        let cross_start = self
            .components
            .last()
            .map_or(0, |block| block.denominators.end);
        let transported = self
            .transport
            .transport(&self.source, self.limits.transport)
            .map_err(algebra)?;
        let mut has_cross = false;
        let mut numerator = Atom::num(0);
        let mut atoms = Vec::new();
        let mut widths = Vec::new();
        for denominator in self.combined.denominators() {
            let mut expression = denominator.constant().to_expression();
            for (coordinate, coefficient) in self
                .combined
                .coordinates()
                .iter()
                .zip(denominator.coefficients())
            {
                if coefficient.is_zero() {
                    continue;
                }
                let ScalarProductCoordinate::LoopLoop { left, right } = *coordinate else {
                    unreachable!("vacuum chart");
                };
                let dot = FunctionBuilder::new(dot_head)
                    .add_arg(&loop_momenta[left])
                    .add_arg(&loop_momenta[right])
                    .finish();
                expression += coefficient.to_expression() * dot;
            }
            widths.push(match expression.as_view() {
                AtomView::Add(sum) => sum.iter().count(),
                _ => 1,
            });
            atoms.push(expression);
        }
        let mut expanded_terms = 0usize;
        for endpoint in transported.terms() {
            has_cross |= endpoint.key().powers()[cross_start..]
                .iter()
                .any(|&p| p != 0);
            let mut term = endpoint.coefficient().to_expression();
            let mut width = 1usize;
            let mut degree = 0usize;
            for (((&base, &power), atom), &atom_width) in self
                .base
                .powers()
                .iter()
                .zip(endpoint.key().powers())
                .zip(&atoms)
                .zip(&widths)
            {
                let exponent = base.checked_sub(power).and_then(|p| u32::try_from(p).ok()).ok_or_else(|| Error::InvalidInput("transport increased a denominator power or numerator degree overflowed".into()))?;
                width = atom_width
                    .checked_pow(exponent)
                    .and_then(|power| width.checked_mul(power))
                    .unwrap_or(usize::MAX);
                if limits.diagnostics_enabled && width > limits.max_polynomial_terms {
                    return Err(Error::ResourceLimit {
                        resource: "projection expanded terms",
                        requested: width,
                        limit: limits.max_polynomial_terms,
                    });
                }
                degree = degree.saturating_add(exponent as usize);
                if limits.diagnostics_enabled && degree > limits.max_scalar_product_degree {
                    return Err(Error::ResourceLimit {
                        resource: "projection numerator degree",
                        requested: degree,
                        limit: limits.max_scalar_product_degree,
                    });
                }
                if exponent != 0 {
                    term *= atom.clone().pow(exponent);
                }
            }
            expanded_terms = expanded_terms.saturating_add(width);
            if limits.diagnostics_enabled && expanded_terms > limits.max_polynomial_terms {
                return Err(Error::ResourceLimit {
                    resource: "projection expanded terms",
                    requested: expanded_terms,
                    limit: limits.max_polynomial_terms,
                });
            }
            numerator += term;
            census(&numerator, limits)?;
        }
        let numerator =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| numerator.expand()))
                .map_err(|_| Error::ExactAlgebra("native numerator expansion panicked".into()))?;
        census(&numerator, limits)?;
        let input = VacuumTensorProjectionInput {
            numerator,
            dot_head,
            component_loop_momenta: self
                .components
                .iter()
                .map(|block| loop_momenta[block.loops.clone()].to_vec())
                .collect(),
            dimension: self.combined.dimension().to_expression(),
        };
        let projected = if has_cross {
            projector(&input)
                .map_err(|error| Error::InvalidInput(format!("tensor projector failed: {error}")))?
        } else {
            VacuumTensorProjection {
                numerator: input.numerator,
                nonzero_conditions: Vec::new(),
            }
        };
        let mut work = WeightedScalarWork::new(limits);
        let lowered = service
            .lower_weighted_in_request(&projected.numerator, &self.base, &Atom::num(1), &mut work)
            .map_err(algebra)?;
        let context = self.combined.coefficient_context();
        let mut conditions = Vec::new();
        let mut seen = HashSet::new();
        let mut retain = |value: Coefficient| -> Result<(), Error> {
            context
                .validate_with_limits(&value, limits.exact_algebra)
                .map_err(algebra)?;
            if value.is_zero() {
                return Err(Error::InvalidInput(
                    "a projection nonzero condition vanished".into(),
                ));
            }
            for value in [value.denominator.clone().into(), value] {
                if seen.insert(value.clone()) {
                    if limits.diagnostics_enabled && conditions.len() >= limits.max_input_terms {
                        return Err(Error::ResourceLimit {
                            resource: "projection conditions",
                            requested: conditions.len().saturating_add(1),
                            limit: limits.max_input_terms,
                        });
                    }
                    conditions.push(value);
                }
            }
            Ok(())
        };
        for condition in self.transport.verified_map().nonzero_conditions() {
            retain(condition.polynomial().clone().into())?;
        }
        let zero =
            IntegralKey::try_new(vec![0; self.combined.denominator_count()]).map_err(algebra)?;
        let mut guard_nodes = 0usize;
        for condition in projected
            .nonzero_conditions
            .iter()
            .chain(lowered.nonzero_conditions())
        {
            guard_nodes = guard_nodes.saturating_add(census(condition, limits)?);
            if limits.diagnostics_enabled && guard_nodes > limits.max_input_nodes {
                return Err(Error::ResourceLimit {
                    resource: "projection guard nodes",
                    requested: guard_nodes,
                    limit: limits.max_input_nodes,
                });
            }
            // Reuse the bounded scalar service to reject loop-dependent or
            // foreign spectators and to preserve uncanceled input poles.
            let guard = service
                .lower_weighted_in_request(condition, &zero, &Atom::num(1), &mut work)
                .map_err(algebra)?;
            if guard.terms().len() != 1
                || guard.terms()[0].integral() != &zero
                || guard.terms()[0].scalar_spectator() != &Atom::num(1)
            {
                return Err(Error::InvalidInput(
                    "tensor projector guard is not a nonzero native coefficient".into(),
                ));
            }
            retain(guard.terms()[0].coefficient().clone())?;
            // The service exposes any pole that vanished while collecting the
            // guard itself. Convert only after its bounded exact validation.
            for pole in guard.nonzero_conditions() {
                use symbolica::prelude::{Q, Z};
                work.preflight(pole.as_view()).map_err(algebra)?;
                let value: Coefficient = pole
                    .try_to_rational_polynomial(&Q, &Z, Some(context.variables().clone()))
                    .map_err(algebra)?;
                retain(value)?;
            }
        }
        let mut terms = Vec::new();
        for term in lowered.terms() {
            if term.scalar_spectator() != &Atom::num(1) || term.common_mass_squared_power() != 0 {
                return Err(Error::InvalidInput(
                    "projected numerator has a foreign scalar coefficient".into(),
                ));
            }
            if term.integral().powers()[cross_start..]
                .iter()
                .any(|&p| p != 0)
            {
                return Err(Error::InvalidInput(
                    "tensor projection left a cross-component scalar product".into(),
                ));
            }
            retain(term.coefficient().denominator.clone().into())?;
            let factors = self
                .components
                .iter()
                .map(|block| {
                    Ok(VacuumFactor {
                        family: block.family.clone(),
                        integral: IntegralKey::try_new(
                            term.integral().powers()[block.denominators.clone()]
                                .iter()
                                .copied(),
                        )
                        .map_err(algebra)?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            terms.push((factors, term.coefficient().clone()));
        }
        Ok(VacuumProductProjection {
            terms,
            nonzero_conditions: conditions,
        })
    }
}

fn census(atom: &Atom, limits: ScalarNumeratorLimits) -> Result<usize, Error> {
    let limit = limits.max_input_nodes;
    let max_depth = limits.max_nesting_depth;
    let mut nodes = 0usize;
    let mut stack = vec![(atom.as_view(), 0usize)];
    while let Some((node, depth)) = stack.pop() {
        symbolica::utils::resources::checkpoint().map_err(Error::Resources)?;
        if limits.diagnostics_enabled && depth > max_depth {
            return Err(Error::ResourceLimit {
                resource: "projection atom depth",
                requested: depth,
                limit: max_depth,
            });
        }
        nodes = nodes.saturating_add(1);
        if limits.diagnostics_enabled && nodes > limit {
            return Err(Error::ResourceLimit {
                resource: "projection atom nodes",
                requested: nodes,
                limit,
            });
        }
        match node {
            AtomView::Fun(f) => stack.extend(f.iter().map(|arg| (arg, depth + 1))),
            AtomView::Add(s) => stack.extend(s.iter().map(|arg| (arg, depth + 1))),
            AtomView::Mul(p) => stack.extend(p.iter().map(|arg| (arg, depth + 1))),
            AtomView::Pow(p) => {
                stack.push((p.get_base(), depth + 1));
                stack.push((p.get_exp(), depth + 1));
            }
            _ => {}
        }
    }
    Ok(nodes)
}
