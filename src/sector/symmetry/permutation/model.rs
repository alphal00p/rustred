use super::super::VerifiedMap;

/// An exact affine self-map compiled into an intrinsic family permutation.
///
/// Construction is private to
/// [`compile`](crate::sector::symmetry::permutation::compile). The retained
/// inverse maps each target denominator to its unique source denominator,
/// while `affine` owns the complete exact proof from which that action was
/// derived.
#[derive(Debug)]
pub struct Verified {
    pub(super) source_for_target: Box<[usize]>,
    pub(super) affine: VerifiedMap,
}

impl Verified {
    /// Number of source and target denominators in the permutation.
    pub fn denominator_count(&self) -> usize {
        self.source_for_target.len()
    }

    /// Inverse action: `source_for_target[j]` is the source denominator whose
    /// image is target denominator `j`.
    pub fn source_for_target(&self) -> &[usize] {
        &self.source_for_target
    }

    /// Complete exact affine proof retained by this compiled permutation.
    pub const fn affine(&self) -> &VerifiedMap {
        &self.affine
    }
}
