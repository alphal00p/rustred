//! Strict-descent check of an assembled rule set under its ONE declared node order.
//!
//! Every rule `target -> sum c_k I_k` of a node table must satisfy `I_k < target` in the node order (the engine's
//! harder-first [`IntegralOrder`] for the chart, with the order's tie priority: `compare(target, I_k) == Less`). The check is independent of how the rules were
//! found: a set that joins rules found under different orders is rejected as soon as one rule is not descending
//! under the declared order. Regression: the T212 reciprocal 2-cycle (Astra): a cheap slice rule of the legacy order
//! and the reversed-ISP corner rules each descend under their own order, and none of the two orders accepts both.

use std::cmp::Ordering;

use super::order::NodeOrder;
use crate::solver::{Integral, IntegralOrder, Power};

/// Parse a table key `I(1,n1+1,0,n3-2,...)` (the engine's Display form) into an [`Integral`].
pub fn parse_key<const N: usize>(text: &str) -> Result<Integral<N>, String> {
    let inner = text
        .trim()
        .strip_prefix("I(")
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| format!("not an integral key: {text}"))?;
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != N {
        return Err(format!(
            "key {text}: {} coordinates, expected {N}",
            parts.len()
        ));
    }
    let mut powers = [Power::default(); N];
    for (i, p) in parts.iter().enumerate() {
        let p = p.trim();
        let power = if let Some(rest) = p.strip_prefix('n') {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let axis: usize = digits
                .parse()
                .map_err(|_| format!("key {text}: bad symbol {p}"))?;
            if axis != i {
                return Err(format!("key {text}: symbol n{axis} at position {i}"));
            }
            let offset = &rest[digits.len()..];
            let value: i16 = if offset.is_empty() {
                0
            } else {
                offset
                    .trim_start_matches('+')
                    .parse()
                    .map_err(|_| format!("key {text}: bad offset {p}"))?
            };
            Power::new(true, value)
        } else {
            Power::new(
                false,
                p.parse()
                    .map_err(|_| format!("key {text}: bad value {p}"))?,
            )
        };
        powers[i] = power.map_err(|e| format!("key {text}: {e}"))?;
    }
    Ok(Integral::new(powers))
}

/// The engine integral order of a node order on a chart (`sector` = the chart mask).
pub fn integral_order<const N: usize>(
    order: &NodeOrder,
    sector: [bool; N],
    arity: usize,
) -> Result<IntegralOrder<N>, String> {
    order.validate(arity)?;
    let base = IntegralOrder::new(sector, [false; N])
        .with_physical_arity(arity)
        .map_err(|e| format!("{e:?}"))?;
    match &order.coordinate_priority {
        None => Ok(base),
        Some(priority) => {
            let mut d = rustred_order::OrderDescriptor::spired_uncut(arity);
            d.coordinate_priority = priority.clone();
            let program = rustred_order::CompiledOrder::compile(d, Default::default())
                .map_err(|e| format!("order program: {e:?}"))?;
            base.with_program(program).map_err(|e| format!("{e:?}"))
        }
    }
}

/// One rule as the descent check sees it.
#[derive(Clone, Debug)]
pub struct KeyRule<const N: usize> {
    pub label: String,
    pub target: Integral<N>,
    pub rhs: Vec<Integral<N>>,
}

/// A term of a rule that is not strictly lower than its target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotLower {
    pub rule: String,
    pub target: String,
    pub term: String,
}

#[derive(Clone, Debug, Default)]
pub struct DescentReport {
    pub rules: usize,
    pub terms: usize,
    pub not_lower: Vec<NotLower>,
}

impl DescentReport {
    pub fn passed(&self) -> bool {
        self.not_lower.is_empty()
    }

    /// Rules with at least one non-lower term.
    pub fn failing_rules(&self) -> Vec<String> {
        let mut out: Vec<String> = self.not_lower.iter().map(|x| x.rule.clone()).collect();
        out.dedup();
        out
    }
}

