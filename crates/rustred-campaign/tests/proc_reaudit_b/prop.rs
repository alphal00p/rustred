//! Admission as a PURE function under timestamped inputs: property tests with a truth model.
use rustred_campaign::admission::{
    AdmissionPolicy, Candidate, Decision, GB, HostView, LowState, RunningJob, admit_one, choose_victims,
};
use std::collections::BTreeMap;
use std::time::Duration;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    fn range(&mut self, a: u64, b: u64) -> u64 {
        a + self.below(b - a + 1)
    }
    fn ms(&mut self, a: u64, b: u64) -> Duration {
        Duration::from_millis(self.range(a, b))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Cat {
    Never,
    FreshConsistent,
    StaleLow,
    SkewedOldLow,
    // categories the documentation does not claim to cover (reported separately):
    FutureLow,
    NewerThanHostGrew,
    OlderWithinSkewShrank,
}

fn safe_cat(c: Cat) -> bool {
    matches!(c, Cat::Never | Cat::FreshConsistent | Cat::StaleLow | Cat::SkewedOldLow)
}

pub fn main(a: &[String]) {
    let seed: u64 = a.first().and_then(|s| s.parse().ok()).unwrap_or(0x0ad3_1551_0000_0001);
    let cases: u64 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(200_000);
    println!("PROP seed={seed} cases={cases}");
    let mut r = Rng(seed);
    // ---------------------------------------------------------------- P1 floor with a truth model
    let mut admits = 0u64;
    let mut viol: BTreeMap<String, (u64, u128)> = BTreeMap::new();
    let mut safe_viol = 0u64;
    let mut stale_dep = 0u64;
    let mut stale_host_admit = 0u64;
    let mut mono_flip = 0u64;
    let mut cat_counts: BTreeMap<Cat, u64> = BTreeMap::new();
    for _ in 0..cases {
        let mut pol = AdmissionPolicy::new(64);
        pol.floor_bytes = if r.below(4) == 0 { r.range(0, 100) * GB } else { 50 * GB };
        if r.below(5) == 0 {
            pol.campaign_mem_cap_bytes = Some(r.range(1, 400) * GB);
        }
        let now = r.ms(100_000, 1_000_000);
        let host_age = if r.below(10) == 0 { r.ms(10_001, 30_000) } else { r.ms(0, 10_000) };
        let hm = now - host_age;
        let extreme = r.below(50) == 0;
        let avail = if extreme { u64::MAX - r.below(1000) } else { r.range(0, 400) * GB + r.below(GB) };
        let nj = r.below(9) as usize;
        let mut jobs = vec![];
        let mut cats = vec![];
        let mut true_out: u128 = 0;
        for id in 0..nj {
            let cap = if extreme && r.below(3) == 0 { u64::MAX / 4 } else { r.range(1, 100_000) * (GB / 1000) };
            let rss_true = r.below(cap.saturating_add(1));
            let pick = r.below(7);
            let (cat, at, meas): (Cat, Option<Duration>, u64) = match pick {
                0 => (Cat::Never, None, r.below(cap)),
                1 => {
                    let t = hm.saturating_sub(r.ms(0, 4_900));
                    (Cat::FreshConsistent, Some(t.max(now.saturating_sub(Duration::from_secs(10)))), rss_true)
                }
                2 => (Cat::StaleLow, Some(now.saturating_sub(r.ms(10_001, 60_000))), r.below(rss_true + 1)),
                3 => (Cat::SkewedOldLow, Some(hm.saturating_sub(r.ms(5_001, 9_000))), r.below(rss_true + 1)),
                4 => (Cat::FutureLow, Some(now + r.ms(1, 20_000)), r.below(rss_true + 1)),
                5 => {
                    let t = hm + r.ms(0, host_age.as_millis() as u64);
                    (Cat::NewerThanHostGrew, Some(t), rss_true + r.below(cap - rss_true + 1))
                }
                _ => (Cat::OlderWithinSkewShrank, Some(hm.saturating_sub(r.ms(0, 4_900))), {
                    // the job shrank between its sample and the host reading: it measured MORE than it uses at hm
                    rss_true + r.below(cap - rss_true + 1)
                }),
            };
            // FreshConsistent must really be fresh: within max age and skew
            jobs.push(RunningJob {
                id: id as u64,
                cap_bytes: cap,
                expected_peak_bytes: cap,
                rss_bytes: meas,
                rss_measured_at: at,
                cores: 1,
                started_seq: id as u64,
                cpu_seconds: 0.0,
                stopping: false,
            });
            *cat_counts.entry(cat).or_default() += 1;
            cats.push(cat);
            true_out += u128::from(cap - rss_true);
        }
        let ccap = if extreme && r.below(2) == 0 { u64::MAX - r.below(100) } else { r.range(1, 100_000) * (GB / 1000) };
        let cand = Candidate { id: 99, expected_peak_bytes: ccap / 2, cap_bytes: ccap, cores: 1, bypassed: 0 };
        let host = HostView { mem_available_bytes: avail, measured_at: hm, now };
        let d = admit_one(&pol, host, &jobs, &cand);
        if d == Decision::Admit {
            admits += 1;
            let truth = i128::try_from(u128::from(avail)).unwrap() - i128::try_from(true_out).unwrap() - i128::from(ccap);
            if truth < i128::from(pol.floor_bytes) {
                let short = (i128::from(pol.floor_bytes) - truth) as u128;
                if cats.iter().all(|c| safe_cat(*c)) {
                    safe_viol += 1;
                    println!("  SAFE-CATEGORY VIOLATION: host={host:?} jobs={jobs:?} cand={cand:?} floor={}", pol.floor_bytes);
                } else {
                    let key = cats.iter().filter(|c| !safe_cat(**c)).map(|c| format!("{c:?}")).collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>().join("+");
                    let e = viol.entry(key).or_default();
                    e.0 += 1;
                    e.1 = e.1.max(short);
                }
            }
            if host_age > pol.max_sample_age {
                stale_host_admit += 1;
            }
        }
        // stale measurements: the decision must not depend on their value
        let mut alt = jobs.clone();
        let mut any_stale = false;
        for (j, c) in alt.iter_mut().zip(&cats) {
            if matches!(c, Cat::StaleLow | Cat::SkewedOldLow | Cat::Never) {
                j.rss_bytes = r.below(j.cap_bytes.saturating_add(1));
                any_stale = true;
            }
        }
        if any_stale && admit_one(&pol, host, &alt, &cand) != d {
            stale_dep += 1;
        }
        // monotone: turning a fresh measurement stale never turns Wait into Admit
        if d != Decision::Admit && !jobs.is_empty() {
            let mut st = jobs.clone();
            let k = r.below(st.len() as u64) as usize;
            st[k].rss_measured_at = Some(now.saturating_sub(Duration::from_secs(60)));
            if admit_one(&pol, host, &st, &cand) == Decision::Admit {
                mono_flip += 1;
            }
        }
    }
    println!("PROP-P1 floor/truth: admissions={admits} violations_in_documented_categories={safe_viol} stale_value_changed_decision={stale_dep} admit_on_stale_host={stale_host_admit} fresh->stale_flip_to_admit={mono_flip}");
    println!("PROP-P1 measurement categories generated: {cat_counts:?}");
    for (k, (n, worst)) in &viol {
        println!("PROP-P1 UNDOCUMENTED-CATEGORY violations [{k}]: {n} cases, worst shortfall below the floor {:.2} GB", *worst as f64 / 1e9);
    }
    // ---------------------------------------------------------------- P2/P3 emergency: persistence and minimal victims
    let (mut plans, mut early_victims, mut not_minimal, mut stopping_chosen, mut uncovered, mut whole_wrong, mut inflight_extra, mut zero_rss) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for _ in 0..cases {
        let mut pol = AdmissionPolicy::new(64);
        pol.victim_order = match r.below(3) {
            0 => rustred_campaign::admission::VictimOrder::NewestFirst,
            1 => rustred_campaign::admission::VictimOrder::LargestRssFirst,
            _ => rustred_campaign::admission::VictimOrder::LeastCpuFirst,
        };
        let now = r.ms(20_000, 100_000);
        let avail = r.below(pol.floor_bytes + 10 * GB);
        let host = HostView { mem_available_bytes: avail, measured_at: now, now };
        let low = LowState { samples: r.below(5) as u32, since: now.saturating_sub(r.ms(0, 12_000)) + if r.below(10) == 0 { r.ms(1, 3000) } else { Duration::ZERO } };
        let nj = r.below(10) as usize;
        let jobs: Vec<RunningJob> = (0..nj)
            .map(|i| RunningJob {
                id: i as u64,
                cap_bytes: 100 * GB,
                expected_peak_bytes: 100 * GB,
                rss_bytes: if r.below(6) == 0 { 0 } else { r.range(1, 60_000) * (GB / 1000) },
                rss_measured_at: Some(now),
                cores: 1,
                started_seq: r.below(1000),
                cpu_seconds: r.below(10_000) as f64,
                stopping: r.below(5) == 0,
            })
            .collect();
        let Some(p) = choose_victims(&pol, host, &jobs, low) else {
            assert!(avail >= pol.floor_bytes);
            continue;
        };
        plans += 1;
        assert!(p.pause_launching);
        let persisted = now.saturating_sub(low.since) >= pol.emergency_persist;
        if (low.samples < pol.emergency_after_samples || !persisted) && !p.victims.is_empty() {
            early_victims += 1;
        }
        let byid = |id: u64| jobs.iter().find(|j| j.id == id).unwrap();
        if p.victims.iter().any(|v| byid(*v).stopping) {
            stopping_chosen += 1;
        }
        if p.victims.iter().any(|v| byid(*v).rss_bytes == 0) {
            zero_rss += 1;
        }
        let target = (pol.floor_bytes - avail) + pol.emergency_margin_bytes;
        let inflight: u64 = jobs.iter().filter(|j| j.stopping).map(|j| j.rss_bytes).sum();
        let holders: Vec<&RunningJob> = jobs.iter().filter(|j| j.rss_bytes > 0 && !j.stopping).collect();
        let freed: u64 = p.victims.iter().map(|v| byid(*v).rss_bytes).sum();
        if !p.victims.is_empty() {
            if inflight >= target {
                inflight_extra += 1;
            }
            if freed + inflight >= target {
                for v in &p.victims {
                    if freed - byid(*v).rss_bytes + inflight >= target {
                        not_minimal += 1;
                        break;
                    }
                }
            } else if p.victims.len() != holders.len() {
                uncovered += 1;
            }
            let whole = !holders.is_empty() && p.victims.len() == holders.len();
            if p.whole_campaign != whole {
                whole_wrong += 1;
            }
        } else if low.samples >= pol.emergency_after_samples && persisted && inflight < target && !holders.is_empty() {
            uncovered += 1;
        }
    }
    println!("PROP-P2/P3 emergency: plans={plans} victims_before_persistence={early_victims} not_inclusion_minimal={not_minimal} stopping_job_chosen={stopping_chosen} zero_rss_victim={zero_rss} deficit_not_covered_but_not_all_holders={uncovered} whole_campaign_flag_wrong={whole_wrong} victims_although_inflight_covers={inflight_extra}");
    // ---------------------------------------------------------------- P4 memory freed LATE (victims stay in `running` with stopping)
    let episodes = (cases / 10).max(1);
    let (mut extra_kills_ok, mut extra_kills_forgetful, mut total_first) = (0u64, 0u64, 0u64);
    for e in 0..episodes {
        let pol = AdmissionPolicy::new(64);
        let nj = r.range(1, 8) as usize;
        let mut jobs: Vec<RunningJob> = (0..nj)
            .map(|i| RunningJob {
                id: i as u64,
                cap_bytes: 100 * GB,
                expected_peak_bytes: 100 * GB,
                rss_bytes: r.range(1, 30) * GB,
                rss_measured_at: Some(Duration::ZERO),
                cores: 1,
                started_seq: i as u64,
                cpu_seconds: 0.0,
                stopping: false,
            })
            .collect();
        let deficit = r.range(1, 40) * GB;
        let delay = r.range(1, 6);
        for forgetful in [false, true] {
            let mut js = jobs.clone();
            let mut avail = pol.floor_bytes - deficit;
            let mut killed: Vec<(u64, u64)> = vec![]; // (id, tick)
            let mut first = 0usize;
            for tick in 0..20u64 {
                let now = Duration::from_secs(tick * 2);
                // memory of victims comes back `delay` ticks after the kill
                for (id, t) in killed.iter() {
                    if *t + delay == tick
                        && let Some(pos) = js.iter().position(|j| j.id == *id)
                    {
                        avail = avail.saturating_add(js[pos].rss_bytes);
                        js.remove(pos);
                    }
                }
                let host = HostView { mem_available_bytes: avail, measured_at: now, now };
                let low = LowState { samples: 2 + tick as u32, since: Duration::ZERO };
                let Some(p) = choose_victims(&pol, host, &js, LowState { samples: low.samples, since: Duration::ZERO }) else { break };
                if tick == 0 || first == 0 {
                    first = first.max(p.victims.len());
                }
                for v in p.victims {
                    killed.push((v, tick));
                    if forgetful {
                        // a caller that drops victims from `running` at once (memory not freed yet)
                        js.retain(|j| j.id != v);
                    } else if let Some(j) = js.iter_mut().find(|j| j.id == v) {
                        j.stopping = true;
                    }
                }
            }
            let total = killed.len();
            if !forgetful {
                total_first += first as u64;
                if total > first {
                    extra_kills_ok += 1;
                    if extra_kills_ok <= 3 {
                        println!("  P4 extra kills although victims were kept as stopping: episode {e} first={first} total={total}");
                    }
                }
            } else if total > first {
                extra_kills_forgetful += 1;
            }
        }
        jobs.clear();
    }
    println!("PROP-P4 memory freed late ({episodes} episodes): extra kills with victims kept in `running` as stopping={extra_kills_ok}; extra kills when the caller drops victims from `running` at once={extra_kills_forgetful} (API contract); first-plan victims total={total_first}");
}
