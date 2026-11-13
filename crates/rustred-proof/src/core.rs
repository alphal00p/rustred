//! Shared pieces of the proof kernel, ported from the H8 kernel (handoff/helper-h10-kernel/src/main.rs):
//! SHA-256 and the hash word stream (K3), the byte-range JSON reader, Montgomery arithmetic + Miller-Rabin,
//! the coefficient tokenizer (grammar in README §2), degree pass, sparse denominator polynomials, patterns and
//! affine forms. Code is kept close to the reviewed kernel; `die` raises a `KernelError` panic that the public
//! entry points of this crate catch and turn into `Err` (see `lib.rs::guarded`).
#![allow(clippy::all)]
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub use crate::big::*;

pub const NV: usize = 21; // n0..n19, d
pub const DV: usize = 20; // index of d

#[derive(Debug)]
pub struct KernelError(pub String);
pub fn die(msg: &str) -> ! {
    std::panic::panic_any(KernelError(msg.to_string()))
}
pub fn jstr(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 32 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
// ===================================================================== SHA-256
#[derive(Clone)]
pub struct Sha256 { h: [u32; 8], buf: [u8; 64], nbuf: usize, len: u64 }
pub const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2];
impl Sha256 {
    pub fn new() -> Self {
        Sha256 { h: [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19],
                 buf: [0; 64], nbuf: 0, len: 0 }
    }
    pub fn block(&mut self, b: &[u8]) {
        let mut w = [0u32; 64];
        for i in 0..16 { w[i] = u32::from_be_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]); }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = self.h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K256[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v[7] = v[6]; v[6] = v[5]; v[5] = v[4]; v[4] = v[3].wrapping_add(t1);
            v[3] = v[2]; v[2] = v[1]; v[1] = v[0]; v[0] = t1.wrapping_add(t2);
        }
        for i in 0..8 { self.h[i] = self.h[i].wrapping_add(v[i]); }
    }
    pub fn update(&mut self, mut d: &[u8]) {
        self.len += d.len() as u64;
        if self.nbuf > 0 {
            let t = (64 - self.nbuf).min(d.len());
            self.buf[self.nbuf..self.nbuf + t].copy_from_slice(&d[..t]);
            self.nbuf += t; d = &d[t..];
            if self.nbuf == 64 { let b = self.buf; self.block(&b); self.nbuf = 0; }
        }
        while d.len() >= 64 { self.block(&d[..64]); d = &d[64..]; }
        if !d.is_empty() { self.buf[..d.len()].copy_from_slice(d); self.nbuf = d.len(); }
    }
    pub fn finish(mut self) -> [u8; 32] {
        let bits = self.len.wrapping_mul(8);
        self.update(&[0x80]);
        while self.nbuf != 56 { self.update(&[0]); }
        self.update(&bits.to_be_bytes());
        let mut o = [0u8; 32];
        for i in 0..8 { o[4 * i..4 * i + 4].copy_from_slice(&self.h[i].to_be_bytes()); }
        o
    }
}
pub fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{:02x}", x)).collect() }

/// word stream: block_i = SHA256(T || C || LE64(i)), i = 0,1,...; each block = 4 LE u64 words.
pub struct Stream { pub base: Sha256, pub ctr: u64, pub words: [u64; 4], pub used: usize, pub drawn: u64 }
impl Stream {
    pub fn new(base: Sha256) -> Self { Stream { base, ctr: 0, words: [0; 4], used: 4, drawn: 0 } }
    pub fn next(&mut self) -> u64 {
        if self.used == 4 {
            let mut h = self.base.clone(); h.update(&self.ctr.to_le_bytes()); self.ctr += 1;
            let d = h.finish();
            for i in 0..4 { self.words[i] = u64::from_le_bytes(d[8 * i..8 * i + 8].try_into().unwrap()); }
            self.used = 0;
        }
        self.used += 1; self.drawn += 1;
        self.words[self.used - 1]
    }
}

