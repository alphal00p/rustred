//! Optional observational stderr JSON. It never feeds an algebra/proof input.
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    io::Write,
    time::{Duration, Instant},
};

const ROW_CADENCE: usize = 64;
const MIN_INTERVAL: Duration = Duration::from_secs(1);

struct State {
    enabled: bool,
    started: Instant,
    last_heartbeat: Duration,
    last_visited: usize,
}
impl State {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            started: Instant::now(),
            last_heartbeat: Duration::ZERO,
            last_visited: 0,
        }
    }
    fn event(&self, name: &str, fields: impl FnOnce() -> Value) -> Option<Value> {
        if !self.enabled {
            return None;
        }
        Some(
            json!({"schema":"rustred.symbolic-progress.v1", "event":name,
            "elapsed_seconds":self.started.elapsed().as_secs_f64(),
            "observational_only":true, "fields":fields()}),
        )
    }
    fn row_due(&mut self, now: Duration, visited: usize) -> bool {
        if !self.enabled
            || visited.saturating_sub(self.last_visited) < ROW_CADENCE
            || now.saturating_sub(self.last_heartbeat) < MIN_INTERVAL
        {
            return false;
        }
        self.last_heartbeat = now;
        self.last_visited = visited;
        true
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::new(false));
}

fn write_event(value: Option<Value>) {
    if let Some(value) = value {
        // A broken diagnostic consumer must not change the mathematical run.
        let mut stderr = std::io::stderr().lock();
        if serde_json::to_writer(&mut stderr, &value).is_ok() {
            let _ = stderr.write_all(b"\n");
        }
    }
}

pub fn reset() {
    let enabled = std::env::var("RUSTRED_SYMBOLIC_PROJECTOR_PROGRESS").as_deref() == Ok("1");
    STATE.with(|state| *state.borrow_mut() = State::new(enabled));
}

pub fn event(name: &str, fields: impl FnOnce() -> Value) {
    STATE.with(|state| write_event(state.borrow().event(name, fields)));
}

pub fn projection_start(rows: usize, forbidden: usize, width: usize) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.last_visited = 0;
        // Do not reset the heartbeat clock across fast successive projections:
        // row-heartbeat events remain at most once per second globally per run.
        write_event(state.event("projection_start", || {
            json!({"source_rows":rows,
            "f_size":forbidden,"augmented_columns":width})
        }));
    });
}

pub fn rows(visited: usize, forbidden: usize, u_nonzeros: usize, l_nonzeros: usize) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let now = state.started.elapsed();
        if state.row_due(now, visited) {
            write_event(state.event("projection_rows", || {
                json!({"rows_visited":visited,
                "f_size":forbidden,"u_nonzeros":u_nonzeros,"l_nonzeros":l_nonzeros})
            }));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_diagnostics_are_lazy_and_emit_nothing() {
        let mut state = State::new(false);
        assert!(
            state
                .event("forbidden", || panic!(
                    "disabled diagnostics evaluated payload"
                ))
                .is_none()
        );
        assert!(!state.row_due(Duration::from_secs(100), 10000));
    }
    #[test]
    fn row_heartbeat_is_sparse_and_at_most_once_per_second() {
        let mut state = State::new(true);
        assert!(!state.row_due(Duration::from_millis(999), 64));
        assert!(!state.row_due(Duration::from_secs(1), 63));
        assert!(state.row_due(Duration::from_secs(1), 64));
        assert!(!state.row_due(Duration::from_millis(1999), 128));
        assert!(state.row_due(Duration::from_secs(2), 128));
        state.last_visited = 0; // New projection does not bypass time throttle.
        assert!(!state.row_due(Duration::from_millis(2500), 64));
        assert!(state.row_due(Duration::from_secs(3), 64));
    }
    #[test]
    fn enabled_event_is_only_a_separate_observational_envelope() {
        let state = State::new(true);
        let value = state
            .event("projection_target", || json!({"rows_visited":7,"f_size":3}))
            .unwrap();
        assert_eq!(value["schema"], "rustred.symbolic-progress.v1");
        assert_eq!(value["observational_only"], true);
        assert_eq!(value["fields"]["rows_visited"], 7);
        assert_eq!(value["fields"]["f_size"], 3);
        assert!(value["elapsed_seconds"].as_f64().unwrap() >= 0.0);
    }
}