/// Check every rule of `rules` under `order`.
pub fn check<const N: usize>(order: &IntegralOrder<N>, rules: &[KeyRule<N>]) -> DescentReport {
    let mut report = DescentReport {
        rules: rules.len(),
        ..Default::default()
    };
    for rule in rules {
        for term in &rule.rhs {
            report.terms += 1;
            // the engine order is harder-first: a lower term compares Greater than its target
            // (the engine's own check: `compare(target, term) != Less` = not lower)
            if order.compare(&rule.target, term) != Ordering::Less {
                report.not_lower.push(NotLower {
                    rule: rule.label.clone(),
                    target: rule.target.to_string(),
                    term: term.to_string(),
                });
            }
        }
    }
    report
}

/// The node order a table header declares: `node_order` (rustred node tables) or, for face-queue tables, the
/// legacy order / a compiled `order` program's coordinate priority (presentation = the header's family as is).
pub fn header_order(header: &serde_json::Value) -> Result<NodeOrder, String> {
    if let Some(v) = header.get("node_order") {
        return serde_json::from_value(v.clone()).map_err(|e| format!("header node_order: {e}"));
    }
    let is_program = header
        .get("order_is_program")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !is_program {
        return Ok(NodeOrder::identity());
    }
    let priority: Vec<usize> = header
        .pointer("/order/coordinate_priority")
        .and_then(|v| v.as_array())
        .ok_or("header order program without coordinate_priority")?
        .iter()
        .map(|x| {
            x.as_u64()
                .map(|x| x as usize)
                .ok_or("coordinate_priority entry")
        })
        .collect::<Result<_, _>>()?;
    let name = header
        .get("order_name")
        .and_then(|v| v.as_str())
        .unwrap_or("program")
        .to_string();
    Ok(NodeOrder {
        name,
        coordinate_priority: Some(priority),
        ..NodeOrder::identity()
    })
}

/// Check a table file (JSONL, table_dump.v1 records) under `order` (default: the header's declared order).
/// Returns the order used and the report per chart.
pub fn check_table_file(
    path: &str,
    order: Option<NodeOrder>,
) -> Result<(NodeOrder, DescentReport), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut header = None;
    let mut rules: Vec<(String, String, String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("{path}: {e}"))?;
        match v.get("type").and_then(|t| t.as_str()) {
            Some("header") => header = Some(v),
            Some("rule") => {
                let chart = v["chart"].as_str().unwrap_or_default().to_string();
                let label = format!("{}#{}", chart, v["index"]);
                let target = v["target"].as_str().ok_or("rule target")?.to_string();
                let rhs = v["rhs"]
                    .as_array()
                    .ok_or("rule rhs")?
                    .iter()
                    .map(|t| t[0].as_str().unwrap_or_default().to_string())
                    .collect();
                rules.push((chart, label, target, rhs));
            }
            _ => {}
        }
    }
    let header = header.ok_or("table without header")?;
    let order = match order {
        Some(o) => o,
        None => header_order(&header)?,
    };
    let arity = header["denominators"]
        .as_u64()
        .ok_or("header denominators")? as usize;
    let report = match arity {
        5 => check_keyed::<5>(&order, arity, &rules),
        9 => check_keyed::<9>(&order, arity, &rules),
        12 => check_keyed::<12>(&order, arity, &rules),
        14 => check_keyed::<14>(&order, arity, &rules),
        20 => check_keyed::<20>(&order, arity, &rules),
        other => Err(format!("arity {other} not compiled in the descent check")),
    }?;
    Ok((order, report))
}

