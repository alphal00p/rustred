//! Exact argument values of line 7 (child-job.md, section "Line 7"): the canonical RATIONAL text, lists of distinct
//! unsigned integers, and a deterministic primality test for `u64` (the pre-flight refuses a non-prime probe prime
//! before anything runs; the engine checks again on its side: two independent cheap checks).
//!
//! Why own code (L11): Symbolica's numerica has a deterministic u64 Miller-Rabin (`Integer::is_prime`,
//! lib/numerica/src/domains/integer.rs:1756 -> `Zp64::is_prime_field`, finite_field.rs:3206), but this crate may
//! depend on serde, serde_json and blake3 only (ruling R-a: the engine core links it). A dependency-free check is
//! also the independent second check the contract owner asked for.
//!
//! Canonical rational text (the one form in arguments, records and digests): `-?(0|[1-9][0-9]*)(/[1-9][0-9]*)?` in
//! lowest terms with a denominator > 1 when present (`4`, `-3/2`; never `4/1`, `8/2`, `-0`, `+1`, `1.5`, `1e3`, ` 1`).
//! Numerator and denominator magnitudes are at most `i64::MAX` (a generic bound of the contract, not of any family).

use std::fmt;

/// An exact rational number in lowest terms (`den > 0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rational {
    num: i64,
    den: i64,
}

fn gcd_u(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Parse an unsigned decimal of at most `i64::MAX`: digits only (leading zeros are left to the canonical check).
fn parse_digits(s: &str) -> Result<u64, String> {
    if s.is_empty() {
        return Err("empty number".into());
    }
    if !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "{s:?} is not a decimal integer (digits only: no sign, point or exponent)"
        ));
    }
    let v: u64 = s.parse().map_err(|_| format!("{s:?} is out of range"))?;
    if v > i64::MAX as u64 {
        return Err(format!("{s:?} exceeds the contract's bound 2^63-1"));
    }
    Ok(v)
}

impl Rational {
    /// The integer `n`.
    pub const fn integer(n: i64) -> Rational {
        Rational { num: n, den: 1 }
    }

    /// `num / den` reduced to lowest terms; `None` when `den == 0` or a magnitude is `i64::MIN`.
    pub fn new(num: i64, den: i64) -> Option<Rational> {
        if den == 0 || num == i64::MIN || den == i64::MIN {
            return None;
        }
        let g = gcd_u(num.unsigned_abs(), den.unsigned_abs()) as i64;
        let (mut n, mut d) = (num / g, den / g);
        if d < 0 {
            (n, d) = (-n, -d);
        }
        Some(Rational { num: n, den: d })
    }

    pub const fn numerator(self) -> i64 {
        self.num
    }

    /// Always > 0.
    pub const fn denominator(self) -> i64 {
        self.den
    }

    /// STRICT parse: `s` must be the canonical text (arguments `--probe-d`, `--dims`; record members `d0`, `d`,
    /// `discovery_d`). The reason is returned on refusal.
    pub fn parse_canonical(s: &str) -> Result<Rational, String> {
        let r = Rational::parse_lenient(s)?;
        if r.to_string() != s {
            return Err(format!(
                "{s:?} is not the canonical rational text (write {:?})",
                r.to_string()
            ));
        }
        Ok(r)
    }

    /// NORMALISING parse (a recipe's `elimination.discovery_d`, coord-pole-engine.md 1: "8/2" and "4" are one value):
    /// `-?a(/b)?` with digits only, `b > 0`, normalised to lowest terms. Still no float, exponent, `+` or blank.
    pub fn parse_lenient(s: &str) -> Result<Rational, String> {
        let (neg, body) = match s.strip_prefix('-') {
            Some(b) => (true, b),
            None => (false, s),
        };
        let (n, d) = match body.split_once('/') {
            Some((n, d)) => (parse_digits(n)?, parse_digits(d)?),
            None => (parse_digits(body)?, 1),
        };
        if d == 0 {
            return Err(format!("{s:?}: zero denominator"));
        }
        let n = if neg { -(n as i64) } else { n as i64 };
        Rational::new(n, d as i64).ok_or_else(|| format!("{s:?} is out of range"))
    }
}

