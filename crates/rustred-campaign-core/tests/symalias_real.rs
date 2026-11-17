//! Lane t162face: one-term symmetry aliases for gap pieces on REAL tables (ignored; run by hand).
//!
//!   SYMALIAS_HEADER_TABLE=<table whose header gives frame, order and family_source (the keys' frame)>
//!   SYMALIAS_PIECES=<pieces.json: [{"name", "fixed", "equations", "domain_bounds"}]>
//!   SYMALIAS_OUT=<directory>   [SYMALIAS_FIRST_INDEX=<index of the first written record, default 0>]
//!   cargo test --release --test symalias_real -- --ignored --exact symalias_on_real_pieces
//!
//! The group is DERIVED from the header's family_source and proved exactly (`replay_base::symrows::verify_derived`);
//! every candidate's engine `sym` record is verified independently (`import::symmetry::verify_sym_record`, steps 1-6);
//! descent of each alias on the piece's whole domain by `parametric::permuted` (via `parametric::symalias`).
//! Output: OUT/candidates.jsonl (header, one line per candidate, summary), OUT/aliases.table.jsonl (the header table's
//! header line verbatim, its chart line, the CHOSEN alias rule records = first candidate in group order that is exact
//! and descends, one per piece, an end record), OUT/aliases.sym.table.jsonl (the same as engine `sym` records, for the
//! engine's replay check), OUT/summary.json.

use std::io::{BufRead, Write};
use std::path::PathBuf;

use rustred_campaign_core::import::symmetry::verify_sym_record;
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::loci::contain::ProofBudget;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::parametric::symalias::{
    Piece, SYMALIAS_SCHEMA, SYMALIAS_VERSION, candidates,
};
use rustred_campaign_core::record::{
    ReadOptions, RecordReader, RhsMode, TableLine, parse_sym_line,
};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{DeriveOptions, verify_derived};
use serde_json::{Value, json};

