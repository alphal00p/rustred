//! An external caller can name the sealed native-program bound in generic
//! wrappers without constructing a program, loading an artifact or reducing.
use rustred::{
    persistence::application_cache::ApplicationCacheBinding, reduction::ClosedUnitMassProgram,
    solver::CandidateUnitMassProgram,
};
use rustred_app::{NativeApplicationCache, NativeCachePreparationBackend, NativeCacheProgram};

fn session_binding<P: NativeCacheProgram>(
    session: &NativeApplicationCache<P>,
) -> &ApplicationCacheBinding {
    session.program().binding()
}

fn accept_optional_backend<P: NativeCacheProgram>(_: Option<NativeCachePreparationBackend<P>>) {}

#[test]
fn native_program_bound_is_public_for_both_native_program_types() {
    let _ = session_binding::<ClosedUnitMassProgram<'static>>;
    let _ = session_binding::<CandidateUnitMassProgram<1>>;
    accept_optional_backend::<ClosedUnitMassProgram<'static>>(None);
    accept_optional_backend::<CandidateUnitMassProgram<1>>(None);
}
