//! Terminal keys of the Ordered engine (owner: M2-I0 rr-iface).
//!
//! A terminal `(PTerminalId, GammaTag)` travels as the `IntegralKey`
//! `[id / 128^2, id / 128 % 128, id % 128] ++ dense tag slots`, so every slot of
//! a recorded step identity fits `i8` (plan M2 section 3.5).
use crate::family::IntegralKey;

use super::error::PError;
use super::tag::GammaTag;
use super::{P_GAMMA_TAG_WIDTH, PTerminalId};

const BASE: u32 = 128;

pub struct PTerminalCodec;

impl PTerminalCodec {
    /// First terminal id without an encoding.
    pub const ID_LIMIT: u32 = BASE * BASE * BASE;
    /// Slots of every terminal key.
    pub const KEY_SLOTS: usize = 3 + P_GAMMA_TAG_WIDTH;

    pub fn encode(id: PTerminalId, tag: &GammaTag) -> Result<IntegralKey, PError> {
        if id.0 >= Self::ID_LIMIT {
            return Err(PError::MalformedLibrary(format!(
                "terminal id {} has no key encoding (limit {})",
                id.0,
                Self::ID_LIMIT
            )));
        }
        let mut slots = Vec::with_capacity(Self::KEY_SLOTS);
        slots.extend([id.0 / (BASE * BASE), id.0 / BASE % BASE, id.0 % BASE].map(i64::from));
        tag.encode_dense(&mut slots)?;
        IntegralKey::try_new(slots).map_err(|error| PError::MalformedLibrary(error.to_string()))
    }

    pub fn decode(key: &IntegralKey) -> Result<(PTerminalId, GammaTag), PError> {
        let powers = key.powers();
        if powers.len() != Self::KEY_SLOTS {
            return Err(PError::MalformedLibrary(format!(
                "a terminal key has {} slots, not {}",
                Self::KEY_SLOTS,
                powers.len()
            )));
        }
        let (id_slots, tag_slots) = powers.split_at(3);
        let mut id = 0;
        for &slot in id_slots {
            let digit = u32::try_from(slot)
                .ok()
                .filter(|&digit| digit < BASE)
                .ok_or_else(|| {
                    PError::MalformedLibrary(format!("terminal id digit {slot} out of range"))
                })?;
            id = id * BASE + digit;
        }
        Ok((PTerminalId(id), GammaTag::decode_dense(tag_slots)?))
    }
}
