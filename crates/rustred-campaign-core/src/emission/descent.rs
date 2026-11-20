//! THE DESCENT MEMBER of an emitted record (emission contract section 14; implementer 2026-11-19, after lane t266's candidate (i):
//! four exact, d = 4 regular symmetry relations that ASCEND in T266's order and close a cycle with the partner's rule). Every
//! right-hand-side term of an emitted relation must be LOWER than its target in the node's order, by the ENGINE's own descent
//! test (`rr_node descent --table <header + the record>`, the order of the table header). Exact, regular and not lower =
//! REFUSED. The lifted path only ASSUMED descent (the node driver records failures as data: `native_descent_not_lower`; the
//! final certification needed a separate strict-descent step): the member applies to EVERY emitted record. PURE: parses and
//! binds the engine's output to the record; the process part runs `rr_node`.
use serde_json::Value;

/// The engine's descent verdict on a one-record table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descent {
    pub not_lower: u64,
    pub rules: u64,
    pub terms: u64,
    pub first: Option<String>,
    pub failing_rules: Vec<String>,
    pub order: Option<String>,
}

impl Descent {
    /// Every term lower than its target.
    pub fn lower(&self) -> bool {
        self.not_lower == 0 && self.failing_rules.is_empty()
    }
    /// The verdict word stored in the record.
    pub fn word(&self) -> &'static str {
        if self.lower() { "LOWER" } else { "NOT_LOWER" }
    }
}

/// Parse `rr_node descent` output (its last JSON line).
pub fn parse(out: &str) -> Result<Descent, String> {
    let line = out
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .ok_or("descent output without a JSON line")?;
    let v: Value = serde_json::from_str(line).map_err(|e| format!("descent output: {e}"))?;
    let n = |k: &str| {
        v[k].as_u64()
            .ok_or_else(|| format!("descent output without {k}"))
    };
    Ok(Descent {
        not_lower: n("not_lower")?,
        rules: n("rules")?,
        terms: n("terms")?,
        first: v["first"].as_str().map(str::to_string),
        failing_rules: v["failing_rules"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        order: v["order"]["name"].as_str().map(str::to_string),
    })
}

/// The descent verdict of a stored output, BOUND to a record body: the output must be of ONE rule with as many terms as the
/// body's rhs (the engine checked this record, not another). Ok(true) = every term lower.
pub fn verify(out: &str, body: &str) -> Result<bool, String> {
    let d = parse(out)?;
    let v: Value = serde_json::from_str(body).map_err(|e| format!("record: {e}"))?;
    let terms = v["rhs"].as_array().map_or(0, Vec::len) as u64;
    if d.rules != 1 || d.terms != terms {
        return Err(format!(
            "the descent output is of {} rule(s) / {} term(s), the record has 1 / {terms}",
            d.rules, d.terms
        ));
    }
    Ok(d.lower())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descent_outputs_bind_to_their_record() {
        // the engine's output on lane t266's candidate (i) K2 (rr_node descent, order phys_rev): NOT LOWER
        let k2 = r#"{"failing_rules":["11111111111111000000#0"],"first":"NotLower { rule: \"11111111111111000000#0\", target: \"I(1,1,1,1,1,1,1,1,1,1,1,1,1,1,0,0,0,-2,0,0)\", term: \"I(1,1,1,1,1,1,1,1,1,1,1,1,1,1,0,0,0,0,-2,0)\" }","not_lower":1,"order":{"coordinate_priority":null,"name":"phys_rev","slot_permutation":[13,12,11,10,9,8,7,6,5,4,3,2,1,0,14,15,16,17,18,19],"source_order":"Input"},"rules":1,"terms":6}"#;
        let d = parse(k2).unwrap();
        assert_eq!(
            (d.lower(), d.word(), d.order.as_deref()),
            (false, "NOT_LOWER", Some("phys_rev"))
        );
        let body6 = serde_json::json!({"rhs": [["a","1"],["b","1"],["c","1"],["d","1"],["e","1"],["f","1"]]}).to_string();
        assert_eq!(verify(k2, &body6), Ok(false));
        // another record (another term count): not bound
        let body2 = serde_json::json!({"rhs": [["a","1"],["b","1"]]}).to_string();
        assert!(verify(k2, &body2).is_err());
        let ok = r#"{"failing_rules":[],"first":null,"not_lower":0,"order":{"name":"phys_rev"},"rules":1,"terms":2}"#;
        assert_eq!(verify(ok, &body2), Ok(true));
        // a 13-rule output is not a one-record check
        let many = r#"{"failing_rules":[],"first":null,"not_lower":0,"order":{"name":"phys_rev"},"rules":13,"terms":22}"#;
        assert!(verify(many, &body2).is_err());
    }
}
