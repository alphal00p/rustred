//! GLUE between lanes that implemented the SAME concept with different types (integration lane int3, 2026-11-14).
//!
//! Nothing here is new logic: every function is an explicit conversion between two existing representations, with the
//! STRICT choice wherever one side cannot express what the other says (a typed [`GlueError`], never a silent
//! approximation). The types themselves are deliberately NOT merged here; the duplicated concepts and the recommended
//! winner of a later unification are listed in WS/notes/implementer.int3.md ("DUPLICATED CONCEPTS").
//!
//! Concepts covered:
//! - key exclusions: ladder [`ladder::Exclusion`] (cover members), acceptance [`accept::Exclusion`] (effective
//!   domain), closure [`KeyExclusion`] (dispatch input), terminals [`terminals::KeyExclusion`] (keeps_terminal);
//! - recipe rules: acceptance [`accept::recipe::Recipe`] + [`RecipeVerdict::Certified`], ladder
//!   [`ladder_recipe::RecipeRule`], closure [`RelationSet`] (dispatch entry);
//! - witness status / witness-absent policy: acceptance [`accept::rule::WitnessStatus`] (+ guard version), core
//!   [`cover::WitnessStatus`], closure [`WitnessAbsent`].
//!
//! All three recipe representations implement the coordinator's ruling (2026-11-14) "an entry matches a key only if the
//! key is in its domain AND certified; otherwise the dispatch falls through": the conversions keep exactly the
//! certified keys, and the tests check that both sides answer alike.

use std::fmt;

use crate::accept;
use crate::accept::recipe::{RecipeVerdict, SolveAnswers};
use crate::accept::rule::{RuleText, sha256_tag};
use crate::closure::{
    DispatchSlot, ExclusionReason, KeyCodec, KeyExclusion, RelationSet, RelationStoreBuilder,
    VERDICT_CERTIFIED, WitnessAbsent,
};
use crate::cover;
use crate::domain::CaseDomain;
use crate::key::Key;
use crate::terminals;
use crate::worklist::affine::AffineEq;
use crate::worklist::ladder;
use crate::worklist::recipe as ladder_recipe;

/// Why a conversion is not available (the strict answer: the caller keeps the item undecided / handles it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlueError {
    /// A LOCUS exclusion: the closure takes per-key exclusions only (a locus is infinite on a FULL node; enumerating
    /// it over a finite range would let the closure dispatch the excluded rule outside that range).
    LocusNotPerKey { equations: Vec<String> },
    /// A ladder exclusion reason with no acceptance `FailureKind` (the acceptance chain never excludes for it).
    NoAcceptKind(String),
    /// The recipe verdict is not `Certified`, or names another recipe.
    NotCertified(String),
    /// A certified key without its run-1 answer (or a failed one) in the answers given.
    MissingAnswer(Key),
    /// The answers given are not the ones certified (relation-set digest differs).
    RelationSetMismatch {
        certified: String,
        recomputed: String,
    },
    /// A ladder recipe rule whose per-key records are not exactly its certified keys.
    RecordsMismatch(String),
    /// A relation digest that is not `<algo>:<64 hex>` / `<64 hex>`.
    Digest(String),
    /// A recipe domain that cannot be represented as a case domain.
    Domain(String),
    /// Key codec / relation store error.
    KeySet(String),
}

impl fmt::Display for GlueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for GlueError {}

/// One record of the CLOSURE's dispatch: table name, record position in that table, label (all three are checked by
/// the closure, closure.md 11.3). For a relation set: `table` = set name, record 0, label 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosureRule<'a> {
    pub table: &'a str,
    pub record: usize,
    pub label: i64,
}

fn key_exclusion(rule: ClosureRule, key: &Key, reason: ExclusionReason) -> KeyExclusion {
    KeyExclusion {
        key: key.clone(),
        table: rule.table.to_string(),
        record: rule.record,
        label: rule.label,
        reason,
    }
}

// ------------------------------------------------------------------------------------------------------------------
// Key exclusions

