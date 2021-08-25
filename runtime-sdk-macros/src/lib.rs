#![feature(box_patterns, once_cell, proc_macro_diagnostic, proc_macro_span)]
#![deny(rust_2018_idioms)]

use proc_macro::TokenStream;

mod error_derive;
mod event_derive;
mod generators;
mod handler_attrs;
#[cfg(test)]
mod test_utils;
mod version_from_cargo;

/// Derives the `Event` trait on an enum.
#[proc_macro_derive(Event, attributes(sdk_event))]
pub fn event_derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    event_derive::derive_event(input).into()
}

/// Derives the `Error` trait on an enum.
// The helper attribute is `sdk_error` to avoid conflict with `thiserror::Error`.
#[proc_macro_derive(Error, attributes(sdk_error, source, from))]
pub fn error_derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    error_derive::derive_error(input).into()
}

/// Constructs an `oasis_sdk::core::common::version::Version` from the Cargo.toml version.
#[proc_macro]
pub fn version_from_cargo(_input: TokenStream) -> TokenStream {
    version_from_cargo::version_from_cargo().into()
}

/// Creates traits for modules and clients to implement.
#[proc_macro_attribute]
pub fn calls(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemTrait);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    handler_attrs::gen_call_items(&input, &args).into()
}

/// Creates traits for modules and clients to implement.
#[proc_macro_attribute]
pub fn queries(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemTrait);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    handler_attrs::gen_query_items(&input, &args).into()
}

/// Registers the provided user-defined types as `Copy` when generating
/// handler methods and clients.
///
/// This macro must be invoked  before the `queries` or `calls` attribute macros.
/// Multiple invocations are additive.
///
/// ## Example
/// ```no_run
/// # use oasis_runtime_sdk_macros::register_copy_types;
/// register_copy_types!(ErrorKind, EncryptionKey)
/// ```
#[proc_macro]
pub fn register_copy_types(input: TokenStream) -> TokenStream {
    let parser = syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated;
    let input = syn::parse_macro_input!(input with parser);
    handler_attrs::register_copy_types(&input);
    TokenStream::new()
}

/// "Helper attributes" for the `calls` and `queries` "derives." This attribute could
/// be stripped by the `calls`/`queries` attributes, but if it's accidentally omitted,
/// not having this one will give really confusing error messages.
#[doc(hidden)]
#[proc_macro_attribute]
pub fn call(_args: TokenStream, input: TokenStream) -> TokenStream {
    // `sdk::method` can only be applied to methods, of course.
    let input = syn::parse_macro_input!(input as syn::TraitItemMethod);
    quote::quote!(#input).into()
}
#[doc(hidden)]
#[proc_macro_attribute]
pub fn query(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::TraitItemMethod);
    quote::quote!(#input).into()
}
