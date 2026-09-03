use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;

use super::super::{ModularCoefficientDag, ModularProbe, ModularProbeIdentity};
use super::{ExactLazyError, ExactLazyOwner};

/// One caller-supplied deterministic finite-field lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ExactLazyProbeSpec {
    ordinal: usize,
    modulus: u64,
    full_integer_point: Box<[i64]>,
}

impl ExactLazyProbeSpec {
    pub(super) fn new(
        ordinal: usize,
        modulus: u64,
        full_integer_point: impl Into<Box<[i64]>>,
    ) -> Self {
        Self {
            ordinal,
            modulus,
            full_integer_point: full_integer_point.into(),
        }
    }
}

#[derive(Debug)]
struct CanonicalProbeSpec {
    ordinal: usize,
    modulus: u64,
    full_integer_point: Box<[i64]>,
    identity: Arc<ModularProbeIdentity>,
}

/// Immutable owner-bound probe schedule.
///
/// Construction validates every point through the production modular probe,
/// requires increasing unique ordinals, and rejects points equivalent in the
/// actual finite field. The classifier always consumes these specs in this
/// order, so wall-clock completion order can never choose authority.
#[derive(Debug)]
pub(super) struct ExactLazyProbeSchedule {
    owner: ExactLazyOwner,
    specs: Box<[CanonicalProbeSpec]>,
}

impl ExactLazyProbeSchedule {
    pub(super) fn try_new(
        owner: &ExactLazyOwner,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        specs: impl IntoIterator<Item = ExactLazyProbeSpec>,
    ) -> Result<Self, ExactLazyError> {
        if !owner.owns_dag(dag.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if !dag.owns_context(context) {
            return Err(ExactLazyError::WrongIndexedContext);
        }
        let specs = specs.into_iter();
        let max_specs = owner.limits().support.max_probes_per_schedule;
        let lower_bound = specs.size_hint().0;
        if lower_bound > max_specs {
            return Err(ExactLazyError::ResourceLimit {
                resource: "exact-lazy probe schedule entries",
                requested: lower_bound,
                limit: max_specs,
            });
        }
        let mut canonical = Vec::new();
        canonical.try_reserve_exact(lower_bound).map_err(|_| {
            ExactLazyError::AllocationFailure {
                resource: "exact-lazy canonical probe schedule",
                requested: lower_bound,
            }
        })?;
        let mut previous_ordinal = None;
        for spec in specs {
            let requested =
                canonical
                    .len()
                    .checked_add(1)
                    .ok_or(ExactLazyError::ResourceCountOverflow {
                        resource: "exact-lazy probe schedule entries",
                    })?;
            if requested > max_specs {
                return Err(ExactLazyError::ResourceLimit {
                    resource: "exact-lazy probe schedule entries",
                    requested,
                    limit: max_specs,
                });
            }
            if previous_ordinal.is_some_and(|previous| previous >= spec.ordinal) {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "probe schedule ordinals are not strictly increasing",
                });
            }
            let probe = ModularProbe::try_new(
                dag,
                context,
                spec.ordinal,
                spec.modulus,
                &spec.full_integer_point,
                owner.limits().coefficient,
            )?;
            let identity = probe.identity_owner();
            if canonical
                .iter()
                .any(|prior: &CanonicalProbeSpec| prior.identity.residue_equivalent(&identity))
            {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "probe schedule contains residue-equivalent points",
                });
            }
            previous_ordinal = Some(spec.ordinal);
            canonical.push(CanonicalProbeSpec {
                ordinal: spec.ordinal,
                modulus: spec.modulus,
                full_integer_point: spec.full_integer_point,
                identity,
            });
        }
        Ok(Self {
            owner: owner.clone(),
            specs: canonical.into_boxed_slice(),
        })
    }

    pub(super) fn require_owner(&self, owner: &ExactLazyOwner) -> Result<(), ExactLazyError> {
        if self.owner.belongs_to(owner) && self.owner.limits() == owner.limits() {
            Ok(())
        } else {
            Err(ExactLazyError::WrongSessionOwner)
        }
    }

    pub(super) const fn len(&self) -> usize {
        self.specs.len()
    }

    pub(super) const fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    pub(super) fn specs(&self) -> impl ExactSizeIterator<Item = ProbeSpecView<'_>> {
        self.specs.iter().map(|spec| ProbeSpecView { spec })
    }
}

#[derive(Clone, Copy)]
pub(super) struct ProbeSpecView<'schedule> {
    spec: &'schedule CanonicalProbeSpec,
}

impl<'schedule> ProbeSpecView<'schedule> {
    pub(super) const fn ordinal(self) -> usize {
        self.spec.ordinal
    }

    pub(super) const fn modulus(self) -> u64 {
        self.spec.modulus
    }

    pub(super) fn full_integer_point(self) -> &'schedule [i64] {
        &self.spec.full_integer_point
    }
}
