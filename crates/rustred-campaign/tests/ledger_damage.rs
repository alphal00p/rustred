//! Byte-level damage table (port of audit B experiment 2 with the NEW expected classes): a pristine ledger with
//! sealed segments, snapshots and a commit mark is damaged in many ways; for every case `open` must either refuse
//! with a typed error or open with EXACTLY the pristine history (a torn tail beyond the commit mark may be
//! recovered with a `recovery` record). Never: a silently altered or shortened acknowledged history.
//! `verify_chain` and `read_records` must never return an altered history either.
//!
//! Run: `cargo test --release -p rustred-campaign --test ledger_damage -- --ignored --nocapture`; a smoke subset
//! runs in debug.
mod ledger_support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use ledger_support::*;
use rustred_campaign::ledger::{self, Event, LedgerConfig, LedgerError};
use rustred_campaign::lock;
use rustred_campaign::state::EvidenceLevel;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn cfg() -> LedgerConfig {
    LedgerConfig {
        rotate_bytes: 8 * 1024,
        ..LedgerConfig::default()
    }
}

/// The pristine ledger: `n` item lives (all acknowledged: the last record is SYNC), several segments.
fn pristine(d: &Path, n: usize) -> (String, u64) {
    let clk = clock();
    let (lk, _) = lock::acquire(d, clk.as_ref(), &["damage".into()]).unwrap();
    let mut l = ledger::Ledger::create(d, &lk, as_dyn(&clk), cfg(), genesis()).unwrap();
    l.append_all(setup_events("T162")).unwrap();
    for k in 0..n {
        l.append_all(item_life("T162", k, 0, EvidenceLevel::IdentityTested, 10))
            .unwrap();
    }
    l.append(Event::Campaign {
        ev: rustred_campaign::state::CampaignEvent::Drain,
        reason: "end".into(),
    })
    .unwrap();
    (l.state().digest(), l.seq())
}

fn copy_dir(from: &Path, to: &Path) {
    let _ = fs::remove_dir_all(to);
    fs::create_dir_all(to.join("ledger")).unwrap();
    for e in fs::read_dir(from.join("ledger")).unwrap() {
        let p = e.unwrap().path();
        let q = to.join("ledger").join(p.file_name().unwrap());
        fs::copy(&p, &q).unwrap();
        fs::set_permissions(&q, fs::Permissions::from_mode(0o644)).unwrap();
    }
}

#[derive(Debug, PartialEq)]
enum Class {
    Refused,
    Exact,
    TornRecovered,
}

/// Opens the damaged copy and classifies; panics on a silently different history.
fn classify(d: &Path, pristine_digest: &str, pristine_seq: u64, what: &str) -> Class {
    let clk = clock();
    let (lk, _) = lock::acquire(d, clk.as_ref(), &["damage".into()]).unwrap();
    let r = ledger::Ledger::open(d, &lk, as_dyn(&clk), cfg());
    let class = match r {
        Err(
            LedgerError::Corrupt { .. }
            | LedgerError::Replay(_)
            | LedgerError::Io { .. }
            | LedgerError::Atomic(_)
            | LedgerError::NotALedger { .. },
        ) => Class::Refused,
        Err(e) => panic!("{what}: unexpected error {e}"),
        Ok((l, rep)) => match rep.recovery {
            None => {
                assert_eq!(
                    l.state().digest(),
                    pristine_digest,
                    "{what}: opened with a different history and no recovery"
                );
                Class::Exact
            }
            Some(rec) => {
                assert!(!rec.forced);
                assert_eq!(
                    rec.last_good_seq, pristine_seq,
                    "{what}: recovery dropped acknowledged records"
                );
                Class::TornRecovered
            }
        },
    };
    // the read-only paths never return an altered history either
    if class == Class::Refused {
        assert!(
            ledger::verify_chain(d).is_err(),
            "{what}: verify_chain accepts what open refused"
        );
    }
    class
}

fn active(d: &Path) -> PathBuf {
    let m = ledger::read_manifest(&ledger::ledger_dir(d)).unwrap();
    ledger::ledger_dir(d).join(m.active.file)
}

fn line_starts(b: &[u8]) -> Vec<usize> {
    let mut v = vec![0];
    for (i, c) in b.iter().enumerate() {
        if *c == b'\n' && i + 1 < b.len() {
            v.push(i + 1);
        }
    }
    v
}

