//! Symmetry-aware node generation (sym-gen, 2026-11-12; default off).
//!
//! The automorphism group of a node's chart (its top sector: chart lines `n >= 1`, every other slot `n <= 0`) is
//! computed and authenticated by the application-time orbit machinery ([`crate::propagator::orbit::owner_orbit`]:
//! exact enumeration of the integer momentum maps `k -> A k + b q`, `q -> s q`, `|det A| = 1` permuting the chart
//! lines, each generator proved by [`crate::sector::symmetry::verify`]). An element acts on a key EXACTLY when every
//! slot with a nonzero power is sent onto one slot (`D_i = +-D_j`): `I(a) = s(g, a) I(g a)`, `(g a)_{g(i)} = a_i`,
//! `s = prod_{i flipped} (-1)^{a_i}`; an element that sends an ISP to a combination of slots acts only on keys with
//! zero power there (the partial action of `orbit.rs`).
//!
//! Generation on a fundamental domain (design: notes/sym-gen.md). Within one chart every node order (legacy
//! SpIRed order with its tie permutation, or a compiled order program) is a lexicographic comparison of linear forms
//! of the powers ([`ChartForms`], cross-checked against the engine's [`IntegralOrder::compare`] on random keys and
//! on group images before use). For a queued case `C` and an element `g` acting exactly on every key of `C`, the
//! differences `w.a - w.(g a)` are affine in the free coordinates; when the first nonzero one is a CONSTANT of the
//! right sign, `g a` is strictly below `a` for every key of `C`, and the case is closed by ONE symmetry rule
//! `I(a) = s(g, a) I(g a)`: a strict descent step under the node's own order (no exceptions: unit coefficient). The
//! queue never searches that case. Termination and coverage are those of the ordinary table: every step strictly
//! descends in the one declared order, and the union of the case regions still covers the chart.

use std::cmp::Ordering;

use crate::propagator::PresentationId;
use crate::propagator::orbit::{SlotForms, owner_orbit};
use crate::propagator::presentation::{Presentation, PresentationKind, Slot, SlotIsp, SlotLine};
use crate::solver::{Integral, IntegralOrder};

/// One group element in PRESENTED slots: the momentum map (row-vector convention `v -> v T` over
/// `(k_1, .., k_L, q)`) and its slot action (`None`: the slot goes to a combination of slots).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymElement {
    pub matrix: Vec<i64>,
    pub action: Vec<Option<(usize, bool)>>,
}

impl SymElement {
    /// Every slot goes onto one slot.
    pub fn total(&self) -> bool {
        self.action.iter().all(Option::is_some)
    }

    pub fn flips(&self) -> bool {
        self.action.iter().any(|a| matches!(a, Some((_, true))))
    }

    /// The exact image of `key` with its sign (`true` = minus), when the action is exact on it.
    pub fn image(&self, key: &[i64]) -> Option<(Vec<i64>, bool)> {
        let mut out = vec![0i64; key.len()];
        let mut negative = false;
        for (slot, &power) in key.iter().enumerate() {
            if power == 0 {
                continue;
            }
            let (target, flip) = (*self.action.get(slot)?)?;
            out[target] = power;
            if flip && power % 2 != 0 {
                negative = !negative;
            }
        }
        Some((out, negative))
    }

    /// The action as JSON `[[target, flip] | null, ...]`.
    pub fn action_json(&self) -> String {
        let parts: Vec<String> = self
            .action
            .iter()
            .map(|a| match a {
                Some((t, f)) => format!("[{t},{f}]"),
                None => "null".to_string(),
            })
            .collect();
        format!("[{}]", parts.join(","))
    }
}

/// The authenticated automorphism group of one node chart.
#[derive(Clone, Debug, Default)]
pub struct NodeGroup {
    pub loops: usize,
    pub arity: usize,
    pub chart: Vec<bool>,
    /// Candidate maps of the enumeration (identity included) / generators proved by `verify`.
    pub candidates: usize,
    pub verified: usize,
    /// Order of the matrix group (`T` and `-T` both counted).
    pub group_order: usize,
    /// Distinct non-identity slot actions (`T` and `-T` act alike on keys).
    pub elements: Vec<SymElement>,
}

