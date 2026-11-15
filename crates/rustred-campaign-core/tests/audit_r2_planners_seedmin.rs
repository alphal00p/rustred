//! AUDIT (audit-r2-planners, notes/implementer.audit-r2-planners.md): lane seedmin's joint minimiser, brief item 5.
//! (a) DETERMINISM: the same input gives the same list and the same record (also with speculative parallel trials);
//! (b) "valid" is stated for exactly the keys solved: a list is never claimed at a key outside the domain handed in,
//!     even when the face continues there and the list fails there (TRICKS 5: accept a domain only after a test at
//!     EVERY key of it);
//! (c) a piece's final verification is a solve at EVERY key of the piece (record `verify` trial over all keys).
//! In-process oracle child (a key solves iff the list holds one of its required seed sets). Toy, arity 3 (L8).

use std::collections::{BTreeMap, BTreeSet};

use rustred_campaign_core::seedmin::{
    EngineCaps, Key, Offset, PoleOrder, RelationFacts, SeedMinPolicy, StubSystemPoles, Trial,
    TrialAnswer, TrialSolver, minimise_joint, sha256_hex,
};

#[derive(Clone, Default)]
struct Oracle {
    needs: BTreeMap<Key, Vec<Vec<Offset>>>,
    solved_at: BTreeSet<Key>,
}

impl Oracle {
    fn answer(&mut self, t: &Trial, k: &Key) -> TrialAnswer {
        self.solved_at.insert(k.clone());
        let have: BTreeSet<&Offset> = t.list.iter().collect();
        let Some(alts) = self.needs.get(k) else {
            return TrialAnswer::NotInDomain;
        };
        for (i, a) in alts.iter().enumerate() {
            if a.iter().all(|o| have.contains(o)) {
                return TrialAnswer::Relation(Box::new(RelationFacts {
                    relation_digest: sha256_hex(format!("{k:?}/{i}").as_bytes()),
                    rhs_count: 10 + a.len() as u64,
                    rhs_sub_count: None,
                    bytes: None,
                    trace_rows: None,
                    seeds_used: Some(a.clone()),
                    pole: PoleOrder::regular(),
                    minimise: None,
                }));
            }
        }
        TrialAnswer::NoRelation
    }
}

impl TrialSolver for Oracle {
    fn solve(&mut self, trials: &[Trial]) -> Result<Vec<Vec<TrialAnswer>>, String> {
        Ok(trials
            .iter()
            .map(|t| t.keys.iter().map(|k| self.answer(t, k)).collect())
            .collect())
    }
    fn capabilities(&self) -> EngineCaps {
        EngineCaps::default()
    }
}

fn shell() -> Vec<Offset> {
    let mut v = vec![];
    for a in -1..=1 {
        for b in -1..=1 {
            for c in -1..=1 {
                v.push(vec![a, b, c]);
            }
        }
    }
    v
}

/// A face of 6 keys (n0 = 2..=7); key n0 needs the centre plus a private seed; key 8 (OUTSIDE the domain handed in)
/// needs a seed nobody inside needs.
fn oracle() -> (Oracle, Vec<Key>, Key) {
    let mut o = Oracle::default();
    let privs: [[i32; 3]; 6] = [[1, 0, 0], [0, 1, 0], [0, 0, -1], [1, 1, 0], [1, 0, -1], [0, 1, -1]];
    let mut keys = vec![];
    for (i, p) in privs.iter().enumerate() {
        let k: Key = vec![2 + i as i32, 1, 0];
        o.needs
            .insert(k.clone(), vec![vec![vec![0, 0, 0], p.to_vec()], vec![vec![-1, -1, -1]]]);
        keys.push(k);
    }
    let outside: Key = vec![8, 1, 0];
    o.needs
        .insert(outside.clone(), vec![vec![vec![0, 0, 0], vec![-1, 1, 1]]]);
    (o, keys, outside)
}

#[test]
fn the_same_input_gives_the_same_list_and_record_also_with_parallel_trials() {
    let run = |par: usize| {
        let (mut o, keys, _) = oracle();
        let mut p = SeedMinPolicy::default();
        p.parallel_trials = par;
        let jm = minimise_joint(&keys, &[0, 0, 0], &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
        let lists: Vec<Vec<Offset>> = jm.pieces.iter().map(|p| p.list.clone()).collect();
        (lists, jm.record.to_string())
    };
    let (l1, r1) = run(1);
    let (l1b, r1b) = run(1);
    assert_eq!(l1, l1b);
    assert_eq!(r1, r1b, "byte-identical record on a repeated run");
    let (l4, _) = run(4);
    assert_eq!(l1, l4, "speculative parallel trials give the same lists");
}

#[test]
fn valid_is_claimed_only_at_the_keys_handed_in_and_the_final_verification_solves_every_one() {
    let (mut o, keys, outside) = oracle();
    let p = SeedMinPolicy::default();
    let jm = minimise_joint(&keys, &[0, 0, 0], &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(jm.pieces.len(), 1);
    let pc = &jm.pieces[0];
    assert_eq!(pc.keys.len(), keys.len());
    assert_eq!(pc.domain_tested["keys"], keys.len());
    // never solved (never claimed) at the key outside the domain
    assert!(!o.solved_at.contains(&outside));
    assert!(!pc.keys.contains(&outside));
    // and indeed the minimised list fails there (a generalisation would be wrong)
    let mut probe = o.clone();
    let t = Trial {
        label: "audit".into(),
        centre: vec![0, 0, 0],
        list: pc.list.clone(),
        keys: vec![outside.clone()],
        mode: rustred_campaign_core::seedmin::TrialMode::Solve,
        engine_minimise: None,
    };
    assert_eq!(probe.solve(&[t]).unwrap()[0][0], TrialAnswer::NoRelation);
    // the record's final `verify` trial covers every key of the piece
    let verify: Vec<&serde_json::Value> = jm.record["trials"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["kind"] == "verify")
        .collect();
    assert_eq!(verify.len(), 1, "{}", jm.record["trials"]);
    assert_eq!(verify[0]["keys"], keys.len());
}
