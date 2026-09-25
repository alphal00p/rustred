use super::{CliError, bad, checkpoint};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    pub schema: String,
    pub manifest: PathBuf,
    pub queries: PathBuf,
    pub owner_base: PathBuf,
    /// Zero/omitted means one owner per dynamically claimed queue entry.
    /// Positive values explicitly group owners into that many independent jobs.
    #[serde(default)]
    pub shards: usize,
    pub jobs: usize,
    pub workers_per_job: usize,
    pub total_workers: usize,
    pub cpus: Vec<usize>,
    pub max_memory_bytes: u64,
    #[serde(default)]
    pub host_reserve_bytes: u64,
    #[serde(default = "checkpoint_interval")]
    pub checkpoint_interval_seconds: u64,
    #[serde(default = "ordered")]
    pub publication_policy: String,
    #[serde(default)]
    pub native_options: Vec<String>,
}
fn checkpoint_interval() -> u64 {
    3600
}
fn ordered() -> String {
    "ordered".into()
}

impl Config {
    pub fn read(path: &Path) -> Result<Self, CliError> {
        let mut config: Self = checkpoint::read_json(path, 1024 * 1024)?;
        let parent = path.parent().unwrap_or(Path::new("."));
        for input in [
            &mut config.manifest,
            &mut config.queries,
            &mut config.owner_base,
        ] {
            *input = std::fs::canonicalize(parent.join(&*input))
                .map_err(|e| bad(format!("{}: {e}", input.display())))?;
        }
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), CliError> {
        if self.schema != "rustred.independent-root-config.v1" {
            return Err(bad("unsupported shard configuration schema"));
        }
        if self.jobs == 0
            || (self.shards != 0 && self.jobs > self.shards)
            || !(1..=64).contains(&self.workers_per_job)
            || self.total_workers == 0
            || self
                .jobs
                .checked_mul(self.workers_per_job)
                .is_none_or(|n| n > self.total_workers)
            || self.total_workers > self.cpus.len()
            || self.max_memory_bytes == 0
            || self.checkpoint_interval_seconds == 0
        {
            return Err(bad(
                "require positive jobs, shards zero/omitted for one owner each, 1..64 workers/job, jobs*workers/job <= total_workers <= distinct CPUs, positive RAM and checkpoint interval",
            ));
        }
        if self.cpus.iter().copied().collect::<BTreeSet<_>>().len() != self.cpus.len() {
            return Err(bad("CPU IDs must be distinct"));
        }
        if !matches!(self.publication_policy.as_str(), "ordered" | "ready") {
            return Err(bad("publication_policy must be ordered or ready"));
        }
        let mut options = self.native_options.iter();
        let mut seen = BTreeSet::new();
        while let Some(option) = options.next() {
            if !seen.insert(option) {
                return Err(bad(format!("duplicate native option {option}")));
            }
            match option.as_str() {
                "--route-domain-overcover"
                | "--reuse-initial-d-bands"
                | "--route-joint-source-support-pruning" => {}
                "--bounded-refinement-axes"
                | "--max-guard-univariate-degree"
                | "--transfer-unreserved-lookahead"
                | "--inspection-workers"
                | "--apply-subdivision-axis"
                | "--apply-subdivision-cut"
                | "--apply-cell-refinement-max-cardinality" => {
                    let value = options
                        .next()
                        .ok_or_else(|| bad(format!("{option} requires a value")))?;
                    if value.starts_with("--") {
                        return Err(bad(format!("{option} requires a value")));
                    }
                }
                _ => {
                    return Err(bad(format!(
                        "native option {option} is not a supported immutable search policy; input, output, work and time limits are supervisor-owned"
                    )));
                }
            }
        }
        // Reuse the native parser's policy compatibility checks without loading
        // any owner program or executing a search.
        let mut args = vec![
            "rustred".to_owned(),
            "owner-domain-match".to_owned(),
            "--manifest".to_owned(),
            "selection.json".to_owned(),
            "--queries".to_owned(),
            "queries.json".to_owned(),
            "--output".to_owned(),
            "result.json".to_owned(),
            "--follow-successors".to_owned(),
            "--unbounded-work".to_owned(),
            "--checkpoint".to_owned(),
            "checkpoint".to_owned(),
            "--workers".to_owned(),
            self.workers_per_job.to_string(),
            "--publication-policy".to_owned(),
            self.publication_policy.clone(),
        ];
        args.extend(self.native_options.iter().cloned());
        super::super::args::parse_args(args.into_iter().map(std::ffi::OsString::from))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        serde_json::from_value(serde_json::json!({"schema":"rustred.independent-root-config.v1","manifest":"m","queries":"q","owner_base":".","shards":2,"jobs":2,"workers_per_job":1,"total_workers":2,"cpus":[0,1],"max_memory_bytes":100})).unwrap()
    }
    #[test]
    fn one_worker_ordered_and_exact_total_are_valid() {
        config().validate().unwrap();
    }
    #[test]
    fn rejects_oversubscription_duplicates_and_policy_injection() {
        let mut c = config();
        c.total_workers = 1;
        assert!(c.validate().is_err());
        let mut c = config();
        c.cpus = vec![0, 0];
        assert!(c.validate().is_err());
        let mut c = config();
        c.native_options = vec!["--max-domains".into(), "2".into()];
        assert!(c.validate().is_err());
        let mut c = config();
        c.publication_policy = "ready".into();
        assert!(c.validate().is_err());
    }
}