fn run(full: bool) {
    let base = scratch("damage-pristine");
    let _c = Cleanup(base.clone());
    let (digest, seq) = pristine(&base, if full { 120 } else { 40 });
    let work = scratch("damage-work");
    let _cw = Cleanup(work.clone());
    let mut r = Rng(4242);
    let mut counts: std::collections::BTreeMap<String, (usize, usize, usize)> = Default::default();
    let mut tally = |name: &str, c: Class| {
        let e = counts.entry(name.to_string()).or_default();
        match c {
            Class::Refused => e.0 += 1,
            Class::Exact => e.1 += 1,
            Class::TornRecovered => e.2 += 1,
        }
    };
    let man = ledger::read_manifest(&ledger::ledger_dir(&base)).unwrap();
    assert!(man.sealed.len() >= 2, "sealed segments: {}", man.sealed.len());
    let reps = if full { 40 } else { 8 };
    // M1 truncate at every byte of the last 3 active lines (acknowledged): always refused
    {
        let b = fs::read(active(&base)).unwrap();
        let starts = line_starts(&b);
        let from = starts[starts.len().saturating_sub(3)];
        let step = if full { 1 } else { 7 };
        for cut in (from..b.len()).step_by(step) {
            copy_dir(&base, &work);
            fs::write(active(&work), &b[..cut]).unwrap();
            let c = classify(&work, &digest, seq, &format!("M1 cut {cut}"));
            assert_eq!(c, Class::Refused, "M1 cut {cut}: acknowledged bytes missing must refuse");
            tally("M1 truncate acknowledged tail", c);
        }
    }
    // M2 truncate a sealed segment; M3c flip a byte in a sealed segment
    for k in 0..reps {
        let s = &man.sealed[r.below(man.sealed.len())];
        copy_dir(&base, &work);
        let p = ledger::ledger_dir(&work).join(&s.file);
        let mut b = fs::read(&p).unwrap();
        if k % 2 == 0 {
            let n = b.len();
            b.truncate(r.below(n));
        } else {
            let i = r.below(b.len());
            b[i] ^= 1 << r.below(8);
        }
        fs::write(&p, &b).unwrap();
        let c = classify(&work, &digest, seq, &format!("M2/M3c sealed {k}"));
        assert_eq!(c, Class::Refused, "sealed damage must refuse at open");
        tally("M2/M3c sealed truncate / flip", c);
    }
    // M3a/b flips in the active segment, anywhere and in the last complete line
    for k in 0..2 * reps {
        copy_dir(&base, &work);
        let p = active(&work);
        let mut b = fs::read(&p).unwrap();
        let starts = line_starts(&b);
        let i = if k % 2 == 0 {
            r.below(b.len())
        } else {
            let s = *starts.last().unwrap();
            s + r.below(b.len() - s - 1)
        };
        b[i] ^= 1 << r.below(8);
        fs::write(&p, &b).unwrap();
        let c = classify(&work, &digest, seq, &format!("M3 active flip {k} at {i}"));
        assert_eq!(c, Class::Refused, "M3: a flipped acknowledged byte must refuse");
        tally("M3a/b active flip", c);
    }
    // M3d manifest flip, M3e snapshot flip: refused or exactly the pristine history
    for k in 0..reps {
        copy_dir(&base, &work);
        let ld = ledger::ledger_dir(&work);
        let p = if k % 2 == 0 {
            ld.join("MANIFEST.json")
        } else {
            ld.join(&man.snapshot.as_ref().unwrap().file)
        };
        let mut b = fs::read(&p).unwrap();
        let i = r.below(b.len());
        b[i] ^= 1 << r.below(8);
        fs::write(&p, &b).unwrap();
        let c = classify(&work, &digest, seq, &format!("M3d/e {k}"));
        tally("M3d/e manifest / snapshot flip", c);
    }
    // M4 delete / duplicate / swap active lines
    {
        let b = fs::read(active(&base)).unwrap();
        let lines: Vec<&[u8]> = b.split(|c| *c == b'\n').filter(|l| !l.is_empty()).collect();
        let n = lines.len();
        let variants: Vec<(String, Vec<usize>)> = vec![
            ("delete first".into(), (1..n).collect()),
            ("delete middle".into(), (0..n).filter(|i| *i != n / 2).collect()),
            ("delete last".into(), (0..n - 1).collect()),
            ("duplicate middle".into(), (0..n).flat_map(|i| if i == n / 2 { vec![i, i] } else { vec![i] }).collect()),
            ("duplicate last".into(), (0..n).chain(std::iter::once(n - 1)).collect()),
            ("swap".into(), (0..n).map(|i| if i == n / 2 { n / 2 + 1 } else if i == n / 2 + 1 { n / 2 } else { i }).collect()),
        ];
        for (name, idx) in variants {
            copy_dir(&base, &work);
            let mut out = Vec::new();
            for i in idx {
                out.extend_from_slice(lines[i]);
                out.push(b'\n');
            }
            fs::write(active(&work), &out).unwrap();
            let c = classify(&work, &digest, seq, &format!("M4 {name}"));
            assert_eq!(c, Class::Refused, "M4 {name}");
            tally("M4 active line edits", c);
        }
    }
    // M5 delete a sealed segment / the active segment; M7 empty files; M8 whitespace overwrite
    {
        let ld = |w: &Path| ledger::ledger_dir(w);
        let cases: Vec<(&str, Box<dyn Fn(&Path)>)> = vec![
            ("delete first sealed", Box::new(|w: &Path| fs::remove_file(ld(w).join(&man.sealed[0].file)).unwrap())),
            ("delete last sealed", Box::new(|w: &Path| fs::remove_file(ld(w).join(&man.sealed.last().unwrap().file)).unwrap())),
            ("delete active", Box::new(|w: &Path| fs::remove_file(active(w)).unwrap())),
            ("empty active", Box::new(|w: &Path| fs::write(active(w), b"").unwrap())),
            ("empty sealed", Box::new(|w: &Path| fs::write(ld(w).join(&man.sealed[0].file), b"").unwrap())),
            ("empty manifest", Box::new(|w: &Path| fs::write(ld(w).join("MANIFEST.json"), b"").unwrap())),
            ("empty commit mark", Box::new(|w: &Path| fs::write(ld(w).join(ledger::COMMIT_FILE), b"").unwrap())),
            ("delete commit mark", Box::new(|w: &Path| fs::remove_file(ld(w).join(ledger::COMMIT_FILE)).unwrap())),
            ("whitespace over active", Box::new(|w: &Path| {
                let p = active(w);
                let mut b = fs::read(&p).unwrap();
                let n = b.len();
                for x in &mut b[n / 3..n / 3 + 200] {
                    if *x != b'\n' {
                        *x = b' ';
                    }
                }
                fs::write(&p, &b).unwrap();
            })),
        ];
        for (name, f) in cases {
            copy_dir(&base, &work);
            f(&work);
            let c = classify(&work, &digest, seq, &format!("M5/M7/M8 {name}"));
            assert_eq!(c, Class::Refused, "{name}");
            tally("M5/M7/M8 deletions, empty files, overwrite", c);
        }
    }
    // M9 torn garbage BEYOND the commit mark: incomplete -> recovered; complete garbage line -> refused
    for (name, bytes, want) in [
        ("incomplete line", b"{\"schema\":\"rustred.campaign".to_vec(), Class::TornRecovered),
        ("NUL block", vec![0u8; 512], Class::TornRecovered),
        ("complete garbage line", b"garbage that is not a record\n".to_vec(), Class::Refused),
    ] {
        copy_dir(&base, &work);
        let p = active(&work);
        let mut b = fs::read(&p).unwrap();
        b.extend_from_slice(&bytes);
        fs::write(&p, &b).unwrap();
        let c = classify(&work, &digest, seq, &format!("M9 {name}"));
        assert_eq!(c, want, "M9 {name}");
        tally("M9 bytes beyond the commit mark", c);
    }
    eprintln!("damage table (refused / exact / torn-recovered):");
    for (k, (a, b, c)) in &counts {
        eprintln!("  {k}: {a} / {b} / {c}");
    }
}

#[test]
fn damage_smoke() {
    run(false);
}

#[test]
#[ignore = "long: full damage table (audit B experiment 2 port)"]
fn damage_table_release() {
    run(true);
}
