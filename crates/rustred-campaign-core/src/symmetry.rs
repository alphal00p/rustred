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
//! (in the given order) that is not refused gives the credit (clean, or pending on unknown Tier C facts); every credit names
//! the group element, the image and the rule. ASSUMPTION ON THE CONSUMER (recorded in closure.md): the importer
//! applies the credit I(k) = I(g k) and the image's rule R as ONE step (the step k -> RHS(R at g k) is what descends).

use std::collections::BTreeSet;

use crate::closure::internal::{CtxView, StandaloneCtx};
use crate::closure::{
    ClosureError, CoefFact, FactSource, FastDispatch, RegFact, RuleBodies, Sectors, TermClass, Tier,
};
use crate::cover::{Dispatch, Dispatcher, RuleRef};
use crate::order::NodeComparator;
use crate::replay_base::algebra::Q;
use crate::replay_base::symrows::{SlotImage, VerifiedGroup};

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

/// What the group of a [`CreditPolicy`] rests on (directing session 2026-11-14; ladderfix2 finding 3 / B-M2 class).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupBasis {
    /// The identity-test lane's unforgeable [`VerifiedGroup`] (every element proved exactly on the family): the only
    /// basis on which a closure's credits can count toward a closure verdict.
    Verified {
        digest: String,
        family_digest: String,
        provenance: String,
    },
    /// Hand-built elements (toy nodes, the standalone diagnostic checker): a closure using such a policy is stamped
    /// `diagnostic` (relaxed check `credits_group_unverified`), never `closed`.
    Unverified { source: String },
}

/// The credit policy of a node: the group, which elements have a certified pure numerator map, and what the group rests
/// on. Fields are PRIVATE (lane closurefix): a verified policy comes only out of [`CreditPolicy::from_verified`], and
/// every other constructor gives an `Unverified` basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreditPolicy {
    group: Vec<GroupElement>,
    /// Indices of elements whose numerator map (`numerators: Some`) is CERTIFIED a one-term identity by the group
    /// data. A key with a non-zero numerator is never credited through any other element (plan 2.4.6 (iii)).
    pure_numerator_maps: BTreeSet<usize>,
    basis: GroupBasis,
}

