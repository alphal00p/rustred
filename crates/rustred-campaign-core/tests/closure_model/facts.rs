//! Fact sources (ported from audit loop-B facts.rs): hidden truth and adversarial in-memory wrappers. The fact-FILE
//! modes of the audit are covered by tests/closure_audit_regressions.rs m4 (facts are bound to rule content + term key).
#![allow(dead_code)]
use super::reference::{Cf, RefFacts, Rf};
use super::spec::NodeSpec;
use rustred_campaign_core::closure::{CoefFact, FactSource, RegFact, RuleId};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub fn h(parts: &(impl Hash + ?Sized)) -> u64 {
    let mut s = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut s);
    s.finish()
}

/// Hidden truth of a node: zero chance per term from the spec (by table name and label).
pub struct Truth {
    pub seed: u64,
    pub zp: HashMap<(String, i64), Vec<u8>>,
    pub rec: HashMap<(String, i64), usize>,
}
impl Truth {
    pub fn new(sp: &NodeSpec) -> Truth {
        let mut zp = HashMap::new();
        let mut rec = HashMap::new();
        for t in &sp.tables {
            for (ri, r) in t.rules.iter().enumerate() {
                zp.insert((t.name.clone(), r.label), r.zp.clone());
                rec.insert((t.name.clone(), r.label), ri);
            }
        }
        Truth {
            seed: sp.fact_seed,
            zp,
            rec,
        }
    }
    /// truth by rule identity (name, label) -- record must match too
    pub fn c(&self, t: &str, rec: usize, label: i64, key: &[i32], term: usize) -> Cf {
        let k = (t.to_string(), label);
        match (self.zp.get(&k), self.rec.get(&k)) {
            (Some(z), Some(&r)) if r == rec && term < z.len() => {
                if h(&(self.seed, t, label, key, term)) % 100 < z[term] as u64 {
                    Cf::Zero
                } else {
                    Cf::NonZero
                }
            }
            _ => Cf::Unknown,
        }
    }
    pub fn r(&self, t: &str, rec: usize, label: i64, key: &[i32]) -> Rf {
        let k = (t.to_string(), label);
        match self.rec.get(&k) {
            Some(&r) if r == rec => {
                if h(&(self.seed, t, label, key, 999usize)) % 100 < 3 {
                    Rf::Pole
                } else {
                    Rf::Regular
                }
            }
            _ => Rf::Unknown,
        }
    }
}

/// Adversarial modes over the truth (in-memory).
pub struct Adv {
    pub truth: Truth,
    pub mode: u32,
}
impl Adv {
    fn cf(&self, t: &str, rec: usize, label: i64, key: &[i32], term: usize) -> Cf {
        let v = self.truth.c(t, rec, label, key, term);
        let r = h(&(self.truth.seed ^ 77, t, label, key, term)) % 100;
        match self.mode {
            1 if r < 25 => Cf::Unknown,
            3 if v == Cf::Zero && r < 25 => Cf::NonZero,
            5 => Cf::Unknown,
            _ => v,
        }
    }
    fn rf(&self, t: &str, rec: usize, label: i64, key: &[i32]) -> Rf {
        let v = self.truth.r(t, rec, label, key);
        let r = h(&(self.truth.seed ^ 78, t, label, key)) % 100;
        match self.mode {
            2 if r < 25 => Rf::Unknown,
            5 => Rf::Unknown,
            _ => v,
        }
    }
}
impl RefFacts for Adv {
    fn coef(&self, t: &str, rec: usize, label: i64, key: &[i32], term: usize) -> Cf {
        self.cf(t, rec, label, key, term)
    }
    fn reg(&self, t: &str, rec: usize, label: i64, key: &[i32]) -> Rf {
        self.rf(t, rec, label, key)
    }
}
impl FactSource for Adv {
    fn coefficient(&self, rule: &RuleId, key: &[i32], term: usize, _term_key: &[i32]) -> CoefFact {
        match self.cf(rule.table, rule.record, rule.label, key, term) {
            Cf::Zero => CoefFact::Zero,
            Cf::NonZero => CoefFact::NonZero,
            Cf::Unknown => CoefFact::Unknown,
        }
    }
    fn regularity(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        match self.rf(rule.table, rule.record, rule.label, key) {
            Rf::Regular => RegFact::Regular,
            Rf::Pole => RegFact::Pole,
            Rf::Unknown => RegFact::Unknown,
        }
    }
    fn label(&self) -> String {
        format!("adv{}", self.mode)
    }
}
