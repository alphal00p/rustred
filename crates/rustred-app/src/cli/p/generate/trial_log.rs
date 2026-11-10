//! Compact opt-in diagnostics from the existing native progress callback.
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{FamilyCloseGenerationStage as Stage, FamilyCloseProgress};

const SCHEMA: &str = "rustred.p-generate-trials.v1";

pub(super) struct Owner<'a> {
    pub label: &'a str,
    pub ordinal: usize,
    pub presentation: u32,
    pub family: &'a str,
}

struct State<W> {
    writer: W,
    error: Option<String>,
    attempt: usize,
    cases: BTreeMap<(usize, u64), usize>,
}

pub(super) struct TrialLog<W = BufWriter<File>> {
    invocation: String,
    state: Mutex<State<W>>,
}

impl TrialLog {
    pub(super) fn open(path: &Path, force: bool) -> io::Result<Self> {
        let writer = OpenOptions::new()
            .write(true)
            .create(force)
            .create_new(!force)
            .truncate(force)
            .open(path)?;
        let invocation = format!(
            "p-generate-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos()
        );
        Ok(Self::new(BufWriter::new(writer), invocation))
    }
}

impl<W: Write> TrialLog<W> {
    fn new(writer: W, invocation: String) -> Self {
        Self {
            invocation,
            state: Mutex::new(State {
                writer,
                error: None,
                attempt: 0,
                cases: BTreeMap::new(),
            }),
        }
    }

    pub(super) fn begin_owner(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.attempt = 0;
        state.cases.clear();
    }