// ===================================================================== JSON (byte ranges into the line)
#[derive(Debug)]
pub enum J { Null, Bool(bool), Num(usize, usize), Str(usize, usize), Arr(Vec<J>), Obj(Vec<((usize, usize), J)>) }
pub struct JP<'a> { pub b: &'a [u8], pub i: usize }
impl<'a> JP<'a> {
    pub fn ws(&mut self) { while self.i < self.b.len() && (self.b[self.i] as char).is_ascii_whitespace() { self.i += 1; } }
    pub fn val(&mut self) -> J {
        self.ws();
        if self.i >= self.b.len() { die("json: unexpected end"); }
        match self.b[self.i] {
            b'{' => {
                self.i += 1; let mut v = Vec::new(); self.ws();
                if self.b[self.i] == b'}' { self.i += 1; return J::Obj(v); }
                loop {
                    self.ws(); let k = match self.val() { J::Str(a, b) => (a, b), _ => die("json: key") };
                    self.ws(); if self.b[self.i] != b':' { die("json: ':'"); } self.i += 1;
                    let x = self.val(); v.push((k, x)); self.ws();
                    match self.b[self.i] { b',' => { self.i += 1; } b'}' => { self.i += 1; return J::Obj(v); } _ => die("json: obj") }
                }
            }
            b'[' => {
                self.i += 1; let mut v = Vec::new(); self.ws();
                if self.b[self.i] == b']' { self.i += 1; return J::Arr(v); }
                loop {
                    v.push(self.val()); self.ws();
                    match self.b[self.i] { b',' => { self.i += 1; } b']' => { self.i += 1; return J::Arr(v); } _ => die("json: arr") }
                }
            }
            b'"' => {
                self.i += 1; let s = self.i;
                while self.b[self.i] != b'"' { if self.b[self.i] == b'\\' { self.i += 1; } self.i += 1; }
                self.i += 1; J::Str(s, self.i - 1)
            }
            b'n' => { self.i += 4; J::Null }
            b't' => { self.i += 4; J::Bool(true) }
            b'f' => { self.i += 5; J::Bool(false) }
            _ => {
                let s = self.i;
                while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') { self.i += 1; }
                if s == self.i { die("json: bad value"); }
                J::Num(s, self.i)
            }
        }
    }
}
pub fn jparse(b: &[u8]) -> J { let mut p = JP { b, i: 0 }; p.val() }
impl J {
    pub fn get<'x>(&'x self, b: &[u8], key: &str) -> Option<&'x J> {
        if let J::Obj(v) = self { for ((s, e), x) in v { if &b[*s..*e] == key.as_bytes() { return Some(x); } } }
        None
    }
    pub fn arr(&self) -> &Vec<J> { if let J::Arr(v) = self { v } else { die("json: expected array") } }
    pub fn s<'b>(&self, b: &'b [u8]) -> &'b [u8] {
        match self { J::Str(s, e) => { let t = &b[*s..*e]; if t.contains(&b'\\') { die("json: escapes in a coefficient/pattern string are not supported") } t } _ => die("json: expected string") }
    }
    pub fn int(&self, b: &[u8]) -> i64 {
        match self {
            J::Num(s, e) => std::str::from_utf8(&b[*s..*e]).unwrap().parse().unwrap_or_else(|_| die("json: int")),
            J::Str(s, e) => std::str::from_utf8(&b[*s..*e]).unwrap().trim().parse().unwrap_or_else(|_| die("json: int string (non-integer P_km?)")),
            _ => die("json: expected int"),
        }
    }
}
// ===================================================================== Montgomery arithmetic mod p (p odd, < 2^63)
#[derive(Clone, Copy)]
pub struct Mont { pub p: u64, pub pinv: u64, pub r2: u64 }
impl Mont {
    pub fn new(p: u64) -> Self {
        let mut inv = 1u64; for _ in 0..7 { inv = inv.wrapping_mul(2u64.wrapping_sub(p.wrapping_mul(inv))); }
        let r = ((1u128 << 64) % p as u128) as u64; let r2 = ((r as u128 * r as u128) % p as u128) as u64;
        Mont { p, pinv: inv.wrapping_neg(), r2 }
    }
    #[inline(always)] pub fn mul(&self, a: u64, b: u64) -> u64 {
        let t = a as u128 * b as u128; let m = (t as u64).wrapping_mul(self.pinv);
        let u = ((t + m as u128 * self.p as u128) >> 64) as u64; if u >= self.p { u - self.p } else { u }
    }
    #[inline(always)] pub fn add(&self, a: u64, b: u64) -> u64 { let s = a + b; if s >= self.p { s - self.p } else { s } }
    #[inline(always)] pub fn sub(&self, a: u64, b: u64) -> u64 { if a >= b { a - b } else { a + self.p - b } }
    pub fn to(&self, x: u64) -> u64 { self.mul(x % self.p, self.r2) }
    pub fn from(&self, x: u64) -> u64 { self.mul(x, 1) }
    pub fn from_i64(&self, x: i64) -> u64 { let r = (x.rem_euclid(self.p as i64)) as u64; self.to(r) }
    pub fn pow(&self, a: u64, mut e: u64) -> u64 { let mut r = self.to(1); let mut b = a; while e > 0 { if e & 1 == 1 { r = self.mul(r, b); } b = self.mul(b, b); e >>= 1; } r }
    pub fn inv(&self, a: u64) -> u64 { self.pow(a, self.p - 2) }
}
pub fn mulmod(a: u64, b: u64, m: u64) -> u64 { ((a as u128 * b as u128) % m as u128) as u64 }
pub fn powmod(mut a: u64, mut e: u64, m: u64) -> u64 { let mut r = 1u64; a %= m; while e > 0 { if e & 1 == 1 { r = mulmod(r, a, m); } a = mulmod(a, a, m); e >>= 1; } r }
pub fn is_prime(n: u64) -> bool {
    if n < 2 { return false; }
    for q in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] { if n % q == 0 { return n == q; } }
    let mut d = n - 1; let mut s = 0; while d % 2 == 0 { d /= 2; s += 1; }
    'a: for a in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let mut x = powmod(a, d, n); if x == 1 || x == n - 1 { continue; }
        for _ in 0..s - 1 { x = mulmod(x, x, n); if x == n - 1 { continue 'a; } }
        return false;
    }
    true
}

