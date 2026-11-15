//! Shared helpers of `tests/k3_oracle.rs` (not a test crate itself): locating the frozen k3 reference tree, a
//! pure-Rust SHA-256 so that every file read is checked against the tree's own `SHA256SUMS`, a small gzip/DEFLATE
//! decoder (RFC 1951/1952, with the CRC-32 and size trailer checked) for the two `.jsonl.gz` files, and the loaders
//! of the rule sources the dispatch tests need (the frozen tree holds verdicts, not rule domains).
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rustred_campaign_core::cover::Table;
use rustred_campaign_core::domain::{DomainOptions, TargetSource};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::record::{ReadOptions, read_bank_predicates, read_table_file};
use serde_json::Value;

/// Workspace root of the p-integral effort (the frozen tree and the handoff copies live under it).
pub const WS: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev";
/// Default location of the frozen tree (used when `RUSTRED_K3_FIXTURES` is unset).
pub const DEFAULT_K3: &str =
    "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/fixtures/helper-k3-20261113";
/// Read-only handoff copy of the checker's sources (rule domains, span source lines).
pub const HO: &str =
    "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/handoff/sym-investigation-for-ben";
/// sha256 of `RULE-PREDICATES2440.json` (the 2,440 qualified T162 bank rules), as in the k3 README / F1 provenance.
pub const RULE_PREDICATES_SHA: &str =
    "0cd3d2cab85a30d9cb46155a1c913b8c21a3495529895e0564c23bc0fb4718f2";

// ------------------------------------------------------------------------------------------------ SHA-256

const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// SHA-256 of `data` as lowercase hex (FIPS 180-4).
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    let mut w = [0u32; 64];
    for chunk in msg.as_chunks::<64>().0 {
        for (i, wi) in w.iter_mut().take(16).enumerate() {
            *wi = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K256[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

// ------------------------------------------------------------------------------------------------ gzip / DEFLATE

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (n, t) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *t = c;
    }
    let mut c = !0u32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    bit: u32,
}

impl Bits<'_> {
    fn bit(&mut self) -> u32 {
        let b = (self.d[self.pos] >> self.bit) & 1;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.pos += 1;
        }
        b as u32
    }
    fn bits(&mut self, n: u32) -> u32 {
        let mut v = 0;
        for i in 0..n {
            v |= self.bit() << i;
        }
        v
    }
    fn align(&mut self) {
        if self.bit != 0 {
            self.bit = 0;
            self.pos += 1;
        }
    }
}

/// Canonical Huffman code (puff-style decoding: counts per length and symbols in canonical order).
struct Huff {
    count: [u16; 16],
    symbol: Vec<u16>,
}

