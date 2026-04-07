//! Minimal integration with [uor-foundation](https://crates.io/crates/uor-foundation).

use uor_foundation::Primitives;

struct AppPrimitives;

impl Primitives for AppPrimitives {
    type String = str;
    type Integer = i64;
    type NonNegativeInteger = u64;
    type PositiveInteger = u64;
    type Decimal = f64;
    type Boolean = bool;
}

fn main() {
    let _ = std::any::type_name::<AppPrimitives>();
}
