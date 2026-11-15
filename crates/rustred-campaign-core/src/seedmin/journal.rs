//! RESUME: a journal of trial answers keyed by the trial digest. A restarted minimisation issues the same trials in the
//! same order (decisions depend only on answers) and gets the journaled answers: the same record. Pure: the journal is
//! a list of text lines the caller persists (the supervisor appends them to `journal.jsonl`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{EngineCaps, Trial, TrialAnswer, TrialSolver};

/// One journal line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalLine {
    pub digest: String,
    pub label: String,
    pub answers: Vec<TrialAnswer>,
}

/// A solver that answers known trials from the journal and passes the others to `inner`.
pub struct JournaledSolver<S> {
    pub inner: S,
    map: BTreeMap<String, Vec<TrialAnswer>>,
    new_lines: Vec<String>,
    /// Trials answered from the journal / by the inner solver.
    pub hits: u64,
    pub misses: u64,
}

impl<S: TrialSolver> JournaledSolver<S> {
    /// From existing journal lines (unreadable lines are an error: a journal is never guessed).
    pub fn from_lines<'a>(
        inner: S,
        lines: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, String> {
        let mut map = BTreeMap::new();
        for (i, l) in lines.into_iter().enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let j: JournalLine =
                serde_json::from_str(l).map_err(|e| format!("journal line {}: {e}", i + 1))?;
            map.insert(j.digest, j.answers);
        }
        Ok(JournaledSolver {
            inner,
            map,
            new_lines: vec![],
            hits: 0,
            misses: 0,
        })
    }

    pub fn new(inner: S) -> Self {
        JournaledSolver {
            inner,
            map: BTreeMap::new(),
            new_lines: vec![],
            hits: 0,
            misses: 0,
        }
    }

    /// Lines added since the last call (to append to the persisted journal).
    pub fn take_new_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.new_lines)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl<S: TrialSolver> TrialSolver for JournaledSolver<S> {
    fn solve(&mut self, trials: &[Trial]) -> Result<Vec<Vec<TrialAnswer>>, String> {
        let digests: Vec<String> = trials.iter().map(|t| t.digest()).collect();
        let mut todo: Vec<Trial> = vec![];
        let mut todo_d: Vec<String> = vec![];
        for (t, d) in trials.iter().zip(&digests) {
            if !self.map.contains_key(d) && !todo_d.contains(d) {
                todo.push(t.clone());
                todo_d.push(d.clone());
            }
        }
        self.hits += (trials.len() - todo.len()) as u64;
        self.misses += todo.len() as u64;
        if !todo.is_empty() {
            let ans = self.inner.solve(&todo)?;
            if ans.len() != todo.len() {
                return Err(format!("{} answers for {} trials", ans.len(), todo.len()));
            }
            for ((t, d), a) in todo.iter().zip(&todo_d).zip(ans) {
                let line = JournalLine {
                    digest: d.clone(),
                    label: t.label.clone(),
                    answers: a.clone(),
                };
                self.new_lines
                    .push(serde_json::to_string(&line).map_err(|e| e.to_string())?);
                self.map.insert(d.clone(), a);
            }
        }
        Ok(digests.iter().map(|d| self.map[d].clone()).collect())
    }

    fn capabilities(&self) -> EngineCaps {
        self.inner.capabilities()
    }
}
