// Best route witness per record: compose the saved map with every owner
// momentum automorphism and count source ISPs whose image is a single
// inactive owner slot (literal), an active owner slot, or a non-slot form.
use std::collections::HashMap;
use std::fs;

type V = [f64; 5];
type M5 = [[f64; 5]; 5];

fn rowmul(v: &V, m: &M5) -> V {
    let mut o = [0.0; 5];
    for j in 0..5 {
        for k in 0..5 {
            o[j] += v[k] * m[k][j];
        }
    }
    o
}
fn matmul(a: &M5, b: &M5) -> M5 {
    let mut o = [[0.0; 5]; 5];
    for i in 0..5 {
        o[i] = rowmul(&a[i], b);
    }
    o
}
fn inv(a: &M5) -> Option<M5> {
    let mut m = [[0.0f64; 10]; 5];
    for i in 0..5 {
        for j in 0..5 {
            m[i][j] = a[i][j];
        }
        m[i][5 + i] = 1.0;
    }
    for c in 0..5 {
        let p = (c..5).max_by(|&x, &y| m[x][c].abs().partial_cmp(&m[y][c].abs()).unwrap())?;
        if m[p][c].abs() < 1e-9 {
            return None;
        }
        m.swap(c, p);
        let pv = m[c][c];
        for j in 0..10 {
            m[c][j] /= pv;
        }
        for r in 0..5 {
            if r != c && m[r][c].abs() > 0.0 {
                let f = m[r][c];
                for j in 0..10 {
                    m[r][j] -= f * m[c][j];
                }
            }
        }
    }
    let mut o = [[0.0; 5]; 5];
    for i in 0..5 {
        for j in 0..5 {
            o[i][j] = m[i][5 + j];
        }
    }
    Some(o)
}
fn key(v: &V) -> Option<[i32; 5]> {
    let mut k = [0i32; 5];
    for i in 0..5 {
        let r = v[i].round();
        if (v[i] - r).abs() > 1e-7 {
            return None;
        }
        k[i] = r as i32;
    }
    Some(k)
}

