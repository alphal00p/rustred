//! Differential coverage of the allocation-free compact-image decode path.
use super::*;
use rustred::solver::DomainPowerSummary;

/// The former transport-domain decoder, retained only as a wire/error oracle.
fn transport_decode<const N: usize>(bytes: &[u8]) -> Result<CompactDomain<N>, &'static str> {
    let mut r = Reader::new(bytes);
    let phase = match r.u8()? {
        0 => Phase::Apply,
        1 => Phase::Route,
        _ => return Err("invalid image phase"),
    };
    let owner_bits = r.u32()?;
    if N < 32 && owner_bits >> N != 0 {
        return Err("image owner beyond arity");
    }
    let rank_present = match r.u8()? {
        0 => false,
        1 => true,
        _ => return Err("invalid flag byte"),
    };
    let rank_value = r.u32()?;
    if !rank_present && rank_value != 0 {
        return Err("non-canonical absent rank");
    }
    let mut lower = Vec::with_capacity(N);
    for _ in 0..N {
        lower.push(u64::from(r.u16()?));
    }
    let mut upper = Vec::with_capacity(N);
    for _ in 0..N {
        let value = r.u16()?;
        upper.push((value != u16::MAX).then_some(u64::from(value)));
    }
    let powers = r.powers()?;
    let domain = Domain {
        phase,
        owner: std::array::from_fn(|axis| owner_bits >> axis & 1 == 1),
        lower,
        upper,
        rank: rank_present.then_some(rank_value),
        powers,
    };
    CompactDomain::try_from_domain(&domain)
}

fn compare_wire<const N: usize>(bytes: &[u8]) {
    assert_eq!(
        read_image::<N>(&mut Reader::new(bytes)),
        transport_decode::<N>(bytes),
        "N={N}, wire={bytes:?}"
    );
}

fn fixture<const N: usize>(case: usize) -> Domain<N> {
    Domain {
        phase: if case % 2 == 0 {
            Phase::Apply
        } else {
            Phase::Route
        },
        owner: std::array::from_fn(|axis| (axis + case) % 3 != 0),
        lower: (0..N).map(|axis| ((axis + case) % 5) as u64).collect(),
        upper: (0..N)
            .map(|axis| ((axis + case) % 3 != 0).then_some(((axis + case) % 5 + 4) as u64))
            .collect(),
        rank: (case % 3 != 0).then_some(if case % 4 == 0 { u32::MAX } else { 20 }),
        powers: match case % 4 {
            0 => DomainPowerBounds::default(),
            1 => DomainPowerBounds {
                max_positive_power: Some(u64::MAX),
                min_power_difference: Some(i64::MIN),
                max_power_difference: Some(i64::MAX),
            },
            // Encoding must retain mathematically malformed bounds too: the
            // existing summary authority, not this codec, rejects them.
            2 => DomainPowerBounds {
                max_positive_power: None,
                min_power_difference: Some(7),
                max_power_difference: Some(-2),
            },
            _ => DomainPowerBounds {
                max_positive_power: Some(0),
                min_power_difference: None,
                max_power_difference: Some(0),
            },
        },
    }
}

fn wire_differential<const N: usize>() {
    for case in 0..12 {
        let domain = fixture::<N>(case);
        let image = CompactDomain::try_from_domain(&domain).unwrap();
        let mut w = Writer::default();
        write_image(&mut w, &image);
        let bytes = w.0;
        let decoded = read_image::<N>(&mut Reader::new(&bytes)).unwrap();
        assert_eq!(decoded, image);
        assert_eq!(decoded.expand(), domain);
        assert_eq!(decoded.digest(), image.digest());
        let mut written = Writer::default();
        write_image(&mut written, &decoded);
        assert_eq!(written.0, bytes);
        assert_eq!(
            decoded.try_native_summary(),
            DomainPowerSummary::try_new(
                domain.owner,
                &domain.lower,
                &domain.upper,
                domain.rank,
                domain.powers,
            )
        );
        for end in 0..=bytes.len() {
            compare_wire::<N>(&bytes[..end]);
        }
        for offset in 0..bytes.len() {
            for value in [0, 1, 2, 255] {
                let mut mutated = bytes.clone();
                mutated[offset] = value;
                compare_wire::<N>(&mutated);
            }
        }
    }
}

#[test]
fn compact_image_stack_decode_matches_transport_bytes_and_errors() {
    wire_differential::<1>();
    wire_differential::<2>();
    wire_differential::<6>();
    wire_differential::<10>();
    wire_differential::<15>();
    wire_differential::<16>();
    wire_differential::<32>();
}

#[test]
fn compact_image_coordinate_sentinels_preserve_decode_error_priority() {
    let mut domain = fixture::<2>(0);
    domain.lower = vec![65534, 0];
    domain.upper = vec![Some(65534), None];
    let image = CompactDomain::try_from_domain(&domain).unwrap();
    let mut w = Writer::default();
    write_image(&mut w, &image);
    assert_eq!(read_image::<2>(&mut Reader::new(&w.0)), Ok(image));
    // Coordinate validation still follows reading all wire fields. A truncated
    // power bound must therefore win over the earlier invalid lower value.
    let lower_at = 1 + 4 + 1 + 4;
    w.0[lower_at..lower_at + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(
        read_image::<2>(&mut Reader::new(&w.0)),
        Err(
            "domain coordinate exceeds the compact queue range (finite coordinates must be <= 65534)"
        )
    );
    assert_eq!(
        read_image::<2>(&mut Reader::new(&w.0[..w.0.len() - 1])),
        Err("truncated bytes")
    );
    let powers_at = lower_at + 4 * 2;
    w.0[powers_at] = 2;
    assert_eq!(
        read_image::<2>(&mut Reader::new(&w.0)),
        Err("invalid flag byte")
    );
}
