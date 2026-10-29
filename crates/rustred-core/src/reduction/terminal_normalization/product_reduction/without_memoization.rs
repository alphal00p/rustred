//! Occurrence-local product composition. A completed child is a transient
//! operand for its immediate parent, never an entry in a reusable result map.
use std::collections::BTreeMap;

use super::arithmetic::{Conditions, accumulate, convolve};
use super::engine::{Request, Row};
use super::model::*;
use crate::reduction::ReductionError;
use crate::reduction::terminal_normalization::VacuumIntegralKey;

type Error = VacuumProductReductionError;

struct Product {
    factors: std::vec::IntoIter<VacuumIntegralKey>,
    terms: Terms,
}

struct Frame {
    target: VacuumIntegralKey,
    rows: std::vec::IntoIter<Row>,
    conditions: Conditions,
    output: Terms,
    product: Option<Product>,
}

impl<P: VacuumFactorProvider> VacuumProductReducer<P> {
    pub(super) fn reduce_occurrence(
        &mut self,
        target: VacuumIntegralKey,
        request: &mut Request,
    ) -> Result<CacheEntry, Error> {
        let mut stack = Vec::new();
        self.push_occurrence(target, &mut stack, request)?;
        loop {
            let frame = stack.last_mut().ok_or(ReductionError::ReducerInvariant {
                detail: "occurrence product stack is empty before completion",
            })?;
            if let Some(product) = &mut frame.product {
                if let Some(child) = product.factors.next() {
                    self.push_occurrence(child, &mut stack, request)?;
                    continue;
                }
                let product = frame
                    .product
                    .take()
                    .ok_or(ReductionError::ReducerInvariant {
                        detail: "occurrence product disappeared before combination",
                    })?;
                for (key, coefficient) in product.terms {
                    accumulate(
                        &self.context,
                        &mut frame.output,
                        key,
                        coefficient,
                        self.limits,
                        &mut request.algebra,
                        &mut request.stats,
                    )?;
                }
            }
            match frame.rows.next() {
                Some(Row::Terminal(terminal, coefficient)) => {
                    accumulate(
                        &self.context,
                        &mut frame.output,
                        vec![terminal],
                        coefficient,
                        self.limits,
                        &mut request.algebra,
                        &mut request.stats,
                    )?;
                }
                Some(Row::Product(factors, coefficient)) => {
                    frame.product = Some(Product {
                        factors: factors.into_iter(),
                        terms: BTreeMap::from([(Vec::new(), coefficient)]),
                    });
                }
                None => {
                    let completed = stack.pop().ok_or(ReductionError::ReducerInvariant {
                        detail: "occurrence frame disappeared at completion",
                    })?;
                    request.algebra.release_pending_frame()?;
                    if !request.active.remove(&completed.target) {
                        return Err(ReductionError::ReducerInvariant {
                            detail: "occurrence product target is not active",
                        }
                        .into());
                    }
                    let result = CacheEntry {
                        terms: completed.output,
                        conditions: completed.conditions.into_values(),
                    };
                    let Some(parent) = stack.last_mut() else {
                        return Ok(result);
                    };
                    parent
                        .conditions
                        .extend(&self.context, result.conditions, self.limits)?;
                    let product =
                        parent
                            .product
                            .as_mut()
                            .ok_or(ReductionError::ReducerInvariant {
                                detail: "completed child has no occurrence-local product parent",
                            })?;
                    product.terms = convolve(
                        &self.context,
                        &product.terms,
                        &result.terms,
                        self.limits,
                        &mut request.algebra,
                        &mut request.stats,
                    )?;
                }
            }
        }
    }

    fn push_occurrence(
        &mut self,
        target: VacuumIntegralKey,
        stack: &mut Vec<Frame>,
        request: &mut Request,
    ) -> Result<(), Error> {
        if !request.active.insert(target.clone()) {
            return Err(Error::InvalidInput(
                "cyclic product provider dependency".into(),
            ));
        }
        // Share the checked provider/factorization path; its child set is only
        // scheduling metadata for the memoized engine. Rows retain every
        // repeated factor occurrence used by this execution path.
        let (rows, conditions, _) = self.expand(&target, request)?;
        request.algebra.retain_pending_frame(
            self.limits
                .reduction
                .diagnostic_limit(self.limits.reduction.max_pending_frames),
        )?;
        stack
            .try_reserve(1)
            .map_err(|_| ReductionError::AllocationFailure {
                resource: "occurrence product application stack",
                requested: stack.len().saturating_add(1),
            })?;
        stack.push(Frame {
            target,
            rows: rows.into_iter(),
            conditions,
            output: BTreeMap::new(),
            product: None,
        });
        Ok(())
    }
}
