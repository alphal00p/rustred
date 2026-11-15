//! The core [`Compose`] trait on lane epsdepth's composition (`epsdepth::compose_streaming` over any
//! `ReductionSource<Coeff = TCoeff>`, e.g. its `RecordSource` of a real closing-loop statement, with its Symbolica
//! backend `SymbolicaLaurent`).
//!
//! - `stream`: one `compose_streaming` call per batch (target = the requested precision: every exponent below it is
//!   decided), each series truncated to that precision and handed over as text (exact rationals);
//! - `exact`: the EXACT composed coefficients of one key (for the n keys of R): an exact composition over Q(t) along the
//!   key's cone with the source's own relations (Symbolica rational functions; cycle check; key budget), mapped back to
//!   d by `t = d - d0` (text in the house syntax). The cones of the regular set's keys are small (low complexity).

use std::collections::HashMap;
use std::sync::Arc;

use symbolica::domains::integer::{Integer, Z};
use symbolica::domains::rational::{Q, Rational};
use symbolica::domains::rational_polynomial::{FromNumeratorAndDenominator, RationalPolynomial};
use symbolica::poly::PolyVariable;

use rustred_campaign_core::epsdepth::{
    ComposeParams, KeyStatus, ReductionSource, compose_streaming,
};
use rustred_campaign_core::key::Key;
use rustred_campaign_core::regbasis::{Compose, ComposeError, ComposedVec, SeriesText};

use crate::epsdepth::{QSeries, SymbolicaExact, SymbolicaLaurent, TCoeff, TRational};
use rustred_campaign_core::epsdepth::Laurent;

use super::series::{QP, RF, ZP, d_vars, expand_t_ratio, rational_text, rf_text, t_vars};

/// A composition backend of lane epsdepth whose series the adapter can hand over as text.
pub trait SeriesExport: Laurent<Coeff = TCoeff> + Clone {
    /// The series truncated to `abs_prec` as the core's text form (Err when its precision is below `abs_prec`).
    fn export(&self, s: &Self::Series, abs_prec: i64) -> Result<SeriesText, String>;
}

impl SeriesExport for SymbolicaLaurent {
    fn export(&self, s: &QSeries, abs_prec: i64) -> Result<SeriesText, String> {
        series_text(s, abs_prec)
    }
}

impl SeriesExport for SymbolicaExact {
    /// An exact rational function of t: expanded exactly (any precision).
    fn export(&self, s: &TRational, abs_prec: i64) -> Result<SeriesText, String> {
        Ok(expand_t_ratio(&s.numerator, &s.denominator, abs_prec, &t_vars()).to_text())
    }
}

/// Lane epsdepth's composition behind the core trait (backend `L`: truncated Laurent or exact).
pub struct EpsdepthCompose<'s, S: ReductionSource<Coeff = TCoeff>, L: SeriesExport> {
    pub src: &'s mut S,
    pub lau: L,
    /// d0 (the source's coefficients are in t = d - d0; the exact texts are mapped back to d with it).
    pub d0: Rational,
    pub nterm: usize,
    index: HashMap<Key, usize>,
    /// Budgets of the composition (`target` is set per call from the requested precision).
    pub params: ComposeParams,
    /// Key budget of one exact cone.
    pub exact_max_keys: usize,
}

impl<'s, S: ReductionSource<Coeff = TCoeff>, L: SeriesExport> EpsdepthCompose<'s, S, L> {
    pub fn new(
        src: &'s mut S,
        lau: L,
        d0: Rational,
        nterm: usize,
        params: ComposeParams,
        exact_max_keys: usize,
    ) -> Self {
        let index = (0..src.n_keys()).map(|i| (src.key(i), i)).collect();
        EpsdepthCompose {
            src,
            lau,
            d0,
            nterm,
            index,
            params,
            exact_max_keys,
        }
    }
}

fn series_text(s: &QSeries, abs_prec: i64) -> Result<SeriesText, String> {
    let (lo, p) = SymbolicaLaurent::series_order(s);
    if p < abs_prec {
        return Err(format!("precision {p} below {abs_prec}"));
    }
    match lo {
        None => Ok(SeriesText::zero(abs_prec)),
        Some(v) if v >= abs_prec => Ok(SeriesText::zero(abs_prec)),
        Some(v) => {
            let mut coeffs = Vec::with_capacity((abs_prec - v) as usize);
            for e in v..abs_prec {
                let c = SymbolicaLaurent::series_coefficient(s, e)
                    .ok_or("coefficient beyond precision")?;
                coeffs.push(rational_text(&c));
            }
            while coeffs.last().is_some_and(|x| x == "0") {
                coeffs.pop();
            }
            Ok(SeriesText {
                start: v,
                coeffs,
                prec: abs_prec,
            })
        }
    }
}

