//! Symmetry credits inside the closure (closure.md 11.6 and 15; lane closurefix: re-audit F1a, F8, R44).
//!
//! The adopted condition itself lives in [`crate::symmetry`] (structural part [`crate::symmetry::element_image`], the
//! Tier C fact checks of one element [`crate::symmetry::try_one`]); this module decides what the CLOSURE does with it:
//! - the image may be served by ANY accepted rule of the node (R44): a table rule, or a CERTIFIED relation-set (recipe)
//!   entry matching the image (in its domain AND in its certified set, ranked before the image's table stop). A recipe
//!   image is exact (no facts): every recorded top key must be strictly below the ORIGINAL key and no recorded key may
//!   carry a positive numerator (both refuse the element, in both tiers); its verdict bits must all be set.
//! - Tier C: the first element not refused (clean or pending) gives the credit (unchanged).
//! - Tier K (keys only, a SUPERSET of every Tier C run): the walk follows the UNION of every element not refused for a
//!   STRUCTURAL reason (no image, image = key, numerator map not pure, image terminal, image not covered / blocked /
//!   witness-absent, no body, recipe unverified or its recorded keys not below / with a positive numerator), whatever
//!   the facts. The key's STATUS is unchanged from the keys-only adopted check: it is credited iff some element passes
//!   it (every top term below the key; positive-numerator terms flagged), the first such element is the credit and
//!   every other candidate an `alternative`; when no element passes but candidates exist, the key stays OPEN
//!   (uncovered) and its candidates are still followed (like a witness-absent key). Fact-dependent conditions never
//!   refuse a candidate in Tier K: its non-lower top terms are followed and counted (`credit_requires_zero`).

use super::internal::CtxView;
use super::{ClosureError, Ctx, TermClass, Tier, VERDICT_CERTIFIED};
use crate::cover::Dispatch;
use crate::symmetry::{self, CreditPolicy, Refusal};

/// What serves the image of a credit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Server {
    /// A table rule (dispatch position), instantiated at the image.
    Rule(u32),
    /// A relation-set entry (set index in the run's dispatch order, record index): its recorded right-hand side.
    Recipe(u32, u32),
}

/// A credit as the closure uses it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CreditC {
    pub element: usize,
    pub image: Vec<i32>,
    pub server: Server,
    /// Tier C, table rule: the image rule's regularity fact is unknown (pending; requested at the image).
    pub regularity_unknown: bool,
    /// Tier K: further structurally admissible elements whose expansions are followed too (superset).
    pub alternatives: Vec<(usize, Vec<i32>, Server)>,
}

impl CreditC {
    /// Every (element, image, server) followed for this credit (primary first).
    pub(super) fn followed(&self) -> impl Iterator<Item = (usize, &[i32], Server)> {
        std::iter::once((self.element, self.image.as_slice(), self.server)).chain(
            self.alternatives
                .iter()
                .map(|(e, i, s)| (*e, i.as_slice(), *s)),
        )
    }
}

