use std::mem::size_of;
use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::sector::ShiftComplexityKey;

use super::super::janet::JanetDivisionEpoch;
use super::super::limits::InvolutiveWorkBudget;
use super::super::{
    EpochId, ForwardShift, InvolutiveError, InvolutiveLimits, OreActionIdentity, OreConsequence,
    OreOrderingAdapter,
};
use super::error::{checked_add, checked_mul, reserve_vec};
use super::model::DagOwner;
use super::ore::{ModularOreRow, ModularOreTerm, SampledSupport};
use super::work::{ModularNormalFormCensus, ModularNormalFormWork};
use super::{
    CoeffRef, ModularCoefficientDag, ModularGuideError, ModularGuideLimits, ModularProbe,
    ModularProbeIdentity,
};

/// One immutable reduction decision in a proposal-only modular trace.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct ModularReductionTraceStep {
    target_shift: ForwardShift,
    divisor_ordinal: usize,
    divisor_leading_shift: ForwardShift,
    operator_shift: ForwardShift,
}

impl ModularReductionTraceStep {
    pub(super) fn target_shift(&self) -> &ForwardShift {
        &self.target_shift
    }

    pub(super) const fn divisor_ordinal(&self) -> usize {
        self.divisor_ordinal
    }

    pub(super) fn divisor_leading_shift(&self) -> &ForwardShift {
        &self.divisor_leading_shift
    }

    pub(super) fn operator_shift(&self) -> &ForwardShift {
        &self.operator_shift
    }
}

/// Residue-free identity of one sampled modular normal-form path.
///
/// Its supports and decisions are determined by a finite-field lane even
/// though scalar residues are stored separately. Equality is suitable only
/// for grouping independent probes of this same frozen problem before exact
/// replay. It has no relation, exact-absence, or certificate API.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct ModularNormalFormTraceIdentity {
    excluded_divisor: Option<usize>,
    sampled_start_leader: Option<ForwardShift>,
    sampled_start_support: Box<[ForwardShift]>,
    steps: Box<[ModularReductionTraceStep]>,
    sampled_remainder_leader: Option<ForwardShift>,
    sampled_remainder_support: Box<[ForwardShift]>,
}

impl ModularNormalFormTraceIdentity {
    pub(super) const fn excluded_divisor(&self) -> Option<usize> {
        self.excluded_divisor
    }

    /// Greatest term nonzero in this probe's sampled start image.
    pub(super) fn sampled_start_leader(&self) -> Option<&ForwardShift> {
        self.sampled_start_leader.as_ref()
    }

    /// Start support of this probe image, not the exact structural row.
    pub(super) fn sampled_start_support(&self) -> &[ForwardShift] {
        &self.sampled_start_support
    }

    /// Structural cancellation choices made by this sampled lane.
    pub(super) fn steps(&self) -> &[ModularReductionTraceStep] {
        &self.steps
    }

    /// Sampled support is proposal data, never an exact zero decision.
    pub(super) fn sampled_remainder_leader(&self) -> Option<&ForwardShift> {
        self.sampled_remainder_leader.as_ref()
    }

    /// An empty sampled remainder is not a zero certificate.
    pub(super) fn sampled_remainder_support(&self) -> &[ForwardShift] {
        &self.sampled_remainder_support
    }
}

/// One-sided nonzero observations aligned to a structural trace.
///
/// A later exact-lazy scheduler may replay the exact coefficient at the same
/// trace location and verify its denominator plus this residue. The values are
/// deliberately excluded from [`ModularNormalFormTraceIdentity`] and cannot
/// publish a consequence by themselves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ModularNonzeroEvidence {
    sampled_start_support_residues: Box<[u64]>,
    step_target_residues: Box<[u64]>,
    sampled_remainder_residues: Box<[u64]>,
}

impl ModularNonzeroEvidence {
    pub(super) fn sampled_start_support_residues(&self) -> &[u64] {
        &self.sampled_start_support_residues
    }

    pub(super) fn step_target_residues(&self) -> &[u64] {
        &self.step_target_residues
    }

    pub(super) fn sampled_remainder_residues(&self) -> &[u64] {
        &self.sampled_remainder_residues
    }
}

