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
    match value {
        Value::Number(value) => value.to_string(),
        Value::String(value) => clean(value, 4096),
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
        paint(
            &clean("SYMBOLIC MASTER REDUCTION", value_width),
            color,
            "1;36",
        ),
    ]);
    let fields = [
        ("Status", "status"),
        ("Installed rule records", "inventory.installed.rules"),
        ("Observed cover rules", "inventory.encountered.rules"),
        ("Starting R cap", "scope.max_starting_rank"),
        ("Starting D cap", "scope.max_starting_d"),
        ("Query domains", "scope.starting_queries"),
        ("Current scope terminals", "inventory.encountered.terminals"),
        ("Retained source keys", "raw_terminals"),
        ("Normalized terminal keys", "normalized_terminals"),
        ("Remaining basis keys", "remaining_terminals"),
        ("Eliminated terminal keys", "eliminated_terminals"),
        ("Ordinary IBP rows", "relation_rows"),
        ("Independent sparse rows", "independent_rows"),
        ("Auxiliary columns", "auxiliary_columns"),
        ("Sparse nonzeros", "nonzeros"),
        ("Completed source work", "completed_work"),
        ("Seed depth", "seed_depth"),
        ("Checkpoint generation", "checkpoint.generation"),
        ("Artifact", "artifact"),
        ("Source scope", "scope_binding"),
    ];
    for (label, name) in fields {
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
        paint("RustRed · Master reduction", color, "1;36"),
        paint(
            "Exact bounded relation search; a nonminimal basis is allowed.",
            color,
            "33"
        ),
        "No numerical master values, independence proof or unrestricted family-closure claim."
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
        assert!(!plain.contains('\u{001b}'));
        assert!(render_master_table(&summary, true, 80).contains("\x1b[1;36m"));
    }
}
