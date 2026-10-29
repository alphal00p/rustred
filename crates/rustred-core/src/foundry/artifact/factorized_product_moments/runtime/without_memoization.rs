//! Adapters for the shared occurrence-local DFS, retaining the authenticated
//! moment recurrences while omitting their completed-result tables.
use super::*;
use crate::reduction::depth_first_without_memoization::{self, DepthFirstKernel, DepthFirstStep};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MomentState {
    Numerator(NumeratorState),
    Radial(RadialState),
    Correlated(CorrelatedState),
}

struct MomentKernel<'borrow, 'program, 'artifact, 'request, 'block> {
    execution: &'borrow mut ProductExecution<'program, 'artifact, 'request>,
    block: Option<&'block CorrelatedProductBlock>,
}

impl ProductExecution<'_, '_, '_> {
    pub(super) fn reduce_numerator_without_memoization(
        &mut self,
        root: NumeratorState,
    ) -> Result<Terms, ReductionError> {
        self.reduce_without_memoization(MomentState::Numerator(root), None)
    }

    pub(super) fn reduce_radial_without_memoization(
        &mut self,
        root: RadialState,
    ) -> Result<Terms, ReductionError> {
        self.reduce_without_memoization(MomentState::Radial(root), None)
    }

    pub(super) fn reduce_correlated_without_memoization(
        &mut self,
        root: CorrelatedState,
        block: &CorrelatedProductBlock,
    ) -> Result<Terms, ReductionError> {
        self.reduce_without_memoization(MomentState::Correlated(root), Some(block))
    }

    fn reduce_without_memoization(
        &mut self,
        root: MomentState,
        block: Option<&CorrelatedProductBlock>,
    ) -> Result<Terms, ReductionError> {
        let initial = vec![(root, self.family.coefficient_context().one())];
        let conditions = self.request.take_step_nonzero_conditions();
        let output = depth_first_without_memoization::run(
            &mut MomentKernel {
                execution: self,
                block,
            },
            initial,
            conditions,
        )?;
        self.request
            .restore_step_nonzero_conditions(output.nonzero_conditions)?;
        Ok(output.terms)
    }
}

impl DepthFirstKernel for MomentKernel<'_, '_, '_, '_, '_> {
    type Key = MomentState;
    type Error = ReductionError;

    fn context(&self) -> &CoefficientContext {
        self.execution.family.coefficient_context()
    }
    fn limits(&self) -> ReductionLimits {
        self.execution.limits
    }
    fn request(&mut self) -> &mut ReductionRequest {
        self.execution.request
    }
    fn cycle_error(&self, key: &MomentState) -> ReductionError {
        ReductionError::FactorizedProductMoment {
            detail: format!(
                "a factorized moment recurrence encountered an active dependency cycle at {key:?}"
            ),
        }
    }
    fn record_work(&mut self, work: ReductionStatistics) {
        self.execution.statistics.merge_work(work);
    }

    fn step(
        &mut self,
        key: &MomentState,
        _: &mut ReductionStatistics,
    ) -> Result<DepthFirstStep<MomentState>, ReductionError> {
        let mut step = DepthFirstStep::default();
        match key {
            MomentState::Numerator(state) => {
                let measure = state.measure()?;
                if measure == 0 {
                    step.terminals = self
                        .execution
                        .reduce_monomial(&state.radial, &state.cross)?;
                } else {
                    self.execution.record_recurrence_work(1)?;
                    step.terms = self
                        .execution
                        .numerator_children(state, measure)?
                        .into_iter()
                        .map(|(child, coefficient)| (MomentState::Numerator(child), coefficient))
                        .collect();
                }
            }
            MomentState::Radial(state) => {
                if state.radial_power == 0 {
                    let target = IntegralKey::try_new([state.denominator_power])?;
                    step.terminals = self
                        .execution
                        .dependency_reducers
                        .get_mut(state.dependency_ordinal)
                        .ok_or(ReductionError::ReducerInvariant {
                            detail: "a product moment references an absent dependency reducer",
                        })?
                        .reduce_unit_mass_in_request(&target, self.execution.request)?
                        .terms()
                        .clone();
                } else {
                    self.execution.record_recurrence_work(1)?;
                    step.terms = radial_children(state)?
                        .into_iter()
                        .map(|child| (MomentState::Radial(child), self.context().one()))
                        .collect();
                }
            }
            MomentState::Correlated(state) => {
                let block = self.block.ok_or(ReductionError::ReducerInvariant {
                    detail: "a correlated occurrence has no product block",
                })?;
                if let Some(coordinate) = state.moment_powers.iter().position(|&power| power != 0) {
                    self.execution.record_recurrence_work(1)?;
                    step.terms = correlated_children(state, block, coordinate, state.measure()?)?
                        .into_iter()
                        .map(|(child, coefficient)| (MomentState::Correlated(child), coefficient))
                        .collect();
                } else {
                    let target = IntegralKey::try_new(state.denominator_powers.iter().copied())?;
                    step.terminals = self
                        .execution
                        .dependency_reducers
                        .get_mut(block.dependency_ordinal)
                        .ok_or(ReductionError::ReducerInvariant {
                            detail: "a correlated moment references an absent dependency reducer",
                        })?
                        .reduce_unit_mass_in_request(&target, self.execution.request)?
                        .terms()
                        .clone();
                }
            }
        }
        step.nonzero_conditions = self.execution.request.take_step_nonzero_conditions();
        Ok(step)
    }
}
