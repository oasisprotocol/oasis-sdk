#![feature(proc_macro_diagnostic)]
#![deny(rust_2018_idioms)]

use proc_macro::TokenStream;

mod event_derive;
#[cfg(test)]
mod test_utils;
mod version_from_cargo;

/// Derives the `Event` trait on an enum.
#[proc_macro_derive(Event, attributes(event))]
pub fn event_derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    event_derive::derive_event(input).into()
}

/// Constructs an `oasis_sdk::core::common::version::Version` from the Cargo.toml version.
#[proc_macro]
pub fn version_from_cargo(_input: TokenStream) -> TokenStream {
    version_from_cargo::version_from_cargo().into()
}
