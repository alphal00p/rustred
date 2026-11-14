//! Symmetry credits (plan 2.4.6; DESIGN-COMMENTS A1, A7; COORDINATOR_REQUIREMENTS R8, R9). Interface:
//! plan/auto-campaign/interfaces/closure.md section 3 and 11.
//!
//! THE ADOPTED CONDITION (the only acceptance condition): a key k without a rule may be served by an image g k
//! (g in the node's automorphism group, applied per POINT by [`GroupElement::apply`]; a key with a non-zero
//! numerator and no numerator map has no image; a key with a non-zero numerator is credited only through an element
//! whose numerator map is certified PURE, i.e. a one-term identity: [`CreditPolicy::pure_numerator_maps`]) ONLY IF
//!  (i) the image is not a declared terminal, and the rule R that the dispatch (over the tables WITHOUT credits, with
//!      the rule-level key exclusions) selects at g k COVERS g k (never an image of an image, never "any rule that
//!      covers g k": the first-match winner);
//!  (ii) EVERY followed top-sector right-hand-side key of R instantiated at g k is strictly below the ORIGINAL k under
//!      the node comparator (the RHS key itself, the integral it names: the DIRECT comparison; audit loop-A ruling);
//!  (iii) the checks of the CLOSURE's tier (there is no separate credit tier, audit loop-A C2): Tier C: R regular at
//!      g k by a fact, every dropped term of R at g k justified by its coefficient-zero fact, a positive-numerator or
//!      non-lower top term with a NonZero fact refuses the element; with Unknown facts the credit is PENDING (its
//!      unknowns are counted undecided and requested by the closure, never `closed`). Tier K: positive-numerator terms
//!      flagged "requires coefficient zero", regularity not checked (Tier K is never closed).
//! NOT "image lower than key", NOT "least orbit member", NOT "mapped back" (g^-1 of the RHS key: a test-only
//! diagnostic, [`CreditChecker::serve_mapped_back_diagnostic`], never an acceptance path). The first group element
//! (in the given order) that passes without unknowns gives the credit; else the first pending one; every credit names
//! the group element, the image and the rule. ASSUMPTION ON THE CONSUMER (recorded in closure.md): the importer
//! applies the credit I(k) = I(g k) and the image's rule R as ONE step (the step k -> RHS(R at g k) is what descends).

use std::collections::BTreeSet;

use crate::closure::internal::{CtxView, StandaloneCtx};
use crate::closure::{
    ClosureError, CoefFact, FactSource, FastDispatch, RegFact, RuleBodies, Sectors, TermClass,
    Tier,
};
use crate::cover::{Dispatch, Dispatcher, RuleRef};
use crate::order::NodeComparator;

pub use crate::order::GroupElement;

/// How the right-hand-side keys at g k are compared with k. Only `Direct` is an acceptance condition; `MappedBack`
/// exists for the test-only diagnostic [`CreditChecker::serve_mapped_back_diagnostic`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CreditFrame {
    /// The RHS key itself (adopted; reproduces the checker's 75 of 170 on the reference node).
    #[default]
    Direct,
    /// g^-1 applied to the RHS key (the integral's preimage): DIAGNOSTIC ONLY.
    MappedBack,
}

/// The credit policy of a node: the group and which elements have a certified pure numerator map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreditPolicy {
    pub group: Vec<GroupElement>,
    /// Indices of elements whose numerator map (`numerators: Some`) is CERTIFIED a one-term identity by the group
    /// data. A key with a non-zero numerator is never credited through any other element (plan 2.4.6 (iii)).
    pub pure_numerator_maps: BTreeSet<usize>,
}