impl NodeGroup {
    /// Order of the group acting on keys (identity included).
    pub fn key_order(&self) -> usize {
        self.elements.len() + 1
    }

    /// Elements acting on EVERY key of the chart (signed slot permutations).
    pub fn total_elements(&self) -> usize {
        self.elements.iter().filter(|e| e.total()).count()
    }

    pub fn to_json(&self) -> String {
        let els: Vec<String> = self
            .elements
            .iter()
            .enumerate()
            .map(|(k, e)| {
                format!(
                    "{{\"element\":{k},\"matrix\":{:?},\"action\":{},\"total\":{}}}",
                    e.matrix,
                    e.action_json(),
                    e.total()
                )
            })
            .collect();
        format!(
            "{{\"loops\":{},\"arity\":{},\"chart\":\"{}\",\"candidates\":{},\"verified_generators\":{},\"group_order\":{},\"key_order\":{},\"total_elements\":{},\"flips\":{},\"elements\":[{}]}}",
            self.loops,
            self.arity,
            self.chart
                .iter()
                .map(|&b| if b { '1' } else { '0' })
                .collect::<String>(),
            self.candidates,
            self.verified,
            self.group_order,
            self.key_order(),
            self.total_elements(),
            self.elements.iter().any(SymElement::flips),
            els.join(",")
        )
    }
}

