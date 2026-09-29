//! Thread-owned Symbolica polynomial contexts and context seals for the
//! parallel native hot path.
//!
//! Inspector threads read one shared copy of the owner programs. Every
//! Symbolica polynomial holds an `Arc` of its context (field and variable
//! map), and every clone, derived polynomial (`zero`, `replace`, GCD
//! intermediates, ...) and drop updates the reference counts of the context
//! and variable map it came from. With all threads deriving from the same
//! owner polynomials, those counts (and RustRed's `Arc<String>` context seal
//! in every `IndexedCoefficient`/`IndexedPolynomial`) are written by every
//! core, and at 96 threads their cache lines move between L3 domains on
//! nearly every operation (W0.3 harness, `TMP/w0/harness/RESULTS.md`
//! section 5; Symbolica MRE `TMP/symbolica-main/mre-archive`).
//!
//! The helpers below derive new polynomials on a context (and variable map)
//! owned by the calling thread, through Symbolica's
//! `MultivariatePolynomial::zero_with_new_context` and
//! `clone_with_context_of`, and hand out a per-thread copy of a context seal.
//! Values are identical (same coefficients, exponents, variables and seal
//! string); only the identity of the context allocation differs, so results
//! stay byte-identical. The shared owner data are then only read.
//!
//! Each cache entry keeps a `Weak` of the shared allocation it was derived
//! from: the allocation cannot be freed and its address reused while the
//! entry exists, so the lookup compares addresses only (no atomic write and
//! no content comparison on the hot path). Entries whose shared value was
//! dropped are pruned when a new entry is inserted.
use std::cell::RefCell;
use std::sync::{Arc, Weak};

use symbolica::prelude::PolyVariable;

use super::CoefficientPolynomial;

struct Template {
    shared_map: Weak<Vec<PolyVariable>>,
    template: CoefficientPolynomial,
}

struct Seal {
    shared: Weak<String>,
    local: Arc<String>,
}

thread_local! {
    static TEMPLATES: RefCell<Vec<Template>> = const { RefCell::new(Vec::new()) };
    static SEALS: RefCell<Vec<Seal>> = const { RefCell::new(Vec::new()) };
}

/// Run `f` on this thread's zero template for `shared`'s variable map.
fn with_template<R>(
    shared: &CoefficientPolynomial,
    f: impl FnOnce(&CoefficientPolynomial) -> R,
) -> R {
    let key = Arc::as_ptr(shared.variables());
    TEMPLATES.with_borrow_mut(|templates| {
        let index = match templates
            .iter()
            .position(|entry| std::ptr::eq(entry.shared_map.as_ptr(), key))
        {
            Some(index) => index,
            None => {
                templates.retain(|entry| entry.shared_map.strong_count() > 0);
                templates.push(Template {
                    shared_map: Arc::downgrade(shared.variables()),
                    template: shared.zero_with_new_context(),
                });
                templates.len() - 1
            }
        };
        f(&templates[index].template)
    })
}

/// `shared.clone()` on the calling thread's context for the same field and
/// variable map.
pub(crate) fn clone_thread_owned(shared: &CoefficientPolynomial) -> CoefficientPolynomial {
    with_template(shared, |template| shared.clone_with_context_of(template))
}

/// `shared.zero_with_capacity(capacity)` on the calling thread's context
/// for the same field and variable map.
pub(crate) fn zero_with_capacity_thread_owned(
    shared: &CoefficientPolynomial,
    capacity: usize,
) -> CoefficientPolynomial {
    with_template(shared, |template| template.zero_with_capacity(capacity))
}

/// This thread's copy of a shared context seal (equal string).
pub(crate) fn thread_owned_seal(shared: &Arc<String>) -> Arc<String> {
    let key = Arc::as_ptr(shared);
    SEALS.with_borrow_mut(|seals| {
        if let Some(entry) = seals
            .iter()
            .find(|entry| std::ptr::eq(entry.shared.as_ptr(), key))
        {
            return entry.local.clone();
        }
        seals.retain(|entry| entry.shared.strong_count() > 0);
        let local = Arc::new(shared.as_str().to_owned());
        seals.push(Seal {
            shared: Arc::downgrade(shared),
            local: local.clone(),
        });
        local
    })
}

/// Whether `candidate` is this thread's copy of `shared` (a pointer test; a
/// copy made by another thread is recognized by the caller's string
/// comparison instead).
pub(crate) fn is_thread_owned_seal_of(shared: &Arc<String>, candidate: &Arc<String>) -> bool {
    let key = Arc::as_ptr(shared);
    SEALS.with_borrow(|seals| {
        seals.iter().any(|entry| {
            std::ptr::eq(entry.shared.as_ptr(), key) && Arc::ptr_eq(&entry.local, candidate)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbolica::prelude::{Integer, MultivariatePolynomial, Z};

    #[test]
    fn thread_owned_copies_are_equal_values_on_a_private_context() {
        let variables: Arc<Vec<PolyVariable>> =
            Arc::new((0..3).map(PolyVariable::Temporary).collect());
        let mut shared = MultivariatePolynomial::<_, u16>::new(&Z, None, variables.clone());
        shared.append_monomial(Integer::from(3), &[1, 0, 2]);
        shared.append_monomial(Integer::from(-5), &[0, 2, 0]);
        let strong_before = Arc::strong_count(&variables);

        let local = clone_thread_owned(&shared);
        assert_eq!(local, shared);
        assert_eq!(local.coefficients, shared.coefficients);
        assert_eq!(local.exponents, shared.exponents);
        assert!(!Arc::ptr_eq(local.variables(), shared.variables()));
        assert_eq!(local.variables().as_ref(), shared.variables().as_ref());
        // The derived copy and later calls do not hold the shared map.
        let again = clone_thread_owned(&shared);
        assert!(Arc::ptr_eq(again.variables(), local.variables()));
        let zero = zero_with_capacity_thread_owned(&shared, 4);
        assert!(zero.is_zero());
        assert!(Arc::ptr_eq(zero.variables(), local.variables()));
        assert_eq!(Arc::strong_count(&variables), strong_before);

        // Another thread gets its own context.
        let other = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let copy = clone_thread_owned(&shared);
                    (Arc::as_ptr(copy.variables()) as usize, copy == shared)
                })
                .join()
                .unwrap()
        });
        assert_ne!(other.0, Arc::as_ptr(local.variables()) as usize);
        assert!(other.1);
    }

    #[test]
    fn thread_owned_seals_are_equal_strings_recognized_by_pointer() {
        let shared = Arc::new("context seal".to_owned());
        let local = thread_owned_seal(&shared);
        assert_eq!(local.as_str(), shared.as_str());
        assert!(!Arc::ptr_eq(&local, &shared));
        assert!(Arc::ptr_eq(&thread_owned_seal(&shared), &local));
        assert!(is_thread_owned_seal_of(&shared, &local));
        assert!(!is_thread_owned_seal_of(&shared, &shared));
        let unrelated = Arc::new("context seal".to_owned());
        assert!(!is_thread_owned_seal_of(&unrelated, &local));
        assert_eq!(Arc::strong_count(&shared), 1);
    }
}
