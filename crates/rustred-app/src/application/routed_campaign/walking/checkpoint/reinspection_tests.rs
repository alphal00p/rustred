//! W0.3 fixture extraction for the native re-inspection harness
//! (`walking/reinspection.rs`). The checkpoint is restored exactly as
//! `restore_copied_production_checkpoint` restores it (request and owner
//! binding, every section digest, decode and validation; no walk, no native
//! owner import, no save); then a fixture is written:
//!
//! - the resume argv and the inspection-relevant request fields;
//! - the owner payload digests the checkpoint bound (the harness refuses to
//!   inspect with any other owner bytes);
//! - the initial admission prefix (`initial_domain_count` domains, extended
//!   to the protected D-band prefix when `--reuse-initial-d-bands` is set):
//!   the only queue state `inspection::inspect` reads, through the indexes
//!   `execution::run_configured` builds from exactly these domains;
//! - the domains to inspect (`sample`), with their IDs and strata.
//!
//! Modes (`RUSTRED_REINSPECTION_MODE`):
//! - `pending` (default): a stratified uniform sample (bottom-k of a seeded
//!   hash per stratum) of native-pending IDs: not inspected, not published
//!   and not delegated. Strata: phase x owner class (hot owner or other) x
//!   ID tercile among pending IDs. Allocation per (phase, class) is split
//!   evenly across the terciles; every stratum's population is recorded so
//!   per-stratum weights can be applied.
//! - `inspected`: every natively inspected ID (dependency-monitor INSPECTED
//!   flag), for the differential proof against a tapped control walk.
//!
//! With `RUSTRED_REINSPECTION_INSPECTED_FIXTURE`, the same restore also
//! writes a second fixture (mode `inspected-sample`): the same stratified
//! sampler over natively inspected IDs (F10 re-inspection population; their
//! persisted records allow a native-stats cross-check).
//!
//! ```text
//! RUSTRED_CHECKPOINT_RESTORE_DIRECTORY=<copy of the checkpoint> \
//! RUSTRED_CHECKPOINT_RESTORE_REQUEST=<run>/request.json \
//! RUSTRED_CHECKPOINT_RESTORE_RECEIPT=<new file> \
//! [RUSTRED_CHECKPOINT_RESTORE_{MANIFEST,OWNER_BASE,QUERIES}=<identical copies>] \
//! RUSTRED_REINSPECTION_FIXTURE=<new file> [RUSTRED_REINSPECTION_MODE=pending|inspected] \
//! [RUSTRED_REINSPECTION_ALLOCATION=Apply/hot:3000,Apply/other:4000,Route/hot:1000,Route/other:2000] \
//! [RUSTRED_REINSPECTION_SEED=20260927] [RUSTRED_REINSPECTION_HOT_OWNER=011101110111000] \
//! [RUSTRED_REINSPECTION_INSPECTED_FIXTURE=<new file> [RUSTRED_REINSPECTION_INSPECTED_ALLOCATION=...]] \
//! cargo test --release -p rustred-app --lib reinspection_extract -- --ignored --nocapture
//! ```
use super::super::super::input;
use super::super::{
    OwnerDomainWalkRequest, mask,
    queue::{CompactDomain, Domain, Phase},
    reinspection::{FIXTURE_SCHEMA, domain_key, inspection_request},
};
use super::restore::Restored;
use super::scale_tests::{RestoredAnalysis, memory, restore_copied_checkpoint};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

/// Persisted node flags (`descendant_closure.rs`).
const SEALED: u8 = 1;
const INSPECTED: u8 = 2;

const DEFAULT_ALLOCATION: &str = "Apply/hot:3000,Apply/other:4000,Route/hot:1000,Route/other:2000";
const DEFAULT_HOT_OWNER: &str = "011101110111000";
const DEFAULT_SEED: u64 = 20_260_927;
const TERCILES: [&str; 3] = ["old", "mid", "young"];

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Apply => "Apply",
        Phase::Route => "Route",
    }
}