fn main() {
    let text = fs::read_to_string("routes.txt").unwrap();
    let mut lines = text.lines();
    let slots: Vec<V> = {
        let n: Vec<f64> = lines.next().unwrap().split_whitespace().map(|x| x.parse().unwrap()).collect();
        (0..15).map(|i| [n[5 * i], n[5 * i + 1], n[5 * i + 2], n[5 * i + 3], n[5 * i + 4]]).collect()
    };
    let mut slot_of: HashMap<[i32; 5], usize> = HashMap::new();
    for (i, s) in slots.iter().enumerate() {
        slot_of.insert(key(s).unwrap(), i);
        let neg: V = [-s[0], -s[1], -s[2], -s[3], -s[4]];
        slot_of.insert(key(&neg).unwrap(), i);
    }
    let bits = |m: &str| -> Vec<bool> { m.chars().map(|c| c == '1').collect() };
    // monomial index for (a<=b)
    let mono = |a: usize, b: usize| -> usize { let (a, b) = if a <= b { (a, b) } else { (b, a) }; let mut idx = 0; for x in 0..a { idx += 5 - x; } idx + (b - a) };
    let quad = |q: &V| -> [f64; 15] { let mut o = [0.0; 15]; for a in 0..5 { for b in 0..5 { o[mono(a, b)] += q[a] * q[b]; } } o };
    // Q: rows = slot squares in monomial coordinates; need coefficients x with x . Q = quad(q)
    let qm: Vec<[f64; 15]> = slots.iter().map(|c| quad(c)).collect();
    let qinv: Vec<[f64; 15]> = {
        let mut m = vec![[0.0f64; 30]; 15];
        for i in 0..15 { for j in 0..15 { m[i][j] = qm[i][j]; } m[i][15 + i] = 1.0; }
        for c in 0..15 { let p = (c..15).max_by(|&x, &y| m[x][c].abs().partial_cmp(&m[y][c].abs()).unwrap()).unwrap(); assert!(m[p][c].abs() > 1e-9, "slot squares not a basis"); m.swap(c, p); let pv = m[c][c]; for j in 0..30 { m[c][j] /= pv; } for r in 0..15 { if r != c { let f = m[r][c]; if f != 0.0 { for j in 0..30 { m[r][j] -= f * m[c][j]; } } } } }
        (0..15).map(|i| { let mut r = [0.0; 15]; for j in 0..15 { r[j] = m[i][15 + j]; } r }).collect()
    };
    // x = quad(q) . Qinv  (row vector)
    let expand = |q: &V| -> [f64; 15] { let y = quad(q); let mut x = [0.0; 15]; for j in 0..15 { for k in 0..15 { x[j] += y[k] * qinv[k][j]; } } x };
    // automorphisms per owner mask
    let mut auts: HashMap<String, Vec<M5>> = HashMap::new();
    let records: Vec<Vec<String>> = lines.map(|l| l.split_whitespace().map(|s| s.to_string()).collect()).collect();
    let mut owners: Vec<String> = records.iter().map(|r| r[1].clone()).collect();
    owners.sort();
    owners.dedup();
    for o in &owners {
        let act: Vec<usize> = bits(o).iter().enumerate().filter(|x| *x.1).map(|x| x.0).collect();
        let actset: Vec<bool> = bits(o);
        // greedy basis
        let mut basis: Vec<usize> = Vec::new();
        for &i in &act {
            let mut cand: Vec<V> = basis.iter().map(|&b| slots[b]).collect();
            cand.push(slots[i]);
            // rank via gaussian elimination
            let mut m = cand.clone();
            let mut r = 0;
            for c in 0..5 {
                if let Some(p) = (r..m.len()).find(|&k| m[k][c].abs() > 1e-9) {
                    m.swap(r, p);
                    for k in 0..m.len() {
                        if k != r {
                            let f = m[k][c] / m[r][c];
                            for j in 0..5 {
                                m[k][j] -= f * m[r][j];
                            }
                        }
                    }
                    r += 1;
                }
            }
            if r == cand.len() {
                basis.push(i);
            }
            if basis.len() == 5 {
                break;
            }
        }
        if basis.len() < 5 {
            auts.insert(o.clone(), vec![]);
            continue;
        }
        let bm: M5 = [slots[basis[0]], slots[basis[1]], slots[basis[2]], slots[basis[3]], slots[basis[4]]];
        let binv = inv(&bm).unwrap();
        let coef: Vec<(usize, V)> = act.iter().map(|&j| (j, rowmul(&slots[j], &binv))).collect();
        let signed: Vec<(usize, f64)> = act.iter().flat_map(|&i| [(i, 1.0), (i, -1.0)]).collect();
        let mut found: Vec<M5> = Vec::new();
        let mut assign: Vec<(usize, f64)> = Vec::new();
        fn rec(
            k: usize,
            assign: &mut Vec<(usize, f64)>,
            signed: &[(usize, f64)],
            coef: &[(usize, V)],
            slots: &[V],
            slot_of: &HashMap<[i32; 5], usize>,
            actset: &[bool],
            binv: &M5,
            act: &[usize],
            found: &mut Vec<M5>,
        ) {
            if k == 5 {
                let bp: M5 = std::array::from_fn(|r| {
                    let (i, s) = assign[r];
                    std::array::from_fn(|c| s * slots[i][c])
                });
                let t = matmul(binv, &bp);
                let mut seen = vec![false; 15];
                for &j in act {
                    let Some(kk) = key(&rowmul(&slots[j], &t)) else { return };
                    let Some(&m) = slot_of.get(&kk) else { return };
                    if !actset[m] || seen[m] {
                        return;
                    }
                    seen[m] = true;
                }
                found.push(t);
                return;
            }
            for &(i, s) in signed {
                if assign.iter().any(|a| a.0 == i) {
                    continue;
                }
                assign.push((i, s));
                let mut ok = true;
                for (_, a) in coef {
                    if (k + 1..5).all(|m| a[m].abs() < 1e-9) {
                        let mut v = [0.0; 5];
                        for (m, &(i2, s2)) in assign.iter().enumerate() {
                            for c in 0..5 {
                                v[c] += a[m] * s2 * slots[i2][c];
                            }
                        }
                        match key(&v).and_then(|kk| slot_of.get(&kk)) {
                            Some(&t) if actset[t] => {}
                            _ => {
                                ok = false;
                                break;
                            }
                        }
                    }
                }
                if ok {
                    rec(k + 1, assign, signed, coef, slots, slot_of, actset, binv, act, found);
                }
                assign.pop();
            }
        }
        rec(0, &mut assign, &signed, &coef, &slots, &slot_of, &actset, &binv, &act, &mut found);
        let lit = found.iter().filter(|t| (0..15).filter(|j| !actset[*j]).all(|j| key(&rowmul(&slots[j], t)).and_then(|k| slot_of.get(&k).copied()).is_some_and(|m| !actset[m]))).count();
        let partial: usize = found.iter().map(|t| (0..15).filter(|j| !actset[*j]).filter(|&j| key(&rowmul(&slots[j], t)).and_then(|k| slot_of.get(&k).copied()).is_some_and(|m| !actset[m])).count()).max().unwrap_or(0);
        eprintln!("AUT\t{o}\t{}\t{}\t{}\t{}", act.len(), found.len(), lit, partial);
        auts.insert(o.clone(), found);
    }
    // evaluate routes
    let mut tot = [0f64; 8];
    println!("source\towner\tt\ttraffic\trr\tsaved_cancel_support\tsaved_cancel_terms\tsaved_nonliteral\tbest_cancel_support\tbest_cancel_terms\tbest_nonliteral\tautomorphisms");
    for r in &records {
        let (src, own) = (&r[0], &r[1]);
        let traffic: f64 = r[2].parse().unwrap();
        let rrc: f64 = r[3].parse().unwrap();
        let nums: Vec<f64> = r[4..54].iter().map(|x| x.parse().unwrap()).collect();
        let a_s: M5 = std::array::from_fn(|i| std::array::from_fn(|j| nums[5 * i + j]));
        let a_o: M5 = std::array::from_fn(|i| std::array::from_fn(|j| nums[25 + 5 * i + j]));
        let t_route = matmul(&a_s, &inv(&a_o).unwrap());
        let sb = bits(src);
        let ob = bits(own);
        let isp: Vec<usize> = (0..15).filter(|&j| !sb[j]).collect();
        let classify = |t: &M5| -> Option<(usize, usize, usize)> {
            // actives must map to owner actives
            for j in 0..15 {
                if sb[j] {
                    let img = key(&rowmul(&slots[j], t)).and_then(|k| slot_of.get(&k).copied());
                    if !img.is_some_and(|m| ob[m]) {
                        return None;
                    }
                }
            }
            let mut support = vec![false; 15];
            let mut terms = 0usize;
            for &j in &isp {
                let x = expand(&rowmul(&slots[j], t));
                for m in 0..15 { if ob[m] && x[m].abs() > 1e-7 { support[m] = true; terms += 1; } }
            }
            let (mut lit, mut non) = (0, 0);
            for &j in &isp { match key(&rowmul(&slots[j], t)).and_then(|k| slot_of.get(&k).copied()) { Some(m) if !ob[m] => lit += 1, _ => non += 1 } }
            let _ = lit;
            Some((support.iter().filter(|x| **x).count(), terms, non))
        };
        let saved = classify(&t_route).expect("saved route maps actives");
        let mut best = saved;
        let list = &auts[own];
        for a in list {
            let t = matmul(&t_route, a);
            if let Some(c) = classify(&t) {
                if (c.0, c.1) < (best.0, best.1) {
                    best = c;
                }
            }
        }
        println!("{src}\t{own}\t{}\t{traffic}\t{rrc}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", sb.iter().filter(|x| **x).count(), saved.0, saved.1, saved.2, best.0, best.1, best.2, list.len());
        tot[0] += traffic;
        tot[1] += traffic * saved.0 as f64;
        tot[2] += traffic * best.0 as f64;
        if saved.0 == 0 { tot[3] += traffic; }
        if best.0 == 0 { tot[4] += traffic; }
        tot[5] += rrc;
    }
    eprintln!("traffic {} | traffic-weighted cancellation support saved {:.3} best {:.3} | literal traffic share saved {:.4} best {:.4}", tot[0], tot[1] / tot[0], tot[2] / tot[0], tot[3] / tot[0], tot[4] / tot[0]);
}