impl Ord for Rational {
    /// Numeric order (exact: cross-multiplication in i128).
    fn cmp(&self, o: &Rational) -> std::cmp::Ordering {
        (self.num as i128 * o.den as i128).cmp(&(o.num as i128 * self.den as i128))
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, o: &Rational) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

impl serde::Serialize for Rational {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for Rational {
    /// Deserialises the CANONICAL text only (records and node options are written canonically).
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Rational, D::Error> {
        let s = String::deserialize(d)?;
        Rational::parse_canonical(&s).map_err(serde::de::Error::custom)
    }
}

/// Parse a comma-separated list of CANONICAL rationals (`--dims`): non-empty, no blank element, pairwise distinct.
pub fn parse_rational_list(s: &str) -> Result<Vec<Rational>, String> {
    let mut out: Vec<Rational> = Vec::new();
    for e in s.split(',') {
        let r = Rational::parse_canonical(e)?;
        if out.contains(&r) {
            return Err(format!("{e:?} listed twice"));
        }
        out.push(r);
    }
    Ok(out)
}

/// Parse a comma-separated list of distinct unsigned integers in canonical decimal (no leading zero, no sign, no
/// blank element), each at most `u64::MAX`.
pub fn parse_uint_list(s: &str) -> Result<Vec<u64>, String> {
    let mut out: Vec<u64> = Vec::new();
    for e in s.split(',') {
        if e.is_empty() || !e.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("{e:?} is not an unsigned decimal integer"));
        }
        if e.len() > 1 && e.starts_with('0') {
            return Err(format!("{e:?} has a leading zero"));
        }
        let v: u64 = e.parse().map_err(|_| format!("{e:?} exceeds 2^64-1"))?;
        if out.contains(&v) {
            return Err(format!("{e} listed twice"));
        }
        out.push(v);
    }
    Ok(out)
}

/// One entry of a centre scan (seed minimisation, coord-seed-minimise.md 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CentreEntry {
    /// The recipe's own centres (coord-seed-minimise.md 7: minimise what already solves).
    Recipe,
    /// Every free slot one step to its floor.
    FloorOfFree,
    /// One centre per free slot (one step to its floor), in increasing slot order.
    EachFreeSlot,
    /// An explicit offset vector (of the node's arity: the engine checks the arity; the contract only that every
    /// explicit vector of one scan has the same length).
    Offset(Vec<i64>),
}

/// Text of the scan tokens.
pub const CENTRE_TOKENS: [&str; 3] = ["recipe", "floor_of_free", "each_free_slot"];

fn parse_canonical_int(t: &str) -> Result<i64, String> {
    let v: i64 = t.parse().map_err(|_| format!("{t:?} is not an integer"))?;
    if v.to_string() != t {
        return Err(format!(
            "{t:?} is not a canonical integer (no '+', leading zero or -0)"
        ));
    }
    Ok(v)
}

/// Parse a centre scan, the value of `--centre-scan` (EXACT text form, everything else refused): entries separated by
/// `,` with no blank; an entry is `recipe`, `floor_of_free`, `each_free_slot` or an explicit offset vector written
/// `(a;b;..;z)`: at least one canonical decimal integer (`0`, `-1`, `12`; no `+`, leading zero, `-0` or blank), `;`
/// between components, in parentheses. Entries pairwise distinct; every explicit vector of one scan has the same
/// length; at most 4096 characters.
pub fn parse_centre_scan(s: &str) -> Result<Vec<CentreEntry>, String> {
    if s.len() > 4096 {
        return Err("centre scan longer than 4096 characters".into());
    }
    let mut out: Vec<CentreEntry> = Vec::new();
    for e in s.split(',') {
        let entry = match e {
            "recipe" => CentreEntry::Recipe,
            "floor_of_free" => CentreEntry::FloorOfFree,
            "each_free_slot" => CentreEntry::EachFreeSlot,
            _ => {
                let inner = e
                    .strip_prefix('(')
                    .and_then(|x| x.strip_suffix(')'))
                    .ok_or_else(|| {
                        format!(
                            "{e:?} is not recipe, floor_of_free, each_free_slot or an offset vector (a;b;..)"
                        )
                    })?;
                let v = inner
                    .split(';')
                    .map(parse_canonical_int)
                    .collect::<Result<Vec<i64>, String>>()?;
                if let Some(CentreEntry::Offset(w)) =
                    out.iter().find(|x| matches!(x, CentreEntry::Offset(_)))
                    && w.len() != v.len()
                {
                    return Err(format!("offset vector {e} has another length than {w:?}"));
                }
                CentreEntry::Offset(v)
            }
        };
        if out.contains(&entry) {
            return Err(format!("centre scan entry {e:?} listed twice"));
        }
        out.push(entry);
    }
    Ok(out)
}