/// Complete successful output of one independent modular lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ModularNormalFormProposal {
    dag_owner: DagOwner,
    epoch: EpochId,
    action: OreActionIdentity,
    context_fingerprint: Arc<String>,
    probe: Arc<ModularProbeIdentity>,
    trace: ModularNormalFormTraceIdentity,
    nonzero_evidence: ModularNonzeroEvidence,
    census: ModularNormalFormCensus,
}

impl ModularNormalFormProposal {
    pub(super) fn probe(&self) -> &ModularProbeIdentity {
        &self.probe
    }

    pub(super) fn trace(&self) -> &ModularNormalFormTraceIdentity {
        &self.trace
    }

    pub(super) fn nonzero_evidence(&self) -> &ModularNonzeroEvidence {
        &self.nonzero_evidence
    }

    pub(super) const fn census(&self) -> ModularNormalFormCensus {
        self.census
    }
}

#[derive(Debug)]
struct FrozenBasisElement {
    leading_shift: ForwardShift,
    row: ModularOreRow,
}

/// Reusable field-independent image of one exact subject and frozen Janet
/// division epoch.
///
/// Exact leaves and row shapes are prepared once. Every call to
/// [`Self::try_probe`] owns a fresh finite field/cache and a transactional DAG
/// suffix that is unconditionally rolled back, so probe paths cannot affect
/// one another.
#[derive(Debug)]
pub(super) struct ModularFrozenNormalFormProblem<'epoch> {
    division: &'epoch JanetDivisionEpoch,
    dag: ModularCoefficientDag,
    context: IndexedCoefficientContext,
    ordering: OreOrderingAdapter,
    exact_limits: InvolutiveLimits,
    limits: ModularGuideLimits,
    epoch: EpochId,
    action: OreActionIdentity,
    excluded_divisor: Option<usize>,
    basis: Box<[FrozenBasisElement]>,
    subject: ModularOreRow,
    problem_row_terms: usize,
    problem_guard_references: usize,
}