// ===================================================================== tokenizer
// coefficient := sum | "(" sum ")" | sum "/(" sum ")" | "(" sum ")/(" sum ")"
// sum := ["+"|"-"] term (("+"|"-") term)* ; term := INT | INT "*" mono | mono ; mono := var ["^" INT] ("*" var ["^" INT])*
// var := "n" INT (0..19) | "d"
pub fn split_coef(s: &[u8]) -> (&[u8], Option<&[u8]>) {
    let s = trim_ws(s);
    let (num, rest) = if s.first() == Some(&b'(') {
        let close = s.iter().position(|&c| c == b')').unwrap_or_else(|| die("coef: unbalanced"));
        (&s[1..close], &s[close + 1..])
    } else {
        match s.iter().position(|&c| c == b'/') { Some(p) => (&s[..p], &s[p..]), None => (s, &s[s.len()..]) }
    };
    if num.contains(&b'(') { die(&format!("coef: unsupported form (nested parentheses): {}", String::from_utf8_lossy(&s[..s.len().min(120)]))); }
    let rest = trim_ws(rest);
    if rest.is_empty() { return (num, None); }
    if rest.len() >= 3 && rest[0] == b'/' && rest[1] == b'(' && rest[rest.len() - 1] == b')' {
        let den = &rest[2..rest.len() - 1];
        if den.contains(&b'(') || den.contains(&b')') || den.contains(&b'/') { die("coef: unsupported denominator form"); }
        return (num, Some(den));
    }
    // rr_node / Symbolica print a single-token denominator without parentheses, e.g. "(3+n9-d)/n15": accept exactly
    // one token (integer, or a variable with an optional power); anything longer ("/2*n0") stays refused
    if rest.len() >= 2 && rest[0] == b'/' {
        let den = trim_ws(&rest[1..]);
        let tok_ok = !den.is_empty()
            && (den.iter().all(|c| c.is_ascii_digit())
                || ((den[0] == b'n' || den[0] == b'd') && den[1..].iter().all(|&c| c.is_ascii_digit() || c == b'^') && !den.ends_with(b"^")));
        if tok_ok { return (num, Some(den)); }
    }
    die(&format!("coef: unsupported form: {}", String::from_utf8_lossy(&s[..s.len().min(120)])))
}
pub fn trim_ws(s: &[u8]) -> &[u8] { let mut a = 0; let mut b = s.len(); while a < b && s[a] == b' ' { a += 1; } while b > a && s[b - 1] == b' ' { b -= 1; } &s[a..b] }

