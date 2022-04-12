//! Module which contains utilities useful for testing and development.

pub mod keymanager;
pub mod keys;
pub mod mock;

/// Constructs a BTreeMap where keys are coerced to strings, and values to cbor::Value.
/// Syntax: `configmap! { "key" => value, ... }`.
macro_rules! configmap {
    // allow trailing comma
    ( $($key:expr => $value:expr,)+ ) => (configmap!($($key => $value),+));
    ( $($key:expr => $value:expr),* ) => {
        {
            let mut m = BTreeMap::new();
            $( m.insert($key.to_string(), cbor::to_value($value)); )*
            m
        }
    };
}
pub(crate) use configmap;