fn sha256_bytes(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

#[test]
#[ignore]
fn symalias_on_real_pieces() {
    let hp = PathBuf::from(std::env::var("SYMALIAS_HEADER_TABLE").expect("SYMALIAS_HEADER_TABLE"));
    let pp = PathBuf::from(std::env::var("SYMALIAS_PIECES").expect("SYMALIAS_PIECES"));
    let out = PathBuf::from(std::env::var("SYMALIAS_OUT").expect("SYMALIAS_OUT"));
    let first: i64 = std::env::var("SYMALIAS_FIRST_INDEX")
        .ok()
        .map(|s| s.parse().unwrap())
        .unwrap_or(0);
    std::fs::create_dir_all(&out).unwrap();
    // header line + chart line of the header table (streamed: only the first lines are read)
    let f = std::io::BufReader::new(std::fs::File::open(&hp).unwrap());
    let mut lines = f.lines();
    let hline = lines.next().unwrap().unwrap();
    let mut chart_line: Option<String> = None;
    for l in lines.take(8) {
        let l = l.unwrap();
        if serde_json::from_str::<Value>(&l)
            .ok()
            .and_then(|v| v.get("type").and_then(Value::as_str).map(str::to_string))
            .as_deref()
            == Some("chart")
        {
            chart_line = Some(l);
            break;
        }
    }
    let hv: Value = serde_json::from_str(&hline).unwrap();
    let mut rd = RecordReader::new(
        std::io::Cursor::new(hline.clone().into_bytes()),
        ReadOptions { rhs: RhsMode::Skip },
    );
    let th = match rd.next() {
        Some(Ok(TableLine::Header(h))) => h,
        other => panic!("first line of {} is not a header: {other:?}", hp.display()),
    };
    let frame = NodeFrame::from_header(&th).unwrap();
    let cmp = NodeComparator::from_header(&hv).unwrap();
    let n = frame.arity();
    let chart: Vec<bool> = (0..n).map(|s| frame.is_line(s)).collect();
    let fam_text = hv["family_source"]
        .as_str()
        .expect("header without family_source");
    let family = Family::parse(fam_text).unwrap();
    let t0 = std::time::Instant::now();
    let vg = verify_derived(&family, &chart, &DeriveOptions::default()).unwrap();
    let group_s = t0.elapsed().as_secs_f64();
    let pieces: Vec<Piece> = serde_json::from_str(&std::fs::read_to_string(&pp).unwrap()).unwrap();
    let header = json!({"schema": SYMALIAS_SCHEMA, "version": SYMALIAS_VERSION, "kind": "header",
        "header_table": hp.display().to_string(), "header_line_sha256": sha256_bytes(hline.as_bytes()),
        "pieces": pp.display().to_string(), "pieces_sha256": sha256_bytes(&std::fs::read(&pp).unwrap()),
        "frame": frame.mask_string(), "comparator": cmp.name(), "family_source_sha256": sha256_bytes(fam_text.as_bytes()),
        "group": {"digest": vg.digest(), "family_digest": vg.family_digest(), "provenance": format!("{:?}", vg.provenance()),
                  "elements": vg.group().elements.len(), "seconds": group_s}});
    let mut cf =
        std::io::BufWriter::new(std::fs::File::create(out.join("candidates.jsonl")).unwrap());
    writeln!(cf, "{header}").unwrap();
    let budget = ProofBudget::default();
    let mut chosen = Vec::new();
    let mut per_piece = Vec::new();
    for (pi, p) in pieces.iter().enumerate() {
        let idx = first + pi as i64;
        let cs = candidates(vg.group(), p, &frame, &cmp, &budget, idx).unwrap();
        let mut pick: Option<usize> = None;
        let mut rows = Vec::new();
        for (ci, c) in cs.iter().enumerate() {
            let line = serde_json::to_vec(&c.sym).unwrap();
            let exact = match parse_sym_line(&line, 1) {
                Ok(s) => verify_sym_record(&s, &family, &vg, &chart),
                Err(e) => Err(format!("{e:?}")),
            };
            let ok = exact.is_ok() && c.shippable();
            if ok && pick.is_none() {
                pick = Some(ci);
            }
            let row = json!({"kind": "candidate", "piece": p.name, "element": c.element, "target": c.target,
                "image": c.image, "descent": c.descent, "fixed_points": c.fixed_points, "exact": match &exact { Ok(e) => json!({"ok": true, "group_element": e}),
                Err(m) => json!({"ok": false, "reason": m}) }, "accepted": ok});
            writeln!(cf, "{row}").unwrap();
            rows.push(row);
        }
        per_piece.push(json!({"piece": p.name, "pure_elements": cs.len(),
            "lower": cs.iter().filter(|c| c.lower()).count(), "shippable": cs.iter().filter(|c| c.shippable()).count(), "chosen_element": pick.map(|i| cs[i].element)}));
        if let Some(i) = pick {
            chosen.push(cs[i].clone());
        }
    }
    let summary = json!({"kind": "summary", "pieces": pieces.len(), "served": chosen.len(), "per_piece": per_piece});
    writeln!(cf, "{summary}").unwrap();
    cf.flush().unwrap();
    let write_table = |name: &str, recs: Vec<&Value>| {
        let mut w = std::io::BufWriter::new(std::fs::File::create(out.join(name)).unwrap());
        writeln!(w, "{hline}").unwrap();
        if let Some(c) = &chart_line {
            writeln!(w, "{c}").unwrap();
        }
        for r in &recs {
            writeln!(w, "{}", serde_json::to_string(r).unwrap()).unwrap();
        }
        writeln!(
            w,
            "{}",
            json!({"type": "end", "charts": 1, "rules": recs.len(), "residuals": 0})
        )
        .unwrap();
        w.flush().unwrap();
    };
    write_table(
        "aliases.table.jsonl",
        chosen.iter().map(|c| &c.rule).collect(),
    );
    write_table(
        "aliases.sym.table.jsonl",
        chosen.iter().map(|c| &c.sym).collect(),
    );
    std::fs::write(
        out.join("summary.json"),
        serde_json::to_string_pretty(&json!({"header": header, "summary": summary})).unwrap(),
    )
    .unwrap();
    eprintln!("{} pieces, {} served", pieces.len(), chosen.len());
}
