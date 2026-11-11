//! Node order: ONE source/slot order per node, recorded in the artefact.
//!
//! The 5L campaign showed (Astra T211/T212/T206, repair agent, 2026-11-11) that a hanging corner lift is a
//! pivot/order choice: reversing the ISP slots (T211), reversing the ISP tie priority (T212) or swapping two
//! physical slots that carry the corner's indices (T206) turns a lift that never finishes into a depth-1 rule.
//! A [`NodeOrder`] makes that choice first class:
//!
//! * `slot_permutation` -- a family *presentation*: new slot `j` carries the propagator expression and the
//!   physical flag of the old slot `perm[j]` (labels stay positional). The integral order of the engine is
//!   positional, so a presentation change changes the order, the chosen pivot and the relation.
//! * `coordinate_priority` -- a comparator-only change: the final lexicographic tie priority of the
//!   (presented) slots, compiled as `OrderDescriptor::spired_uncut(n)` with that priority (Astra T212
//!   `isp_reverse`). `None` keeps the engine's legacy order.
//! * `source_order` -- the source-row visit order of every case (`input`, `reverse`, `rot:K`). It does not
//!   change the integral order (rules found under different source orders share one descent order).
//!
//! A table generated under a node order records it in its header; every rule of the node, the numeric
//! endpoint cases included, is generated under it, and the assembled table is checked for strict descent under
//! it ([`super::descent`]). Rules found under different orders are never mixed.

use serde::{Deserialize, Serialize};

/// Source-row visit order of every case search.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceOrder {
    Input,
    Reverse,
    Rotate(usize),
}

impl SourceOrder {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "input" => Ok(Self::Input),
            "reverse" => Ok(Self::Reverse),
            s if s.starts_with("rot:") => s[4..].parse().map(Self::Rotate).map_err(|e| format!("source order {s}: {e}")),
            other => Err(format!("unknown source order {other} (input|reverse|rot:K)")),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Input => "input".into(),
            Self::Reverse => "reverse".into(),
            Self::Rotate(k) => format!("rot:{k}"),
        }
    }

    /// The visit permutation of `0..basis`; `None` = the stored order.
    pub fn visit(&self, basis: usize) -> Option<Vec<usize>> {
        match self {
            Self::Input => None,
            Self::Reverse => Some((0..basis).rev().collect()),
            Self::Rotate(k) => Some((0..basis).map(|i| (i + k) % basis.max(1)).collect()),
        }
    }
}

/// The one order under which a node is generated. See the module documentation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeOrder {
    /// Short name (variant label), e.g. `id`, `isp_rev_free`, `phys_swap_0_8`, `tie_isp_rev`.
    pub name: String,
    /// New slot `j` carries old slot `perm[j]`; `None` = the family as given.
    pub slot_permutation: Option<Vec<usize>>,
    /// Final tie priority of the presented slots (compiled descriptor); `None` = legacy order.
    pub coordinate_priority: Option<Vec<usize>>,
    pub source_order: SourceOrder,
}

impl Default for NodeOrder {
    fn default() -> Self {
        Self::identity()
    }
}

impl NodeOrder {
    pub fn identity() -> Self {
        Self { name: "id".into(), slot_permutation: None, coordinate_priority: None, source_order: SourceOrder::Input }
    }

    pub fn is_identity_presentation(&self) -> bool {
        self.slot_permutation.as_ref().is_none_or(|p| p.iter().enumerate().all(|(j, &i)| i == j))
    }

    /// Validate against the arity `n`.
    pub fn validate(&self, n: usize) -> Result<(), String> {
        for (what, v) in [("slot_permutation", &self.slot_permutation), ("coordinate_priority", &self.coordinate_priority)] {
            if let Some(p) = v {
                let mut seen = vec![false; n];
                if p.len() != n {
                    return Err(format!("{what}: length {} != arity {n}", p.len()));
                }
                for &i in p {
                    if i >= n || std::mem::replace(&mut seen[i], true) {
                        return Err(format!("{what}: not a permutation of 0..{n}"));
                    }
                }
            }
        }
        Ok(())
    }

    /// old slot -> new slot.
    pub fn old_to_new(&self, n: usize) -> Vec<usize> {
        let mut inv: Vec<usize> = (0..n).collect();
        if let Some(p) = &self.slot_permutation {
            for (j, &i) in p.iter().enumerate() {
                inv[i] = j;
            }
        }
        inv
    }

