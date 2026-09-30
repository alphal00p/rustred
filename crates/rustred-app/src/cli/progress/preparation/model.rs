//! Read-only preparation observations. No field is a closure/completion proof.

use super::super::family_close::telemetry::{NativeSnapshot, finite_seconds};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const SCHEMA: &str = "rustred.preparation-progress.v1";
pub(super) const MAX_STATUS_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const STALE_SECONDS: f64 = 30.0;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Status {
    pub(super) schema: String,
    pub(super) run_id: String,
    pub(super) attempt_id: String,
    pub(super) snapshot_seq: u64,
    pub(super) phase: String,
    pub(super) state: String,
    pub(super) timestamp_unix_seconds: f64,
    pub(super) elapsed_seconds: f64,
    pub(super) resources: Resources,
    pub(super) jobs: Vec<Job>,
    pub(super) aggregate: Option<Aggregate>,
    pub(super) stop_reason: Option<String>,
    pub(super) family_closure_claim: bool,
    /// Preserve producer extensions for the JSON/web consumer; never interpret
    /// raw diagnostic text as counters or scheduling/completion authority.
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Resources {
    pub(super) jobs: u64,
    pub(super) workers_per_job: u64,
    pub(super) total_workers: u64,
    pub(super) slots: Vec<Vec<u64>>,
    pub(super) memory: Memory,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Memory {
    pub(super) requested_hard_memory_bytes: Option<u64>,
    pub(super) effective_hard_memory_bytes: Option<u64>,
    pub(super) effective_soft_memory_bytes: Option<u64>,
    pub(super) host_memory_reserve_bytes: Option<u64>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Aggregate {
    pub(super) sampled_rss_bytes: Option<u64>,
    pub(super) sampled_peak_tree_rss_bytes: Option<u64>,
    pub(super) observed_cores: Option<f64>,
    pub(super) host_available_bytes: Option<u64>,
    pub(super) active_jobs: Option<u64>,
    pub(super) pending_jobs: Option<u64>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Job {
    pub(super) id: String,
    pub(super) state: String,
    pub(super) metadata: Metadata,
    pub(super) slot: Option<u64>,
    #[serde(default)]
    pub(super) cpus: Vec<u64>,
    pub(super) workers: Option<u64>,
    pub(super) pid: Option<u32>,
    pub(super) pid_start: Option<u64>,
    pub(super) elapsed_seconds: Option<f64>,
    pub(super) sampled_rss_bytes: Option<u64>,
    pub(super) observed_cores: Option<f64>,
    pub(super) checkpoint_files_observed: Option<u64>,
    pub(super) native_progress: Option<NativeObservation>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Metadata {
    pub(super) parent: u64,
    pub(super) root: String,
    pub(super) selected_sectors: Vec<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct NativeObservation {
    pub(super) status: String,
    pub(super) native_observed_at: Option<f64>,
    pub(super) age_seconds: Option<f64>,
    /// A malformed optional native payload must not hide the supervisor table.
    #[serde(default, deserialize_with = "deserialize_native")]
    pub(super) snapshot: Option<NativeSnapshot>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

fn deserialize_native<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<NativeSnapshot>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value::<NativeSnapshot>(value)
        .ok()
        .filter(|snapshot| snapshot.validate().is_ok()))
}

impl Status {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_STATUS_BYTES {
            return Err("preparation snapshot exceeds size limit".into());
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid preparation snapshot: {error}"))?;
        if value.schema != SCHEMA {
            return Err(format!("unsupported preparation schema {:?}", value.schema));
        }
        if value.family_closure_claim {
            return Err("preparation telemetry cannot assert family closure".into());
        }
        if !finite_seconds(value.timestamp_unix_seconds)
            || !finite_seconds(value.elapsed_seconds)
            || value.jobs.len() > 4096
            || value.run_id.len() > 4096
            || value.attempt_id.len() > 256
        {
            return Err("invalid preparation time, identity or job count".into());
        }
        let mut ids = BTreeSet::new();
        for job in &value.jobs {
            if job.id.len() > 256
                || !ids.insert(&job.id)
                || job.metadata.selected_sectors.len() > 65536
                || job.metadata.root.len() > 64
                || job.cpus.len() > 65536
                || job.elapsed_seconds.is_some_and(|n| !finite_seconds(n))
                || job.observed_cores.is_some_and(|n| !finite_seconds(n))
            {
                return Err("invalid preparation job observation".into());
            }
        }
        if value
            .aggregate
            .as_ref()
            .and_then(|a| a.observed_cores)
            .is_some_and(|n| !finite_seconds(n))
        {
            return Err("invalid aggregate CPU observation".into());
        }
        Ok(value)
    }

    pub(super) fn age(&self, now: f64) -> f64 {
        (now - self.timestamp_unix_seconds).max(0.0)
    }
    pub(super) fn terminal(&self) -> bool {
        matches!(self.state.as_str(), "completed" | "failed" | "stopped")
    }
    pub(super) fn failed(&self) -> bool {
        self.state == "failed" || self.jobs.iter().any(|job| job.state == "failed")
    }
    pub(super) fn completed_jobs(&self) -> usize {
        self.jobs
            .iter()
            .filter(|job| matches!(job.state.as_str(), "completed" | "retained"))
            .count()
    }

    /// A saved "fresh" flag cannot remain fresh while the supervisor is gone.
    /// Expiration only clears optional presentation data, never parent outcomes.
    pub(super) fn expire_native(&mut self, now: f64) {
        let age = self.age(now);
        for job in &mut self.jobs {
            if let Some(observation) = &mut job.native_progress {
                if observation.status == "fresh"
                    && observation.age_seconds.is_none_or(|native_age| {
                        !finite_seconds(native_age) || native_age + age > 90.0
                    })
                {
                    observation.status = "stale".into();
                    observation.snapshot = None;
                }
            }
        }
    }
}

impl Job {
    pub(super) fn native(&self) -> Option<&NativeSnapshot> {
        let observation = self.native_progress.as_ref()?;
        if observation.status != "fresh" {
            return None;
        }
        let snapshot = observation.snapshot.as_ref()?;
        if self.pid != Some(snapshot.pid)
            || self
                .pid_start
                .is_some_and(|start| Some(start) != snapshot.process_start_ticks)
        {
            return None;
        }
        Some(snapshot)
    }

    pub(super) fn active(&self) -> bool {
        matches!(self.state.as_str(), "running" | "draining")
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(crate) fn fixture() -> serde_json::Value {
        serde_json::json!({"schema":SCHEMA,"run_id":"run","attempt_id":"attempt","snapshot_seq":1,
            "phase":"generation","state":"running","timestamp_unix_seconds":100.0,"elapsed_seconds":2.0,
            "resources":{"jobs":1,"workers_per_job":2,"total_workers":2,"slots":[[0,1]],"memory":{}},
            "jobs":[{"id":"parent-3","state":"pending","metadata":{"parent":3,"root":"11","selected_sectors":["11"]}}],
            "aggregate":null,"stop_reason":null,"family_closure_claim":false})
    }

    #[test]
    fn unknown_native_data_remains_unknown_not_zero() {
        let mut input = fixture();
        input["jobs"][0]["native_progress"] =
            serde_json::json!({"status":"fresh","snapshot":{"schema":"future"}});
        let status = Status::parse(&serde_json::to_vec(&input).unwrap()).unwrap();
        assert!(status.jobs[0].native().is_none());
        assert!(status.jobs[0].checkpoint_files_observed.is_none());
        assert_eq!(status.completed_jobs(), 0);
    }

    #[test]
    fn rejects_authority_claim_and_duplicate_job_ids() {
        let mut input = fixture();
        input["family_closure_claim"] = true.into();
        assert!(Status::parse(&serde_json::to_vec(&input).unwrap()).is_err());
        input["family_closure_claim"] = false.into();
        let job = input["jobs"][0].clone();
        input["jobs"].as_array_mut().unwrap().push(job);
        assert!(Status::parse(&serde_json::to_vec(&input).unwrap()).is_err());
    }

    #[test]
    fn supervisor_heartbeat_cannot_refresh_an_old_native_snapshot() {
        let mut input = fixture();
        input["jobs"][0]["native_progress"] = serde_json::json!({
            "status":"fresh", "age_seconds":2.0, "snapshot":null});
        let mut status = Status::parse(&serde_json::to_vec(&input).unwrap()).unwrap();
        status.expire_native(189.0);
        assert_eq!(
            status.jobs[0].native_progress.as_ref().unwrap().status,
            "stale"
        );
        assert_eq!(status.jobs[0].state, "pending");
    }

    #[test]
    fn json_round_trip_preserves_diagnostics_without_interpreting_them() {
        let mut input = fixture();
        input["jobs"][0]["diagnostic_stderr_tail"] = "Generated 99999 sectors; CLOSED".into();
        input["checkpoint"] = serde_json::json!({"in_sector_resume":false});
        let status = Status::parse(&serde_json::to_vec(&input).unwrap()).unwrap();
        assert_eq!(status.completed_jobs(), 0);
        assert!(status.jobs[0].native().is_none());
        let output = serde_json::to_value(status).unwrap();
        assert_eq!(
            input["jobs"][0]["diagnostic_stderr_tail"],
            output["jobs"][0]["diagnostic_stderr_tail"]
        );
        assert_eq!(input["checkpoint"], output["checkpoint"]);
    }
}
