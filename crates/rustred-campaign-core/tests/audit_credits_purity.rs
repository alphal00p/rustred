//! Independent audit of the symmetry-credit rule as changed by lane import3 (lane audit-credits, 2026-11-14):
//! PER-SLOT NUMERATOR PURITY. Every image the credit policy can grant must be the exact one-term relation
//! I(key) = I(image) (coefficient 1). Two INDEPENDENT routes:
//!  (a) GEOMETRY: the element's momentum matrix T (the verifier's proof matrix) and the family momenta; for every slot
//!      with a non-zero power (and every eps-shifted slot) the momentum image c_i T must be +- the momentum of one slot t
//!      (same eps shift); the image key is then b[t] = a[i]. No `SlotImage` data and no code of `symmetry.rs` is used.
//!  (b) the expanded symmetry row of that element at the key (`rows_at(.., Expanded, ..)`): it must be exactly
//!      {key: +1, image: -1}.
//!
//! The policy side is exercised twice: the public `partial_image` / `apply_framed` for EVERY element (the selection
//! rule of `element_image` replicated), and end to end through the public `CreditChecker::serve` (which runs
//! `element_image` and the adopted condition) on a catch-all table, where the FIRST admissible element is credited.
//! Families: a two-loop sunset (inline), the m2 fixture nodes, the handoff families (T162, T178, T266 presented from
//! the jointest9 header) and the 283-family set (charts from the family flags). Nothing here is node-specific code.

use rustred_campaign_core::closure::{NoFacts, RuleBodies, Sectors, Tier};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{
    DeriveOptions, SymRowMode, VerifiedGroup, rows_at, verify_derived,
};
use rustred_campaign_core::symmetry::{CreditChecker, CreditPolicy, apply_framed};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const WS: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev";

fn ws() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_WS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(WS))
}

/// The chart flags of a family text: the last argument of every `prop(..)` (1 = line).
fn flags(text: &str) -> Vec<bool> {
    let mut out = vec![];
    let mut rest = text;
    while let Some(p) = rest.find("prop(") {
        let tail = &rest[p + 5..];
        // matching close paren of prop(
        let mut depth = 1;
        let mut end = 0;
        for (i, ch) in tail.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let inner = &tail[..end];
        let last = inner.rsplit(',').next().unwrap().trim();
        out.push(last == "1");
        rest = &tail[end..];
    }
    out
}

/// Tiny deterministic generator (xorshift64*).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i32
    }
}

fn canon(v: &[i64]) -> Vec<i64> {
    match v.iter().find(|&&x| x != 0) {
        Some(&x) if x < 0 => v.iter().map(|y| -y).collect(),
        _ => v.to_vec(),
    }
}

/// Fraction-free determinant (Bareiss) of a small integer matrix.
fn det(mut a: Vec<Vec<i128>>) -> i128 {
    let n = a.len();
    let mut sign = 1i128;
    let mut prev = 1i128;
    for k in 0..n {
        if a[k][k] == 0 {
            let Some(p) = (k + 1..n).find(|&r| a[r][k] != 0) else {
                return 0;
            };
            a.swap(k, p);
            sign = -sign;
        }
        for i in k + 1..n {
            for j in k + 1..n {
                a[i][j] = (a[i][j] * a[k][k] - a[i][k] * a[k][j]) / prev;
            }
        }
        prev = a[k][k];
    }
    sign * a[n - 1][n - 1]
}