impl<'epoch> ModularFrozenNormalFormProblem<'epoch> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_new(
        subject: &OreConsequence,
        division: &'epoch JanetDivisionEpoch,
        excluded_divisor: Option<usize>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        exact_limits: InvolutiveLimits,
        limits: ModularGuideLimits,
    ) -> Result<Self, ModularGuideError> {
        division.require_ordering(ordering)?;
        subject.try_validate(ordering, context, exact_limits)?;
        ordering.require_arity("modular frozen Janet epoch", division.arity())?;
        if let Some(ordinal) = excluded_divisor {
            if ordinal >= division.elements().len() {
                return Err(ModularGuideError::InvalidExcludedDivisor {
                    ordinal,
                    basis_rows: division.elements().len(),
                });
            }
        }

        let mut problem_row_terms = subject.row().terms().len();
        let mut problem_guard_references = subject.required_nonzero_guards().len();
        for element in division.elements() {
            problem_row_terms = checked_add(
                "modular normal-form problem row terms",
                problem_row_terms,
                element.consequence().row().terms().len(),
            )?;
            problem_guard_references = checked_add(
                "modular normal-form problem guard references",
                problem_guard_references,
                element.consequence().required_nonzero_guards().len(),
            )?;
        }
        let mut admission = ModularNormalFormWork::default();
        admission.admit_problem(
            division.elements().len(),
            problem_row_terms,
            problem_guard_references,
            limits,
        )?;

        let mut dag = ModularCoefficientDag::try_new(context, limits)?;
        let mut basis = Vec::new();
        reserve_vec(
            &mut basis,
            division.elements().len(),
            "modular frozen Janet basis rows",
        )?;
        for (position, element) in division.elements().iter().enumerate() {
            if element.ordinal() != position {
                return Err(ModularGuideError::Invariant {
                    detail: "a frozen Janet basis ordinal is not its array position",
                });
            }
            let leading_coefficient = element
                .consequence()
                .row()
                .coefficient(element.leading_shift())
                .ok_or(ModularGuideError::Invariant {
                    detail: "a frozen Janet basis leader is absent from its exact row",
                })?;
            if leading_coefficient != &context.one() {
                return Err(ModularGuideError::Invariant {
                    detail: "a frozen Janet basis row is not exactly monic",
                });
            }
            basis.push(FrozenBasisElement {
                leading_shift: element.leading_shift().clone(),
                row: ModularOreRow::try_from_exact(
                    element.consequence(),
                    &mut dag,
                    context,
                    &mut admission,
                    limits,
                )?,
            });
        }
        let subject =
            ModularOreRow::try_from_exact(subject, &mut dag, context, &mut admission, limits)?;
        Ok(Self {
            division,
            dag,
            context: context.clone(),
            ordering: ordering.clone(),
            exact_limits,
            limits,
            epoch: division.epoch().clone(),
            action: ordering.identity().clone(),
            excluded_divisor,
            basis: basis.into_boxed_slice(),
            subject,
            problem_row_terms,
            problem_guard_references,
        })
    }

    pub(super) fn owns(&self, proposal: &ModularNormalFormProposal) -> bool {
        self.dag.owner().belongs_to(&proposal.dag_owner)
            && self.epoch == proposal.epoch
            && self.action == proposal.action
            && self.context.owns_fingerprint(&proposal.context_fingerprint)
    }

    pub(super) fn try_probe(
        &mut self,
        probe_ordinal: usize,
        modulus: u64,
        full_integer_point: &[i64],
    ) -> Result<ModularNormalFormProposal, ModularGuideError> {
        let checkpoint = self.dag.checkpoint();
        let result = self.try_probe_inner(probe_ordinal, modulus, full_integer_point);
        self.dag.try_rollback(checkpoint)?;
        result
    }

    fn try_probe_inner(
        &mut self,
        probe_ordinal: usize,
        modulus: u64,
        full_integer_point: &[i64],
    ) -> Result<ModularNormalFormProposal, ModularGuideError> {
        let mut work = ModularNormalFormWork::default();
        work.admit_problem(
            self.basis.len(),
            self.problem_row_terms,
            self.problem_guard_references,
            self.limits,
        )?;
        let mut probe = ModularProbe::try_new(
            &self.dag,
            &self.context,
            probe_ordinal,
            modulus,
            full_integer_point,
            self.limits,
        )?;
        let mut divisor_scratch = self.division.try_divisor_scratch(self.exact_limits)?;
        let mut divisor_work = InvolutiveWorkBudget::default();
        let mut divisor_limits = self.exact_limits;
        divisor_limits.max_divisor_index_query_operations = divisor_limits
            .max_divisor_index_query_operations
            .min(self.limits.max_divisor_index_query_operations);
        charge_proposal_header(&mut work, self.limits)?;

        for element in &self.basis {
            work.observe_live_row(
                element.row.terms().len(),
                element.row.guards().len(),
                self.limits,
            )?;
            element.row.try_require_guards(&self.dag, &mut probe)?;
            if element.row.coefficient(&element.leading_shift) != Some(&self.dag.one()) {
                return Err(ModularGuideError::Invariant {
                    detail: "an imported frozen Janet basis leader is not structurally one",
                });
            }
        }

        let mut subject = self.subject.try_copy(&mut work, self.limits)?;
        subject.try_require_guards(&self.dag, &mut probe)?;
        let mut support =
            subject.try_sampled_support(&self.dag, &mut probe, &mut work, self.limits)?;
        let sampled_start_leader = sampled_leader(&support, &subject, &self.ordering)?;
        charge_support_trace(
            &mut work,
            support.len(),
            sampled_start_leader.is_some(),
            self.ordering.arity(),
            self.limits,
        )?;
        let sampled_start_support = support.try_shifts(&subject)?;
        let sampled_start_residues = clone_residues(support.residues())?;

        let mut steps = Vec::new();
        let mut step_target_residues = Vec::new();
        let mut previous_target: Option<ShiftComplexityKey> = None;
        loop {
            let selected = select_reduction(
                &support,
                &subject,
                self.division,
                self.excluded_divisor,
                &self.ordering,
                &mut divisor_scratch,
                divisor_limits,
                &mut divisor_work,
                &mut work,
                self.limits,
            )?;
            let Some(selected) = selected else {
                break;
            };
            if previous_target
                .as_ref()
                .is_some_and(|previous| selected.target_key >= *previous)
            {
                return Err(ModularGuideError::Invariant {
                    detail: "modular Janet reduction target did not strictly decrease",
                });
            }
            let frozen =
                self.basis
                    .get(selected.divisor_ordinal)
                    .ok_or(ModularGuideError::Invariant {
                        detail: "a selected modular Janet divisor disappeared",
                    })?;
            let divisor = &frozen.row;
            let operator_shift = selected
                .target_shift
                .try_checked_sub(&frozen.leading_shift, self.exact_limits)?;
            let trace_step = ModularReductionTraceStep {
                target_shift: selected.target_shift.clone(),
                divisor_ordinal: selected.divisor_ordinal,
                divisor_leading_shift: frozen.leading_shift.clone(),
                operator_shift: operator_shift.clone(),
            };
            charge_step_trace(&mut work, &trace_step, self.ordering.arity(), self.limits)?;
            steps
                .try_reserve(1)
                .map_err(|_| ModularGuideError::AllocationFailure {
                    resource: "modular normal-form trace steps",
                    requested: steps.len().saturating_add(1),
                })?;
            step_target_residues.try_reserve(1).map_err(|_| {
                ModularGuideError::AllocationFailure {
                    resource: "modular normal-form target residues",
                    requested: step_target_residues.len().saturating_add(1),
                }
            })?;

            work.charge_normal_form_step(self.limits)?;
            let multiplier = self.dag.try_neg(&selected.target_coefficient)?;
            subject = subject.try_left_axpy(
                &multiplier,
                &operator_shift,
                divisor,
                &self.ordering,
                &mut self.dag,
                &mut probe,
                self.exact_limits,
                &mut work,
                self.limits,
            )?;
            if subject.contains_shift(&selected.target_shift) {
                return Err(ModularGuideError::Invariant {
                    detail: "modular monic AXPY did not structurally cancel its target",
                });
            }
            steps.push(trace_step);
            step_target_residues.push(selected.target_residue);
            previous_target = Some(selected.target_key);
            support = subject.try_sampled_support(&self.dag, &mut probe, &mut work, self.limits)?;
        }

        let sampled_remainder_leader = sampled_leader(&support, &subject, &self.ordering)?;
        charge_support_trace(
            &mut work,
            support.len(),
            sampled_remainder_leader.is_some(),
            self.ordering.arity(),
            self.limits,
        )?;
        let sampled_remainder_support = support.try_shifts(&subject)?;
        let sampled_remainder_residues = clone_residues(support.residues())?;

        let identity = probe.identity_owner();
        let probe_census = probe.census();
        let census = work.finish(
            divisor_work.census().divisor_index_query_operations(),
            self.dag.node_count(),
            self.dag.physical_delta_count(),
            probe_census,
        );
        Ok(ModularNormalFormProposal {
            dag_owner: self.dag.owner().clone(),
            epoch: self.epoch.clone(),
            action: self.action.clone(),
            context_fingerprint: self.context.fingerprint_owner(),
            probe: identity,
            trace: ModularNormalFormTraceIdentity {
                excluded_divisor: self.excluded_divisor,
                sampled_start_leader,
                sampled_start_support,
                steps: steps.into_boxed_slice(),
                sampled_remainder_leader,
                sampled_remainder_support,
            },
            nonzero_evidence: ModularNonzeroEvidence {
                sampled_start_support_residues: sampled_start_residues,
                step_target_residues: step_target_residues.into_boxed_slice(),
                sampled_remainder_residues,
            },
            census,
        })
    }
}