/// Bottom-k of a seeded hash per stratum (uniform without replacement).
/// Strata: phase x owner class (hot or other) x tercile of `ids` (in ID
/// order); `allocation` gives the count per "phase/class", split evenly
/// across the terciles. Returns the chosen (id, stratum), sorted by id, and a
/// report with each stratum's population, sample size and weight.
fn stratified<const N: usize>(
    domains: &[CompactDomain<N>],
    ids: &[usize],
    allocation: &[(String, usize)],
    hot: &str,
    seed: u64,
) -> (Vec<(usize, String)>, BTreeMap<String, u64>, Value) {
    let cuts = [ids.len() / 3, 2 * ids.len() / 3];
    let mut population: BTreeMap<String, u64> = BTreeMap::new();
    let mut heaps: BTreeMap<String, std::collections::BinaryHeap<(u64, usize)>> = BTreeMap::new();
    let quota = |phase: &str, class: &str, tercile: usize| -> usize {
        let total = allocation
            .iter()
            .find(|(k, _)| k == &format!("{phase}/{class}"))
            .map_or(0, |(_, v)| *v);
        total / 3 + usize::from(tercile < total % 3)
    };
    for (rank, &id) in ids.iter().enumerate() {
        let domain = &domains[id];
        let phase = phase_name(domain.phase());
        let class = if mask(&domain.owner()) == hot {
            "hot"
        } else {
            "other"
        };
        let tercile = usize::from(rank >= cuts[0]) + usize::from(rank >= cuts[1]);
        let stratum = format!("{phase}/{class}/{}", TERCILES[tercile]);
        *population.entry(stratum.clone()).or_default() += 1;
        let k = quota(phase, class, tercile);
        if k == 0 {
            continue;
        }
        let heap = heaps.entry(stratum).or_default();
        let hash = splitmix(seed ^ splitmix(id as u64));
        if heap.len() < k {
            heap.push((hash, id));
        } else if heap.peek().is_some_and(|&(top, _)| hash < top) {
            heap.pop();
            heap.push((hash, id));
        }
    }
    let mut chosen = Vec::new();
    let mut per_stratum = serde_json::Map::new();
    for (stratum, heap) in heaps {
        let mut picked: Vec<usize> = heap.into_iter().map(|(_, id)| id).collect();
        picked.sort_unstable();
        let size = population[&stratum];
        per_stratum.insert(
            stratum.clone(),
            json!({"population":size,"sampled":picked.len(),
                "weight":size as f64 / picked.len().max(1) as f64}),
        );
        chosen.extend(picked.into_iter().map(|id| (id, stratum.clone())));
    }
    chosen.sort_unstable();
    let report = json!({"ids":ids.len(),"tercile_cut_ids":cuts.iter().map(|&c| ids.get(c)).collect::<Vec<_>>(),
        "seed":seed,"hot_owner":hot,"allocation":allocation,"strata":per_stratum,
        "method":"bottom-k of splitmix64(seed ^ splitmix64(id)) per stratum (uniform without replacement); strata = phase x owner class x ID tercile of the eligible ID list"});
    (chosen, population, report)
}

struct Extract {
    fixture: PathBuf,
    mode: String,
    seed: u64,
    hot: String,
    allocation: Vec<(String, usize)>,
    /// Optional second fixture from the same restore: a stratified sample of
    /// natively inspected IDs (the F10 / stats cross-check population).
    inspected: Option<(PathBuf, Vec<(String, usize)>)>,
}

