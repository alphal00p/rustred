//! Symmetry credits (plan 2.4.6; DESIGN-COMMENTS A1, A7; COORDINATOR_REQUIREMENTS R8, R9). Interface:
//! plan/auto-campaign/interfaces/closure.md section 3.
//!
//! THE ADOPTED CONDITION (the only one implemented here): a key k without a rule may be served by an image g k
//! (g in the node's automorphism group, applied per POINT by [`GroupElement::apply`]; a key with a non-zero
//! numerator and no numerator map has no image) ONLY IF
//!  (i) the rule R that the dispatch (over the tables WITHOUT credits) selects at g k COVERS g k (never an image of an
//!      image, never "any rule that covers g k": the first-match winner);
//!  (ii) EVERY top-sector right-hand-side key of R instantiated at g k is strictly below the ORIGINAL k under the
//!      node comparator (by default the RHS key itself, the integral it names, as the checker's h13b d2.py compares;
//!      [`CreditFrame::MappedBack`] compares g^-1 of it instead);
//!  (iii) Tier C: every positive-numerator term of R at g k has a verified zero coefficient, and R is regular at g k.
//! NOT "image lower than key", NOT "least orbit member". The first group element (in the given order) that passes
//! gives the credit; every credit names the group element, the image and the rule.

use crate::closure::internal::{CtxView, StandaloneCtx};
use crate::closure::{
    ClosureError, CoefFact, FactSource, FastDispatch, RegFact, RuleBodies, RuleName, Sectors, TermClass, Tier,
};
use crate::cover::{Dispatch, Dispatcher, RuleRef};
use crate::order::NodeComparator;

pub use crate::order::GroupElement;

/// How the right-hand-side keys at g k are compared with k.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CreditFrame {
    /// The RHS key itself (adopted; reproduces the checker's 75 of 170 on T178).
    #[default]
    Direct,
    /// g^-1 applied to the RHS key (the integral's preimage).
    MappedBack,
}

/// The credit policy of a node.
#[derive(Clone, Debug)]
pub struct CreditPolicy {
    pub group: Vec<GroupElement>,
    pub tier: Tier,
    pub frame: CreditFrame,
}

/// A symmetry credit: k is served by `rule` at `image` = g_element k.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credit {
    pub element: usize,
    pub image: Vec<i32>,
    pub rule: RuleRef,
    /// Dispatch position of `rule`.
    pub position: u32,
    /// Distinct top-sector RHS keys (all strictly below k).
    pub top_terms: Vec<Vec<i32>>,
    /// Indices of positive-numerator terms (Tier K: flagged "requires coefficient zero"; Tier C: verified zero).
    pub zero_required: Vec<usize>,
}

/// Why a group element does not give a credit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No image (non-zero numerator and no numerator map, or a malformed element).
    NoImage,
    ImageEqualsKey,
    /// The dispatch at g k is not `Covered`.
    ImageNotCovered(Dispatch),
    NoBody,
    TermNotBelow { term: usize, rhs: Vec<i32> },
    CoefficientNonZero { term: usize },
    CoefficientUnknown { term: usize },
    Irregular,
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

/// The adopted condition for one element (see the module documentation).
fn try_element(key: &[i32], g: &GroupElement, pol: &CreditPolicy, ctx: &dyn CtxView) -> Result<Result<Credit, Refusal>, ClosureError> {
    if g.lines.len() > key.len() {
        return Ok(Err(Refusal::NoImage));
    }
    let Some(image) = g.apply(key) else { return Ok(Err(Refusal::NoImage)) };
    if image == key {
        return Ok(Err(Refusal::ImageEqualsKey));
    }
    let rule = match ctx.dispatch(&image) {
        Dispatch::Covered(r) => r,
        d => return Ok(Err(Refusal::ImageNotCovered(d))),
    };
    let pos = ctx.position(rule);
    let tier_c = pol.tier == Tier::C;
    if tier_c {
        match ctx.facts().regularity(&ctx.rule_id(pos), &image) {
            RegFact::Regular => {}
            RegFact::Pole => return Ok(Err(Refusal::Irregular)),
            RegFact::Unknown => {
                if ctx.require_regularity() {
                    return Ok(Err(Refusal::RegularityUnknown));
                }
            }
        }
    }
    let (mut buf, mut cm) = (vec![], vec![]);
    let Some(nt) = ctx.instantiate(pos, &image, &mut buf, &mut cm)? else { return Ok(Err(Refusal::NoBody)) };
    let n = ctx.arity();
    let mut top = vec![];
    let mut zero_required = vec![];
    for j in 0..nt {
        let t = &buf[j * n..(j + 1) * n];
        let fact = if tier_c { ctx.facts().coefficient(&ctx.rule_id(pos), &image, j) } else { CoefFact::Unknown };
        if fact == CoefFact::Zero {
            continue;
        }
        match ctx.sectors().classify_masked(t, cm[j]) {
            TermClass::PositiveNumerator { .. } => {
                if tier_c {
                    return Ok(Err(if fact == CoefFact::NonZero {
                        Refusal::CoefficientNonZero { term: j }
                    } else {
                        Refusal::CoefficientUnknown { term: j }
                    }));
                }
                zero_required.push(j);
            }
            TermClass::Top => {
                let cmpd = match pol.frame {
                    CreditFrame::Direct => t.to_vec(),
                    CreditFrame::MappedBack => match apply_inverse(g, t) {
                        Some(x) => x,
                        None => return Ok(Err(Refusal::TermNotBelow { term: j, rhs: t.to_vec() })),
                    },
                };
                if !ctx.cmp().lower(&cmpd, key) {
                    return Ok(Err(Refusal::TermNotBelow { term: j, rhs: t.to_vec() }));
                }
                top.push(t.to_vec());
            }
            TermClass::Subsector { .. } | TermClass::ZeroSector { .. } => {}
        }
    }
    top.sort();
    top.dedup();
    Ok(Ok(Credit { element: 0, image, rule, position: pos, top_terms: top, zero_required }))
}

