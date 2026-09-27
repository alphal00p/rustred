//! Parallel scan of the CP5 records JSONL segments (one JSON object per
//! line, in commit order) or of a pretty-printed `result.json` domain array.
//! Only the scalar fields the census needs are kept.
use crate::ckpt::{Manifest, map};
use rayon::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Default)]
struct Matching {
    pieces: Option<f64>,
    rules: Option<f64>,
    predicates: Option<f64>,
}
#[derive(Deserialize, Default)]
struct Stats {
    events: Option<f64>,
    successors: Option<f64>,
    apply_domains: Option<f64>,
    route_domains: Option<f64>,
    term_visits: Option<f64>,
    zero_terms: Option<f64>,
    shift_groups: Option<f64>,
    same_support_successors: Option<f64>,
    strict_subsupport_successors: Option<f64>,
    conditional_successors: Option<f64>,
    optional_coefficient_refusals: Option<f64>,
    selected_pieces: Option<f64>,
    matching: Option<Matching>,
}
#[derive(Deserialize)]
struct Line<'a> {
    id: u64,
    #[serde(borrow)]
    record_kind: &'a str,
    #[serde(borrow)]
    phase: Option<&'a str>,
    seconds: Option<f64>,
    stats: Option<Stats>,
    frontiers: Option<Vec<serde::de::IgnoredAny>>,
    error: Option<serde::de::IgnoredAny>,
    representative_id: Option<u64>,
}

pub const K_NATIVE: u8 = 0;
pub const K_DELEGATED: u8 = 1;
pub const K_OTHER: u8 = 2;
/// `partial_initial_overlap_inspection`: a native inspection of a D-residual.
pub const K_PARTIAL: u8 = 3;

#[derive(Clone, Copy, Default, Debug)]
pub struct Rec {
    pub id: u32,
    /// Record index in commit order (line number over all segments).
    pub seq: u32,
    pub kind: u8,
    /// 0 Apply, 1 Route, 255 unknown.
    pub phase: u8,
    /// Generation of the segment that holds the record.
    pub gen: u8,
    pub frontiers: u16,
    pub error: bool,
    pub seconds: f32,
    pub events: u32,
    pub successors: u32,
    pub pieces: u32,
    pub rules: u32,
    pub predicates: u32,
    pub term_visits: u32,
    pub zero_terms: u32,
    pub shift_groups: u32,
    pub same_support: u32,
    pub subsupport: u32,
    pub conditional: u32,
    pub coeff_refusals: u32,
    pub selected: u32,
    /// Representative (delegation target) for delegated records, else u32::MAX.
    pub rep: u32,
}

fn c(v: Option<f64>) -> u32 {
    v.map_or(0, |x| x.min(u32::MAX as f64) as u32)
}

fn parse(line: &[u8], gen: u8) -> Rec {
    let l: Line = serde_json::from_slice(line).unwrap_or_else(|e| {
        panic!("record parse: {e}: {}", String::from_utf8_lossy(&line[..line.len().min(300)]))
    });
    let st = l.stats.unwrap_or_default();
    let m = st.matching.unwrap_or_default();
    let phase = match l.phase {
        Some("Apply") => 0,
        Some("Route") => 1,
        _ => 255,
    };
    let succ = if phase == 1 {
        c(st.apply_domains) + c(st.route_domains)
    } else {
        c(st.successors)
    };
    Rec {
        id: l.id as u32,
        seq: 0,
        kind: match l.record_kind {
            "native_inspection" => K_NATIVE,
            "delegated_not_inspected" => K_DELEGATED,
            "partial_initial_overlap_inspection" => K_PARTIAL,
            _ => K_OTHER,
        },
        phase,
        gen,
        frontiers: l.frontiers.map_or(0, |f| f.len().min(u16::MAX as usize) as u16),
        error: l.error.is_some(),
        seconds: l.seconds.unwrap_or(0.0) as f32,
        events: c(st.events),
        successors: succ,
        pieces: c(m.pieces),
        rules: c(m.rules),
        predicates: c(m.predicates),
        term_visits: c(st.term_visits),
        zero_terms: c(st.zero_terms),
        shift_groups: c(st.shift_groups),
        same_support: c(st.same_support_successors),
        subsupport: c(st.strict_subsupport_successors),
        conditional: c(st.conditional_successors),
        coeff_refusals: c(st.optional_coefficient_refusals),
        selected: c(st.selected_pieces),
        rep: l.representative_id.map_or(u32::MAX, |r| r as u32),
    }
}