pub struct Term<'a> { pub neg: bool, pub coef: &'a [u8], pub nf: usize, pub fv: [u8; NV], pub fe: [u32; NV] }
pub struct Terms<'a> { pub s: &'a [u8], pub i: usize }
impl<'a> Terms<'a> {
    pub fn new(s: &'a [u8]) -> Self { Terms { s, i: 0 } }
    pub fn uint(&mut self) -> &'a [u8] { let st = self.i; while self.i < self.s.len() && self.s[self.i].is_ascii_digit() { self.i += 1; } &self.s[st..self.i] }
    pub fn next(&mut self) -> Option<Term<'a>> {
        let s = self.s;
        while self.i < s.len() && s[self.i] == b' ' { self.i += 1; }
        if self.i >= s.len() { return None; }
        let mut neg = false; let mut sawsign = false;
        while self.i < s.len() && (s[self.i] == b'+' || s[self.i] == b'-' || s[self.i] == b' ') { if s[self.i] == b'-' { neg = !neg; } if s[self.i] != b' ' { sawsign = true; } self.i += 1; }
        let _ = sawsign;
        let mut t = Term { neg, coef: &s[0..0], nf: 0, fv: [0; NV], fe: [0; NV] };
        if self.i < s.len() && s[self.i].is_ascii_digit() {
            t.coef = self.uint();
            if self.i < s.len() && s[self.i] == b'*' { self.i += 1; } else { return Some(t); }
        }
        loop {
            if self.i >= s.len() { die("tok: dangling '*'"); }
            let v: usize = match s[self.i] {
                b'n' => { self.i += 1; let d = self.uint(); if d.is_empty() || d.len() > 2 { die("tok: bad var"); } let v = dnum(d) as usize; if v >= 20 { die("tok: var index >= 20"); } v }
                b'd' => { self.i += 1; DV }
                c => die(&format!("tok: unexpected '{}' at {} in {}", c as char, self.i, String::from_utf8_lossy(&s[self.i.saturating_sub(40)..(self.i + 40).min(s.len())]))),
            };
            let mut e = 1u32;
            if self.i < s.len() && s[self.i] == b'^' { self.i += 1; let d = self.uint(); if d.is_empty() || d.len() > 4 { die("tok: bad exponent"); } e = dnum(d) as u32; }
            if let Some(k) = (0..t.nf).find(|&k| t.fv[k] as usize == v) { t.fe[k] += e; } else { t.fv[t.nf] = v as u8; t.fe[t.nf] = e; t.nf += 1; }
            if self.i < s.len() && s[self.i] == b'*' { self.i += 1; continue; }
            if self.i < s.len() && s[self.i].is_ascii_digit() { die("tok: digit after variable"); }
            return Some(t);
        }
    }
}
#[inline(always)] pub fn dnum(d: &[u8]) -> u64 { let mut x = 0u64; for &c in d { x = x * 10 + (c - b'0') as u64; } x }
#[inline(always)] pub fn coef_i64(c: &[u8]) -> Option<i64> { if c.is_empty() { return Some(1); } if c.len() > 18 { return None; } Some(dnum(c) as i64) }

