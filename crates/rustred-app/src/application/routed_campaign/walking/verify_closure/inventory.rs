//! Read-only inventory of the native symbolic covers in one saved generation.
//!
//! CP6 stores inspection counts, not rule or terminal identities. This census
//! therefore observes the full native reinspection already performed by the
//! cold verifier. It never substitutes every residual in an installed payload
//! for terminals actually encountered by that inspection.

use super::{
    OwnerDomainWalkVerifyOptions, OwnerDomainWalkVerifyReferenceLevers,
    OwnerDomainWalkVerifyReinspect, verify_closure_with_progress,
};
use crate::{
    AppError, CandidateArtifactPage, CandidateTerminalNormalization,
    CandidateTerminalNormalizationLimits, OwnerDomainWalkRequest,
};
use rustred::family::{IntegralFamily, IntegralKey};
use rustred::sector::OrderingPolicy;
use rustred::solver::{CandidateOwnerPrograms, OwnerDomainMatchDisposition, OwnerDomainMatchPiece};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Resource bounds apply only to this optional census, not the saved campaign.
#[derive(Clone, Debug)]
pub struct OwnerDomainWalkInventoryOptions {
    pub verification: OwnerDomainWalkVerifyOptions,
    pub max_unique_rules: usize,
    pub max_unique_terminals: usize,
}

impl OwnerDomainWalkInventoryOptions {
    pub fn new(checkpoint: impl Into<PathBuf>) -> Self {
        let mut verification = OwnerDomainWalkVerifyOptions::new(checkpoint);
        verification.reference_levers = OwnerDomainWalkVerifyReferenceLevers::AsRun;
        verification.require_closure = true;
        Self {
            verification,
            max_unique_rules: 1_000_000,
            max_unique_terminals: 1_000_000,
        }
    }
}

/// A deterministic, paged census bound to a captured checkpoint generation.
///
/// "Encountered" refers to the saved inspected symbolic-domain covers,
/// including conditional successors and conservative route covers. It is not
/// a proof that every reported integral occurs with nonzero coefficient in a
/// concrete starting reduction, nor a claim of master independence/minimality.
pub struct OwnerDomainWalkInventory {
    creator_pid: u32,
    family: Arc<IntegralFamily>,
    verification: Value,
    installed: Value,
    rules: BTreeMap<RuleIdentity, u64>,
    terminals: BTreeSet<IntegralKey>,
    selected_rule_events: u64,
    terminal_events: u64,
    zero_events: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RuleIdentity {
    owner: Vec<bool>,
    batch: usize,
    rule: usize,
}

#[derive(Default)]
struct State {
    family: Option<Arc<IntegralFamily>>,
    installed: Value,
    rules: BTreeMap<RuleIdentity, u64>,
    terminals: BTreeSet<IntegralKey>,
    selected_rule_events: u64,
    terminal_events: u64,
    zero_events: u64,
    error: Option<AppError>,
}

pub(super) struct Collector {
    state: Mutex<State>,
    max_unique_rules: usize,
    max_unique_terminals: usize,
}

impl Collector {
    fn new(options: &OwnerDomainWalkInventoryOptions) -> Self {
        Self {
            state: Mutex::new(State::default()),
            max_unique_rules: options.max_unique_rules,
            max_unique_terminals: options.max_unique_terminals,
        }
    }

