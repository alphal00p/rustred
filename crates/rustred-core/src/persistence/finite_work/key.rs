use super::*;

/// Exactly the native Work order: phase, literal Apply owner, integral powers.
/// It is a plain wire key, not a closed-work certificate or terminal identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, bincode::Encode, bincode::Decode)]
pub enum GeneratedWorkKey {
    Route(Vec<i64>),
    Apply { owner: Vec<bool>, powers: Vec<i64> },
}
impl GeneratedWorkKey {
    pub fn from_native<const N: usize>(
        work: &crate::solver::CandidateRoutedWork<N>,
    ) -> GeneratedWorkResult<Self> {
        use crate::solver::CandidateRoutedWork;
        let arity = work.target().powers().len();
        if !crate::fits_storage(arity, N) {
            return Err(corrupt("native Work has invalid storage arity"));
        }
        let value = match work {
            CandidateRoutedWork::Route(key) => Self::Route(key.powers().to_vec()),
            CandidateRoutedWork::Apply {
                owner_sector,
                target,
            } => {
                if owner_sector[arity..].iter().any(|&active| active) {
                    return Err(corrupt("nonzero stored owner padding"));
                }
                Self::Apply {
                    owner: owner_sector[..arity].to_vec(),
                    powers: target.powers().to_vec(),
                }
            }
        };
        value.validate(arity)?;
        Ok(value)
    }
    pub fn to_native<const N: usize>(
        &self,
    ) -> GeneratedWorkResult<crate::solver::CandidateRoutedWork<N>> {
        use crate::{family::IntegralKey, solver::CandidateRoutedWork};
        let arity = self.powers().len();
        self.validate(arity)?;
        if !crate::fits_storage(arity, N) {
            return Err(corrupt("stored Work exceeds native storage arity"));
        }
        let key = IntegralKey::try_new(self.powers().iter().copied()).map_err(corrupt)?;
        Ok(match self {
            Self::Route(_) => CandidateRoutedWork::Route(key),
            Self::Apply { owner, .. } => CandidateRoutedWork::Apply {
                owner_sector: std::array::from_fn(|i| owner.get(i).copied().unwrap_or(false)),
                target: key,
            },
        })
    }
    pub fn powers(&self) -> &[i64] {
        match self {
            Self::Route(powers) | Self::Apply { powers, .. } => powers,
        }
    }
    pub fn validate(&self, arity: usize) -> GeneratedWorkResult<()> {
        if arity == 0 || self.powers().len() != arity {
            return Err(corrupt("stored Work arity"));
        }
        if let Self::Apply { owner, powers } = self {
            if owner.len() != arity
                || owner
                    .iter()
                    .zip(powers)
                    .any(|(&active, &power)| active != (power > 0))
            {
                return Err(corrupt("stored Apply owner differs from physical support"));
            }
        }
        Ok(())
    }
    pub fn heap_bytes(&self) -> GeneratedWorkResult<usize> {
        let (powers, owner) = match self {
            Self::Route(powers) => (powers, 0),
            Self::Apply { owner, powers } => (powers, owner.capacity()),
        };
        powers
            .capacity()
            .checked_mul(std::mem::size_of::<i64>())
            .and_then(|n| n.checked_add(owner))
            .ok_or_else(|| resource("stored key payload overflow"))
    }
}
