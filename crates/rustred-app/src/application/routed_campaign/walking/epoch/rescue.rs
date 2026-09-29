//! Resume-boundary rescue authority. Bitsets exclude future lookup authority;
//! neither quarantine nor abandonment removes historical records or edges.
use super::super::rescue::AmendmentRef;
use std::io::{self, Read, Write};

pub(super) struct State {
    pub amendments: Vec<AmendmentRef>,
    pub abandoned: Vec<u64>,
}

pub(super) fn contains(bits: &[u64], id: u32) -> bool {
    bits.get(id as usize / 64)
        .is_some_and(|word| word >> (id % 64) & 1 != 0)
}

pub(super) fn job_flags<const N: usize>(state: &super::state::EpochState<N>, id: u32) -> u16 {
    if state
        .rescue
        .as_ref()
        .is_some_and(|rescue| contains(&rescue.abandoned, id))
    {
        super::job::JOB_RESCUE_ABANDONED
    } else {
        0
    }
}

pub(super) fn count(bits: &[u64]) -> u64 {
    bits.iter().map(|word| u64::from(word.count_ones())).sum()
}

pub(super) fn valid(bits: &[u64], total: usize) -> bool {
    bits.len() == total.div_ceil(64)
        && (total % 64 == 0 || bits.last().is_none_or(|word| word >> (total % 64) == 0))
}

/// Two fixed-width bitsets with an explicit bounded header. Allocation is
/// determined by the authenticated arena watermark, never by an input count.
pub(super) fn write<W: Write>(
    total: u32,
    quarantine: &[u64],
    abandoned: &[u64],
    output: W,
) -> io::Result<(W, super::checkpoint::Digest)> {
    if !valid(quarantine, total as usize) || !valid(abandoned, total as usize) {
        return Err(io::Error::other("rescue bitset shape differs"));
    }
    let mut stream = super::checkpoint::Stream::new(output);
    stream.write_all(b"ERSC0001")?;
    stream.write_all(&u64::from(total).to_le_bytes())?;
    for bits in [quarantine, abandoned] {
        for word in bits {
            stream.write_all(&word.to_le_bytes())?;
        }
    }
    stream.finish()
}

pub(super) fn read(mut input: impl Read, total: u32) -> io::Result<(Vec<u64>, Vec<u64>)> {
    let mut header = [0u8; 16];
    input.read_exact(&mut header)?;
    if &header[..8] != b"ERSC0001"
        || u64::from_le_bytes(header[8..].try_into().unwrap()) != u64::from(total)
    {
        return Err(io::Error::other("rescue bitset header differs"));
    }
    let words = (total as usize).div_ceil(64);
    let mut read_bits = || -> io::Result<Vec<u64>> {
        let mut bits = Vec::new();
        bits.try_reserve_exact(words)
            .map_err(|_| io::Error::other("rescue bitset allocation"))?;
        for _ in 0..words {
            let mut word = [0u8; 8];
            input.read_exact(&mut word)?;
            bits.push(u64::from_le_bytes(word));
        }
        if !valid(&bits, total as usize) {
            return Err(io::Error::other("rescue bitset padding"));
        }
        Ok(bits)
    };
    let quarantine = read_bits()?;
    let abandoned = read_bits()?;
    if abandoned.iter().zip(&quarantine).any(|(a, q)| a & !q != 0) {
        return Err(io::Error::other("abandoned obligation outside quarantine"));
    }
    Ok((quarantine, abandoned))
}