/// Check a CASE DOMAIN object (coord-seed-minimise.md 7: "the schema of the recipe `domain`", coord-recipe-solve.md
/// 3): a JSON object with at least one of the members `fixed` (list of integers or null: null = free slot),
/// `equations` (list of non-empty strings: index polynomials that vanish on the domain), `bounds` (`{"min": [int |
/// null], "max": [int | null]}`, lists of the length of `fixed` when both are given) and no other member. The value
/// of `--minimise-target` is such an object as compact JSON text (the contract has no other domain shape; a JSON
/// value in `args` follows the precedent of `--order`); the polynomials and the arity are the engine's checks.
pub fn check_case_domain(v: &serde_json::Value) -> Result<(), String> {
    let m = v.as_object().ok_or("a case domain is a JSON object")?;
    if m.is_empty() {
        return Err("a case domain names at least one of fixed, equations, bounds".into());
    }
    for k in m.keys() {
        if !["fixed", "equations", "bounds"].contains(&k.as_str()) {
            return Err(format!(
                "case domain member {k:?} is not fixed / equations / bounds"
            ));
        }
    }
    let int_or_null = |x: &serde_json::Value| x.is_null() || x.as_i64().is_some();
    let mut arity: Option<usize> = None;
    if let Some(f) = m.get("fixed") {
        let f = f.as_array().ok_or("fixed is not a list")?;
        if f.is_empty() || !f.iter().all(int_or_null) {
            return Err("fixed is not a non-empty list of integers / null".into());
        }
        arity = Some(f.len());
    }
    if let Some(e) = m.get("equations") {
        let ok = e.as_array().is_some_and(|e| {
            e.iter()
                .all(|x| x.as_str().is_some_and(|s| !s.trim().is_empty()))
        });
        if !ok {
            return Err("equations is not a list of polynomial texts".into());
        }
    }
    if let Some(b) = m.get("bounds") {
        let b = b.as_object().ok_or("bounds is not an object")?;
        if b.is_empty() || b.keys().any(|k| k != "min" && k != "max") {
            return Err("bounds has members other than min / max (or none)".into());
        }
        for side in b.values() {
            let side = side.as_array().ok_or("a bounds side is not a list")?;
            if !side.iter().all(int_or_null) || arity.is_some_and(|n| n != side.len()) {
                return Err(
                    "a bounds side is not a list of integers / null of the domain's length".into(),
                );
            }
        }
    }
    Ok(())
}

/// Parse the value of `--minimise-target`: the compact JSON text of a case domain ([`check_case_domain`]), at most
/// 4096 characters.
pub fn parse_case_domain(s: &str) -> Result<serde_json::Value, String> {
    if s.len() > 4096 {
        return Err("case domain longer than 4096 characters".into());
    }
    let v: serde_json::Value =
        serde_json::from_str(s).map_err(|e| format!("case domain is not JSON: {e}"))?;
    check_case_domain(&v)?;
    Ok(v)
}

/// Lower bound (inclusive) of a modular prime of line 7 (`--probe-primes`, `--primes` of node-basis): 2^40.
pub const PRIME_MIN: u64 = 1 << 40;
/// Upper bound (exclusive): 2^62.
pub const PRIME_END: u64 = 1 << 62;

/// The engine's default probe primes (coord-pole-engine.md 8): 2^61 - 1 and 2^61 - 31 (both checked prime by
/// [`is_prime_u64`] in the tests).
pub const PROBE_DEFAULT_PRIMES: [u64; 2] = [2_305_843_009_213_693_951, 2_305_843_009_213_693_921];

