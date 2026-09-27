//! D6 (symmetry canonicalisation): realized merges on a CP5 checkpoint.
//!
//! For every Apply domain Q of owner o and every element s of the verified
//! literal automorphism group that stabilizes o (s != identity), test exactly
//! whether the image sQ (I(n) = I(sn)) is contained in
//!   (realized)     an admitted Apply domain of o with a smaller ID,
//!   (native)       a natively inspected Apply domain of o with a smaller ID,
//!   (upper bound)  any admitted Apply domain of o other than Q.
//! The identity element gives the plain-containment baseline (domains an
//! earlier admission already contained, which the legacy engine missed or
//! could not see), so "symmetry-only" merges exclude those. Counts are by
//! domain and weighted by the native seconds of natively inspected domains
//! (exact seconds-weighted shares over all natives, i.e. the quantity a PPS
//! draw over native seconds estimates).
//!
//! Optional Route part (--route-sample N): uniform sample of Route domains;
//! every non-identity family element maps a Route domain of mask m to the
//! Route bucket of mask s(m).
use crate::ckpt;
use crate::geom::{Dom, MAXN};
use crate::kd::{Filter, Tree};
use crate::recs;
use crate::tight::{self, Tight};
use crate::util::{self, Opts};
use rayon::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

struct Group {
    elems: Vec<Vec<usize>>,
    identity: usize,
    /// Owner bits -> stabilizer element indices (identity included).
    stab: HashMap<u16, Vec<usize>>,
}

fn mask_bits(m: &str) -> u16 {
    m.bytes()
        .enumerate()
        .fold(0u16, |a, (i, b)| if b == b'1' { a | 1 << i } else { a })
}

fn image_mask(src: &[usize], owner: u16, n: usize) -> u16 {
    (0..n).fold(0u16, |a, j| {
        if owner >> src[j] & 1 == 1 {
            a | 1 << j
        } else {
            a
        }
    })
}

fn load_group(path: &str, n: usize) -> Group {
    let v: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let elems: Vec<Vec<usize>> = v["group"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            e["source_for_target"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as usize)
                .collect()
        })
        .collect();
    assert!(elems.iter().all(|e| e.len() == n), "group arity");
    let identity = elems
        .iter()
        .position(|e| e.iter().enumerate().all(|(i, &s)| i == s))
        .expect("identity element");
    let mut stab = HashMap::new();
    for o in v["owners"].as_array().unwrap() {
        let bits = mask_bits(o["mask"].as_str().unwrap());
        let s: Vec<usize> = o["stabilizer"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap() as usize)
            .collect();
        // Re-check the stabilizer from the permutations themselves.
        for &e in &s {
            assert_eq!(
                image_mask(&elems[e], bits, n),
                bits,
                "stabilizer element does not fix the owner"
            );
        }
        let full: Vec<usize> = (0..elems.len())
            .filter(|&e| image_mask(&elems[e], bits, n) == bits)
            .collect();
        assert_eq!(full, s, "stabilizer list differs from recomputation");
        stab.insert(bits, s);
    }
    Group {
        elems,
        identity,
        stab,
    }
}

#[derive(Default, Clone, Copy)]
struct Flags {
    id_before: bool,
    id_other: bool,
    sym_before: bool,
    sym_native: bool,
    sym_other: bool,
}

#[derive(Default, Clone)]
struct Tally {
    domains: u64,
    natives: u64,
    seconds: f64,
    n: [u64; 7],
    s: [f64; 7],
    nat: [u64; 7],
}
// Index order of the flag columns in `Tally::n/s/nat`.
const COLS: [&str; 7] = [
    "identity_before",
    "identity_other",
    "sym_before",
    "sym_native_before",
    "sym_other",
    "sym_before_not_identity_before",
    "sym_other_not_identity_other",
];

