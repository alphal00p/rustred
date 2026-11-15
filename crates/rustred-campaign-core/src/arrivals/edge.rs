//! The ROUTE of a DAG edge parent -> child, read from an edge file (definition D3 of WS/notes/implementer.arrivals.md).
//!
//! An edge file (schema `handdown-routes.edge.v1`, written by the engine's census route probe: `census::route_one` with
//! the library's static choice) lists the parent's one- or multi-line-lower sectors; a ROUTED sector (fate starting
//! with `routed`) lands on the child's class and carries, per parent TABLE slot t, either `{"line": j}` (the surviving
//! parent line t is child table line j, power carried over) or `{"form": {j: "p/q", ..}, "q2": "c"}` (the parent slot as
//! the exact linear form sum_j c_j D^child_j + c q^2 in the child's table slots, q^2 = 1). Several maps may be stored
//! per sector (`chosen`, `first`, `c0`, ..): the caller names the one to use (default `chosen`).
//!
//! FRAME BINDING ([`EdgeSide::check_frame`]): the edge's table-frame propagators (its `presentation_slots` through
//! `slot_map_table_to_presentation`) must equal the propagators of the node's own table header (`family_source`, read
//! by the core's family model [`crate::replay_base::family::Family`]) slot by slot, up to the sign of each momentum, and
//! its `table_lines` must equal the header's line mask. The edge file is DATA: nothing here knows a node.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::closure::sha256::Sha256;

/// The schema this reader accepts.
pub const EDGE_SCHEMA: &str = "handdown-routes.edge.v1";

/// An exact rational with i64 parts (den > 0, reduced).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rat {
    pub num: i64,
    pub den: i64,
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a as i64
}

impl Rat {
    pub fn new(num: i64, den: i64) -> Result<Rat, String> {
        if den == 0 {
            return Err("zero denominator".into());
        }
        let g = gcd(num, den).max(1);
        let s = if den < 0 { -1 } else { 1 };
        Ok(Rat {
            num: s * num / g,
            den: s * den / g,
        })
    }
    pub fn is_zero(&self) -> bool {
        self.num == 0
    }
    /// Parse `p`, `-p`, `p/q` (also a JSON number).
    pub fn parse(v: &Value) -> Result<Rat, String> {
        match v {
            Value::Number(n) => n
                .as_i64()
                .map(|x| Rat { num: x, den: 1 })
                .ok_or_else(|| format!("rational {n}: not an integer")),
            Value::String(s) => {
                // outer parentheses (the engine writes `(2)`, `(-1/2)`) and whitespace are not part of the number
                let mut s = s.trim();
                while let Some(x) = s.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
                    s = x.trim();
                }
                match s.split_once('/') {
                    None => s
                        .parse::<i64>()
                        .map(|x| Rat { num: x, den: 1 })
                        .map_err(|e| format!("rational {s:?}: {e}")),
                    Some((p, q)) => Rat::new(
                        p.trim()
                            .parse()
                            .map_err(|e| format!("rational {s:?}: {e}"))?,
                        q.trim()
                            .parse()
                            .map_err(|e| format!("rational {s:?}: {e}"))?,
                    ),
                }
            }
            x => Err(format!("rational {x}: a string or an integer expected")),
        }
    }
    /// `p/q` text (`p` when q = 1).
    pub fn text(&self) -> String {
        if self.den == 1 {
            self.num.to_string()
        } else {
            format!("{}/{}", self.num, self.den)
        }
    }
}

/// What a parent table slot becomes in the child frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotSub {
    /// A surviving parent line: child table line `j` (power carried over).
    Line(usize),
    /// A parent numerator or dropped line: sum_j c_j D^child_j + q2 (q^2 = 1); `coefs` sorted by slot, no zeros.
    Form { coefs: Vec<(usize, Rat)>, q2: Rat },
}