// degree info of a text: per-variable degree (all 21), total degree over identity vars, monomials
#[derive(Clone, Debug)]
pub struct Deg { pub v: [u32; NV], pub tot: u32, pub nmono: u64, pub maxcoefbits: u32 }
pub fn text_deg(s: &[u8], idv: &[bool; NV]) -> Deg {
    let mut d = Deg { v: [0; NV], tot: 0, nmono: 0, maxcoefbits: 0 };
    let mut it = Terms::new(s);
    while let Some(t) = it.next() {
        d.nmono += 1; let mut tt = 0;
        for k in 0..t.nf { let v = t.fv[k] as usize; d.v[v] = d.v[v].max(t.fe[k]); if idv[v] { tt += t.fe[k]; } }
        d.tot = d.tot.max(tt);
        let b = if t.coef.is_empty() { 1 } else { (t.coef.len() as f64 * 3.3219).ceil() as u32 };
        d.maxcoefbits = d.maxcoefbits.max(b);
    }
    d
}

// sparse polynomial with i128 coefficients (for denominators only; overflow is an error)
pub type Poly = Vec<(u128, i128)>; // (packed exponents 6 bits x 21, coeff), sorted by key
pub fn pack(fv: &[u8], fe: &[u32], nf: usize) -> u128 { let mut k = 0u128; for i in 0..nf { if fe[i] > 63 { die("poly: exponent > 63"); } k += (fe[i] as u128) << (6 * fv[i] as u32); } k }
pub fn poly_parse(s: &[u8]) -> Poly {
    let mut m: HashMap<u128, i128> = HashMap::new(); let mut it = Terms::new(s);
    while let Some(t) = it.next() {
        let c = if t.coef.is_empty() { 1i128 } else { std::str::from_utf8(t.coef).unwrap().parse::<i128>().unwrap_or_else(|_| die("poly: coefficient overflow")) };
        let c = if t.neg { -c } else { c };
        *m.entry(pack(&t.fv, &t.fe, t.nf)).or_insert(0) += c;
    }
    let mut v: Poly = m.into_iter().filter(|x| x.1 != 0).collect(); v.sort(); v
}
pub fn poly_mul(a: &Poly, b: &Poly) -> Poly {
    let mut m: HashMap<u128, i128> = HashMap::new();
    for (ka, ca) in a { for (kb, cb) in b {
        // exponent fields must not overflow 6 bits: check per field
        for f in 0..NV { if ((ka >> (6 * f)) & 63) + ((kb >> (6 * f)) & 63) > 63 { die("poly: exponent overflow in product"); } }
        let c = ca.checked_mul(*cb).unwrap_or_else(|| die("poly: coefficient overflow"));
        let e = m.entry(ka + kb).or_insert(0); *e = e.checked_add(c).unwrap_or_else(|| die("poly: overflow"));
    } }
    let mut v: Poly = m.into_iter().filter(|x| x.1 != 0).collect(); v.sort(); v
}
pub fn gcd(a: i128, b: i128) -> i128 { let (mut a, mut b) = (a.abs(), b.abs()); while b != 0 { let t = a % b; a = b; b = t; } a }
pub fn poly_normal(p: &Poly) -> Poly { // primitive, leading (largest key) coefficient positive
    if p.is_empty() { return p.clone(); }
    let mut g = 0; for (_, c) in p { g = gcd(g, *c); }
    let s = if p.last().unwrap().1 < 0 { -1 } else { 1 };
    p.iter().map(|(k, c)| (*k, c / g * s)).collect()
}