    /// new slot -> old slot.
    pub fn new_to_old(&self, n: usize) -> Vec<usize> {
        self.slot_permutation.clone().unwrap_or_else(|| (0..n).collect())
    }

    /// The family presented in this order (labels positional, expression and flag move together).
    pub fn present_family(&self, text: &str) -> Result<String, String> {
        match &self.slot_permutation {
            None => Ok(text.to_string()),
            Some(p) => permute_family_text(text, p),
        }
    }

    /// Map a case given in the ORIGINAL slots (fixed values, linear equations in `n<i>`) to the presented slots.
    pub fn present_case(&self, fixed: &[Option<i64>], equations: &[String]) -> (Vec<Option<i64>>, Vec<String>) {
        let n = fixed.len();
        let new_to_old = self.new_to_old(n);
        let old_to_new = self.old_to_new(n);
        let fixed = (0..n).map(|j| fixed[new_to_old[j]]).collect();
        let equations = equations.iter().map(|e| rename_index_variables(e, &old_to_new)).collect();
        (fixed, equations)
    }

    /// Map a case given in the PRESENTED slots back to the original slots.
    pub fn original_case(&self, fixed: &[Option<i64>], equations: &[String]) -> (Vec<Option<i64>>, Vec<String>) {
        let n = fixed.len();
        let new_to_old = self.new_to_old(n);
        let mut out = vec![None; n];
        for (j, v) in fixed.iter().enumerate() {
            out[new_to_old[j]] = *v;
        }
        let equations = equations.iter().map(|e| rename_index_variables(e, &new_to_old)).collect();
        (out, equations)
    }

    /// One-line JSON (the artefact record of the order).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("node order json")
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("node order json: {e}"))
    }
}