/// One routed sector of the parent onto the child's class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutedSector {
    /// The parent table line slots dropped (sorted) and their mask.
    pub dropped: Vec<usize>,
    pub dropped_mask: u64,
    pub kept: Vec<usize>,
    pub class: Option<String>,
    /// The map used (one entry per parent table slot).
    pub subst: Vec<SlotSub>,
    /// The candidate index of the library's choice (echo of the file; `None` if absent).
    pub chosen_candidate: Option<i64>,
}

/// One side (parent or child) of the edge: its table frame as the edge file states it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeSide {
    pub label: String,
    /// The table the probe read (a path as written in the file).
    pub table: String,
    /// Per table slot: the momentum (loops then externals) whose square the slot is.
    pub momenta: Vec<Vec<i64>>,
    pub table_lines: Vec<usize>,
}

/// An edge file.
#[derive(Clone, Debug)]
pub struct EdgeFile {
    pub schema: String,
    pub edge: String,
    pub parent: EdgeSide,
    pub child: EdgeSide,
    pub owner_class: Option<String>,
    pub map: String,
    pub sectors: Vec<RoutedSector>,
    /// Sectors of the file that are NOT routed onto the child (counted; their masks).
    pub other_sectors: Vec<u64>,
    /// sha256 of the file bytes (hex).
    pub sha256: String,
    pub bytes: u64,
    pub path: String,
}

fn side(v: &Value, what: &str) -> Result<EdgeSide, String> {
    let label = v["label"].as_str().unwrap_or("").to_string();
    let table = v["table"].as_str().unwrap_or("").to_string();
    let sm: Vec<usize> = v["slot_map_table_to_presentation"]
        .as_array()
        .ok_or_else(|| format!("{what}: no slot_map_table_to_presentation"))?
        .iter()
        .map(|x| x.as_u64().map(|u| u as usize))
        .collect::<Option<_>>()
        .ok_or_else(|| format!("{what}: bad slot map"))?;
    let ps = v["presentation_slots"]
        .as_array()
        .ok_or_else(|| format!("{what}: no presentation_slots"))?;
    let vecs =
        |x: &Value| -> Option<Vec<i64>> { x.as_array()?.iter().map(Value::as_i64).collect() };
    let mut momenta = vec![];
    for (t, &p) in sm.iter().enumerate() {
        let s = ps
            .get(p)
            .ok_or_else(|| format!("{what}: table slot {t} -> presentation slot {p} missing"))?;
        let m = if let Some(l) = s.get("line") {
            vecs(l).ok_or_else(|| format!("{what}: slot {p}: bad line"))?
        } else if let Some(i) = s.get("isp") {
            let a = vecs(&i[0]).ok_or_else(|| format!("{what}: slot {p}: bad isp"))?;
            let b = vecs(&i[1]).ok_or_else(|| format!("{what}: slot {p}: bad isp"))?;
            let neg: Vec<i64> = b.iter().map(|x| -x).collect();
            if a != b && a != neg {
                return Err(format!(
                    "{what}: slot {p}: a scalar product of two different momenta (unsupported: not a square)"
                ));
            }
            a
        } else {
            return Err(format!(
                "{what}: presentation slot {p}: neither line nor isp"
            ));
        };
        momenta.push(m);
    }
    let table_lines = v["table_lines"]
        .as_array()
        .ok_or_else(|| format!("{what}: no table_lines"))?
        .iter()
        .map(|x| x.as_u64().map(|u| u as usize))
        .collect::<Option<_>>()
        .ok_or_else(|| format!("{what}: bad table_lines"))?;
    Ok(EdgeSide {
        label,
        table,
        momenta,
        table_lines,
    })
}

