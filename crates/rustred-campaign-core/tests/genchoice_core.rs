//! Lane sizegen: the pure generation-choice library (interfaces/genchoice.md).

use rustred_campaign_core::economy::genchoice::*;

const FAM: &str = "I(\n name(X),\n loops(k1,k2),\n externals(q),\n dimension(d),\n prop(D1,(k1)^2,1),\n prop(D2,(k1+k2-q)^2,1),\n prop(D3,(k2)^2,1),\n prop(D4,(k1+q)^2,0),\n prop(D5,(k2-q)^2,0),\n gram(q,q,1)\n)\n";

fn shape() -> FamilyShape {
    family_shape(FAM).unwrap()
}

fn row(fixed: &[Option<i64>], monomials: u64) -> RuleRow {
    RuleRow {
        case: fixed
            .iter()
            .map(|x| x.map_or("_".to_string(), |v| v.to_string()))
            .collect::<Vec<_>>()
            .join(","),
        fixed: fixed.to_vec(),
        monomials,
        rhs_terms: 1,
        bytes: 100,
    }
}

#[test]
fn family_shape_reads_flags_with_nested_commas() {
    let s = shape();
    assert_eq!(s.arity, 5);
    assert_eq!(s.lines, vec![0, 1, 2]);
    assert_eq!(s.isps, vec![3, 4]);
    // interleaved flags and a prop name inside a word are handled generically
    let t = "I(prop(A,(k1,k2)^2,0),xprop(B,1,1),prop(C,f(a,b),1))";
    let s2 = family_shape(t).unwrap();
    assert_eq!(
        (s2.arity, s2.lines.clone(), s2.isps.clone()),
        (2, vec![1], vec![0])
    );
    assert!(family_shape("I(prop(A,x,2))").is_err());
    assert!(family_shape("nothing").is_err());
}

#[test]
fn r1_classes() {
    assert_eq!(r1_class(true, Some(1)), 0);
    assert_eq!(r1_class(true, None), 1);
    assert_eq!(r1_class(true, Some(3)), 2);
    assert_eq!(r1_class(false, Some(0)), 0);
    assert_eq!(r1_class(false, None), 1);
    assert_eq!(r1_class(false, Some(-2)), 2);
}

#[test]
fn bfi_vote_puts_boundary_first_free_next_interior_last_weighted_by_size() {
    let s = shape();
    // heavy case: line 2 at its boundary, line 0 free, line 1 dotted (interior); ISP 4 at 0, ISP 3 free
    let heavy = row(&[None, Some(2), Some(1), None, Some(0)], 1_000_000);
    // many light cases with the opposite preference
    let light: Vec<RuleRow> = (0..50)
        .map(|_| row(&[Some(1), None, Some(2), Some(0), None], 10))
        .collect();
    let mut rows = light.clone();
    rows.push(heavy.clone());
    let p = bfi_priority(&s, &rows, &[], VoteWeight::Monomials, 1_000_000);
    assert_eq!(
        p,
        vec![2, 0, 1, 4, 3],
        "the heavy case decides under size weights"
    );
    let u = bfi_priority(&s, &rows, &[], VoteWeight::Uniform, 1_000_000);
    assert_eq!(
        u,
        vec![0, 1, 2, 3, 4],
        "uniform weights follow the majority"
    );
    // no case: slot order inside each group
    assert_eq!(
        bfi_priority(&s, &[], &[], VoteWeight::Monomials, 1_000_000),
        vec![0, 1, 2, 3, 4]
    );
    // a REFUSED (heavy) case outweighs the light majority even under uniform row weights
    let refused = vec![vec![None, Some(2), Some(1), None, Some(0)]];
    assert_eq!(
        bfi_priority(&s, &light, &refused, VoteWeight::Uniform, 1_000_000),
        vec![2, 0, 1, 4, 3]
    );
    // a row of another arity is ignored
    let odd = RuleRow {
        fixed: vec![None],
        ..heavy
    };
    assert_eq!(
        bfi_priority(&s, &[odd], &[], VoteWeight::Monomials, 1_000_000),
        vec![0, 1, 2, 3, 4]
    );
}

