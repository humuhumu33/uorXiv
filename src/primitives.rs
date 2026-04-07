use uor_foundation::Primitives;

/// Concrete primitive family for this sandbox: owned [`String`] for addresses and digests.
#[derive(Debug, Default, Clone, Copy)]
pub struct AppPrimitives;

impl Primitives for AppPrimitives {
    type String = String;
    type Integer = i64;
    type NonNegativeInteger = u64;
    type PositiveInteger = u64;
    type Decimal = f64;
    type Boolean = bool;
}