struct SelectedReduction {
    target_shift: ForwardShift,
    target_coefficient: CoeffRef,
    target_residue: u64,
    target_key: ShiftComplexityKey,
    divisor_ordinal: usize,
}

#[allow(clippy::too_many_arguments)]
fn select_reduction(
    support: &SampledSupport,
    subject: &ModularOreRow,
    division: &JanetDivisionEpoch,
    excluded_divisor: Option<usize>,
    ordering: &OreOrderingAdapter,
    divisor_scratch: &mut super::super::divisor_index::JanetDivisorScratch,
    divisor_limits: InvolutiveLimits,
    divisor_work: &mut InvolutiveWorkBudget,
    work: &mut ModularNormalFormWork,
    limits: ModularGuideLimits,
) -> Result<Option<SelectedReduction>, ModularGuideError> {
    let mut selected: Option<SelectedReduction> = None;
    for (term, residue) in support.entries(subject) {
        let divisor_ordinal = division
            .try_janet_divisor_with_scratch(
                term.shift(),
                excluded_divisor,
                divisor_scratch,
                divisor_limits,
                divisor_work,
            )
            .map_err(map_divisor_query_error)?;
        let logical_visits = match divisor_ordinal {
            Some(ordinal) => checked_add("modular normal-form divisor visits", ordinal, 1)?,
            None => division.elements().len(),
        };
        work.charge_divisor_visits(logical_visits, limits)?;
        let Some(divisor_ordinal) = divisor_ordinal else {
            continue;
        };
        let target_key = ordering.try_key(term.shift())?;
        if selected
            .as_ref()
            .is_none_or(|current| target_key > current.target_key)
        {
            selected = Some(SelectedReduction {
                target_shift: term.shift().clone(),
                target_coefficient: term.coefficient().clone(),
                target_residue: residue,
                target_key,
                divisor_ordinal,
            });
        }
    }
    Ok(selected)
}