impl Huff {
    fn new(lengths: &[u8]) -> Huff {
        let mut count = [0u16; 16];
        for &l in lengths {
            count[l as usize] += 1;
        }
        count[0] = 0;
        let mut offs = [0u16; 16];
        for l in 1..16 {
            offs[l] = offs[l - 1] + count[l - 1];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Huff { count, symbol }
    }
    fn decode(&self, br: &mut Bits) -> usize {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= br.bit() as i32;
            let count = self.count[len] as i32;
            if code - count < first {
                return self.symbol[(index + code - first) as usize] as usize;
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        panic!("inflate: invalid Huffman code");
    }
}

const LBASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEXT: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn inflate_block(br: &mut Bits, out: &mut Vec<u8>, lit: &Huff, dist: &Huff) {
    loop {
        let sym = lit.decode(br);
        if sym < 256 {
            out.push(sym as u8);
        } else if sym == 256 {
            return;
        } else {
            let s = sym - 257;
            assert!(s < 29, "inflate: bad length symbol");
            let len = LBASE[s] as usize + br.bits(LEXT[s] as u32) as usize;
            let ds = dist.decode(br);
            assert!(ds < 30, "inflate: bad distance symbol");
            let d = DBASE[ds] as usize + br.bits(DEXT[ds] as u32) as usize;
            assert!(d <= out.len(), "inflate: distance too far back");
            let start = out.len() - d;
            for i in 0..len {
                let b = out[start + i];
                out.push(b);
            }
        }
    }
}

/// Raw DEFLATE stream -> bytes; returns the bytes and the number of input bytes consumed.
fn inflate(data: &[u8]) -> (Vec<u8>, usize) {
    let mut br = Bits {
        d: data,
        pos: 0,
        bit: 0,
    };
    let mut out = Vec::with_capacity(data.len() * 8);
    loop {
        let last = br.bit();
        match br.bits(2) {
            0 => {
                br.align();
                let p = br.pos;
                let len = u16::from_le_bytes([data[p], data[p + 1]]) as usize;
                let nlen = u16::from_le_bytes([data[p + 2], data[p + 3]]) as usize;
                assert_eq!(len, !nlen & 0xffff, "inflate: stored block length check");
                out.extend_from_slice(&data[p + 4..p + 4 + len]);
                br.pos = p + 4 + len;
            }
            1 => {
                let mut l = [0u8; 288];
                l[..144].fill(8);
                l[144..256].fill(9);
                l[256..280].fill(7);
                l[280..].fill(8);
                inflate_block(&mut br, &mut out, &Huff::new(&l), &Huff::new(&[5u8; 30]));
            }
            2 => {
                let nlen = br.bits(5) as usize + 257;
                let ndist = br.bits(5) as usize + 1;
                let ncode = br.bits(4) as usize + 4;
                const ORDER: [usize; 19] = [
                    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                ];
                let mut cl = [0u8; 19];
                for &o in ORDER.iter().take(ncode) {
                    cl[o] = br.bits(3) as u8;
                }
                let ch = Huff::new(&cl);
                let mut lengths = Vec::with_capacity(nlen + ndist);
                while lengths.len() < nlen + ndist {
                    let sym = ch.decode(&mut br);
                    match sym {
                        0..=15 => lengths.push(sym as u8),
                        16 => {
                            let prev = *lengths
                                .last()
                                .expect("inflate: repeat with no previous length");
                            let n = 3 + br.bits(2);
                            lengths.extend(std::iter::repeat_n(prev, n as usize));
                        }
                        17 => {
                            let n = 3 + br.bits(3);
                            lengths.extend(std::iter::repeat_n(0, n as usize));
                        }
                        _ => {
                            let n = 11 + br.bits(7);
                            lengths.extend(std::iter::repeat_n(0, n as usize));
                        }
                    }
                }
                assert_eq!(lengths.len(), nlen + ndist, "inflate: code lengths overrun");
                let lit = Huff::new(&lengths[..nlen]);
                let dist = Huff::new(&lengths[nlen..]);
                inflate_block(&mut br, &mut out, &lit, &dist);
            }
            _ => panic!("inflate: invalid block type 3"),
        }
        if last == 1 {
            break;
        }
    }
    br.align();
    (out, br.pos)
}

/// gunzip a single-member gzip file; the CRC-32 and ISIZE trailer are checked.
pub fn gunzip(gz: &[u8]) -> Vec<u8> {
    assert!(
        gz.len() > 18 && gz[0] == 0x1f && gz[1] == 0x8b && gz[2] == 8,
        "not a gzip/deflate file"
    );
    let flg = gz[3];
    let mut p = 10;
    if flg & 4 != 0 {
        let xlen = u16::from_le_bytes([gz[p], gz[p + 1]]) as usize;
        p += 2 + xlen;
    }
    for bit in [8u8, 16] {
        if flg & bit != 0 {
            while gz[p] != 0 {
                p += 1;
            }
            p += 1;
        }
    }
    if flg & 2 != 0 {
        p += 2;
    }
    let (out, used) = inflate(&gz[p..]);
    let t = p + used;
    let crc = u32::from_le_bytes([gz[t], gz[t + 1], gz[t + 2], gz[t + 3]]);
    let isize = u32::from_le_bytes([gz[t + 4], gz[t + 5], gz[t + 6], gz[t + 7]]);
    assert_eq!(crc, crc32(&out), "gzip CRC-32 mismatch");
    assert_eq!(isize, out.len() as u32, "gzip ISIZE mismatch");
    out
}

// ------------------------------------------------------------------------------------------------ the oracle tree

/// The frozen k3 tree with its `SHA256SUMS`; every read is verified.
pub struct Oracle {
    pub root: PathBuf,
    pub sums: Vec<(String, String)>,
}

/// Locate the frozen tree: `RUSTRED_K3_FIXTURES`, else the default path if it exists, else `None` after printing why
/// (the caller then skips). A set variable that points nowhere is an error, not a skip.
pub fn oracle() -> Option<Oracle> {
    let root = match std::env::var_os("RUSTRED_K3_FIXTURES") {
        Some(v) => {
            let p = PathBuf::from(v);
            assert!(
                p.join("SHA256SUMS").is_file(),
                "RUSTRED_K3_FIXTURES={} has no SHA256SUMS",
                p.display()
            );
            p
        }
        None => {
            let p = PathBuf::from(DEFAULT_K3);
            if !p.join("SHA256SUMS").is_file() {
                println!(
                    "SKIP: RUSTRED_K3_FIXTURES is unset and the default frozen tree {DEFAULT_K3} does not exist"
                );
                return None;
            }
            p
        }
    };
    let text = std::fs::read_to_string(root.join("SHA256SUMS")).expect("read SHA256SUMS");
    let sums = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let (h, f) = l
                .split_once("  ")
                .expect("SHA256SUMS line format `hash  path`");
            (h.to_string(), f.to_string())
        })
        .collect();
    Some(Oracle { root, sums })
}