    pub(super) fn observe(&self, owner: &Owner<'_>, event: &FamilyCloseProgress) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.error.is_some() {
            return;
        }
        if matches!(event, FamilyCloseProgress::Preparing { .. }) {
            state.attempt += 1;
            state.cases.clear();
            return;
        }
        let FamilyCloseProgress::Generating {
            ordinal,
            sector,
            stage,
            elapsed,
        } = event
        else {
            return;
        };
        if !matches!(stage, Stage::Case { .. } | Stage::RuleTrialFinished { .. }) {
            return;
        }
        if state.attempt == 0 {
            state.error = Some("trial log received case work before preparation".into());
            return;
        }
        let key = (*ordinal, *sector);
        let case_ordinal = match stage {
            Stage::Case { .. } => {
                let case = state.cases.entry(key).or_default();
                *case += 1;
                *case
            }
            Stage::RuleTrialFinished { .. } => match state.cases.get(&key).copied() {
                Some(case) => case,
                None => {
                    state.error = Some("trial log received a trial before its case".into());
                    return;
                }
            },
            _ => unreachable!(),
        };
        let record = serde_json::json!({
            "schema": SCHEMA,
            "invocation": self.invocation,
            "owner": owner.label,
            "owner_ordinal": owner.ordinal,
            "presentation": owner.presentation,
            "family": owner.family,
            "attempt": state.attempt,
            "sector_ordinal": ordinal,
            "sector": sector,
            "case_ordinal": case_ordinal,
            "elapsed_seconds": elapsed.as_secs_f64(),
            "stage": stage,
        });
        let written = serde_json::to_writer(&mut state.writer, &record)
            .map_err(io::Error::other)
            .and_then(|()| state.writer.write_all(b"\n"));
        if let Err(error) = written {
            state.error = Some(error.to_string());
        }
    }

    /// Called even after generation fails, so partial measurements are flushed.
    pub(super) fn finish(&self) -> io::Result<()> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(error) = state.writer.flush() {
            state.error.get_or_insert_with(|| error.to_string());
        }
        match &state.error {
            Some(error) => Err(io::Error::other(error.clone())),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Clone, Default)]
    struct Bytes(Arc<Mutex<Vec<u8>>>);

    impl Write for Bytes {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn owner() -> Owner<'static> {
        Owner {
            label: "bebe",
            ordinal: 0,
            presentation: 42,
            family: "test-family",
        }
    }

    fn prepare(log: &TrialLog<impl Write>) {
        log.begin_owner();
        log.observe(
            &owner(),
            &FamilyCloseProgress::Preparing {
                arity: 14,
                elapsed: Duration::ZERO,
            },
        );
    }

    fn event(stage: Stage) -> FamilyCloseProgress {
        FamilyCloseProgress::Generating {
            ordinal: 2,
            sector: 7,
            stage,
            elapsed: Duration::from_micros(200),
        }
    }

    fn trial() -> Stage {
        Stage::RuleTrialFinished {
            trial: 1,
            outcome: "duplicate-candidate",
            search_seeds: 2,
            search_rows: 3,
            independent_rows: 4,
            exact_trace_rows: 5,
            exact_trace_terms: 6,
            exact_lifts: 1,
            guard_branches: 8,
            geometry_calls: 9,
            search_us: 100,
            exact_materialization_us: 70,
            guard_extraction_us: 11,
            geometry_us: 12,
            trial_elapsed_us: 123,
            case_elapsed_us: 150,
            score: Some([17; 14]),
            target_matches_baseline: Some(true),
            duplicate_of_trial: Some(0),
            seed_first_depth: Some(1),
            seed_last_depth: Some(2),
        }
    }

    #[test]
    fn metrics_hook_streams_native_trial_fields_with_case_identity() {
        let bytes = Bytes::default();
        let log = TrialLog::new(bytes.clone(), "test-invocation".into());
        prepare(&log);
        log.observe(&owner(), &event(Stage::Case { pending: 3 }));
        log.observe(&owner(), &event(trial()));
        log.observe(
            &owner(),
            &event(Stage::Discovery {
                depth: 1,
                seeds: 2,
                rows: 3,
            }),
        );
        log.finish().unwrap();
        let raw = bytes.0.lock().unwrap();
        let records: Vec<serde_json::Value> = std::str::from_utf8(&raw)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(records.len(), 2);
        let record = &records[1];
        assert_eq!(record["schema"], SCHEMA);
        assert_eq!(record["invocation"], "test-invocation");
        assert_eq!(record["owner"], "bebe");
        assert_eq!(record["owner_ordinal"], 0);
        assert_eq!(record["presentation"], 42);
        assert_eq!(record["sector_ordinal"], 2);
        assert_eq!(record["sector"], 7);
        assert_eq!(record["attempt"], 1);
        assert_eq!(record["case_ordinal"], 1);
        assert_eq!(record["stage"], serde_json::to_value(trial()).unwrap());
    }

    #[test]
    fn metrics_hook_resets_cases_between_attempts_and_owners() {
        let bytes = Bytes::default();
        let log = TrialLog::new(bytes.clone(), "test".into());
        prepare(&log);
        log.observe(&owner(), &event(Stage::Case { pending: 0 }));
        log.observe(
            &owner(),
            &FamilyCloseProgress::Preparing {
                arity: 14,
                elapsed: Duration::ZERO,
            },
        );
        log.observe(&owner(), &event(Stage::Case { pending: 0 }));
        log.observe(&owner(), &event(trial()));
        prepare(&log);
        log.observe(&owner(), &event(Stage::Case { pending: 0 }));
        log.observe(&owner(), &event(trial()));
        log.finish().unwrap();
        let raw = bytes.0.lock().unwrap();
        let records: Vec<serde_json::Value> = std::str::from_utf8(&raw)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(records[2]["attempt"], 2);
        assert_eq!(records[2]["case_ordinal"], 1);
        assert_eq!(records[4]["attempt"], 1);
        assert_eq!(records[4]["case_ordinal"], 1);
    }

    struct Broken {
        flush_only: bool,
    }

    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.flush_only {
                Ok(bytes.len())
            } else {
                Err(io::Error::other("test write failure"))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.flush_only {
                Err(io::Error::other("test flush failure"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn metrics_hook_reports_write_and_flush_failures() {
        for flush_only in [false, true] {
            let log = TrialLog::new(Broken { flush_only }, "test".into());
            prepare(&log);
            log.observe(&owner(), &event(Stage::Case { pending: 0 }));
            let error = log.finish().unwrap_err().to_string();
            assert!(error.contains(if flush_only {
                "test flush failure"
            } else {
                "test write failure"
            }));
        }
    }

    #[test]
    fn metrics_hook_rejects_trial_without_case_identity() {
        let log = TrialLog::new(io::sink(), "test".into());
        prepare(&log);
        log.observe(&owner(), &event(trial()));
        assert!(
            log.finish()
                .unwrap_err()
                .to_string()
                .contains("before its case")
        );
    }
}