impl CreditPolicy {
    /// An UNVERIFIED policy (hand-built group) with no certified numerator maps: credits from it make a closure
    /// `diagnostic`. Use [`CreditPolicy::from_verified`] for a closure verdict.
    pub fn new(group: Vec<GroupElement>) -> CreditPolicy {
        CreditPolicy::unverified(group, BTreeSet::new(), "hand-built")
    }
    /// An UNVERIFIED policy with the given numerator maps declared pure (tests, diagnostics).
    pub fn unverified(
        group: Vec<GroupElement>,
        pure_numerator_maps: BTreeSet<usize>,
        source: &str,
    ) -> CreditPolicy {
        CreditPolicy {
            group,
            pure_numerator_maps,
            basis: GroupBasis::Unverified {
                source: source.to_string(),
            },
        }
    }
    /// The same elements with these numerator maps declared pure: always an UNVERIFIED policy (declaring maps pure is
    /// not a proof; a verified policy derives them from the verified action).
    pub fn with_pure_numerator_maps(self, pure: BTreeSet<usize>) -> CreditPolicy {
        let source = match &self.basis {
            GroupBasis::Verified { digest, .. } => {
                format!("{digest} with caller-declared numerator maps")
            }
            GroupBasis::Unverified { source } => source.clone(),
        };
        CreditPolicy::unverified(self.group, pure, &source)
    }
    /// THE constructor for closure credits: the elements of the identity-test lane's [`VerifiedGroup`] (same element
    /// indices; the identity is not an element), in the node's frame. Each element's action on the LINE slots must be
    /// a permutation of the line slots (an exact image without sign, or a one-term linear image with coefficient 1
    /// and no constant); otherwise `Err`. Its numerator map is given (and certified pure) iff every other slot has
    /// such an image onto another non-line slot; else it has none (it then acts on numerator-free keys only).
    /// Convention (as the rows' key action): `D_i(kT) = D_t(k)` gives `I(a) = I(b)` with `b[t] = a[i]`.
    pub fn from_verified(g: &VerifiedGroup, lines: &[bool]) -> Result<CreditPolicy, String> {
        let sg = g.group();
        if sg.arity != lines.len() || sg.chart.as_slice() != lines {
            return Err(format!(
                "verified group chart {:?} (arity {}) is not the node's line mask {:?}",
                sg.chart, sg.arity, lines
            ));
        }
        let (ls, os) = slot_lists(lines);
        let pos_l: Vec<Option<usize>> = (0..lines.len())
            .map(|s| ls.iter().position(|&x| x == s))
            .collect();
        let pos_o: Vec<Option<usize>> = (0..lines.len())
            .map(|s| os.iter().position(|&x| x == s))
            .collect();
        let exact = |a: Option<&SlotImage>| -> Option<usize> {
            match a? {
                SlotImage::Exact {
                    target,
                    flip: false,
                } => Some(*target),
                SlotImage::Linear { terms, constant }
                    if constant.is_zero() && terms.len() == 1 && terms[0].1 == Q::ONE =>
                {
                    Some(terms[0].0)
                }
                _ => None,
            }
        };
        let mut group = Vec::with_capacity(sg.elements.len());
        let mut pure = BTreeSet::new();
        for (ei, e) in sg.elements.iter().enumerate() {
            let lperm: Vec<usize> = ls
                .iter()
                .map(|&s| {
                    exact(e.action.get(s))
                        .and_then(|t| pos_l.get(t).copied().flatten())
                        .ok_or_else(|| {
                            format!(
                                "verified element {ei}: line slot {s} has no exact image on a line"
                            )
                        })
                })
                .collect::<Result<_, _>>()?;
            let nperm: Option<Vec<usize>> = os
                .iter()
                .map(|&s| exact(e.action.get(s)).and_then(|t| pos_o.get(t).copied().flatten()))
                .collect();
            let nperm = nperm.filter(|p| {
                let mut seen = vec![false; p.len()];
                p.iter().all(|&j| !std::mem::replace(&mut seen[j], true))
            });
            if nperm.is_some() {
                pure.insert(ei);
            }
            group.push(GroupElement {
                lines: lperm,
                numerators: nperm,
            });
        }
        let pol = CreditPolicy {
            group,
            pure_numerator_maps: pure,
            basis: GroupBasis::Verified {
                digest: g.digest().to_string(),
                family_digest: g.family_digest().to_string(),
                provenance: format!("{:?}", g.provenance()),
            },
        };
        pol.validate(lines)?;
        Ok(pol)
    }
    pub fn group(&self) -> &[GroupElement] {
        &self.group
    }
    pub fn pure_numerator_maps(&self) -> &BTreeSet<usize> {
        &self.pure_numerator_maps
    }
    pub fn basis(&self) -> &GroupBasis {
        &self.basis
    }
    pub fn is_verified(&self) -> bool {
        matches!(self.basis, GroupBasis::Verified { .. })
    }
    /// Check the policy against the node's line mask (audit loop-A m1, loop-B m2): an element's `lines` permutes the
    /// LINE slots of the mask (in slot order, wherever they sit: the action is generic over the frame, L8) and its
    /// `numerators` (if given) the remaining slots; both must be permutations of the right length. Certified indices
    /// must exist.
    pub fn validate(&self, lines: &[bool]) -> Result<(), String> {
        let nl = lines.iter().filter(|&&l| l).count();
        let nn = lines.len() - nl;
        let perm = |v: &[usize], n: usize| {
            let mut seen = vec![false; n];
            v.len() == n
                && v.iter()
                    .all(|&j| j < n && !std::mem::replace(&mut seen[j], true))
        };
        for (i, g) in self.group.iter().enumerate() {
            if !perm(&g.lines, nl) {
                return Err(format!(
                    "group element {i}: lines is not a permutation of the node's {nl} line slots"
                ));
            }
            if let Some(m) = &g.numerators
                && !perm(m, nn)
            {
                return Err(format!(
                    "group element {i}: numerators is not a permutation of the node's {nn} other slots"
                ));
            }
        }
        if let Some(&i) = self
            .pure_numerator_maps
            .iter()
            .find(|&&i| i >= self.group.len())
        {
            return Err(format!(
                "certified numerator map of element {i}: no such element"
            ));
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
    /// A relation set (recipe) serves g k: not handled by this element path (the closure credits through a certified
    /// recipe-served image itself, closure::credit, R44 / re-audit F8).
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
    /// A relation set (recipe) matches g k but its record's verdict bits are not all set (lane closurefix, R44/F8).
    ImageRecipeUnverified,
}

/// The slot lists of a line mask: (line slots, other slots), in slot order.
fn slot_lists(lines: &[bool]) -> (Vec<usize>, Vec<usize>) {
    let l = (0..lines.len()).filter(|&s| lines[s]).collect();
    let o = (0..lines.len()).filter(|&s| !lines[s]).collect();
    (l, o)
}

/// `g k` in the node's frame: `(g k)[L[lines[i]]] = k[L[i]]` over the line slots L of the mask, likewise `numerators`
/// over the other slots; `None` when the key has a non-zero numerator and no numerator map is given (or the element
/// does not fit the mask). Equals the core's `GroupElement::apply` when the lines come first.
pub fn apply_framed(g: &GroupElement, key: &[i32], lines: &[bool]) -> Option<Vec<i32>> {
    let (ls, os) = slot_lists(lines);
    if g.lines.len() != ls.len() || key.len() != lines.len() {
        return None;
    }
    let mut out = key.to_vec();
    for (i, &j) in g.lines.iter().enumerate() {
        out[*ls.get(j)?] = key[ls[i]];
    }
    match &g.numerators {
        Some(m) => {
            if m.len() != os.len() {
                return None;
            }
            for (i, &j) in m.iter().enumerate() {
                out[*os.get(j)?] = key[os[i]];
            }
        }
        None => {
            if os.iter().any(|&s| key[s] != 0) {
                return None;
            }
        }
    }
    Some(out)
}

/// g^-1 applied to `key` in the node's frame (inverse of [`apply_framed`]).
fn apply_inverse(g: &GroupElement, key: &[i32], lines: &[bool]) -> Option<Vec<i32>> {
    let (ls, os) = slot_lists(lines);
    let mut out = key.to_vec();
    for (i, &j) in g.lines.iter().enumerate() {
        out[ls[i]] = key[*ls.get(j)?];
    }
    match &g.numerators {
        Some(m) => {
            for (i, &j) in m.iter().enumerate() {
                out[os[i]] = key[*os.get(j)?];
            }
        }
        None => {
            if os.iter().any(|&s| key[s] != 0) {
                return None;
            }
        }
    }
    Some(out)
}

/// The STRUCTURAL part of the adopted condition for one element (it never depends on facts; lane closurefix, re-audit
/// F1): the element fits the line mask, the image exists and differs from the key, a key with a non-zero numerator
/// needs a certified pure numerator map, the image is not a declared terminal. Returns the image and its dispatch (with
/// the rule-level key exclusions). Shared by [`try_element`] and the closure's Tier K candidate enumeration.
pub(crate) fn element_image(
    key: &[i32],
    ei: usize,
    pol: &CreditPolicy,
    ctx: &dyn CtxView,
) -> Result<(Vec<i32>, Dispatch), Refusal> {
    let g = &pol.group[ei];
    let lines = &ctx.sectors().lines;
    if g.lines.len() != lines.iter().filter(|&&l| l).count() {
        return Err(Refusal::MalformedElement(
            "line permutation does not fit the line mask".into(),
        ));
    }
    let Some(image) = apply_framed(g, key, lines) else {
        return Err(Refusal::NoImage);
    };
    if image == key {
        return Err(Refusal::ImageEqualsKey);
    }
    let has_numerator = key.iter().zip(lines).any(|(&v, &l)| !l && v != 0);
    if has_numerator && !pol.pure_numerator_maps.contains(&ei) {
        return Err(Refusal::NumeratorMapNotPure);
    }
    if ctx.is_terminal(&image) {
        return Err(Refusal::ImageIsTerminal);
    }
    let d = ctx.dispatch(&image);
    Ok((image, d))
}

/// The adopted condition for ONE element in the direct frame (closure-internal entry, lane closurefix): as the
/// element loop of [`serve_with`].
pub(crate) fn try_one(
    key: &[i32],
    ei: usize,
    pol: &CreditPolicy,
    ctx: &dyn CtxView,
) -> Result<Result<Credit, Refusal>, ClosureError> {
    try_element(key, ei, pol, ctx, CreditFrame::Direct)
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
    let (image, d) = match element_image(key, ei, pol, ctx) {
        Ok(x) => x,
        Err(r) => return Ok(Err(r)),
    };
    let g = &pol.group[ei];
    let lines = &ctx.sectors().lines;
    if ctx.recipe_serves(&image, &d) {
        // the image is served by a relation set (R48), not by the table rule: the closure serves such images itself
        // (closure::credit, R44); this element path (and the standalone checker, which has no relation sets) refuses
        return Ok(Err(Refusal::ImageServedByRecipe));
    }
    // the image's record: covered, or blocked only for missing per-key evidence that IS bound at the image (image lift,
    // closure.md 17; a per-key proof implies regularity at the image)
    let (rule, proof_lifted) = match &d {
        Dispatch::Covered(r) => (*r, false),
        Dispatch::Blocked { at, reason } => match ctx.image_lift(*at, reason.clone(), &image) {
            Some(by_proof) => (*at, by_proof),
            None => return Ok(Err(Refusal::ImageNotCovered(d))),
        },
        _ => return Ok(Err(Refusal::ImageNotCovered(d))),
    };
    let pos = ctx.position(rule);
    let (mut buf, mut cm) = (vec![], vec![]);
    let Some(nt) = ctx.instantiate(pos, &image, &mut buf, &mut cm)? else {
        return Ok(Err(Refusal::NoBody));
    };
    let tier_c = ctx.tier() == Tier::C;
    let mut undecided = vec![];
    if tier_c && !proof_lifted {
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
                    CreditFrame::MappedBack => apply_inverse(g, t, lines),
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

/// Internal entry used by the closure (dispatch through its index): the first element that is not refused (clean or
/// pending), else every element's refusal.
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
    for ei in 0..pol.group.len() {
        match try_element(key, ei, pol, ctx, frame)? {
            // the first element not refused, clean or PENDING (its unknown facts are requested): the choice does not
            // depend on which facts happen to be known, so it never flips when facts arrive
            Ok(c) => return Ok(Ok(c)),
            Err(r) => refusals.push((ei, r)),
        }
    }
    Ok(Err(refusals))
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