impl Oracle {
    pub fn expected_sha(&self, rel: &str) -> &str {
        &self
            .sums
            .iter()
            .find(|(_, f)| f == rel)
            .unwrap_or_else(|| panic!("{rel} is not listed in SHA256SUMS"))
            .0
    }
    /// Raw bytes of `rel`, after checking its sha256 against `SHA256SUMS` (fails loudly on any change).
    pub fn bytes(&self, rel: &str) -> Vec<u8> {
        let b = std::fs::read(self.root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let got = sha256_hex(&b);
        assert_eq!(
            got,
            self.expected_sha(rel),
            "frozen tree changed: sha256 of {rel} differs from SHA256SUMS"
        );
        b
    }
    /// Text of `rel` (gunzipped if it ends in `.gz`), sha256-verified.
    pub fn text(&self, rel: &str) -> String {
        let b = self.bytes(rel);
        let b = if rel.ends_with(".gz") { gunzip(&b) } else { b };
        String::from_utf8(b).expect("utf-8")
    }
    pub fn json(&self, rel: &str) -> Value {
        serde_json::from_str(&self.text(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }
    pub fn jsonl(&self, rel: &str) -> Vec<Value> {
        self.text(rel)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{rel}: {e}")))
            .collect()
    }
}

pub fn as_key(v: &Value) -> Key {
    v.as_array()
        .expect("key array")
        .iter()
        .map(|x| x.as_i64().expect("integer") as i32)
        .collect()
}

pub fn keys(v: &Value) -> Vec<Key> {
    v.as_array().expect("key list").iter().map(as_key).collect()
}

/// Box enumerated with the crate: dots 0..=4, rank up to `max_r(dots)`, line power cap.
pub fn box_keys(
    frame: &NodeFrame,
    max_r: impl Fn(u32) -> u32,
    max_line_power: Option<i32>,
) -> Vec<Key> {
    let mut k = vec![];
    for d in 0..=4 {
        for r in 0..=max_r(d) {
            k.extend(frame.chart_layer(d, r, max_line_power));
        }
    }
    k.sort();
    k
}

/// T162 box B (presented frame, 9 lines): line power <= 5, D <= 4, r <= 1 if D = 0 else r <= 2.
pub fn t162_box_b() -> Vec<Key> {
    box_keys(
        &NodeFrame::without_header_lines_first(20, 9),
        |d| if d == 0 { 1 } else { 2 },
        Some(5),
    )
}

/// T178 order B box (family frame, 10 lines): D <= 4, r <= 2 if D = 0 else r <= 3.
pub fn t178_box() -> Vec<Key> {
    box_keys(
        &NodeFrame::without_header_lines_first(20, 10),
        |d| if d == 0 { 2 } else { 3 },
        None,
    )
}

// ------------------------------------------------------------------------------------------------ rule sources

fn verified_path(p: &Path, sha: &str) -> PathBuf {
    let b = std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    assert_eq!(
        sha256_hex(&b),
        sha,
        "rule source {} changed (sha256)",
        p.display()
    );
    p.to_path_buf()
}

/// The caller's statement for the T162 bank (no target field): every predicate says
/// `no_recenter_or_permutation_applied: true` and is written on the presented slots.
pub const T162_BANK_TARGETS: TargetSource = TargetSource::CanonicalByConstruction {
    attestation: "T162 bank RULE-PREDICATES2440: written on the presented slots, no recenter or permutation applied",
};

/// The caller's statement for the T178B case exports (no target field): exports of engine-written table_dump.v1
/// tables, whose targets the node driver canonicalises to zero offsets (driver.rs, recenter canonicalisation); the
/// two raw tables at hand (R0_try1, rgq-g0-10) have canonical targets on all 1,015 records (tests/cover_raw_tables.rs).
pub const T178B_EXPORT_TARGETS: TargetSource = TargetSource::CanonicalByConstruction {
    attestation: "T178B case exports of engine table_dump.v1 tables (targets canonicalised by the node driver)",
};

/// The 2,440 qualified T162 bank rules as one table labelled by `global_ordinal0`, ascending (so first match =
/// smallest covering id). Source: `RUSTRED_CAMPAIGN_FIXTURES/T162/RULE-PREDICATES2440.json`, else the handoff copy
/// `HO/astra-uncovered-domain-T162/RULE-PREDICATES2440.json`; sha256-checked. `None` (with a printed reason) if absent.
pub fn t162_bank_table() -> Option<Table> {
    let cands: Vec<PathBuf> = std::env::var_os("RUSTRED_CAMPAIGN_FIXTURES")
        .map(|d| PathBuf::from(d).join("T162/RULE-PREDICATES2440.json"))
        .into_iter()
        .chain(std::iter::once(
            PathBuf::from(HO).join("astra-uncovered-domain-T162/RULE-PREDICATES2440.json"),
        ))
        .collect();
    let Some(p) = cands.iter().find(|p| p.is_file()) else {
        println!("SKIP: no RULE-PREDICATES2440.json found (tried {cands:?})");
        return None;
    };
    let p = verified_path(p, RULE_PREDICATES_SHA);
    let mut bank = read_bank_predicates(&p).unwrap();
    bank.sort_by_key(|b| b.global_ordinal0);
    assert_eq!(bank.len(), 2440);
    Some(
        Table::from_bank(
            "T162-bank-qualified",
            p.display().to_string(),
            &bank,
            20,
            T162_BANK_TARGETS,
        )
        .unwrap(),
    )
}

/// The 38 T178 order-B case exports as tables in cfg order (table index = k3 `piece_index`, label = case record
/// index `i`). Source: `RUSTRED_CAMPAIGN_FIXTURES/T178B/{cfg_t178_regenB.json, cases/}`, else the handoff copy
/// `HO/t178-orderB/{cfg_t178_regenB.json, cases/t178B/}`. Every file is checked against the in-tree manifest
/// `tests/data/fixtures.MANIFEST.sha256`. Returns the piece names, the tables and the header frame.
pub fn t178_tables() -> Option<(Vec<String>, Vec<Table>, NodeFrame)> {
    let manifest: HashMap<String, String> = include_str!("../data/fixtures.MANIFEST.sha256")
        .lines()
        .filter_map(|l| {
            l.split_once("  ")
                .map(|(h, f)| (f.to_string(), h.to_string()))
        })
        .collect();
    let (cfg, cases) = match std::env::var_os("RUSTRED_CAMPAIGN_FIXTURES") {
        Some(d)
            if PathBuf::from(&d)
                .join("T178B/cfg_t178_regenB.json")
                .is_file() =>
        {
            let d = PathBuf::from(d).join("T178B");
            (d.join("cfg_t178_regenB.json"), d.join("cases"))
        }
        _ => {
            let d = PathBuf::from(HO).join("t178-orderB");
            (d.join("cfg_t178_regenB.json"), d.join("cases/t178B"))
        }
    };
    if !cfg.is_file() {
        println!("SKIP: no T178B case exports found ({})", cfg.display());
        return None;
    }
    let cfg = verified_path(&cfg, &manifest["T178B/cfg_t178_regenB.json"]);
    let cfg: Value = serde_json::from_str(&std::fs::read_to_string(cfg).unwrap()).unwrap();
    let (mut names, mut tables, mut frame0) = (vec![], vec![], None::<NodeFrame>);
    for t in cfg["tables"].as_array().unwrap() {
        let rel = t.as_str().unwrap().trim_start_matches("cases/t178B/");
        let p = verified_path(&cases.join(rel), &manifest[&format!("T178B/cases/{rel}")]);
        let tf = read_table_file(&p, ReadOptions::default()).unwrap();
        let frame = NodeFrame::from_header(tf.header.as_ref().expect("header")).unwrap();
        if let Some(f0) = &frame0 {
            assert_eq!(f0, &frame);
        }
        frame0 = Some(frame.clone());
        let name = rel.trim_end_matches(".jsonl").to_string();
        for r in &tf.rules {
            assert_eq!(r.dispatch.as_deref(), Some("Partition"));
        }
        let table = Table::from_table_file(
            name.clone(),
            p.display().to_string(),
            &tf,
            // LEGACY reference numbers (k3 F5): the exports carry no witness loci, so the witness check is OFF here
            // (re-audit RA-MAJOR-1: with it on, every export credit is Blocked(WitnessNotCarried)); every credit says
            // `Assurance::CheckDisabled`. The strict dispatch from the raw tables is in tests/cover_raw_tables.rs.
            DomainOptions {
                targets: T178B_EXPORT_TARGETS,
                witness: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(table.skipped_count(), 0);
        assert_eq!(table.len(), tf.rules.len());
        names.push(name);
        tables.push(table);
    }
    assert_eq!(tables.len(), 38);
    Some((names, tables, frame0.unwrap()))
}
