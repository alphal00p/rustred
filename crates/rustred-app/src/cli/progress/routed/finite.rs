//! Finite scope progress comes only from the last native committed checkpoint.
//! Keep fixed-size summary fields in the existing latest-event slot; no cursor
//! scans or additional locks on observation or heartbeat.
use serde_json::{Value, json};

pub(super) fn retain(previous: &Value, event: &mut Value) {
    if event["operation"] != "finite_demand" {
        return;
    }
    if previous["operation"] == "finite_demand" {
        for field in [
            "finite_committed",
            "finite_generation",
            "finite_residency",
            "finite_local",
        ] {
            if let Some(value) = previous.get(field) {
                event[field] = value.clone();
            }
        }
    }
    match event["event"].as_str() {
        Some("finite_checkpoint_saved" | "finite_checkpoint_restored") => {
            // These counters were computed at durable boundaries by native
            // lifecycle code. Native scheduled/completed nodes cannot advance them.
            event["finite_committed"] = event["committed"].clone();
            event["finite_generation"] = json!({
                "number":event["generation_number"], "digest":event["generation"]});
            // The next cursor may designate a new window. Never display the
            // preceding window's local completion as that window's progress.
            event["finite_local"] = Value::Null;
        }
        Some("finite_backend_native") => {
            event["finite_local"] = json!({"row":event["row"],"window":event["window"],
                "snapshot":event.get("snapshot").unwrap_or(&event["native"])});
        }
        Some("finite_backend_resident_hydrated" | "finite_backend_resident_fallback") => {
            event["finite_residency"] = json!({"status":event["status"],
                "requested_bytes":event["requested_bytes"],"resident":event["resident"],
                "fallback":event["event"] == "finite_backend_resident_fallback"});
        }
        Some("finished") => {
            event["finite_local"] = Value::Null;
        }
        _ => {}
    }
}

pub(super) fn local_snapshot(event: &Value) -> Option<&Value> {
    if event["operation"] == "finite_demand" {
        Some(&event["finite_local"]["snapshot"])
    } else {
        None
    }
}

pub(super) fn annotate_heartbeat(record: &mut Value) {
    if record["progress"]["operation"] != "finite_demand" {
        return;
    }
    let committed = &record["progress"]["finite_committed"];
    record["required_scope_progress"] = if committed.is_object() {
        json!({"closed_rows":committed["exhausted_required_rows"],
            "total_rows":committed["required_rows"],
            "required_complete":committed["required_complete"],
            "from_native_committed_checkpoint":true})
    } else {
        Value::Null
    };
    record["expanded_nodes_scope"] = json!("current_window_only_not_input_closure");
}