#[test]
fn orders_are_comparator_only_priorities() {
    let s = shape();
    assert_eq!(coordinate_priority(OrderKind::Id, &s, None), Some(None));
    assert_eq!(
        coordinate_priority(OrderKind::IspRev, &s, None),
        Some(Some(vec![0, 1, 2, 4, 3]))
    );
    assert_eq!(
        coordinate_priority(OrderKind::LinesRev, &s, None),
        Some(Some(vec![2, 1, 0, 3, 4]))
    );
    assert_eq!(coordinate_priority(OrderKind::Bfi, &s, None), None);
    assert_eq!(
        coordinate_priority(OrderKind::Bfi, &s, Some(&[1, 0, 2, 3, 4])),
        Some(Some(vec![1, 0, 2, 3, 4]))
    );
    let j: serde_json::Value =
        serde_json::from_str(&order_json("bfi", Some(&[1, 0, 2, 3, 4]))).unwrap();
    assert_eq!(j["slot_permutation"], serde_json::Value::Null);
    assert_eq!(j["source_order"], "Input");
    assert_eq!(j["coordinate_priority"], serde_json::json!([1, 0, 2, 3, 4]));
    let k: serde_json::Value = serde_json::from_str(&order_json("id", None)).unwrap();
    assert_eq!(k["coordinate_priority"], serde_json::Value::Null);
}

#[test]
fn default_policy_candidates_and_args() {
    let p = Policy::default();
    p.check().unwrap();
    let c = p.candidates();
    assert_eq!(c.len(), 8);
    assert_eq!(c[0].id, "id-default-std");
    assert_eq!(c[1].id, "id-rung-std");
    assert_eq!(c[2].id, "bfi-default-std");
    assert_eq!(p.bfi_base().unwrap().id, "id-default-std");
    let a = p.args(&c[1], "{}");
    assert_eq!(a, vec!["--order", "{}", "--seed-pose-rung"]);
    // a bfi order without a base is refused; a duplicate order is refused
    let mut q = Policy {
        orders: vec![OrderKind::Bfi],
        ..Policy::default()
    };
    assert!(q.check().is_err());
    q.orders = vec![OrderKind::Id, OrderKind::Id];
    assert!(q.check().is_err());
    q = Policy::default();
    q.economy.factor = f64::INFINITY;
    assert!(q.check().is_err());
    q.economy.optimise = true;
    q.check().unwrap();
}

const TABLE: &str = r#"{"type":"header","schema":"table_dump.v1"}
{"type":"chart","chart":"11100"}
{"type":"rule","chart":"11100","index":0,"fixed":[null,null,null,null,null],"equations":[],"rhs":[["I(a)","1"]],"monomials":10}
{"type":"rule","chart":"11100","index":1,"fixed":[null,1,null,0,null],"equations":["n0-n2"],"rhs":[["I(a)","1"],["I(b)","2"]],"monomials":5000}
{"type":"residual","chart":"11100","key":"I(1,1,1,0,0)"}
{"type":"end","charts":1,"rules":2,"rhs_terms":3,"residuals":1,"chart_errors":0,"seconds":12.5,"vmhwm_kb":2048}"#;

#[test]
fn table_measures() {
    let c = CostModel::default();
    let (m, rows) = measure_table(TABLE.lines(), 1000, &c).unwrap();
    assert_eq!(m.rules, 2);
    assert_eq!(m.residuals, 1);
    assert_eq!(m.residual_keys, vec!["I(1,1,1,0,0)"]);
    assert_eq!(m.monomials, 5010);
    assert_eq!(m.max_monomials, 5000);
    assert_eq!(m.rhs_terms, 3);
    assert!(m.end_record);
    assert_eq!(m.engine_seconds, Some(12.5));
    assert_eq!(m.vmhwm_kb, Some(2048));
    assert_eq!(
        m.use_cost_ns,
        30.0 * 1000.0 + 2.0 * 20000.0 + 100.0 * 5010.0
    );
    assert_eq!(rows[1].case, "_,1,_,0,_|n0-n2");
    assert!(measure_table(["not json"], 1, &c).is_err());
}

fn tm(rules: u64, bytes: u64, monomials: u64, max: u64) -> TableMeasure {
    let c = CostModel::default();
    TableMeasure {
        rules,
        table_bytes: bytes,
        monomials,
        max_monomials: max,
        end_record: true,
        use_cost_ns: use_cost_ns(&c, bytes, rules, monomials),
        ..TableMeasure::default()
    }
}