/// LADDER cover-member exclusion ([`ladder::Exclusion`], repair-library.md v1.2 C1) -> ACCEPTANCE exclusion
/// ([`accept::Exclusion`], acceptance.md check 6): the adapter `from_ladder` that acceptance.md section 7 asked for.
/// Target: `Key` -> `Key`; `Locus{AffineEq}` -> `Locus{engine spelling "expr" meaning expr = 0}` (round trip through
/// `AffineEq::parse`). Reason: `OnPole` / `Elided` / `Blocked` / `PoleLocusChild` -> `Blocked` (first selected,
/// unusable), `NotDescending` -> `NotDescending`, `IdentityFailed` -> `NonMembership`; every other ladder reason
/// (coverage / identity UNKNOWN, evidence below level, not credited here, preferred terminal) has no acceptance kind:
/// the acceptance chain never excludes for it (it stays Undecided or is not a failure) -> [`GlueError::NoAcceptKind`].
/// The full ladder reason is kept in `disproof` (JSON).
pub fn accept_exclusion_from_ladder(e: &ladder::Exclusion) -> Result<accept::Exclusion, GlueError> {
    use accept::FailureKind as F;
    use ladder::ExclusionReason as R;
    let reason = match &e.reason {
        R::OnPole | R::Elided | R::Blocked { .. } | R::PoleLocusChild { .. } => F::Blocked,
        R::NotDescending { .. } => F::NotDescending,
        R::IdentityFailed { .. } => F::NonMembership,
        other => return Err(GlueError::NoAcceptKind(format!("{other:?}"))),
    };
    let target = match &e.target {
        ladder::ExclusionTarget::Key(k) => accept::ExclusionTarget::Key(k.clone()),
        ladder::ExclusionTarget::Locus { equations } => accept::ExclusionTarget::Locus {
            equations: equations.iter().map(AffineEq::to_engine).collect(),
        },
    };
    Ok(accept::Exclusion {
        target,
        reason,
        disproof: format!(
            "ladder:{}",
            serde_json::to_string(&e.reason).unwrap_or_else(|_| format!("{:?}", e.reason))
        ),
    })
}

/// ACCEPTANCE effective-domain exclusions ([`accept::Exclusion`] of an accepted rule) -> CLOSURE exclusion input
/// ([`KeyExclusion`], closure.md 11.3) for the record `rule` of the closure's dispatch. Same semantics on both sides:
/// at the key the rule is skipped and the dispatch continues with the next entry. A `Key` target gives one
/// exclusion (reason `Other("accept:<kind>:<disproof>")`); a `Locus` target -> [`GlueError::LocusNotPerKey`].
pub fn closure_exclusions_from_accept(
    rule: ClosureRule,
    exclusions: &[accept::Exclusion],
) -> Result<Vec<KeyExclusion>, GlueError> {
    exclusions
        .iter()
        .map(|e| match &e.target {
            accept::ExclusionTarget::Key(k) => Ok(key_exclusion(
                rule,
                k,
                ExclusionReason::Other(format!("accept:{:?}:{}", e.reason, e.disproof)),
            )),
            accept::ExclusionTarget::Locus { equations } => Err(GlueError::LocusNotPerKey {
                equations: equations.clone(),
            }),
        })
        .collect()
}

/// LADDER cover-member exclusions (`CoverMember.exclusions`, the member's effective domain) -> CLOSURE exclusion
/// input for the record `rule` (where the caller placed the accepted member). Lossless on the reason:
/// `PreferredTerminal` -> `KeepsTerminal`, every other reason -> `Other("ladder:<json>")`. A `Locus` target ->
/// [`GlueError::LocusNotPerKey`].
pub fn closure_exclusions_from_ladder(
    rule: ClosureRule,
    exclusions: &[ladder::Exclusion],
) -> Result<Vec<KeyExclusion>, GlueError> {
    exclusions
        .iter()
        .map(|e| match &e.target {
            ladder::ExclusionTarget::Key(k) => Ok(key_exclusion(
                rule,
                k,
                match &e.reason {
                    ladder::ExclusionReason::PreferredTerminal => ExclusionReason::KeepsTerminal,
                    r => ExclusionReason::Other(format!(
                        "ladder:{}",
                        serde_json::to_string(r).unwrap_or_else(|_| format!("{r:?}"))
                    )),
                },
            )),
            ladder::ExclusionTarget::Locus { equations } => Err(GlueError::LocusNotPerKey {
                equations: equations.iter().map(AffineEq::to_engine).collect(),
            }),
        })
        .collect()
}