pub(super) fn dashboard(record: &Value) -> [String; 6] {
    let event = &record["progress"];
    let committed = &event["finite_committed"];
    let local = &event["finite_local"];
    let snapshot = &local["snapshot"];
    let n = |key| snapshot[key].as_u64().unwrap_or(0);
    let status = event["result"]["status"]
        .as_str()
        .or(event["status"].as_str())
        .or(event["event"].as_str())
        .unwrap_or("preparing");
    let required_line = match (
        committed["exhausted_required_rows"].as_u64(),
        committed["required_rows"].as_u64(),
    ) {
        (Some(done), Some(total)) => {
            let filled = if total == 0 {
                usize::from(committed["required_complete"] == true) * 20
            } else {
                ((done.min(total) as u128 * 20) / total as u128) as usize
            };
            format!(
                "[{}{}] {done}/{total} required rows closed in committed scope",
                "#".repeat(filled),
                "-".repeat(20 - filled)
            )
        }
        _ => "[--------------------] Required scope awaiting native checkpoint".into(),
    };
    let index = |value: &Value| value.as_u64().map_or_else(|| "—".into(), |n| n.to_string());
    let row = local.get("row").unwrap_or(&committed["current_row"]);
    let window = local.get("window").unwrap_or(&committed["current_window"]);
    let generation = index(&event["finite_generation"]["number"]);
    let residency = match event["finite_residency"]["status"].as_str() {
        Some("active") => "resident Work active",
        Some("disabled") => "paged Work (resident disabled)",
        Some("declined") if event["finite_residency"]["fallback"] == true => {
            "paged Work (resident grant exhausted)"
        }
        Some("declined") => "paged Work (resident grant declined)",
        _ => "Work residency pending",
    };
    let failure = snapshot["first_failure"]["detail"]
        .as_str()
        .or(event["result"]["error"].as_str());
    let last = if let Some(detail) = failure {
        let detail: String = detail
            .chars()
            .filter(|c| !c.is_control())
            .take(180)
            .collect();
        format!("Failure: {detail}; committed scope above is retained")
    } else {
        format!(
            "Last update {:.1}s ago; local nodes are not completed inputs; no family closure claim",
            record["progress_age_seconds"].as_f64().unwrap_or(0.)
        )
    };
    [
        format!("RustRed finite-demand campaign — {status}"),
        required_line,
        format!(
            "Row {row} / window {window} (zero-based): local nodes {}/{} expanded/discovered; {} queued",
            n("completed_nodes"),
            n("scheduled_nodes"),
            n("queued_nodes"),
            row = index(row),
            window = index(window)
        ),
        format!(
            "Committed Work {}  input occurrences {}  generation {generation}\nAuxiliary rows {}/{} closed; required complete {}\n{residency}",
            index(&committed["completed_work"]),
            index(&committed["input_occurrences"]),
            index(&committed["exhausted_auxiliary_rows"]),
            index(&committed["auxiliary_rows"]),
            committed["required_complete"]
                .as_bool()
                .map_or("unknown", |value| if value { "yes" } else { "no" })
        ),
        format!(
            "Native {}/{} active (not CPU utilization)  elapsed {:.1}s  RSS {:.3} GB  cancel {}",
            n("active_nodes"),
            n("workers"),
            record["elapsed_seconds"].as_f64().unwrap_or(0.),
            record["process_rss_bytes"].as_u64().unwrap_or(0) as f64 / 1e9,
            record["cancel_requested"]
        ),
        last,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint(generation: u64, done: u64) -> Value {
        json!({"operation":"finite_demand","event":"finite_checkpoint_saved",
            "generation_number":generation,"generation":"digest",
            "committed":{"required_rows":10,"exhausted_required_rows":done,
                "required_complete":false,"auxiliary_rows":2,"exhausted_auxiliary_rows":1,
                "current_row":done,"current_window":0,"completed_work":generation*100,
                "input_occurrences":generation*3}})
    }
    fn observe(previous: &mut Value, mut event: Value) {
        retain(previous, &mut event);
        *previous = event;
    }

    #[test]
    fn finite_committed_scope_survives_native_windows_without_claiming_input_completion() {
        let mut latest = json!({});
        observe(&mut latest, checkpoint(2, 1));
        for window in [0, 1] {
            observe(
                &mut latest,
                json!({"operation":"finite_demand","event":"finite_backend_native",
                "row":1,"window":window,"snapshot":{"scheduled_nodes":100,"completed_nodes":100,
                    "active_nodes":0,"workers":4}}),
            );
            assert_eq!(latest["finite_committed"]["exhausted_required_rows"], 1);
            assert_eq!(latest["finite_committed"]["input_occurrences"], 6);
            assert_eq!(latest["finite_generation"]["number"], 2);
            let mut heartbeat = json!({"progress":latest});
            annotate_heartbeat(&mut heartbeat);
            assert_eq!(heartbeat["required_scope_progress"]["closed_rows"], 1);
            assert!(dashboard(&heartbeat)[1].contains("1/10 required rows closed"));
            assert!(dashboard(&heartbeat)[2].contains("local nodes 100/100"));
            assert!(dashboard(&heartbeat)[5].contains("not completed inputs"));
        }
        observe(&mut latest, checkpoint(3, 2));
        assert!(latest["finite_local"].is_null());
        assert_eq!(latest["finite_committed"]["input_occurrences"], 9);
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finite_backend_native",
            "row":2,"window":0,"snapshot":{"scheduled_nodes":7,"completed_nodes":1}}),
        );
        assert_eq!(local_snapshot(&latest).unwrap()["completed_nodes"], 1);
        assert_eq!(latest["finite_committed"]["completed_work"], 300);
    }

    #[test]
    fn finite_restore_residency_and_failure_remain_visible_without_changing_legacy_events() {
        let mut latest = json!({});
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finite_backend_resident_hydrated",
            "status":"active","requested_bytes":16000000000u64,"resident":{"records":200}}),
        );
        let mut restored = checkpoint(5, 3);
        restored["event"] = json!("finite_checkpoint_restored");
        observe(&mut latest, restored);
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finite_backend_native",
            "row":3,"window":0,"snapshot":{"scheduled_nodes":9,"completed_nodes":9}}),
        );
        assert!(dashboard(&json!({"progress":latest}))[3].contains("resident Work active"));
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finite_backend_resident_fallback",
            "status":"declined","requested_bytes":16000000000u64}),
        );
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finished",
            "result":{"status":"cancelled","complete":false,"error":"cancelled"}}),
        );
        let lines = dashboard(&json!({"progress":latest}));
        assert!(lines[0].contains("cancelled"));
        assert!(lines[1].contains("3/10 required rows closed"));
        assert!(lines[3].contains("generation 5"));
        assert!(lines[3].contains("resident grant exhausted"));
        assert!(lines[5].contains("committed scope above is retained"));
        let legacy = json!({"operation":"owner_domain_walk","completed_nodes":7});
        let mut unchanged = legacy.clone();
        retain(&latest, &mut unchanged);
        assert_eq!(unchanged, legacy);
        let mut heartbeat = json!({"progress":legacy});
        let before = heartbeat.clone();
        annotate_heartbeat(&mut heartbeat);
        assert_eq!(heartbeat, before);
    }
}