/// TCoeff (t^v n1/d1, n1, d1 over Q in t) -> a rational function over Z in t.
pub fn tcoeff_rf(c: &TCoeff, tv: &Arc<Vec<PolyVariable>>) -> Option<RF> {
    match c {
        TCoeff::Zero => None,
        TCoeff::Ratio { v, n1, d1 } => {
            let to_qp = |u: &[Rational], shift: u32| -> QP {
                let mut p = QP::new(&Q, Some(u.len()), tv.clone());
                for (i, x) in u.iter().enumerate() {
                    if !x.is_zero() {
                        p.append_monomial(x.clone(), &[(i as u32 + shift) as u16]);
                    }
                }
                p
            };
            let (sn, sd) = if *v >= 0 {
                (*v as u32, 0)
            } else {
                (0, (-*v) as u32)
            };
            let n = to_qp(n1.coefficients(), sn);
            let d = to_qp(d1.coefficients(), sd);
            Some(qp_ratio(&n, &d, tv))
        }
    }
}

/// n/d over Q -> RF over Z (common denominator cleared).
fn qp_ratio(n: &QP, d: &QP, tv: &Arc<Vec<PolyVariable>>) -> RF {
    let mut l = Integer::one();
    for c in n.coefficients.iter().chain(&d.coefficients) {
        l = l.lcm(&c.denominator());
    }
    let z = |p: &QP| -> ZP {
        let mut out = ZP::new(&Z, Some(p.nterms()), tv.clone());
        for i in 0..p.nterms() {
            out.append_monomial(
                (&p.coefficients[i] * &Rational::from(l.clone())).numerator(),
                &[p.exponents(i)[0]],
            );
        }
        out
    };
    RationalPolynomial::from_num_den(z(n), z(d), &Z, true)
}

/// A rational function of t -> the same function of d (t = d - d0), over Z in the variable d.
pub fn rf_t_to_d(r: &RF, d0: &Rational) -> RF {
    let dv = d_vars();
    let back = |p: &ZP| -> QP {
        let q: QP = p
            .map_coeff(|c| Rational::from(c.clone()), Q)
            .shift_var(0, &(-d0.clone()));
        let mut out = QP::new(&Q, Some(q.nterms()), dv.clone());
        for i in 0..q.nterms() {
            out.append_monomial(q.coefficients[i].clone(), &[q.exponents(i)[0]]);
        }
        out
    };
    qp_ratio(&back(&r.numerator), &back(&r.denominator), &dv)
}