fn map_divisor_query_error(error: InvolutiveError) -> ModularGuideError {
    match error {
        InvolutiveError::ResourceLimit {
            resource,
            requested,
            limit,
        } if resource == "Janet divisor index query operations" => {
            ModularGuideError::ResourceLimit {
                resource: "modular Janet divisor-index query operations",
                requested,
                limit,
            }
        }
        other => ModularGuideError::Involutive(other),
    }
}

fn sampled_leader(
    support: &SampledSupport,
    row: &ModularOreRow,
    ordering: &OreOrderingAdapter,
) -> Result<Option<ForwardShift>, ModularGuideError> {
    let mut leading: Option<(&ModularOreTerm, ShiftComplexityKey)> = None;
    for term in support.iter(row) {
        let key = ordering.try_key(term.shift())?;
        if leading.as_ref().is_none_or(|(_, current)| key > *current) {
            leading = Some((term, key));
        }
    }
    Ok(leading.map(|(term, _)| term.shift().clone()))
}

fn clone_residues(values: &[u64]) -> Result<Box<[u64]>, ModularGuideError> {
    let mut result = Vec::new();
    reserve_vec(
        &mut result,
        values.len(),
        "modular nonzero-evidence residues",
    )?;
    result.extend_from_slice(values);
    Ok(result.into_boxed_slice())
}

fn charge_support_trace(
    work: &mut ModularNormalFormWork,
    support_len: usize,
    has_leader: bool,
    arity: usize,
    limits: ModularGuideLimits,
) -> Result<(), ModularGuideError> {
    let retained_shifts = checked_add(
        "modular normal-form trace support shifts",
        support_len,
        usize::from(has_leader),
    )?;
    let cells = checked_mul(
        "modular normal-form trace shift coordinate cells",
        retained_shifts,
        arity,
    )?;
    let shift_bytes = checked_mul(
        "modular normal-form trace bytes",
        retained_shifts,
        checked_add(
            "modular normal-form trace bytes",
            size_of::<ForwardShift>(),
            checked_mul("modular normal-form trace bytes", arity, size_of::<u64>())?,
        )?,
    )?;
    let residue_bytes = checked_mul(
        "modular normal-form trace bytes",
        support_len,
        size_of::<u64>(),
    )?;
    work.charge_trace(
        0,
        cells,
        checked_add(
            "modular normal-form trace bytes",
            shift_bytes,
            residue_bytes,
        )?,
        limits,
    )
}

fn charge_step_trace(
    work: &mut ModularNormalFormWork,
    _step: &ModularReductionTraceStep,
    arity: usize,
    limits: ModularGuideLimits,
) -> Result<(), ModularGuideError> {
    let cells = checked_mul("modular normal-form trace shift coordinate cells", 3, arity)?;
    let coordinate_bytes = checked_mul("modular normal-form trace bytes", cells, size_of::<u64>())?;
    let bytes = checked_add(
        "modular normal-form trace bytes",
        size_of::<ModularReductionTraceStep>(),
        coordinate_bytes,
    )?;
    work.charge_trace(
        1,
        cells,
        checked_add("modular normal-form trace bytes", bytes, size_of::<u64>())?,
        limits,
    )
}

fn charge_proposal_header(
    work: &mut ModularNormalFormWork,
    limits: ModularGuideLimits,
) -> Result<(), ModularGuideError> {
    work.charge_trace(0, 0, size_of::<ModularNormalFormProposal>(), limits)
}
