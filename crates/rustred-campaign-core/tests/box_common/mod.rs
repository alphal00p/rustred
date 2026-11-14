//! TEST helper / example generator: emits the spec's band boxes (the R* box presets of `tests/data/box/
//! rstar_presets.json`) as `rustred.campaign.box.v1` DATA for a given chart. The library knows no box shape (rule L8);
//! the frozen five-loop counts are tests of this generator plus the generic enumerator.
#![allow(dead_code)]

use rustred_campaign_core::boxdef::{BandSpec, BoxDef};

pub const PRESETS: &str = include_str!("../data/box/rstar_presets.json");

pub fn preset(id: &str) -> BandSpec {
    let v: serde_json::Value = serde_json::from_str(PRESETS).expect("presets JSON");
    let p = v["presets"]
        .as_array()
        .expect("presets")
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("no preset {id}"));
    BandSpec::from_value(p).expect("band spec")
}

/// A lines-first chart: `n_lines` line slots, then numerator slots up to `arity`.
pub fn lines_first(arity: usize, n_lines: usize) -> Vec<bool> {
    (0..arity).map(|i| i < n_lines).collect()
}

/// The R* union band ("box B") for a chart, as box DATA (JSON round trip, so the spec form is what is tested).
pub fn rstar_union(chart: &[bool]) -> BoxDef {
    let b = BoxDef::from_band(&preset("rstar-union-v1"), chart).expect("band box");
    BoxDef::from_json(&b.to_json()).expect("round trip")
}

pub fn rstar_main(chart: &[bool]) -> BoxDef {
    let b = BoxDef::from_band(&preset("rstar-main-v1"), chart).expect("band box");
    BoxDef::from_json(&b.to_json()).expect("round trip")
}

/// Peak resident set (VmHWM) of this process in kB, if readable.
pub fn vm_hwm_kb() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|l| l.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}