/// Internal entry used by the closure (dispatch through its index).
pub(crate) fn serve_with(
    key: &[i32],
    pol: &CreditPolicy,
    ctx: &dyn CtxView,
) -> Result<Result<Credit, Vec<(usize, Refusal)>>, ClosureError> {
    let mut refusals = vec![];
    for (ei, g) in pol.group.iter().enumerate() {
        match try_element(key, g, pol, ctx)? {
            Ok(mut c) => {
                c.element = ei;
                return Ok(Ok(c));
            }
            Err(r) => refusals.push((ei, r)),
        }
    }
    Ok(Err(refusals))
}

/// The adopted symmetry-credit condition at one key (standalone; the closure uses the same function through its
/// dispatch index). `sectors` gives the line mask (and zero sectors). Returns the credit, or every element's refusal.
#[allow(clippy::too_many_arguments)]
pub fn serve_by_image(
    key: &[i32],
    policy: &CreditPolicy,
    disp: &Dispatcher,
    bodies: &RuleBodies,
    cmp: &NodeComparator,
    sectors: &Sectors,
    facts: &dyn FactSource,
    require_regularity: bool,
) -> Result<Result<Credit, Vec<(usize, Refusal)>>, ClosureError> {
    if key.len() != disp.arity() {
        return Err(ClosureError::KeyArity { expected: disp.arity(), got: key.len() });
    }
    let fast = FastDispatch::new(disp);
    let names = (0..fast.n_entries() as u32)
        .map(|p| {
            let r = fast.rule(p);
            RuleName { table: disp.tables()[r.table].name.clone(), record: r.record, label: r.label }
        })
        .collect();
    let ctx = StandaloneCtx {
        fast,
        disp,
        bodies,
        cmp,
        sectors: sectors.clone(),
        facts,
        names,
        require_regularity,
    };
    serve_with(key, policy, &ctx)
}

/// A reusable standalone credit checker (builds the dispatch index once; for many keys).
pub struct CreditChecker<'a> {
    ctx: StandaloneCtx<'a>,
}

impl<'a> CreditChecker<'a> {
    pub fn new(
        disp: &'a Dispatcher,
        bodies: &'a RuleBodies,
        cmp: &'a NodeComparator,
        sectors: Sectors,
        facts: &'a dyn FactSource,
        require_regularity: bool,
    ) -> Self {
        let fast = FastDispatch::new(disp);
        let names = (0..fast.n_entries() as u32)
            .map(|p| {
                let r = fast.rule(p);
                RuleName { table: disp.tables()[r.table].name.clone(), record: r.record, label: r.label }
            })
            .collect();
        CreditChecker { ctx: StandaloneCtx { fast, disp, bodies, cmp, sectors, facts, names, require_regularity } }
    }
    pub fn serve(&self, key: &[i32], policy: &CreditPolicy) -> Result<Result<Credit, Vec<(usize, Refusal)>>, ClosureError> {
        serve_with(key, policy, &self.ctx)
    }
    /// The first-match dispatch (the same as `Dispatcher::first_match`).
    pub fn dispatch(&self, key: &[i32]) -> Dispatch {
        self.ctx.dispatch(key)
    }
}
