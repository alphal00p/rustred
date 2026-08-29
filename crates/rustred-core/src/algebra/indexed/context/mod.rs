//! Construction, authentication, and exact field arithmetic.

mod arithmetic;
mod binding;
mod construction;
mod model;
mod values;

pub(super) use construction::authenticate_index_symbol;
pub(crate) use model::BoundIndexedCoefficient;
pub use model::IndexedCoefficientContext;