// ===================================================================== patterns / columns
pub type Pat = Vec<(bool, i64)>; // (symbolic?, offset or value)
pub fn parse_pat(s: &[u8]) -> Pat {
    let s = std::str::from_utf8(s).unwrap().replace(' ', "");
    if !s.starts_with("I(") || !s.ends_with(')') { die(&format!("pattern: {}", s)); }
    s[2..s.len() - 1].split(',').enumerate().map(|(i, a)| {
        if let Some(r) = a.strip_prefix('n') {
            let p = r.find(|c: char| c == '+' || c == '-').unwrap_or(r.len());
            let idx: usize = r[..p].parse().unwrap_or_else(|_| die("pattern: index"));
            if idx != i { die("pattern: slot index mismatch"); }
            (true, if p < r.len() { r[p..].trim_start_matches('+').parse().unwrap_or_else(|_| die("pattern: offset")) } else { 0 })
        } else { (false, a.parse().unwrap_or_else(|_| die(&format!("pattern: value {}", a)))) }
    }).collect()
}
pub fn pat_str(p: &Pat) -> String {
    let v: Vec<String> = p.iter().enumerate().map(|(i, &(s, o))| if s { if o == 0 { format!("n{}", i) } else { format!("n{}{:+}", i, o) } } else { o.to_string() }).collect();
    format!("I({})", v.join(","))
}

/// affine form c0 + sum lin[v] x_v over the 21 variables, integer coefficients
#[derive(Clone, Debug, PartialEq)]
pub struct Aff { pub c0: i64, pub lin: Vec<(u8, i64)> }
pub fn parse_affine(s: &[u8]) -> Result<Aff, String> {
    let (num, den) = split_coef(s);
    if den.is_some() { return Err("row coefficient has a denominator".into()); }
    let mut a = Aff { c0: 0, lin: vec![] }; let mut lin = [0i64; NV];
    let mut it = Terms::new(num);
    while let Some(t) = it.next() {
        let c = coef_i64(t.coef).ok_or("row coefficient too large")?; let c = if t.neg { -c } else { c };
        let deg: u32 = t.fe[..t.nf].iter().sum();
        if deg == 0 { a.c0 += c; } else if deg == 1 { lin[t.fv[0] as usize] += c; } else { return Err(format!("NOT AFFINE: {}", String::from_utf8_lossy(s))); }
    }
    for v in 0..NV { if lin[v] != 0 { a.lin.push((v as u8, lin[v])); } }
    Ok(a)
}

pub fn read_lines_matching(path: &str, mut f: impl FnMut(&[u8]) -> bool) {
    let fh = File::open(path).unwrap_or_else(|e| die(&format!("open {}: {}", path, e)));
    let mut r = BufReader::with_capacity(1 << 20, fh);
    let mut line = Vec::new();
    loop {
        line.clear();
        let n = r.read_until(b'\n', &mut line).unwrap_or_else(|e| die(&format!("read {}: {}", path, e)));
        if n == 0 {
            break;
        }
        while line.last() == Some(&b'\n') || line.last() == Some(&b'\r') {
            line.pop();
        }
        if !f(&line) {
            break;
        }
    }
}
pub fn peak_mb() -> f64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for l in s.lines() {
        if l.starts_with("VmHWM:") {
            let kb: f64 = l.split_whitespace().nth(1).unwrap().parse().unwrap();
            return kb / 1024.0;
        }
    }
    -1.0
}
pub fn find(h: &[u8], n: &[u8]) -> bool {
    h.windows(n.len()).any(|w| w == n)
}