impl RestoredAnalysis for Extract {
    fn analyze<const N: usize>(
        &mut self,
        request: &OwnerDomainWalkRequest,
        _selection: &input::Selection,
        restored: &Restored<N>,
        receipt: &mut Value,
    ) -> Result<(), String> {
        let started = Instant::now();
        let state = &restored.state;
        let domains: &[CompactDomain<N>] = &state.queue.domains;
        let n = domains.len();
        let flags: Vec<u8> = state.closure.borrow().node_flags().collect();
        if flags.len() != n {
            return Err("dependency monitor does not cover the domain inventory".into());
        }
        let ledger = state.queue.delegation.as_ref();
        let initial_count = state.initial_domain_count;
        // run_configured: InitialOrthants from expand_prefix(initial_domain_count),
        // InitialOverlapIndex from expand_prefix(ledger.initial_prefix()).
        let overlap_prefix = if request.reuse_initial_d_bands {
            Some(
                ledger
                    .and_then(|l| l.initial_prefix())
                    .ok_or("initial D-band reuse requires a protected initial prefix")?,
            )
        } else {
            None
        };
        let prefix = initial_count.max(overlap_prefix.unwrap_or(0));
        let initial_domains: Vec<Value> = state
            .queue
            .expand_prefix(prefix)
            .iter()
            .map(|d| serde_json::to_value(&**d).map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        let inspected_ids: Vec<usize> = (0..n).filter(|&id| flags[id] & INSPECTED != 0).collect();

        let write = |path: &PathBuf,
                     mode: &str,
                     chosen: &[(usize, String)],
                     sampling: Value,
                     population: &BTreeMap<String, u64>,
                     receipt: &Value|
         -> Result<usize, String> {
            let mut sample = Vec::with_capacity(chosen.len());
            for (i, (id, stratum)) in chosen.iter().enumerate() {
                let domain: Domain<N> = domains[*id].expand();
                let status = match ledger {
                    None => "no_ledger",
                    Some(l) if l.is_published(*id) => "published",
                    Some(l) if l.delegated_to(*id).is_some() => "delegated_unpublished",
                    Some(l) if l.can_dispatch(*id) => "reserved",
                    Some(_) => "unreserved_or_started",
                };
                sample.push(
                    json!({"i":i,"id":id,"stratum":stratum,"key":domain_key(&domain),
                    "status":status,"flags":flags[*id],
                    "domain":serde_json::to_value(&domain).map_err(|e| e.to_string())?}),
                );
            }
            let fixture = json!({"schema":FIXTURE_SCHEMA,"arity":N,
                "argv":receipt["argv_resume"],"inspection_request":inspection_request(request),
                "owner_digests":receipt["owner_digests"],
                "initial_domain_count":initial_count,"overlap_prefix":overlap_prefix,
                "initial_domains":initial_domains,"mode":mode,"sampling":sampling,
                "strata_population":population,
                "provenance":{"directory":receipt["directory"],"generation":receipt["manifest"]["generation"],
                    "saved_executable_blake3":receipt["executable"]["saved_blake3"],
                    "domains":n,"inspected_ids":inspected_ids.len(),
                    "walk_semantics_version":receipt["manifest"]["walk_semantics_version"],
                    "extracted_by":std::env::current_exe().ok(),
                    "restore_receipt":std::env::var_os("RUSTRED_CHECKPOINT_RESTORE_RECEIPT").map(PathBuf::from)},
                "sample":sample});
            let bytes = serde_json::to_vec(&fixture).map_err(|e| e.to_string())?;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .and_then(|mut f| f.write_all(&bytes))
                .map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(bytes.len())
        };

        let (chosen, population, sampling) = match self.mode.as_str() {
            "inspected" => {
                let mut population = BTreeMap::new();
                let chosen: Vec<(usize, String)> = inspected_ids
                    .iter()
                    .map(|&id| {
                        let stratum = format!("{}/inspected", phase_name(domains[id].phase()));
                        *population.entry(stratum.clone()).or_default() += 1;
                        (id, stratum)
                    })
                    .collect();
                (
                    chosen,
                    population,
                    json!({"method":"every natively inspected ID"}),
                )
            }
            "pending" => {
                let pending = |id: usize| match ledger {
                    Some(l) => {
                        flags[id] & INSPECTED == 0
                            && !l.is_published(id)
                            && l.delegated_to(id).is_none()
                    }
                    None => flags[id] & (INSPECTED | SEALED) == 0,
                };
                let ids: Vec<usize> = (0..n).filter(|&id| pending(id)).collect();
                let (chosen, population, mut report) =
                    stratified(domains, &ids, &self.allocation, &self.hot, self.seed);
                report["eligible_definition"] = json!(
                    "native pending: closure INSPECTED flag clear, not published and not delegated in the ledger (Reserved and Unreserved/Started IDs); without a ledger: neither INSPECTED nor SEALED"
                );
                (chosen, population, report)
            }
            other => return Err(format!("unknown RUSTRED_REINSPECTION_MODE {other}")),
        };
        let bytes = write(
            &self.fixture,
            &self.mode,
            &chosen,
            sampling,
            &population,
            receipt,
        )?;
        let mut summary = json!({"fixture":self.fixture,"mode":self.mode,
            "fixture_bytes":bytes,"sample":chosen.len(),"initial_prefix":prefix,
            "overlap_prefix":overlap_prefix,"strata_population":population,
            "inspected_ids":inspected_ids.len()});
        if let Some((path, allocation)) = &self.inspected {
            let (chosen, population, mut report) = stratified(
                domains,
                &inspected_ids,
                allocation,
                &self.hot,
                self.seed ^ 0x5eed,
            );
            report["eligible_definition"] = json!("natively inspected: closure INSPECTED flag set");
            let bytes = write(
                path,
                "inspected-sample",
                &chosen,
                report,
                &population,
                receipt,
            )?;
            summary["inspected_fixture"] = json!({"fixture":path,"fixture_bytes":bytes,
                "sample":chosen.len(),"strata_population":population});
        }
        summary["seconds"] = json!(started.elapsed().as_secs_f64());
        receipt["analysis_summary"] = summary;
        receipt["memory"]["after_fixture"] = memory();
        Ok(())
    }
}

fn allocation(text: &str) -> Vec<(String, usize)> {
    text.split(',')
        .map(|part| {
            let (key, value) = part.split_once(':').expect("stratum:count");
            (key.to_owned(), value.parse().expect("count"))
        })
        .collect()
}

#[test]
#[ignore = "needs a copied CP5 checkpoint and its campaign request (see module docs)"]
fn reinspection_extract() {
    let fixture = PathBuf::from(
        std::env::var_os("RUSTRED_REINSPECTION_FIXTURE").expect("RUSTRED_REINSPECTION_FIXTURE"),
    );
    let mode = std::env::var("RUSTRED_REINSPECTION_MODE").unwrap_or_else(|_| "pending".into());
    let seed = std::env::var("RUSTRED_REINSPECTION_SEED")
        .ok()
        .map_or(DEFAULT_SEED, |s| s.parse().expect("seed"));
    let hot = std::env::var("RUSTRED_REINSPECTION_HOT_OWNER")
        .unwrap_or_else(|_| DEFAULT_HOT_OWNER.into());
    let inspected = std::env::var_os("RUSTRED_REINSPECTION_INSPECTED_FIXTURE").map(|path| {
        let text = std::env::var("RUSTRED_REINSPECTION_INSPECTED_ALLOCATION")
            .unwrap_or_else(|_| DEFAULT_ALLOCATION.into());
        (PathBuf::from(path), allocation(&text))
    });
    restore_copied_checkpoint(
        "reinspection_extract",
        "rustred.reinspection-extract.v1",
        "reinspection-extract-receipt",
        &mut Extract {
            fixture,
            mode,
            seed,
            hot,
            allocation: allocation(
                &std::env::var("RUSTRED_REINSPECTION_ALLOCATION")
                    .unwrap_or_else(|_| DEFAULT_ALLOCATION.into()),
            ),
            inspected,
        },
    );
}
