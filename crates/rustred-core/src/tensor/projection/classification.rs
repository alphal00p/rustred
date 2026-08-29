//! Tensor-factor classification and admitted rank-two contraction.

use symbolica::atom::{AtomView, FunctionBuilder};

use crate::family::ScalarProductCoordinate;

use super::super::error::{TensorError, TensorHeadKind, check_limit};
use super::super::service::TensorService;
use super::super::syntax::{contains_exact_atom, first_reserved_head, reserved_kind};
use super::super::term::{InternalSlot, MomentumRef, TermParts};

impl TensorService<'_> {
    pub(super) fn classify_factor(
        &self,
        factor: AtomView<'_>,
        parts: &mut TermParts,
    ) -> Result<(), TensorError> {
        match factor {
            AtomView::Fun(function) => {
                let Some(kind) = reserved_kind(function.get_symbol(), &self.heads) else {
                    self.retain_opaque_scalar(factor, parts)?;
                    return Ok(());
                };
                if function.get_nargs() != 2 {
                    return Err(TensorError::MalformedReservedHead {
                        head: kind,
                        expected_arity: 2,
                        actual_arity: Some(function.get_nargs()),
                    });
                }
                let mut arguments = function.iter();
                let first = arguments
                    .next()
                    .expect("authenticated arity has a first argument");
                let second = arguments
                    .next()
                    .expect("authenticated arity has a second argument");
                self.classify_reserved_factor(kind, first, second, factor, parts)?;
            }
            AtomView::Var(variable) => {
                if let Some(head) = reserved_kind(variable.get_symbol(), &self.heads) {
                    return Err(TensorError::MalformedReservedHead {
                        head,
                        expected_arity: 2,
                        actual_arity: None,
                    });
                }
                self.retain_opaque_scalar(factor, parts)?;
            }
            AtomView::Add(_) => {
                if first_reserved_head(factor, &self.heads).is_some() {
                    return Err(TensorError::UnsupportedNestedTensorSum);
                }
                self.retain_opaque_scalar(factor, parts)?;
            }
            AtomView::Pow(_) | AtomView::Mul(_) | AtomView::Num(_) => {
                self.retain_opaque_scalar(factor, parts)?;
            }
        }
        check_limit(
            "internal tensor rank",
            parts.internal_slots.len(),
            self.limits.max_internal_rank,
        )
    }

    fn classify_reserved_factor(
        &self,
        kind: TensorHeadKind,
        first: AtomView<'_>,
        second: AtomView<'_>,
        factor: AtomView<'_>,
        parts: &mut TermParts,
    ) -> Result<(), TensorError> {
        match kind {
            TensorHeadKind::LoopVector => {
                let loop_index = match self.resolve_momentum(first)? {
                    MomentumRef::Loop(index) => index,
                    MomentumRef::External(_) => {
                        return Err(TensorError::UnknownMomentum {
                            momentum: first.to_owned(),
                        });
                    }
                };
                self.ensure_nonreserved(second)?;
                self.push_internal_slot(
                    InternalSlot::Free {
                        loop_index,
                        index: second.to_owned(),
                    },
                    parts,
                )?;
            }
            TensorHeadKind::ExternalVector => {
                if !matches!(self.resolve_momentum(first)?, MomentumRef::External(_)) {
                    return Err(TensorError::UnknownMomentum {
                        momentum: first.to_owned(),
                    });
                }
                self.ensure_nonreserved(second)?;
                parts.retained_tensor_indices.push(second.to_owned());
                parts.outside_factors.push(factor.to_owned());
            }
            TensorHeadKind::Metric => {
                self.ensure_nonreserved(first)?;
                self.ensure_nonreserved(second)?;
                parts.retained_tensor_indices.push(first.to_owned());
                parts.retained_tensor_indices.push(second.to_owned());
                parts.outside_factors.push(factor.to_owned());
            }
            TensorHeadKind::Dot => self.classify_dot(first, second, factor, parts)?,
        }
        Ok(())
    }

    fn classify_dot(
        &self,
        first: AtomView<'_>,
        second: AtomView<'_>,
        factor: AtomView<'_>,
        parts: &mut TermParts,
    ) -> Result<(), TensorError> {
        let left = self.resolve_momentum(first)?;
        let right = self.resolve_momentum(second)?;
        match (left, right) {
            (MomentumRef::Loop(left), MomentumRef::Loop(right)) => {
                self.push_scalar_product(ScalarProductCoordinate::LoopLoop { left, right }, parts)?
            }
            (MomentumRef::Loop(loop_index), MomentumRef::External(external_index))
            | (MomentumRef::External(external_index), MomentumRef::Loop(loop_index)) => {
                self.push_internal_slot(
                    InternalSlot::ExternalContracted {
                        loop_index,
                        external_index,
                    },
                    parts,
                )?;
            }
            (MomentumRef::External(_), MomentumRef::External(_)) => {
                parts.scalar_factors.push(factor.to_owned());
            }
        }
        Ok(())
    }

    fn retain_opaque_scalar(
        &self,
        factor: AtomView<'_>,
        parts: &mut TermParts,
    ) -> Result<(), TensorError> {
        if let Some(head) = first_reserved_head(factor, &self.heads) {
            return Err(TensorError::ReservedHeadInUnsupportedPosition { head });
        }
        for momentum in self.momenta.loop_momenta() {
            // Numeric labels such as Vakint's `k(1, mu)` IDs acquire momentum
            // meaning only in a reserved-head argument. The number `1` in an
            // opaque scalar such as `f(1)` is therefore not the vector `k(1)`.
            if matches!(momentum.as_view(), AtomView::Num(_)) {
                continue;
            }
            if contains_exact_atom(factor, momentum.as_view()) {
                return Err(TensorError::LoopMomentumInOpaqueScalar {
                    momentum: momentum.clone(),
                });
            }
        }
        parts.scalar_factors.push(factor.to_owned());
        Ok(())
    }

    fn ensure_nonreserved(&self, atom: AtomView<'_>) -> Result<(), TensorError> {
        if let Some(head) = first_reserved_head(atom, &self.heads) {
            Err(TensorError::ReservedHeadInUnsupportedPosition { head })
        } else {
            Ok(())
        }
    }

    fn resolve_momentum(&self, momentum: AtomView<'_>) -> Result<MomentumRef, TensorError> {
        if let Some(index) = self
            .momenta
            .loop_momenta()
            .iter()
            .position(|candidate| candidate.as_view() == momentum)
        {
            return Ok(MomentumRef::Loop(index));
        }
        if let Some(index) = self
            .momenta
            .external_momenta()
            .iter()
            .position(|candidate| candidate.as_view() == momentum)
        {
            return Ok(MomentumRef::External(index));
        }
        Err(TensorError::UnknownMomentum {
            momentum: momentum.to_owned(),
        })
    }

    fn push_scalar_product(
        &self,
        coordinate: ScalarProductCoordinate,
        parts: &mut TermParts,
    ) -> Result<(), TensorError> {
        let requested = parts.scalar_products.len().checked_add(1).ok_or(
            TensorError::ResourceCountOverflow {
                resource: "projected scalar products per term",
            },
        )?;
        check_limit(
            "projected scalar products per term",
            requested,
            self.limits.max_scalar_products_per_term,
        )?;
        parts.scalar_products.push(coordinate);
        Ok(())
    }

    fn push_internal_slot(
        &self,
        slot: InternalSlot,
        parts: &mut TermParts,
    ) -> Result<(), TensorError> {
        let requested = parts.internal_slots.len().checked_add(1).ok_or(
            TensorError::ResourceCountOverflow {
                resource: "internal tensor rank",
            },
        )?;
        check_limit(
            "internal tensor rank",
            requested,
            self.limits.max_internal_rank,
        )?;
        parts.internal_slots.push(slot);
        Ok(())
    }

    pub(super) fn reject_unsupported_index_contractions(
        &self,
        parts: &TermParts,
    ) -> Result<(), TensorError> {
        for (position, slot) in parts.internal_slots.iter().enumerate() {
            let InternalSlot::Free { index, .. } = slot else {
                continue;
            };
            let repeats_free = parts.internal_slots[..position].iter().any(|candidate| {
                matches!(candidate, InternalSlot::Free { index: previous, .. } if previous == index)
            });
            if repeats_free || parts.retained_tensor_indices.contains(index) {
                return Err(TensorError::UnsupportedLorentzIndexContraction {
                    index: index.clone(),
                });
            }
        }
        Ok(())
    }

    pub(super) fn contract_rank_two(
        &self,
        left: InternalSlot,
        right: InternalSlot,
        parts: &mut TermParts,
    ) -> Result<(), TensorError> {
        let (left_loop, right_loop) = match (&left, &right) {
            (InternalSlot::Free { loop_index, .. }, InternalSlot::Free { loop_index: r, .. })
            | (
                InternalSlot::Free { loop_index, .. },
                InternalSlot::ExternalContracted { loop_index: r, .. },
            )
            | (
                InternalSlot::ExternalContracted { loop_index, .. },
                InternalSlot::Free { loop_index: r, .. },
            )
            | (
                InternalSlot::ExternalContracted { loop_index, .. },
                InternalSlot::ExternalContracted { loop_index: r, .. },
            ) => (*loop_index, *r),
        };
        self.push_scalar_product(
            ScalarProductCoordinate::LoopLoop {
                left: left_loop,
                right: right_loop,
            },
            parts,
        )?;
        let outside = match (left, right) {
            (InternalSlot::Free { index: left, .. }, InternalSlot::Free { index: right, .. }) => {
                FunctionBuilder::new(self.heads.metric())
                    .add_arg(left)
                    .add_arg(right)
                    .finish()
            }
            (
                InternalSlot::Free { index, .. },
                InternalSlot::ExternalContracted { external_index, .. },
            )
            | (
                InternalSlot::ExternalContracted { external_index, .. },
                InternalSlot::Free { index, .. },
            ) => FunctionBuilder::new(self.heads.external_vector())
                .add_arg(self.momenta.external_momenta()[external_index].clone())
                .add_arg(index)
                .finish(),
            (
                InternalSlot::ExternalContracted {
                    external_index: left,
                    ..
                },
                InternalSlot::ExternalContracted {
                    external_index: right,
                    ..
                },
            ) => FunctionBuilder::new(self.heads.dot())
                .add_arg(self.momenta.external_momenta()[left].clone())
                .add_arg(self.momenta.external_momenta()[right].clone())
                .finish(),
        };
        parts.outside_factors.push(outside);
        Ok(())
    }
}