/// Text inside the first balanced `name(` .. `)`.
fn call_args(src: &str, name: &str) -> Option<String> {
    let start = src.find(&format!("{name}("))? + name.len() + 1;
    let mut depth = 1;
    for (k, ch) in src[start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(src[start..start + k].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// `(loops, [(momentum over loops then q, physical)])` of a compact family `I(..., prop(Dk,(mom)^2,flag), ...)`.
#[allow(clippy::type_complexity)]
pub fn parse_props(text: &str) -> Result<(usize, Vec<(Vec<i64>, bool)>), String> {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let loops: Vec<String> = call_args(&compact, "loops")
        .ok_or("family without loops(..)")?
        .split(',')
        .map(str::to_string)
        .collect();
    let externals: Vec<String> = call_args(&compact, "externals")
        .ok_or("family without externals(..)")?
        .split(',')
        .map(str::to_string)
        .collect();
    if externals.len() != 1 {
        return Err("symmetry: exactly one external momentum is supported".into());
    }
    let vars: Vec<String> = loops.iter().cloned().chain(externals).collect();
    let mut out = Vec::new();
    let mut rest = compact.as_str();
    while let Some(at) = rest.find("prop(") {
        let body = call_args(&rest[at..], "prop").ok_or("unbalanced prop(")?;
        rest = &rest[at + 5..];
        // D<k>,(mom)^2,flag
        let (_, tail) = body.split_once(',').ok_or("prop without momentum")?;
        let (sq, flag) = tail.rsplit_once(',').ok_or("prop without flag")?;
        let mom = sq
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(")^2"))
            .ok_or_else(|| format!("symmetry: only squared propagators are supported ({sq})"))?;
        let physical = match flag {
            "1" => true,
            "0" => false,
            other => return Err(format!("prop flag {other}")),
        };
        out.push((linear(mom, &vars)?, physical));
    }
    Ok((loops.len(), out))
}

fn linear(mom: &str, vars: &[String]) -> Result<Vec<i64>, String> {
    let s: Vec<char> = mom.chars().collect();
    let mut v = vec![0i64; vars.len()];
    let mut i = 0;
    while i < s.len() {
        let mut sign = 1;
        if s[i] == '+' || s[i] == '-' {
            if s[i] == '-' {
                sign = -1;
            }
            i += 1;
        }
        let start = i;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        let coef: i64 = if i > start {
            s[start..i]
                .iter()
                .collect::<String>()
                .parse()
                .map_err(|_| "momentum coefficient")?
        } else {
            1
        };
        if i < s.len() && s[i] == '*' {
            i += 1;
        }
        let vstart = i;
        while i < s.len() && (s[i].is_ascii_alphanumeric() || s[i] == '_') {
            i += 1;
        }
        let name: String = s[vstart..i].iter().collect();
        let pos = vars
            .iter()
            .position(|x| *x == name)
            .ok_or_else(|| format!("momentum {mom}: unknown symbol {name}"))?;
        v[pos] += sign * coef;
    }
    Ok(v)
}

/// The authenticated automorphism group of chart `chart` (presented slots, `true` = line `n >= 1`) of the compact
/// family `text`, by the application-time orbit search (`owner_orbit`, generators proved by `verify`).
pub fn node_group(text: &str, chart: &[bool]) -> Result<NodeGroup, String> {
    let (loops, props) = parse_props(text)?;
    if props.len() != chart.len() {
        return Err(format!(
            "chart arity {} != {} propagators",
            chart.len(),
            props.len()
        ));
    }
    let family = super::driver::compile_family(text)?;
    let context = family.coefficient_context().clone();
    // presentation slots: chart lines first (Slot::Line), then every other slot as an ISP `p.p`
    let order: Vec<usize> = (0..props.len())
        .filter(|&i| chart[i])
        .chain((0..props.len()).filter(|&i| !chart[i]))
        .collect();
    let slots: Vec<Slot> = order
        .iter()
        .map(|&i| {
            let m = props[i].0.clone();
            if chart[i] {
                Slot::Line(SlotLine {
                    momentum: m,
                    eps_units: 0,
                })
            } else {
                Slot::Isp(SlotIsp {
                    left: m.clone(),
                    right: m,
                })
            }
        })
        .collect();
    let presentation = Presentation::from_slots(
        PresentationId(0),
        PresentationKind::Caller,
        "symgen",
        u8::try_from(loops).map_err(|_| "loops")?,
        slots,
        &context,
        None,
    )
    .map_err(|e| format!("symmetry presentation: {e}"))?;
    let lines = chart.iter().filter(|&&b| b).count();
    let support: u64 = if lines >= 64 {
        u64::MAX
    } else {
        (1u64 << lines) - 1
    };
    let orbit = owner_orbit(&presentation, support).map_err(|e| format!("orbit: {e}"))?;
    let n = loops + 1;
    let forms = SlotForms::new(&presentation, n);
    let mut group = NodeGroup {
        loops,
        arity: props.len(),
        chart: chart.to_vec(),
        candidates: orbit.candidates,
        verified: orbit.verified,
        group_order: orbit.group_order,
        elements: Vec::new(),
    };
    let mut seen = std::collections::BTreeSet::new();
    for t in &orbit.group {
        let action = forms.action(t);
        // back to presented slots
        let mut presented: Vec<Option<(usize, bool)>> = vec![None; props.len()];
        for (pos, image) in action.iter().enumerate() {
            presented[order[pos]] = image.map(|(target, flip)| (order[usize::from(target)], flip));
        }
        if presented
            .iter()
            .enumerate()
            .all(|(i, a)| *a == Some((i, false)))
        {
            continue;
        }
        // the chart lines must go onto chart lines (owner_orbit keeps only such maps; checked again here)
        if (0..props.len()).any(|i| chart[i] && !presented[i].is_some_and(|(t, _)| chart[t])) {
            continue;
        }
        if seen.insert(presented.clone()) {
            group.elements.push(SymElement {
                matrix: t
                    .iter()
                    .map(|&x| i64::try_from(x).unwrap_or(i64::MAX))
                    .collect(),
                action: presented,
            });
        }
    }
    Ok(group)
}

/// A node order restricted to one chart: lexicographic over linear forms of the powers (`(weights, reversed)`:
/// component `(w.a).cmp(&(w.b))`, reversed when the flag is set), equal to [`IntegralOrder::compare`] on chart keys
/// (`Less` = left harder).
#[derive(Clone, Debug)]
pub struct ChartForms {
    pub forms: Vec<(Vec<i64>, bool)>,
}

impl ChartForms {
    /// The forms of `order` on `chart` (lines `true`); `None` for orders this module does not model (cuts,
    /// two-sided axes).
    pub fn from_order<const N: usize>(order: &IntegralOrder<N>, chart: &[bool]) -> Option<Self> {
        let n = chart.len();
        if n > N {
            return None;
        }
        let unit = |axis: usize, sign: i64| -> Vec<i64> {
            let mut w = vec![0i64; n];
            w[axis] = sign;
            w
        };
        let side = |axis: usize| if chart[axis] { 1i64 } else { -1 };
        let mut forms: Vec<(Vec<i64>, bool)> = Vec::new();
        if let Some(program) = order.program() {
            if (0..n).any(|a| program.is_two_sided(a)) || program.arity() != n {
                return None;
            }
            let d = program.descriptor();
            let row = |r: &rustred_order::DegreeRow| -> Option<Vec<i64>> {
                (0..n)
                    .map(|a| {
                        let w = if chart[a] { r.active[a] } else { r.inactive[a] };
                        i64::try_from(w).ok().map(|w| w * side(a))
                    })
                    .collect()
            };
            for r in &d.pre_support_degree_rows {
                forms.push((row(r)?, true));
            }
            for r in &d.degree_rows {
                forms.push((row(r)?, true));
            }
            use rustred_order::{CoordinateGroups, Direction};
            let passes: Vec<Option<bool>> = match d.coordinate_groups {
                CoordinateGroups::Interleaved => vec![None],
                CoordinateGroups::ActiveFirst => vec![Some(true), Some(false)],
                CoordinateGroups::InactiveFirst => vec![Some(false), Some(true)],
            };
            for pass in passes {
                for &axis in &d.coordinate_priority {
                    if pass.is_some_and(|p| p != chart[axis]) {
                        continue;
                    }
                    let direction = if chart[axis] {
                        d.active_direction
                    } else {
                        d.inactive_direction
                    };
                    forms.push((unit(axis, side(axis)), direction != Direction::Descending));
                }
            }
        } else {
            if order.deltas().iter().any(|&d| d) {
                return None;
            }
            let perm: Vec<usize> = match order.permutation() {
                Some(p) => p.iter().copied().filter(|&i| i < n).collect(),
                None => (0..n).collect(),
            };
            forms.push(((0..n).map(|a| -side(a)).collect(), false));
            forms.push(((0..n).map(|a| i64::from(!chart[a])).collect(), false));
            for &p in &perm {
                if chart[p] {
                    forms.push((unit(p, 1), false));
                }
            }
            for &p in &perm {
                if !chart[p] {
                    forms.push((unit(p, -1), false));
                }
            }
        }
        Some(Self { forms })
    }

    pub fn compare(&self, a: &[i64], b: &[i64]) -> Ordering {
        for (w, reversed) in &self.forms {
            let l: i128 = w.iter().zip(a).map(|(&x, &y)| i128::from(x * y)).sum();
            let r: i128 = w.iter().zip(b).map(|(&x, &y)| i128::from(x * y)).sum();
            let c = if *reversed { r.cmp(&l) } else { l.cmp(&r) };
            if c != Ordering::Equal {
                return c;
            }
        }
        Ordering::Equal
    }

    /// Cross-check against the engine's order on `samples` random chart keys and their group images (deterministic
    /// xorshift): the number of disagreements (0 = the forms ARE the order on these keys).
    pub fn mismatches<const N: usize>(
        &self,
        order: &IntegralOrder<N>,
        chart: &[bool],
        group: &NodeGroup,
        samples: usize,
    ) -> usize {
        let n = chart.len();
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = |m: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % m
        };
        let key = |next: &mut dyn FnMut(u64) -> u64| -> Vec<i64> {
            (0..n)
                .map(|a| {
                    if chart[a] {
                        1 + next(3) as i64
                    } else if next(2) == 0 {
                        0
                    } else {
                        -(next(3) as i64)
                    }
                })
                .collect()
        };
        let integral = |k: &[i64]| -> Option<Integral<N>> {
            let mut v = [0i16; N];
            for (slot, &x) in k.iter().enumerate() {
                v[slot] = i16::try_from(x).ok()?;
            }
            Integral::numeric(v).ok()
        };
        let mut bad = 0usize;
        for s in 0..samples {
            let a = key(&mut next);
            let b = if !group.elements.is_empty() && s % 2 == 0 {
                let e = &group.elements[next(group.elements.len() as u64) as usize];
                match e.image(&a) {
                    Some((img, _)) => img,
                    None => key(&mut next),
                }
            } else {
                key(&mut next)
            };
            let (Some(ia), Some(ib)) = (integral(&a), integral(&b)) else {
                continue;
            };
            if order.compare(&ia, &ib) != self.compare(&a, &b) {
                bad += 1;
            }
        }
        bad
    }
}

/// What the group says about a whole case (fixed coordinates; free = `None`; equations are ignored, which only
/// enlarges the region the verdict is proved for).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaseVerdict {
    /// `g a` is strictly below `a` in the node order for every key `a` of the case: `I(a) = s I(g a)`.
    Lower {
        element: usize,
    },
    /// `g a = a` and the sign is `-1` for every key of the case: every integral of the case is zero.
    Zero {
        element: usize,
    },
    None,
}

