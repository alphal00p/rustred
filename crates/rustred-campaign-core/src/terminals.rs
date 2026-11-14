//! Terminals (plan 2.4.9 as amended by the user's ruling U-9 / DEV-4; R17, U2): terminal proposals only after the
//! FULL ladder of a point failed (a [`LadderExhausted`] token, which only the ladder makes), the per-node FALLBACK
//! terminal set with its bound (spec, default 100), removal of keys an accepted rule covers.
//!
//! U-9 (BINDING): master counts, knock-out flags and published master lists are NOT inputs. A supplied master count
//! is an OPTIONAL comparison column (`reference: N` beside `campaign: M`): [`compare_with_reference`] records the
//! difference as information, never as a failure or a trigger of search. What protects against declaring a
//! non-master terminal is the exhausted ladder (full comparator, depth 3, unfloored, children) plus the bound.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::repair::{Coverage, CoverageVerdict};
use crate::worklist::items::{Key, MasterCounts, PreferredTerminals};
use crate::worklist::ladder::LadderExhausted;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalError {
    /// Adding the key would exceed `terminals.max` (attention `terminals_over_cap`).
    OverBound { bound: usize },
    /// The token names another key.
    TokenMismatch,
    /// The ladder did not try a depth-3 unfloored setting for this key (2.4.4 (7): never given up before depth 3).
    NotDeepTried,
}

/// The fallback terminal set F of one node: explicit keys from the spec, and keys added by rule after their full
/// ladder failed. It only SHRINKS under accepted rules (a key re-enters only if its covering rule is removed).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FallbackSet {
    bound: usize,
    keys: BTreeSet<Key>,
    /// Keys from the spec (explicit) vs added by rule.
    explicit: BTreeSet<Key>,
}

impl FallbackSet {
    pub fn new(bound: usize) -> Self {
        FallbackSet {
            bound,
            keys: BTreeSet::new(),
            explicit: BTreeSet::new(),
        }
    }

    /// Spec keys (`terminals.fallback`): refused above the bound.
    pub fn with_explicit(bound: usize, keys: Vec<Key>) -> Result<Self, TerminalError> {
        let mut f = FallbackSet::new(bound);
        for k in keys {
            if f.keys.len() >= bound && !f.keys.contains(&k) {
                return Err(TerminalError::OverBound { bound });
            }
            f.explicit.insert(k.clone());
            f.keys.insert(k);
        }
        Ok(f)
    }

    /// Add a key BY RULE: only with the ladder's exhaustion token for that key (depth 3 unfloored tried), and only up
    /// to the bound.
    pub fn add_by_rule(&mut self, key: &[i32], token: &LadderExhausted) -> Result<bool, TerminalError> {
        if token.key() != key {
            return Err(TerminalError::TokenMismatch);
        }
        if !token.depth3_unfloored_tried() {
            return Err(TerminalError::NotDeepTried);
        }
        if self.keys.contains(key) {
            return Ok(false);
        }
        if self.keys.len() >= self.bound {
            return Err(TerminalError::OverBound { bound: self.bound });
        }
        self.keys.insert(key.to_vec());
        Ok(true)
    }

    /// Remove every key that the tables in force now cover (`terminal_removed {reason: covered_by_rule}`); returns
    /// the removed keys with the covering rule.
    pub fn remove_covered(&mut self, tables: &dyn Coverage) -> Vec<(Key, String)> {
        let mut out = Vec::new();
        let keys: Vec<Key> = self.keys.iter().cloned().collect();
        for k in keys {
            if let CoverageVerdict::Covered { rule } = tables.verdict(&k) {
                self.keys.remove(&k);
                self.explicit.remove(&k);
                out.push((k, rule));
            }
        }
        out
    }

    /// Drop keys the closure no longer reaches (2.4.9 (e)).
    pub fn retain_reached(&mut self, reached: &BTreeSet<Key>) -> Vec<Key> {
        let gone: Vec<Key> = self.keys.iter().filter(|k| !reached.contains(*k)).cloned().collect();
        for k in &gone {
            self.keys.remove(k);
            self.explicit.remove(k);
        }
        gone
    }