/// Route (a): the one-term image of `key` under the momentum map T, or why there is none.
fn geometric_image(fam: &Family, t: &[i64], key: &[i32]) -> Result<Vec<i32>, String> {
    let l = fam.loops();
    let w = l + 1;
    let c = fam.momenta();
    let ks = fam.kshift();
    let n = fam.n();
    // measure: |det| of the loop block = 1, q -> +-q (row of q is +- e_q)
    let block: Vec<Vec<i128>> = (0..l)
        .map(|i| (0..l).map(|j| t[i * w + j] as i128).collect())
        .collect();
    if det(block).abs() != 1 {
        return Err("loop block not unimodular".into());
    }
    if (0..l).any(|j| t[l * w + j] != 0) || t[l * w + l].abs() != 1 {
        return Err("q is not mapped to +-q".into());
    }
    let cans: Vec<Vec<i64>> = c.iter().map(|m| canon(m)).collect();
    let mut b = vec![0i32; n];
    let mut hit = vec![false; n];
    for i in 0..n {
        if key[i] == 0 && ks[i] == 0 {
            continue;
        }
        let img: Vec<i64> = (0..w)
            .map(|j| (0..w).map(|v| c[i][v] * t[v * w + j]).sum())
            .collect();
        let ci = canon(&img);
        let Some(tg) = (0..n).find(|&s| cans[s] == ci) else {
            return Err(format!("slot {i} (power {}) maps to a combination", key[i]));
        };
        if ks[tg] != ks[i] {
            return Err(format!("slot {i} -> {tg} changes the eps shift"));
        }
        if key[i] != 0 {
            if std::mem::replace(&mut hit[tg], true) {
                return Err(format!("slot {tg} hit twice"));
            }
            b[tg] = key[i];
        }
    }
    Ok(b)
}

/// Route (b): the expanded row of element `ei` at `key` is exactly {key: 1, image: -1}.
fn row_is_one_term(
    vg: &VerifiedGroup,
    ei: usize,
    key: &[i32],
    image: &[i32],
) -> Result<(), String> {
    let seed = (0u64, key.iter().map(|&x| x as i64).collect::<Vec<i64>>());
    let rows = rows_at(vg, &seed, SymRowMode::Expanded, 1 << 14)
        .map_err(|e| format!("UNAVAILABLE: {e}"))?;
    let Some(r) = rows.iter().find(|r| r.element == ei) else {
        return Err("no expanded row for the element at the key".into());
    };
    let im: Vec<i64> = image.iter().map(|&x| x as i64).collect();
    let mut want: BTreeMap<Vec<i64>, (i128, i128)> = BTreeMap::new();
    want.insert(seed.1.clone(), (1, 1));
    want.insert(im, (-1, 1));
    let got: BTreeMap<Vec<i64>, (i128, i128)> = r
        .terms
        .iter()
        .map(|((m, k), q)| {
            assert_eq!(*m, 0);
            (k.clone(), (q.n, q.d))
        })
        .collect();
    if got != want {
        return Err(format!(
            "row has {} terms: {:?}",
            got.len(),
            got.iter().take(4).collect::<Vec<_>>()
        ));
    }
    Ok(())
}

/// The image the policy grants for element `ei` (the selection of `symmetry::element_image`, replicated on the
/// public API): a key with a non-zero numerator and an element outside `pure_numerator_maps` goes through
/// `partial_image`, every other key through `apply_framed`.
fn granted(pol: &CreditPolicy, ei: usize, key: &[i32], lines: &[bool]) -> Option<Vec<i32>> {
    let has_num = key.iter().zip(lines).any(|(&v, &l)| !l && v != 0);
    if has_num && !pol.pure_numerator_maps().contains(&ei) {
        pol.partial_image(ei, key, lines)
    } else {
        apply_framed(&pol.group()[ei], key, lines)
    }
}

#[derive(Default, Debug)]
struct Tally {
    families: usize,
    elements: usize,
    keys: usize,
    pairs: usize,
    granted: usize,
    granted_numerator: usize,
    granted_partial_only: usize,
    verified_geometry: usize,
    verified_rows: usize,
    /// the expanded rows of SOME element at the key exceed the expansion bound (the row route is unavailable there;
    /// the geometric route still decides)
    rows_unavailable: usize,
    refuted: Vec<String>,
    one_term_rows_not_granted: usize,
    end_to_end_credits: usize,
    end_to_end_numerator: usize,
}

