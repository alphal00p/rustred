/// An exact affine self-map compiled into an intrinsic family permutation.
///
/// Construction is private to
/// [`compile`](crate::sector::symmetry::permutation::compile). The retained
/// inverse maps each target denominator to its unique source denominator. The
/// complete affine proof is consumed during compilation and is not duplicated
/// in the reusable transport object.
#[derive(Debug)]
pub struct Verified {
    pub(super) source_for_target: Box<[usize]>,
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

    pub(in crate::sector::symmetry) fn into_source_for_target(self) -> Box<[usize]> {
        self.source_for_target
    }
}
