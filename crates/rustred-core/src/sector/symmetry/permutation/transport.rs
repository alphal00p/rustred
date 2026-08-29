use crate::sector::Restrictions;

use super::{Error, TransportError, Verified};

impl Verified {
    /// Check a cut/pattern policy when selecting this intrinsic permutation
    /// for application. The policy is not cloned or retained.
    pub fn check_restrictions(&self, restrictions: &Restrictions) -> Result<(), Error> {
        let expected = self.denominator_count();
        if restrictions.arity() != expected {
            return Err(Error::WrongRestrictionArity {
                expected,
                actual: restrictions.arity(),
            });
        }

        for (target, &source) in self.source_for_target.iter().enumerate() {
            let source_cut = restrictions.cuts().required_active().active_bits()[source];
            let target_cut = restrictions.cuts().required_active().active_bits()[target];
            if source_cut != target_cut {
                return Err(Error::CutMismatch { source, target });
            }
            if restrictions.pattern().slots()[source] != restrictions.pattern().slots()[target] {
                return Err(Error::PatternMismatch { source, target });
            }
        }
        Ok(())
    }

    /// Transport source powers into reusable caller-owned target storage.
    ///
    /// If `D_source[i] = D_target[pi(i)]`, this writes
    /// `target[pi(i)] = source[i]`. Call [`Self::check_restrictions`] once when
    /// selecting the permutation for a concrete reduction policy; repeated
    /// transports then perform no allocation and no policy scan.
    pub fn transport_into(&self, source: &[i64], target: &mut [i64]) -> Result<(), TransportError> {
        let expected = self.denominator_count();
        if source.len() != expected {
            return Err(TransportError::WrongSourceArity {
                expected,
                actual: source.len(),
            });
        }
        if target.len() != expected {
            return Err(TransportError::WrongTargetArity {
                expected,
                actual: target.len(),
            });
        }
        for (target_denominator, &source_denominator) in self.source_for_target.iter().enumerate() {
            target[target_denominator] = source[source_denominator];
        }
        Ok(())
    }
}