/// The verdict of the group on a case (first element in group order that decides).
#[allow(clippy::needless_range_loop)]
pub fn case_verdict(group: &NodeGroup, forms: &ChartForms, fixed: &[Option<i64>]) -> CaseVerdict {
    let n = fixed.len();
    'elements: for (k, e) in group.elements.iter().enumerate() {
        // exact on every key of the case: a slot that goes to a combination must be fixed at 0
        if (0..n).any(|i| e.action[i].is_none() && fixed[i] != Some(0)) {
            continue;
        }
        let target = |i: usize| e.action[i].map_or(i, |(t, _)| t);
        for (w, reversed) in &forms.forms {
            // D(a) = w.a - w.(g a) = sum_i (w_i - w_{g(i)}) a_i
            let mut constant = 0i128;
            for i in 0..n {
                let c = w[i] - w[target(i)];
                if c == 0 {
                    continue;
                }
                match fixed[i] {
                    Some(v) => constant += i128::from(c) * i128::from(v),
                    None => continue 'elements, // depends on a free coordinate: undecided for this element
                }
            }
            let c = if *reversed {
                0.cmp(&constant)
            } else {
                constant.cmp(&0)
            };
            match c {
                Ordering::Equal => {}
                Ordering::Less => return CaseVerdict::Lower { element: k },
                Ordering::Greater => continue 'elements,
            }
        }
        // every form ties: a fixed point of the case when g fixes the coordinates pointwise
        let pointwise = (0..n).all(|i| {
            let t = target(i);
            t == i || (fixed[i].is_some() && fixed[i] == fixed[t])
        });
        if !pointwise {
            continue;
        }
        let mut odd = false;
        for i in 0..n {
            if let Some((_, true)) = e.action[i] {
                match fixed[i] {
                    Some(v) => odd ^= v % 2 != 0,
                    None => continue 'elements, // sign depends on the key
                }
            }
        }
        if odd {
            return CaseVerdict::Zero { element: k };
        }
    }
    CaseVerdict::None
}

