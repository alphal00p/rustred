//! # polerank: rows of rho and their rank over F_p from t-adic replay results (lane tadicjob; interfaces/tadic-job.md 4)
//!
//! The t-adic replay returns, per request (a key or an integer-weighted combination of a cell's keys) and per prime, the
//! Laurent series at d0 of every output term's RATIONAL coefficient (`basis` x `GammaTag`). This module turns them into
//! the eps^0 free-symbol row rho(request) and the rank of those rows over a cell:
//!
//! 1. NORMALISATION (data): a term `R:<id>` is the package terminal `id` (`library.json`), whose catalogue row maps it to
//!    masters as `sum rational(d) x prod PGamma(m)^e` (`PGamma(m) = Gamma(1 + m eps)`, constant term 1); a term `M:<name>`
//!    is that master; a node-level term `I(..)` is its own coordinate; `U` and catalogue entries without a master are
//!    closed forms (known to every order). The master coefficient is C_m(eps) = sum r(t) x row(t) x U(eps), t = d - d0 =
//!    -2 eps. U = prod Gamma(1 + m eps)^e of the term's tag times the row's: when it is not 1 its coefficients contain
//!    gamma_E, pi^2, zeta(3), zeta(5), ...; these are replaced by seeded random elements of F_p (gamma_E, pi^2 and every
//!    odd zeta independent; even zetas = rational x pi^(2k)); the generic rank this gives is the true rank unless the
//!    constants satisfy an unexpected algebraic relation (it is never smaller than the true rank).
//! 2. ROWS: for every master m with known depth q_m (data) and every order j > q_m with [eps^-j] C_m != 0 (the polar
//!    part BEYOND the known depth, the only part that brings a free symbol into eps^0), the entry [eps^-j] C_m at the
//!    coordinate (m, j); with a dependence table each (m, j) is replaced by its printed combination of symbols BEFORE the
//!    rank (TRICKS 22); without one each (m, j) is its own symbol and the rank is an UPPER BOUND (said in the record).
//! 3. RANK: Symbolica `Matrix<Zp64>::row_reduce` per prime over the rank requests; pivots = surviving symbols; every
//!    prime of every seed must give the same rank and pivots.
//!
//! Precision: every series carries its absolute precision; products follow prec(ab) = min(prec a + val b, prec b +
//! val a) (Symbolica `Series<Zp64>` multiplies; the zero series are kept here with val = prec). A master whose C_m is
//! not known down to order -q_m is NOT DECIDED (PRECISION), never read as regular.
//!
//! SYMBOLICA VERIFICATION (L11, vendored tree 9cd57678): parsing (`try_parse!` through `SymbolicaAlgebra::parse_ratio`),
//! exact expansion of a rational function at d0 (`SymbolicaLaurent::expand_coeff`, extended Euclid), series products
//! (`Series<Zp64>` Mul / Add, src/poly/series.rs:1083 / 1188), field arithmetic (`Zp64`) and the rank / RREF
//! (`Matrix::row_reduce`, numerica tensors/matrix.rs:2016) are Symbolica's. OWN code: (i) the exponential of a power
//! series over F_p (`Series::exp` exists only for `Series<AtomField>`, series.rs:1383; none over a finite field), a
//! five-line recurrence; (ii) the Bernoulli numbers of the even zeta values (data); (iii) the split of a catalogue row
//! into its top-level products (text), each product parsed by Symbolica; (iv) the precision bookkeeping of zero series.
//!
//! Generic (L8): masters, terminals, depths, dependences, primes and keys are data; nothing names a node.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{Value, json};
use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::{Q, Rational};
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::poly::PolyVariable;
use symbolica::poly::series::Series;
use symbolica::prelude::AtomCore;
use symbolica::tensors::matrix::Matrix;

use crate::SymbolicaAlgebra;
use crate::epsdepth::{SymbolicaLaurent, TCoeff};

/// Version of the rank record (bump on any change of its meaning).
pub const POLERANK_VERSION: &str = "rustred.campaign.polerank.v1";
/// Schema of the known-depth table this module reads.
pub const DEPTH_SCHEMA: &str = "rustred.campaign.known-depth.v1";
/// Schema of the dependence table this module reads.
pub const DEPENDENCE_SCHEMA: &str = "rustred.campaign.dependences.v1";
/// Helper VH 2's schemas (read as well): known depth, symbol dependences, terminal map.
pub const HELPER_DEPTH_SCHEMA: &str = "helper-vh2.known-depth.v1";
pub const HELPER_DEPENDENCE_SCHEMA: &str = "helper-vh2.symbol-dependence.v1";
pub const HELPER_TERMINAL_MAP_SCHEMA: &str = "helper-vh2.terminal-map.v1";

type Elem = FiniteFieldElement<u64>;
type PSeries = Series<Zp64>;

// ---------------------------------------------------------------------------------------------------------------
// data: depths, dependences, terminal map
// ---------------------------------------------------------------------------------------------------------------

/// Known depth per master: fully known through eps^q (data, sha256-bound by the caller).
#[derive(Clone, Debug, Default)]
pub struct DepthTable {
    pub known_through: BTreeMap<String, i64>,
    /// Masters known to every order (closed forms).
    pub fully_known: BTreeSet<String>,
    pub label: String,
}

impl DepthTable {
    pub fn from_json(v: &Value) -> Result<DepthTable, String> {
        if v["schema"] == HELPER_DEPTH_SCHEMA {
            return DepthTable::from_helper_json(v);
        }
        if v["schema"] != DEPTH_SCHEMA {
            return Err(format!(
                "depth table schema {} is not {DEPTH_SCHEMA}",
                v["schema"]
            ));
        }
        let mut t = DepthTable {
            label: v["label"].as_str().unwrap_or("").to_string(),
            ..Default::default()
        };
        for (m, e) in v["masters"].as_object().ok_or("depth table: masters")? {
            let q = e["known_through"]
                .as_i64()
                .ok_or(format!("depth table: {m}: known_through"))?;
            t.known_through.insert(m.clone(), q);
        }
        for m in v["fully_known"].as_array().into_iter().flatten() {
            t.fully_known
                .insert(m.as_str().ok_or("fully_known")?.to_string());
        }
        Ok(t)
    }
}

impl DepthTable {
    /// Helper VH 2's `helper-vh2.known-depth.v1` (lists `ggppss`, `new`, `forcer` with `id` and `q` = fully known
    /// through, null = unknown; `closed` = known to every order; `unmapped` terminals have no depth: their terms are
    /// refused by the terminal map).
    pub fn from_helper_json(v: &Value) -> Result<DepthTable, String> {
        let mut t = DepthTable {
            label: format!(
                "{} {}",
                v["schema"].as_str().unwrap_or(""),
                v["data_version"].as_str().unwrap_or("")
            ),
            ..Default::default()
        };
        for sec in ["ggppss", "new", "forcer"] {
            for e in v[sec].as_array().ok_or(format!("known depth: {sec}"))? {
                let id = e["id"].as_str().ok_or("known depth: id")?.to_string();
                if let Some(q) = e["q"].as_i64() {
                    t.known_through.insert(id, q);
                }
            }
        }
        for e in v["closed"].as_array().into_iter().flatten() {
            if let Some(id) = e["id"].as_str() {
                t.fully_known.insert(id.to_string());
            }
        }
        Ok(t)
    }
}

/// Printed dependences: (master, order) -> sum of c_i symbol_i (the known part omitted).
#[derive(Clone, Debug, Default)]
pub struct Dependences {
    pub map: BTreeMap<(String, i64), Vec<(String, Rational)>>,
    pub label: String,
}

impl Dependences {
    pub fn from_json(v: &Value) -> Result<Dependences, String> {
        if v["schema"] == HELPER_DEPENDENCE_SCHEMA {
            return Dependences::from_helper_json(v);
        }
        if v["schema"] != DEPENDENCE_SCHEMA {
            return Err(format!(
                "dependence table schema {} is not {DEPENDENCE_SCHEMA}",
                v["schema"]
            ));
        }
        let mut d = Dependences {
            label: v["label"].as_str().unwrap_or("").to_string(),
            ..Default::default()
        };
        for e in v["entries"].as_array().ok_or("dependences: entries")? {
            let m = e["master"]
                .as_str()
                .ok_or("dependences: master")?
                .to_string();
            let j = e["order"].as_i64().ok_or("dependences: order")?;
            let mut comb = vec![];
            for (s, c) in e["symbols"].as_object().ok_or("dependences: symbols")? {
                let c = parse_rational(c.as_str().ok_or("dependences: coefficient text")?)?;
                comb.push((s.clone(), c));
            }
            d.map.insert((m, j), comb);
        }
        Ok(d)
    }
}

