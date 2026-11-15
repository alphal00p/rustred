//! INDEPENDENT CHECK of a shipped composite relation (interfaces/repairflow.md 17; proposed for the import's
//! `supplied_relations` of kind symmetry-composite): the image is RE-DERIVED from the verified group, every replaced
//! term's first match must be the cited record (record_sha256), and the composition is REPEATED in a modular field (the
//! core's certificate backend `CheckAlgebra`: residues at the two engine primes x several values of d, no Symbolica);
//! every shipped coefficient text evaluated at the same contexts must agree, and the key sets must be equal. A modular
//! agreement is the engine's own zero-test standard (two 61-bit primes), stated as such in the verdict.
//! Pure except for the record lines, which the caller supplies (`line_of`).

use std::cmp::Ordering;

use super::composite::{
    Catalogue, ComposeAlgebra, ComposeOptions, RuleUse, Service, compose_image, image_of,
    rows_by_element,
};
use crate::accept::rule::RuleText;
use crate::closure::bodies::{RhsSlot, parse_rhs_pattern, rule_point};
use crate::cover::Dispatch;
use crate::import::load::ImportedSet;
use crate::poly::ENGINE_PRIMES;
use crate::replay_base::algebra::{CheckAlgebra, Ctx, Q};
use crate::replay_base::symrows::VerifiedGroup;

/// The modular field: one residue per context `(p, d)`.
pub struct ModField<'a, QA: CheckAlgebra> {
    pub alg: &'a QA,
    pub ctx: Vec<(u64, u64)>,
}

impl<'a, QA: CheckAlgebra> ModField<'a, QA> {
    /// The two engine primes x the given values of d.
    pub fn engine(alg: &'a QA, ds: &[u64]) -> Self {
        let ctx = ENGINE_PRIMES
            .iter()
            .flat_map(|&p| ds.iter().map(move |&d| (p, d % p)))
            .collect();
        ModField { alg, ctx }
    }
    /// A coefficient text at every context (`point` = the values of n0..n{N-1}; empty for a text in d alone).
    pub fn eval_text(&self, text: &str, point: &[i32]) -> Result<Vec<u64>, String> {
        let ctxs: Vec<Ctx> = self
            .ctx
            .iter()
            .map(|&(p, d)| Ctx {
                p,
                vals: point
                    .iter()
                    .map(|&x| self.alg.from_i128(p, x as i128))
                    .chain(std::iter::once(d))
                    .collect(),
            })
            .collect();
        let mut num = vec![0u64; ctxs.len()];
        let mut den = vec![0u64; ctxs.len()];
        self.alg
            .eval_ratio(text, point.len(), &ctxs, &mut num, &mut den)
            .map_err(|e| format!("coefficient {text:.80}: {e}"))?;
        let mut out = Vec::with_capacity(num.len());
        for (k, (&(p, _), (&n, &dd))) in self.ctx.iter().zip(num.iter().zip(&den)).enumerate() {
            let inv = self.alg.inv(p, dd).ok_or_else(|| {
                format!("denominator zero at context {k} (choose other d values)")
            })?;
            out.push(self.alg.mul(p, n, inv));
        }
        Ok(out)
    }
}

impl<QA: CheckAlgebra> ComposeAlgebra for ModField<'_, QA> {
    type E = Vec<u64>;
    fn from_q(&self, q: Q) -> Result<Vec<u64>, String> {
        self.ctx
            .iter()
            .map(|&(p, _)| {
                self.alg
                    .q_modp(q, p)
                    .ok_or_else(|| "rational with a denominator divisible by p".to_string())
            })
            .collect()
    }
    fn add(&self, a: &Vec<u64>, b: &Vec<u64>) -> Result<Vec<u64>, String> {
        Ok(self
            .ctx
            .iter()
            .zip(a.iter().zip(b))
            .map(|(&(p, _), (&x, &y))| self.alg.add(p, x, y))
            .collect())
    }
    fn mul(&self, a: &Vec<u64>, b: &Vec<u64>) -> Result<Vec<u64>, String> {
        Ok(self
            .ctx
            .iter()
            .zip(a.iter().zip(b))
            .map(|(&(p, _), (&x, &y))| self.alg.mul(p, x, y))
            .collect())
    }
    fn neg(&self, a: &Vec<u64>) -> Result<Vec<u64>, String> {
        Ok(self
            .ctx
            .iter()
            .zip(a)
            .map(|(&(p, _), &x)| self.alg.neg(p, x))
            .collect())
    }
    fn div(&self, a: &Vec<u64>, b: &Vec<u64>) -> Result<Vec<u64>, String> {
        self.ctx
            .iter()
            .zip(a.iter().zip(b))
            .map(|(&(p, _), (&x, &y))| {
                self.alg
                    .inv(p, y)
                    .map(|i| self.alg.mul(p, x, i))
                    .ok_or_else(|| "division by a residue zero at one context".to_string())
            })
            .collect()
    }
    fn is_zero(&self, a: &Vec<u64>) -> Result<bool, String> {
        let z = a.iter().filter(|&&x| x == 0).count();
        if z == a.len() {
            Ok(true)
        } else if z == 0 {
            Ok(false)
        } else {
            Err("zero at some contexts only (undecided: choose other d values)".into())
        }
    }
    fn text(&self, a: &Vec<u64>) -> String {
        format!("{a:?}")
    }
}

