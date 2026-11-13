//! Regression tests for the findings of the two independent audits of this crate (WS/notes/implementer.audit-core-A.md
//! and implementer.audit-core-B.md) and ruling F1 of the coordinator. One test per finding id; each test name starts
//! with the id. Every assertion here was FALSE at commit 62781c64 (the audited state; the auditors' probe outputs are
//! quoted in the comments): the old crate wrapped, accepted or credited where these tests now demand an error, a
//! refusal or an open verdict. Governing principle: FAIL CLOSED.

use rustred_campaign_core::poly::{BigInt, ENGINE_PRIMES, ParseOptions, Poly, PolyError, Value};

// ============================================================================================== poly (audit A)

/// A m1: `n18446744073709551616` (2^64) was read as `n0` (`v as usize` truncation), `...617` as `n1`.
#[test]
fn audit_a_m1_variable_index_never_wraps() {
    for s in [
        "n18446744073709551616",
        "n18446744073709551617",
        "n340282366920938463463374607431768211456", // beyond u128
        "n3",
    ] {
        assert!(
            matches!(
                Poly::parse_d_free(s, 3),
                Err(PolyError::VariableOutOfRange { .. }) | Err(PolyError::Syntax { .. })
            ),
            "{s} must be refused"
        );
    }
    assert_eq!(Poly::parse_d_free("n2", 3).unwrap(), Poly::var(2));
}

/// A m2: the parser accepted `n01` -> n1, `n00` -> n0 and `n1=2` -> n1-2, which the engine's `Expr::parse` refuses.
/// Strict is now the default; the equation form needs the explicit lenient option.
#[test]
fn audit_a_m2_strict_parser_is_the_default() {
    for s in ["n01", "n00", "n1=2", "n1 = n2", "n0+n007"] {
        assert!(Poly::parse_d_free(s, 3).is_err(), "{s} must be refused");
        assert!(Poly::parse(s, 3, true).is_err(), "{s} must be refused");
    }
    let lenient = ParseOptions {
        allow_d: false,
        allow_equation: true,
    };
    assert_eq!(
        Poly::parse_with("n1=2", 3, lenient).unwrap(),
        Poly::parse_d_free("n1-2", 3).unwrap()
    );
    assert!(Poly::parse_with("n01=2", 3, lenient).is_err());
    // forms the engine accepts stay accepted (unary signs, n0, n10)
    assert_eq!(
        Poly::parse_d_free("+-+n1", 3).unwrap(),
        Poly::parse_d_free("-n1", 3).unwrap()
    );
    assert!(Poly::parse_d_free("n0+n10", 11).is_ok());
}

/// A m3: `primitive_normal` computed `sign * (c/g)` unchecked: a leading coefficient `i128::MIN` with gcd 1 panicked
/// in debug (wrapped in release, giving a polynomial with another zero set).
#[test]
fn audit_a_m3_primitive_normal_is_checked() {
    let p = Poly::parse_d_free("n0 + (-(2^63))*(2^63)*2*n1", 3).unwrap();
    let q = p.primitive_normal(); // panicked before
    assert_eq!(q, p, "no representable normal form: returned unchanged");
    for k in [[0, 0, 0], [1, 0, 0], [0, 1, 0], [-3, 5, 1]] {
        assert_eq!(q.is_zero_at(&k).unwrap(), p.is_zero_at(&k).unwrap());
    }
    // ordinary case unchanged
    assert_eq!(
        Poly::parse_d_free("-4*n0+6*n1", 3)
            .unwrap()
            .primitive_normal(),
        Poly::parse_d_free("-2*n0+3*n1", 3).unwrap() // positive on the largest monomial n1
    );
}

/// A N1 (value part): the engine's zero test works modulo two 61-bit primes; a nonzero value divisible by both is
/// "zero" for the engine. The crate now detects exactly that case (the domain part is
/// `audit_a_n1_domain_fails_closed_on_engine_zero_disagreement`).
#[test]
fn audit_a_n1_value_detects_engine_modular_zero() {
    let pp = ENGINE_PRIMES[0] as i128 * ENGINE_PRIMES[1] as i128;
    assert!(Value::Small(pp).engine_zero_disagrees());
    assert!(Value::Small(-pp).engine_zero_disagrees());
    assert!(!Value::Small(pp + 1).engine_zero_disagrees());
    assert!(!Value::Small(ENGINE_PRIMES[0] as i128).engine_zero_disagrees());
    assert!(!Value::Small(0).engine_zero_disagrees());
    assert!(Value::Big(BigInt::from_i128(pp).mul(&BigInt::from_i128(3))).engine_zero_disagrees());
    assert!(
        !Value::Big(
            BigInt::from_i128(pp)
                .mul(&BigInt::from_i128(3))
                .add(&BigInt::from_i128(1))
        )
        .engine_zero_disagrees()
    );
}

