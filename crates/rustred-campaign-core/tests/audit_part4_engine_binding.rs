//! Re-audit part 4 (lane audit-part4): the BINDING between a minted coverage proof and what the ENGINE dispatches.
//!
//! `economy::prove_coverage_preserved` proves "no dispatch gap" about the `CaseDomain`s it is given and cross-checks
//! them against the CORE dispatcher only. These tests build the domains the way the crate itself builds them from
//! table files (`record::read_table_file` + `cover::Table::from_table_file`, the path `assemble::dispatch_set` /
//! `tables_in_force` uses) and show where the engine's `Table::find_rule` (rustred-core node/mastermap.rs,
//! node/bounds.rs) reads the same file differently. The table files are written to `AUDIT_P4_OUT` (if set) so that the
//! frozen engine binary can be asked (`rr_node first-match`, inside a private pid namespace): see
//! notes/implementer.reaudit-part4.md for the commands and the engine's answers.
//!
//! Synthetic tables only (lane rule L8).
//!
//! INVERTED by lane partfix4 (notes/implementer.partfix4.md): the reader keeps `domain_bounds` and gives them the
//! engine's meaning (`domain::bound_branches`: exception branches at the rule point), the digest is v4, and a
//! selection bound on a target-pattern record is a refused precondition: B1 / B1b / B2 / B2b now assert the refusal
//! or the engine's dispatch (the engine answers recorded by the audit).

use std::path::{Path, PathBuf};

use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table};
use rustred_campaign_core::domain::DomainOptions;
use rustred_campaign_core::economy::{ProofWork, prove_coverage_preserved, table_set_digest};
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::partition::{DispatchOrderStatement, DispatcherSource};
use rustred_campaign_core::record::{ReadOptions, read_table_file};