/// Split a byte buffer into ~`chunk` pieces at line boundaries.
fn chunks(b: &[u8], chunk: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut s = 0;
    while s < b.len() {
        let mut e = (s + chunk).min(b.len());
        while e < b.len() && b[e - 1] != b'\n' {
            e += 1;
        }
        out.push((s, e));
        s = e;
    }
    out
}

/// All records of the manifest, in commit order, with `seq` assigned.
pub fn records(m: &Manifest) -> Vec<Rec> {
    let mut all: Vec<Rec> = Vec::new();
    for seg in &m.records {
        let b = map(&seg.path);
        let gen = seg.generation as u8;
        let parts: Vec<Vec<Rec>> = chunks(&b, 64 << 20)
            .into_par_iter()
            .map(|(s, e)| {
                b[s..e]
                    .split(|&x| x == b'\n')
                    .filter(|l| !l.is_empty())
                    .map(|l| parse(l, gen))
                    .collect()
            })
            .collect();
        let before = all.len();
        for p in parts {
            all.extend(p);
        }
        assert_eq!(all.len() - before, seg.count, "{} record count", seg.path.display());
    }
    for (i, r) in all.iter_mut().enumerate() {
        r.seq = i as u32;
    }
    all
}

/// Records of a pretty-printed `result.json` (four-space indented objects in
/// a `domains` array, as written by `owner-domain-match`). Returns the records
/// and the geometry of each record, parsed from its own fields.
pub fn result_json(path: &std::path::Path, n: usize) -> Vec<(Rec, crate::geom::Dom)> {
    let b = map(path);
    // Collect object spans: lines "    {" .. "    }," at indentation 4.
    let mut spans = Vec::new();
    let mut start = None;
    let mut pos = 0usize;
    let mut inside = false;
    for line in b.split(|&x| x == b'\n') {
        if line == b"  \"domains\": [" {
            inside = true;
        } else if inside && (line == b"  ]," || line == b"  ]") {
            inside = false;
        }
        if !inside {
            pos += line.len() + 1;
            continue;
        }
        if line == b"    {" {
            start = Some(pos);
        } else if (line == b"    }," || line == b"    }") && start.is_some() {
            spans.push((start.take().unwrap(), pos + 5));
        }
        pos += line.len() + 1;
    }
    let mut out: Vec<(Rec, crate::geom::Dom)> = spans
        .par_iter()
        .map(|&(s, e)| {
            let text = &b[s..e];
            let rec = parse(text, 0);
            let v: serde_json::Value = serde_json::from_slice(text).unwrap();
            (rec, dom_from_json(&v, n))
        })
        .collect();
    for (i, r) in out.iter_mut().enumerate() {
        r.0.seq = i as u32;
    }
    out
}

pub fn dom_from_json(v: &serde_json::Value, n: usize) -> crate::geom::Dom {
    use crate::geom::*;
    let mut d = Dom::default();
    d.phase = if v["phase"] == "Route" { 1 } else { 0 };
    let owner = v["owner"].as_str().unwrap().as_bytes();
    assert_eq!(owner.len(), n);
    for i in 0..n {
        if owner[i] == b'1' {
            d.owner |= 1 << i;
        }
        d.lo[i] = v["lower"][i].as_u64().unwrap() as u8;
        d.hi[i] = v["upper"][i].as_u64().map_or(INF8, |x| x as u8);
    }
    d.rank = v["rank"].as_u64().map_or(INF8, |x| x as u8);
    let pb = &v["power_bounds"];
    d.amax = pb["max_positive_power"].as_u64().map_or(NONE16, |x| x as u16);
    d.dmin = pb["min_power_difference"].as_i64().map_or(DLO_NONE, |x| x as i16);
    d.dmax = pb["max_power_difference"].as_i64().map_or(DHI_NONE, |x| x as i16);
    d
}

#[inline]
pub fn is_native(kind: u8) -> bool {
    kind == K_NATIVE || kind == K_PARTIAL
}
