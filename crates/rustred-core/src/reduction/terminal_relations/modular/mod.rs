//! Opt-in finite-field discovery with exact source-circuit publication.
//!
//! Sampled dependence is only a search heuristic. The exact source cursor is
//! never advanced, and exhausting this separate worklist proves no rank,
//! completeness or minimality statement. Symbolica owns all field arithmetic,
//! sparse elimination, reconstruction and full-column product validation.

use super::*;
use crate::algebra::{ExactAlgebraLimits, IndexedCoefficientContext, IndexedContextLimits};
use crate::identity::IndexShift;
use crate::solver::{
    ProjectedSourceWeightEvent, ProjectedSourceWeightLimits, SectorExecutor,
    reconstruct_projected_source_weights,
};
use rayon::prelude::*;
use std::collections::HashSet;
use symbolica::domains::SelfRing;

mod codec;
mod discovery;
mod labels;
#[cfg(test)]
mod tests;
mod validation;

/// Caller-owned operational budgets for one step/frame. Defaults impose no
/// resource quotas beyond native integer/exponent representation bounds.
/// Batch and candidate windows schedule resumable work, never cap the search.
#[derive(Clone, Copy, Debug)]
pub struct TerminalRelationModularOptions {
    /// Maximum additional ordinary-source definitions prepared in one step.
    /// Retained discovery rows and the finite source inventory have no new
    /// total-row ceiling here; the caller owns process/RAM admission.
    pub batch_rows: usize,
    /// Caller-selected retries of an unlucky discovery image, using fresh
    /// generic points. Defaults to the usize representation bound.
    pub discovery_samples: usize,
    /// Maximum lifting attempts in one step; pending proposals are resumable.
    pub max_candidates: usize,
    pub reconstruction: ProjectedSourceWeightLimits,
}
impl Default for TerminalRelationModularOptions {
    fn default() -> Self {
        Self {
            batch_rows: 64,
            discovery_samples: usize::MAX,
            max_candidates: 8,
            reconstruction: ProjectedSourceWeightLimits {
                arithmetic: ExactAlgebraLimits::unbounded_resources(),
                max_rows: usize::MAX,
                max_columns: usize::MAX,
                max_input_nonzeros: usize::MAX,
                max_coefficient_terms: usize::MAX,
                // Symbolica's public reconstruction API requires doubled
                // degrees to fit u16 and rejects any larger allowance.
                max_degree: u16::MAX / 2,
                max_probes: usize::MAX,
                max_attempts: usize::MAX,
                max_primes: usize::MAX,
                max_cached_images: usize::MAX,
                max_cached_values: usize::MAX,
                max_weight_slots: usize::MAX,
            },
        }
    }
}
impl TerminalRelationModularOptions {
    fn validate(self) -> Result<(), TerminalRelationError> {
        let r = self.reconstruction;
        if r.max_degree > u16::MAX / 2 {
            return Err(invalid(
                "Symbolica reconstruction requires max_degree <= u16::MAX/2 so doubled degrees fit its native exponent representation",
            ));
        }
        if self.batch_rows == 0
            || self.discovery_samples == 0
            || self.max_candidates == 0
            || r.max_rows == 0
            || r.max_columns == 0
            || r.max_input_nonzeros == 0
            || r.max_coefficient_terms == 0
            || r.max_degree == 0
            || r.max_probes == 0
            || r.max_attempts == 0
            || r.max_primes < 2
            || r.max_cached_images == 0
            || r.max_cached_values == 0
            || r.max_weight_slots == 0
        {
            return Err(invalid(
                "modular budgets must be positive; at least two lifting primes required",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalRelationModularStats {
    /// Ordinary rows scanned after the frozen exact cursor; excludes baseline U.
    pub source_rows_scanned: u64,
    /// Finite ordinary-source suffix available at construction.
    pub total_source_rows: u64,
    /// Rank of the current sampled image, never characteristic-zero authority.
    pub modular_independent_rows: usize,
    pub candidates_found: usize,
    pub pending_candidates: usize,
    pub accepted_relations: usize,
    pub reconstruction_misses: usize,
    /// Finite heuristic source inventory exhausted; never an exact rank claim.
    pub search_exhausted: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalRelationModularProgress {
    pub stage: &'static str,
    pub statistics: TerminalRelationModularStats,
}

/// Stable definition of a circuit input. Inherited rows refer to the frozen
/// prefix of the exact checkpoint's independent basis, under its saved guards.
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub enum TerminalRelationModularSource {
    InheritedBasis {
        ordinal: usize,
    },
    Ordinary {
        seed_ordinal: usize,
        source_ordinal: usize,
    },
}

/// A reconstruction miss is retained as an explicit incomplete proposal. It
/// says nothing about existence of a characteristic-zero relation.
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub enum TerminalRelationModularProposalStatus {
    Pending,
    Rejected { reason: String },
    Accepted { circuit: usize },
}

#[derive(Clone, Debug)]
pub struct TerminalRelationModularCircuit {
    /// Full discovery-input prefix whose domain this circuit preserves.
    pub scanned_sources: usize,
    /// Frozen reconstructed prefix of the selected source frame, including
    /// reconstructed zero slots. Post-hit selected rows and all other discovery
    /// sources have structurally fixed zero weights; their guards remain saved.
    pub sources: Vec<TerminalRelationModularSource>,
    /// Sparse slots in `sources`, not global seed/source ordinals. Every slot
    /// of this selected frame, including zero, was natively reconstructed.
    /// All unselected discovery-source weights are structurally fixed to zero.
    pub weights: BTreeMap<usize, Coefficient>,
    pub image: TerminalRelationRow,
    pub nonzero_conditions: Vec<Coefficient>,
}

#[derive(Clone)]
struct SourceRow {
    identity: TerminalRelationModularSource,
    terms: TerminalRelationRow,
    conditions: Vec<Coefficient>,
}
struct Candidate {
    target: IntegralKey,
    sources: Vec<usize>,
    status: TerminalRelationModularProposalStatus,
}

/// Separate resumable discovery state. Only exactly validated terminal images
/// enter `session`; its original ordinary-source cursor remains unchanged.
pub struct TerminalRelationModularSession {
    /// Original exact authority, before any new modular circuit was admitted.
    /// New maps are always replayed from this immutable baseline on cold load.
    baseline: Vec<u8>,
    session: TerminalRelationSession,
    options: TerminalRelationModularOptions,
    initial_basis_rows: usize,
    start_ordinal: usize,
    next_ordinal: usize,
    rows: Vec<SourceRow>,
    image: discovery::DiscoveryImage,
    candidates: Vec<Candidate>,
    circuits: Vec<TerminalRelationModularCircuit>,
    reconstruction_misses: usize,
    /// Transient bounded execution policy, never checkpoint mathematics.
    executor: SectorExecutor,
}

impl TerminalRelationModularSession {
    /// Freeze the existing exact publication and initialize native discovery
    /// from its already-proved basis, without implicit native transport quotas.
    pub fn new(
        session: TerminalRelationSession,
        options: TerminalRelationModularOptions,
    ) -> Result<Self, TerminalRelationError> {
        Self::new_with_io(session, options, BinaryIoLimits::unbounded_resources())
    }

    /// As [`Self::new`], with caller-owned native baseline transport budgets.
    pub fn new_with_io(
        session: TerminalRelationSession,
        options: TerminalRelationModularOptions,
        io: BinaryIoLimits,
    ) -> Result<Self, TerminalRelationError> {
        options.validate()?;
        if session.assistance.is_some() {
            return Err(invalid(
                "modular circuits currently require ordinary unassisted sources",
            ));
        }
        if session.rebuild_cursor != session.rebuild.len() {
            return Err(invalid(
                "finish the exact row-space rebuild before modular discovery",
            ));
        }
        let start_ordinal = session
            .seed_cursor
            .checked_mul(session.sources.len())
            .and_then(|n| n.checked_add(session.source_cursor))
            .ok_or_else(|| invalid("modular source cursor overflow"))?;
        let rows: Vec<_> = session
            .basis_rows()
            .into_iter()
            .enumerate()
            .map(|(ordinal, terms)| SourceRow {
                identity: TerminalRelationModularSource::InheritedBasis { ordinal },
                terms,
                conditions: Vec::new(),
            })
            .collect();
        let image = discovery::DiscoveryImage::new(&session, 0)?;
        let baseline = session.to_native_bytes(io)?;
        let mut result = Self {
            baseline,
            initial_basis_rows: rows.len(),
            session,
            options,
            start_ordinal,
            next_ordinal: start_ordinal,
            rows,
            image,
            candidates: Vec::new(),
            circuits: Vec::new(),
            reconstruction_misses: 0,
            executor: SectorExecutor::new(1).map_err(invalid)?,
        };
        result.rebuild_image()?;
        // Existing proved terminal pivots are already represented by session.
        result.candidates.clear();
        Ok(result)
    }

    pub fn session(&self) -> &TerminalRelationSession {
        &self.session
    }
    pub fn into_session(self) -> TerminalRelationSession {
        self.session
    }
    pub fn options(&self) -> TerminalRelationModularOptions {
        self.options
    }
    /// Set the transient bounded pool used for independent circuit lifts and
    /// nested native Rayon work. This setting does not change checkpoint math.
    pub fn set_workers(&mut self, workers: usize) -> Result<(), TerminalRelationError> {
        self.executor = SectorExecutor::new(workers).map_err(invalid)?;
        Ok(())
    }
    pub fn workers(&self) -> usize {
        self.executor.workers()
    }
    pub fn set_max_columns(&mut self, maximum: Option<usize>) -> Result<(), TerminalRelationError> {
        check(
            "modular columns",
            self.image.columns.len(),
            maximum.unwrap_or(usize::MAX),
        )?;
        self.session.set_max_columns(maximum)
    }
    /// Changing operational reconstruction budgets retries incomplete proposals.
    pub fn set_options(
        &mut self,
        options: TerminalRelationModularOptions,
    ) -> Result<(), TerminalRelationError> {
        options.validate()?;
        self.options = options;
        self.retry_rejected();
        Ok(())
    }
    /// Retry incomplete/policy-rejected proposals; accepted maps remain proved.
    pub fn retry_rejected(&mut self) {
        for candidate in &mut self.candidates {
            if matches!(
                candidate.status,
                TerminalRelationModularProposalStatus::Rejected { .. }
            ) {
                candidate.status = TerminalRelationModularProposalStatus::Pending;
            }
        }
    }
    pub fn circuits(&self) -> &[TerminalRelationModularCircuit] {
        &self.circuits
    }
    pub fn proposals(
        &self,
    ) -> impl Iterator<Item = (&IntegralKey, &TerminalRelationModularProposalStatus)> {
        self.candidates.iter().map(|c| (&c.target, &c.status))
    }
    pub fn statistics(&self) -> TerminalRelationModularStats {
        TerminalRelationModularStats {
            source_rows_scanned: (self.next_ordinal - self.start_ordinal) as u64,
            total_source_rows: (self.session.seeds.len() * self.session.sources.len()
                - self.start_ordinal) as u64,
            modular_independent_rows: self.image.accepted_sources.len(),
            candidates_found: self.candidates.len(),
            pending_candidates: self
                .candidates
                .iter()
                .filter(|c| matches!(c.status, TerminalRelationModularProposalStatus::Pending))
                .count(),
            accepted_relations: self.circuits.len(),
            reconstruction_misses: self.reconstruction_misses,
            search_exhausted: self.next_ordinal
                == self.session.seeds.len() * self.session.sources.len(),
        }
    }
    /// True when the finite heuristic scan has drained and every proposal has
    /// been attempted. Rejected proposals remain recorded; this does not prove
    /// exact source-span exhaustion, absence of relations, or master minimality.
    pub fn is_search_complete(&self) -> bool {
        let stats = self.statistics();
        stats.search_exhausted && stats.pending_candidates == 0
    }
    /// Advance at most `batch_rows` ordinary definitions and attempt at most
    /// `max_candidates` pending lifts. These are per-call scheduling windows,
    /// not total search limits. Cancellation is honored between native rows
    /// and before lifting tasks; all started tasks drain before exact commit.
    pub fn step(
        &mut self,
        cancel: &AtomicBool,
    ) -> Result<TerminalRelationModularStats, TerminalRelationError> {
        self.step_observed(cancel, |_| {})
    }
    /// As [`Self::step`], with diagnostics on the caller's coordinator thread.
    /// Worker callbacks cannot publish maps or mutate mathematical authority.
    pub fn step_observed(
        &mut self,
        cancel: &AtomicBool,
        mut observe: impl FnMut(TerminalRelationModularProgress),
    ) -> Result<TerminalRelationModularStats, TerminalRelationError> {
        let total = self.session.seeds.len() * self.session.sources.len();
        let end = self
            .next_ordinal
            .saturating_add(self.options.batch_rows)
            .min(total);
        let executor = self.executor.clone();
        while self.next_ordinal < end && !cancel.load(Ordering::Relaxed) {
            let seed_ordinal = self.next_ordinal / self.session.sources.len();
            let source_ordinal = self.next_ordinal % self.session.sources.len();
            let (terms, conditions) = executor.install(|| {
                let (raw, conditions) = self.session.sources.row(
                    &self.session.seeds[seed_ordinal],
                    source_ordinal,
                    self.session.limits.algebra,
                )?;
                Ok::<_, TerminalRelationError>((self.session.normalized_row(&raw)?, conditions))
            })?;
            self.rows.push(SourceRow {
                identity: TerminalRelationModularSource::Ordinary {
                    seed_ordinal,
                    source_ordinal,
                },
                terms,
                conditions,
            });
            let discovered = executor.install(|| {
                self.discover_last().and_then(|result| {
                    if result.is_none() {
                        self.rebuild_image()
                    } else {
                        Ok(())
                    }
                })
            });
            if let Err(error) = discovered {
                // A refused preparation/sample never consumes an ordinary
                // source or leaves its identity duplicated on a later retry.
                self.rows.pop();
                return Err(error);
            }
            self.next_ordinal += 1;
            observe(TerminalRelationModularProgress {
                stage: "finite-field discovery",
                statistics: self.statistics(),
            });
        }
        let pending: Vec<_> = self
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                matches!(
                    candidate.status,
                    TerminalRelationModularProposalStatus::Pending
                )
            })
            .take(self.options.max_candidates)
            .map(|(index, _)| index)
            .collect();
        if !pending.is_empty() && !cancel.load(Ordering::Relaxed) {
            observe(TerminalRelationModularProgress {
                stage: "source-weight reconstruction and exact products",
                statistics: self.statistics(),
            });
        }
        // Every task reads the same frozen exact authority and source frame.
        // Native nested Rayon work stays inside this one bounded pool. Width
        // one deliberately uses a serial iterator, never the global Rayon pool.
        let state: &Self = self;
        let run = |&index: &usize| {
            (!cancel.load(Ordering::Relaxed)).then(|| state.lift(index, &mut |_| {}))
        };
        let results: Vec<_> = executor.install(|| {
            if executor.workers() == 1 {
                pending.iter().map(run).collect()
            } else {
                pending.par_iter().map(run).collect()
            }
        });
        // Exact publication remains ordered, after all started tasks drain.
        for (index, result) in pending.into_iter().zip(results) {
            let Some(result) = result else {
                continue;
            };
            match result {
                Ok(mut circuit) => {
                    let target = &self.candidates[index].target;
                    let column = self.session.columns.len()
                        - 1
                        - self.session.column_offsets[target] as usize;
                    if self.session.reducer.pivots()[column].is_some() {
                        // A different saved/retried circuit may already have
                        // proved this target. Skipping it is a heuristic miss;
                        // reducing it further could introduce a new symbolic
                        // pivot inversion outside the reconstructed circuit.
                        self.candidates[index].status =
                            TerminalRelationModularProposalStatus::Rejected {
                                reason: "target already has an exact terminal pivot".into(),
                            };
                        observe(TerminalRelationModularProgress {
                            stage: "exact target already proved",
                            statistics: self.statistics(),
                        });
                        continue;
                    }
                    // Earlier successes in this same speculative batch may
                    // have added guards after this task froze its input.
                    for guard in &self.session.conditions {
                        if !circuit.nonzero_conditions.contains(guard) {
                            circuit.nonzero_conditions.push(guard.clone());
                        }
                    }
                    let before = self.session.terminal_relations;
                    executor.install(|| self.session.add_row(circuit.image.clone()))?;
                    if self.session.terminal_relations > before {
                        for guard in &circuit.nonzero_conditions {
                            if !self.session.conditions.contains(guard) {
                                self.session.conditions.push(guard.clone());
                            }
                        }
                        self.candidates[index].status =
                            TerminalRelationModularProposalStatus::Accepted {
                                circuit: self.circuits.len(),
                            };
                        self.circuits.push(circuit);
                    } else {
                        self.candidates[index].status =
                            TerminalRelationModularProposalStatus::Rejected {
                                reason: "exact image supplies no new terminal pivot".into(),
                            };
                    }
                }
                Err(error) => {
                    self.reconstruction_misses += 1;
                    self.candidates[index].status =
                        TerminalRelationModularProposalStatus::Rejected {
                            reason: error.to_string(),
                        };
                }
            }
            observe(TerminalRelationModularProgress {
                stage: "exact circuit checked",
                statistics: self.statistics(),
            });
        }
        Ok(self.statistics())
    }

    fn lift(
        &self,
        index: usize,
        observe: &mut dyn FnMut(ProjectedSourceWeightEvent),
    ) -> Result<TerminalRelationModularCircuit, TerminalRelationError> {
        let candidate = &self.candidates[index];
        let context = IndexedCoefficientContext::try_new_with_limits(
            self.session.family.coefficient_context(),
            "terminal-modular-circuits-v1",
            self.session.family.denominator_count(),
            IndexedContextLimits {
                max_index_variables: usize::MAX,
                max_fingerprint_bytes: usize::MAX,
                max_native_symbol_name_bytes: usize::MAX,
            },
        )
        .map_err(algebra)?;
        let terminal_order: Vec<_> = self
            .session
            .columns
            .iter()
            .filter(|key| self.session.terminals.contains(*key))
            .cloned()
            .collect();
        let harder: BTreeSet<_> = terminal_order
            .iter()
            .take_while(|key| **key != candidate.target)
            .cloned()
            .collect();
        let selected: Vec<_> = candidate
            .sources
            .iter()
            .map(|&source| &self.rows[source])
            .collect();
        let labels = labels::ColumnLabels::new(
            &self.session,
            &self.image,
            &selected,
            &harder,
            &candidate.target,
        )?;
        let frame = selected
            .iter()
            .map(|source| {
                source
                    .terms
                    .iter()
                    .map(|(k, c)| {
                        Ok((
                            labels.label(k)?.clone(),
                            context
                                .lift_with_limits(c, self.options.reconstruction.arithmetic)
                                .map_err(algebra)?,
                        ))
                    })
                    .collect::<Result<BTreeMap<_, _>, TerminalRelationError>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        // This is an explicitly selected source circuit: all unselected
        // original weights are structurally fixed to zero. Keep every actual
        // selected-frame auxiliary, even if its discovery value vanished, and
        // every harder terminal (including absent exact-zero obligations).
        // Unused columns outside this exact frame are zero by construction.
        let forbidden = harder
            .iter()
            .chain(
                candidate
                    .sources
                    .iter()
                    .flat_map(|&source| self.rows[source].terms.keys()),
            )
            .filter(|key| !self.session.terminals.contains(*key) || harder.contains(*key))
            .map(|key| Ok(labels.label(key)?.clone()))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let proposal = reconstruct_projected_source_weights(
            &context,
            &frame,
            labels.label(&candidate.target)?,
            &forbidden,
            self.options.reconstruction,
            observe,
        )
        .map_err(algebra)?;
        let assignment = vec![0; self.session.family.denominator_count()];
        let mut conditions = self.session.conditions.clone();
        let mut condition_seen: HashSet<_> = conditions.iter().cloned().collect();
        // All scanned inputs retain their domain, including heuristic exclusions
        // and zero-weight sources. Source selection is not a domain shortcut.
        for guard in self.rows.iter().flat_map(|r| &r.conditions) {
            if condition_seen.insert(guard.clone()) {
                conditions.push(guard.clone());
            }
        }
        let mut denominator_seen = HashSet::new();
        for value in self.rows.iter().flat_map(|r| r.terms.values()) {
            if !value.denominator.is_constant() && denominator_seen.insert(&value.denominator) {
                let pole: Coefficient = value.denominator.clone().into();
                if condition_seen.insert(pole.clone()) {
                    conditions.push(pole);
                }
            }
        }
        let mut restore = |value: &crate::algebra::IndexedCoefficient| -> Result<Coefficient, TerminalRelationError> {
            let (value, pole) = context.specialize(value, &assignment, self.session.limits.algebra).map_err(algebra)?;
            if let Some(pole) = pole {
                let pole: Coefficient = pole.into();
                if condition_seen.insert(pole.clone()) { conditions.push(pole); }
            }
            Ok(value)
        };
        let weights = proposal
            .weights
            .iter()
            .map(|(&ordinal, value)| Ok((ordinal, restore(value)?)))
            .collect::<Result<BTreeMap<_, _>, TerminalRelationError>>()?;
        let image = proposal
            .image
            .iter()
            .map(|(key, value)| Ok((labels.physical(key)?.clone(), restore(value)?)))
            .collect::<Result<TerminalRelationRow, TerminalRelationError>>()?;
        if image.is_empty()
            || image
                .keys()
                .any(|key| !self.session.terminals.contains(key) || harder.contains(key))
            || image.get(&candidate.target).is_none_or(|c| !c.is_one())
        {
            return Err(invalid(
                "exact reconstructed image is not an admitted terminal-only relation",
            ));
        }
        Ok(TerminalRelationModularCircuit {
            scanned_sources: self.rows.len(),
            sources: candidate.sources[..proposal.prefix_rows]
                .iter()
                .map(|&source| self.rows[source].identity.clone())
                .collect(),
            weights,
            image,
            nonzero_conditions: conditions,
        })
    }

    /// Persist a distinct discovery envelope, including immutable exact baseline
    /// and exact circuit proofs. The ordinary exact checkpoint schema is intact.
    pub fn to_native_bytes(
        &self,
        limits: BinaryIoLimits,
    ) -> Result<Vec<u8>, TerminalRelationError> {
        codec::encode(self, limits)
    }
    /// Restore sampled U/L-pattern incrementally and exactly revalidate every
    /// newly accepted full-column source product before rebuilding publication.
    pub fn from_native_bytes(
        bytes: &[u8],
        limits: TerminalRelationLimits,
        io: BinaryIoLimits,
    ) -> Result<Self, TerminalRelationError> {
        codec::decode(bytes, limits, io)
    }
}
