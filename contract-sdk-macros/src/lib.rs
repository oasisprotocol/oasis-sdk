#![feature(proc_macro_diagnostic)]
#![deny(rust_2018_idioms)]

use proc_macro::TokenStream;

mod error_derive;
mod event_derive;
#[cfg(test)]
mod test_utils;
mod util;

/// Derives the `Error` trait on an enum.
// The helper attribute is `sdk_error` to avoid conflict with `thiserror::Error`.
#[proc_macro_derive(Error, attributes(sdk_error, source, from))]
pub fn error_derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    error_derive::derive_error(input).into()
}

/// Derives the `Event` trait on an enum.
#[proc_macro_derive(Event, attributes(sdk_event))]
pub fn event_derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    event_derive::derive_event(input).into()
}

#[proc_macro]
pub fn create_contract(input: TokenStream) -> TokenStream {
    let contract_ident = syn::parse_macro_input!(input as syn::Ident);
    std::env::var_os("CARGO_PRIMARY_PACKAGE")
        .map(|_| quote::quote!(::oasis_contract_sdk::__create_contract!(#contract_ident);).into())
        .unwrap_or_default()
}