/// Keys: random sample (lines in -1..=3, 0-4 numerators in -3..=-1, sometimes a positive numerator), plus the full
/// small box when the arity allows.
fn keys_for(lines: &[bool], sample: usize, seed: u64) -> Vec<Vec<i32>> {
    let n = lines.len();
    let mut rng = Rng(seed | 1);
    let os: Vec<usize> = (0..n).filter(|&s| !lines[s]).collect();
    let mut out = vec![];
    for i in 0..sample {
        // half the sample in the chart (every line >= 1): the end-to-end checker credits only such keys
        let lo = if i % 2 == 1 { 1 } else { -1 };
        let mut k: Vec<i32> = (0..n)
            .map(|s| if lines[s] { rng.range(lo, 3) } else { 0 })
            .collect();
        if !os.is_empty() {
            let m = rng.range(0, 4.min(os.len() as i32));
            for _ in 0..m {
                let s = os[rng.next() as usize % os.len()];
                k[s] = rng.range(-3, -1);
            }
            if rng.next().is_multiple_of(16) {
                let s = os[rng.next() as usize % os.len()];
                k[s] = 1;
            }
        }
        out.push(k);
    }
    if n <= 9 {
        // full box: lines 0..=2, numerators -2..=0
        let ranges: Vec<(i32, i32)> = (0..n)
            .map(|s| if lines[s] { (0, 2) } else { (-2, 0) })
            .collect();
        let mut k: Vec<i32> = ranges.iter().map(|r| r.0).collect();
        loop {
            out.push(k.clone());
            let mut i = 0;
            loop {
                if i == n {
                    return out;
                }
                if k[i] < ranges[i].1 {
                    k[i] += 1;
                    break;
                }
                k[i] = ranges[i].0;
                i += 1;
            }
        }
    }
    out
}

/// A catch-all table over the chart (one free record) whose body is one subsector term: every image is covered and no
/// top term exists, so `CreditChecker::serve` credits the first element `element_image` admits.
fn catch_all(lines: &[bool]) -> (Dispatcher, RuleBodies) {
    let n = lines.len();
    let mut t = Table::new("ALL".to_string(), n);
    t.push(1, CaseDomain::free(n, Some(lines.to_vec())))
        .unwrap();
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&d);
    let l0 = lines.iter().position(|&l| l).unwrap();
    let term: Vec<String> = (0..n)
        .map(|s| {
            if s == l0 {
                "0".to_string()
            } else {
                format!("n{s}")
            }
        })
        .collect();
    b.set(&d, 0, 0, &[format!("I({})", term.join(","))])
        .unwrap();
    (d, b)
}

