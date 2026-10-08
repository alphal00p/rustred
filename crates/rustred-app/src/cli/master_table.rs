//! Bounded terminal presentation of a native artifact summary; no file I/O.
use serde_json::Value;
use tabled::builder::Builder;
use tabled::settings::Style;

fn clean(value: &str, limit: usize) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

fn field(summary: &Value, name: &str) -> String {
    let value = name.split('.').fold(summary, |value, key| &value[key]);
    if name == "normalization_profile" && value.is_null() {
        return "conservative-v1 (legacy)".into();
    }
    match value {
        Value::Number(value) => value.to_string(),
        Value::String(value) => clean(value, 4096),
        Value::Bool(value) => if *value { "yes" } else { "no" }.into(),
        _ => "not reported".into(),
    }
}

fn paint(value: &str, color: bool, code: &str) -> String {
    if color {
        format!("\x1b[{code}m{value}\x1b[0m")
    } else {
        value.into()
    }
}

/// Pretty summary only. Unbounded rule/terminal payloads never enter this view.
pub(super) fn render_master_table(summary: &Value, color: bool, width: usize) -> String {
    let width = width.clamp(40, 160);
    let label_width = 25.min(width / 2);
    let value_width = width.saturating_sub(label_width + 7);
    let mut builder = Builder::default();
    builder.push_record([
        paint("ARTIFACT", color, "1;36"),
        paint(&clean("SAVED CAMPAIGN", value_width), color, "1;36"),
    ]);
    let fields = [
        ("Status", "status"),
        ("Selection", "scope_selection"),
        ("Master refinement", "refinement_status"),
        ("Installed rule records", "inventory.installed.rules"),
        ("Observed cover rules", "inventory.encountered.rules"),
        ("Published R cap", "scope.max_starting_rank"),
        ("Published D cap", "scope.max_starting_d"),
        ("Requested R cap", "requested_scope.max_starting_rank"),
        ("Requested D cap", "requested_scope.max_starting_d"),
        ("Query domains", "scope.starting_queries"),
        ("Current scope terminals", "inventory.encountered.terminals"),
        ("Primary source keys", "raw_terminals"),
        ("Normalization profile", "normalization_profile"),
        ("Primary normalized keys", "normalized_terminals"),
        ("Primary output basis keys", "remaining_terminals"),
        ("Primary before collection", "finite_remaining_terminals"),
        ("Collection strategy", "collection.strategy"),
        ("Collection families", "collection.families"),
        ("All-family source keys", "collection.raw_terminals"),
        ("All-family normalized", "collection.normalized_terminals"),
        ("All-family before collect", "collection.before_collection"),
        ("Full-U alias classes", "collection.global_alias_classes"),
        (
            "Stored diagonal identities",
            "collection.terminal_equations",
        ),
        ("Retained proof layers", "collection.proof_layers"),
        ("Finite feedback discovery", "finite_feedback"),
        ("Finite feedback stage", "collection.finite_feedback_stage"),
        ("Feedback source rows", "collection.finite_feedback_rows"),
        ("Feedback columns", "collection.finite_feedback_columns"),
        (
            "Feedback auxiliary cols",
            "collection.finite_feedback_auxiliary_columns",
        ),
        (
            "Feedback full-U aliases",
            "collection.finite_feedback_aliases",
        ),
        (
            "Feedback identities",
            "collection.finite_feedback_equations",
        ),
        ("All-family basis keys", "collection.remaining_terminals"),
        (
            "Feedback sparse nonzeros",
            "collection.finite_feedback_nonzeros",
        ),
        (
            "Feedback replay work",
            "collection.finite_feedback_replay_operations",
        ),
        ("Unchanged pass-through", "collection.passthrough_terminals"),
        ("Primary eliminated keys", "eliminated_terminals"),
        ("Ordinary IBP rows", "relation_rows"),
        ("Saved-rule assistance", "saved_rule_assistance"),
        ("Circuit assistance", "circuit_symmetry_assistance"),
        (
            "Inherited saved authority",
            "inherited_equation_authority.saved_rule_assistance",
        ),
        (
            "Inherited circuit rows",
            "inherited_equation_authority.circuit_symmetry_assistance",
        ),
        ("Assisted equation rows", "completed_assistance_rows"),
        ("Independent sparse rows", "independent_rows"),
        ("Auxiliary columns", "auxiliary_columns"),
        ("Sparse nonzeros", "nonzeros"),
        ("Completed source work", "completed_work"),
        ("Seed depth", "seed_depth"),
        ("Checkpoint generation", "checkpoint.generation"),
        ("Artifact", "artifact"),
        ("Source checkpoint", "source_checkpoint"),
        ("Source scope", "scope_binding"),
    ];
    for (label, name) in fields {
        if (name.starts_with("requested_scope.") && summary["requested_scope"].is_null())
            || (name.starts_with("collection.") && summary["collection"].is_null())
            || (name == "source_checkpoint" && summary[name].is_null())
            || (name.starts_with("inherited_equation_authority.")
                && summary["inherited_equation_authority"].is_null())
        {
            continue;
        }
        let label = if name == "collection.global_alias_classes"
            && summary["collection"]["proof_layers"].as_u64().unwrap_or(0) > 1
        {
            "Historical alias classes"
        } else if summary["artifact"].is_null() && label.starts_with("Published") {
            if label == "Published R cap" {
                "Inventoried R cap"
            } else {
                "Inventoried D cap"
            }
        } else {
            label
        };
        let value = field(summary, name);
        // Wrap by Unicode scalars: report fields are ASCII IDs, numeric values
        // and file paths, never mathematical pretty-printed expressions.
        let chunks: Vec<String> = value
            .chars()
            .collect::<Vec<_>>()
            .chunks(value_width.max(1))
            .map(|chunk| chunk.iter().collect())
            .collect();
        for (index, chunk) in chunks.into_iter().enumerate() {
            let label = if index == 0 {
                clean(label, label_width)
            } else {
                String::new()
            };
            builder.push_record([
                label,
                paint(
                    &chunk,
                    color,
                    if name == "remaining_terminals" || name == "eliminated_terminals" {
                        "32"
                    } else {
                        "37"
                    },
                ),
            ]);
        }
    }
    let mut table = builder.build();
    table.with(Style::modern_rounded());
    format!(
        "{}\n{table}\n{}\n{}\n",
        paint("RustRed · Saved campaign artifact", color, "1;36"),
        paint(
            "Normalized terminals are candidate masters, not an independence claim.",
            color,
            "33"
        ),
        "Symbolic package only: numerical master values and routed coefficient application are not included."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_is_explicit_bounded_and_colors_are_optional() {
        let summary = serde_json::json!({
            "status":"completed_nonminimal", "raw_terminals":116,
            "remaining_terminals":49, "artifact":"/scratch/artifact\u{001b}[2J",
        });
        let plain = render_master_table(&summary, false, 80);
        assert!(plain.contains("49"));
        assert!(plain.contains("nonminimal"));
        assert!(plain.contains("not reported"));
        assert!(plain.contains("conservative-v1 (legacy)"));
        assert!(!plain.contains('\u{001b}'));
        assert!(render_master_table(&summary, true, 80).contains("\x1b[1;36m"));
    }

    #[test]
    fn earlier_publication_keeps_published_and_requested_caps_distinct() {
        let summary = serde_json::json!({
            "status":"published_unrefined", "refinement_status":"not requested",
            "artifact":"/artifact", "scope":{"max_starting_rank":0,"max_starting_d":9},
            "requested_scope":{"max_starting_rank":2,"max_starting_d":10},
            "scope_selection":"earlier publication; new request not published"
        });
        let table = render_master_table(&summary, false, 110);
        for text in [
            "Published R cap",
            "Requested R cap",
            "not requested",
            "earlier publication",
        ] {
            assert!(table.contains(text));
        }
        assert!(!table.contains("Inventoried R cap"));
    }
}