impl Dependences {
    /// Helper VH 2's `helper-vh2.symbol-dependence.v1`: every printed coefficient with symbols (FREE = its own symbol,
    /// DEPENDENT = known part + combination) and the new masters' symbol parts per order (`new_master_symbol_parts`).
    pub fn from_helper_json(v: &Value) -> Result<Dependences, String> {
        let mut d = Dependences {
            label: format!(
                "{} ({} printed coefficients)",
                v["schema"].as_str().unwrap_or(""),
                v["dependences"].as_array().map_or(0, |a| a.len())
            ),
            ..Default::default()
        };
        let comb = |o: &Value| -> Result<Vec<(String, Rational)>, String> {
            o.as_object()
                .ok_or("dependences: symbols")?
                .iter()
                .map(|(s, c)| {
                    Ok((
                        s.clone(),
                        parse_rational(c.as_str().ok_or("coefficient text")?)?,
                    ))
                })
                .collect()
        };
        for e in v["dependences"]
            .as_array()
            .ok_or("dependences: dependences")?
        {
            let m = e["master"]
                .as_str()
                .ok_or("dependences: master")?
                .to_string();
            let j = e["order"].as_i64().ok_or("dependences: order")?;
            d.map.insert((m, j), comb(&e["symbols"])?);
        }
        for e in v["new_master_symbol_parts"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let m = e["master"].as_str().ok_or("new master")?.to_string();
            for (o, x) in e["orders"].as_object().into_iter().flatten() {
                let j: i64 = o.parse().map_err(|e| format!("new master order: {e}"))?;
                d.map.insert((m.clone(), j), comb(&x["symbols"])?);
            }
        }
        Ok(d)
    }
}

fn parse_rational(s: &str) -> Result<Rational, String> {
    use std::str::FromStr;
    let s = s.trim();
    let (n, d) = match s.split_once('/') {
        Some((n, d)) => (n.trim(), d.trim()),
        None => (s, "1"),
    };
    let n = Integer::from_str(n).map_err(|e| format!("rational {s}: {e}"))?;
    let d = Integer::from_str(d).map_err(|e| format!("rational {s}: {e}"))?;
    if d == 0 {
        return Err(format!("rational {s}: zero denominator"));
    }
    Ok(Rational::new(n, d))
}

/// The exchange basis (helper VH 2's `helper-vh2.exchange-list.v1`, TRICKS 28 / 30 / 47): per exchanged master m the
/// relation `int[m] = sum_n B[m][n] new[n] + sum_k A[m][k] int[k]` exact in d; rotating a reduction sends
/// c'(target) += c_m x coefficient(m -> target) and drops m (no reduction is redone). The new masters' known depth q
/// comes from the same file. Rules are parsed (Symbolica) when first met.
pub struct Exchange {
    rules: BTreeMap<String, Vec<(String, String)>>,
    pub new_q: BTreeMap<String, i64>,
    parsed: std::cell::RefCell<BTreeMap<String, Vec<(String, TCoeff)>>>,
    pub label: String,
}

impl Exchange {
    pub fn from_json(v: &Value) -> Result<Exchange, String> {
        let mut rules = BTreeMap::new();
        for r in v["relation"]["rows"]
            .as_array()
            .ok_or("exchange: relation.rows")?
        {
            let m = r["master_out"]
                .as_str()
                .ok_or("exchange: master_out")?
                .to_string();
            let mut ts = vec![];
            for e in r["B"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(r["A"].as_array().into_iter().flatten())
            {
                let target = e["master"].as_str().ok_or("exchange: master")?.to_string();
                let num = e["coefficient"]["num"].as_str().ok_or("exchange: num")?;
                let den = e["coefficient"]["den"].as_str().ok_or("exchange: den")?;
                ts.push((target, format!("({num})/({den})")));
            }
            rules.insert(m, ts);
        }
        let mut new_q = BTreeMap::new();
        for n in v["new_master_definitions"]
            .as_array()
            .ok_or("exchange: new_master_definitions")?
        {
            new_q.insert(
                n["master"]
                    .as_str()
                    .ok_or("exchange: new master")?
                    .to_string(),
                n["q"].as_i64().ok_or("exchange: q")?,
            );
        }
        Ok(Exchange {
            rules,
            new_q,
            parsed: Default::default(),
            label: format!(
                "{} {}",
                v["schema"].as_str().unwrap_or(""),
                v["data_version"].as_str().unwrap_or("")
            ),
        })
    }

    pub fn exchanged(&self) -> impl Iterator<Item = &String> {
        self.rules.keys()
    }

    fn rule(
        &self,
        alg: &SymbolicaAlgebra,
        lau: &SymbolicaLaurent,
        m: &str,
    ) -> Result<Vec<(String, TCoeff)>, String> {
        if let Some(r) = self.parsed.borrow().get(m) {
            return Ok(r.clone());
        }
        let mut out = vec![];
        for (target, text) in self
            .rules
            .get(m)
            .ok_or(format!("exchange: no rule for {m}"))?
        {
            let (n, d) = alg.parse_ratio(text, 0)?;
            out.push((target.clone(), lau.coeff_from_zpolys(&n, &d, 0)?));
        }
        self.parsed.borrow_mut().insert(m.to_string(), out.clone());
        Ok(out)
    }
}

/// One product of a row entry: an exact rational function of t (at d0) times prod Gamma(1 + m eps)^e.
#[derive(Clone, Debug)]
pub struct RowPart {
    pub coeff: TCoeff,
    pub tag: BTreeMap<i64, i64>,
}

/// One master of a terminal's row (None = a closed form, known to every order).
#[derive(Clone, Debug)]
pub struct RowEntry {
    pub master: Option<String>,
    pub parts: Vec<RowPart>,
}

/// How a term basis maps to masters.
#[derive(Clone, Debug)]
pub enum TermRow {
    /// Known to every order (`U`, closed forms).
    Closed,
    /// The basis is itself a coordinate (node-level masters / exits, package `M:<name>`): rational 1, no tag.
    Itself(String),
    /// A package terminal with its catalogue row.
    Rows(Vec<RowEntry>),
    /// A terminal without a row (catalogue status unmapped): every order unknown.
    Unmapped(String),
}

/// The terminal map of a package: `R:<id>` -> catalogue rows (built from `library.json` and the catalogue).
#[derive(Clone, Debug, Default)]
pub struct TerminalMap {
    pub rows: BTreeMap<String, TermRow>,
}

type ZPolyR = symbolica::poly::polynomial::MultivariatePolynomial<
    symbolica::domains::integer::IntegerRing,
    u16,
>;

/// A catalogue coefficient `sum rational(d) x prod PGamma(m)^e` as its products: Symbolica parses the whole text with
/// every `PGamma(m)` replaced by its own polynomial variable (`rustred::pg<m>`), as ONE rational function in d and those
/// variables; the numerator's monomials are grouped by their Gamma exponents; the denominator must be a single Gamma
/// monomial times a polynomial in d (else refused). Returns (numerator in d, denominator in d, tag) per product, the
/// polynomials on the variable map [d, pg...] with degree 0 in every pg.
/// One product of a catalogue row: numerator and denominator in d, and its Gamma tag.
pub type RowProduct = (ZPolyR, ZPolyR, BTreeMap<i64, i64>);

pub fn row_products(text: &str) -> Result<Vec<RowProduct>, String> {
    // the PGamma indices present
    let mut ms: BTreeSet<i64> = BTreeSet::new();
    let mut rest = text;
    while let Some(i) = rest.find("PGamma(") {
        let r = &rest[i + 7..];
        let close = r.find(')').ok_or("PGamma(")?;
        ms.insert(
            r[..close]
                .trim()
                .parse()
                .map_err(|e| format!("PGamma index: {e}"))?,
        );
        rest = &r[close..];
    }
    let name = |m: i64| {
        if m < 0 {
            format!("pgm{}", -m)
        } else {
            format!("pgp{m}")
        }
    };
    let mut replaced = text.to_string();
    for &m in &ms {
        replaced = replaced.replace(&format!("PGamma({m})"), &name(m));
    }
    let mut syms = vec!["d".to_string()];
    syms.extend(ms.iter().map(|&m| name(m)));
    let mut vars = vec![];
    for s in &syms {
        let q = format!("rustred::{s}");
        let ns = NamespacedSymbol::try_parse(&q).ok_or(format!("symbol {q}"))?;
        vars.push(PolyVariable::Symbol(
            SymbolBuilder::new(ns).build().map_err(|e| e.to_string())?,
        ));
    }
    let vars = Arc::new(vars);
    let atom = symbolica::try_parse!(&replaced, default_namespace = "rustred")
        .map_err(|e| e.to_string())?;
    let rp = atom
        .as_view()
        .try_to_rational_polynomial::<_, _, u16>(
            &Q,
            &symbolica::domains::integer::Z,
            Some(vars.clone()),
        )
        .map_err(|e| format!("row coefficient is not a rational function: {e}"))?;
    if rp.numerator.get_vars_ref().len() != vars.len() {
        return Err("row coefficient has symbols other than d and PGamma".into());
    }
    let nv = vars.len();
    let gexp = |e: &[u16]| -> Vec<u16> { e[1..].to_vec() };
    // denominator: one Gamma monomial
    let mut dg: Option<Vec<u16>> = None;
    let mut den = ZPolyR::new(&symbolica::domains::integer::Z, None, vars.clone());
    for (c, e) in rp
        .denominator
        .coefficients
        .iter()
        .zip(rp.denominator.exponents_iter())
    {
        let g = gexp(e);
        if dg.as_ref().is_some_and(|x| *x != g) {
            return Err("row denominator is not a single Gamma monomial".into());
        }
        dg = Some(g);
        let mut ex = vec![0u16; nv];
        ex[0] = e[0];
        den.append_monomial(c.clone(), &ex);
    }
    let dg = dg.unwrap_or_else(|| vec![0; nv - 1]);
    let mut groups: BTreeMap<Vec<u16>, ZPolyR> = BTreeMap::new();
    for (c, e) in rp
        .numerator
        .coefficients
        .iter()
        .zip(rp.numerator.exponents_iter())
    {
        let g = groups
            .entry(gexp(e))
            .or_insert_with(|| ZPolyR::new(&symbolica::domains::integer::Z, None, vars.clone()));
        let mut ex = vec![0u16; nv];
        ex[0] = e[0];
        g.append_monomial(c.clone(), &ex);
    }
    let mut out = vec![];
    for (g, num) in groups {
        let mut tag = BTreeMap::new();
        for (i, &m) in ms.iter().enumerate() {
            let e = i64::from(g[i]) - i64::from(dg[i]);
            if m != 0 && e != 0 {
                tag.insert(m, e);
            }
        }
        out.push((num, den.clone(), tag));
    }
    Ok(out)
}

/// Parse a GammaTag text of the engine (`GammaTag([(m,e),...])`) into its exponents.
pub fn parse_gamma_tag(text: &str) -> Result<BTreeMap<i64, i64>, String> {
    let mut tag = BTreeMap::new();
    let t = text.trim();
    if t.is_empty() {
        return Ok(tag);
    }
    let inner = t
        .strip_prefix("GammaTag([")
        .and_then(|x| x.strip_suffix("])"))
        .ok_or(format!("gamma tag {t}"))?;
    for pair in inner.split("),").filter(|x| !x.trim().is_empty()) {
        let p = pair.trim().trim_start_matches('(').trim_end_matches(')');
        let (m, e) = p.split_once(',').ok_or(format!("gamma tag pair {p}"))?;
        let m: i64 = m.trim().parse().map_err(|e| format!("gamma tag: {e}"))?;
        let e: i64 = e.trim().parse().map_err(|e| format!("gamma tag: {e}"))?;
        *tag.entry(m).or_insert(0) += e;
    }
    tag.retain(|&m, e| m != 0 && *e != 0);
    Ok(tag)
}

impl TerminalMap {
    /// From a package's `library.json` and a master catalogue (`vakint.p-master-catalogue.v1`): terminal `id` ->
    /// (owner presentation's family file stem, key) -> catalogue entry.
    pub fn from_package(
        alg: &SymbolicaAlgebra,
        lau: &SymbolicaLaurent,
        library: &Value,
        catalogue: &Value,
    ) -> Result<TerminalMap, String> {
        let owners: BTreeMap<&str, &Value> = library["owners"]
            .as_array()
            .ok_or("library: owners")?
            .iter()
            .filter_map(|o| o["class"].as_str().map(|c| (c, o)))
            .collect();
        let pres = library["presentations"]
            .as_array()
            .ok_or("library: presentations")?;
        let mut cat: BTreeMap<(String, Vec<i64>), &Value> = BTreeMap::new();
        for t in catalogue["terminals"]
            .as_array()
            .ok_or("catalogue: terminals")?
        {
            let node = t["node"].as_str().ok_or("catalogue: node")?.to_string();
            let key: Vec<i64> = t["key"]
                .as_array()
                .ok_or("catalogue: key")?
                .iter()
                .filter_map(Value::as_i64)
                .collect();
            cat.insert((node, key), t);
        }
        let mut rows = BTreeMap::new();
        for (id, t) in library["terminals"]
            .as_array()
            .ok_or("library: terminals")?
            .iter()
            .enumerate()
        {
            let name = format!("R:{id}");
            if t == "unit" {
                rows.insert(name, TermRow::Closed);
                continue;
            }
            let r = &t["residual"];
            let owner = owners
                .get(r["owner"].as_str().ok_or("library: residual owner")?)
                .ok_or(format!("terminal {id}: owner not in the library"))?;
            let p = pres
                .get(owner["presentation"].as_u64().ok_or("owner presentation")? as usize)
                .ok_or("owner presentation index")?;
            let fam = p["family"].as_str().ok_or("presentation family")?;
            let stem = fam
                .rsplit('/')
                .next()
                .unwrap_or(fam)
                .trim_end_matches(".rrbin")
                .to_string();
            let key: Vec<i64> = r["key"]
                .as_array()
                .ok_or("residual key")?
                .iter()
                .filter_map(Value::as_i64)
                .collect();
            let Some(e) = cat.get(&(stem.clone(), key.clone())) else {
                rows.insert(
                    name,
                    TermRow::Unmapped(format!("{stem} {key:?}: no catalogue entry")),
                );
                continue;
            };
            let comb = e["combination"].as_array();
            if comb.is_none() || e["status"] == "unmapped" {
                rows.insert(
                    name,
                    TermRow::Unmapped(format!("{stem} {key:?}: {}", e["status"])),
                );
                continue;
            }
            let mut entries = vec![];
            for c in comb.into_iter().flatten() {
                let master = c["master"].as_str().map(str::to_string);
                let text = c["coefficient"].as_str().ok_or("catalogue coefficient")?;
                let mut parts = vec![];
                if master.is_some() {
                    alg.budget
                        .max_text_bytes
                        .checked_sub(text.len())
                        .ok_or("row text above the parse budget")?;
                    for (n, d, tag) in row_products(text)? {
                        parts.push(RowPart {
                            coeff: lau.coeff_from_zpolys(&n, &d, 0)?,
                            tag,
                        });
                    }
                }
                entries.push(RowEntry { master, parts });
            }
            rows.insert(name, TermRow::Rows(entries));
        }
        Ok(TerminalMap { rows })
    }

