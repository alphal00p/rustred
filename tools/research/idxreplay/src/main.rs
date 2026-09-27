//! idxreplay: offline replay of admission lookups against a CP5 checkpoint's
//! live candidate index (W0.4 of the fable_5_1 next-push plan).
//!
//! Modes:
//!   static  --ckpt DIR --gen G [--samples N] [--fracs 1024,512,256]
//!           [--threads 1,48,90] [--seconds S] [--out FILE] [--pairs P]
//!       Sampled requests (miss proxies: live IDs of the newest segment,
//!       self excluded; hit proxies: retired IDs of the two newest segments)
//!       on three layouts: today's (L0, stored and rebuilt), SoA in ID order,
//!       SoA ordered by finite-upper pattern then lexicographic lower corner.
//!       Policies min-ID / first-found, reverse checks, word-only passes,
//!       thinning fractions (per 1024) and thread counts.
//!   streams --ckpt DIR --gen G --trace DIR [--jobs N] [--out FILE]
//!       Real successor streams from an `admission-trace` run resumed from
//!       the same checkpoint: the resolution pipeline (exact, self, Local,
//!       per-job MRU k=1/4/16/64, helpers/orthants, layers, miss) per request.
//!   dynamic --trace DIR [--out FILE]
//!       Rebuild today's index from scratch in the coordinator's commit order
//!       of a traced run and check every admission's outcome and counters.
//!   lag     --ckpt DIR [--out FILE]
//!       Container age of every committed hit edge (stale-miss rate vs lag).
mod ckpt;
mod dynamic;
mod l0;
mod lag;
mod pipeline;
mod soa;
mod stat;
mod streams;
mod trace;
mod util;
mod world;

use std::collections::HashMap;

pub struct Args(HashMap<String, String>);
impl Args {
    fn parse() -> (String, Args) {
        let a: Vec<String> = std::env::args().collect();
        let mode = a.get(1).cloned().unwrap_or_default();
        let mut m = HashMap::new();
        let mut i = 2;
        while i < a.len() {
            let k = a[i].trim_start_matches("--").to_string();
            let v = a.get(i + 1).cloned().unwrap_or_default();
            m.insert(k, v);
            i += 2;
        }
        (mode, Args(m))
    }
    pub fn get(&self, k: &str) -> Option<&str> {
        self.0.get(k).map(|s| s.as_str())
    }
    pub fn req(&self, k: &str) -> &str {
        self.get(k).unwrap_or_else(|| panic!("missing --{k}"))
    }
    pub fn num(&self, k: &str, d: usize) -> usize {
        self.get(k).map_or(d, |v| v.parse().unwrap())
    }
    pub fn list(&self, k: &str, d: &str) -> Vec<usize> {
        self.get(k).unwrap_or(d).split(',').filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect()
    }
}

fn main() {
    #[cfg(target_arch = "x86_64")]
    assert!(
        std::arch::is_x86_feature_detected!("avx512bw") && std::arch::is_x86_feature_detected!("avx512vl"),
        "AVX-512BW/VL required"
    );
    let (mode, args) = Args::parse();
    match mode.as_str() {
        "static" => stat::run(&args),
        "streams" => streams::run(&args),
        "dynamic" => dynamic::run(&args),
        "lag" => lag::run(&args),
        _ => {
            eprintln!("usage: idxreplay static|streams|dynamic|lag --ckpt DIR --gen G ... (see src/main.rs)");
            std::process::exit(2);
        }
    }
}