    pub fn keys(&self) -> impl Iterator<Item = &Key> {
        self.keys.iter()
    }
    pub fn len(&self) -> usize {
        self.keys.len()
    }
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
    pub fn bound(&self) -> usize {
        self.bound
    }
    pub fn is_explicit(&self, k: &[i32]) -> bool {
        self.explicit.contains(k)
    }
}

/// Flags of a terminal set. Only `OverBound` and `CoveredTerminal` block acceptance (U-9); `NotDeepTried` blocks
/// the declaration of that key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalFlag {
    OverBound { count: usize, bound: usize },
    /// A declared terminal lies in an accepted rule's domain (export gate).
    CoveredTerminal { key: Key, rule: String },
    NotDeepTried { key: Key },
}

/// Checks of a declared terminal set: the bound, no terminal inside a rule's domain, every key's ladder exhausted
/// down to depth 3 unfloored. Preferred terminals (P-4) are exempt from the ladder condition, are kept inside a
/// rule's domain by a key exclusion (see [`preferred_exclusions`]) and count against the bound only when the spec says
/// so.
pub fn check_terminals(
    terminals: &[(Key, Option<LadderExhausted>)],
    bound: usize,
    tables: &dyn Coverage,
    preferred: &PreferredTerminals,
    lines: &[bool],
) -> Vec<TerminalFlag> {
    let mut flags = Vec::new();
    let counted = terminals
        .iter()
        .filter(|(k, _)| preferred.count_against_bound || !preferred.matches(k, lines))
        .count();
    if counted > bound {
        flags.push(TerminalFlag::OverBound { count: counted, bound });
    }
    for (k, tok) in terminals {
        if preferred.matches(k, lines) {
            continue;
        }
        if let CoverageVerdict::Covered { rule } = tables.verdict(k) {
            flags.push(TerminalFlag::CoveredTerminal { key: k.clone(), rule });
        }
        if let Some(t) = tok
            && !t.depth3_unfloored_tried()
        {
            flags.push(TerminalFlag::NotDeepTried { key: k.clone() });
        }
    }
    flags
}

/// A key-exclusion record `key_excluded {node, rule, key, reason: keeps_terminal}` (plan 2.4.7 D7 / 2.4.13): the
/// rule is not applied at a preferred terminal, so the terminal is kept (honoured by the export and the importer).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyExclusion {
    pub key: Key,
    pub rule: String,
    pub reason: String,
}

/// Exclusions needed to keep the REACHED preferred keys terminal where a rule's domain contains them.
pub fn preferred_exclusions(
    reached_preferred: &[Key],
    tables: &dyn Coverage,
) -> Vec<KeyExclusion> {
    reached_preferred
        .iter()
        .filter_map(|k| match tables.verdict(k) {
            CoverageVerdict::Covered { rule } => Some(KeyExclusion {
                key: k.clone(),
                rule,
                reason: "keeps_terminal".into(),
            }),
            _ => None,
        })
        .collect()
}

/// The preferred-terminal report of a node (P-4 (d), (e)).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreferredReport {
    /// Preferred keys the closure reached and that stay terminals (possibly by exclusion).
    pub used: Vec<Key>,
    /// Preferred keys of the closure kept by a key exclusion (subset of `used`).
    pub kept_by_exclusion: Vec<KeyExclusion>,
    /// Key entries the closure never reached (not needed).
    pub not_needed: Vec<Key>,
    /// Preferred keys that could not be kept, with the reason (e.g. the exclusion was refused by a later check).
    pub not_kept: Vec<(Key, String)>,
    /// Terminals that are NOT preferred, with why they are terminals.
    pub other_terminals: Vec<(Key, String)>,
    /// Malformed or inapplicable entries: warnings, never errors.
    pub warnings: Vec<String>,
}