    /// From helper VH 2's `helper-vh2.terminal-map.v1` (per `R:<id>` / `U` the route-v3 row; `route` = "v3" or "v2f",
    /// the v2f entry where it differs); master `CLOSED` = a closed form; a terminal without a row is unmapped.
    pub fn from_helper_json(
        alg: &SymbolicaAlgebra,
        lau: &SymbolicaLaurent,
        v: &Value,
        route: &str,
    ) -> Result<TerminalMap, String> {
        if v["schema"] != HELPER_TERMINAL_MAP_SCHEMA {
            return Err(format!(
                "terminal map schema {} is not {HELPER_TERMINAL_MAP_SCHEMA}",
                v["schema"]
            ));
        }
        let mut rows = BTreeMap::new();
        for (name, t) in v["terminals"]
            .as_object()
            .ok_or("terminal map: terminals")?
        {
            let entry = if route == "v2f" && t["v2f"].is_object() {
                &t["v2f"]
            } else {
                &t["v3"]
            };
            let Some(comb) = entry["combination"].as_array() else {
                rows.insert(
                    name.clone(),
                    TermRow::Unmapped(format!("{name}: {}", entry["source"])),
                );
                continue;
            };
            if name == "U"
                || comb
                    .iter()
                    .all(|c| c["master"] == "CLOSED" || c["master"].is_null())
            {
                rows.insert(name.clone(), TermRow::Closed);
                continue;
            }
            let mut entries = vec![];
            for c in comb {
                let master = c["master"]
                    .as_str()
                    .filter(|m| *m != "CLOSED")
                    .map(str::to_string);
                let text = c["coefficient"]
                    .as_str()
                    .ok_or("terminal map coefficient")?;
                let mut parts = vec![];
                if master.is_some() {
                    alg.budget
                        .max_text_bytes
                        .checked_sub(text.len())
                        .ok_or("row text above the parse budget")?;
                    for (n, d, tag) in row_products(text)? {
                        parts.push(RowPart {
                            coeff: lau.coeff_from_zpolys(&n, &d, 0)?,
                            tag,
                        });
                    }
                }
                entries.push(RowEntry { master, parts });
            }
            rows.insert(name.clone(), TermRow::Rows(entries));
        }
        Ok(TerminalMap { rows })
    }