    pub(super) fn prepared<const N: usize>(
        &self,
        programs: &CandidateOwnerPrograms<N>,
    ) -> Result<(), AppError> {
        let mut state = self.state.lock().expect("inventory preparation");
        if state.family.is_some() {
            return Err(AppError::execution("inventory owner preparation repeated"));
        }
        state.family = Some(Arc::clone(programs.context().family_owner()));
        let physical_arity = programs.context().family().denominator_count();
        let mut rules = 0usize;
        let mut terminal_records = 0usize;
        let batches = programs
            .installed_inventory()
            .map(|(owner, batch, batch_rules, batch_terminals)| {
                let owner =
                    physical_owner(owner, physical_arity).map_err(AppError::internal_invariant)?;
                let owner: String = owner
                    .iter()
                    .map(|&active| if active { '1' } else { '0' })
                    .collect();
                rules += batch_rules;
                terminal_records += batch_terminals;
                Ok(json!({"owner":owner,"batch":batch,
                    "rules":batch_rules,"declared_terminals":batch_terminals}))
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        state.installed = json!({
            "owners":programs.owner_count(),"batches":batches.len(),
            "rules":rules,"declared_terminal_records":terminal_records,
            "owner_qualified_unique_declared_terminals":programs.terminal_count(),
            "batch_inventory":batches,
            "scope":"actually installed owner programs, including preferred programs and overlays; not all sectors in each source artifact",
        });
        Ok(())
    }

    pub(super) fn classified<const N: usize>(
        &self,
        piece: &OwnerDomainMatchPiece<N>,
    ) -> ControlFlow<()> {
        let mut state = self.state.lock().expect("inventory classification");
        if state.error.is_some() {
            return ControlFlow::Break(());
        }
        let result = self.record(&mut state, piece);
        if let Err(error) = result {
            state.error = Some(error);
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }

    fn record<const N: usize>(
        &self,
        state: &mut State,
        piece: &OwnerDomainMatchPiece<N>,
    ) -> Result<(), AppError> {
        let physical_arity = state
            .family
            .as_ref()
            .ok_or_else(|| AppError::internal_invariant("inventory family was not prepared"))?
            .denominator_count();
        let owner =
            physical_owner(piece.owner(), physical_arity).map_err(AppError::internal_invariant)?;
        match piece.disposition() {
            OwnerDomainMatchDisposition::SelectedRule { batch, rule } => {
                let identity = RuleIdentity {
                    owner: owner.to_vec(),
                    batch,
                    rule,
                };
                if !state.rules.contains_key(&identity)
                    && state.rules.len() >= self.max_unique_rules
                {
                    return Err(AppError::limit(format!(
                        "unique encountered rules exceed inventory limit {}",
                        self.max_unique_rules
                    )));
                }
                *state.rules.entry(identity).or_default() += 1;
                state.selected_rule_events += 1;
            }
            OwnerDomainMatchDisposition::Terminal { .. } => {
                let key = terminal_key(piece.owner(), piece.lower(), piece.upper(), physical_arity)
                    .map_err(AppError::internal_invariant)?;
                if !state.terminals.contains(&key)
                    && state.terminals.len() >= self.max_unique_terminals
                {
                    return Err(AppError::limit(format!(
                        "unique encountered terminals exceed inventory limit {}",
                        self.max_unique_terminals
                    )));
                }
                state.terminals.insert(key);
                state.terminal_events += 1;
            }
            OwnerDomainMatchDisposition::ExactZeroSector => state.zero_events += 1,
            _ => {}
        }
        Ok(())
    }

    fn finish(self, verification: Value) -> Result<OwnerDomainWalkInventory, AppError> {
        let state = self.state.into_inner().expect("inventory finish");
        if let Some(error) = state.error {
            return Err(error);
        }
        Ok(OwnerDomainWalkInventory {
            creator_pid: std::process::id(),
            family: state
                .family
                .ok_or_else(|| AppError::execution("inventory family was not prepared"))?,
            verification,
            installed: state.installed,
            rules: state.rules,
            terminals: state.terminals,
            selected_rule_events: state.selected_rule_events,
            terminal_events: state.terminal_events,
            zero_events: state.zero_events,
        })
    }
}

/// The solver may share a larger fixed-capacity buffer. Only authenticated
/// inactive padding can be removed at this public physical-coordinate boundary.
fn physical_owner(owner: &[bool], physical_arity: usize) -> Result<&[bool], String> {
    if physical_arity == 0 || physical_arity > owner.len() {
        return Err("inventory physical arity exceeds its storage or is empty".into());
    }
    if owner[physical_arity..].iter().any(|&active| active) {
        return Err("inventory owner has an active padding coordinate".into());
    }
    Ok(&owner[..physical_arity])
}

/// A terminal classification fixes *every* exponent. Recovering the integral
/// is a checked coordinate conversion, not a search or symbolic evaluation.
fn terminal_key(
    owner: &[bool],
    lower: &[u64],
    upper: &[Option<u64>],
    physical_arity: usize,
) -> Result<IntegralKey, String> {
    if owner.len() != lower.len() || owner.len() != upper.len() {
        return Err("terminal classification arity mismatch".into());
    }
    let owner = physical_owner(owner, physical_arity)?;
    if lower[physical_arity..].iter().any(|&value| value != 0)
        || upper[physical_arity..]
            .iter()
            .any(|&value| value != Some(0))
    {
        return Err("terminal classification has nonzero or open padding".into());
    }
    let values = owner
        .iter()
        .zip(&lower[..physical_arity])
        .zip(&upper[..physical_arity])
        .map(|((&positive, &value), &upper)| {
            if upper != Some(value) {
                return Err("terminal classification is not a singleton".to_owned());
            }
            let value = if positive {
                i128::from(value) + 1
            } else {
                -i128::from(value)
            };
            i64::try_from(value)
                .map_err(|_| "terminal exponent exceeds integral-key range".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    IntegralKey::try_new(values).map_err(|e| e.to_string())
}

/// Inventory one captured checkpoint with a full, read-only native replay.
/// No campaign file, installed rule, numerical catalog or checkpoint is changed.
pub fn owner_domain_walk_inventory(
    request: &OwnerDomainWalkRequest,
    options: &OwnerDomainWalkInventoryOptions,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<OwnerDomainWalkInventory, AppError> {
    if options.verification.reinspect != OwnerDomainWalkVerifyReinspect::All
        || options.verification.reference_levers != OwnerDomainWalkVerifyReferenceLevers::AsRun
        || options.verification.mutation.is_some()
    {
        return Err(AppError::input(
            "walk inventory requires full reinspection, AsRun native policies, and no mutation",
        ));
    }
    if options.max_unique_rules == 0 || options.max_unique_terminals == 0 {
        return Err(AppError::input(
            "inventory unique-entry limits must be positive",
        ));
    }
    let collector = Collector::new(options);
    let verification = verify_closure_with_progress(
        request,
        &options.verification,
        cancellation,
        &observer,
        Some(&collector),
    )?;
    collector.finish(verification)
}

impl OwnerDomainWalkInventory {
    pub fn family_owner(&self) -> &Arc<IntegralFamily> {
        &self.family
    }

    pub fn terminal_keys(&self) -> &BTreeSet<IntegralKey> {
        &self.terminals
    }

    pub fn summary(&self) -> Value {
        json!({
            "schema":"rustred.walk-inventory.v1",
            "complete":self.verification["verdict"]=="PASS",
            "family_fingerprint":self.family.fingerprint(),
            "family_count":1,"cross_family_merged":false,
            "installed":self.installed,
            "encountered":{
                "rules":self.rules.len(),"terminals":self.terminals.len(),
                "selected_rule_events":self.selected_rule_events,
                "terminal_events":self.terminal_events,"zero_sector_events":self.zero_events,
                "scope":"non-abandoned native symbolic-domain covers in the captured checkpoint; partial/G2 records use only their recorded inspected residuals",
                "concrete_target_reachability_claim":false,
                "complete_reinspection":self.verification["reinspection"]["complete"],
                "validated_inventory":self.verification["verdict"]=="PASS",
                "caveat":"conditional successors, route covers, helper domains and previously inspected retired/quarantined records may overapproximate current required-query reductions; abandoned uninspected records and declared but unencountered payload residuals are excluded",
            },
            "master_minimality_claim":false,"family_closure_claim":false,
            "verification":self.verification,
        })
    }

    pub fn rules(
        &self,
        start: usize,
        limit: usize,
    ) -> Result<CandidateArtifactPage<Value>, AppError> {
        let take = page(self.rules.len(), start, limit)?;
        Ok(CandidateArtifactPage {
            total: self.rules.len(),
            start,
            items: self.rules.iter().skip(start).take(take).map(|(id, events)| {
                let owner: String = id.owner.iter().map(|&x| if x {'1'} else {'0'}).collect();
                json!({"owner":owner,"batch":id.batch,"rule":id.rule,"classification_events":events})
            }).collect(),
        })
    }

    pub fn terminals(
        &self,
        start: usize,
        limit: usize,
    ) -> Result<CandidateArtifactPage<Vec<i64>>, AppError> {
        let take = page(self.terminals.len(), start, limit)?;
        Ok(CandidateArtifactPage {
            total: self.terminals.len(),
            start,
            items: self
                .terminals
                .iter()
                .skip(start)
                .take(take)
                .map(|key| key.powers().to_vec())
                .collect(),
        })
    }

    /// Explicit optional algebra, applied once to the common family's union of
    /// encountered keys, never to all residuals declared in the input bundles.
    pub fn normalize_terminals(
        &self,
        limits: CandidateTerminalNormalizationLimits,
    ) -> Result<CandidateTerminalNormalization, AppError> {
        if self.creator_pid != std::process::id() {
            return Err(AppError::execution(
                "inventory normalization cannot be reused after fork; reinspect in a fresh process",
            ));
        }
        if self.verification["verdict"] != "PASS" {
            return Err(AppError::input(
                "terminal normalization requires a complete validated inventory",
            ));
        }
        CandidateTerminalNormalization::from_keys(
            &self.family,
            &self.terminals,
            OrderingPolicy::SpiredUncutV1,
            limits,
        )
    }
}

fn page(total: usize, start: usize, limit: usize) -> Result<usize, AppError> {
    if limit == 0 || limit > 1000 {
        return Err(AppError::input("inventory page size must be in 1..=1000"));
    }
    if start > total {
        return Err(AppError::input("inventory page start exceeds total"));
    }
    Ok(limit.min(total - start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singleton_terminal_keys_are_exact_and_checked() {
        assert_eq!(
            terminal_key(
                &[true, false, false],
                &[2, 0, 5],
                &[Some(2), Some(0), Some(5)],
                3,
            )
            .unwrap()
            .powers(),
            &[3, 0, -5]
        );
        assert!(terminal_key(&[true], &[0], &[None], 1).is_err());
        assert!(terminal_key(&[true], &[0], &[Some(1)], 1).is_err());
        assert!(terminal_key(&[true], &[i64::MAX as u64], &[Some(i64::MAX as u64)], 1).is_err());
        assert_eq!(
            terminal_key(&[false], &[1u64 << 63], &[Some(1u64 << 63)], 1)
                .unwrap()
                .powers(),
            &[i64::MIN]
        );
        assert!(terminal_key(&[false], &[u64::MAX], &[Some(u64::MAX)], 1).is_err());
        assert!(terminal_key(&[false], &[], &[], 1).is_err());
    }

    #[test]
    fn capacity_terminal_keys_and_owner_masks_project_only_zero_padding() {
        let owner = [true, false, false, false];
        let lower = [2, 5, 0, 0];
        let upper = [Some(2), Some(5), Some(0), Some(0)];
        assert_eq!(physical_owner(&owner, 2).unwrap(), &[true, false]);
        assert_eq!(
            terminal_key(&owner, &lower, &upper, 2).unwrap().powers(),
            &[3, -5]
        );
        assert!(physical_owner(&[true, false, true, false], 2).is_err());
        assert!(terminal_key(&[true, false, true, false], &lower, &upper, 2).is_err());
        assert!(terminal_key(&owner, &[2, 5, 1, 0], &upper, 2).is_err());
        assert!(terminal_key(&owner, &lower, &[Some(2), Some(5), None, Some(0)], 2).is_err());
        assert!(terminal_key(&owner, &lower, &[Some(2), Some(5), Some(1), Some(0)], 2).is_err());
        assert!(terminal_key(&owner, &lower, &upper, 0).is_err());
        assert!(terminal_key(&owner, &lower, &upper, 5).is_err());
        assert!(physical_owner(&owner, 5).is_err());
    }

    #[test]
    fn inventory_pages_never_expand_unbounded_detail() {
        assert_eq!(page(7, 5, 1000).unwrap(), 2);
        assert_eq!(page(7, 7, 1).unwrap(), 0);
        assert!(page(7, 8, 1).is_err());
        assert!(page(7, 0, 0).is_err());
        assert!(page(7, 0, 1001).is_err());
        let options = OwnerDomainWalkInventoryOptions::new("unused");
        assert_eq!(
            options.verification.reinspect,
            OwnerDomainWalkVerifyReinspect::All
        );
        assert_eq!(
            options.verification.reference_levers,
            OwnerDomainWalkVerifyReferenceLevers::AsRun
        );
        assert!(options.verification.require_closure);
    }

    #[test]
    #[cfg(feature = "cli")]
    fn saved_scope_inventory_excludes_unvisited_installed_terminals() {
        use super::super::e2e_tests::{Fixture, run};
        let fixture = Fixture {
            rank: 0,
            queries: json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[{
                "id":"sunset-corner", "owner":"111", "lower":[0,0,0], "upper":[0,0,0],
                "max_numerator_rank":0,
                "power_bounds":{"max_positive_power":3,"min_power_difference":3,"max_power_difference":3}
            }]}),
            lookahead: 1,
            omit: &[],
            routes: json!([]),
        };
        let run = run("terminal-inventory", &fixture, &[]);
        let mut options = OwnerDomainWalkInventoryOptions::new(run.dir.0.join("checkpoint"));
        let inspect = |options: &OwnerDomainWalkInventoryOptions| {
            owner_domain_walk_inventory(&run.request, options, &AtomicBool::new(false), |_| {})
                .unwrap()
        };
        let inventory = inspect(&options);
        let summary = inventory.summary();
        assert_eq!(summary["verification"]["verdict"], "PASS", "{summary}");
        assert_eq!(summary["encountered"]["terminals"], 1);
        assert_eq!(summary["encountered"]["rules"], 0);
        assert!(
            inventory
                .terminal_keys()
                .iter()
                .all(|key| key.powers().len() == inventory.family_owner().denominator_count())
        );
        assert!(
            summary["installed"]["batch_inventory"]
                .as_array()
                .unwrap()
                .iter()
                .all(|batch| batch["owner"].as_str().unwrap().len()
                    == inventory.family_owner().denominator_count())
        );
        assert!(summary["installed"]["rules"].as_u64().unwrap() > 0);
        assert!(
            summary["installed"]["owner_qualified_unique_declared_terminals"]
                .as_u64()
                .unwrap()
                > 1
        );
        assert_eq!(
            inventory.terminals(0, 10).unwrap().items,
            vec![vec![1, 1, 1]]
        );
        options.verification.threads = 2;
        let parallel = inspect(&options);
        assert_eq!(summary["encountered"], parallel.summary()["encountered"]);
        assert_eq!(inventory.terminal_keys(), parallel.terminal_keys());
        options.verification.reinspect = OwnerDomainWalkVerifyReinspect::None;
        assert!(
            owner_domain_walk_inventory(&run.request, &options, &AtomicBool::new(false), |_| {})
                .is_err()
        );
        options.verification.reinspect = OwnerDomainWalkVerifyReinspect::All;
        options.verification.mutation =
            Some(super::super::OwnerDomainWalkVerifyMutation::DroppedEdge);
        assert!(
            owner_domain_walk_inventory(&run.request, &options, &AtomicBool::new(false), |_| {})
                .is_err()
        );
    }

    #[test]
    #[cfg(feature = "cli")]
    fn rule_inventory_replays_unchanged_events_and_normalizes_only_complete_results() {
        use super::super::e2e_tests::{Fixture, run};
        let fixture = Fixture {
            rank: 0,
            queries: json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[{
                "id":"sunset-one-dot", "owner":"111", "lower":[0,0,0], "upper":[1,0,0],
                "max_numerator_rank":0,
                "power_bounds":{"max_positive_power":4,"min_power_difference":3,"max_power_difference":4}
            }]}),
            lookahead: 1,
            omit: &[],
            routes: json!([]),
        };
        let run = run("rule-inventory", &fixture, &[]);
        let mut options = OwnerDomainWalkInventoryOptions::new(run.dir.0.join("checkpoint"));
        let cancellation = AtomicBool::new(false);
        let inventory =
            owner_domain_walk_inventory(&run.request, &options, &cancellation, |_| {}).unwrap();
        let summary = inventory.summary();
        assert_eq!(summary["complete"], true, "{summary}");
        assert!(summary["encountered"]["rules"].as_u64().unwrap() > 0);
        assert!(summary["encountered"]["terminals"].as_u64().unwrap() > 0);
        let plain = super::super::owner_domain_walk_verify_closure(
            &run.request,
            &options.verification,
            &cancellation,
            |_| {},
        )
        .unwrap();
        for field in [
            "inspected",
            "events",
            "successor_events",
            "admitted_domains",
            "frontiers",
            "errors",
            "uncovered",
        ] {
            assert_eq!(
                summary["verification"]["reinspection"]["tally"][field],
                plain["reinspection"]["tally"][field],
                "changed native count: {field}"
            );
        }
        let normalized = inventory
            .normalize_terminals(CandidateTerminalNormalizationLimits::default())
            .unwrap()
            .metadata()
            .unwrap();
        assert_eq!(
            normalized["unique_raw_terminals"],
            summary["encountered"]["terminals"]
        );
        assert_eq!(normalized["master_minimality_claim"], false);
        options.verification.threads = 2;
        let parallel =
            owner_domain_walk_inventory(&run.request, &options, &cancellation, |_| {}).unwrap();
        assert_eq!(
            inventory.rules(0, 1000).unwrap().items,
            parallel.rules(0, 1000).unwrap().items
        );
        assert_eq!(inventory.terminal_keys(), parallel.terminal_keys());
        let stopped = owner_domain_walk_inventory(&run.request, &options, &cancellation, |event| {
            if event["event"] == "verify_prepared" {
                cancellation.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        })
        .unwrap();
        assert_eq!(stopped.summary()["complete"], false);
        assert!(
            stopped
                .normalize_terminals(CandidateTerminalNormalizationLimits::default())
                .is_err()
        );
    }
}