fn out_dir(tag: &str) -> PathBuf {
    let base = std::env::var_os("AUDIT_P4_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("audit_p4_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(dir: &Path, name: &str, lines: &[String]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, lines.join("\n") + "\n").unwrap();
    p
}

fn header(mask: &str) -> String {
    format!(
        r#"{{"type":"header","schema":"table_dump.v1","family":"audit-p4","denominators":{},"mask":"{mask}"}}"#,
        mask.len()
    )
}

fn chart(mask: &str) -> String {
    let guards: Vec<String> = mask
        .chars()
        .map(|c| {
            if c == '1' {
                "\"n>=1\"".into()
            } else {
                "\"n<=0\"".into()
            }
        })
        .collect();
    format!(
        r#"{{"type":"chart","chart":"{mask}","guards":[{}]}}"#,
        guards.join(",")
    )
}

/// Load a table file as the crate's assembly does (`Table::from_table_file`, default options) into a dispatcher whose
/// chart is the frame mask.
fn load(p: &Path) -> (Dispatcher, NodeFrame) {
    let tf = read_table_file(p, ReadOptions::default()).unwrap();
    let frame = NodeFrame::from_header(tf.header.as_ref().unwrap()).unwrap();
    let t = Table::from_table_file("t", "audit-p4", &tf, DomainOptions::default()).unwrap();
    let mask = frame.mask().to_vec();
    let d = Dispatcher::new(vec![t], vec![0])
        .unwrap()
        .with_chart(mask)
        .unwrap();
    (d, frame)
}

/// B1 (engine binding): the crate's table reader DROPS a rule record's `domain_bounds` (record.rs `parse_line` has no
/// such field; `CaseDomain::from_rule_record` sets `bounds: []`), while the engine's `Table::load` turns them into
/// extra exception branches (node/bounds.rs `bound_exclusions`). Assembly builds the table sets in force (and checks
/// a FULL-node retirement's proof against them) through exactly this reader (`assemble::dispatch_set`). So a proof is
/// minted for a table set whose record A excludes `n0 = 1, 2` in the engine: the engine has NO rule at keys 1 and 2
/// of the new set (`rr_node first-match` answers `label: null`), the core dispatcher says `Covered` by A.
#[test]
fn p4_b1_domain_bounds_dropped_by_the_core_reader_proof_minted() {
    let dir = out_dir("b1");
    let mask = "1";
    let a = r#"{"type":"rule","chart":"1","index":0,"fixed":[null],"equations":[],"exceptions":[],"target":"I(n0)","rhs":[["I(n0-1)","1"]],"witness_loci":[],"dispatch":"Partition","domain_bounds":{"min":[3],"max":[null]}}"#;
    let b = r#"{"type":"rule","chart":"1","index":1,"fixed":[null],"equations":[],"exceptions":[],"target":"I(n0)","rhs":[["I(n0-1)","1"]],"witness_loci":[],"dispatch":"Partition"}"#;
    let old = write(
        &dir,
        "old.jsonl",
        &[header(mask), chart(mask), a.into(), b.into()],
    );
    let new = write(&dir, "new.jsonl", &[header(mask), chart(mask), a.into()]);
    let (dold, frame) = load(&old);
    let (dnew, _) = load(&new);
    // partfix4: the core's view of record A = the engine's: n0 = 1, 2 excluded (two exception branches)
    assert_eq!(dnew.tables()[0].domains[0].exceptions.len(), 2);
    let e = prove_coverage_preserved(
        &DispatcherSource::new(&dold),
        &DispatcherSource::new(&dnew),
        &frame,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
    )
    .expect_err("retiring B opens keys 1 and 2: no proof");
    eprintln!("B1 refused: {e}");
    // the engine's answers (audit): key 1 -> null, 2 -> null, 3 -> A, 1000 -> A; the core now says the same
    for (k, served) in [(1, false), (2, false), (3, true), (1000, true)] {
        assert_eq!(
            matches!(dnew.first_match(&[k]).unwrap(), Dispatch::Covered(_)),
            served,
            "core: key {k}"
        );
    }
    let _ = table_set_digest(&DispatcherSource::new(&dnew), &frame);
    // engine (find_rule with bound_exclusions): A is excluded at n0 = 1, 2 -> no rule at keys [1], [2] of `new`
    eprintln!(
        "B1 files: {} {} (ask: rr_node first-match --table <new> --key 1)",
        old.display(),
        new.display()
    );
}

/// B1b (TOCTOU of the digest): two table files that differ ONLY in a record's `domain_bounds` have the SAME table-set
/// digest v3 (the digest covers the core's domains, which do not carry the field), so a proof minted on the bound-free
/// version passes `Retirement::check_tables` for the bounded version (and vice versa).
#[test]
fn p4_b1b_digest_does_not_bind_domain_bounds() {
    let dir = out_dir("b1b");
    let mask = "11";
    let base = r#"{"type":"rule","chart":"11","index":0,"fixed":[null,null],"equations":[],"exceptions":[],"target":"I(n0,n1)","rhs":[["I(n0-1,n1)","1"]],"witness_loci":[]"#;
    let plain = format!("{base}}}");
    let bounded = format!(r#"{base},"domain_bounds":{{"min":[5,7],"max":[null,null]}}}}"#);
    let p1 = write(&dir, "plain.jsonl", &[header(mask), chart(mask), plain]);
    let p2 = write(&dir, "bounded.jsonl", &[header(mask), chart(mask), bounded]);
    let (d1, f1) = load(&p1);
    let (d2, f2) = load(&p2);
    assert_ne!(
        table_set_digest(&DispatcherSource::new(&d1), &f1),
        table_set_digest(&DispatcherSource::new(&d2), &f2),
        "partfix4: the digest (v4) sees domain_bounds"
    );
}

/// B2 (engine binding, "same record" claim of interfaces/partition.md 14.1): a selection bound `n0 - 2 >= 0` on the
/// rule point of a record with a TARGET PATTERN `I(n0+3)` (what `import::load` builds from `domain_bounds.min = [2]`,
/// import/load.rs ~1055-1095; it does not require a canonical target) passes `engine_readable_bound` (it checks the
/// rule-point slot kind on the frame mask), but the engine reads it as the exception branch `n0 - 1` only
/// (node/bounds.rs: a line `min m` excludes 1..m-1, assuming the rule point stays in the chart). At keys 1..=3 the rule
/// point is `-2..=0`: the core says NotSelected (bound violated) and dispatches to the catch-all S, the engine SELECTS R
/// (outside its bounds). Old set: three point records Q1..Q3 serve keys 1..=3; retiring them mints a proof although
/// the engine then applies R at keys 1..=3 instead of S (core) / Q (old).
#[test]
fn p4_b2_bound_with_a_target_pattern_engine_selects_where_the_core_does_not() {
    use rustred_campaign_core::cover::RuleRef;
    use rustred_campaign_core::domain::{CaseDomain, TargetMap, TargetSlot};
    use rustred_campaign_core::partition::DomainList;
    use rustred_campaign_core::poly::Poly;
    let n = 1;
    let mask = vec![true];
    let frame = NodeFrame::without_header_lines_first(1, 1);
    let mut r = CaseDomain::free(n, Some(mask.clone()));
    r.target = TargetMap::Pattern(vec![TargetSlot::Param {
        param: 0,
        offset: 3,
    }]);
    r.bounds = vec![Poly::parse_d_free_exact("n0-2", n).unwrap()];
    let s = CaseDomain::free(n, Some(mask.clone()));
    let q = |v: i32| {
        let mut d = CaseDomain::free(n, Some(mask.clone()));
        d.fixed = vec![Some(v)];
        d
    };
    let mut old = DomainList::new(n);
    for (i, d) in [q(1), q(2), q(3), r.clone(), s.clone()]
        .into_iter()
        .enumerate()
    {
        old.push(RuleRef::bare(0, i, i as i64), d);
    }
    let mut new = DomainList::new(n);
    new.push(RuleRef::bare(0, 0, 3), r.clone());
    new.push(RuleRef::bare(0, 1, 4), s.clone());
    let e = prove_coverage_preserved(
        &old,
        &new,
        &frame,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
    )
    .expect_err("partfix4: a selection bound on a target-pattern record is not engine-readable");
    assert!(e.contains("target pattern"), "{e}");
    // the core's verdicts of the GE domain: R NotSelected at keys 1..=3 (rule point -2..=0 below the bound), applies from key 5
    for k in 1..=4 {
        assert!(!r.applies(&[k]).applies(), "core: R does not apply at {k}");
    }
    assert!(r.applies(&[5]).applies());
    // the engine's table for the same records (R with domain_bounds and the pattern; S free)
    let dir = out_dir("b2");
    let rr = r#"{"type":"rule","chart":"1","index":3,"fixed":[null],"equations":[],"exceptions":[],"target":"I(n0+3)","rhs":[["I(n0+2)","1"]],"witness_loci":[],"dispatch":"Partition","domain_bounds":{"min":[2],"max":[null]}}"#;
    let ss = r#"{"type":"rule","chart":"1","index":4,"fixed":[null],"equations":[],"exceptions":[],"target":"I(n0)","rhs":[["I(n0-1)","1"]],"witness_loci":[],"dispatch":"Partition"}"#;
    let p = write(
        &dir,
        "new.jsonl",
        &[header("1"), chart("1"), rr.into(), ss.into()],
    );
    // partfix4: the record READ FROM THE FILE dispatches as the engine (audit's answers: label 3 (R) at keys 1..=3, 4
    // (S) at key 4, 3 from 5)
    let (d, _) = load(&p);
    let label = |k: i32| match d.first_match(&[k]).unwrap() {
        Dispatch::Covered(r) => r.label,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        (1..=6).map(label).collect::<Vec<_>>(),
        vec![3, 3, 3, 4, 3, 3]
    );
}

/// (b) eps-shifted frames are REFUSED before any proof: `NodeFrame::from_header` refuses a non-"0" power shift (as the
/// engine's `Table::load`), so no table, no dispatcher source and no frame of an eps family can reach the minting
/// function (the escape-hatch constructors carry no shifts at all).
#[test]
fn p4_b_eps_frames_are_refused_before_minting() {
    let dir = out_dir("eps");
    let h = r#"{"type":"header","schema":"table_dump.v1","family":"audit-p4","denominators":1,"mask":"1","power_shifts":["1"]}"#;
    let a = r#"{"type":"rule","chart":"1","index":0,"fixed":[null],"equations":[],"exceptions":[],"target":"I(n0)","rhs":[["I(n0-1)","1"]],"witness_loci":[]}"#;
    let p = write(&dir, "eps.jsonl", &[h.into(), chart("1"), a.into()]);
    let tf = read_table_file(&p, ReadOptions::default()).unwrap();
    assert!(NodeFrame::from_header(tf.header.as_ref().unwrap()).is_err());
    assert!(Table::from_table_file("t", "audit-p4", &tf, DomainOptions::default()).is_err());
    let h0 = r#"{"type":"header","schema":"table_dump.v1","family":"audit-p4","denominators":1,"mask":"1","power_shifts":["0"]}"#;
    let p0 = write(&dir, "eps0.jsonl", &[h0.into(), chart("1"), a.into()]);
    let tf0 = read_table_file(&p0, ReadOptions::default()).unwrap();
    assert!(NodeFrame::from_header(tf0.header.as_ref().unwrap()).is_ok());
}

/// B1 end to end through ASSEMBLY (the production path of a FULL-node retirement, `FullRetirements::BoundProof`):
/// the proof minted on `assemble::tables_in_force` before / after retiring B is accepted by `assemble`, which ships A
/// verbatim (with its `domain_bounds`); the engine's first match on the SHIPPED table has no rule at keys 1 and 2.
#[test]
fn p4_b1_end_to_end_assembly_no_longer_mints() {
    use rustred_campaign_core::accept::NodeTarget;
    use rustred_campaign_core::assemble::{
        AssemblyInput, FullRetirements, InputTable, tables_in_force,
    };
    use rustred_campaign_core::replay_base::sha256::sha256_hex;
    let dir = out_dir("b1e2e");
    let mask = "1";
    let a = r#"{"type":"rule","chart":"1","index":0,"fixed":[null],"equations":[],"exceptions":[],"target":"I(n0)","rhs":[["I(n0-1)","1"]],"witness_loci":[],"dispatch":"Partition","domain_bounds":{"min":[3],"max":[null]}}"#;
    let b = r#"{"type":"rule","chart":"1","index":1,"fixed":[null],"equations":[],"exceptions":[],"target":"I(n0)","rhs":[["I(n0-1)","1"]],"witness_loci":[],"dispatch":"Partition"}"#;
    let t = write(
        &dir,
        "node.table.jsonl",
        &[
            header(mask),
            chart(mask),
            a.into(),
            b.into(),
            r#"{"type":"end"}"#.into(),
        ],
    );
    let tag = |l: &str| format!("sha256:{}", sha256_hex(l.as_bytes()));
    let vb = tag(b);
    let inp = AssemblyInput {
        node: "SYNTH".into(),
        target: NodeTarget::Full,
        inputs: vec![InputTable {
            id: "node".into(),
            path: t.clone(),
            priority: 0,
            sha256: None,
        }],
        accepted: vec![],
        terminals: vec![],
        credits: vec![],
        retirements: vec![],
        removals: vec![],
        full_retirements: FullRetirements::BoundProof,
        keep_skipped: false,
        used: None,
        renumber_index: true,
        recipes: 0,
    };
    let frame = NodeFrame::without_header_lines_first(1, 1);
    let before = tables_in_force(&inp, &[]).unwrap();
    let after = tables_in_force(&inp, std::slice::from_ref(&vb)).unwrap();
    assert_eq!(
        after.tables()[0].domains[0].exceptions.len(),
        2,
        "partfix4: tables in force read A WITH its bounds"
    );
    prove_coverage_preserved(
        &DispatcherSource::new(&before),
        &DispatcherSource::new(&after),
        &frame,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
    )
    .expect_err("partfix4: retiring B opens keys 1, 2 of the tables in force: no proof");
    // (what was accepted before: a FULL retirement with a proof minted on the misread set; since partfix4 such a
    // retirement also needs the ENGINE cross-check, which fails on the shipped table: tests/partfix4_regressions.rs
    // pf4_engine_cross_check_catches_a_misread_table)
}

/// B2b: node/bounds.rs tests a bound on a FIXED slot against `fixed[i]` of the KEY, while `domain_bounds` bounds the
/// rule-point parameter n_i: with the target `I(n1,n0)` (slots swapped) and `fixed = [5, null]`, `min = [3, null]`
/// bounds pt0 = k1, but the engine compares fixed[0] = 5 >= 3 and excludes nothing: it selects R at keys (5,1), (5,2),
/// where the core (bound k1 - 3 >= 0) says NotSelected and dispatches to S.
#[test]
fn p4_b2b_bound_on_a_fixed_slot_is_tested_on_the_wrong_slot_by_the_engine() {
    use rustred_campaign_core::cover::RuleRef;
    use rustred_campaign_core::domain::{CaseDomain, TargetMap, TargetSlot};
    use rustred_campaign_core::partition::DomainList;
    use rustred_campaign_core::poly::Poly;
    let n = 2;
    let mask = vec![true, true];
    let frame = NodeFrame::without_header_lines_first(2, 2);
    let mut r = CaseDomain::free(n, Some(mask.clone()));
    r.fixed = vec![Some(5), None];
    r.target = TargetMap::Pattern(vec![
        TargetSlot::Param {
            param: 1,
            offset: 0,
        },
        TargetSlot::Param {
            param: 0,
            offset: 0,
        },
    ]);
    r.bounds = vec![Poly::parse_d_free_exact("n0-3", n).unwrap()];
    let s = CaseDomain::free(n, Some(mask.clone()));
    let mut new = DomainList::new(n);
    new.push(RuleRef::bare(0, 0, 0), r.clone());
    new.push(RuleRef::bare(0, 1, 1), s);
    prove_coverage_preserved(
        &new,
        &new,
        &frame,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
    )
    .expect_err("partfix4: a selection bound on a target-pattern record is not engine-readable");
    assert!(!r.applies(&[5, 1]).applies() && !r.applies(&[5, 2]).applies());
    assert!(r.applies(&[5, 3]).applies());
    let dir = out_dir("b2b");
    let rr = r#"{"type":"rule","chart":"11","index":0,"fixed":[5,null],"equations":[],"exceptions":[],"target":"I(n1,n0)","rhs":[["I(4,1)","1"]],"witness_loci":[],"dispatch":"Partition","domain_bounds":{"min":[3,null],"max":[null,null]}}"#;
    let ss = r#"{"type":"rule","chart":"11","index":1,"fixed":[null,null],"equations":[],"exceptions":[],"target":"I(n0,n1)","rhs":[["I(n0-1,n1)","1"]],"witness_loci":[],"dispatch":"Partition"}"#;
    let p = write(
        &dir,
        "new.jsonl",
        &[header("11"), chart("11"), rr.into(), ss.into()],
    );
    // partfix4: read from the file, R serves (5,1), (5,2), (5,3) as in the engine (audit's answers)
    let (d, _) = load(&p);
    for k in [[5, 1], [5, 2], [5, 3]] {
        assert!(
            matches!(d.first_match(&k).unwrap(), Dispatch::Covered(r) if r.label == 0),
            "{k:?}"
        );
    }
}
