//! Test-only structured progress snapshots for bounded release diagnostics.
//!
//! Production completion has no observer, callback, formatting, or
//! thread-local access. Ignored release tests opt in explicitly and consume a
//! single snapshot after a typed stop.

use std::cell::RefCell;

use super::janet::JanetDivisionEpoch;
use super::{InvolutiveWorkCensus, JanetBasisEpoch, JanetInitialReductionCensus};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JanetDiagnosticPhase {
    ChartLift,
    InitialPreprocessing,
    InitialAutoreduction,
    Completion,
    CompletionAutoreduction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticBasis {
    pub(crate) rows: usize,
    pub(crate) revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JanetDiagnosticCheckpoint {
    pub(crate) phase: JanetDiagnosticPhase,
    pub(crate) lifted_rows: usize,
    pub(crate) initial_reduction: Option<JanetInitialReductionCensus>,
    pub(crate) initial_basis: Option<JanetDiagnosticBasis>,
    pub(crate) current_basis: Option<JanetDiagnosticBasis>,
    pub(crate) autoreduction_pass: Option<usize>,
    /// Exact at a work-ledger cap; otherwise the most recent coarse phase,
    /// pass, or epoch checkpoint.
    pub(crate) work_at_last_checkpoint: InvolutiveWorkCensus,
}

thread_local! {
    static CHECKPOINT: RefCell<Option<JanetDiagnosticCheckpoint>> = const { RefCell::new(None) };
}

pub(crate) fn begin() {
    CHECKPOINT.with(|slot| {
        *slot.borrow_mut() = Some(JanetDiagnosticCheckpoint {
            phase: JanetDiagnosticPhase::ChartLift,
            lifted_rows: 0,
            initial_reduction: None,
            initial_basis: None,
            current_basis: None,
            autoreduction_pass: None,
            work_at_last_checkpoint: InvolutiveWorkCensus::default(),
        });
    });
}

pub(crate) fn take() -> Option<JanetDiagnosticCheckpoint> {
    CHECKPOINT.with(|slot| slot.borrow_mut().take())
}

pub(crate) fn record_lifted_rows(rows: usize) {
    update(|checkpoint| {
        checkpoint.phase = JanetDiagnosticPhase::InitialPreprocessing;
        checkpoint.lifted_rows = rows;
    });
}

pub(super) fn record_initial_basis(
    census: JanetInitialReductionCensus,
    epoch: &JanetBasisEpoch,
    work: InvolutiveWorkCensus,
) {
    let basis = basis(epoch);
    update(|checkpoint| {
        checkpoint.phase = JanetDiagnosticPhase::InitialAutoreduction;
        checkpoint.initial_reduction = Some(census);
        checkpoint.initial_basis = Some(basis);
        checkpoint.current_basis = Some(basis);
        checkpoint.autoreduction_pass = None;
        checkpoint.work_at_last_checkpoint = work;
    });
}

pub(super) fn record_autoreduction_division_pass(
    epoch: &JanetDivisionEpoch,
    pass: usize,
    work: InvolutiveWorkCensus,
) {
    update(|checkpoint| {
        checkpoint.current_basis = Some(division_basis(epoch));
        checkpoint.autoreduction_pass = Some(pass);
        checkpoint.work_at_last_checkpoint = work;
    });
}

pub(super) fn record_completion_epoch(epoch: &JanetBasisEpoch, work: InvolutiveWorkCensus) {
    update(|checkpoint| {
        checkpoint.phase = JanetDiagnosticPhase::Completion;
        checkpoint.current_basis = Some(basis(epoch));
        checkpoint.autoreduction_pass = None;
        checkpoint.work_at_last_checkpoint = work;
    });
}

pub(super) fn record_completion_division_autoreduction(
    epoch: &JanetDivisionEpoch,
    work: InvolutiveWorkCensus,
) {
    update(|checkpoint| {
        checkpoint.phase = JanetDiagnosticPhase::CompletionAutoreduction;
        checkpoint.current_basis = Some(division_basis(epoch));
        checkpoint.autoreduction_pass = None;
        checkpoint.work_at_last_checkpoint = work;
    });
}

pub(super) fn record_work_at_typed_stop(work: InvolutiveWorkCensus) {
    update(|checkpoint| checkpoint.work_at_last_checkpoint = work);
}

fn basis(epoch: &JanetBasisEpoch) -> JanetDiagnosticBasis {
    division_basis(epoch.division())
}

fn division_basis(epoch: &JanetDivisionEpoch) -> JanetDiagnosticBasis {
    JanetDiagnosticBasis {
        rows: epoch.elements().len(),
        revision: epoch.epoch().revision(),
    }
}

fn update(mut operation: impl FnMut(&mut JanetDiagnosticCheckpoint)) {
    CHECKPOINT.with(|slot| {
        if let Some(checkpoint) = slot.borrow_mut().as_mut() {
            operation(checkpoint);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opt_in_checkpoint_is_thread_local_consumable_and_resettable() {
        assert_eq!(take(), None);
        begin();
        record_lifted_rows(9);
        let checkpoint = take().expect("an opted-in checkpoint must exist");
        assert_eq!(checkpoint.phase, JanetDiagnosticPhase::InitialPreprocessing);
        assert_eq!(checkpoint.lifted_rows, 9);
        assert_eq!(take(), None);

        begin();
        assert_eq!(take().unwrap().phase, JanetDiagnosticPhase::ChartLift);
        assert_eq!(take(), None);
    }
}