#[test]
fn ranking_status_first_then_cost_band_then_largest_rule() {
    let probes = vec![
        // 0: heavy (wall stop) but tiny: loses to every finished probe
        (0, RunStatus::Heavy, Some(tm(10, 1000, 100, 50)), 0),
        // 1: max_cases, large rules
        (
            1,
            RunStatus::MaxCases,
            Some(tm(100, 10_000_000, 9_000_000, 2_000_000)),
            0,
        ),
        // 2: max_cases, small rules
        (
            2,
            RunStatus::MaxCases,
            Some(tm(100, 100_000, 50_000, 3_000)),
            0,
        ),
        // 3: failed: excluded
        (3, RunStatus::Failed, None, 0),
        // 4: complete, same band as 2, larger largest rule
        (
            4,
            RunStatus::Complete,
            Some(tm(100, 100_000, 50_000, 30_000)),
            0,
        ),
        // 5: open regions
        (5, RunStatus::OpenRegions, Some(tm(100, 1000, 100, 10)), 0),
        // 6: heavy without a table (wall stop: nothing sealed): eligible, after the measured heavy one
        (6, RunStatus::Heavy, None, 0),
        // 7: lost at its memory cap (no table): ranked like a heavy run, never an error
        (7, RunStatus::LostAtCap, None, 0),
    ];
    assert_eq!(ranking(&probes), vec![2, 4, 1, 5, 0, 6, 7]);
    // a probe that refused one heavy relation loses to every probe of its status that refused none
    let mut p2 = probes.clone();
    p2[2].3 = 1;
    assert_eq!(ranking(&p2), vec![4, 1, 2, 5, 0, 6, 7]);
    assert_eq!(band(0.0), 0);
    assert_eq!(band(1.0), 1);
    assert_eq!(band(3.0), 2);
    assert_eq!(band(f64::NAN), 0);
}

fn run(cand: usize, ok: bool, cpu: u64, bytes: u64, alt: bool) -> FullRun {
    FullRun {
        cand,
        acceptable: ok,
        cpu_ms: cpu,
        wall_ms: 0,
        measure: Some(tm(10, bytes, bytes / 10, bytes / 100)),
        alternative: alt,
    }
}

#[test]
fn economy_two_alternatives_capped_at_factor_times_c1() {
    let p = Policy::default();
    let r = vec![5, 2, 7, 1];
    assert_eq!(next_step(&r, &[], &p), NextStep::First(5));
    // first full run fails -> next
    let d1 = vec![run(5, false, 1000, 1, false)];
    assert_eq!(next_step(&r, &d1, &p), NextStep::First(2));
    // first acceptable at 2 with C1 = 100 s: alternatives 7 and 1, each capped at 2 x 100 / 2 = 100 s
    let d2 = vec![
        run(5, false, 1000, 1, false),
        run(2, true, 100_000, 5000, false),
    ];
    assert_eq!(
        next_step(&r, &d2, &p),
        NextStep::Alternatives(vec![(7, Some(100)), (1, Some(100))])
    );
    let d3 = vec![
        run(5, false, 1000, 1, false),
        run(2, true, 100_000, 5000, false),
        run(7, true, 90_000, 3000, true),
        run(1, false, 100_000, 0, true),
    ];
    assert_eq!(next_step(&r, &d3, &p), NextStep::Decide);
    assert_eq!(choose(&d3, Select::Bytes), Some(7));
    assert_eq!(choose(&d3, Select::UseCost), Some(7));
    let acct = economy_account(&r, &d3, &p);
    assert_eq!(acct["c1_cpu_ms"], 100_000);
    assert_eq!(acct["budget_cpu_ms"], 200_000);
    assert_eq!(acct["extra_cpu_ms"], 190_000);
    // factor 0: no alternative
    let mut q = Policy::default();
    q.economy.factor = 0.0;
    assert_eq!(next_step(&r, &d2, &q), NextStep::Decide);
    // optimise: every remaining ranked candidate, no cap
    q.economy.optimise = true;
    assert_eq!(
        next_step(&r, &d2, &q),
        NextStep::Alternatives(vec![(7, None), (1, None)])
    );
}