/// TERMINALS key exclusion ([`terminals::KeyExclusion`] `{key, rule (text), reason}`, plan 2.4.13) -> CLOSURE
/// exclusion for the record `rule` the caller resolved the rule text to. `reason == "keeps_terminal"` ->
/// `KeepsTerminal`, anything else -> `Other(reason)`.
pub fn closure_exclusion_from_terminal(
    rule: ClosureRule,
    e: &terminals::KeyExclusion,
) -> KeyExclusion {
    key_exclusion(
        rule,
        &e.key,
        if e.reason == "keeps_terminal" {
            ExclusionReason::KeepsTerminal
        } else {
            ExclusionReason::Other(e.reason.clone())
        },
    )
}

// ------------------------------------------------------------------------------------------------------------------
// Recipe rules -> relation sets

/// A relation digest text `<algo>:<64 hex>` or `<64 hex>` as the 32 bytes the closure stores (the engine's BLAKE3 hex
/// read by `closure::read_solve_results` gives the same bytes). Anything else -> [`GlueError::Digest`].
pub fn relation_digest_bytes(text: &str) -> Result<[u8; 32], GlueError> {
    let hex = text.rsplit_once(':').map(|(_, h)| h).unwrap_or(text);
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(GlueError::Digest(text.to_string()));
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .map_err(|_| GlueError::Digest(text.to_string()))?;
    }
    Ok(out)
}

/// ACCEPTANCE recipe certification ([`accept::recipe::Recipe`] + [`RecipeVerdict::Certified`] + the [`SolveAnswers`]
/// it was certified from) -> CLOSURE relation set ([`RelationSet`], closure.md section 12) named `name` at `slot`.
/// The set holds EXACTLY the certified keys, each with its run-1 right-hand-side keys and relation digest, verdict
/// bits [`VERDICT_CERTIFIED`] (the chain checked regular, descending under the full comparator, exact); its domain is
/// the recipe's case domain. So the closure's dispatch rule "matches iff in domain AND certified, else falls through"
/// equals acceptance's [`accept::recipe::RecipeEntry::matches`]. Refused: a verdict that is not `Certified`, of
/// another recipe digest, answers of another recipe, a certified key without an ok run-1 answer, and answers whose
/// canonical relation-set text does not reproduce the verdict's `relation_set_digest` (not the answers certified).
/// Slot: acceptance composes (before, recipe, after): use `DispatchSlot::BeforeTable(<first table of after>)`, or
/// `DispatchSlot::Last` when nothing follows.
pub fn relation_set_from_accepted_recipe(
    name: &str,
    recipe: &accept::recipe::Recipe,
    verdict: &RecipeVerdict,
    answers: &SolveAnswers,
    codec: KeyCodec,
    slot: DispatchSlot,
) -> Result<RelationSet, GlueError> {
    let RecipeVerdict::Certified {
        recipe_digest,
        certified,
        relation_set_digest,
        ..
    } = verdict
    else {
        return Err(GlueError::NotCertified(format!("{verdict:?}")));
    };
    if *recipe_digest != recipe.digest() || answers.recipe_digest != recipe.digest() {
        return Err(GlueError::NotCertified(format!(
            "recipe {} / verdict {recipe_digest} / answers {}",
            recipe.digest(), answers.recipe_digest
        )));
    }
    let mut b = RelationStoreBuilder::new(codec);
    let mut rel = String::new();
    for k in certified {
        let a = answers
            .get(k, 1)
            .filter(|a| a.ok)
            .ok_or_else(|| GlueError::MissingAnswer(k.clone()))?;
        let d = a
            .relation_digest
            .as_deref()
            .ok_or_else(|| GlueError::MissingAnswer(k.clone()))?;
        rel.push_str(&a.line);
        rel.push('\n');
        b.push(k, &a.rhs_keys, relation_digest_bytes(d)?, VERDICT_CERTIFIED)
            .map_err(|e| GlueError::KeySet(format!("{e:?}")))?;
    }
    let recomputed = sha256_tag(rel.as_bytes());
    if recomputed != *relation_set_digest {
        return Err(GlueError::RelationSetMismatch {
            certified: relation_set_digest.clone(),
            recomputed,
        });
    }
    Ok(RelationSet {
        name: name.to_string(),
        recipe_digest: recipe.digest().to_string(),
        domain: recipe.domain().clone(),
        relations: b
            .finish()
            .map_err(|e| GlueError::KeySet(format!("{e:?}")))?,
        slot,
    })
}

