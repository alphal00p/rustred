//! Lane t266run, DATA TOOLING (ignored): converts the coordinator lane t266-close's native evidence files into the
//! import's evidence forms (no mathematics here; the evidence store re-checks every term list against OUR
//! instantiation when it is built, interfaces/evidence-store.md 3):
//! - facts TSV (`label TAB key TAB flags TAB fnv64(all term keys) TAB n_terms TAB zero idx|- TAB unknown idx|- TAB
//!   record_sha256`; flags 1 in_case, 2 chart_ok, 4 branch null, 8 regular, 16 no error, 32 hidden, 64 irregular) ->
//!   `rustred.campaign.fact-verdict.v1` INDEX FORM lines;
//! - certification TSV (`label TAB record_sha256 TAB key TAB status`) -> compact proof verdict lines.
//!
//! Env: `T266RUN_CONVERT_FACTS` (a facts TSV), `T266RUN_CONVERT_CERT` (a directory of certification TSVs),
//! `T266RUN_CONVERT_OUT` (output directory). Node names here are test data (L8).

use serde_json::json;
use std::io::{BufRead, BufWriter, Write};
use std::path::PathBuf;

fn list(s: &str) -> Vec<u64> {
    if s == "-" || s.is_empty() {
        return vec![];
    }
    s.split(',').map(|x| x.parse().expect("index")).collect()
}

fn key(s: &str) -> Vec<i64> {
    s.split(',').map(|x| x.parse().expect("key slot")).collect()
}

#[test]
#[ignore = "data tooling: converts t266-close's native evidence files (needs the env variables)"]
fn convert_t266_close_native_evidence() {
    let out = PathBuf::from(std::env::var("T266RUN_CONVERT_OUT").expect("T266RUN_CONVERT_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    if let Ok(facts) = std::env::var("T266RUN_CONVERT_FACTS") {
        let rd = std::io::BufReader::with_capacity(1 << 22, std::fs::File::open(&facts).unwrap());
        let name = PathBuf::from(&facts)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let op = out.join(format!("facts-{name}.jsonl"));
        let mut w = BufWriter::with_capacity(1 << 22, std::fs::File::create(&op).unwrap());
        let (mut n, mut usable) = (0u64, 0u64);
        for l in rd.lines() {
            let l = l.unwrap();
            if l.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = l.split('\t').collect();
            assert_eq!(f.len(), 8, "facts line {}: {l}", n + 1);
            let flags: u32 = f[2].parse().unwrap();
            let ok = flags & (1 | 2 | 4 | 16) == (1 | 2 | 4 | 16);
            let verdict = if !ok {
                format!("unusable:flags={flags}")
            } else if flags & 8 != 0 {
                "regular".into()
            } else if flags & 64 != 0 {
                "irregular".into()
            } else if flags & 32 != 0 {
                "hidden".into()
            } else {
                "unknown".into()
            };
            usable += ok as u64;
            let v = json!({"schema":"rustred.campaign.fact-verdict.v1","index": f[0].parse::<i64>().unwrap(),
                "record_sha256": f[7], "key": key(f[1]), "verdict": verdict,
                "n_terms": f[4].parse::<u64>().unwrap(), "zero_idx": list(f[5]), "unknown_idx": list(f[6]),
                "terms_fnv64": f[3], "producer": "rr_proof instantiate (t266-close facts, native TSV)"});
            writeln!(w, "{v}").unwrap();
            n += 1;
        }
        w.flush().unwrap();
        println!("T266RUN CONVERT facts {facts}: {n} lines ({usable} usable) -> {}", op.display());
    }
    if let Ok(cert) = std::env::var("T266RUN_CONVERT_CERT") {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&cert)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "tsv"))
            .collect();
        files.sort();
        let op = out.join("proofs-cert.jsonl");
        let mut w = BufWriter::with_capacity(1 << 22, std::fs::File::create(&op).unwrap());
        let mut n = 0u64;
        for p in &files {
            for l in std::fs::read_to_string(p).unwrap().lines() {
                if l.trim().is_empty() {
                    continue;
                }
                let f: Vec<&str> = l.split('\t').collect();
                assert_eq!(f.len(), 4, "{}: {l}", p.display());
                let v = json!({"record_sha256": f[1], "index": f[0].parse::<i64>().unwrap(), "key": key(f[2]),
                    "verdict": f[3], "producer": "rr_proof instance-span (t266-close cert, native TSV)"});
                writeln!(w, "{v}").unwrap();
                n += 1;
            }
        }
        w.flush().unwrap();
        println!(
            "T266RUN CONVERT cert {cert}: {} files, {n} lines -> {}",
            files.len(),
            op.display()
        );
    }
}
