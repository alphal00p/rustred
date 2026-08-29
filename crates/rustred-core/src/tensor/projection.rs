//! Bounded global rank-two isotropic projection.

use symbolica::atom::{Atom, AtomView};

use super::error::{TensorError, check_limit, checked_mul};
use super::model::{ProjectedTensorTerm, TensorGuard, TensorGuardOrigin, TensorProjection};
use super::service::TensorService;
use super::syntax::census_atom;
use super::term::TermParts;

mod classification;

impl TensorService<'_> {
    pub(super) fn project_impl(&self, numerator: &Atom) -> Result<TensorProjection, TensorError> {
        let input_nodes = census_atom(
            numerator.as_view(),
            "tensor numerator nodes",
            self.limits.max_input_nodes,
            self.limits,
        )?;
        let compared_labels = self
            .momenta
            .loop_momenta()
            .iter()
            .filter(|momentum| !matches!(momentum.as_view(), AtomView::Num(_)))
            .count();
        let label_checks = checked_mul(
            "opaque-scalar loop-momentum label checks",
            input_nodes,
            compared_labels,
        )?;
        check_limit(
            "opaque-scalar loop-momentum label checks",
            label_checks,
            self.limits.max_loop_momentum_label_checks,
        )?;
        let input_terms = match numerator.as_view() {
            AtomView::Add(sum) => sum.iter().count(),
            _ => 1,
        };
        check_limit(
            "tensor numerator terms",
            input_terms,
            self.limits.max_input_terms,
        )?;

        let mut terms = Vec::new();
        terms
            .try_reserve_exact(input_terms.min(self.limits.max_projected_terms))
            .map_err(|_| TensorError::AllocationFailure {
                resource: "projected tensor terms",
                requested: input_terms.min(self.limits.max_projected_terms),
            })?;
        let mut needs_dimension_guard = false;
        match numerator.as_view() {
            AtomView::Add(sum) => {
                for term in sum.iter() {
                    if let Some(projected) = self.project_term(term, &mut needs_dimension_guard)? {
                        let requested = terms.len().checked_add(1).ok_or(
                            TensorError::ResourceCountOverflow {
                                resource: "projected tensor terms",
                            },
                        )?;
                        check_limit(
                            "projected tensor terms",
                            requested,
                            self.limits.max_projected_terms,
                        )?;
                        terms.push(projected);
                    }
                }
            }
            term => {
                if let Some(projected) = self.project_term(term, &mut needs_dimension_guard)? {
                    check_limit("projected tensor terms", 1, self.limits.max_projected_terms)?;
                    terms.push(projected);
                }
            }
        }

        let mut guards = Vec::new();
        if needs_dimension_guard {
            guards
                .try_reserve_exact(1)
                .map_err(|_| TensorError::AllocationFailure {
                    resource: "tensor nonzero guards",
                    requested: 1,
                })?;
            guards.push(TensorGuard {
                polynomial: self.presentation().family().dimension().numerator.clone(),
                origin: TensorGuardOrigin::RankTwoProjectorDimension,
            });
        }
        Ok(TensorProjection {
            family_identity: self.presentation().family().fingerprint_owner(),
            lane: self.lane(),
            terms,
            guards,
        })
    }

    fn project_term(
        &self,
        term: AtomView<'_>,
        needs_dimension_guard: &mut bool,
    ) -> Result<Option<ProjectedTensorTerm>, TensorError> {
        let factor_count = match term {
            AtomView::Mul(product) => product.iter().count(),
            _ => 1,
        };
        check_limit(
            "tensor factors per term",
            factor_count,
            self.limits.max_factors_per_term,
        )?;
        let mut parts = TermParts::new();
        parts
            .scalar_factors
            .try_reserve_exact(factor_count)
            .map_err(|_| TensorError::AllocationFailure {
                resource: "tensor scalar factors",
                requested: factor_count,
            })?;
        parts
            .outside_factors
            .try_reserve_exact(factor_count)
            .map_err(|_| TensorError::AllocationFailure {
                resource: "tensor outside factors",
                requested: factor_count,
            })?;
        parts
            .scalar_products
            .try_reserve_exact(factor_count.min(self.limits.max_scalar_products_per_term))
            .map_err(|_| TensorError::AllocationFailure {
                resource: "projected scalar products",
                requested: factor_count.min(self.limits.max_scalar_products_per_term),
            })?;
        parts
            .internal_slots
            .try_reserve_exact(factor_count.min(self.limits.max_internal_rank))
            .map_err(|_| TensorError::AllocationFailure {
                resource: "internal tensor slots",
                requested: factor_count.min(self.limits.max_internal_rank),
            })?;
        let retained_index_capacity =
            checked_mul("retained outside tensor indices", factor_count, 2)?;
        parts
            .retained_tensor_indices
            .try_reserve_exact(retained_index_capacity)
            .map_err(|_| TensorError::AllocationFailure {
                resource: "retained outside tensor indices",
                requested: retained_index_capacity,
            })?;
        match term {
            AtomView::Mul(product) => {
                for factor in product.iter() {
                    self.classify_factor(factor, &mut parts)?;
                }
            }
            factor => self.classify_factor(factor, &mut parts)?,
        }

        let rank = parts.internal_slots.len();
        check_limit("internal tensor rank", rank, self.limits.max_internal_rank)?;
        if rank > 0 {
            self.reject_unsupported_index_contractions(&parts)?;
        }
        if rank % 2 == 1 {
            return Ok(None);
        }
        let coefficient = if rank == 0 {
            self.presentation().family().coefficient_context().one()
        } else if rank == 2 {
            let family = self.presentation().family();
            if family.dimension().is_zero() {
                return Err(TensorError::SingularDimension);
            }
            let one = family.coefficient_context().one();
            let inverse_dimension = family.coefficient_context().try_div(
                &one,
                family.dimension(),
                self.limits.exact_algebra,
            )?;
            let right = parts
                .internal_slots
                .pop()
                .expect("rank-two admission has a second internal slot");
            let left = parts
                .internal_slots
                .pop()
                .expect("rank-two admission has a first internal slot");
            self.contract_rank_two(left, right, &mut parts)?;
            if !family.dimension().numerator.is_constant() {
                *needs_dimension_guard = true;
            }
            inverse_dimension
        } else {
            return Err(TensorError::UnsupportedEvenRank { rank, supported: 2 });
        };

        let scalar_spectator = product_or_one(parts.scalar_factors);
        if scalar_spectator.as_view().is_zero() {
            return Ok(None);
        }
        Ok(Some(ProjectedTensorTerm {
            coefficient,
            scalar_spectator,
            outside_tensor: product_or_one(parts.outside_factors),
            scalar_products: parts.scalar_products,
        }))
    }
}

fn product_or_one(factors: Vec<Atom>) -> Atom {
    if factors.is_empty() {
        Atom::num(1)
    } else {
        Atom::mul_many(factors)
    }
}