#[test]
fn economy_first_attempt_limit_and_completed_probes() {
    let mut p = Policy::default();
    p.full.max_first_attempts = 2;
    let r = vec![0, 1, 2];
    let d = vec![run(0, false, 1, 1, false), run(1, false, 1, 1, false)];
    assert!(matches!(next_step(&r, &d, &p), NextStep::NoCandidate(_)));
    // a probe that completed the node for the top candidate is its full run: no full job, alternatives next
    let d2 = vec![run(0, true, 10_000, 100, false)];
    assert_eq!(
        next_step(&r, &d2, &Policy::default()),
        NextStep::Alternatives(vec![(1, Some(10)), (2, Some(10))])
    );
    // a lower-ranked candidate whose probe completed is never re-run, but competes in the choice
    let d3 = vec![run(0, true, 10_000, 100, false), run(2, true, 5, 50, false)];
    assert_eq!(
        next_step(&r, &d3, &Policy::default()),
        NextStep::Alternatives(vec![(1, Some(20))]),
        "one alternative gets the whole budget"
    );
    assert_eq!(choose(&d3, Select::Bytes), Some(2));
    assert!(matches!(next_step(&[], &[], &p), NextStep::NoCandidate(_)));
}

#[test]
fn per_rule_comparison_matches_cases() {
    let w = vec![row(&[None, Some(1)], 10), row(&[Some(2), None], 20)];
    let o = vec![row(&[None, Some(1)], 1000), row(&[Some(3), None], 5)];
    let (lines, sum) = per_rule_comparison(&w, &[("lost".into(), o)]);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["others"]["lost"], 1000);
    assert_eq!(lines[1]["others"]["lost"], serde_json::Value::Null);
    assert_eq!(sum["lost"]["shared_cases"], 1);
    assert_eq!(sum["lost"]["loser_monomials_on_shared"], 1000);
    assert_eq!(sum["lost"]["only_in_loser"], 1);
}

#[test]
fn config_parse_and_import_spec() {
    let h = "a".repeat(64);
    let txt = format!(
        r#"{{"schema":"{CONFIG_SCHEMA}","program":"rr_node","nodes":[{{"node":"N1","family":"/f.sym","family_sha256":"{h}","box":{{"path":"/b.json","sha256":"{h}"}}}}],"policy":{{"probe":{{"cases":10}}}}}}"#
    );
    let c = Config::parse(&txt).unwrap();
    assert_eq!(c.policy.probe.cases, 10);
    assert_eq!(
        c.policy.probe.wall_case_s,
        Some(600),
        "defaults fill the rest"
    );
    assert!(
        Config::parse(&txt.replace("\"cases\"", "\"casez\"")).is_err(),
        "unknown field refused"
    );
    assert!(Config::parse(&txt.replace(&h, "XYZ")).is_err());
    let s = import_spec("N1", "/b.json", &h, "/t.jsonl", &h);
    assert_eq!(s["sources"][0]["table_terminals"], "ledger_and_residuals");
    assert_eq!(s["start"]["format"], "box");
}

#[test]
fn deep_cases_refused_first_then_most_constrained_and_deep_ranking() {
    let s = shape();
    let oc = |f: Vec<Option<i64>>| {
        OpenCase::from_case(&serde_json::json!({"frame":"presented","fixed":f,"equations":[]}))
            .unwrap()
    };
    let refused = vec![vec![oc(vec![None, None, Some(1), None, None])]];
    let open = vec![
        vec![
            oc(vec![Some(1), Some(2), None, Some(0), None]),
            oc(vec![Some(1), Some(1), Some(1), Some(-1), Some(0)]),
            oc(vec![None, None, Some(1), None, None]),
        ],
        vec![oc(vec![Some(1), Some(1), Some(1), Some(-1), Some(0)])],
    ];
    let d = deep_cases(&s, &refused, &open, 3);
    assert_eq!(d.len(), 3);
    assert_eq!(
        d[0].fixed,
        vec![None, None, Some(1), None, None],
        "refused cases first"
    );
    assert_eq!(
        d[1].fixed,
        vec![Some(1), Some(1), Some(1), Some(-1), Some(0)],
        "most fixed slots next, deduplicated"
    );
    assert_eq!(d[2].fixed, vec![Some(1), Some(2), None, Some(0), None]);
    assert_eq!(deep_cases(&s, &refused, &open, 1).len(), 1);
    // ranking with the deep stage: the same deep cases decide before the shallow cost bands
    let dm = |monomials: u64, refused: u64| {
        Some(Some(DeepMeasure {
            solved: 3,
            refused,
            monomials,
            max_monomials: monomials,
            cpu_ms: 0,
        }))
    };
    let probes = vec![
        (
            0,
            RunStatus::MaxCases,
            Some(tm(100, 100_000, 50_000, 3_000)),
            0,
            dm(1_000_000, 0),
        ),
        (
            1,
            RunStatus::MaxCases,
            Some(tm(100, 10_000_000, 9_000_000, 2_000_000)),
            0,
            dm(1_000, 0),
        ),
        (
            2,
            RunStatus::MaxCases,
            Some(tm(100, 100_000, 50_000, 3_000)),
            0,
            dm(10, 1),
        ),
        (
            3,
            RunStatus::MaxCases,
            Some(tm(100, 100_000, 50_000, 3_000)),
            0,
            Some(None),
        ),
    ];
    assert_eq!(ranking_deep(&probes), vec![1, 0, 2, 3]);
}