/// `n<i>` -> `n<map[i]>` in an index expression (word-bounded, no other symbol touched).
pub fn rename_index_variables(expr: &str, map: &[usize]) -> String {
    let b = expr.as_bytes();
    let mut out = String::with_capacity(expr.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        let prev_word = i > 0 && ((b[i - 1] as char).is_ascii_alphanumeric() || b[i - 1] == b'_');
        if c == 'n' && !prev_word && i + 1 < b.len() && b[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let next_word = j < b.len() && ((b[j] as char).is_ascii_alphanumeric() || b[j] == b'_');
            if !next_word {
                let k: usize = expr[i + 1..j].parse().expect("digits");
                out.push('n');
                out.push_str(&map.get(k).copied().unwrap_or(k).to_string());
                i = j;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// One `prop(LABEL,EXPR,FLAG)` line of a compact family file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropLine {
    pub line: usize,
    pub indent: String,
    pub label: String,
    pub expr: String,
    pub flag: String,
    pub trailing_comma: bool,
}

/// The propagator lines of a compact family (`prop(D1,(k1)^2,1),`), in slot order.
pub fn family_props(text: &str) -> Vec<PropLine> {
    let mut out = Vec::new();
    for (k, raw) in text.lines().enumerate() {
        let t = raw.trim_end();
        let body = t.trim_start();
        if !body.starts_with("prop(") {
            continue;
        }
        let indent = t[..t.len() - body.len()].to_string();
        let trailing_comma = body.ends_with(',');
        let inner = body.trim_end_matches(',');
        let Some(inner) = inner.strip_prefix("prop(").and_then(|s| s.strip_suffix(')')) else { continue };
        let Some(first) = inner.find(',') else { continue };
        let Some(last) = inner.rfind(',') else { continue };
        if last <= first {
            continue;
        }
        out.push(PropLine {
            line: k,
            indent,
            label: inner[..first].to_string(),
            expr: inner[first + 1..last].to_string(),
            flag: inner[last + 1..].trim().to_string(),
            trailing_comma,
        });
    }
    out
}

/// New slot `j` carries the expression and flag of old slot `perm[j]`; labels stay positional.
pub fn permute_family_text(text: &str, perm: &[usize]) -> Result<String, String> {
    let props = family_props(text);
    if props.len() != perm.len() {
        return Err(format!("permutation arity {} != {} propagators", perm.len(), props.len()));
    }
    let order = NodeOrder { slot_permutation: Some(perm.to_vec()), ..NodeOrder::identity() };
    order.validate(props.len())?;
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    for (j, p) in props.iter().enumerate() {
        let src = &props[perm[j]];
        lines[p.line] = format!("{}prop({},{},{}){}", p.indent, p.label, src.expr, src.flag, if p.trailing_comma { "," } else { "" });
    }
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// Physical flags (`flag != 0`) of the family slots.
pub fn physical_slots(text: &str) -> Vec<bool> {
    family_props(text).iter().map(|p| p.flag.trim() != "0").collect()
}

/// Indices `i` with `n<i>` in a linear equation.
pub fn equation_variables(expr: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let b = expr.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'n' && (i == 0 || !(b[i - 1] as char).is_ascii_alphanumeric()) {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 {
                out.push(expr[i + 1..j].parse().unwrap());
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Cheap order variants for a (hanging) case given in the ORIGINAL slots: the probe candidates.
///
/// Presentation variants (p5l-repair `p5l_orders.py` naming): `id`, `isp_rev_all`, `isp_rev_free` (the ISP slots
/// that are free in the case, reversed among themselves; Astra T211), `isp_last_K`/`isp_first_K` (one free ISP to
/// the lowest/highest priority), `phys_swap_A_B` (two physical slots carrying the case's free, dotted or equation
/// indices; Astra T206), `phys_rev`. Comparator variants: `tie_isp_rev` (ISP tie priority reversed, presentation
/// unchanged; Astra T212). Source variant: `src_rev` (reverse source visit; same integral order as `id`).
pub fn variants(family_text: &str, fixed: &[Option<i64>], equations: &[String], max_phys_pairs: usize) -> Vec<NodeOrder> {
    let phys_flags = physical_slots(family_text);
    let n = phys_flags.len();
    let phys: Vec<usize> = (0..n).filter(|&i| phys_flags[i]).collect();
    let isp: Vec<usize> = (0..n).filter(|&i| !phys_flags[i]).collect();
    let eqvars: Vec<usize> = equations.iter().flat_map(|e| equation_variables(e)).collect();
    let free_isp: Vec<usize> = isp.iter().copied().filter(|&i| fixed.get(i).copied().flatten().is_none()).collect();
    let carry: Vec<usize> = phys
        .iter()
        .copied()
        .filter(|&i| {
            let f = fixed.get(i).copied().flatten();
            f.is_none() || !matches!(f, Some(0) | Some(1)) || eqvars.contains(&i)
        })
        .collect();
    let with_slots = |slots: &[usize], new_order: &[usize]| -> Vec<usize> {
        let mut p: Vec<usize> = (0..n).collect();
        for (s, o) in slots.iter().zip(new_order) {
            p[*s] = *o;
        }
        p
    };
    let pres = |name: String, p: Vec<usize>| NodeOrder { name, slot_permutation: Some(p), ..NodeOrder::identity() };
    let mut out = vec![NodeOrder::identity()];
    if isp.len() > 1 {
        let rev: Vec<usize> = isp.iter().rev().copied().collect();
        out.push(pres("isp_rev_all".into(), with_slots(&isp, &rev)));
        let priority: Vec<usize> = phys.iter().copied().chain(isp.iter().rev().copied()).collect();
        out.push(NodeOrder { name: "tie_isp_rev".into(), coordinate_priority: Some(priority), ..NodeOrder::identity() });
    }
    if free_isp.len() > 1 {
        let rev: Vec<usize> = free_isp.iter().rev().copied().collect();
        out.push(pres("isp_rev_free".into(), with_slots(&free_isp, &rev)));
    }
    for &k in &free_isp {
        let rest: Vec<usize> = isp.iter().copied().filter(|&i| i != k).collect();
        let last: Vec<usize> = rest.iter().copied().chain([k]).collect();
        let first: Vec<usize> = [k].into_iter().chain(rest.iter().copied()).collect();
        out.push(pres(format!("isp_last_{k}"), with_slots(&isp, &last)));
        out.push(pres(format!("isp_first_{k}"), with_slots(&isp, &first)));
    }
    let mut pairs = 0;
    'pairs: for (x, &a) in carry.iter().enumerate() {
        for &b in &carry[x + 1..] {
            if pairs >= max_phys_pairs {
                break 'pairs;
            }
            let mut p: Vec<usize> = (0..n).collect();
            p.swap(a, b);
            out.push(pres(format!("phys_swap_{a}_{b}"), p));
            pairs += 1;
        }
    }
    if phys.len() > 1 {
        let rev: Vec<usize> = phys.iter().rev().copied().collect();
        out.push(pres("phys_rev".into(), with_slots(&phys, &rev)));
    }
    out.push(NodeOrder { name: "src_rev".into(), source_order: SourceOrder::Reverse, ..NodeOrder::identity() });
    // dedup identical (presentation, priority, source) triples, keeping the first name
    let mut seen: Vec<(Vec<usize>, Option<Vec<usize>>, SourceOrder)> = Vec::new();
    out.retain(|o| {
        let key = (o.new_to_old(n), o.coordinate_priority.clone(), o.source_order.clone());
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAM: &str = "I(\n  name(T),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1-k2)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";

    #[test]
    fn permute_family_moves_expression_and_flag_keeps_labels() {
        let out = permute_family_text(FAM, &[2, 1, 0, 4, 3]).unwrap();
        let props = family_props(&out);
        assert_eq!(props[0].label, "D1");
        assert_eq!(props[0].expr, "(k1-k2)^2");
        assert_eq!(props[2].expr, "(k1)^2");
        assert_eq!(props[3].expr, "(k2+q)^2");
        assert_eq!(props[3].flag, "0");
        assert!(out.ends_with(")\n"));
        // identity is the identity
        assert_eq!(permute_family_text(FAM, &[0, 1, 2, 3, 4]).unwrap(), FAM);
        assert!(permute_family_text(FAM, &[0, 0, 2, 3, 4]).is_err());
    }

    #[test]
    fn case_mapping_roundtrip_and_equation_renaming() {
        let o = NodeOrder { slot_permutation: Some(vec![2, 1, 0, 4, 3]), ..NodeOrder::identity() };
        let fixed = vec![None, Some(2), Some(1), Some(0), None];
        let eqs = vec!["n0+n2-n4".to_string()];
        let (f2, e2) = o.present_case(&fixed, &eqs);
        assert_eq!(f2, vec![Some(1), Some(2), None, None, Some(0)]);
        assert_eq!(e2, vec!["n2+n0-n3".to_string()]);
        let (f3, e3) = o.original_case(&f2, &e2);
        assert_eq!(f3, fixed);
        assert_eq!(e3, eqs);
        assert_eq!(rename_index_variables("n10+n1-dn1", &(0..11).rev().collect::<Vec<_>>()), "n0+n9-dn1");
    }

    #[test]
    fn variants_cover_the_campaign_rungs_and_dedup() {
        // T206-like case: physical slot 0 and 2 free, slot 1 dotted, ISPs fixed
        let fixed = vec![None, Some(2), None, Some(0), Some(0)];
        let names: Vec<String> = variants(FAM, &fixed, &["n0+n1-n2".into()], 10).into_iter().map(|o| o.name).collect();
        assert!(names.contains(&"id".to_string()));
        assert!(names.contains(&"phys_swap_0_2".to_string()));
        assert!(names.contains(&"phys_swap_1_2".to_string()));
        assert!(names.contains(&"tie_isp_rev".to_string()));
        assert!(names.contains(&"src_rev".to_string()));
        // isp_rev_all of a 2-ISP family equals isp_rev_free when both are free -> dedup keeps one
        let fixed2 = vec![Some(1), Some(1), Some(1), None, None];
        let v2: Vec<String> = variants(FAM, &fixed2, &[], 10).into_iter().map(|o| o.name).collect();
        assert!(v2.contains(&"isp_rev_all".to_string()));
        assert!(!v2.contains(&"isp_rev_free".to_string()));
        for o in variants(FAM, &fixed2, &[], 10) {
            o.validate(5).unwrap();
            assert_eq!(NodeOrder::from_json(&o.to_json()).unwrap(), o);
        }
    }

    #[test]
    fn source_order_visits() {
        assert_eq!(SourceOrder::parse("reverse").unwrap().visit(3), Some(vec![2, 1, 0]));
        assert_eq!(SourceOrder::parse("rot:1").unwrap().visit(3), Some(vec![1, 2, 0]));
        assert_eq!(SourceOrder::parse("input").unwrap().visit(3), None);
        assert!(SourceOrder::parse("x").is_err());
    }
}