impl Tally {
    fn add(&mut self, f: &Flags, native: bool, sec: f64) {
        self.domains += 1;
        if native {
            self.natives += 1;
            self.seconds += sec;
        }
        let v = [
            f.id_before,
            f.id_other,
            f.sym_before,
            f.sym_native,
            f.sym_other,
            f.sym_before && !f.id_before,
            f.sym_other && !f.id_other,
        ];
        for (c, &b) in v.iter().enumerate() {
            if b {
                self.n[c] += 1;
                if native {
                    self.nat[c] += 1;
                    self.s[c] += sec;
                }
            }
        }
    }
    fn merge(&mut self, o: &Tally) {
        self.domains += o.domains;
        self.natives += o.natives;
        self.seconds += o.seconds;
        for c in 0..7 {
            self.n[c] += o.n[c];
            self.s[c] += o.s[c];
            self.nat[c] += o.nat[c];
        }
    }
    fn json(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("domains".into(), json!(self.domains));
        m.insert("natives".into(), json!(self.natives));
        m.insert("native_seconds".into(), json!(self.seconds));
        for (c, name) in COLS.iter().enumerate() {
            m.insert(
                (*name).into(),
                json!({
                    "domains": self.n[c],
                    "domain_share": self.n[c] as f64 / self.domains.max(1) as f64,
                    "natives": self.nat[c],
                    "native_share": self.nat[c] as f64 / self.natives.max(1) as f64,
                    "native_seconds": self.s[c],
                    "seconds_share": if self.seconds > 0.0 { self.s[c] / self.seconds } else { 0.0 },
                }),
            );
        }
        Value::Object(m)
    }
}

fn build_tree(doms: &[Dom], ids: &[u32], native: &[bool], n: usize) -> Tree {
    let k = tight::key_len(n);
    let mut keys = vec![0i16; ids.len() * k];
    keys.par_chunks_mut(k)
        .zip(ids.par_iter())
        .for_each(|(key, &id)| tight::container_key(&doms[id as usize], n, key));
    let nat: Vec<bool> = ids.iter().map(|&id| native[id as usize]).collect();
    Tree::build(k, ids.to_vec(), nat, keys)
}