/// The first-match catalogue with each record's coefficient texts evaluated in the modular field.
pub struct ModCatalogue<'a, 'b, QA: CheckAlgebra> {
    pub set: &'a ImportedSet,
    pub line_of: &'b dyn Fn(usize, usize) -> Result<Vec<u8>, String>,
    pub field: &'b ModField<'a, QA>,
}

impl<QA: CheckAlgebra> Catalogue<ModField<'_, QA>> for ModCatalogue<'_, '_, QA> {
    fn serve(&self, _: &ModField<'_, QA>, term: &[i32]) -> Result<Service<Vec<u64>>, String> {
        let rr = match self.set.disp.first_match(term) {
            Ok(Dispatch::Covered(rr)) => rr,
            Ok(d) => return Ok(Service::Unserved(format!("{d:?}"))),
            Err(e) => return Ok(Service::Unserved(format!("{e:?}"))),
        };
        let line = (self.line_of)(rr.table, rr.record)?;
        let rule = RuleText::parse(&line, &self.set.frame).map_err(|e| format!("{e:?}"))?;
        let dom = rule.domain.as_ref().ok_or("record without a domain")?;
        let mut pt = vec![];
        if !rule_point(dom, term, &mut pt) {
            return Ok(Service::Unserved("no rule point at the term".into()));
        }
        let arity = self.set.frame.arity();
        let mut rhs = Vec::with_capacity(rule.rhs.len());
        for (pat, text) in &rule.rhs {
            let slots = parse_rhs_pattern(pat, arity)?;
            let k: Option<Vec<i32>> = slots
                .iter()
                .map(|s| match *s {
                    RhsSlot::Param { param, offset } => pt.get(param)?.checked_add(offset),
                    RhsSlot::Const(c) => Some(c),
                })
                .collect();
            let k = k.ok_or("term key out of range")?;
            rhs.push((k, self.field.eval_text(text, &pt)?));
        }
        let info = &self.set.rules[rr.table][rr.record];
        Ok(Service::Rule {
            rule: RuleUse {
                table: rr.table,
                record: rr.record,
                source: info.source.clone(),
                label: info.label,
                record_sha256: info.record_sha256.clone(),
            },
            rhs,
        })
    }
}

/// Verify one shipped composite: `rhs` = the shipped (key, coefficient text in d) list, `element`, `image` and
/// `records` (term, record_sha256) = its verification line. Ok(()) = verified (modular, engine standard).
#[allow(clippy::too_many_arguments)]
pub fn verify_shipped<QA: CheckAlgebra>(
    qalg: &QA,
    field: &ModField<'_, QA>,
    cat: &ModCatalogue<'_, '_, QA>,
    group: &VerifiedGroup,
    key: &[i32],
    element: usize,
    image: &[(Vec<i32>, Q)],
    records: &[(Vec<i32>, String)],
    rhs: &[(Vec<i32>, String)],
    lower: &dyn Fn(&[i32], &[i32]) -> bool,
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
    max_row_terms: usize,
    max_depth: usize,
) -> Result<(), String> {
    // (a) the image re-derived from the verified group
    let by = rows_by_element(qalg, group, key, max_row_terms)?;
    let rows = by
        .get(&element)
        .ok_or("the element gives no row at the key")?;
    let mut im =
        image_of(qalg, rows, key, cmp)?.ok_or("the key is not a pivot of the element's rows")?;
    im.sort_by(|a, b| cmp(&b.0, &a.0));
    let mut want: Vec<(Vec<i32>, Q)> = image.to_vec();
    want.sort_by(|a, b| cmp(&b.0, &a.0));
    if im != want {
        return Err("the image differs from the re-derived one".into());
    }
    // (b) + (c) the composition repeated in the modular field, the replaced terms' records compared
    let opts = ComposeOptions {
        max_depth,
        ..ComposeOptions::default()
    };
    let c = match compose_image(field, cat, &im, key, lower, cmp, &opts)? {
        Ok(c) => c,
        Err(r) => return Err(format!("the repeated composition is refused: {r:?}")),
    };
    let mut got: Vec<(Vec<i32>, String)> = c
        .replacements
        .iter()
        .map(|r| (r.term.clone(), r.rule.record_sha256.clone()))
        .collect();
    let mut cited: Vec<(Vec<i32>, String)> = records.to_vec();
    got.sort();
    cited.sort();
    if got != cited {
        return Err("the replaced terms / records differ from the cited ones".into());
    }
    if c.rhs.len() != rhs.len() {
        return Err(format!(
            "{} terms recomposed, {} shipped",
            c.rhs.len(),
            rhs.len()
        ));
    }
    for (k, text) in rhs {
        let mine = c
            .rhs
            .iter()
            .find(|(q, _)| q == k)
            .ok_or_else(|| format!("shipped term {k:?} not in the recomposed relation"))?;
        if !lower(k, key) {
            return Err(format!("shipped term {k:?} is not lower than the key"));
        }
        if field.eval_text(text, &[])? != mine.1 {
            return Err(format!("coefficient of {k:?} differs (mod p)"));
        }
    }
    Ok(())
}
