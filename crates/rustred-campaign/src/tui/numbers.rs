//! "Website = terminal" (plan M8 (4)): the numbers of the SUMMARY screen read back from a rendered screen, and the
//! same numbers taken directly from campaign.json without the dashboard's adapter. A test (and the coordinator's
//! script) compares the two maps; the website's loader is expected to produce the second map from the same file.

use std::collections::BTreeMap;

use serde_json::Value;

use super::format::{count, gib, thousands};

/// Labels of the summary header and the key each value is stored under.
const LABELS: &[(&str, &str)] = &[
    ("seq ", "seq"),
    ("CPU ", "cpu"),
    ("RAM ", "ram_used_g"),
    ("NODES ", "nodes_total"),
    ("FULL ", "nodes_full"),
    ("BOXED ", "nodes_boxed"),
    ("OPEN ", "nodes_open"),
    ("CERTIFIED at the level asked: ", "nodes_certified"),
    ("ITEMS ", "items_total"),
    ("open ", "items_open"),
    ("RULES accepted ", "rules_accepted"),
    ("rejected ", "rules_rejected"),
    ("TERMINALS ", "terminals_declared"),
    ("DEFECT ", "defect_keys"),
    ("cpu-h ", "cpu_hours"),
];

fn token_after(line: &str, label: &str) -> Option<String> {
    // the label must start the line or follow a space
    let mut from = 0;
    while let Some(p) = line[from..].find(label) {
        let at = from + p;
        if at == 0 || line.as_bytes()[at - 1] == b' ' {
            let rest = &line[at + label.len()..];
            let tok: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
            if !tok.is_empty() {
                return Some(tok);
            }
        }
        from = at + label.len();
    }
    None
}

/// Numbers of the SUMMARY header block, read from the screen text (first 12 lines; banner, problem, message and
/// NEXT lines carry free text and are skipped).
pub fn screen_numbers(screen: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in screen.lines().take(12).filter(|l| {
        !(l.starts_with("NEXT") || l.starts_with('!') || l.starts_with(' ') || l.starts_with('>'))
    }) {
        for (label, key) in LABELS {
            if out.contains_key(*key) {
                continue;
            }
            if let Some(t) = token_after(line, label) {
                out.insert((*key).to_string(), t);
            }
        }
    }
    if let Some(cpu) = out.remove("cpu")
        && let Some((a, b)) = cpu.split_once('/')
    {
        out.insert("cpus_in_use".into(), a.into());
        out.insert("cpus_allowed".into(), b.into());
    }
    if let Some(r) = out.get("ram_used_g").cloned() {
        out.insert(
            "ram_used_g".into(),
            r.split('/').next().unwrap_or("").to_string(),
        );
    }
    if let Some(h) = out.get("cpu_hours").cloned() {
        out.insert(
            "cpu_hours".into(),
            h.split('/').next().unwrap_or("").to_string(),
        );
    }
    out
}

fn uv(c: &Value, p: &str) -> Option<u64> {
    c.pointer(p).and_then(Value::as_u64)
}

/// The same numbers straight from campaign.json (independent of `model::build`).
pub fn campaign_json_numbers(c: &Value) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let mut put = |k: &str, v: String| {
        m.insert(k.to_string(), v);
    };
    put("seq", count(uv(c, "/ledger/seq")));
    let allowed = uv(c, "/resources/cpus_allowed").filter(|v| *v > 0);
    put("cpus_in_use", count(uv(c, "/resources/cpus_in_use")));
    put("cpus_allowed", count(allowed));
    // an RSS of 0 while jobs are live was not sampled (shown `?`)
    let rss = uv(c, "/resources/measured_rss_bytes")
        .filter(|r| *r > 0 || uv(c, "/resources/live_jobs").unwrap_or(0) == 0);
    put("ram_used_g", gib(rss));
    put("nodes_total", count(uv(c, "/nodes/total")));
    put("nodes_full", count(uv(c, "/nodes/by_closure/full")));
    put("nodes_boxed", count(uv(c, "/nodes/by_closure/boxed")));
    put("nodes_open", count(uv(c, "/nodes/by_closure/open")));
    put("nodes_certified", count(uv(c, "/nodes/certified")));
    put("items_total", count(uv(c, "/items/total")));
    let open: Option<u64> = c
        .pointer("/items/by_phase")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "accepted" | "escalated" | "terminal" | "retired"
                    )
                })
                .filter_map(|(_, v)| v.as_u64())
                .sum()
        });
    put("items_open", count(open));
    put("rules_accepted", count(uv(c, "/rules/accepted")));
    put("rules_rejected", count(uv(c, "/rules/rejected")));
    put("terminals_declared", count(uv(c, "/terminals/declared")));
    put(
        "defect_keys",
        match uv(c, "/closure/defect_keys") {
            Some(d) => thousands(d),
            None => "not".into(), // "not checked yet"
        },
    );
    put(
        "cpu_hours",
        super::format::cpu_hours(c.pointer("/cost/cpu_seconds").and_then(Value::as_f64)),
    );
    m
}