/// A N2: `Value` derived `PartialEq` structurally, so `Small(0) != Big(0)`.
#[test]
fn audit_a_n2_value_equality_is_numeric() {
    assert_eq!(Value::Small(0), Value::Big(BigInt::zero()));
    assert_eq!(Value::Small(-7), Value::Big(BigInt::from_i128(-7)));
    assert_eq!(
        Value::Big(BigInt::from_i128(i128::MIN)),
        Value::Small(i128::MIN)
    );
    assert_ne!(Value::Small(1), Value::Big(BigInt::from_i128(2)));
    let huge = BigInt::from_i128(i128::MAX).mul(&BigInt::from_i128(4));
    assert_ne!(Value::Big(huge.clone()), Value::Small(i128::MAX));
    assert_eq!(Value::Big(huge.clone()), Value::Big(huge));
}

/// A N4: `Poly::var(slot as u16)` collided with the `d` variable at slot 65535.
#[test]
fn audit_a_n4_variable_cannot_collide_with_d() {
    assert!(matches!(
        Poly::parse_d_free("n65535", 70_000),
        Err(PolyError::VariableOutOfRange { .. })
    ));
    assert!(std::panic::catch_unwind(|| Poly::var(65_535)).is_err());
    assert!(!Poly::parse_d_free("n65534", 70_000).unwrap().has_d());
}

// ============================================================================ frame, records, domains, dispatch

use rustred_campaign_core::cover::{
    CoverError, Dispatch, Dispatcher, LoadError, SourceFormat, Table, WitnessProvenance,
};
use rustred_campaign_core::domain::{
    CaseDomain, DomainError, DomainOptions, NotInCase, PoleKind, TargetMap, TargetSlot,
    TargetSource, Verdict,
};
use rustred_campaign_core::key::{FrameError, FrameSource, NodeFrame};
use rustred_campaign_core::poly::EvalError;
use rustred_campaign_core::record::{
    ReadOptions, RecordError, RecordReader, RuleRecord, TableFile, TableHeader, TableLine,
};

const EXPORT: TargetSource = TargetSource::CanonicalByConstruction {
    attestation: "unit test: a case export of an engine table",
};

fn read_lines(text: &str) -> Vec<Result<TableLine, RecordError>> {
    RecordReader::new(text.as_bytes(), ReadOptions::default()).collect()
}

fn one_rule(line: &str) -> RuleRecord {
    match read_lines(line).pop().unwrap().unwrap() {
        TableLine::Rule(r) => *r,
        o => panic!("{o:?}"),
    }
}

fn header(line: &str) -> TableHeader {
    match read_lines(line).pop().unwrap().unwrap() {
        TableLine::Header(h) => *h,
        o => panic!("{o:?}"),
    }
}

/// A table file from JSONL text (both formats), as `read_table_file` would build it.
fn table_file(text: &str) -> TableFile {
    let mut t = TableFile::default();
    for l in read_lines(text) {
        match l.unwrap() {
            TableLine::Header(h) => t.header = Some(*h),
            TableLine::Rule(r) => t.rules.push(*r),
            TableLine::Chart { chart, guards } => t.charts.push((chart, guards)),
            TableLine::Other { kind } => t.others.push((kind, 1)),
        }
    }
    t
}

const H3: &str = r#"{"type":"header","schema":"table_dump.v1","denominators":3,"mask":"110","power_shifts":["0","0","0"]}"#;

fn frame3() -> NodeFrame {
    NodeFrame::from_header(&header(H3)).unwrap()
}

/// A rule line of arity 3 (table_dump.v1) with the given fields spliced in.
fn rule3(extra: &str) -> String {
    format!(
        r#"{{"type":"rule","chart":"110","index":0,"fixed":[null,null,null],"equations":[],"target":"I(n0,n1,n2)","rhs":[],"exceptions":[],"dispatch":"Partition"{extra}}}"#
    )
}

