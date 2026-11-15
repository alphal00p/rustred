//! INDEPENDENT CHECK of a shipped composite relation (interfaces/repairflow.md 17; proposed for the import's
//! `supplied_relations` of kind symmetry-composite): the image is RE-DERIVED from the verified group, every replaced
//! term's first match must be the cited record (record_sha256), and the composition is REPEATED in a modular field (the
//! core's certificate backend `CheckAlgebra`: residues at the two engine primes x several values of d, no Symbolica);
//! every shipped coefficient text evaluated at the same contexts must agree, and the key sets must be equal. A modular
//! agreement is the engine's own zero-test standard (two 61-bit primes), stated as such in the verdict.
//! Pure except for the record lines, which the caller supplies (`line_of`).

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde_json::Value;

use super::composite::{
    COMPOSITE_SCHEMA, Catalogue, ComposeAlgebra, ComposeOptions, RuleUse, Service, compose_image,
    image_of, import_lower, rows_by_element,
};
use crate::accept::rule::RuleText;
use crate::closure::bodies::{RhsSlot, parse_rhs_pattern, rule_point};
use crate::cover::Dispatch;
use crate::import::lines::parse_key_text;
use crate::import::load::ImportedSet;
use crate::independent_algebra::Independent;
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
    // (d) MAP AGAINST MAP (audit r2-sym F2): the shipped terms are DISTINCT, every recomposed term is shipped with the
    // equal coefficient, and no other term is shipped (a repeated term can no longer hide a missing one)
    let mut shipped: BTreeMap<&[i32], &str> = BTreeMap::new();
    for (k, text) in rhs {
        if shipped.insert(k.as_slice(), text.as_str()).is_some() {
            return Err(format!("shipped term {k:?} repeated"));
        }
    }
    let mut mine: BTreeMap<&[i32], &Vec<u64>> = BTreeMap::new();
    for (k, v) in &c.rhs {
        if mine.insert(k.as_slice(), v).is_some() {
            return Err(format!("recomposed term {k:?} repeated"));
        }
    }
    for (k, text) in &shipped {
        let m = mine
            .get(k)
            .ok_or_else(|| format!("shipped term {k:?} not in the recomposed relation"))?;
        if !lower(k, key) {
            return Err(format!("shipped term {k:?} is not lower than the key"));
        }
        if field.eval_text(text, &[])? != **m {
            return Err(format!("coefficient of {k:?} differs (mod p)"));
        }
    }
    if let Some(k) = mine.keys().find(|k| !shipped.contains_key(*k)) {
        return Err(format!(
            "recomposed term {k:?} not shipped ({} recomposed, {} shipped)",
            mine.len(),
            shipped.len()
        ));
    }
    Ok(())
}

/// The values of d of the re-verification (data; DIFFERENT from the composer's own self-check points 1000003, 7777771,
/// 123456791, so the two checks share no evaluation point).
pub const DEFAULT_D_VALUES: [u64; 3] = [2_000_029, 9_999_991, 314_159_257];

/// Options of [`verify_line`] (data, never constants of a node).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineOptions {
    /// Values of d (x the two engine primes) of the modular re-composition.
    pub d_values: Vec<u64>,
    /// Bound of one expanded symmetry row.
    pub max_row_terms: usize,
    /// Smallest replacement depth tried (the line's own depths + 1 when larger).
    pub min_depth: usize,
}

impl Default for LineOptions {
    fn default() -> Self {
        LineOptions {
            d_values: DEFAULT_D_VALUES.to_vec(),
            max_row_terms: crate::import::symmetry::DEFAULT_MAX_TERMS,
            min_depth: 1,
        }
    }
}

/// Verify ONE shipped relation `I(key) = sum rhs` from its verification line `vline` (schema
/// `rustred.campaign.symmetry-composite.v1`: target, element, image, replacements [{term, depth, record_sha256}])
/// against `set` (its first-match catalogue, record lines read and SHA-256-checked) and the verified `group`:
/// [`verify_shipped`] with the import's descent test and comparator, the modular field of `opts.d_values`. The ONE
/// entry used by the import (load-time, import/supplied_verify.rs) and by the supervisor's re-check
/// (adapters/symcomp.rs). Same backend as the composer (rows_by_element / image_of / compose_image).
pub fn verify_line(
    set: &ImportedSet,
    group: &VerifiedGroup,
    key: &[i32],
    rhs: &[(Vec<i32>, String)],
    vline: &Value,
    opts: &LineOptions,
) -> Result<(), String> {
    let arity = set.frame.arity();
    if vline["schema"] != COMPOSITE_SCHEMA {
        return Err(format!("verification line schema is not {COMPOSITE_SCHEMA}"));
    }
    let kt = |t: &Value| -> Result<Vec<i32>, String> {
        t.as_str()
            .and_then(|s| parse_key_text(s, arity))
            .ok_or_else(|| format!("key text {t}"))
    };
    if kt(&vline["target"])? != key {
        return Err("the verification line's target is not the record's key".into());
    }
    let image: Vec<(Vec<i32>, Q)> = vline["image"]
        .as_array()
        .ok_or("image")?
        .iter()
        .map(|p| {
            let q = p[1].as_str().ok_or("image coefficient text")?;
            Ok((kt(&p[0])?, Independent.q_parse(q)?))
        })
        .collect::<Result<_, String>>()?;
    let reps = vline["replacements"].as_array().ok_or("replacements")?;
    let records: Vec<(Vec<i32>, String)> = reps
        .iter()
        .map(|x| {
            Ok((
                kt(&x["term"])?,
                x["record_sha256"]
                    .as_str()
                    .ok_or("replacement record_sha256")?
                    .to_string(),
            ))
        })
        .collect::<Result<_, String>>()?;
    let depth = reps
        .iter()
        .filter_map(|x| x["depth"].as_u64())
        .map(|d| d as usize + 1)
        .max()
        .unwrap_or(0)
        .max(opts.min_depth);
    let element = vline["element"].as_u64().ok_or("element")? as usize;
    let field = ModField::engine(&Independent, &opts.d_values);
    let line_of =
        |t: usize, r: usize| crate::import::exact_facts::record_line(set, t, r).map_err(|e| e.to_string());
    let cat = ModCatalogue {
        set,
        line_of: &line_of,
        field: &field,
    };
    let lines = set.frame.mask().to_vec();
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let lower = |k: &[i32], t: &[i32]| import_lower(&lines, k, t, &cmp);
    verify_shipped(
        &Independent,
        &field,
        &cat,
        group,
        key,
        element,
        &image,
        &records,
        rhs,
        &lower,
        &cmp,
        opts.max_row_terms,
        depth,
    )
}