fn audit_family(name: &str, text: &str, chart: &[bool], sample: usize, t: &mut Tally) {
    let fam = match Family::parse(text) {
        Ok(f) => f,
        Err(e) => {
            println!("{name}: family not parsed ({e:?}): skipped");
            return;
        }
    };
    let vg = match verify_derived(&fam, chart, &DeriveOptions::default()) {
        Ok(v) => v,
        Err(e) => {
            println!("{name}: group not derived/verified ({e:?}): skipped");
            return;
        }
    };
    let pol = CreditPolicy::from_verified(&vg, chart).unwrap();
    let ne = pol.group().len();
    t.families += 1;
    t.elements += ne;
    let keys = keys_for(
        chart,
        sample,
        fam.source_digest().len() as u64 * 7919 + name.len() as u64,
    );
    t.keys += keys.len();
    let (d, b) = catch_all(chart);
    let cmp = NodeComparator::legacy(chart.len(), None).unwrap();
    let checker = CreditChecker::new(
        &d,
        &b,
        &cmp,
        Sectors::new(chart.to_vec()),
        &NoFacts,
        Tier::K,
    );
    let (mut g0, mut p0) = (t.granted, t.granted_partial_only);
    for k in &keys {
        let has_num = k.iter().zip(chart).any(|(&v, &l)| !l && v != 0);
        for ei in 0..ne {
            t.pairs += 1;
            let mat = &vg.proofs()[ei].matrix;
            let geo = geometric_image(&fam, mat, k);
            match granted(&pol, ei, k, chart) {
                Some(im) if im != *k => {
                    t.granted += 1;
                    if has_num {
                        t.granted_numerator += 1;
                        if !pol.pure_numerator_maps().contains(&ei) {
                            t.granted_partial_only += 1;
                        }
                    }
                    match &geo {
                        Ok(g) if *g == im => t.verified_geometry += 1,
                        other => t.refuted.push(format!(
                            "{name} element {ei} key {k:?}: granted {im:?}, geometry {other:?}"
                        )),
                    }
                    match row_is_one_term(&vg, ei, k, &im) {
                        Ok(()) => t.verified_rows += 1,
                        Err(e) if e.starts_with("UNAVAILABLE") => t.rows_unavailable += 1,
                        Err(e) => t.refuted.push(format!(
                            "{name} element {ei} key {k:?}: granted {im:?}, expanded row: {e}"
                        )),
                    }
                }
                _ => {
                    if let Ok(g) = &geo
                        && g != k
                    {
                        t.one_term_rows_not_granted += 1;
                    }
                }
            }
        }
        // end to end: the credit the public checker gives (first admissible element) is exact
        if k.iter()
            .zip(chart)
            .all(|(&v, &l)| if l { v >= 1 } else { v <= 0 })
            && let Ok(Ok(c)) = checker.serve(k, &pol)
        {
            t.end_to_end_credits += 1;
            if has_num {
                t.end_to_end_numerator += 1;
            }
            let mat = &vg.proofs()[c.element].matrix;
            match geometric_image(&fam, mat, k) {
                Ok(g) if g == c.image => {}
                other => t.refuted.push(format!(
                    "{name} END-TO-END element {} key {k:?}: credited image {:?}, geometry {other:?}",
                    c.element, c.image
                )),
            }
            if let Err(e) = row_is_one_term(&vg, c.element, k, &c.image)
                && !e.starts_with("UNAVAILABLE")
            {
                t.refuted.push(format!(
                    "{name} END-TO-END element {} key {k:?}: expanded row: {e}",
                    c.element
                ));
            }
        }
    }
    g0 = t.granted - g0;
    p0 = t.granted_partial_only - p0;
    println!(
        "{name}: arity {} |G|-1 = {ne}, pure maps {:?}, keys {}, granted {g0} (partial-only numerator {p0})",
        chart.len(),
        pol.pure_numerator_maps(),
        keys.len()
    );
}

const SUNSET: &str = "I(name(S),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k1+k2+q)^2,1),prop(D4,(k1+q)^2,0),prop(D5,(k2+q)^2,0),gram(q,q,1))";

#[test]
fn audit_credits_per_slot_purity_small_families() {
    let mut t = Tally::default();
    audit_family("SUNSET", SUNSET, &flags(SUNSET), 2000, &mut t);
    // eps families: a power shift on a NUMERATOR (its D^(k eps) factor is present at power 0) and on a LINE
    for (name, shift) in [
        ("SUNSET-eps-D4", "D4"),
        ("SUNSET-eps-D5", "D5"),
        ("SUNSET-eps-D1", "D1"),
    ] {
        let text = SUNSET.replace(
            "gram(q,q,1))",
            &format!("gram(q,q,1),power_shift({shift},(4-d)/2))"),
        );
        let before = t.families;
        audit_family(name, &text, &flags(&text), 2000, &mut t);
        assert_eq!(t.families, before + 1, "{name}: eps family not audited");
    }
    let m2 = ws().join("fixtures/campaign-20261114/m2");
    for name in ["TOY2kite", "L3a", "L4a", "T194", "T281"] {
        let p = m2.join(name).join("family.sym");
        let Ok(text) = std::fs::read_to_string(&p) else {
            println!("SKIP {}", p.display());
            continue;
        };
        // the m2 tables use the mask of the header (lines first); the family flags agree
        let chart = flags(&text);
        audit_family(name, &text, &chart, 6000, &mut t);
    }
    let ho = ws().join("handoff/sym-investigation-for-ben");
    for (name, p) in [
        (
            "T162-presented-P3",
            ho.join("t162-bank/family/T162-presented-P3.sym"),
        ),
        ("T178", ho.join("families/T178.sym")),
        ("T266", ho.join("families/T266.sym")),
    ] {
        let Ok(text) = std::fs::read_to_string(&p) else {
            println!("SKIP {}", p.display());
            continue;
        };
        let chart = flags(&text);
        audit_family(name, &text, &chart, 6000, &mut t);
    }
    println!("TALLY {t:#?}");
    assert!(t.families >= 1);
    assert!(
        t.refuted.is_empty(),
        "REFUTED credits: {:#?}",
        &t.refuted[..t.refuted.len().min(20)]
    );
}