impl<S: ReductionSource<Coeff = TCoeff>, L: SeriesExport> Compose for EpsdepthCompose<'_, S, L> {
    fn n_terminals(&self) -> usize {
        self.nterm
    }

    fn stream(
        &mut self,
        keys: &[Key],
        abs_prec: i64,
        visit: &mut dyn FnMut(usize, Result<ComposedVec, ComposeError>),
    ) -> Result<(), String> {
        let mut sel = Vec::with_capacity(keys.len());
        let mut pos: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, k) in keys.iter().enumerate() {
            match self.index.get(k) {
                Some(&g) => {
                    pos.entry(g).or_default().push(i);
                    sel.push(g);
                }
                None => visit(
                    i,
                    Err(ComposeError::Undecided(
                        "key not in the reduction source".into(),
                    )),
                ),
            }
        }
        sel.sort_unstable();
        sel.dedup();
        if sel.is_empty() {
            return Ok(());
        }
        let params = ComposeParams {
            target: abs_prec,
            ..self.params.clone()
        };
        let nterm = self.nterm;
        let mut seen: HashMap<usize, bool> = HashMap::new();
        let lau_ref = self.lau.clone();
        compose_streaming(
            self.src,
            &mut self.lau,
            &sel,
            nterm,
            &params,
            Some(&mut |g: usize, r: Result<&[Option<L::Series>], &str>| {
                let res: Result<ComposedVec, ComposeError> = match r {
                    Err(e) => Err(ComposeError::Undecided(e.to_string())),
                    Ok(v) => v
                        .iter()
                        .map(|s| match s {
                            None => Ok(SeriesText::zero(abs_prec)),
                            Some(s) => lau_ref.export(s, abs_prec),
                        })
                        .collect::<Result<Vec<_>, String>>()
                        .map_err(ComposeError::Undecided),
                };
                if let Some(ix) = pos.get(&g) {
                    for &i in ix {
                        visit(i, res.clone());
                    }
                }
                seen.insert(g, true);
            }),
        )?;
        for g in sel {
            if !seen.contains_key(&g) {
                for &i in &pos[&g] {
                    visit(
                        i,
                        Err(ComposeError::Undecided(
                            "not handed over by the composition".into(),
                        )),
                    );
                }
            }
        }
        Ok(())
    }

    fn exact(&mut self, key: &[i32]) -> Result<Vec<String>, ComposeError> {
        let root = *self
            .index
            .get(key)
            .ok_or_else(|| ComposeError::Unavailable("key not in the reduction source".into()))?;
        let tv = t_vars();
        // cone discovery (batched by layer)
        let mut terms: HashMap<usize, Vec<(usize, TCoeff)>> = HashMap::new();
        let mut frontier = vec![root];
        let mut seen: HashMap<usize, ()> = HashMap::new();
        seen.insert(root, ());
        while !frontier.is_empty() {
            if seen.len() > self.exact_max_keys {
                return Err(ComposeError::Undecided(format!(
                    "exact cone above {} keys",
                    self.exact_max_keys
                )));
            }
            let mut served = vec![];
            for &k in &frontier {
                match self.src.status(k) {
                    KeyStatus::Terminal(_) => {}
                    KeyStatus::Served => served.push(k),
                    KeyStatus::Open(w) => {
                        return Err(ComposeError::Undecided(format!(
                            "open key in the cone: {w}"
                        )));
                    }
                }
            }
            let mut next = vec![];
            let mut err: Option<String> = None;
            self.src
                .relations(&served, &mut |k, r| match r {
                    Ok(t) => {
                        for (c, _) in &t {
                            if !seen.contains_key(c) {
                                seen.insert(*c, ());
                                next.push(*c);
                            }
                        }
                        terms.insert(k, t);
                    }
                    Err(e) => err = Some(e),
                })
                .map_err(ComposeError::Undecided)?;
            if let Some(e) = err {
                return Err(ComposeError::Undecided(format!("relation: {e}")));
            }
            frontier = next;
        }
        // post-order with cycle check
        let mut order = vec![];
        let mut state: HashMap<usize, u8> = HashMap::new();
        let mut stack = vec![(root, false)];
        while let Some((k, done)) = stack.pop() {
            if done {
                state.insert(k, 2);
                order.push(k);
                continue;
            }
            match state.get(&k) {
                Some(2) => continue,
                Some(1) => return Err(ComposeError::Undecided("cycle in the cone".into())),
                _ => {}
            }
            state.insert(k, 1);
            stack.push((k, true));
            if let Some(t) = terms.get(&k) {
                for (c, _) in t {
                    match state.get(c) {
                        Some(2) => {}
                        Some(1) => return Err(ComposeError::Undecided("cycle in the cone".into())),
                        _ => stack.push((*c, false)),
                    }
                }
            }
        }
        let zero = || {
            RationalPolynomial::from_num_den(
                ZP::new(&Z, None, tv.clone()),
                ZP::new(&Z, None, tv.clone()).one(),
                &Z,
                false,
            )
        };
        let mut vecs: HashMap<usize, Vec<RF>> = HashMap::new();
        for &k in &order {
            let v: Vec<RF> = match self.src.status(k) {
                KeyStatus::Terminal(j) => (0..self.nterm)
                    .map(|i| {
                        if i == j {
                            RationalPolynomial::from_num_den(
                                ZP::new(&Z, None, tv.clone()).one(),
                                ZP::new(&Z, None, tv.clone()).one(),
                                &Z,
                                false,
                            )
                        } else {
                            zero()
                        }
                    })
                    .collect(),
                _ => {
                    let mut acc: Vec<RF> = (0..self.nterm).map(|_| zero()).collect();
                    for (c, coeff) in &terms[&k] {
                        let Some(rc) = tcoeff_rf(coeff, &tv) else {
                            continue;
                        };
                        let child = &vecs[c];
                        for i in 0..self.nterm {
                            if !child[i].numerator.is_zero() {
                                acc[i] = &acc[i] + &(&rc * &child[i]);
                            }
                        }
                    }
                    acc
                }
            };
            vecs.insert(k, v);
        }
        let d0 = self.d0.clone();
        Ok(vecs[&root]
            .iter()
            .map(|r| rf_text(&rf_t_to_d(r, &d0)))
            .collect())
    }
}