impl CreditPolicy {
    /// A policy with no certified numerator maps (numerator-free keys only, unless maps are certified later).
    pub fn new(group: Vec<GroupElement>) -> CreditPolicy {
        CreditPolicy {
            group,
            pure_numerator_maps: BTreeSet::new(),
        }
    }
    /// Check the policy against the node's line mask: every element's line block must be the line slots
    /// `0..lines.len()` of the mask (audit loop-A m1), and certified indices must exist.
    pub fn validate(&self, lines: &[bool]) -> Result<(), String> {
        for (i, g) in self.group.iter().enumerate() {
            let nl = g.lines.len();
            if nl > lines.len() || lines.iter().enumerate().any(|(s, &l)| l != (s < nl)) {
                return Err(format!(
                    "group element {i}: line block 0..{nl} is not the line mask of the node"
                ));
            }
        }
        if let Some(&i) = self.pure_numerator_maps.iter().find(|&&i| i >= self.group.len()) {
            return Err(format!("certified numerator map of element {i}: no such element"));
        }
        Ok(())
    }
}

/// A symmetry credit: k is served by `rule` at `image` = g_element k.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credit {
    pub element: usize,
    pub image: Vec<i32>,
    pub rule: RuleRef,
    /// Dispatch position of `rule`.
    pub position: u32,
    /// Distinct followed top-sector RHS keys (strictly below k, or pending a coefficient fact).
    pub top_terms: Vec<Vec<i32>>,
    /// Indices of positive-numerator terms flagged "requires coefficient zero" (Tier K).
    pub zero_required: Vec<usize>,
    /// Tier C: the unknown facts this credit still needs (pending credit; empty = decided).
    pub undecided: Vec<Refusal>,
}

/// Why a group element does not give a credit (or, in [`Credit::undecided`], what a pending credit still needs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No image (non-zero numerator and no numerator map, or a malformed element).
    NoImage,
    ImageEqualsKey,
    /// The group element does not fit the key (core `GroupElement::apply` error).
    MalformedElement(String),
    /// The key has a non-zero numerator and the element's numerator map is not certified pure.
    NumeratorMapNotPure,
    /// The image is a declared terminal (never reduced by a rule; audit loop-A m3).
    ImageIsTerminal,
    /// The dispatch at g k is not `Covered`.
    ImageNotCovered(Dispatch),
    /// A relation set (recipe) serves g k: credits through recipe-served images are not implemented yet.
    ImageServedByRecipe,
    NoBody,
    TermNotBelow {
        term: usize,
        rhs: Vec<i32>,
    },
    CoefficientNonZero {
        term: usize,
    },
    Irregular,
    /// Pending (Tier C): a positive-numerator term without a coefficient fact.
    CoefficientUnknown {
        term: usize,
    },
    /// Pending (Tier C): a top term that is not below k, without a coefficient fact.
    TermNotBelowUnknown {
        term: usize,
        rhs: Vec<i32>,
    },
    /// Pending (Tier C): no regularity fact for R at g k.
    RegularityUnknown,
}

/// g^-1 applied to `key` (inverse of [`GroupElement::apply`]).
fn apply_inverse(g: &GroupElement, key: &[i32]) -> Option<Vec<i32>> {
    let nl = g.lines.len();
    let mut out = key.to_vec();
    for (i, &j) in g.lines.iter().enumerate() {
        out[i] = *key.get(j)?;
    }
    match &g.numerators {
        Some(m) => {
            for (i, &j) in m.iter().enumerate() {
                out[nl + i] = *key.get(nl + j)?;
            }
        }
        None => {
            if key[nl..].iter().any(|&x| x != 0) {
                return None;
            }
        }
    }
    Some(out)
}

