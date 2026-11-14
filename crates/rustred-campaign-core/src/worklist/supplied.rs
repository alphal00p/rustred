//! WAVE 0 hook (R43, interfaces/extra-rules.md section 5.2; decision of the directing session, addendum 4): a
//! [`CandidateSource`] may OFFER candidate relations for an item before any search wave, with no solver call. Offered
//! candidates go through exactly the ordinary acceptance (coverage at the gap points by d1-d4, per-key regularity,
//! descent under the full node comparator, identity evidence at the node's required level) and the ordinary economy
//! selection against searched candidates; their claims and domains are never trusted; a failing candidate is dropped
//! with its recorded verdicts (explain shows why). Only the no-op default ships; real sources (the
//! `rustred.campaign.supplied-rules.v1` reader) are deferred.

use super::items::{Item, NodeParams};
use super::ladder::CandidateRule;

/// One offered candidate with its provenance (the attempt key becomes "<item>|offer|<source>").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuppliedCandidate {
    pub candidate: CandidateRule,
    /// Engine rule-record form (case, terms, seeds), copied to provenance; never evidence.
    pub record: serde_json::Value,
    pub source_rule: String,
    pub instance: String,
}

pub trait CandidateSource {
    /// A short stable name (part of the attempt key).
    fn name(&self) -> &str;
    /// Candidates instantiated on this item's case (no solver call).
    fn offer(&mut self, item: &Item, params: &NodeParams) -> Vec<SuppliedCandidate>;
}

/// The default source: offers nothing (wave 0 is then a no-op).
pub struct NoSupplied;

impl CandidateSource for NoSupplied {
    fn name(&self) -> &str {
        "none"
    }
    fn offer(&mut self, _item: &Item, _params: &NodeParams) -> Vec<SuppliedCandidate> {
        vec![]
    }
}