/// T266 in the PRESENTED frame of the jointest9 tables (the frame the import and the walk use): the header's
/// `family_source` must equal the handoff family presented by the header's slot permutation (momenta compared), and
/// every image the policy grants in that frame is exact.
#[test]
fn audit_credits_t266_presented_frame() {
    use rustred_campaign_core::import;
    use rustred_campaign_core::import::symmetry::presented_family_text;
    let table = ws().join("SCRATCHPAD/t266/jointest9/F.table.jsonl");
    let fam_p = ws().join("handoff/sym-investigation-for-ben/families/T266.sym");
    if !table.is_file() || !fam_p.is_file() {
        println!("SKIP: T266 data missing");
        return;
    }
    let h = import::lines::read_header("jointest9", &table).unwrap();
    let frame = rustred_campaign_core::key::NodeFrame::from_header(&h).unwrap();
    let cmp = NodeComparator::from_header(&h.raw).unwrap();
    let header_src = h.raw["family_source"].as_str().unwrap();
    let n2o = cmp.presentation().new_to_old().to_vec();
    let fam_text = std::fs::read_to_string(&fam_p).unwrap();
    // the import's own presentation keeps the family's labels (D14 first, ...): Family::parse refuses that text
    // (labels must be D1..Dn in order) -> an explicit `family` file with a non-identity presentation cannot load
    let presented = presented_family_text(&fam_text, &n2o).unwrap();
    println!(
        "presented_family_text + Family::parse: {:?}",
        Family::parse(&presented).err()
    );
    let fh = Family::parse(header_src).unwrap();
    let ff = Family::parse(&fam_text).unwrap();
    println!(
        "T266 presentation new_to_old {n2o:?}; mask {:?}",
        frame.mask()
    );
    for (j, &o) in n2o.iter().enumerate() {
        assert_eq!(
            canon(&fh.momenta()[j]),
            canon(&ff.momenta()[o]),
            "header family_source slot {j} is not family slot {o}"
        );
        assert_eq!(fh.kshift()[j], ff.kshift()[o]);
    }
    assert_eq!(
        flags(header_src),
        frame.mask().to_vec(),
        "header flags differ from the frame mask"
    );
    let mut t = Tally::default();
    audit_family("T266-presented", header_src, frame.mask(), 20000, &mut t);
    println!("TALLY {t:#?}");
    assert!(
        t.refuted.is_empty(),
        "REFUTED credits: {:#?}",
        &t.refuted[..t.refuted.len().min(20)]
    );
}

/// The 283-family set (charts from the family flags; eps families included: power shifts exercise the "power 0 is
/// absent" claim, which FAILS for a slot carrying an eps shift unless that slot is mapped exactly).
#[test]
fn audit_credits_per_slot_purity_283_families() {
    let dir = ws().join("handoff/sym-investigation-for-ben/families-283");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        println!("SKIP {}", dir.display());
        return;
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "sym"))
        .collect();
    files.sort();
    let budget = std::env::var("AUDIT_CREDITS_283_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(400);
    let t0 = std::time::Instant::now();
    let mut t = Tally::default();
    let mut eps = 0;
    for p in &files {
        if t0.elapsed().as_secs() > budget {
            println!("time budget reached after {} families", t.families);
            break;
        }
        let text = std::fs::read_to_string(p).unwrap();
        if text.contains("power_shift") {
            eps += 1;
        }
        let chart = flags(&text);
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        audit_family(&name, &text, &chart, 600, &mut t);
    }
    println!("eps families seen: {eps}");
    println!("TALLY {t:#?}");
    assert!(
        t.refuted.is_empty(),
        "REFUTED credits: {:#?}",
        &t.refuted[..t.refuted.len().min(20)]
    );
}

#[allow(dead_code)]
fn _unused(_: &Path) {}