/// LADDER recipe domain ([`ladder_recipe::Recipe`], the engine's `rustred.recipe.v1` file) -> core [`CaseDomain`]:
/// since lane ladderfix2 (re-audit A M6) the recipe builds its own core domain ([`ladder_recipe::Recipe::case_domain`],
/// THE dispatcher semantics: chart test, fixed slots, equations, exception branches, bounds); this is that call with
/// the glue error type.
pub fn case_domain_of_ladder_recipe(r: &ladder_recipe::Recipe) -> Result<CaseDomain, GlueError> {
    r.case_domain().map_err(GlueError::Domain)
}

/// LADDER accepted recipe rule ([`ladder_recipe::RecipeRule`]: recipe, certified keys, per-key certificates) ->
/// CLOSURE relation set named `name` at `slot`: exactly the certified keys with their recorded right-hand-side keys
/// and relation digests, verdict bits [`VERDICT_CERTIFIED`] (the ladder certifies a key only if regular, descending
/// and exact), domain from [`case_domain_of_ladder_recipe`]. Refused: records that are not exactly the certified keys.
pub fn relation_set_from_ladder_recipe(
    name: &str,
    rule: &ladder_recipe::RecipeRule,
    codec: KeyCodec,
    slot: DispatchSlot,
) -> Result<RelationSet, GlueError> {
    let mut rec_keys: Vec<&Key> = rule.records().iter().map(|c| &c.key).collect();
    rec_keys.sort();
    let certified = rule.certified();
    let mut cert: Vec<&Key> = certified.iter().collect();
    cert.sort();
    if rec_keys != cert {
        return Err(GlueError::RecordsMismatch(format!(
            "{} records for {} certified keys",
            rec_keys.len(),
            cert.len()
        )));
    }
    let mut b = RelationStoreBuilder::new(codec);
    for c in rule.records() {
        b.push(
            &c.key,
            &c.rhs_keys,
            relation_digest_bytes(&c.relation_digest)?,
            VERDICT_CERTIFIED,
        )
        .map_err(|e| GlueError::KeySet(format!("{e:?}")))?;
    }
    Ok(RelationSet {
        name: name.to_string(),
        recipe_digest: rule.recipe_digest().to_string(),
        domain: case_domain_of_ladder_recipe(rule.recipe())?,
        relations: b
            .finish()
            .map_err(|e| GlueError::KeySet(format!("{e:?}")))?,
        slot,
    })
}

// ------------------------------------------------------------------------------------------------------------------
// Witness status and the witness-absent policy

/// ACCEPTANCE witness status of a record ([`RuleText::witness_status`] + guard version, acceptance.md section 11) ->
/// core [`cover::WitnessStatus`] (campaign-core-domain.md section 8), the STRICT combination: `Elided` -> `Elided`;
/// loci present AND guard version >= 2 -> `Complete`; loci absent, or present with a pre-guard record ->
/// `NotCarried` (credit only with per-key exact engine facts, as acceptance's check 0). The core's own loader reads
/// only the literal "elided" and ignores the guard version (more permissive: listed in the int3 notes).
pub fn core_witness_status(rule: &RuleText) -> cover::WitnessStatus {
    match (rule.witness_status(), rule.guard_version.unwrap_or(0) >= 2) {
        (accept::rule::WitnessStatus::Elided, _) => cover::WitnessStatus::Elided,
        (accept::rule::WitnessStatus::Present { .. }, true) => cover::WitnessStatus::Complete,
        _ => cover::WitnessStatus::NotCarried,
    }
}