/// The prime rule of line 7: `p` in [2^40, 2^62) and prime. `Err` names the reason.
pub fn check_line7_prime(p: u64) -> Result<(), String> {
    if !(PRIME_MIN..PRIME_END).contains(&p) {
        return Err(format!("{p} is outside [2^40, 2^62)"));
    }
    if !is_prime_u64(p) {
        return Err(format!("{p} is not prime"));
    }
    Ok(())
}

/// Parse a prime list of line 7: [`parse_uint_list`] + [`check_line7_prime`] for every element.
pub fn parse_prime_list(s: &str) -> Result<Vec<u64>, String> {
    let v = parse_uint_list(s)?;
    for &p in &v {
        check_line7_prime(p)?;
    }
    Ok(v)
}

fn mul_mod(a: u64, b: u64, m: u64) -> u64 {
    ((a as u128 * b as u128) % m as u128) as u64
}

fn pow_mod(mut b: u64, mut e: u64, m: u64) -> u64 {
    let mut r = 1 % m;
    b %= m;
    while e > 0 {
        if e & 1 == 1 {
            r = mul_mod(r, b, m);
        }
        b = mul_mod(b, b, m);
        e >>= 1;
    }
    r
}

/// The Miller-Rabin bases of [`is_prime_u64`]: the first twelve primes. EXACT for every n < psi_12 =
/// 318,665,857,834,031,151,167,461 (> 2^64 = 18,446,744,073,709,551,616): the smallest strong pseudoprime to all of
/// them (Y. Jiang, Y. Deng, "Strong pseudoprimes to the first eight prime bases", Math. Comp. 83 (2014) 2915-2924;
/// J. Sorenson, J. Webster, "Strong pseudoprimes to twelve prime bases", Math. Comp. 86 (2017) 985-1003). So the test
/// is deterministic on all of `u64` (no randomness, no probability).
pub const MR_BASES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];

/// Deterministic primality of a `u64` (strong-probable-prime test to the bases [`MR_BASES`], exact below 2^64).
pub fn is_prime_u64(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    for p in MR_BASES {
        if n.is_multiple_of(p) {
            return n == p;
        }
    }
    // n odd, > 37: n - 1 = d * 2^s with d odd
    let s = (n - 1).trailing_zeros();
    let d = (n - 1) >> s;
    'bases: for a in MR_BASES {
        let mut x = pow_mod(a, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..s {
            x = mul_mod(x, x, n);
            if x == n - 1 {
                continue 'bases;
            }
        }
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rational_texts() {
        for ok in [
            "0",
            "4",
            "-4",
            "1/2",
            "-3/2",
            "9223372036854775807",
            "1/9223372036854775807",
        ] {
            assert_eq!(Rational::parse_canonical(ok).unwrap().to_string(), ok);
        }
        for bad in [
            "",
            "-",
            "+1",
            "-0",
            "04",
            "4/1",
            "8/2",
            "2/4",
            "1/0",
            "0/1",
            "1/-2",
            "1.5",
            "1e3",
            " 1",
            "1 ",
            "1//2",
            "1/2/3",
            "/2",
            "1/",
            "9223372036854775808",
            "a",
        ] {
            assert!(Rational::parse_canonical(bad).is_err(), "{bad:?} accepted");
        }
        assert_eq!(
            Rational::parse_lenient("8/2").unwrap(),
            Rational::integer(4)
        );
        assert_eq!(Rational::parse_lenient("-6/4").unwrap().to_string(), "-3/2");
        assert_eq!(Rational::parse_lenient("0/7").unwrap().to_string(), "0");
        assert!(Rational::parse_lenient("1/0").is_err());
        assert!(Rational::parse_lenient("1.0").is_err());
        assert_eq!(Rational::new(6, -4).unwrap().to_string(), "-3/2");
    }

    #[test]
    fn lists() {
        assert_eq!(parse_uint_list("3,1,2").unwrap(), vec![3, 1, 2]);
        for bad in [
            "",
            "1,",
            ",1",
            "1,,2",
            "01",
            "1,1",
            "-1",
            "1 ,2",
            "18446744073709551616",
        ] {
            assert!(parse_uint_list(bad).is_err(), "{bad:?} accepted");
        }
        assert_eq!(parse_rational_list("4,1/2,-3").unwrap().len(), 3);
        assert!(parse_rational_list("4,8/2").is_err());
        assert!(parse_rational_list("4,4").is_err());
    }
}