// ===================================================================== points and streaming evaluation (kernel K2/K3)
#[derive(Clone)]
pub enum PKind { Mod(Mont), Ex }
#[derive(Clone)]
pub struct Point {
    pub kind: PKind,
    // values of the 21 variables (fixed slots = rule values). mod: residues; exact: integers, d = dnum/dden
    pub mv: [u64; NV], pub ev: Vec<Big>, pub dden: Big, pub label: String,
    // power tables: mod pw[v][e] (Montgomery); exact epw[v][e]; for d with dden != 1: epw[d][k] = dnum^k dden^(G-k)
    pub pw: Vec<Vec<u64>>, pub epw: Vec<Vec<Big>>, pub dden_is_one: bool,
}
pub fn build_tables(pt: &mut Point, maxe: &[u32; NV], g: u32) {
    match pt.kind {
        PKind::Mod(m) => {
            pt.pw = (0..NV).map(|v| { let x = m.to(pt.mv[v]); let mut t = vec![m.to(1)]; for e in 1..=maxe[v] as usize { let y = m.mul(t[e - 1], x); t.push(y); } t }).collect();
        }
        PKind::Ex => {
            pt.dden_is_one = pt.dden == Big::one();
            pt.epw = (0..NV).map(|v| {
                if v == DV && !pt.dden_is_one {
                    let mut an = vec![Big::one()]; let mut bn = vec![Big::one()];
                    for e in 1..=g as usize { let a = an[e - 1].mul(&pt.ev[DV]); an.push(a); let b = bn[e - 1].mul(&pt.dden); bn.push(b); }
                    (0..=g as usize).map(|k| an[k].mul(&bn[g as usize - k])).collect()
                } else {
                    let mut t = vec![Big::one()]; for e in 1..=maxe[v] as usize { let y = t[e - 1].mul(&pt.ev[v]); t.push(y); } t
                }
            }).collect();
        }
    }
}
/// value of a sum text at every point: mod -> Montgomery residue; exact -> integer X (true value * dden^G)
#[derive(Clone)]
pub enum Val { M(u64), E(Big) }
pub fn eval_sum(s: &[u8], pts: &[Point], out: &mut Vec<Val>, scratch: &mut (Vec<u64>, Vec<u64>), nmono: &mut u64) {
    out.clear();
    for p in pts { out.push(match p.kind { PKind::Mod(_) => Val::M(0), PKind::Ex => Val::E(Big::zero()) }); }
    let mut it = Terms::new(s);
    while let Some(t) = it.next() {
        *nmono += 1;
        let ci = coef_i64(t.coef); let cb = if ci.is_none() { Some(Big::from_dec(t.coef)) } else { None };
        for (pi, p) in pts.iter().enumerate() {
            match p.kind {
                PKind::Mod(m) => {
                    let mut x = match ci { Some(c) => m.from_i64(c), None => m.to(cb.as_ref().unwrap().mod_u64(m.p)) };
                    for k in 0..t.nf { x = m.mul(x, p.pw[t.fv[k] as usize][t.fe[k] as usize]); }
                    if let Val::M(a) = &mut out[pi] { *a = if t.neg { m.sub(*a, x) } else { m.add(*a, x) }; }
                }
                PKind::Ex => {
                    let (a, b) = (&mut scratch.0, &mut scratch.1);
                    a.clear();
                    match ci { Some(c) => { if c != 0 { a.push(c as u64); } } None => a.extend_from_slice(&cb.as_ref().unwrap().m) }
                    let mut hasd = false;
                    for k in 0..t.nf {
                        let v = t.fv[k] as usize; if v == DV { hasd = true; }
                        mag_mul_into(b, a, &p.epw[v][t.fe[k] as usize].m); std::mem::swap(a, b);
                    }
                    if !hasd && !p.dden_is_one { mag_mul_into(b, a, &p.epw[DV][0].m); std::mem::swap(a, b); }
                    // sign: coefficient sign xor negative factors (odd powers of negative values)
                    let mut neg = t.neg;
                    for k in 0..t.nf { let v = t.fv[k] as usize; if v == DV && !p.dden_is_one { if p.epw[DV][t.fe[k] as usize].neg { neg = !neg; } } else if p.ev[v].neg && t.fe[k] % 2 == 1 { neg = !neg; } }
                    if let Val::E(acc) = &mut out[pi] { acc.add_signed(neg, a); }
                }
            }
        }
    }
}