    /// The row of a term basis: `R:<id>` from the map, `U` closed, `M:<name>` the master, anything else (a node-level
    /// key) its own coordinate.
    pub fn row_of(&self, basis: &str) -> TermRow {
        if basis == "U" {
            return TermRow::Closed;
        }
        if let Some(r) = self.rows.get(basis) {
            return r.clone();
        }
        if basis.starts_with("R:") {
            return TermRow::Unmapped(format!("{basis}: not in the terminal map"));
        }
        TermRow::Itself(basis.strip_prefix("M:").unwrap_or(basis).to_string())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// truncated Laurent series mod p with explicit precision
// ---------------------------------------------------------------------------------------------------------------

/// A Laurent series in eps over F_p known for every order < `prec`; `s = None` = zero to `prec`.
#[derive(Clone, Debug)]
pub struct Lau {
    pub s: Option<PSeries>,
    pub prec: i64,
}

/// The F_p context of one prime: field, eps variable, transcendental values.
pub struct Fp {
    pub p: u64,
    pub f: Zp64,
    var: Arc<PolyVariable>,
    gamma: Elem,
    pi2: Elem,
    zeta_odd: BTreeMap<i64, Elem>,
    zeta_even_over_pi: Vec<Rational>,
}

fn int_mod(f: &Zp64, i: &Integer) -> Elem {
    let p = Integer::from(f.get_prime());
    let mut r = i % &p;
    if r < 0 {
        r += &p;
    }
    f.to_element(u64::try_from(r).expect("residue fits u64"))
}

/// zeta(2k) / pi^(2k) for k = 1 .. kmax: (-1)^(k+1) B_2k 2^(2k-1) / (2k)! (Bernoulli numbers by their recurrence).
fn zeta_even_rationals(kmax: usize) -> Vec<Rational> {
    let n = 2 * kmax + 1;
    let mut b: Vec<Rational> = vec![Q.one()];
    let binom = |n: usize, k: usize| -> Integer {
        let mut r = Integer::from(1);
        for i in 0..k {
            r = r * Integer::from((n - i) as i64) / Integer::from((i + 1) as i64);
        }
        r
    };
    for m in 1..=n {
        let mut s = Q.zero();
        for (k, bk) in b.iter().enumerate().take(m) {
            s = &s + &(bk * &Rational::from(binom(m + 1, k)));
        }
        b.push(-(&s / &Rational::from((m + 1) as i64)));
    }
    let mut out = vec![Q.zero()];
    for k in 1..=kmax {
        let mut fact = Integer::from(1);
        for i in 1..=(2 * k) {
            fact *= Integer::from(i as i64);
        }
        let mut two = Integer::from(1);
        for _ in 0..(2 * k - 1) {
            two *= Integer::from(2);
        }
        let mut v = &b[2 * k] * &Rational::new(two, fact);
        if k % 2 == 0 {
            v = -v;
        }
        out.push(v);
    }
    out
}

/// A seeded value mod p (SHA-256 of seed, prime and name: reproducible).
fn seeded(seed: u64, p: u64, name: &str) -> u64 {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(format!("polerank-transcendental:{seed}:{p}:{name}").as_bytes());
    let hex = h.finish_hex();
    let x = u128::from_str_radix(&hex[..32], 16).unwrap_or(1);
    (x % u128::from(p)) as u64
}

impl Fp {
    pub fn new(p: u64, seed: u64) -> Result<Fp, String> {
        let ns = NamespacedSymbol::try_parse("rustred::eps").ok_or("symbol rustred::eps")?;
        let sym = SymbolBuilder::new(ns).build().map_err(|e| e.to_string())?;
        let f = Zp64::new(p);
        let mut zeta_odd = BTreeMap::new();
        for k in (3..80).step_by(2) {
            zeta_odd.insert(k, f.to_element(seeded(seed, p, &format!("zeta{k}"))));
        }
        Ok(Fp {
            p,
            gamma: f.to_element(seeded(seed, p, "gamma")),
            pi2: f.to_element(seeded(seed, p, "pi2")),
            zeta_odd,
            zeta_even_over_pi: zeta_even_rationals(40),
            var: Arc::new(PolyVariable::Symbol(sym)),
            f,
        })
    }

    /// The seeded value of pi^2 (tests and records).
    pub fn pi2(&self) -> u64 {
        self.f.from_element(&self.pi2)
    }

    fn rat(&self, r: &Rational) -> Result<Elem, String> {
        let d = int_mod(&self.f, &r.denominator());
        if self.f.is_zero(&d) {
            return Err(format!("the prime {} divides a denominator", self.p));
        }
        Ok(self
            .f
            .mul(&int_mod(&self.f, &r.numerator()), &self.f.inv(&d)))
    }

    fn base(&self, n: usize) -> PSeries {
        Series::new(
            &self.f,
            Some(n),
            self.var.clone(),
            self.f.zero(),
            Rational::from(n.max(1) as i64),
        )
    }

    /// The series of residues `c[i]` at orders `v + i` (eps), known below `prec`.
    pub fn from_coeffs(&self, v: i64, coeffs: &[Elem], prec: i64) -> Lau {
        let first = coeffs.iter().position(|c| !self.f.is_zero(c));
        let Some(i0) = first else {
            return Lau { s: None, prec };
        };
        let v0 = v + i0 as i64;
        if v0 >= prec {
            return Lau { s: None, prec };
        }
        let n = (prec - v0) as usize;
        let b = self.base(n);
        let terms: Vec<PSeries> = coeffs[i0..]
            .iter()
            .enumerate()
            .take(n)
            .filter(|(_, c)| !self.f.is_zero(c))
            .map(|(i, c)| b.monomial(*c, Rational::from(v0 + i as i64)))
            .collect();
        Lau {
            s: Some(Series::sum(terms)),
            prec,
        }
    }

    /// Valuation (first nonzero order) or the precision for a zero series.
    pub fn val(&self, a: &Lau) -> i64 {
        match &a.s {
            None => a.prec,
            Some(s) if s.is_zero() => a.prec,
            Some(s) => s
                .get_trailing_exponent()
                .numerator()
                .to_i64()
                .unwrap_or(a.prec),
        }
    }

    pub fn coeff(&self, a: &Lau, k: i64) -> Option<Elem> {
        if k >= a.prec {
            return None;
        }
        Some(match &a.s {
            None => self.f.zero(),
            Some(s) => s
                .coefficient(Rational::from(k))
                .unwrap_or_else(|| self.f.zero()),
        })
    }

    pub fn mul(&self, a: &Lau, b: &Lau) -> Lau {
        let prec = (a.prec.saturating_add(self.val(b))).min(b.prec.saturating_add(self.val(a)));
        match (&a.s, &b.s) {
            (Some(x), Some(y)) if !x.is_zero() && !y.is_zero() => {
                let mut s = x * y;
                s.truncate_absolute_order(Rational::from(prec.max(self.val(a) + self.val(b))));
                self.normalise(Lau { s: Some(s), prec })
            }
            _ => Lau { s: None, prec },
        }
    }

    pub fn add(&self, a: &Lau, b: &Lau) -> Lau {
        let prec = a.prec.min(b.prec);
        let s = match (&a.s, &b.s) {
            (None, None) => None,
            (Some(x), None) | (None, Some(x)) => Some(x.clone()),
            (Some(x), Some(y)) => Some(x + y),
        };
        self.normalise(Lau { s, prec })
    }

    pub fn scale(&self, a: &Lau, c: &Elem) -> Lau {
        if self.f.is_zero(c) {
            return Lau {
                s: None,
                prec: a.prec,
            };
        }
        Lau {
            s: a.s.clone().map(|s| s.mul_coeff(c)),
            prec: a.prec,
        }
    }

    /// Re-express with only the coefficients below `prec` (a coefficient at or above the precision is not known).
    fn normalise(&self, a: Lau) -> Lau {
        let Some(s) = &a.s else { return a };
        let v = self.val(&a);
        if s.is_zero() || v >= a.prec {
            return Lau {
                s: None,
                prec: a.prec,
            };
        }
        let coeffs: Vec<Elem> = (v..a.prec)
            .map(|k| {
                s.coefficient(Rational::from(k))
                    .unwrap_or_else(|| self.f.zero())
            })
            .collect();
        self.from_coeffs(v, &coeffs, a.prec)
    }

    /// prod Gamma(1 + m eps)^e mod p known below `prec` (the transcendental constants by their seeded values).
    pub fn gamma_unit(&self, tag: &BTreeMap<i64, i64>, prec: i64) -> Result<Lau, String> {
        let n = prec.max(1) as usize;
        if tag.is_empty() {
            return Ok(self.from_coeffs(0, &[self.f.one()], prec.max(1)));
        }
        // log U = sum_k L_k eps^k: L_1 = -gamma sum e m; L_k = (-1)^k zeta(k) / k sum e m^k
        let mut l = vec![self.f.zero(); n];
        for (k, lk) in l.iter_mut().enumerate().skip(1) {
            let mut s = Integer::from(0);
            for (&m, &e) in tag {
                let mut mk = Integer::from(1);
                for _ in 0..k {
                    mk *= Integer::from(m);
                }
                s += Integer::from(e) * mk;
            }
            let s = int_mod(&self.f, &s);
            let z = if k == 1 {
                self.f.neg(&self.gamma)
            } else if k % 2 == 1 {
                let zk = *self
                    .zeta_odd
                    .get(&(k as i64))
                    .ok_or("zeta index beyond the table")?;
                self.f.neg(&zk)
            } else {
                let r = self
                    .zeta_even_over_pi
                    .get(k / 2)
                    .ok_or("even zeta index beyond the table")?;
                let mut pk = self.f.one();
                for _ in 0..(k / 2) {
                    pk = self.f.mul(&pk, &self.pi2);
                }
                self.f.mul(&self.rat(r)?, &pk)
            };
            let inv_k = self.f.inv(&self.f.to_element(k as u64));
            *lk = self.f.mul(&self.f.mul(&s, &z), &inv_k);
        }
        // exp: E_0 = 1, E_j = (1/j) sum_{k=1..j} k L_k E_{j-k}   (own: no exp over a finite field in Symbolica)
        let mut ecoef = vec![self.f.zero(); n];
        ecoef[0] = self.f.one();
        for j in 1..n {
            let mut acc = self.f.zero();
            for k in 1..=j {
                let t = self.f.mul(
                    &self.f.mul(&self.f.to_element(k as u64), &l[k]),
                    &ecoef[j - k],
                );
                acc = self.f.add(&acc, &t);
            }
            ecoef[j] = self.f.mul(&acc, &self.f.inv(&self.f.to_element(j as u64)));
        }
        Ok(self.from_coeffs(0, &ecoef, prec.max(1)))
    }

    /// An exact rational function of t (at d0) expanded over Q and reduced mod p, as a series in eps known below `prec`.
    pub fn rational_series(
        &self,
        lau: &SymbolicaLaurent,
        c: &TCoeff,
        prec: i64,
    ) -> Result<Lau, String> {
        let v = c.valuation();
        let Some(qs) = lau.expand_coeff(c, prec)? else {
            return Ok(Lau { s: None, prec });
        };
        let vt = match v {
            rustred_campaign_core::epsdepth::Val::Order(o) => o,
            _ => return Ok(Lau { s: None, prec }),
        };
        let m2 = self.f.neg(&self.f.to_element(2));
        let m2inv = self.f.inv(&m2);
        let mut coeffs = vec![];
        for k in vt..prec {
            let r = SymbolicaLaurent::series_coefficient(&qs, k).unwrap_or_else(|| Q.zero());
            // t^k = (-2)^k eps^k
            let mut w = self.f.one();
            let base = if k >= 0 { m2 } else { m2inv };
            for _ in 0..k.unsigned_abs() {
                w = self.f.mul(&w, &base);
            }
            coeffs.push(self.f.mul(&self.rat(&r)?, &w));
        }
        Ok(self.from_coeffs(vt, &coeffs, prec))
    }

    /// A replayed term's residues in t (orders v .. prec-1) as a series in eps.
    pub fn term_series(&self, valuation: Option<i64>, prec: i64, coeffs: &[u64]) -> Lau {
        let Some(v) = valuation else {
            return Lau { s: None, prec };
        };
        let m2 = self.f.neg(&self.f.to_element(2));
        let m2inv = self.f.inv(&m2);
        let c: Vec<Elem> = coeffs
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                let k = v + i as i64;
                let mut w = self.f.one();
                let base = if k >= 0 { m2 } else { m2inv };
                for _ in 0..k.unsigned_abs() {
                    w = self.f.mul(&w, &base);
                }
                self.f.mul(&self.f.to_element(x % self.p), &w)
            })
            .collect();
        self.from_coeffs(v, &c, prec)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// rows of rho per request and prime
// ---------------------------------------------------------------------------------------------------------------

/// One request's row at one prime.
#[derive(Clone, Debug)]
pub enum RowOutcome {
    Row {
        /// symbol -> value (nonzero entries).
        entries: BTreeMap<String, u64>,
        /// master -> need (max(0, -valuation of C_m)).
        needs: BTreeMap<String, i64>,
        /// masters with need > known depth: (master, need, q).
        beyond: Vec<(String, i64, i64)>,
    },
    NotDecided(String),
}

/// One term as read from a result (one prime).
#[derive(Clone, Debug)]
struct TermAtPrime {
    basis: String,
    tag: BTreeMap<i64, i64>,
    valuation: Option<i64>,
    precision: i64,
    coeffs: Vec<u64>,
}

fn terms_at_prime(request: &Value, prime: u64) -> Result<Vec<TermAtPrime>, String> {
    let mut out = vec![];
    for t in request["terms"].as_array().ok_or("request: terms")? {
        let basis = t["basis"].as_str().ok_or("term basis")?.to_string();
        let tag = parse_gamma_tag(t["tag"].as_str().unwrap_or(""))?;
        let pp = t["per_prime"]
            .as_array()
            .ok_or("per_prime")?
            .iter()
            .find(|x| x["prime"].as_str().and_then(|s| s.parse::<u64>().ok()) == Some(prime));
        let Some(pp) = pp else {
            // no series at this prime: the term is zero there to the request's precision (C4 checks seeds agree)
            continue;
        };
        let valuation = pp["valuation"].as_i64();
        let precision = pp["precision"].as_i64().ok_or("precision")?;
        let coeffs = pp["coefficients"]
            .as_array()
            .ok_or("coefficients")?
            .iter()
            .map(|c| {
                c.as_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .ok_or("coefficient".to_string())
            })
            .collect::<Result<Vec<u64>, String>>()?;
        out.push(TermAtPrime {
            basis,
            tag,
            valuation,
            precision,
            coeffs,
        });
    }
    Ok(out)
}

/// The settings of the rank step.
pub struct RankData<'a> {
    pub map: &'a TerminalMap,
    pub depths: &'a DepthTable,
    pub dependences: Option<&'a Dependences>,
    pub lau: &'a SymbolicaLaurent,
    /// The exchanged basis (None = the old basis) and the parser it needs.
    pub exchange: Option<(&'a Exchange, &'a SymbolicaAlgebra)>,
}

impl RankData<'_> {
    fn depth(&self, m: &str) -> Option<i64> {
        if let Some((x, _)) = self.exchange
            && let Some(q) = x.new_q.get(m)
        {
            return Some(*q);
        }
        self.depths.known_through.get(m).copied()
    }
}

/// The row of one request at one prime (interfaces/tadic-job.md 4).
pub fn request_row(fp: &Fp, data: &RankData<'_>, request: &Value) -> RowOutcome {
    match request_row_inner(fp, data, request) {
        Ok(r) => r,
        Err(why) => RowOutcome::NotDecided(why),
    }
}

fn request_row_inner(fp: &Fp, data: &RankData<'_>, request: &Value) -> Result<RowOutcome, String> {
    if request["status"] != "OK" {
        return Ok(RowOutcome::NotDecided(format!(
            "REQUEST:{}",
            request["error"]["code"].as_str().unwrap_or("not OK")
        )));
    }
    let terms = terms_at_prime(request, fp.p)?;
    // target precision per master: -q (orders below -q needed); planned per product
    let mut acc: BTreeMap<String, Lau> = BTreeMap::new();
    let target = |m: &str| -> Option<i64> { data.depth(m).map(|q| -q) };
    for t in &terms {
        let r = fp.term_series(t.valuation, t.precision, &t.coeffs);
        let row = data.map.row_of(&t.basis);
        let contribs: Vec<(String, Lau)> = match row {
            TermRow::Closed => continue,
            // TRICKS 59: ANY term on an unmapped terminal (every order of it unknown, the regular part too) makes the
            // request incomplete, whatever its series: never counted, neither as zero nor as non-zero
            TermRow::Unmapped(_) => {
                return Ok(RowOutcome::NotDecided(format!(
                    "UNMAPPED_TERMINAL:{}",
                    t.basis
                )));
            }
            TermRow::Itself(m) => {
                let u = fp.gamma_unit(&t.tag, target(&m).unwrap_or(0) - fp.val(&r) + 1)?;
                vec![(m, fp.mul(&r, &u))]
            }
            TermRow::Rows(entries) => {
                let mut v = vec![];
                for e in entries {
                    let Some(m) = e.master else { continue };
                    let tgt = target(&m).unwrap_or(0);
                    let mut sum: Option<Lau> = None;
                    for part in &e.parts {
                        let vrow = match part.coeff.valuation() {
                            rustred_campaign_core::epsdepth::Val::Order(o) => o,
                            _ => continue,
                        };
                        // enough of the row and of the unit for the target of this master
                        let need_row = tgt - fp.val(&r) + 1;
                        let rs =
                            fp.rational_series(data.lau, &part.coeff, need_row.max(vrow + 1))?;
                        let mut tag = t.tag.clone();
                        for (&m2, &e2) in &part.tag {
                            *tag.entry(m2).or_insert(0) += e2;
                        }
                        tag.retain(|&m2, e2| m2 != 0 && *e2 != 0);
                        let u = fp.gamma_unit(&tag, tgt - fp.val(&r) - vrow + 1)?;
                        let prod = fp.mul(&fp.mul(&r, &rs), &u);
                        sum = Some(match sum {
                            None => prod,
                            Some(s) => fp.add(&s, &prod),
                        });
                    }
                    if let Some(s) = sum {
                        v.push((m, s));
                    }
                }
                v
            }
        };
        for (m, c) in contribs {
            let next = match acc.remove(&m) {
                None => c,
                Some(a) => fp.add(&a, &c),
            };
            acc.insert(m, next);
        }
    }
    // the exchanged basis: rotate every exchanged master's coefficient onto its rule's targets (exact in d)
    if let Some((x, alg)) = data.exchange {
        let present: Vec<String> = x
            .exchanged()
            .filter(|m| acc.contains_key(*m))
            .cloned()
            .collect();
        for m in present {
            let cm = acc.remove(&m).expect("present");
            for (target, coeff) in x.rule(alg, data.lau, &m)? {
                let vt = match coeff.valuation() {
                    rustred_campaign_core::epsdepth::Val::Order(o) => o,
                    _ => continue,
                };
                let tq = data.depth(&target).unwrap_or(0);
                let need = (-tq - fp.val(&cm) + 1).max(vt + 1);
                let s = fp.rational_series(data.lau, &coeff, need)?;
                let prod = fp.mul(&cm, &s);
                let next = match acc.remove(&target) {
                    None => prod,
                    Some(a) => fp.add(&a, &prod),
                };
                acc.insert(target, next);
            }
        }
    }
    let mut entries: BTreeMap<String, u64> = BTreeMap::new();
    let mut needs = BTreeMap::new();
    let mut beyond = vec![];
    for (m, c) in &acc {
        let v = fp.val(c);
        let need = if c.s.is_some() { (-v).max(0) } else { 0 };
        needs.insert(m.clone(), need);
        if data.depths.fully_known.contains(m) {
            continue;
        }
        let Some(q) = data.depth(m) else {
            if c.s.is_some() && v < 0 {
                return Ok(RowOutcome::NotDecided(format!("MISSING_DEPTH:{m}")));
            }
            continue;
        };
        if c.prec < -q {
            return Ok(RowOutcome::NotDecided(format!(
                "PRECISION:{m} (known below eps^{}, needed below eps^{})",
                c.prec, -q
            )));
        }
        if need > q {
            beyond.push((m.clone(), need, q));
        }
        for j in (q + 1)..=need {
            let Some(x) = fp.coeff(c, -j) else { continue };
            if fp.f.is_zero(&x) {
                continue;
            }
            let mut add = |sym: String, val: Elem| {
                let cur = entries
                    .get(&sym)
                    .map(|&y| fp.f.to_element(y))
                    .unwrap_or_else(|| fp.f.zero());
                let s = fp.f.add(&cur, &val);
                if fp.f.is_zero(&s) {
                    entries.remove(&sym);
                } else {
                    entries.insert(sym, fp.f.from_element(&s));
                }
            };
            match data.dependences.and_then(|d| d.map.get(&(m.clone(), j))) {
                Some(comb) => {
                    for (sym, cr) in comb {
                        add(sym.clone(), fp.f.mul(&x, &fp.rat(cr)?));
                    }
                }
                None => add(format!("{m}[eps^{j}]"), x),
            }
        }
    }
    Ok(RowOutcome::Row {
        entries,
        needs,
        beyond,
    })
}

/// Rank and pivots of rows at one prime (Symbolica `Matrix<Zp64>::row_reduce`): pivots = the symbols of the pivot
/// columns of the reduced echelon form, columns in the sorted order of `symbols`.
pub fn rank_at_prime(
    fp: &Fp,
    rows: &[&BTreeMap<String, u64>],
) -> Result<(usize, Vec<String>), String> {
    let symbols: Vec<String> = rows
        .iter()
        .flat_map(|r| r.keys().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if rows.is_empty() || symbols.is_empty() {
        return Ok((0, vec![]));
    }
    let (nr, nc) = (rows.len(), symbols.len());
    let col: BTreeMap<&str, usize> = symbols
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    let mut data = vec![fp.f.zero(); nr * nc];
    for (i, r) in rows.iter().enumerate() {
        for (s, &v) in r.iter() {
            data[i * nc + col[s.as_str()]] = fp.f.to_element(v % fp.p);
        }
    }
    let mut m = Matrix::from_linear(data, nr as u32, nc as u32, fp.f.clone())
        .map_err(|e| format!("matrix: {e:?}"))?;
    let rank = m.row_reduce(nc as u32);
    let mut pivots = vec![];
    for i in 0..rank {
        if let Some(c) = (0..nc).find(|&c| !fp.f.is_zero(&m[(i as u32, c as u32)])) {
            pivots.push(symbols[c].clone());
        }
    }
    Ok((rank, pivots))
}

/// The cell's rank record from ONE result (both seeds inside it): per prime the rows of the rank requests (labels
/// with `rank_prefix`), the rank and pivots; per request needs and the polar part beyond the known depths.
pub fn cell_rank(
    data: &RankData<'_>,
    result: &Value,
    rank_prefix: &str,
    transcendental_seed: u64,
) -> Value {
    let primes: Vec<(u64, u64)> = {
        // (prime, seed) from the per-prime series of the first OK request
        let mut v = BTreeSet::new();
        for r in result["requests"].as_array().into_iter().flatten() {
            for t in r["terms"].as_array().into_iter().flatten() {
                for pp in t["per_prime"].as_array().into_iter().flatten() {
                    if let (Some(p), Some(s)) = (
                        pp["prime"].as_str().and_then(|x| x.parse::<u64>().ok()),
                        pp["seed"].as_u64(),
                    ) {
                        v.insert((p, s));
                    }
                }
            }
        }
        v.into_iter().collect()
    };
    let requests: Vec<&Value> = result["requests"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let mut per_prime = vec![];
    let mut per_request: BTreeMap<String, Value> = BTreeMap::new();
    let mut undecided: Vec<String> = vec![];
    let mut transcendental_used = false;
    for &(p, seed) in &primes {
        let fp = match Fp::new(p, transcendental_seed) {
            Ok(f) => f,
            Err(e) => {
                undecided.push(format!("prime {p}: {e}"));
                continue;
            }
        };
        let mut rows = vec![];
        let mut keep = vec![];
        for r in &requests {
            let label = r["label"].as_str().unwrap_or("?").to_string();
            let out = request_row(&fp, data, r);
            match &out {
                RowOutcome::Row {
                    entries,
                    needs,
                    beyond,
                } => {
                    let e = per_request
                        .entry(label.clone())
                        .or_insert_with(|| json!({"label": label, "per_prime": []}));
                    e["per_prime"].as_array_mut().expect("array").push(json!({"prime": p.to_string(), "seed": seed,
                        "row_nonzero": entries.len(), "row": entries.iter().map(|(k, v)| (k.clone(), json!(v.to_string()))).collect::<serde_json::Map<String, Value>>(), "needs": needs,
                        "beyond": beyond.iter().map(|(m, n, q)| json!({"master": m, "need": n, "q": q})).collect::<Vec<_>>()}));
                    if label.starts_with(rank_prefix) {
                        keep.push(entries.clone());
                    }
                }
                RowOutcome::NotDecided(why) => {
                    let e = per_request
                        .entry(label.clone())
                        .or_insert_with(|| json!({"label": label, "per_prime": []}));
                    e["per_prime"]
                        .as_array_mut()
                        .expect("array")
                        .push(json!({"prime": p.to_string(), "seed": seed,
                        "not_decided": why}));
                    if label.starts_with(rank_prefix) {
                        undecided.push(format!("{label} at {p}: {why}"));
                    }
                }
            }
        }
        for k in &keep {
            rows.push(k);
        }
        match rank_at_prime(&fp, &rows) {
            Ok((rank, pivots)) => per_prime.push(
                json!({"prime": p.to_string(), "seed": seed, "rows": rows.len(),
                "rank": rank, "pivots": pivots}),
            ),
            Err(e) => undecided.push(format!("rank at {p}: {e}")),
        }
        transcendental_used = true;
    }
    // certificates: plan max need and max series length per request and seed (the replay's own figures)
    for r in &requests {
        let label = r["label"].as_str().unwrap_or("?").to_string();
        let certs: Vec<Value> = r["certificate"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| {
                json!({"seed": c["seed"],
                "plan_max_need": c.get("plan_max_need").cloned().or_else(|| c["plan"].get("max_need").cloned()),
                "max_series_length": c.get("max_series_length").cloned().or_else(|| c["plan"].get("max_length").cloned()),
                "trace_ops": c.get("trace_ops").cloned()})
            })
            .collect();
        if let Some(e) = per_request.get_mut(&label) {
            e["replay"] = json!(certs);
        }
    }
    let ranks: BTreeSet<(u64, Vec<String>)> = per_prime
        .iter()
        .map(|x| {
            (
                x["rank"].as_u64().unwrap_or(0),
                x["pivots"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|s| s.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
            )
        })
        .collect();
    let n_rank_requests = requests
        .iter()
        .filter(|r| {
            r["label"]
                .as_str()
                .is_some_and(|l| l.starts_with(rank_prefix))
        })
        .count();
    let seeds: BTreeSet<u64> = primes.iter().map(|x| x.1).collect();
    // TRICKS 59: a cell touching an unmapped terminal is INCOMPLETE (listed), never a count
    let unmapped: BTreeSet<String> = undecided
        .iter()
        .filter_map(|u| u.split("UNMAPPED_TERMINAL:").nth(1))
        .map(|s| s.split_whitespace().next().unwrap_or(s).to_string())
        .collect();
    let (verdict, rank, pivots) = if !unmapped.is_empty() {
        (
            format!(
                "incomplete: unmapped terminal ({})",
                unmapped.iter().cloned().collect::<Vec<_>>().join(", ")
            ),
            Value::Null,
            Value::Null,
        )
    } else if !undecided.is_empty() {
        (
            format!("not decided ({})", undecided[0]),
            Value::Null,
            Value::Null,
        )
    } else if seeds.len() < 2 {
        (
            "not decided (SINGLE_SEED)".to_string(),
            Value::Null,
            Value::Null,
        )
    } else if ranks.len() != 1 {
        (
            "not decided (RANK_DISAGREEMENT between primes / seeds)".to_string(),
            Value::Null,
            Value::Null,
        )
    } else {
        let (r, p) = ranks.iter().next().cloned().expect("one");
        let stable = n_rank_requests as u64 >= r + 3;
        (
            if !stable {
                format!("not decided (NEEDS_MORE_COMBINATIONS: rank {r} from {n_rank_requests})")
            } else if r == 0 {
                "count zero: yes".to_string()
            } else {
                format!("count zero: no (rank {r})")
            },
            json!(r),
            json!(p),
        )
    };
    // cell needs: max over the rank requests of each master's need (= max over the cell's keys w.h.p.)
    let mut cell_needs: BTreeMap<String, i64> = BTreeMap::new();
    for (label, e) in &per_request {
        if !label.starts_with(rank_prefix) {
            continue;
        }
        for pp in e["per_prime"].as_array().into_iter().flatten() {
            for (m, n) in pp["needs"].as_object().into_iter().flatten() {
                let n = n.as_i64().unwrap_or(0);
                let c = cell_needs.entry(m.clone()).or_insert(0);
                *c = (*c).max(n);
            }
        }
    }
    let beyond: Vec<Value> = cell_needs
        .iter()
        .filter_map(|(m, &n)| {
            data.depth(m)
                .filter(|&q| n > q)
                .map(|q| json!({"master": m, "need": n, "q": q}))
        })
        .collect();
    // TRICKS 57 / 60: in the exchanged basis the count is an OBSERVATION, never a verdict. Violations (masters beyond depth)
    // are "an exchange still to be found": the PROPORTIONALITY CHECK (the violating polar rows are multiples of one row:
    // rank <= 1 of the rows without dependences, at every prime) and the CANDIDATE exchange = the master in the most
    // (master, request) violations (the exchange loop's first rule; ties: the master name).
    let mut exchange_observation = Value::Null;
    let verdict = if let Some((_, _)) = data.exchange {
        let mut count: BTreeMap<String, usize> = BTreeMap::new();
        for e in per_request.values() {
            if let Some(pp) = e["per_prime"].as_array().and_then(|a| a.first()) {
                for b in pp["beyond"].as_array().into_iter().flatten() {
                    *count
                        .entry(b["master"].as_str().unwrap_or("").to_string())
                        .or_insert(0) += 1;
                }
            }
        }
        if count.is_empty() {
            exchange_observation = json!({"violations": 0});
            format!("OBSERVATION (exchanged basis): 0 masters beyond depth; {verdict}")
        } else {
            let naive = RankData {
                map: data.map,
                depths: data.depths,
                dependences: None,
                lau: data.lau,
                exchange: data.exchange,
            };
            let mut ranks = BTreeSet::new();
            for &(p, _) in &primes {
                let Ok(fp) = Fp::new(p, transcendental_seed) else {
                    continue;
                };
                let rows: Vec<BTreeMap<String, u64>> = requests
                    .iter()
                    .filter(|r| {
                        r["label"]
                            .as_str()
                            .is_some_and(|l| l.starts_with(rank_prefix))
                    })
                    .filter_map(|r| match request_row(&fp, &naive, r) {
                        RowOutcome::Row { entries, .. } => Some(entries),
                        RowOutcome::NotDecided(_) => None,
                    })
                    .collect();
                let refs: Vec<&BTreeMap<String, u64>> = rows.iter().collect();
                if let Ok((rk, _)) = rank_at_prime(&fp, &refs) {
                    ranks.insert(rk);
                }
            }
            let proportional = ranks.len() == 1 && ranks.iter().all(|&r| r <= 1);
            let candidate = count
                .iter()
                .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
                .map(|(m, n)| json!({"master": m, "violations_cleared": n}));
            exchange_observation = json!({"violations": count.values().sum::<usize>(),
                "masters_beyond": count, "proportional": proportional, "naive_ranks": ranks,
                "candidate_exchange": candidate,
                "wording": "an exchange still to be found (TRICKS 60): the published pole bound fails on these keys"});
            format!(
                "OBSERVATION (exchanged basis): an exchange still to be found ({} violations; proportional: {}); {verdict}",
                count.values().sum::<usize>(),
                if proportional { "yes" } else { "no" }
            )
        }
    } else {
        verdict
    };
    json!({"version": POLERANK_VERSION, "rank_prefix": rank_prefix, "rank_requests": n_rank_requests,
        "verdict": verdict, "rank": rank, "pivots": pivots,
        "rank_kind": if data.dependences.is_some() {"rank of rho (dependences substituted)"} else {"NAIVE UPPER BOUND (no dependence table: every master order beyond its known depth is its own symbol)"},
        "depths": data.depths.label, "dependences": data.dependences.map(|d| d.label.clone()),
        "basis": data.exchange.map(|(x, _)| format!("exchanged ({})", x.label)).unwrap_or_else(|| "old".into()),
        "per_prime": per_prime, "requests": per_request.into_values().collect::<Vec<_>>(),
        "cell_needs": cell_needs, "cell_beyond": beyond,
        "exchange_observation": exchange_observation,
        "transcendental_substitution": {"used": transcendental_used, "seed": transcendental_seed,
            "constants": "gamma_E, pi^2, zeta(odd) seeded random in F_p; zeta(2k) = rational x pi^2k"},
        "failure_probability": {
            "weights": "<= rank / 2^62 per cell (Schwartz-Zippel on a maximal minor; weights uniform in [1, 2^62))",
            "transcendentals": "<= (rank x series length) / p per prime",
            "mod_p": "the rank mod p is a lower bound of the rank over Q; equality fails only if p divides a maximal minor (no height bound: two primes per seed and two seeds must agree)"}})
}

/// The node-level EXACT route (lane tadicjob, the certificate's second route; interfaces/tadic-job.md 3): our own
/// composition of ONE candidate rr_node table onto its masters and exits, exactly in Q(d) (Symbolica rational
/// functions), independent of the replay: first-match dispatch by the campaign core (`read_table_file`,
/// `cover::Table`, `Dispatcher::first_match`), the rule point (`closure::bodies::rule_point`), right-hand-side keys by
/// `parse_rhs_pattern`, coefficients read at the rule point (`SymbolicaLaurent::coeff_from_text`); a term on a key
/// with a positive numerator index must have an exactly zero coefficient (else refused); keys outside the node's top
/// sector (a line slot of the header mask at power <= 0) are exits; masters are the table's `residual` records (or the
/// caller's list). Generic: arity, mask, masters and keys are data.
pub mod nodeexact {
    use super::*;
    use crate::epsdepth::{SymbolicaExact, TRational};
    use rustred_campaign_core::closure::bodies::{RhsSlot, parse_rhs_pattern, rule_point};
    use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table};
    use rustred_campaign_core::domain::DomainOptions;
    use rustred_campaign_core::record::{ReadOptions, read_table_file};
    use std::collections::HashMap;

    /// A loaded candidate table.
    pub struct NodeTable {
        disp: Dispatcher,
        lines: Vec<String>,
        rule_lines: Vec<usize>,
        mask: Vec<bool>,
        arity: usize,
        pub masters: Vec<Vec<i32>>,
        /// Sector masks (slot '1' = power >= 1) of the table's `zero_sectors` record: keys there are exactly zero.
        zero: BTreeSet<String>,
    }

    fn key_text(k: &[i32]) -> String {
        format!(
            "I({})",
            k.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    /// `I(a,b,..)` -> key.
    pub fn parse_key(s: &str) -> Result<Vec<i32>, String> {
        s.trim()
            .strip_prefix("I(")
            .and_then(|x| x.strip_suffix(')'))
            .ok_or(format!("key {s}"))?
            .split(',')
            .map(|x| x.trim().parse::<i32>().map_err(|e| format!("key {s}: {e}")))
            .collect()
    }

    impl NodeTable {
        pub fn load(
            path: &std::path::Path,
            masters: Option<Vec<Vec<i32>>>,
        ) -> Result<NodeTable, String> {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let lines: Vec<String> = text.lines().map(str::to_string).collect();
            let tf = read_table_file(path, ReadOptions::default())
                .map_err(|e| format!("table: {e:?}"))?;
            let header = tf.header.as_ref().ok_or("table without header")?;
            let mask: Vec<bool> = header.raw["mask"]
                .as_str()
                .ok_or("header without mask")?
                .chars()
                .map(|c| c == '1')
                .collect();
            let arity = mask.len();
            let table = Table::from_table_file(
                "node",
                path.display().to_string(),
                &tf,
                DomainOptions::default(),
            )
            .map_err(|e| format!("table domains: {e:?}"))?;
            let disp =
                Dispatcher::new(vec![table], vec![0]).map_err(|e| format!("dispatcher: {e:?}"))?;
            let rule_lines = tf.rules.iter().map(|r| r.line_no).collect();
            let mut found = vec![];
            let mut zero = BTreeSet::new();
            for l in &lines {
                let v: Value = match serde_json::from_str(l) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if v["type"] == "residual" {
                    match &v["key"] {
                        Value::String(s) => found.push(parse_key(s)?),
                        Value::Array(k) => found.push(
                            k.iter()
                                .filter_map(|x| x.as_i64().map(|y| y as i32))
                                .collect(),
                        ),
                        _ => return Err("residual record without key".into()),
                    }
                }
                if v["type"] == "zero_sectors" {
                    if v["scope"].as_str().is_some_and(|s| s != "downset") {
                        return Err(format!("zero_sectors scope {} not read", v["scope"]));
                    }
                    for m in v["masks"].as_array().into_iter().flatten() {
                        zero.insert(m.as_str().ok_or("zero sector mask")?.to_string());
                    }
                }
            }
            let masters = masters.unwrap_or(found);
            Ok(NodeTable {
                disp,
                lines,
                rule_lines,
                mask,
                arity,
                masters,
                zero,
            })
        }

        fn in_top_sector(&self, k: &[i32]) -> bool {
            k.iter().zip(&self.mask).all(|(&x, &line)| !line || x >= 1)
        }

        fn support_violated(&self, k: &[i32]) -> bool {
            k.iter().zip(&self.mask).any(|(&x, &line)| !line && x > 0)
        }

        /// The exact composition of `key` onto masters and exits (basis text -> rational function of t = d - d0).
        pub fn compose(
            &self,
            alg: &SymbolicaAlgebra,
            lau: &SymbolicaLaurent,
            ex: &SymbolicaExact,
            key: &[i32],
            memo: &mut HashMap<Vec<i32>, BTreeMap<String, TRational>>,
            depth: usize,
        ) -> Result<BTreeMap<String, TRational>, String> {
            if let Some(m) = memo.get(key) {
                return Ok(m.clone());
            }
            if depth > 100_000 {
                return Err("composition deeper than 100000 steps (a cycle?)".into());
            }
            let one = || -> TRational {
                let c = lau.coeff_from_text(alg, "1", 0, &[]).expect("one");
                ex.rational(&c).expect("one is not zero")
            };
            let sector: String = key
                .iter()
                .map(|&x| if x >= 1 { '1' } else { '0' })
                .collect();
            if self.zero.contains(&sector) {
                memo.insert(key.to_vec(), BTreeMap::new());
                return Ok(BTreeMap::new());
            }
            if self.masters.iter().any(|m| m.as_slice() == key) || !self.in_top_sector(key) {
                let r = BTreeMap::from([(key_text(key), one())]);
                memo.insert(key.to_vec(), r.clone());
                return Ok(r);
            }
            let rr = match self
                .disp
                .first_match(key)
                .map_err(|e| format!("dispatch {key:?}: {e:?}"))?
            {
                Dispatch::Covered(r) => r,
                other => {
                    return Err(format!(
                        "{}: not covered by the table ({other:?})",
                        key_text(key)
                    ));
                }
            };
            let domain = &self.disp.tables()[0].domains[rr.record];
            let mut pt = vec![];
            if !rule_point(domain, key, &mut pt) {
                return Err(format!("{}: no rule point", key_text(key)));
            }
            let line_no = self.rule_lines[rr.source_index];
            let rec: Value = serde_json::from_str(&self.lines[line_no - 1])
                .map_err(|e| format!("rule line: {e}"))?;
            let rel = crate::valuation::relation::RelationText::of_record(&rec)?;
            let mut out: BTreeMap<String, TRational> = BTreeMap::new();
            for (pat, coeff, _) in &rel.terms {
                let slots = parse_rhs_pattern(pat, self.arity)?;
                let k2: Vec<i32> = slots
                    .iter()
                    .map(|s| match s {
                        RhsSlot::Param { param, offset } => pt[*param] + offset,
                        RhsSlot::Const(c) => *c,
                    })
                    .collect();
                let c = lau.coeff_from_text(alg, coeff, self.arity, &pt)?;
                let Some(cr) = ex.rational(&c) else {
                    continue;
                };
                if self.support_violated(&k2) {
                    return Err(format!(
                        "{}: term {} outside the support with a non-zero coefficient",
                        key_text(key),
                        key_text(&k2)
                    ));
                }
                if k2 == key {
                    return Err(format!(
                        "{}: the rule serves the key with itself",
                        key_text(key)
                    ));
                }
                for (b, v) in self.compose(alg, lau, ex, &k2, memo, depth + 1)? {
                    let add = &cr * &v;
                    let next = match out.remove(&b) {
                        None => add,
                        Some(a) => &a + &add,
                    };
                    if !next.numerator.is_zero() {
                        out.insert(b, next);
                    }
                }
            }
            memo.insert(key.to_vec(), out.clone());
            Ok(out)
        }
    }

    /// Compare a node-level replay result with the exact composition: every residue of every term at every prime and
    /// every order below its precision. Returns (residues compared, mismatches as JSON).
    pub fn compare(
        alg: &SymbolicaAlgebra,
        table: &NodeTable,
        result: &Value,
        d0: &str,
    ) -> Result<(u64, Vec<Value>), String> {
        let lau = SymbolicaLaurent::new(d0)?;
        let lau0 = SymbolicaLaurent::new("0")?;
        let ex = SymbolicaExact::new()?;
        let mut memo = HashMap::new();
        let mut compared = 0u64;
        let mut bad = vec![];
        for q in result["requests"].as_array().ok_or("result: requests")? {
            if q["status"] != "OK" {
                continue;
            }
            let label = q["label"].as_str().unwrap_or("?");
            // the exact request: sum of weight x composition
            let mut exact: BTreeMap<String, TRational> = BTreeMap::new();
            for i in q["inputs"].as_array().into_iter().flatten() {
                let k = parse_key(i["key"].as_str().ok_or("input key")?)?;
                let w = i["weight"].as_i64().unwrap_or(1);
                let wc = ex
                    .rational(&lau.coeff_from_text(alg, &w.to_string(), 0, &[])?)
                    .ok_or("zero weight")?;
                for (b, v) in table.compose(alg, &lau, &ex, &k, &mut memo, 0)? {
                    let add = &wc * &v;
                    let next = match exact.remove(&b) {
                        None => add,
                        Some(a) => &a + &add,
                    };
                    if !next.numerator.is_zero() {
                        exact.insert(b, next);
                    }
                }
            }
            let mut seen = BTreeSet::new();
            for t in q["terms"].as_array().into_iter().flatten() {
                let basis = t["basis"].as_str().unwrap_or("").to_string();
                seen.insert(basis.clone());
                let tc = exact
                    .get(&basis)
                    .map(|r| lau0.coeff_from_zpolys(&r.numerator, &r.denominator, 0));
                for pp in t["per_prime"].as_array().into_iter().flatten() {
                    let p: u64 = pp["prime"]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .ok_or("prime")?;
                    let prec = pp["precision"].as_i64().ok_or("precision")?;
                    let v = pp["valuation"].as_i64();
                    let cs: Vec<u64> = pp["coefficients"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|c| c.as_str().and_then(|s| s.parse().ok()))
                        .collect();
                    let fp = Fp::new(p, 0)?;
                    let low = match (&tc, v) {
                        (Some(Ok(c)), _) => match c.valuation() {
                            rustred_campaign_core::epsdepth::Val::Order(o) => o.min(v.unwrap_or(o)),
                            _ => v.unwrap_or(prec),
                        },
                        _ => v.unwrap_or(prec),
                    };
                    let series = match &tc {
                        Some(Ok(c)) => lau0.expand_coeff(c, prec)?,
                        Some(Err(e)) => return Err(e.clone()),
                        None => None,
                    };
                    for k in low..prec {
                        let got = match v {
                            Some(v0) if k >= v0 => cs.get((k - v0) as usize).copied().unwrap_or(0),
                            _ => 0,
                        };
                        let want = match &series {
                            Some(s) => {
                                let r = SymbolicaLaurent::series_coefficient(s, k)
                                    .unwrap_or_else(|| Q.zero());
                                fp.f.from_element(&fp.rat(&r)?)
                            }
                            None => 0,
                        };
                        compared += 1;
                        if got != want {
                            bad.push(json!({"label": label, "term": basis, "prime": p.to_string(), "order": k,
                                "replay": got.to_string(), "exact": want.to_string()}));
                        }
                    }
                }
            }
            // an exact term the replay does not list must have no coefficient below the precision
            for (b, r) in &exact {
                if seen.contains(b) {
                    continue;
                }
                let c = lau0.coeff_from_zpolys(&r.numerator, &r.denominator, 0)?;
                if let rustred_campaign_core::epsdepth::Val::Order(o) = c.valuation()
                    && o < result["expansion"]["order"].as_i64().unwrap_or(0)
                {
                    bad.push(
                        json!({"label": label, "term": b, "missing_in_replay": true, "order": o}),
                    );
                }
            }
        }
        Ok((compared, bad))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamma_tags_parse() {
        let t = parse_gamma_tag("GammaTag([(-2,5),(-1,-10),(1,-5)])").unwrap();
        assert_eq!(t, BTreeMap::from([(-2, 5), (-1, -10), (1, -5)]));
        assert!(parse_gamma_tag("GammaTag([])").unwrap().is_empty());
        assert!(parse_gamma_tag("").unwrap().is_empty());
    }

    #[test]
    fn even_zetas_are_the_known_rationals() {
        let z = zeta_even_rationals(4);
        assert_eq!(z[1], Rational::new(Integer::from(1), Integer::from(6)));
        assert_eq!(z[2], Rational::new(Integer::from(1), Integer::from(90)));
        assert_eq!(z[3], Rational::new(Integer::from(1), Integer::from(945)));
        assert_eq!(z[4], Rational::new(Integer::from(1), Integer::from(9450)));
    }
}