/// Build the report from the declared terminals (with their ladder tokens), the keys the closure reached, the
/// exclusions written and the ones refused.
pub fn preferred_report(
    preferred: &PreferredTerminals,
    lines: &[bool],
    terminals: &[(Key, Option<LadderExhausted>)],
    reached: &BTreeSet<Key>,
    exclusions: &[KeyExclusion],
    refused: &[(Key, String)],
) -> PreferredReport {
    let mut r = PreferredReport {
        warnings: preferred.warnings(lines),
        ..Default::default()
    };
    for (k, tok) in terminals {
        if preferred.matches(k, lines) {
            r.used.push(k.clone());
        } else {
            let why = match tok {
                Some(t) => format!(
                    "ladder exhausted ({} attempts, depth 3 unfloored tried: {})",
                    t.attempts(),
                    t.depth3_unfloored_tried()
                ),
                None => "declared without a ladder token (spec fallback set)".into(),
            };
            r.other_terminals.push((k.clone(), why));
        }
    }
    r.kept_by_exclusion = exclusions.iter().filter(|e| preferred.matches(&e.key, lines)).cloned().collect();
    r.not_kept = refused.to_vec();
    for e in &preferred.entries {
        if let crate::worklist::items::PreferredEntry::Key(k) = e
            && !reached.contains(k)
        {
            r.not_needed.push(k.clone());
        }
    }
    r
}

/// Optional comparison column (U-9): never a failure, never a trigger.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceComparison {
    pub campaign: usize,
    pub reference_ibp: Option<u32>,
    pub reference_with_symmetry: Option<u32>,
    pub source: String,
    /// "own estimate" when the count was computed by the campaign itself (e.g. an IBP rank count through a trait).
    pub label: String,
    /// Information only.
    pub note: String,
}

pub fn compare_with_reference(campaign: usize, reference: &MasterCounts, label: &str) -> ReferenceComparison {
    let note = match reference.ibp {
        None => "no reference".to_string(),
        Some(n) if n as usize == campaign => "equal".into(),
        Some(n) => format!("campaign {campaign} vs reference {n} (information only, U-9)"),
    };
    ReferenceComparison {
        campaign,
        reference_ibp: reference.ibp,
        reference_with_symmetry: reference.with_symmetry,
        source: reference.source.clone(),
        label: label.to_string(),
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repair::NoTables;

    #[test]
    fn bound_and_token() {
        let mut f = FallbackSet::new(1);
        let t1 = LadderExhausted::for_tests(vec![1, 2], true);
        let t2 = LadderExhausted::for_tests(vec![2, 1], true);
        assert_eq!(f.add_by_rule(&[1, 2], &t1), Ok(true));
        assert_eq!(f.add_by_rule(&[2, 1], &t2), Err(TerminalError::OverBound { bound: 1 }));
        assert_eq!(f.add_by_rule(&[2, 1], &t1), Err(TerminalError::TokenMismatch));
        let shallow = LadderExhausted::for_tests(vec![3, 3], false);
        let mut g = FallbackSet::new(5);
        assert_eq!(g.add_by_rule(&[3, 3], &shallow), Err(TerminalError::NotDeepTried));
        let lines = vec![true, true];
        let none = PreferredTerminals::default();
        let flags = check_terminals(&[(vec![1, 2], None), (vec![2, 1], None)], 1, &NoTables, &none, &lines);
        assert_eq!(flags, vec![TerminalFlag::OverBound { count: 2, bound: 1 }]);
        // a preferred terminal does not count against the bound by default
        let pref = PreferredTerminals {
            entries: vec![crate::worklist::items::PreferredEntry::Key(vec![2, 1])],
            count_against_bound: false,
        };
        let flags = check_terminals(&[(vec![1, 2], None), (vec![2, 1], None)], 1, &NoTables, &pref, &lines);
        assert!(flags.is_empty());
    }
}