pub fn run(dir: &Path, opts: &Opts) {
    let t0 = std::time::Instant::now();
    let n_arity_override = opts.num("arity", 15usize);
    let ck = util::load(dir, opts, true);
    let n = ck.n;
    let _ = n_arity_override;
    assert!(n <= MAXN);
    let group = load_group(opts.get("group").expect("--group"), n);
    let out_path = opts.get("out").expect("--out");
    let doms = &ck.doms;
    let total = doms.len();
    // Native flag and seconds per ID.
    let native: Vec<bool> = ck.native_of.iter().map(|&r| r != u32::MAX).collect();
    let seconds: Vec<f32> = ck
        .native_of
        .iter()
        .map(|&r| {
            if r == u32::MAX {
                0.0
            } else {
                ck.recs[r as usize].seconds
            }
        })
        .collect();
    let kind: Vec<u8> = {
        let mut k = vec![2u8; total]; // 2 = no record (pending)
        for r in &ck.recs {
            let id = r.id as usize;
            if recs::is_native(r.kind) {
                k[id] = 0;
            } else if k[id] == 2 {
                k[id] = 1; // delegated / other committed record
            }
        }
        k
    };
    // Buckets.
    let mut apply: BTreeMap<u16, Vec<u32>> = BTreeMap::new();
    let mut route: BTreeMap<u16, Vec<u32>> = BTreeMap::new();
    for (id, d) in doms.iter().enumerate() {
        match d.phase {
            0 => apply.entry(d.owner).or_default().push(id as u32),
            1 => route.entry(d.owner).or_default().push(id as u32),
            _ => {}
        }
    }
    eprintln!(
        "apply buckets {} route buckets {} ({:.1} s)",
        apply.len(),
        route.len(),
        t0.elapsed().as_secs_f64()
    );
    for o in apply.keys() {
        assert!(
            group.stab.contains_key(o),
            "Apply owner {} missing from the group file",
            Dom {
                owner: *o,
                ..Default::default()
            }
            .owner_string(n)
        );
    }
    // Trees per Apply owner.
    let owners: Vec<u16> = apply.keys().copied().collect();
    let trees: Vec<Tree> = owners
        .par_iter()
        .map(|o| build_tree(doms, &apply[o], &native, n))
        .collect();
    let tree_of: HashMap<u16, usize> = owners.iter().enumerate().map(|(i, &o)| (o, i)).collect();
    eprintln!("apply trees built ({:.1} s)", t0.elapsed().as_secs_f64());

    let k = tight::key_len(n);
    let done = AtomicU64::new(0);
    let queries = AtomicU64::new(0);
    let empties = AtomicU64::new(0);
    let generation_of = |id: usize| util::id_gen(&ck.m, id);
    // Per (owner, kind, generation) tallies.
    let skip_apply = opts.get("skip-apply").is_some();
    // Per-ID flags (--flags-out): bit0 sym_before, bit1 sym_other, bit2
    // identity_other, bit3 sym_native_before, bit4 first sym hit in another
    // Route mask, bit7 evaluated.
    let flags: Vec<std::sync::atomic::AtomicU8> =
        (0..total).map(|_| std::sync::atomic::AtomicU8::new(0)).collect();
    let setf = |id: u32, f: &Flags, cross: bool| {
        let b = (f.sym_before as u8)
            | (f.sym_other as u8) << 1
            | (f.id_other as u8) << 2
            | (f.sym_native as u8) << 3
            | (cross as u8) << 4
            | 0x80;
        flags[id as usize].store(b, Ordering::Relaxed);
    };
    let per: Vec<(u16, BTreeMap<(u8, u64), Tally>)> = owners
        .par_iter()
        .filter(|_| !skip_apply)
        .map(|&o| {
            let tree = &trees[tree_of[&o]];
            let ids = &apply[&o];
            let stab = &group.stab[&o];
            let nontrivial: Vec<&Vec<usize>> = stab
                .iter()
                .filter(|&&e| e != group.identity)
                .map(|&e| &group.elems[e])
                .collect();
            let ident: Vec<usize> = (0..n).collect();
            let part: Vec<BTreeMap<(u8, u64), Tally>> = ids
                .par_chunks(4096)
                .map(|chunk| {
                    let mut stack = Vec::new();
                    let mut q = vec![0i16; k];
                    let mut local: BTreeMap<(u8, u64), Tally> = BTreeMap::new();
                    let mut nq = 0u64;
                    for &id in chunk {
                        let d = &doms[id as usize];
                        let t: Tight = tight::tight(d, n);
                        if t.empty {
                            empties.fetch_add(1, Ordering::Relaxed);
                        }
                        let mut f = Flags::default();
                        tight::query_key(&t, n, &ident, &mut q);
                        nq += 1;
                        f.id_before = tree.find(&q, Filter::Before(id), &mut stack).is_some();
                        f.id_other =
                            f.id_before || tree.find(&q, Filter::Other(id), &mut stack).is_some();
                        for src in &nontrivial {
                            tight::query_key(&t, n, src, &mut q);
                            nq += 1;
                            if !f.sym_native
                                && tree
                                    .find(&q, Filter::NativeBefore(id), &mut stack)
                                    .is_some()
                            {
                                f.sym_native = true;
                            }
                            if !f.sym_before
                                && (f.sym_native
                                    || tree.find(&q, Filter::Before(id), &mut stack).is_some())
                            {
                                f.sym_before = true;
                            }
                            if !f.sym_other
                                && (f.sym_before
                                    || tree.find(&q, Filter::Other(id), &mut stack).is_some())
                            {
                                f.sym_other = true;
                            }
                            if f.sym_native {
                                break;
                            }
                        }
                        setf(id, &f, false);
                        let key = (kind[id as usize], generation_of(id as usize));
                        local.entry(key).or_default().add(
                            &f,
                            native[id as usize],
                            seconds[id as usize] as f64,
                        );
                    }
                    queries.fetch_add(nq, Ordering::Relaxed);
                    let c =
                        done.fetch_add(chunk.len() as u64, Ordering::Relaxed) + chunk.len() as u64;
                    if c % (1 << 20) < chunk.len() as u64 {
                        eprintln!("  apply {c} domains ({:.1} s)", t0.elapsed().as_secs_f64());
                    }
                    local
                })
                .collect();
            let mut acc: BTreeMap<(u8, u64), Tally> = BTreeMap::new();
            for p in part {
                for (key, t) in p {
                    acc.entry(key).or_default().merge(&t);
                }
            }
            (o, acc)
        })
        .collect();
    eprintln!(
        "apply queries {} done ({:.1} s)",
        queries.load(Ordering::Relaxed),
        t0.elapsed().as_secs_f64()
    );

    let mut total_all = Tally::default();
    let mut total_sym_owners = Tally::default();
    let mut by_kind: BTreeMap<u8, Tally> = BTreeMap::new();
    let mut by_gen: BTreeMap<u64, Tally> = BTreeMap::new();
    let mut owner_rows = Vec::new();
    for (o, acc) in &per {
        let mut t = Tally::default();
        for ((kd, g), x) in acc {
            t.merge(x);
            by_kind.entry(*kd).or_default().merge(x);
            by_gen.entry(*g).or_default().merge(x);
        }
        total_all.merge(&t);
        let stab = &group.stab[o];
        if stab.len() > 1 {
            total_sym_owners.merge(&t);
        }
        let mut row = t.json();
        let obj = row.as_object_mut().unwrap();
        obj.insert(
            "owner".into(),
            json!(Dom {
                owner: *o,
                ..Default::default()
            }
            .owner_string(n)),
        );
        obj.insert("stabilizer_order".into(), json!(stab.len()));
        owner_rows.push(row);
    }
    let kind_names = ["native", "committed_non_native", "pending_no_record"];
    let mut out = json!({
        "schema": "rustred.wv.sym.v1",
        "checkpoint": dir.display().to_string(),
        "generation": ck.m.generation,
        "domains_total": total,
        "apply_domains": total_all.domains,
        "empty_apply_domains": empties.load(Ordering::Relaxed),
        "group_order": group.elems.len(),
        "apply_queries": queries.load(Ordering::Relaxed),
        "columns": COLS,
        "apply_all_owners": total_all.json(),
        "apply_owners_with_nontrivial_stabilizer": total_sym_owners.json(),
        "apply_by_kind": by_kind.iter().map(|(k, t)| (kind_names[*k as usize].to_string(), t.json())).collect::<serde_json::Map<_, _>>(),
        "apply_by_domain_generation": by_gen.iter().map(|(g, t)| (g.to_string(), t.json())).collect::<serde_json::Map<_, _>>(),
        "apply_per_owner": owner_rows,
        "seconds_elapsed_apply": t0.elapsed().as_secs_f64(),
    });

    // ---- Audit (sampled): kd-tree vs linear scan, and pointwise checks ----
    let audit_n: usize = opts.num("audit", 0usize);
    if audit_n > 0 {
        let pool: Vec<u32> = owners
            .iter()
            .filter(|o| group.stab[o].len() > 1)
            .flat_map(|o| apply[o].iter().copied())
            .collect();
        let mut s = opts.num("seed", 20260927u64) ^ 0x5bd1e995 | 1;
        let mut draw = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        let picks: Vec<(u32, usize)> = (0..audit_n)
            .map(|_| {
                let id = pool[(draw() % pool.len() as u64) as usize];
                let st: Vec<usize> = group.stab[&doms[id as usize].owner]
                    .iter()
                    .copied()
                    .filter(|&e| e != group.identity)
                    .collect();
                (id, st[(draw() % st.len() as u64) as usize])
            })
            .collect();
        let cap = opts.num("audit-cap", 200_000u128);
        // (agree, positives, pos_enum_ok, pos_enum_skipped, neg_pointwise_checked, neg_pointwise_ok)
        let res: Vec<(bool, bool, u8, bool, u8)> = picks
            .par_iter()
            .map(|&(id, e)| {
                let d = &doms[id as usize];
                let src = &group.elems[e];
                let t = tight::tight(d, n);
                let mut q = vec![0i16; k];
                tight::query_key(&t, n, src, &mut q);
                let mut stack = Vec::new();
                let tree = &trees[tree_of[&d.owner]];
                let got = tree.find(&q, Filter::Before(id), &mut stack);
                let mut key = vec![0i16; k];
                let linear = apply[&d.owner].iter().any(|&c| {
                    c < id && {
                        tight::container_key(&doms[c as usize], n, &mut key);
                        tight::dominated(&key, &q)
                    }
                });
                let img = tight::permute(d, n, src);
                let npts = crate::geom::points(&img, n);
                // pos: 0 = n/a, 1 = every point inside the found container, 2 = violation
                let mut pos = 0u8;
                let mut skipped = false;
                if let Some(c) = got {
                    match npts {
                        Some(p) if p <= cap => {
                            let cd = &doms[c as usize];
                            let mut ok = true;
                            crate::geom::enumerate(&img, n, &mut |_, x, sa, sr| {
                                ok &= crate::geom::contains_point(cd, n, x, sa + img.t(), sr);
                            });
                            pos = if ok { 1 } else { 2 };
                        }
                        _ => skipped = true,
                    }
                }
                // neg: 0 = n/a, 1 = no earlier container holds every point, 2 = one does
                let mut neg = 0u8;
                if got.is_none() && npts.is_some_and(|p| p <= 2000) {
                    let mut pts = Vec::new();
                    crate::geom::enumerate(&img, n, &mut |_, x, sa, sr| {
                        pts.push((*x, sa + img.t(), sr))
                    });
                    let hit = apply[&d.owner].iter().any(|&c| {
                        c < id
                            && pts.iter().all(|(x, a, r)| {
                                crate::geom::contains_point(&doms[c as usize], n, x, *a, *r)
                            })
                    });
                    neg = if hit { 2 } else { 1 };
                }
                (got.is_some() == linear, got.is_some(), pos, skipped, neg)
            })
            .collect();
        let cnt =
            |f: &dyn Fn(&(bool, bool, u8, bool, u8)) -> bool| res.iter().filter(|r| f(r)).count();
        out.as_object_mut().unwrap().insert(
            "audit".into(),
            json!({
                "sampled_pairs": res.len(),
                "tree_vs_linear_scan_agree": cnt(&|r| r.0),
                "positives": cnt(&|r| r.1),
                "positives_pointwise_verified": cnt(&|r| r.2 == 1),
                "positives_pointwise_violations": cnt(&|r| r.2 == 2),
                "positives_skipped_over_cap": cnt(&|r| r.3),
                "negatives_pointwise_checked": cnt(&|r| r.4 != 0),
                "negatives_pointwise_confirmed": cnt(&|r| r.4 == 1),
                "negatives_pointwise_contradicted": cnt(&|r| r.4 == 2),
                "pointwise_predicate": "geom::contains_point (census, independent of tight.rs) over geom::enumerate",
            }),
        );
        eprintln!("audit done ({:.1} s)", t0.elapsed().as_secs_f64());
    }

    // ---- Route (sampled) ----
    let sample: usize = opts.num("route-sample", 0usize);
    if sample > 0 || opts.get("route-all").is_some() {
        let rkeys: Vec<u16> = route.keys().copied().collect();
        let rtrees: Vec<Tree> = rkeys
            .par_iter()
            .map(|m| build_tree(doms, &route[m], &native, n))
            .collect();
        let rtree_of: HashMap<u16, usize> =
            rkeys.iter().enumerate().map(|(i, &m)| (m, i)).collect();
        eprintln!("route trees built ({:.1} s)", t0.elapsed().as_secs_f64());
        let all_route: Vec<u32> = rkeys
            .iter()
            .flat_map(|m| route[m].iter().copied())
            .collect();
        let mut s = opts.num("seed", 20260927u64) | 1;
        let mut draw = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        // --route-all: every Route domain (sample size ignored), no draws.
        let picks: Vec<u32> = if opts.get("route-all").is_some() {
            all_route.clone()
        } else {
            (0..sample)
                .map(|_| all_route[(draw() % all_route.len() as u64) as usize])
                .collect()
        };
        let ident: Vec<usize> = (0..n).collect();
        let res: Vec<(bool, bool, bool, bool, bool)> = picks
            .par_chunks(1024)
            .flat_map_iter(|chunk| {
                let mut stack = Vec::new();
                let mut q = vec![0i16; k];
                chunk
                    .iter()
                    .map(|&id| {
                        let d = &doms[id as usize];
                        let t = tight::tight(d, n);
                        tight::query_key(&t, n, &ident, &mut q);
                        let own = &rtrees[rtree_of[&d.owner]];
                        let idb = own.find(&q, Filter::Before(id), &mut stack).is_some();
                        let ido = idb || own.find(&q, Filter::Other(id), &mut stack).is_some();
                        let (mut sb, mut so, mut cross) = (false, false, false);
                        for (e, src) in group.elems.iter().enumerate() {
                            if e == group.identity {
                                continue;
                            }
                            let im = image_mask(src, d.owner, n);
                            let Some(&ti) = rtree_of.get(&im) else {
                                continue;
                            };
                            tight::query_key(&t, n, src, &mut q);
                            let tr = &rtrees[ti];
                            if !sb && tr.find(&q, Filter::Before(id), &mut stack).is_some() {
                                sb = true;
                                cross |= im != d.owner;
                            }
                            if !so && (sb || tr.find(&q, Filter::Other(id), &mut stack).is_some()) {
                                so = true;
                            }
                            if sb {
                                break;
                            }
                        }
                        setf(
                            id,
                            &Flags {
                                id_before: idb,
                                id_other: ido,
                                sym_before: sb,
                                sym_native: false,
                                sym_other: so,
                            },
                            cross,
                        );
                        (idb, ido, sb, so, cross)
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let c = |f: &dyn Fn(&(bool, bool, bool, bool, bool)) -> bool| {
            res.iter().filter(|r| f(r)).count()
        };
        let ns = res.len() as f64;
        out.as_object_mut().unwrap().insert(
            "route_sample".into(),
            json!({
                "route_domains": all_route.len(),
                "route_buckets": rkeys.len(),
                "sample": res.len(),
                "all_route_domains_evaluated": opts.get("route-all").is_some(),
                "seed": opts.num("seed", 20260927u64),
                "identity_before": c(&|r| r.0) as f64 / ns,
                "identity_other": c(&|r| r.1) as f64 / ns,
                "sym_before": c(&|r| r.2) as f64 / ns,
                "sym_other": c(&|r| r.3) as f64 / ns,
                "sym_before_not_identity_before": c(&|r| r.2 && !r.0) as f64 / ns,
                "sym_other_not_identity_other": c(&|r| r.3 && !r.1) as f64 / ns,
                "sym_before_first_hit_in_other_mask": c(&|r| r.4) as f64 / ns,
                "binomial_sigma_at_share_0_5": (0.25 / ns).sqrt(),
            }),
        );
    }
    out.as_object_mut()
        .unwrap()
        .insert("seconds_elapsed".into(), json!(t0.elapsed().as_secs_f64()));
    std::fs::write(out_path, serde_json::to_string_pretty(&out).unwrap()).unwrap();
    if let Some(fp) = opts.get("flags-out") {
        let bytes: Vec<u8> = flags.iter().map(|f| f.load(Ordering::Relaxed)).collect();
        std::fs::write(fp, bytes).unwrap();
        eprintln!("wrote flags {fp}");
    }
    let _ = ckpt::HEADER;
    eprintln!("wrote {out_path} ({:.1} s)", t0.elapsed().as_secs_f64());
}
