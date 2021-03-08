#![deny(rust_2018_idioms)]

use proc_macro::TokenStream;

mod version_from_cargo;

/// Constructs an `oasis_sdk::core::common::version::Version` from the Cargo.toml version.
#[proc_macro]
pub fn version_from_cargo(_input: TokenStream) -> TokenStream {
    version_from_cargo::version_from_cargo().into()
}