/// The exact checks of a recipe-served image (verdict bits, recorded keys below the original key, no positive
/// numerator): `Ok` or the refusal (fact independent, so the same in both tiers).
fn recipe_image_check(ctx: &Ctx, key: &[i32], si: u32, ri: u32) -> Result<(), Refusal> {
    let set = &ctx.rsets[si as usize].1;
    let st = &set.relations;
    if st.verdict(ri as usize) & VERDICT_CERTIFIED != VERDICT_CERTIFIED {
        return Err(Refusal::ImageRecipeUnverified);
    }
    let sw = st.codec().words();
    let mut t = vec![0i32; ctx.arity];
    for (j, c) in st.rhs_codes(ri as usize).chunks_exact(sw).enumerate() {
        st.codec().decode(c, &mut t);
        match ctx.sectors.classify_masked(&t, 0) {
            TermClass::PositiveNumerator { .. } => {
                return Err(Refusal::CoefficientNonZero { term: j });
            }
            TermClass::Top if !ctx.cmp.lower(&t, key) => {
                return Err(Refusal::TermNotBelow {
                    term: j,
                    rhs: t.clone(),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// What the closure does at an UNCOVERED key with a credit policy.
pub(super) enum CreditOutcome {
    /// Credited (Tier K: with the further candidates to follow).
    Credited(CreditC),
    /// Tier K only: no element passes the keys-only check, but these candidates (first = `element`) are followed.
    OpenFollowed(CreditC, usize),
    /// Every element refused (count).
    Refused(usize),
}

/// The closure's credit for an UNCOVERED key (module documentation).
pub(super) fn serve_credit(
    ctx: &Ctx,
    key: &[i32],
    pol: &CreditPolicy,
) -> Result<CreditOutcome, ClosureError> {
    let tier_c = ctx.cfg.tier == Tier::C;
    // (element, image, server, regularity unknown (Tier C) / passes the keys-only check (Tier K))
    let mut cands: Vec<(usize, Vec<i32>, Server, bool)> = vec![];
    let mut refusals = vec![];
    for ei in 0..pol.group().len() {
        let (image, d) = match symmetry::element_image(key, ei, pol, ctx) {
            Ok(x) => x,
            Err(r) => {
                refusals.push((ei, r));
                continue;
            }
        };
        if let Some((si, ri)) = ctx.recipe_match(&image, &d) {
            match recipe_image_check(ctx, key, si, ri) {
                Ok(()) => cands.push((ei, image, Server::Recipe(si, ri), !tier_c)),
                Err(r) => {
                    refusals.push((ei, r));
                    continue;
                }
            }
        } else if tier_c {
            match symmetry::try_one(key, ei, pol, ctx)? {
                Ok(c) => {
                    let reg = c.undecided.contains(&Refusal::RegularityUnknown);
                    cands.push((ei, c.image, Server::Rule(c.position), reg));
                }
                Err(r) => {
                    refusals.push((ei, r));
                    continue;
                }
            }
        } else {
            match d {
                Dispatch::Covered(r) if ctx.bodies.has(r.table, r.record) => {
                    // the keys-only adopted check (non-lower top terms refuse it; facts are never consulted)
                    let clean = symmetry::try_one(key, ei, pol, ctx)?.is_ok();
                    cands.push((ei, image, Server::Rule(ctx.position(r)), clean));
                }
                Dispatch::Covered(_) => {
                    refusals.push((ei, Refusal::NoBody));
                    continue;
                }
                // an image blocked only for missing per-key evidence (elided / witness loci not carried) under the
                // per-key lift: structurally admissible (a Tier C run with the per-key proof at the image credits it,
                // closure.md 17), so the superset follows it; it passes the keys-only check only when the proof is bound
                Dispatch::Blocked { at, reason }
                    if ctx.cfg.per_key_lift == super::PerKeyLift::Proof
                        && matches!(
                            reason,
                            crate::domain::Verdict::WitnessNotCarried | crate::domain::Verdict::Elided
                        )
                        && ctx.bodies.has(at.table, at.record) =>
                {
                    let clean = symmetry::try_one(key, ei, pol, ctx)?.is_ok();
                    cands.push((ei, image, Server::Rule(ctx.position(at)), clean));
                }
                d => {
                    refusals.push((ei, Refusal::ImageNotCovered(d)));
                    continue;
                }
            }
        }
        if tier_c {
            // the first element not refused, clean or PENDING: the choice never depends on which facts are known
            break;
        }
    }
    if cands.is_empty() {
        return Ok(CreditOutcome::Refused(refusals.len()));
    }
    if tier_c {
        let (element, image, server, regularity_unknown) = cands.swap_remove(0);
        return Ok(CreditOutcome::Credited(CreditC {
            element,
            image,
            server,
            regularity_unknown,
            alternatives: vec![],
        }));
    }
    // Tier K: the primary is the first element passing the keys-only check (else the key stays open), every other
    // candidate is followed as well
    let n_refused = refusals.len() + cands.iter().filter(|c| !c.3).count();
    let first_clean = cands.iter().position(|c| c.3);
    let p = first_clean.unwrap_or(0);
    let (element, image, server, _) = cands.remove(p);
    let c = CreditC {
        element,
        image,
        server,
        regularity_unknown: false,
        alternatives: cands.into_iter().map(|(e, i, s, _)| (e, i, s)).collect(),
    };
    Ok(match first_clean {
        Some(_) => CreditOutcome::Credited(c),
        None => CreditOutcome::OpenFollowed(c, n_refused),
    })
}
