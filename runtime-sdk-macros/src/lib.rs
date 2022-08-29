#![feature(proc_macro_diagnostic)]
#![deny(rust_2018_idioms)]

use proc_macro::TokenStream;

mod error_derive;
mod event_derive;
mod generators;
mod method_handler_derive;
#[cfg(test)]
mod test_utils;
mod version_from_cargo;

/// Emits a compile-time error `msg` from a macro, and uses the span of the
/// macro invocation if possible. If span info is not available (should only
/// happen in unit tests), panics with `msg`.
fn emit_compile_error<S: Into<String> + Clone + std::panic::RefUnwindSafe>(msg: S) -> ! {
    std::panic::catch_unwind(|| {
        proc_macro2::Span::call_site()
            .unwrap()
            .error(msg.clone().into())
            .emit();
    })
    .ok()
    .or_else(|| {
        panic!("{}", msg.into());
    });
    unreachable!(); // error().emit() already halts compilation, but type checker doesn't know that
}

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

/// Derives traits from a non-trait `impl` block (rather than from a `struct`).
///
/// Only the `MethodHandler` trait is supported. In other words, given an
/// `impl MyModule` block, the macro derives `impl MethodHandler for MyModule`.
/// See also the `#[handler]` attribute.
#[proc_macro_attribute]
pub fn sdk_derive(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemImpl);
    if args.to_string() == "MethodHandler" {
        method_handler_derive::derive_method_handler(input).into()
    } else {
        emit_compile_error("#[sdk_derive] only supports #[sdk_derive(MethodHandler)]");
    }
}

/// A helper attribute for `#[sdk_derive(...)]`. It doesn't do anyting on its own;
/// it only mark functions that represent a paratime method handler.
/// The permitted forms are:
///  - `#[handler(call = "my_module.MyCall")]`: Marks a function that handles
///        the "my_module.MyCall" call and can be passed to
///        oasis_runtime_sdk::module::dispatch_call.
///  - `#[handler(prefetch = "my_module.MyCall")]`: Marks a function that handles
///        the request to prefetch any data ahead of the "my_module.MyCall" call.
///        Its signature should be `Fn(
///          add_prefix: &mut dyn FnMut(Prefix) -> (),
///          body: cbor::Value,
///          auth_info: &AuthInfo,
///        ) -> Result<(), oasis_runtime_sdk::error::RuntimeError>`
///  - `#[handler(query = "my_module.MyQuery")]`: Marks a function that handles
///        the "my_module.MyQuery" query and can be passed to
///        oasis_runtime_sdk::module::dispatch_query.
///  - `#[handler(message_result = "my_module.MyMR")]`: Marks a function that handles
///        the "my_module.MyMR" message result and can be passed to
///        oasis_runtime_sdk::module::dispatch_message_result.
///
/// Query handler can also contain the `expensive` tag. Example:
/// `#[handler(query = "my_module.MyQuery", expensive)]`.
/// Queries tagged `expensive` can be enabled/disabled are disabled by default to avoid
/// excessive costs to the node operator. This can be overridden in the node config.
///
/// NOTE: This attribute is parsed by the `#[sdk_derive(...)]` macro, which cannot
/// interpret the attribute name semantically. Use `#[handler]`, not
/// `#[oasis_runtime_sdk_macros::handler]` or other paths/aliases.
#[proc_macro_attribute]
pub fn handler(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Constructs an `oasis_sdk::core::common::version::Version` from the Cargo.toml version.
#[proc_macro]
pub fn version_from_cargo(_input: TokenStream) -> TokenStream {
    version_from_cargo::version_from_cargo().into()
}