/// Keyed rules (chart, label, target, rhs keys) under one order; the chart mask is each rule's sector.
pub fn check_keyed<const N: usize>(
    order: &NodeOrder,
    arity: usize,
    rules: &[(String, String, String, Vec<String>)],
) -> Result<DescentReport, String> {
    let mut total = DescentReport::default();
    let mut charts: Vec<&str> = rules.iter().map(|r| r.0.as_str()).collect();
    charts.sort_unstable();
    charts.dedup();
    for chart in charts {
        let bits: Vec<bool> = chart.chars().map(|c| c == '1').collect();
        if bits.len() != arity {
            return Err(format!("chart {chart}: arity {}", bits.len()));
        }
        let mut sector = [false; N];
        sector[..arity].copy_from_slice(&bits);
        let io = integral_order::<N>(order, sector, arity)?;
        let keyed: Vec<KeyRule<N>> = rules
            .iter()
            .filter(|r| r.0 == chart)
            .map(|(_, label, target, rhs)| {
                Ok(KeyRule {
                    label: label.clone(),
                    target: parse_key(target)?,
                    rhs: rhs
                        .iter()
                        .map(|k| parse_key(k))
                        .collect::<Result<_, String>>()?,
                })
            })
            .collect::<Result<_, String>>()?;
        let r = check(&io, &keyed);
        total.rules += r.rules;
        total.terms += r.terms;
        total.not_lower.extend(r.not_lower);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("testdata/t212_mixed_orders.json")).unwrap()
    }

    fn keyed(set: &serde_json::Value, prefix: &str) -> Vec<(String, String, String, Vec<String>)> {
        set.as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    "11111111111100000000".to_string(),
                    format!("{prefix}#{}", r["index"]),
                    r["target"].as_str().unwrap().to_string(),
                    r["rhs_keys"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|k| k.as_str().unwrap().to_string())
                        .collect(),
                )
            })
            .collect()
    }

    fn tie_isp_rev() -> NodeOrder {
        NodeOrder {
            name: "tie_isp_rev".into(),
            coordinate_priority: Some((0..12).chain((12..20).rev()).collect()),
            ..NodeOrder::identity()
        }
    }

    #[test]
    fn parse_key_roundtrip() {
        for k in ["I(1,1,n2,n3+1,0,-2)", "I(n0-3,1,1,1,1,1)"] {
            assert_eq!(parse_key::<6>(k).unwrap().to_string(), k);
        }
        assert!(parse_key::<6>("I(1,1,n3,1,1,1)").is_err());
        assert!(parse_key::<5>("I(1,1,1,1,1,1)").is_err());
    }

    #[test]
    fn each_t212_set_descends_under_its_own_order() {
        let f = fixture();
        let v1 = keyed(&f["sets"]["v1_identity"], "v1");
        let v3 = keyed(&f["sets"]["v3_tie_isp_rev"], "v3");
        let r1 = check_keyed::<20>(&NodeOrder::identity(), 20, &v1).unwrap();
        assert!(r1.passed() && r1.rules == 1 && r1.terms == 234, "{r1:?}");
        let r3 = check_keyed::<20>(&tie_isp_rev(), 20, &v3).unwrap();
        assert!(
            r3.passed() && r3.rules == 3 && r3.terms == 147 + 255 + 172,
            "{:?}",
            r3.not_lower.first()
        );
    }

    #[test]
    fn t212_reciprocal_two_cycle_mixing_orders_is_rejected_under_every_one_order() {
        let f = fixture();
        let mut mixed = keyed(&f["sets"]["v1_identity"], "v1");
        mixed.extend(keyed(&f["sets"]["v3_tie_isp_rev"], "v3"));
        let under_id = check_keyed::<20>(&NodeOrder::identity(), 20, &mixed).unwrap();
        assert!(!under_id.passed());
        assert!(
            under_id.failing_rules().iter().all(|r| r.starts_with("v3")),
            "{:?}",
            under_id.failing_rules()
        );
        // Astra's common-order audit: 11 terms of the v3 rules are not lower under the legacy order
        assert_eq!(under_id.not_lower.len(), 11);
        let under_rev = check_keyed::<20>(&tie_isp_rev(), 20, &mixed).unwrap();
        assert!(!under_rev.passed());
        assert_eq!(under_rev.failing_rules(), vec!["v1#0".to_string()]);
        // Astra's order audit: 11 non-lower terms of v1/r0 under the reversed ISP priority
        assert_eq!(under_rev.not_lower.len(), 11);
    }

    #[test]
    fn header_order_reads_programs_and_node_orders() {
        let h: serde_json::Value = serde_json::json!({"order_is_program": true, "order_name": "isp_reverse", "order": {"coordinate_priority": [0,1,2,4,3]}});
        let o = header_order(&h).unwrap();
        assert_eq!(o.coordinate_priority, Some(vec![0, 1, 2, 4, 3]));
        let h2 = serde_json::json!({"node_order": serde_json::to_value(tie_isp_rev()).unwrap()});
        assert_eq!(header_order(&h2).unwrap(), tie_isp_rev());
        assert_eq!(
            header_order(&serde_json::json!({"order_is_program": false})).unwrap(),
            NodeOrder::identity()
        );
    }
}