#[test]
fn first_run_is_the_fastest_within_the_size_slack_alternatives_follow_size() {
    let p = Policy::default();
    let fi = |cand, class: (u8, u64), size_band, cpu_ms| FirstInfo {
        cand,
        class,
        size_band,
        cpu_ms,
    };
    // ranking by size: 3 (band 10), 1 (band 11, 4x faster), 2 (band 12), 0 (deep refusal)
    let ranking = vec![3, 1, 2, 0];
    let info = vec![
        fi(3, (0, 0), 10, 400_000),
        fi(1, (0, 0), 11, 100_000),
        fi(2, (0, 0), 12, 1_000),
        fi(0, (0, 1), 5, 10),
    ];
    let f = first_order(&ranking, &info, &p);
    assert_eq!(
        f,
        vec![1, 3, 2, 0],
        "2 is outside the slack, 0 is of a worse class"
    );
    let mut q = Policy::default();
    q.first.by = FirstBy::BestSize;
    assert_eq!(first_order(&ranking, &info, &q), ranking);
    // the fast candidate runs first; once acceptable, the alternatives are the size ranking without it
    assert_eq!(next_step_with(&ranking, &f, &[], &p), NextStep::First(1));
    let d = vec![run(1, true, 10_000, 500, false)];
    assert_eq!(
        next_step_with(&ranking, &f, &d, &p),
        NextStep::Alternatives(vec![(3, Some(10)), (2, Some(10))])
    );
    assert_eq!(economy_account(&f, &d, &p)["first_acceptable"], 1);
}

#[test]
fn deep_cases_sample_every_case_type_round_robin() {
    let s = shape();
    let oc = |f: Vec<Option<i64>>| {
        OpenCase::from_case(&serde_json::json!({"frame":"presented","fixed":f,"equations":[]}))
            .unwrap()
    };
    // type A (no free line, one free ISP, no interior slot): two cases with 4 fixed slots; type B (two free lines): 3
    let open = vec![vec![
        oc(vec![Some(1), Some(1), Some(1), Some(0), None]),
        oc(vec![Some(1), Some(1), Some(1), None, Some(0)]),
        oc(vec![None, None, Some(1), Some(0), Some(0)]),
    ]];
    let d = deep_cases(&s, &[], &open, 2);
    let free_lines = |c: &OpenCase| c.fixed[..3].iter().filter(|x| x.is_none()).count();
    assert_eq!(d.len(), 2);
    assert_ne!(
        free_lines(&d[0]),
        free_lines(&d[1]),
        "one case of each type before a second of any: {d:?}"
    );
}

#[test]
fn alternative_caps_follow_the_first_runs_wall_time_when_known() {
    let p = Policy::default();
    let r = vec![0, 1, 2];
    let mut first = run(0, true, 100_000, 100, false);
    first.wall_ms = 260_000;
    // two alternatives share 2 x 260 s of wall (CPU 100 s: a shared host ran at wall / cpu = 2.6)
    assert_eq!(
        next_step(&r, &[first], &p),
        NextStep::Alternatives(vec![(1, Some(260)), (2, Some(260))])
    );
}

#[test]
fn probe_stage_only_runs_no_full_job() {
    let mut p = Policy::default();
    p.full.max_first_attempts = 0;
    p.check().unwrap();
    match next_step(&[2, 0, 1], &[], &p) {
        NextStep::NoCandidate(w) => assert!(w.contains("probe stage only"), "{w}"),
        o => panic!("{o:?}"),
    }
}