/// The adopted condition for one element (see the module documentation). `frame` is `Direct` except in the
/// diagnostic.
fn try_element(
    key: &[i32],
    ei: usize,
    pol: &CreditPolicy,
    ctx: &dyn CtxView,
    frame: CreditFrame,
) -> Result<Result<Credit, Refusal>, ClosureError> {
    let g = &pol.group[ei];
    if g.lines.len() > key.len() {
        return Ok(Err(Refusal::NoImage));
    }
    let image = match g.apply(key) {
        Ok(Some(x)) => x,
        Ok(None) => return Ok(Err(Refusal::NoImage)),
        Err(e) => return Ok(Err(Refusal::MalformedElement(format!("{e:?}")))),
    };
    if image == key {
        return Ok(Err(Refusal::ImageEqualsKey));
    }
    let lines = &ctx.sectors().lines;
    let has_numerator = key
        .iter()
        .zip(lines)
        .any(|(&v, &l)| !l && v != 0);
    if has_numerator && !pol.pure_numerator_maps.contains(&ei) {
        return Ok(Err(Refusal::NumeratorMapNotPure));
    }
    if ctx.is_terminal(&image) {
        return Ok(Err(Refusal::ImageIsTerminal));
    }
    let d = ctx.dispatch(&image);
    if ctx.recipe_serves(&image, &d) {
        // the image is served by a relation set (R48), not by the table rule: credits through recipe-served images
        // are not implemented yet (refused, fail closed)
        return Ok(Err(Refusal::ImageServedByRecipe));
    }
    let rule = match d {
        Dispatch::Covered(r) => r,
        d => return Ok(Err(Refusal::ImageNotCovered(d))),
    };
    let pos = ctx.position(rule);
    let (mut buf, mut cm) = (vec![], vec![]);
    let Some(nt) = ctx.instantiate(pos, &image, &mut buf, &mut cm)? else {
        return Ok(Err(Refusal::NoBody));
    };
    let tier_c = ctx.tier() == Tier::C;
    let mut undecided = vec![];
    if tier_c {
        match ctx.facts().regularity(&ctx.rule_id(pos), &image) {
            RegFact::Regular => {}
            RegFact::Pole => return Ok(Err(Refusal::Irregular)),
            RegFact::Unknown => {
                if !ctx.relax_regularity() {
                    undecided.push(Refusal::RegularityUnknown);
                }
            }
        }
    }
    let n = ctx.arity();
    let mut top = vec![];
    let mut zero_required = vec![];
    for j in 0..nt {
        let t = &buf[j * n..(j + 1) * n];
        let fact = if tier_c {
            ctx.facts().coefficient(&ctx.rule_id(pos), &image, j, t)
        } else {
            CoefFact::Unknown
        };
        if fact == CoefFact::Zero {
            continue;
        }
        match ctx.sectors().classify_masked(t, cm[j]) {
            TermClass::PositiveNumerator { .. } => {
                if !tier_c {
                    zero_required.push(j);
                } else if fact == CoefFact::NonZero {
                    return Ok(Err(Refusal::CoefficientNonZero { term: j }));
                } else {
                    undecided.push(Refusal::CoefficientUnknown { term: j });
                }
            }
            TermClass::Top => {
                let cmpd = match frame {
                    CreditFrame::Direct => Some(t.to_vec()),
                    CreditFrame::MappedBack => apply_inverse(g, t),
                };
                let below = cmpd.is_some_and(|c| ctx.cmp().lower(&c, key));
                if !below {
                    if !tier_c || fact == CoefFact::NonZero {
                        return Ok(Err(Refusal::TermNotBelow {
                            term: j,
                            rhs: t.to_vec(),
                        }));
                    }
                    undecided.push(Refusal::TermNotBelowUnknown {
                        term: j,
                        rhs: t.to_vec(),
                    });
                }
                top.push(t.to_vec());
            }
            TermClass::Subsector { .. } | TermClass::ZeroSector { .. } => {}
        }
    }
    top.sort();
    top.dedup();
    Ok(Ok(Credit {
        element: ei,
        image,
        rule,
        position: pos,
        top_terms: top,
        zero_required,
        undecided,
    }))
}

/// Internal entry used by the closure (dispatch through its index): the first element that passes without unknowns,
/// else the first pending one, else every element's refusal.
pub(crate) fn serve_with(
    key: &[i32],
    pol: &CreditPolicy,
    ctx: &dyn CtxView,
) -> Result<Result<Credit, Vec<(usize, Refusal)>>, ClosureError> {
    serve_frame(key, pol, ctx, CreditFrame::Direct)
}