/// The orbit minimum of a numeric chart key (in the node order) with the element and sign that reach it; `None` when
/// the key is its own representative.
pub fn least_image(
    group: &NodeGroup,
    forms: &ChartForms,
    key: &[i64],
) -> Option<(usize, Vec<i64>, bool)> {
    let mut best: Option<(usize, Vec<i64>, bool)> = None;
    for (k, e) in group.elements.iter().enumerate() {
        let Some((img, neg)) = e.image(key) else {
            continue;
        };
        let bound = best.as_ref().map_or(key, |b| b.1.as_slice());
        // Greater = the image is easier (lower) than the bound
        if forms.compare(bound, &img) == Ordering::Less {
            best = Some((k, img, neg));
        }
    }
    best
}

/// A key pattern of a case under an element: `I(..)` with `n<i>` at the image position of each free slot.
#[allow(clippy::needless_range_loop)]
pub fn image_pattern(e: &SymElement, fixed: &[Option<i64>]) -> String {
    let n = fixed.len();
    let mut out = vec![String::new(); n];
    for i in 0..n {
        let t = e.action[i].map_or(i, |(t, _)| t);
        out[t] = match fixed[i] {
            Some(v) => v.to_string(),
            None => format!("n{i}"),
        };
    }
    for (t, s) in out.iter_mut().enumerate() {
        if s.is_empty() {
            *s = fixed[t].map_or(format!("n{t}"), |v| v.to_string());
        }
    }
    format!("I({})", out.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_images_carry_signs_and_partial_actions() {
        let e = SymElement {
            matrix: vec![],
            action: vec![Some((1, false)), Some((0, false)), Some((2, true)), None],
        };
        assert_eq!(e.image(&[1, 2, -1, 0]), Some((vec![2, 1, -1, 0], true)));
        assert_eq!(e.image(&[1, 2, -2, 0]), Some((vec![2, 1, -2, 0], false)));
        assert_eq!(e.image(&[1, 2, 0, -1]), None);
        assert!(!e.total());
    }

    #[test]
    fn verdicts_decide_only_from_fixed_coordinates() {
        // legacy forms of a 2-line chart, identity tie order: abs, numerator, then a0, a1 (smaller first = harder)
        let forms = ChartForms {
            forms: vec![
                (vec![-1, -1], false),
                (vec![0, 0], false),
                (vec![1, 0], false),
                (vec![0, 1], false),
            ],
        };
        let group = NodeGroup {
            elements: vec![SymElement {
                matrix: vec![],
                action: vec![Some((1, false)), Some((0, false))],
            }],
            ..Default::default()
        };
        // (1, 3) vs swap (3, 1): a0 = 1 < 3: (1, 3) is harder, its image is lower
        assert_eq!(
            case_verdict(&group, &forms, &[Some(1), Some(3)]),
            CaseVerdict::Lower { element: 0 }
        );
        assert_eq!(
            case_verdict(&group, &forms, &[Some(3), Some(1)]),
            CaseVerdict::None
        );
        // free coordinate: undecided
        assert_eq!(
            case_verdict(&group, &forms, &[None, Some(1)]),
            CaseVerdict::None
        );
        assert_eq!(
            least_image(&group, &forms, &[1, 3]),
            Some((0, vec![3, 1], false))
        );
        assert_eq!(least_image(&group, &forms, &[3, 1]), None);
    }

    #[test]
    fn a_flipped_fixed_point_is_zero() {
        let forms = ChartForms {
            forms: vec![(vec![1, 0], false), (vec![0, -1], false)],
        };
        let group = NodeGroup {
            elements: vec![SymElement {
                matrix: vec![],
                action: vec![Some((0, false)), Some((1, true))],
            }],
            ..Default::default()
        };
        assert_eq!(
            case_verdict(&group, &forms, &[None, Some(-1)]),
            CaseVerdict::Zero { element: 0 }
        );
        assert_eq!(
            case_verdict(&group, &forms, &[None, Some(-2)]),
            CaseVerdict::None
        );
        assert_eq!(
            case_verdict(&group, &forms, &[None, None]),
            CaseVerdict::None
        );
    }

    #[test]
    fn sunset_like_family_has_its_line_permutations() {
        let text = "I(name(S),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k1+k2+q)^2,1),prop(D4,(k1+q)^2,0),prop(D5,(k2+q)^2,0),gram(q,q,1))";
        let g = node_group(text, &[true, true, true, false, false]).unwrap();
        assert!(g.key_order() >= 2, "{}", g.to_json());
        assert!(
            g.elements
                .iter()
                .all(|e| (0..3).all(|i| e.action[i].is_some_and(|(t, _)| t < 3)))
        );
    }
}