/// A M1: the rule `target` was never read. `target:"I(2,n1,n2)"` gave `Applies` at [1,1,0] (the engine requires
/// k[0] = 2); `target:"I(n0+1,n1,n2)"` with `equations:["n0-2"]` gave `Applies` at [2,1,0] (the engine evaluates the
/// equation at the rule point [1,1,0]) and not at [3,1,0]. Now the key is matched through the target exactly as
/// `find_rule` does; a missing target needs the caller's explicit canonical-by-construction statement.
#[test]
fn audit_a_m1_target_pattern_is_matched_as_the_engine() {
    let f = frame3();
    let o = DomainOptions::default();
    // constant slot
    let line = r#"{"type":"rule","chart":"110","index":0,"fixed":[null,null,null],"equations":["n0-1"],"target":"I(2,n1,n2)","rhs":[],"exceptions":[],"dispatch":"Partition"}"#;
    let d = CaseDomain::from_rule_record(&one_rule(line), &f, o).unwrap();
    assert!(matches!(d.target, TargetMap::Pattern(_)));
    assert_eq!(
        d.applies(&[1, 1, 0]),
        Verdict::NotInCase(NotInCase::TargetConstant {
            slot: 0,
            constant: 2,
            got: 1
        })
    );
    // at [2,1,0] the engine's rule point is [0,1,0] (unmapped parameter n0 -> fixed or 0): n0-1 = -1
    assert_eq!(
        d.applies(&[2, 1, 0]),
        Verdict::NotInCase(NotInCase::Equation(0))
    );
    // offset slot
    let line = r#"{"type":"rule","chart":"110","index":0,"fixed":[null,null,null],"equations":["n0-2"],"target":"I(n0+1,n1,n2)","rhs":[],"exceptions":[["n0-3"]],"dispatch":"Partition"}"#;
    let d = CaseDomain::from_rule_record(&one_rule(line), &f, o).unwrap();
    assert_eq!(
        d.target,
        TargetMap::Pattern(vec![
            TargetSlot::Param {
                param: 0,
                offset: 1
            },
            TargetSlot::Param {
                param: 1,
                offset: 0
            },
            TargetSlot::Param {
                param: 2,
                offset: 0
            },
        ])
    );
    assert_eq!(
        d.applies(&[2, 1, 0]),
        Verdict::NotInCase(NotInCase::Equation(0))
    );
    assert_eq!(d.applies(&[3, 1, 0]), Verdict::Applies);
    assert_eq!(
        d.applies(&[4, 1, 0]),
        Verdict::NotInCase(NotInCase::Equation(0))
    );
    // ... and the exception is evaluated at the rule point too: pt[0] = 3 at key [4,..] is excluded by the equation,
    // so plant a domain without the equation
    let line2 = line.replace(r#""equations":["n0-2"]"#, r#""equations":[]"#);
    let d2 = CaseDomain::from_rule_record(&one_rule(&line2), &f, o).unwrap();
    assert_eq!(d2.applies(&[4, 1, 0]), Verdict::OnException { branch: 0 });
    assert_eq!(d2.applies(&[3, 1, 0]), Verdict::Applies);
    // through the dispatcher (index + slow scan agree)
    let mut t = Table::new("t", 3);
    t.push(7, d).unwrap();
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    for (k, want) in [([3, 1, 0], Some(7)), ([2, 1, 0], None), ([4, 1, -1], None)] {
        assert_eq!(
            disp.first_match(&k).unwrap().winner().map(|w| w.label),
            want
        );
        assert_eq!(
            disp.first_match(&k).unwrap(),
            disp.first_match_slow(&k).unwrap()
        );
    }
    // canonical targets (the engine's own form) are recognised
    let d = CaseDomain::from_rule_record(&one_rule(&rule3("")), &f, o).unwrap();
    assert_eq!(d.target, TargetMap::Canonical);
    let fixed = r#"{"type":"rule","chart":"110","index":0,"fixed":[1,null,null],"equations":[],"target":"I(1,n1,n2)","rhs":[],"exceptions":[],"dispatch":"Partition"}"#;
    let d = CaseDomain::from_rule_record(&one_rule(fixed), &f, o).unwrap();
    assert_eq!(d.target, TargetMap::Canonical);
    // no target: refused unless the caller states canonical-by-construction (case exports, banks)
    let export = r#"{"i":0,"index":0,"chart":"110","fixed":[null,null,null],"equations":[],"exceptions":[],"dispatch":"Partition"}"#;
    let rec = one_rule(export);
    assert_eq!(
        CaseDomain::from_rule_record(&rec, &f, o),
        Err(DomainError::MissingTarget)
    );
    let ok = CaseDomain::from_rule_record(
        &rec,
        &f,
        DomainOptions {
            targets: EXPORT,
            ..o
        },
    )
    .unwrap();
    assert_eq!(ok.target, TargetMap::Canonical);
    // the statement is recorded in the loaded table's provenance
    let tf = table_file(&format!("{H3}\n{export}"));
    assert!(matches!(
        Table::from_table_file("e", "e", &tf, o),
        Err(LoadError::Record {
            err: DomainError::MissingTarget,
            ..
        })
    ));
    let t = Table::from_table_file(
        "e",
        "e",
        &tf,
        DomainOptions {
            targets: EXPORT,
            ..o
        },
    )
    .unwrap();
    assert_eq!(t.provenance.targets, Some(EXPORT));
    assert_eq!(t.provenance.format, SourceFormat::CaseExport);
    // bank predicates carry no target: FromRecord is refused
    let bank: rustred_campaign_core::record::BankPredicate = serde_json::from_str(
        r#"{"global_ordinal0":0,"raw_fixed20":[null,null,null],"no_recenter_or_permutation_applied":true}"#,
    )
    .unwrap();
    assert_eq!(
        CaseDomain::from_bank_predicate(&bank, 3, TargetSource::FromRecord),
        Err(DomainError::MissingTarget)
    );
    assert!(CaseDomain::from_bank_predicate(&bank, 3, EXPORT).is_ok());
    // unreadable or unsupported targets are typed errors
    for bad in [
        "I(n0,n0,n2)",
        "I(n0,n1)",
        "I(n0,n1,n7)",
        "I(n0,n1,x)",
        "I(n01,n1,n2)",
    ] {
        let l = rule3("").replace(r#""target":"I(n0,n1,n2)""#, &format!(r#""target":"{bad}""#));
        assert!(
            matches!(
                CaseDomain::from_rule_record(&one_rule(&l), &f, o),
                Err(DomainError::Target(_))
            ),
            "{bad}"
        );
    }
}

/// A M2: a record with `dispatch:"Numerical"` gave `Applies`; the engine's `Table::load` marks it `skip` and
/// `find_rule` never returns it. Now it is never a candidate, and the table reports the skipped records.
#[test]
fn audit_a_m2_non_partition_records_are_never_candidates() {
    let f = frame3();
    let numerical = rule3("").replace(r#""dispatch":"Partition""#, r#""dispatch":"Numerical""#);
    assert_eq!(
        CaseDomain::from_rule_record(&one_rule(&numerical), &f, DomainOptions::default()),
        Err(DomainError::NotDispatched {
            dispatch: "Numerical".into()
        })
    );
    // mixed table: record 0 (Numerical, applies everywhere), record 1 (Partition, n0 = 1), record 2 (no dispatch
    // field = Partition for the engine, applies everywhere)
    let r0 = numerical.replace(r#""index":0"#, r#""index":10"#);
    let r1 = r#"{"type":"rule","chart":"110","index":11,"fixed":[1,null,null],"equations":[],"target":"I(1,n1,n2)","rhs":[],"exceptions":[],"dispatch":"Partition"}"#;
    let r2 = r#"{"type":"rule","chart":"110","index":12,"fixed":[null,null,null],"equations":[],"target":"I(n0,n1,n2)","rhs":[],"exceptions":[]}"#;
    let r3 = rule3("")
        .replace(
            r#""dispatch":"Partition""#,
            r#""dispatch":"AfterBaselinePartitionWholePiece""#,
        )
        .replace(r#""index":0"#, r#""index":13"#);
    let tf = table_file(&format!("{H3}\n{r0}\n{r1}\n{r2}\n{r3}"));
    let t = Table::from_table_file("mixed", "mixed", &tf, DomainOptions::default()).unwrap();
    assert_eq!(t.skipped_count(), 2);
    assert_eq!(t.len(), 2);
    assert_eq!(
        t.provenance
            .skipped_not_dispatched
            .iter()
            .map(|s| (s.label, s.dispatch.as_str()))
            .collect::<Vec<_>>(),
        vec![(10, "Numerical"), (13, "AfterBaselinePartitionWholePiece")]
    );
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    assert_eq!(
        disp.first_match(&[1, 1, 0])
            .unwrap()
            .winner()
            .map(|w| w.label),
        Some(11)
    );
    assert_eq!(
        disp.first_match(&[2, 1, 0])
            .unwrap()
            .winner()
            .map(|w| w.label),
        Some(12)
    );
}

/// A M3: a pole verdict, a recenter bound and a chart mismatch let first match move on to the next rule (the old
/// crate reported `Covered(R2)`); the engine selects R1 and then fails. Now first match STOPS at the first selected
/// rule and the key is `Blocked{rule, reason}`: open, naming the rule. Exceptions, equations, fixed slots and target
/// constants still pass the search on (selection).
#[test]
fn audit_a_m3_selected_but_unusable_rule_blocks_the_key() {
    let f = frame3();
    let o = DomainOptions::default();
    let free = || CaseDomain::free(3, Some(vec![true, true, false]));
    // R1 with a witness pole n0 - 2 (not among its exceptions), then a free R2
    let r1 = one_rule(&rule3(r#","witness_loci":["n0-2"]"#));
    let d1 = CaseDomain::from_rule_record(&r1, &f, o).unwrap();
    let mut t = Table::new("t", 3);
    t.push(1, d1).unwrap();
    t.push(2, free()).unwrap();
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    let k = [2, 1, 0];
    let d = disp.first_match(&k).unwrap();
    assert_eq!(
        d,
        Dispatch::Blocked {
            at: rustred_campaign_core::cover::RuleRef {
                table: 0,
                record: 0,
                label: 1
            },
            reason: Verdict::OnPole {
                kind: PoleKind::Witness,
                index: 0
            }
        }
    );
    assert!(d.is_open() && !d.is_covered() && d.winner().is_none());
    assert_eq!(d, disp.first_match_slow(&k).unwrap());
    // off the pole R1 wins
    assert_eq!(
        disp.first_match(&[3, 1, 0])
            .unwrap()
            .winner()
            .map(|w| w.label),
        Some(1)
    );
    // union mode lists R1 as blocked, R2 as applying
    let (yes, open) = disp.union_cover(&k).unwrap();
    assert_eq!(yes.iter().map(|w| w.label).collect::<Vec<_>>(), vec![2]);
    assert_eq!(open.len(), 1);
    // batch counts it as blocked, not uncovered and not covered
    let rep = disp.batch(&[k.to_vec(), vec![3, 1, 0]], 1).unwrap();
    assert_eq!(
        (rep.covered, rep.uncovered, rep.blocked, rep.open()),
        (1, 0, 1, 1)
    );
    // recenter bound (opt-in) blocks too
    let rr = one_rule(&rule3(r#","recenter":[-1,0,0]"#)); // n0 >= 2
    let dr = CaseDomain::from_rule_record(
        &rr,
        &f,
        DomainOptions {
            recenter_bounds: true,
            ..o
        },
    )
    .unwrap();
    let mut t = Table::new("t", 3);
    t.push(1, dr).unwrap();
    t.push(2, free()).unwrap();
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    assert!(matches!(
        disp.first_match(&[1, 1, 0]).unwrap(),
        Dispatch::Blocked {
            reason: Verdict::RecenterBound(0),
            ..
        }
    ));
    assert_eq!(
        disp.first_match(&[2, 1, 0])
            .unwrap()
            .winner()
            .map(|w| w.label),
        Some(1)
    );
    // selection failures pass the search on: exception, equation, fixed slot, target constant
    for extra in [r#","exceptions":[["n0-1"]]"#, r#","equations":["n0-5"]"#] {
        let line = rule3("").replace(
            if extra.contains("exceptions") {
                r#","exceptions":[]"#
            } else {
                r#","equations":[]"#
            },
            extra,
        );
        let mut t = Table::new("t", 3);
        t.push(
            1,
            CaseDomain::from_rule_record(&one_rule(&line), &f, o).unwrap(),
        )
        .unwrap();
        t.push(2, free()).unwrap();
        let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
        assert_eq!(
            disp.first_match(&[1, 1, 0])
                .unwrap()
                .winner()
                .map(|w| w.label),
            Some(2),
            "{extra}"
        );
    }
    // keys outside the common chart are not dispatched at all
    assert_eq!(
        disp.first_match(&[0, 1, 0]).unwrap(),
        Dispatch::OutsideChart { slot: 0 }
    );
    assert_eq!(
        disp.first_match(&[1, 1, 1]).unwrap(),
        Dispatch::OutsideChart { slot: 2 }
    );
    // a record chart other than the frame mask is refused at load (the engine refuses such a chart)
    let other = rule3("").replace(r#""chart":"110""#, r#""chart":"100""#);
    assert!(matches!(
        CaseDomain::from_rule_record(&one_rule(&other), &f, o),
        Err(DomainError::ChartNotFrame { .. })
    ));
    let chart_line = r#"{"type":"chart","chart":"100","guards":["n>=1","n<=0","n<=0"]}"#;
    let tf = table_file(&format!("{H3}\n{chart_line}\n{}", rule3("")));
    assert!(matches!(
        Table::from_table_file("c", "c", &tf, o),
        Err(LoadError::Chart(_))
    ));
    // domains with different charts cannot share a dispatcher
    let mut a = Table::new("a", 3);
    a.push(1, CaseDomain::free(3, Some(vec![true, true, false])))
        .unwrap();
    a.push(2, CaseDomain::free(3, Some(vec![true, false, false])))
        .unwrap();
    assert!(matches!(
        Dispatcher::new(vec![a], vec![0]),
        Err(CoverError::Invalid(_))
    ));
}

/// A N1 (domain part): a literal that is nonzero but vanishes modulo both engine primes. As an exception literal the
/// old crate said `Applies` where the engine skips the rule. Now an evaluation error, and first match is Undecided.
#[test]
fn audit_a_n1_domain_fails_closed_on_engine_zero_disagreement() {
    let pp = ENGINE_PRIMES[0] as i128 * ENGINE_PRIMES[1] as i128;
    let lit = Poly::var(0).mul(&Poly::constant(pp)).unwrap();
    let mut d = CaseDomain::free(3, None);
    d.exceptions.push(vec![lit.clone()]);
    assert_eq!(
        d.applies(&[1, 1, 0]),
        Verdict::Error(EvalError::EngineZeroTest)
    );
    assert_eq!(d.applies(&[0, 1, 0]), Verdict::OnException { branch: 0 });
    let mut e = CaseDomain::free(3, None);
    e.equations.push(lit);
    assert_eq!(
        e.applies(&[1, 1, 0]),
        Verdict::Error(EvalError::EngineZeroTest)
    );
    let mut t = Table::new("t", 3);
    t.push(0, d).unwrap();
    t.push(1, CaseDomain::free(3, None)).unwrap();
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    assert!(matches!(
        disp.first_match(&[1, 1, 0]).unwrap(),
        Dispatch::Undecided { .. }
    ));
}

/// Ruling F1 (coordinator): a record with ELIDED witness loci is unusable: where it would win, the key is Blocked,
/// never a credit (old crate: `Undecided` from raw records, a credit from a bank predicate whose witness list holds
/// "elided" was a load error, and from case exports, which carry no witness loci, an undetectable credit: the loaded
/// table now says so in its provenance).
#[test]
fn ruling_f1_elided_witness_loci_block_from_every_source_format() {
    // table_dump.v1
    let el = rule3(r#","witness_loci":["elided"]"#);
    let tf = table_file(&format!("{H3}\n{el}"));
    let mut t = Table::from_table_file("raw", "raw", &tf, DomainOptions::default()).unwrap();
    assert_eq!(t.provenance.witness, WitnessProvenance::Checked);
    t.push(9, CaseDomain::free(3, None)).unwrap();
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    assert!(matches!(
        disp.first_match(&[1, 1, 0]).unwrap(),
        Dispatch::Blocked {
            reason: Verdict::Elided,
            ..
        }
    ));
    // with the witness check off the provenance says Disabled
    let t = Table::from_table_file(
        "raw",
        "raw",
        &tf,
        DomainOptions {
            witness: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(t.provenance.witness, WitnessProvenance::Disabled);
    // bank predicate with an elided witness locus
    let bank: rustred_campaign_core::record::BankPredicate = serde_json::from_str(
        r#"{"global_ordinal0":0,"raw_fixed20":[null,null,null],"raw_rule_witness_loci_generic_d_nonzero":["elided","n0-7"],"no_recenter_or_permutation_applied":true}"#,
    )
    .unwrap();
    let d = CaseDomain::from_bank_predicate(&bank, 3, EXPORT).unwrap();
    assert!(d.witness_elided);
    assert_eq!(d.applies(&[1, 1, 0]), Verdict::Elided);
    assert_eq!(
        d.applies(&[7, 1, 0]),
        Verdict::OnPole {
            kind: PoleKind::Witness,
            index: 0
        }
    );
    // case export: no witness field at all -> provenance NotCarried (the elided status cannot be seen)
    let export = r#"{"i":0,"index":0,"chart":"110","fixed":[null,null,null],"equations":[],"exceptions":[],"dispatch":"Partition"}"#;
    let tf = table_file(&format!("{H3}\n{export}"));
    let t = Table::from_table_file(
        "e",
        "e",
        &tf,
        DomainOptions {
            targets: EXPORT,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(t.provenance.witness, WitnessProvenance::NotCarried);
}

/// A M5: eps protection depended on the caller copying `power_shifts` into the frame (`from_mask` started without
/// shifts, so an eps record loaded as integer-only); numeric `0` and padded `"0 "` shifts were accepted. Now the frame
/// is built from the header by one constructor; a frame without a header is a visibly named escape hatch.
#[test]
fn audit_a_m5_frame_is_built_from_the_header() {
    let f = frame3();
    assert_eq!(f.source(), &FrameSource::Header);
    assert_eq!(f.power_shifts(), &["0", "0", "0"]);
    for shifts in [
        r#"[0,"0","0"]"#,
        r#"["0 ","0","0"]"#,
        r#"["0","1","0"]"#,
        r#"["0","eps","0"]"#,
    ] {
        let h = header(&H3.replace(r#"["0","0","0"]"#, shifts));
        assert!(
            matches!(NodeFrame::from_header(&h), Err(FrameError::Unsupported(_))),
            "{shifts}"
        );
    }
    // malformed headers
    for bad in [
        H3.replace(r#""mask":"110","#, ""),
        H3.replace(r#""denominators":3,"#, ""),
        H3.replace(r#""denominators":3"#, r#""denominators":4"#),
        H3.replace(r#"["0","0","0"]"#, r#"["0","0"]"#),
        H3.replace(r#"["0","0","0"]"#, r#""0""#),
    ] {
        assert!(NodeFrame::from_header(&header(&bad)).is_err(), "{bad}");
    }
    // absent / null power_shifts: no shift (as the engine)
    let h = header(&H3.replace(r#","power_shifts":["0","0","0"]"#, ""));
    assert!(NodeFrame::from_header(&h).unwrap().is_integer_only());
    let h = header(&H3.replace(r#"["0","0","0"]"#, "null"));
    assert!(NodeFrame::from_header(&h).is_ok());
    // escape hatches are labelled
    assert_eq!(
        NodeFrame::without_header_lines_first(3, 2).source(),
        &FrameSource::WithoutHeader
    );
    assert_eq!(
        NodeFrame::without_header_from_mask("110").unwrap().source(),
        &FrameSource::WithoutHeader
    );
    // the loader builds the frame from the header itself: an eps header refuses the whole table
    let tf = table_file(&format!(
        "{}\n{}",
        H3.replace(r#"["0","0","0"]"#, r#"["0","1","0"]"#),
        rule3("")
    ));
    assert!(matches!(
        Table::from_table_file("eps", "eps", &tf, DomainOptions::default()),
        Err(LoadError::Frame(FrameError::Unsupported(_)))
    ));
}

/// A m4: the JSON line scanner took the FIRST of duplicate keys (serde_json, which the engine uses, takes the last),
/// accepted junk after the closing brace, read `exceptions: null` or a missing `exceptions` as "no exceptions" (the
/// engine's load fails), and a short `recenter` silently gave fewer bounds.
#[test]
fn audit_a_m4_record_reader_is_strict() {
    let dup = r#"{"type":"rule","fixed":[1,null,null],"fixed":[null,null,null],"target":"I(n0,n1,n2)","rhs":[],"exceptions":[]}"#;
    assert!(read_lines(dup)[0].is_err());
    let junk = format!("{} trailing junk", rule3(""));
    assert!(read_lines(&junk)[0].is_err());
    assert!(read_lines(&format!("{}}}", rule3("")))[0].is_err());
    for no_exc in [
        rule3("").replace(r#","exceptions":[]"#, r#","exceptions":null"#),
        rule3("").replace(r#","exceptions":[]"#, ""),
    ] {
        assert!(read_lines(&no_exc)[0].is_err(), "{no_exc}");
    }
    // table_dump.v1 rules need target and rhs (Table::load)
    assert!(read_lines(&rule3("").replace(r#""target":"I(n0,n1,n2)","#, ""))[0].is_err());
    assert!(read_lines(&rule3("").replace(r#""rhs":[],"#, ""))[0].is_err());
    // short recenter with recenter bounds: typed error
    let r = one_rule(&rule3(r#","recenter":[0]"#));
    assert!(matches!(
        CaseDomain::from_rule_record(
            &r,
            &frame3(),
            DomainOptions {
                recenter_bounds: true,
                ..Default::default()
            }
        ),
        Err(DomainError::Malformed(_))
    ));
    // a second header line in one file
    let p = std::env::temp_dir().join(format!("core-fix-two-headers-{}.jsonl", std::process::id()));
    std::fs::write(&p, format!("{H3}\n{H3}\n")).unwrap();
    assert!(rustred_campaign_core::record::read_table_file(&p, ReadOptions::default()).is_err());
    let _ = std::fs::remove_file(&p);
}

/// A m5: `applies_in_chart` panicked on a short key and said `Applies` on a long one; a `CaseDomain` with
/// inconsistent public fields made `first_match` panic. Now typed verdicts / errors.
#[test]
fn audit_a_m5_bad_input_is_a_typed_error_not_a_panic() {
    let mut d = CaseDomain::free(3, None);
    d.fixed[2] = Some(0);
    assert!(matches!(
        d.applies_in_chart(&[1, 1]),
        Verdict::NotInCase(NotInCase::Arity { .. })
    ));
    assert!(matches!(
        d.applies_in_chart(&[1, 1, 0, 7]),
        Verdict::NotInCase(NotInCase::Arity { .. })
    ));
    let mut bad = CaseDomain::free(3, None);
    bad.fixed.push(Some(1));
    assert!(bad.validate().is_err());
    let mut t = Table::new("t", 3);
    assert!(t.push(0, bad.clone()).is_err());
    // public fields edited after push: the dispatcher validates again
    t.push(0, CaseDomain::free(3, None)).unwrap();
    t.domains[0] = bad;
    assert!(Dispatcher::new(vec![t], vec![0]).is_err());
    // a polynomial beyond the arity
    let mut p = CaseDomain::free(3, None);
    p.equations.push(Poly::var(5));
    assert!(p.validate().is_err());
    // frame predicates on a key of the wrong length fail closed
    let f = frame3();
    assert!(!f.in_chart_orthant(&[1, 1]));
    assert!(!f.is_top_sector(&[1, 1, 0, 0]));
    assert_eq!(f.dots(&[1, 1]), None);
    assert!(f.classify(&[1, 1]).is_err());
}

/// A m6: a key of the wrong length, a dispatcher without domains (arity 0) and a table missing from the priority all
/// yielded `Uncovered`. Now typed errors.
#[test]
fn audit_a_m6_caller_mistakes_are_errors_not_uncovered() {
    let mut t = Table::new("t", 3);
    t.push(0, CaseDomain::free(3, None)).unwrap();
    let disp = Dispatcher::new(vec![t.clone()], vec![0]).unwrap();
    assert_eq!(
        disp.first_match(&[1, 1, 0, 0]),
        Err(CoverError::KeyArity {
            expected: 3,
            got: 4
        })
    );
    assert!(disp.first_match_slow(&[1]).is_err());
    assert!(disp.union_cover(&[1]).is_err());
    assert!(disp.batch(&[vec![1, 1, 0], vec![1]], 2).is_err());
    assert!(matches!(
        Dispatcher::new(vec![], vec![]),
        Err(CoverError::Arity(_))
    ));
    assert!(matches!(
        Dispatcher::new(vec![t.clone(), t.clone()], vec![1]),
        Err(CoverError::BadPriority(_))
    ));
    let other = Table::new("u", 4);
    assert!(matches!(
        Dispatcher::new(vec![t, other], vec![0, 1]),
        Err(CoverError::Arity(_))
    ));
    // an empty table of known arity is fine: everything uncovered, but by decision, not by accident
    let empty = Dispatcher::new(vec![Table::new("e", 3)], vec![0]).unwrap();
    assert_eq!(empty.first_match(&[1, 1, 0]).unwrap(), Dispatch::Uncovered);
}

/// A m7: `TableHeader.two_sided` dropped boolean flags (read as u64 slot indices) and `coordinate_priority` preferred
/// `order.*` over `node_order.*` (the engine uses node_order). Flags are now read as the engine's table_import reads
/// them (bool, or number meaning true iff 1).
#[test]
fn audit_a_m7_header_fields_read_as_the_engine() {
    let h = header(
        r#"{"type":"header","denominators":4,"mask":"1100","node_order":{"coordinate_priority":[3,2,1,0]},"order":{"coordinate_priority":[0,1,2,3],"two_sided":[true,false,1,0]}}"#,
    );
    assert_eq!(h.two_sided, vec![true, false, true, false]);
    assert_eq!(h.coordinate_priority, Some(vec![3, 2, 1, 0]));
    assert_eq!(NodeFrame::from_header(&h).unwrap().two_sided(), &[0, 2]);
    let only_order = header(
        r#"{"type":"header","denominators":2,"mask":"10","order":{"coordinate_priority":[1,0]}}"#,
    );
    assert_eq!(only_order.coordinate_priority, Some(vec![1, 0]));
    for bad in [
        r#"{"type":"header","denominators":2,"mask":"10","order":{"two_sided":["yes",false]}}"#,
        r#"{"type":"header","denominators":2,"mask":"10","order":{"coordinate_priority":[1,"0"]}}"#,
        r#"{"type":"header","denominators":2,"mask":"10","node_order":{"coordinate_priority":[-1,0]}}"#,
    ] {
        assert!(read_lines(bad)[0].is_err(), "{bad}");
    }
}