fn sub(v: &Value, t: usize) -> Result<SlotSub, String> {
    if let Some(j) = v.get("line") {
        return j
            .as_u64()
            .map(|j| SlotSub::Line(j as usize))
            .ok_or_else(|| format!("slot {t}: bad line"));
    }
    let f = v
        .get("form")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("slot {t}: neither line nor form"))?;
    let mut coefs = vec![];
    for (j, c) in f {
        let j: usize = j
            .parse()
            .map_err(|e| format!("slot {t}: form slot {j:?}: {e}"))?;
        let c = Rat::parse(c).map_err(|e| format!("slot {t}: {e}"))?;
        if !c.is_zero() {
            coefs.push((j, c));
        }
    }
    coefs.sort();
    let q2 = match v.get("q2") {
        None | Some(Value::Null) => Rat { num: 0, den: 1 },
        Some(x) => Rat::parse(x).map_err(|e| format!("slot {t}: q2: {e}"))?,
    };
    Ok(SlotSub::Form { coefs, q2 })
}

fn usizes(v: &Value) -> Option<Vec<usize>> {
    match v {
        Value::Array(a) => a.iter().map(|x| x.as_u64().map(|u| u as usize)).collect(),
        Value::Number(n) => n.as_u64().map(|u| vec![u as usize]),
        _ => None,
    }
}