/// ACCEPTANCE regularity back-end ([`accept::facts::Regularity`]) -> CLOSURE witness-absent policy ([`WitnessAbsent`],
/// closure.md 11.6a). Both lanes implement the coordinator's ruling "a record without witness loci credits a key only
/// with a per-key EXACT ENGINE fact": a back-end that declares `engine_fact_digest()` may lift
/// (`LiftWithRegularFact`; the closure's fact source must then answer `witness_lift` from the same engine facts);
/// facts computed from the record cannot see hidden poles -> `Block`. (The LADDER blocks `NotCarried` unconditionally
/// in `judge_point`: its equivalent is `Block`.)
pub fn closure_witness_policy(facts: &dyn accept::facts::Regularity) -> WitnessAbsent {
    if facts.engine_fact_digest().is_some() {
        WitnessAbsent::LiftWithRegularFact
    } else {
        WitnessAbsent::Block
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closure::{ClosureConfig, HandDowns, KeySet, NoFacts, RuleBodies, Tier, closure};
    use crate::cover::{Dispatcher, Table};
    use crate::order::NodeComparator;
    use std::collections::BTreeMap;

    const CHART: [bool; 4] = [true, true, false, false];

    fn window() -> Vec<Key> {
        let mut out = vec![];
        for a in -1..=4 {
            for b in -1..=4 {
                for c in -2..=1 {
                    out.push(vec![a, b, c, 0]);
                }
            }
        }
        out
    }

    fn eq(coeffs: &[(usize, i64)], c: i64) -> AffineEq {
        AffineEq::try_new(coeffs.to_vec(), c).unwrap().unwrap()
    }

    /// Round trip of the target (Key exact; Locus via the engine spelling and back) and the reason table.
    #[test]
    fn ladder_to_accept_exclusion_round_trip_and_reasons() {
        let locus = vec![eq(&[(0, 1), (1, -2)], 1), eq(&[(2, 1)], 0)];
        let e = ladder::Exclusion {
            target: ladder::ExclusionTarget::Locus {
                equations: locus.clone(),
            },
            reason: ladder::ExclusionReason::OnPole,
        };
        let a = accept_exclusion_from_ladder(&e).unwrap();
        assert_eq!(a.reason, accept::FailureKind::Blocked);
        let accept::ExclusionTarget::Locus { equations } = &a.target else {
            panic!("{a:?}")
        };
        let back: Vec<AffineEq> = equations
            .iter()
            .map(|s| AffineEq::parse(s, 4).unwrap())
            .collect();
        assert_eq!(back, locus);
        for k in window() {
            let holds_back = back.iter().all(|x| x.holds(&k));
            assert_eq!(e.target.holds(&k), holds_back, "{k:?}");
        }
        let k = ladder::Exclusion {
            target: ladder::ExclusionTarget::Key(vec![2, 1, 0, 0]),
            reason: ladder::ExclusionReason::NotDescending {
                rhs: vec![3, 1, 0, 0],
            },
        };
        let a = accept_exclusion_from_ladder(&k).unwrap();
        assert_eq!(
            (a.target, a.reason),
            (
                accept::ExclusionTarget::Key(vec![2, 1, 0, 0]),
                accept::FailureKind::NotDescending
            )
        );
        for r in [
            ladder::ExclusionReason::IdentityUnknown,
            ladder::ExclusionReason::NotCreditedHere,
            ladder::ExclusionReason::PreferredTerminal,
            ladder::ExclusionReason::CoverageUnknown { why: "x".into() },
        ] {
            let e = ladder::Exclusion {
                target: ladder::ExclusionTarget::Key(vec![1, 1, 0, 0]),
                reason: r,
            };
            assert!(matches!(
                accept_exclusion_from_ladder(&e),
                Err(GlueError::NoAcceptKind(_))
            ));
        }
    }

    /// Two free tables A (first) and B: the converted exclusions make the closure skip A exactly at the excluded keys
    /// (dispatch continues with B), as both the ladder's and acceptance's effective domains say; loci are refused.
    #[test]
    fn closure_exclusions_agree_with_effective_domains() {
        let mut ta = Table::new("A", 4);
        ta.push(0, CaseDomain::free(4, Some(CHART.to_vec())))
            .unwrap();
        let mut tb = Table::new("B", 4);
        tb.push(7, CaseDomain::free(4, Some(CHART.to_vec())))
            .unwrap();
        let d = Dispatcher::new(vec![ta, tb], vec![0, 1]).unwrap();
        let mut bodies = RuleBodies::new(&d);
        bodies.set(&d, 0, 0, &["I(0,n1,n2,n3)"]).unwrap();
        bodies.set(&d, 1, 0, &["I(0,n1,n2,n3)"]).unwrap();
        let excl_key = vec![2, 1, 0, 0];
        let other_key = vec![3, 1, 0, 0];
        let rule = ClosureRule {
            table: "A",
            record: 0,
            label: 0,
        };
        let from_ladder = closure_exclusions_from_ladder(
            rule,
            &[ladder::Exclusion {
                target: ladder::ExclusionTarget::Key(excl_key.clone()),
                reason: ladder::ExclusionReason::NotCreditedHere,
            }],
        )
        .unwrap();
        let acc = accept::Exclusion {
            target: accept::ExclusionTarget::Key(excl_key.clone()),
            reason: accept::FailureKind::Irregular,
            disproof: "pole".into(),
        };
        let from_accept = closure_exclusions_from_accept(rule, std::slice::from_ref(&acc)).unwrap();
        let from_term = vec![closure_exclusion_from_terminal(
            rule,
            &terminals::KeyExclusion {
                key: excl_key.clone(),
                rule: "A#0".into(),
                reason: "keeps_terminal".into(),
            },
        )];
        assert_eq!(from_term[0].reason, ExclusionReason::KeepsTerminal);
        let codec = KeyCodec::new(4, 6).unwrap();
        let start = KeySet::from_keys(codec, [excl_key.as_slice(), other_key.as_slice()]).unwrap();
        for ex in [from_ladder, from_accept, from_term] {
            let cfg = ClosureConfig {
                tier: Tier::K,
                threads: 1,
                hand_downs: HandDowns::Keys,
                exclusions: ex,
                ..Default::default()
            };
            let cmp = NodeComparator::legacy(4, None).unwrap();
            let c = closure(&d, &bodies, &cmp, &start, &cfg, &NoFacts).unwrap();
            let w = |k: &Key| c.winner(c.index_of(k).unwrap()).unwrap().table;
            assert_eq!(w(&excl_key), "B");
            assert_eq!(w(&other_key), "A");
        }
        let locus = accept::Exclusion {
            target: accept::ExclusionTarget::Locus {
                equations: vec!["n0-2".into()],
            },
            reason: accept::FailureKind::Blocked,
            disproof: String::new(),
        };
        assert!(matches!(
            closure_exclusions_from_accept(rule, &[acc, locus]),
            Err(GlueError::LocusNotPerKey { .. })
        ));
    }

    /// A certified ladder recipe rule built the only way there is (lane ladderfix2: private fields): through
    /// `accept_recipe` with a scripted per-key solver that solves exactly `certified`.
    fn ladder_rule(certified: &[Key]) -> ladder_recipe::RecipeRule {
        use crate::worklist::items::{FullAttemptEnded, FullAttemptEvidence, NodeParams};
        let recipe = ladder_recipe::Recipe {
            schema: ladder_recipe::RECIPE_SCHEMA.into(),
            id: "r".into(),
            arity: 4,
            chart: "1100".into(),
            domain: ladder_recipe::RecipeDomain {
                fixed: vec![None, None, Some(0), None],
                equations: vec![eq(&[(0, 1), (1, -1)], -1).to_engine()],
                exceptions: vec![vec![eq(&[(0, 1)], -3).to_engine()]],
                bounds: None,
            },
            seeds: ladder_recipe::RecipeSeeds {
                unit: ladder_recipe::RECIPE_UNIT.into(),
                centres: vec![],
                generator: ladder_recipe::RecipeGenerator::Ball(ladder_recipe::RecipeBall {
                    radius: 1,
                    down: vec![0, 1],
                }),
                min_offset: BTreeMap::new(),
                max_offset: BTreeMap::new(),
                layer_cap: None,
                sector_filter: false,
                sector_slots: None,
            },
            symmetry_rows: false,
            elimination: ladder_recipe::RecipeElimination {
                convention: ladder_recipe::RECIPE_ELIMINATION.into(),
            },
            provenance: None,
        };
        struct Scripted(Vec<Key>);
        impl crate::repair::CaseSolver for Scripted {
            fn solve(
                &mut self,
                _r: &crate::repair::CaseRequest,
                _b: &crate::worklist::seed::AttemptBudget,
            ) -> crate::worklist::ladder::SolveAnswer {
                unreachable!()
            }
            fn solve_recipe_at_key(
                &mut self,
                _r: &ladder_recipe::Recipe,
                k: &[i32],
            ) -> Result<ladder_recipe::KeyRelation, ladder_recipe::RecipeFailure> {
                if !self.0.iter().any(|c| c.as_slice() == k) {
                    return Err(ladder_recipe::RecipeFailure::NoRelation);
                }
                Ok(ladder_recipe::KeyRelation {
                    key: k.to_vec(),
                    rhs_keys: vec![vec![0, k[1], 0, 0]],
                    relation_digest: format!("blake3:{}", "ab".repeat(32)),
                    regular: crate::worklist::ladder::Tri::Pass,
                    proof_ok: true,
                    determinism_id: Some("det".into()),
                    cost_ms: 1,
                    recipe_digest: Some("eng".into()),
                    binary_digest: None,
                })
            }
        }
        struct Any;
        impl crate::repair::Descent for Any {
            fn lower(&self, _a: &[i32], _b: &[i32]) -> bool {
                true
            }
        }
        let params = NodeParams::with_lines("N", CHART.to_vec());
        let ended = FullAttemptEnded::from_evidence(
            &params,
            FullAttemptEvidence {
                node: "N".into(),
                cpu_ms: u64::MAX / 2,
                fixer_rounds: 99,
                open: vec!["x".into()],
            },
        )
        .unwrap();
        let rule = ladder_recipe::accept_recipe(
            &recipe,
            certified,
            &mut Scripted(certified.to_vec()),
            &Any,
            &params,
            &ended,
        );
        assert_eq!(rule.certified_count(), certified.len());
        rule
    }

    /// The ladder's `RecipeRule::matches` and the converted relation set's closure match (in domain AND certified)
    /// agree on every chart key of a window (domain with a fixed slot, an equation, an exception and an empty branch).
    #[test]
    fn ladder_recipe_matches_equals_relation_set_match() {
        // n0 = n1 + 1, n2 = 0, not n0 = 3: (2,1), (4,3) in domain; (3,2) excepted (only (2,1) can be certified:
        // acceptance refuses a key outside the domain)
        let certified = vec![vec![2, 1, 0, 0]];
        let rule = ladder_rule(&certified);
        let set = relation_set_from_ladder_recipe(
            "rec",
            &rule,
            KeyCodec::new(4, 6).unwrap(),
            DispatchSlot::Last,
        )
        .unwrap();
        let mut n_match = 0;
        for k in window() {
            let in_chart = k
                .iter()
                .zip(CHART)
                .all(|(&x, l)| if l { x >= 1 } else { x <= 0 });
            if !in_chart {
                continue;
            }
            let closure_match = set.in_domain(&k) && set.relations.find(&k).is_some();
            assert_eq!(rule.matches(&k), closure_match, "{k:?}");
            assert_eq!(
                rule.recipe().in_domain(&k),
                set.in_domain(&k),
                "domain {k:?}"
            );
            n_match += closure_match as usize;
        }
        assert_eq!(n_match, 1, "only (2,1): (3,2) is on the exception branch");
        // records and the certified set agree by construction now (private fields, built only by accept_recipe)
        let off = ladder_rule(&[]);
        assert_eq!(off.certified_count(), 0);
    }

    #[test]
    fn relation_digest_forms() {
        let h = "0f".repeat(32);
        assert_eq!(relation_digest_bytes(&h).unwrap(), [0x0f; 32]);
        assert_eq!(
            relation_digest_bytes(&format!("blake3:{h}")).unwrap(),
            [0x0f; 32]
        );
        assert!(relation_digest_bytes("sha256:rel-[1, 2]").is_err());
        assert!(relation_digest_bytes(&"0".repeat(63)).is_err());
    }

    struct EngineFacts;
    impl accept::facts::Regularity for EngineFacts {
        fn regular(&mut self, _: &RuleText, _: &[i32]) -> accept::facts::RegVerdict {
            accept::facts::RegVerdict::Unknown("test".into())
        }
        fn engine_fact_digest(&self) -> Option<String> {
            Some("sha256:facts".into())
        }
    }

    #[test]
    fn witness_policy_follows_the_fact_back_end() {
        assert_eq!(
            closure_witness_policy(&accept::facts::ExactRecordFacts::new()),
            WitnessAbsent::Block
        );
        assert_eq!(
            closure_witness_policy(&EngineFacts),
            WitnessAbsent::LiftWithRegularFact
        );
    }
}
