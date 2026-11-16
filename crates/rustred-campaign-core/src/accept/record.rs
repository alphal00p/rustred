//! The evidence record of an accepted candidate (schema `rustred.campaign.acceptance.v1`, interface section 4).
//! Canonical JSON by the campaign's ONE rule (`schema::canon::canonical_json`); id = `sha256:` of that text. No
//! time, host or thread count inside.

use serde::Serialize;
use serde_json::{Value, json};

use super::prover::{Evidence, EvidenceArtefact, keys_digest};
use super::rule::sha256_tag;
use super::{AcceptPolicy, Candidate, CheckOutcome, Exclusion, NodeTarget, NodeView, Scope};
use crate::key::Key;

pub const ACCEPTANCE_SCHEMA: &str = "rustred.campaign.acceptance.v1";

/// The record (a canonical JSON value plus its id). Fields are private (audit-accept-B M2): built only by the chain.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AcceptanceRecord {
    value: Value,
    /// `sha256:` of [`AcceptanceRecord::canonical`].
    id: String,
    canonical: String,
}

pub(crate) struct RecordParts<'a, 'b> {
    pub node: &'a NodeView<'b>,
    pub candidate: &'a Candidate<'b>,
    pub policy: &'a AcceptPolicy,
    pub working_digest: String,
    pub working_count: u64,
    pub served: &'a [Key],
    pub stopped: u64,
    pub displaced: u64,
    pub gap_count: u64,
    pub credited: &'a [Key],
    pub not_credited: &'a [(Key, String)],
    pub checks: &'a [CheckOutcome],
    pub exclusions: &'a [Exclusion],
    pub evidence_level: Evidence,
    pub artefact: &'a EvidenceArtefact,
    pub scope: &'a Scope,
    pub shadowed: Option<String>,
    pub pattern_not_proved: Vec<usize>,
    /// FULL scope: why symbolic regularity on the whole domain is not shown (audit-accept-B C3)
    pub regularity_not_proved: &'a [String],
    /// "witness_loci" or "regularity_fact:<fact file digest>" (addendum 3)
    pub credit_basis: String,
    /// [`super::node_identity`] of the frame and comparator the chain judged in
    pub node_identity: &'a str,
    /// ([`super::record_set_digest`], count) of the records in force before the candidate
    pub in_force: (&'a str, u64),
}

impl AcceptanceRecord {
    /// The canonical JSON value.
    pub fn value(&self) -> &Value {
        &self.value
    }
    /// `sha256:` of the canonical text (the record id).
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn canonical(&self) -> &str {
        &self.canonical
    }

    pub(super) fn build(p: RecordParts) -> AcceptanceRecord {
        let n = p.node.arity();
        let rule = p.candidate.rule;
        let target = match &p.node.target {
            NodeTarget::Full => "full".to_string(),
            NodeTarget::Boxed { box_id } => format!("boxed:{box_id}"),
        };
        let value = json!({
            "schema": ACCEPTANCE_SCHEMA,
            "node": p.node.node,
            "target": target,
            "rule": {
                "line_sha256": rule.line_sha256,
                "index": rule.index(),
                "source": p.candidate.source,
            },
            "node_identity": p.node_identity,
            "in_force": {"count": p.in_force.1, "set_digest": p.in_force.0},
            "position": p.candidate.position,
            "required_evidence": p.node.required,
            "policy": p.policy,
            "working_range": {"count": p.working_count, "digest": p.working_digest},
            "served": {"count": p.served.len(), "digest": keys_digest(n, p.served)},
            "stopped": {"count": p.stopped},
            "displaced": {"count": p.displaced},
            "gap_points": {
                "count": p.gap_count,
                "credited": p.credited,
                "not_credited": p.not_credited.iter().map(|(k, w)| json!({"key": k, "why": w})).collect::<Vec<_>>(),
            },
            "checks": p.checks,
            "exclusions": p.exclusions,
            "evidence": {
                "level": p.evidence_level,
                "ledger_word": p.evidence_level.ledger_word(),
                "plan_level": p.evidence_level.plan_level(),
                "artefact": p.artefact,
            },
            "scope": p.scope,
            "credit_basis": p.credit_basis,
            "symbolic": {"shadowed": p.shadowed, "pattern_descent_not_proved": p.pattern_not_proved,
                "regularity_not_proved": p.regularity_not_proved},
            "verdict": "accepted",
        });
        let canonical = crate::schema::canon::canonical_json(&value, false)
            .expect("acceptance record holds integers only");
        AcceptanceRecord {
            id: sha256_tag(canonical.as_bytes()),
            value,
            canonical,
        }
    }
}