impl EdgeFile {
    /// Parse an edge file's bytes with map `map` (e.g. `chosen`).
    pub fn parse(bytes: &[u8], path: &str, map: &str) -> Result<EdgeFile, String> {
        let mut h = Sha256::new();
        h.update(bytes);
        let sha256 = h.finish_hex();
        let v: Value = serde_json::from_slice(bytes).map_err(|e| format!("{path}: {e}"))?;
        let schema = v["schema"].as_str().unwrap_or("").to_string();
        if schema != EDGE_SCHEMA {
            return Err(format!(
                "{path}: schema {schema:?} (expected {EDGE_SCHEMA})"
            ));
        }
        let parent = side(&v["parent"], "parent")?;
        let child = side(&v["child"], "child")?;
        let arity = parent.momenta.len();
        if child.momenta.len() != arity {
            return Err(format!(
                "{path}: parent arity {arity} != child arity {}",
                child.momenta.len()
            ));
        }
        if arity > 64 {
            return Err(format!("{path}: arity {arity} > 64"));
        }
        let mut sectors = vec![];
        let mut other = vec![];
        for (i, s) in v["sectors"]
            .as_array()
            .ok_or_else(|| format!("{path}: no sectors"))?
            .iter()
            .enumerate()
        {
            let dropped = usizes(&s["parent_table_slots_dropped"])
                .or_else(|| usizes(&s["parent_table_slot_dropped"]))
                .ok_or_else(|| format!("{path}: sector {i}: no parent_table_slots_dropped"))?;
            let mut dropped = dropped;
            dropped.sort_unstable();
            dropped.dedup();
            if dropped.iter().any(|&t| t >= arity) {
                return Err(format!("{path}: sector {i}: dropped slot out of range"));
            }
            let mask = dropped.iter().fold(0u64, |a, &t| a | (1u64 << t));
            if !s["fate"].as_str().unwrap_or("").starts_with("routed") {
                other.push(mask);
                continue;
            }
            let kept = usizes(&s["parent_table_lines_kept"])
                .ok_or_else(|| format!("{path}: sector {i}: no parent_table_lines_kept"))?;
            let m = s["substitution"][map]
                .as_array()
                .ok_or_else(|| format!("{path}: sector {i}: no substitution map {map:?}"))?;
            if m.len() != arity {
                return Err(format!(
                    "{path}: sector {i}: map of {} slots for arity {arity}",
                    m.len()
                ));
            }
            let subst = m
                .iter()
                .enumerate()
                .map(|(t, x)| sub(x, t))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("{path}: sector {i}: {e}"))?;
            // consistency: line entries exactly on the kept lines, a bijection onto the child's lines; forms in range
            let mut img = vec![];
            for (t, x) in subst.iter().enumerate() {
                match x {
                    SlotSub::Line(j) => {
                        if !kept.contains(&t) {
                            return Err(format!(
                                "{path}: sector {i}: slot {t} maps to a line but is not a kept line"
                            ));
                        }
                        img.push(*j);
                    }
                    SlotSub::Form { coefs, .. } => {
                        if kept.contains(&t) {
                            return Err(format!(
                                "{path}: sector {i}: kept line {t} written as a form"
                            ));
                        }
                        if coefs.iter().any(|(j, _)| *j >= arity) {
                            return Err(format!("{path}: sector {i}: form slot out of range"));
                        }
                    }
                }
            }
            img.sort_unstable();
            if img != child.table_lines {
                return Err(format!(
                    "{path}: sector {i}: line map image {img:?} != child lines {:?}",
                    child.table_lines
                ));
            }
            sectors.push(RoutedSector {
                dropped,
                dropped_mask: mask,
                kept,
                class: s["class"].as_str().map(str::to_string),
                subst,
                chosen_candidate: s["chosen_candidate_index"].as_i64(),
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for s in &sectors {
            if !seen.insert(s.dropped_mask) {
                return Err(format!(
                    "{path}: two routed sectors for dropped mask {:#x}",
                    s.dropped_mask
                ));
            }
        }
        Ok(EdgeFile {
            schema,
            edge: v["edge"].as_str().unwrap_or("").to_string(),
            parent,
            child,
            owner_class: v["owner"]["class"].as_str().map(str::to_string),
            map: map.to_string(),
            sectors,
            other_sectors: other,
            sha256,
            bytes: bytes.len() as u64,
            path: path.to_string(),
        })
    }

    /// Read and parse an edge file.
    pub fn read(path: &std::path::Path, map: &str) -> Result<EdgeFile, String> {
        let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        EdgeFile::parse(&b, &path.display().to_string(), map)
    }

    /// The routed sector of a dropped mask.
    pub fn sector(&self, dropped_mask: u64) -> Option<&RoutedSector> {
        self.sectors.iter().find(|s| s.dropped_mask == dropped_mask)
    }

    /// The identity of the map used (for the arrival record): file sha256, map name, per sector the dropped slots.
    pub fn identity(&self) -> Value {
        serde_json::json!({"path": self.path, "sha256": self.sha256, "bytes": self.bytes, "schema": self.schema,
            "edge": self.edge, "map": self.map, "parent_label": self.parent.label, "child_label": self.child.label,
            "parent_table": self.parent.table, "child_table": self.child.table, "owner_class": self.owner_class,
            "sectors": self.sectors.iter().map(|s| serde_json::json!({"dropped": s.dropped, "class": s.class,
                "chosen_candidate": s.chosen_candidate})).collect::<Vec<_>>()})
    }
}

impl EdgeSide {
    /// Check this side's table frame against a table header's `family_source` text and line mask (module docs).
    /// Returns the number of slots compared.
    pub fn check_frame(&self, family_source: &str, mask: &[bool]) -> Result<usize, String> {
        let fam = crate::replay_base::family::Family::parse(family_source)
            .map_err(|e| format!("family_source: {e}"))?;
        let m = fam.momenta();
        if m.len() != self.momenta.len() {
            return Err(format!(
                "{}: the header has {} slots, the edge {}",
                self.label,
                m.len(),
                self.momenta.len()
            ));
        }
        for (t, (a, b)) in m.iter().zip(&self.momenta).enumerate() {
            let neg: Vec<i64> = b.iter().map(|x| -x).collect();
            if a != b && *a != neg {
                return Err(format!(
                    "{}: table slot {t}: header momentum {a:?} != edge momentum {b:?} (up to sign)",
                    self.label
                ));
            }
        }
        let lines: Vec<usize> = mask
            .iter()
            .enumerate()
            .filter(|(_, l)| **l)
            .map(|(i, _)| i)
            .collect();
        if lines != self.table_lines {
            return Err(format!(
                "{}: header lines {lines:?} != edge table_lines {:?}",
                self.label, self.table_lines
            ));
        }
        Ok(m.len())
    }
}

/// Per dropped mask: which edge (index into `edges`) routes it (several edge files of ONE parent; a mask routed by two
/// files is refused: the census gives each sector one owner).
pub fn mask_owners(edges: &[EdgeFile]) -> Result<BTreeMap<u64, usize>, String> {
    let mut out = BTreeMap::new();
    for (i, e) in edges.iter().enumerate() {
        for s in &e.sectors {
            if let Some(j) = out.insert(s.dropped_mask, i) {
                return Err(format!(
                    "dropped mask {:#x} routed by two edge files ({} and {})",
                    s.dropped_mask, edges[j].path, e.path
                ));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A toy edge: a 2-loop parent with lines 0..3, numerator 4; the child drops parent line 2.
    pub(crate) fn toy_edge() -> Value {
        let pres = |v: &[[i64; 3]]| {
            v.iter()
                .map(|m| json!({"line": m, "eps_units": 0}))
                .collect::<Vec<_>>()
        };
        json!({"schema": EDGE_SCHEMA, "edge": "P->C",
            "parent": {"label": "P", "table": "p.jsonl", "slot_map_table_to_presentation": [0,1,2,3,4],
                "presentation_slots": pres(&[[1,0,0],[0,1,0],[1,0,-1],[0,1,-1],[1,-1,0]]), "table_lines": [0,1,2,3]},
            "child": {"label": "C", "table": "c.jsonl", "slot_map_table_to_presentation": [0,1,2,3,4],
                "presentation_slots": pres(&[[1,0,0],[0,1,0],[0,1,-1],[1,0,-1],[1,1,0]]), "table_lines": [0,1,2]},
            "owner": {"class": "toy"},
            "sectors": [
                {"fate": "other class", "parent_table_slots_dropped": [0]},
                {"fate": "routed onto the child's class", "parent_table_slots_dropped": [2],
                 "parent_table_lines_kept": [0,1,3], "class": "toy", "chosen_candidate_index": 0,
                 "substitution": {"chosen": [{"line": 0}, {"line": 1}, {"form": {"3": "1"}, "q2": "0"}, {"line": 2},
                     {"form": {"0": "2", "1": "2", "4": "-1"}, "q2": "0"}]}}]})
    }

    #[test]
    fn parses_checks_and_refuses() {
        let e = EdgeFile::parse(toy_edge().to_string().as_bytes(), "toy", "chosen").unwrap();
        assert_eq!(e.sectors.len(), 1);
        assert_eq!(e.sectors[0].dropped_mask, 1 << 2);
        assert_eq!(e.other_sectors, vec![1]);
        assert!(e.sector(4).is_some() && e.sector(1).is_none());
        let pf = "I(name(P),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),\
                  prop(D3,(k1-q)^2,1),prop(D4,(-k2+q)^2,1),prop(D5,(k1-k2)^2,0),gram(q,q,1))";
        let lines = [true, true, true, true, false];
        assert_eq!(e.parent.check_frame(pf, &lines).unwrap(), 5);
        // a permuted header is refused
        let bad = pf
            .replace("(k1-q)^2", "(k2-q)^2")
            .replace("(-k2+q)^2", "(k1-q)^2");
        assert!(e.parent.check_frame(&bad, &lines).is_err());
        assert!(
            e.parent
                .check_frame(pf, &[true, true, true, false, false])
                .is_err()
        );
        // a routed sector whose line map is not a bijection onto the child's lines is refused
        let mut v = toy_edge();
        v["sectors"][1]["substitution"]["chosen"][3] = json!({"line": 1});
        assert!(EdgeFile::parse(v.to_string().as_bytes(), "toy", "chosen").is_err());
        // an unknown map name is refused
        assert!(EdgeFile::parse(toy_edge().to_string().as_bytes(), "toy", "c9").is_err());
        assert_eq!(Rat::parse(&json!("-6/4")).unwrap(), Rat { num: -3, den: 2 });
        assert_eq!(Rat::parse(&json!("(2)")).unwrap(), Rat { num: 2, den: 1 });
        assert_eq!(
            Rat::parse(&json!(" ( -1/2 ) ")).unwrap(),
            Rat { num: -1, den: 2 }
        );
    }
}