fn serve_frame(
    key: &[i32],
    pol: &CreditPolicy,
    ctx: &dyn CtxView,
    frame: CreditFrame,
) -> Result<Result<Credit, Vec<(usize, Refusal)>>, ClosureError> {
    let mut refusals = vec![];
    let mut pending: Option<Credit> = None;
    for ei in 0..pol.group.len() {
        match try_element(key, ei, pol, ctx, frame)? {
            Ok(c) if c.undecided.is_empty() => return Ok(Ok(c)),
            Ok(c) => {
                if pending.is_none() {
                    pending = Some(c);
                }
            }
            Err(r) => refusals.push((ei, r)),
        }
    }
    Ok(match pending {
        Some(c) => Ok(c),
        None => Err(refusals),
    })
}

/// The adopted symmetry-credit condition at one key (standalone; the closure uses the same function through its
/// dispatch index), with the checks of `tier`. Fact identities: the table's `provenance.source` (the EC-2 table path)
/// and rule digests without a table coefficient-file digest (`unrecorded`). No declared terminals, no exclusions.
/// `sectors` gives the line mask (and zero sectors). Returns the credit, or every element's refusal.
#[allow(clippy::too_many_arguments)]
pub fn serve_by_image(
    key: &[i32],
    policy: &CreditPolicy,
    disp: &Dispatcher,
    bodies: &RuleBodies,
    cmp: &NodeComparator,
    sectors: &Sectors,
    facts: &dyn FactSource,
    tier: Tier,
) -> Result<Result<Credit, Vec<(usize, Refusal)>>, ClosureError> {
    if key.len() != disp.arity() {
        return Err(ClosureError::KeyArity {
            expected: disp.arity(),
            got: key.len(),
        });
    }
    CreditChecker::new(disp, bodies, cmp, sectors.clone(), facts, tier).serve(key, policy)
}

/// A reusable standalone credit checker (builds the dispatch index once; for many keys).
pub struct CreditChecker<'a> {
    ctx: StandaloneCtx<'a>,
}

impl<'a> CreditChecker<'a> {
    /// A checker with the checks of `tier` (see [`serve_by_image`]).
    pub fn new(
        disp: &'a Dispatcher,
        bodies: &'a RuleBodies,
        cmp: &'a NodeComparator,
        sectors: Sectors,
        facts: &'a dyn FactSource,
        tier: Tier,
    ) -> Self {
        let fast = FastDispatch::new(disp);
        let (names, digests) = crate::closure::standalone_rule_ids(disp, bodies, &fast);
        CreditChecker {
            ctx: StandaloneCtx {
                fast,
                disp,
                bodies,
                cmp,
                sectors,
                facts,
                names,
                digests,
                tier,
            },
        }
    }
    pub fn serve(
        &self,
        key: &[i32],
        policy: &CreditPolicy,
    ) -> Result<Result<Credit, Vec<(usize, Refusal)>>, ClosureError> {
        if let Err(e) = policy.validate(&self.ctx.sectors.lines) {
            return Err(ClosureError::Config(e));
        }
        serve_with(key, policy, &self.ctx)
    }
    /// TEST-ONLY DIAGNOSTIC (not an acceptance condition, audit loop-A ruling on D1): as [`Self::serve`] but the
    /// top terms are compared mapped back (g^-1 of the RHS key) with k.
    pub fn serve_mapped_back_diagnostic(
        &self,
        key: &[i32],
        policy: &CreditPolicy,
    ) -> Result<Result<Credit, Vec<(usize, Refusal)>>, ClosureError> {
        serve_frame(key, policy, &self.ctx, CreditFrame::MappedBack)
    }
    /// The first-match dispatch (the same as `Dispatcher::first_match`).
    pub fn dispatch(&self, key: &[i32]) -> Dispatch {
        self.ctx.dispatch(key)
    }
}
