//! Tests on toy tables: a planted gap, a planted cycle and a planted pole must each be caught; and a randomized
//! differential test of the face recursion against brute-force enumeration (the decision procedure must return the
//! EXACT uncovered set). Toy data only (L8: names and counts appear in tests only).

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;

    use crate::cover::Cover;
    use crate::face::Face;
    use crate::keys::{Rng, for_each_key};
    use crate::table::Table;
    use crate::{descent, dispatch, poles};

    fn tmpdir() -> std::path::PathBuf {
        let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());
        let d = std::path::PathBuf::from(base).join(format!(
            "rr_pverify_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn header(n: usize, mask: &str) -> serde_json::Value {
        let prio: Vec<usize> = (0..n).collect();
        json!({"type": "header", "schema": "table_dump.v1", "denominators": n, "mask": mask,
               "power_shifts": vec!["0"; n],
               "node_order": {"name": "toy", "slot_permutation": null, "coordinate_priority": prio, "source_order": "Input"},
               "order_is_program": true,
               "order": {"support_priority": prio, "coordinate_priority": prio, "two_sided": []}})
    }

    struct ToyRule {
        fixed: Vec<Option<i64>>,
        equations: Vec<&'static str>,
        exceptions: Vec<Vec<String>>,
        rhs: Vec<(String, String)>,
    }

    fn rule_json(n: usize, mask: &str, i: usize, r: &ToyRule) -> serde_json::Value {
        let target = format!(
            "I({})",
            (0..n)
                .map(|j| match r.fixed[j] {
                    Some(x) => x.to_string(),
                    None => format!("n{j}"),
                })
                .collect::<Vec<_>>()
                .join(",")
        );
        json!({"type": "rule", "chart": mask, "index": i, "fixed": r.fixed, "equations": r.equations,
               "target": target, "exceptions": r.exceptions,
               "rhs": r.rhs.iter().map(|(p, c)| json!([p, c])).collect::<Vec<_>>(), "dispatch": "Partition"})
    }

    fn write_table(
        dir: &std::path::Path,
        name: &str,
        n: usize,
        mask: &str,
        rules: &[ToyRule],
        masters: &[&str],
    ) -> String {
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(f, "{}", header(n, mask)).unwrap();
        writeln!(
            f,
            "{}",
            json!({"type": "zero_sectors", "scope": "downset", "count": 0, "masks": []})
        )
        .unwrap();
        let guards: Vec<&str> = mask
            .chars()
            .map(|c| if c == '1' { "n>=1" } else { "n<=0" })
            .collect();
        writeln!(
            f,
            "{}",
            json!({"type": "chart", "chart": mask, "guards": guards, "zero": false})
        )
        .unwrap();
        for (i, r) in rules.iter().enumerate() {
            writeln!(f, "{}", rule_json(n, mask, i, r)).unwrap();
        }
        for m in masters {
            writeln!(f, "{}", json!({"type": "residual", "key": m})).unwrap();
        }
        writeln!(f, "{}", json!({"type": "end"})).unwrap();
        p.to_string_lossy().to_string()
    }

    /// n = 3, two lines and one numerator; master I(1,1,0).
    fn good_rules() -> Vec<ToyRule> {
        vec![
            // n0 >= 2: lower n0
            ToyRule {
                fixed: vec![None, None, None],
                equations: vec![],
                exceptions: vec![vec!["-1+n0".into()]],
                rhs: vec![("I(n0-1,n1,n2)".into(), "(n1+d)/(-1+n0)".into())],
            },
            // n0 = 1, n1 >= 2
            ToyRule {
                fixed: vec![Some(1), None, None],
                equations: vec![],
                exceptions: vec![vec!["-1+n1".into()]],
                rhs: vec![("I(1,n1-1,n2)".into(), "(d-4)/(-1+n1)".into())],
            },
            // n0 = n1 = 1, n2 <= -1: raise the numerator; the term with n2+1 is never an ISP (n2 = 0 excluded)
            ToyRule {
                fixed: vec![Some(1), Some(1), None],
                equations: vec![],
                exceptions: vec![vec!["n2".into()]],
                rhs: vec![("I(1,1,n2+1)".into(), "n2/(d-4)".into())],
            },
        ]
    }

    fn cover_uncovered(path: &str, r: i128) -> (Vec<Vec<i64>>, usize) {
        let t = Table::load(path).unwrap();
        let mut c = Cover::new(&t, Some(r)).unwrap();
        let o = c.solve(&Face::top(), 0);
        (o.uncovered.iter().cloned().collect(), o.undecided.len())
    }

    fn brute_uncovered(path: &str, r: i64) -> Vec<Vec<i64>> {
        let t = Table::load(path).unwrap();
        let mut out = vec![];
        for_each_key(&t.mask, r, &mut |k| {
            if dispatch::first_match(&t, k).is_none() {
                out.push(k.to_vec());
            }
        });
        out.sort();
        out
    }

    #[test]
    fn good_toy_table_passes() {
        let d = tmpdir();
        let p = write_table(&d, "good.jsonl", 3, "110", &good_rules(), &["I(1,1,0)"]);
        let (u, und) = cover_uncovered(&p, 12);
        assert_eq!(und, 0);
        assert_eq!(u, vec![vec![1, 1, 0]]);
        assert_eq!(brute_uncovered(&p, 12), u);
        let de = descent::run(&p).unwrap();
        assert_eq!(de["verdict"], "YES", "{de}");
        let po = poles::run(&p, Some(12), None).unwrap();
        assert_eq!(po["verdict"], "YES", "{po}");
    }

    #[test]
    fn planted_gap_is_caught() {
        let d = tmpdir();
        let mut rules = good_rules();
        rules.remove(1); // n0 = 1, n1 >= 2 is now served by nothing
        let p = write_table(&d, "gap.jsonl", 3, "110", &rules, &["I(1,1,0)"]);
        let (u, und) = cover_uncovered(&p, 6);
        assert_eq!(und, 0);
        assert!(
            u.contains(&vec![1, 2, 0]) && u.contains(&vec![1, 7, 0]) && u.contains(&vec![1, 3, -2])
        );
        assert_eq!(u, brute_uncovered(&p, 6));
    }

    #[test]
    fn planted_cycle_is_caught() {
        let d = tmpdir();
        let mut rules = good_rules();
        // a rule placed first on n0 = 1, n1 >= 2 that maps the key UP (to n1 + 1): with rule 1 it forms a 2-cycle
        rules.insert(
            0,
            ToyRule {
                fixed: vec![Some(1), None, None],
                equations: vec![],
                exceptions: vec![vec!["-1+n1".into()], vec!["-3+n1".into()]],
                rhs: vec![("I(1,n1+1,n2)".into(), "1".into())],
            },
        );
        let p = write_table(&d, "cycle.jsonl", 3, "110", &rules, &["I(1,1,0)"]);
        let de = descent::run(&p).unwrap();
        assert_eq!(de["verdict"], "NO");
        assert!(
            de["failures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["rule"] == 0),
            "{de}"
        );
    }

    #[test]
    fn planted_pole_is_caught() {
        let d = tmpdir();
        let mut rules = good_rules();
        // rule 1 loses its exception: its coefficient 1/(n1-1) has a pole at the selected keys I(1,1,n2)
        rules[1].exceptions.clear();
        let p = write_table(&d, "pole.jsonl", 3, "110", &rules, &["I(1,1,0)"]);
        let po = poles::run(&p, Some(6), None).unwrap();
        assert_eq!(po["verdict"], "NO");
        let v = po["violations"].as_array().unwrap();
        assert!(
            v.iter()
                .any(|x| x["rule"] == 1 && x["key"] == json!([1, 1, -1])),
            "{po}"
        );
        // a pole hidden by an EARLIER rule is not a violation
        let mut rules2 = good_rules();
        rules2[1].exceptions.clear();
        rules2.swap(1, 2);
        let p2 = write_table(&d, "pole2.jsonl", 3, "110", &rules2, &["I(1,1,0)"]);
        let po2 = poles::run(&p2, Some(6), None).unwrap();
        assert!(
            po2["violations"]
                .as_array()
                .unwrap()
                .iter()
                .all(|x| x["key"] != json!([1, 1, -1])),
            "{po2}"
        );
    }

    #[test]
    fn bivariate_quadratic_solver() {
        use crate::expr::{Expr, poly_vars, to_zpoly, zpoly_to_q};
        let vars = poly_vars(2);
        // 3x^2 - 7xy + 4y^2 - 3x + 6y - 2 = (3x - 4y + 6)(x - y - 3) + 16
        let e = Expr::parse("-2-3*n0-7*n0*n1+3*n0^2+6*n1+4*n1^2", 2).unwrap();
        let p = zpoly_to_q(&to_zpoly(&e.root, 2, &vars).unwrap());
        let zs = crate::cover::bivariate_quadratic_zeros(&p, &[0, 1]).unwrap();
        // brute force over a box
        let mut bf = vec![];
        for x in -200i64..=200 {
            for y in -200i64..=200 {
                if -2 - 3 * x - 7 * x * y + 3 * x * x + 6 * y + 4 * y * y == 0 {
                    bf.push((x, y));
                }
            }
        }
        assert_eq!(zs, bf);
        assert!(zs.contains(&(38, 34)) && zs.contains(&(83, 64)));
    }

    /// Random toy tables: the face recursion returns exactly the brute-force uncovered set.
    #[test]
    fn differential_random_tables() {
        let d = tmpdir();
        let mut rng = Rng::new(7);
        let n = 4;
        let mask = "1100";
        let polys = [
            "1-n0+n0*n1-n1", // (n0-1)(n1-1)
            "-6+n0^2+n1",    // nonlinear, few zeros
            "n2*n3-2",       // nonlinear
            "-n0+n1",        // multi-variable affine
            "-3+n0+n1",
            "n2-n3",
            "1+n2+2*n3",
        ];
        for case in 0..120 {
            let nr = rng.range(3, 9) as usize;
            let mut rules = vec![];
            for _ in 0..nr {
                let mut fixed = vec![None; n];
                #[allow(clippy::needless_range_loop)]
                for j in 0..n {
                    if rng.range(0, 2) == 0 {
                        fixed[j] = Some(if j < 2 {
                            rng.range(1, 3)
                        } else {
                            -rng.range(0, 2)
                        });
                    }
                }
                let mut exceptions = vec![];
                for _ in 0..rng.range(0, 3) {
                    let mut br = vec![];
                    for _ in 0..rng.range(1, 2) {
                        if rng.range(0, 3) == 0 {
                            br.push(
                                polys[rng.range(0, polys.len() as i64 - 1) as usize].to_string(),
                            );
                        } else {
                            let v = rng.range(0, n as i64 - 1) as usize;
                            let c = if v < 2 {
                                rng.range(1, 3)
                            } else {
                                -rng.range(0, 2)
                            };
                            br.push(format!("{}+n{v}", -c));
                        }
                    }
                    exceptions.push(br);
                }
                let equations = if rng.range(0, 6) == 0 {
                    vec!["-n0+n1"]
                } else {
                    vec![]
                };
                rules.push(ToyRule {
                    fixed,
                    equations,
                    exceptions,
                    rhs: vec![],
                });
            }
            let p = write_table(&d, &format!("rand{case}.jsonl"), n, mask, &rules, &[]);
            let r = 5;
            let (u, und) = cover_uncovered(&p, r as i128);
            assert_eq!(und, 0, "case {case}");
            assert_eq!(
                u,
                brute_uncovered(&p, r),
                "case {case}: face recursion differs from brute force"
            );
        }
    }
}
