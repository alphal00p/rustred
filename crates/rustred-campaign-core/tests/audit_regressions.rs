//! Regression tests for the findings of the two independent audits of this crate (WS/notes/implementer.audit-core-A.md
//! and implementer.audit-core-B.md) and ruling F1 of the coordinator. One test per finding id; each test name starts
//! with the id. Every assertion here was FALSE at commit 62781c64 (the audited state; the auditors' probe outputs are
//! quoted in the comments): the old crate wrapped, accepted or credited where these tests now demand an error, a
//! refusal or an open verdict. Governing principle: FAIL CLOSED.

use rustred_campaign_core::poly::{BigInt, ENGINE_PRIMES, ParseOptions, Poly, PolyError, Value};

// ============================================================================================== poly (audit A)

/// A m1: `n18446744073709551616` (2^64) was read as `n0` (`v as usize` truncation), `...617` as `n1`.
#[test]
fn audit_a_m1_variable_index_never_wraps() {
    for s in [
        "n18446744073709551616",
        "n18446744073709551617",
        "n340282366920938463463374607431768211456", // beyond u128
        "n3",
    ] {
        assert!(
            matches!(
                Poly::parse_d_free(s, 3),
                Err(PolyError::VariableOutOfRange { .. }) | Err(PolyError::Syntax { .. })
            ),
            "{s} must be refused"
        );
    }
    assert_eq!(Poly::parse_d_free("n2", 3).unwrap(), Poly::var(2));
}

/// A m2: the parser accepted `n01` -> n1, `n00` -> n0 and `n1=2` -> n1-2, which the engine's `Expr::parse` refuses.
/// Strict is now the default; the equation form needs the explicit lenient option.
#[test]
fn audit_a_m2_strict_parser_is_the_default() {
    for s in ["n01", "n00", "n1=2", "n1 = n2", "n0+n007"] {
        assert!(Poly::parse_d_free(s, 3).is_err(), "{s} must be refused");
        assert!(Poly::parse(s, 3, true).is_err(), "{s} must be refused");
    }
    let lenient = ParseOptions {
        allow_d: false,
        allow_equation: true,
    };
    assert_eq!(
        Poly::parse_with("n1=2", 3, lenient).unwrap(),
        Poly::parse_d_free("n1-2", 3).unwrap()
    );
    assert!(Poly::parse_with("n01=2", 3, lenient).is_err());
    // forms the engine accepts stay accepted (unary signs, n0, n10)
    assert_eq!(
        Poly::parse_d_free("+-+n1", 3).unwrap(),
        Poly::parse_d_free("-n1", 3).unwrap()
    );
    assert!(Poly::parse_d_free("n0+n10", 11).is_ok());
}

/// A m3: `primitive_normal` computed `sign * (c/g)` unchecked: a leading coefficient `i128::MIN` with gcd 1 panicked
/// in debug (wrapped in release, giving a polynomial with another zero set).
#[test]
fn audit_a_m3_primitive_normal_is_checked() {
    let p = Poly::parse_d_free("n0 + (-(2^63))*(2^63)*2*n1", 3).unwrap();
    let q = p.primitive_normal(); // panicked before
    assert_eq!(q, p, "no representable normal form: returned unchanged");
    for k in [[0, 0, 0], [1, 0, 0], [0, 1, 0], [-3, 5, 1]] {
        assert_eq!(q.is_zero_at(&k).unwrap(), p.is_zero_at(&k).unwrap());
    }
    // ordinary case unchanged
    assert_eq!(
        Poly::parse_d_free("-4*n0+6*n1", 3)
            .unwrap()
            .primitive_normal(),
        Poly::parse_d_free("-2*n0+3*n1", 3).unwrap() // positive on the largest monomial n1
    );
}

/// A N1 (value part): the engine's zero test works modulo two 61-bit primes; a nonzero value divisible by both is
/// "zero" for the engine. The crate now detects exactly that case (the domain part is
/// `audit_a_n1_domain_fails_closed_on_engine_zero_disagreement`).
#[test]
fn audit_a_n1_value_detects_engine_modular_zero() {
    let pp = ENGINE_PRIMES[0] as i128 * ENGINE_PRIMES[1] as i128;
    assert!(Value::Small(pp).engine_zero_disagrees());
    assert!(Value::Small(-pp).engine_zero_disagrees());
    assert!(!Value::Small(pp + 1).engine_zero_disagrees());
    assert!(!Value::Small(ENGINE_PRIMES[0] as i128).engine_zero_disagrees());
    assert!(!Value::Small(0).engine_zero_disagrees());
    assert!(Value::Big(BigInt::from_i128(pp).mul(&BigInt::from_i128(3))).engine_zero_disagrees());
    assert!(!Value::Big(BigInt::from_i128(pp).mul(&BigInt::from_i128(3)).add(&BigInt::from_i128(1)))
        .engine_zero_disagrees());
}

/// A N2: `Value` derived `PartialEq` structurally, so `Small(0) != Big(0)`.
#[test]
fn audit_a_n2_value_equality_is_numeric() {
    assert_eq!(Value::Small(0), Value::Big(BigInt::zero()));
    assert_eq!(Value::Small(-7), Value::Big(BigInt::from_i128(-7)));
    assert_eq!(
        Value::Big(BigInt::from_i128(i128::MIN)),
        Value::Small(i128::MIN)
    );
    assert_ne!(Value::Small(1), Value::Big(BigInt::from_i128(2)));
    let huge = BigInt::from_i128(i128::MAX).mul(&BigInt::from_i128(4));
    assert_ne!(Value::Big(huge.clone()), Value::Small(i128::MAX));
    assert_eq!(Value::Big(huge.clone()), Value::Big(huge));
}

/// A N4: `Poly::var(slot as u16)` collided with the `d` variable at slot 65535.
#[test]
fn audit_a_n4_variable_cannot_collide_with_d() {
    assert!(matches!(
        Poly::parse_d_free("n65535", 70_000),
        Err(PolyError::VariableOutOfRange { .. })
    ));
    assert!(std::panic::catch_unwind(|| Poly::var(65_535)).is_err());
    assert!(!Poly::parse_d_free("n65534", 70_000).unwrap().has_d());
}
