#![feature(proc_macro_diagnostic)]
#![deny(rust_2018_idioms)]

use proc_macro::TokenStream;

mod error_derive;
mod event_derive;
mod evm_derive;
mod generators;
mod module_derive;
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

/// Derives the `EvmEvent` trait on a struct.
#[proc_macro_derive(EvmEvent, attributes(evm_event))]
pub fn evm_event_derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    evm_derive::derive_evm_event(input).into()
}

/// Derives the `EvmError` trait on an enum.
#[proc_macro_derive(EvmError, attributes(evm_error))]
pub fn evm_error_derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    evm_derive::derive_evm_error(input).into()
}

/// Derives traits from a non-trait `impl` block (rather than from a `struct`).
///
/// Only the `Module` and `EvmContract` traits are supported. In other words,
/// given an `impl MyModule` block, the macro derives implementations needed either
/// for implementing a module (see also the `#[handler]` and `#[migration]` attributes)
/// or for implementing an EVM contract (see also the `#[evm_method]` attribute).
#[proc_macro_attribute]
pub fn sdk_derive(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemImpl);
    match args.to_string().as_str() {
        "Module" => module_derive::derive_module(input).into(),
        "EvmContract" => evm_derive::derive_evm_contract(input).into(),
        _ => emit_compile_error(
            "#[sdk_derive] only supports #[sdk_derive(Module)] and #[sdk_derive(EvmContract)]",
        ),
    }
}

/// A helper attribute for `#[sdk_derive(...)]`. It doesn't do anyting on its own;
/// it only marks functions that represent a paratime method handler.
/// The permitted forms are:
///  - `#[handler(call = "my_module.MyCall")]`: Marks a function that handles
///    the "my_module.MyCall" call and can be passed to
///    oasis_runtime_sdk::module::dispatch_call.
///
///  - `#[handler(prefetch = "my_module.MyCall")]`: Marks a function that handles
///    the request to prefetch any data ahead of the "my_module.MyCall" call.
///    Its signature should be `Fn(
///      add_prefix: &mut dyn FnMut(Prefix) -> (),
///      body: cbor::Value,
///      auth_info: &AuthInfo,
///    ) -> Result<(), oasis_runtime_sdk::error::RuntimeError>`
///
///  - `#[handler(query = "my_module.MyQuery")]`: Marks a function that handles
///    the "my_module.MyQuery" query and can be passed to
///    oasis_runtime_sdk::module::dispatch_query.
///
///  - `#[handler(message_result = "my_module.MyMR")]`: Marks a function that handles
///    the "my_module.MyMR" message result and can be passed to
///    oasis_runtime_sdk::module::dispatch_message_result.
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

/// A helper attribute for `#[sdk_derive(...)]`. It doesn't do anything on its own;
/// it only marks functions that represent a module state migration.
///
/// The permitted forms are:
///  - `#[migration(init)]`: Marks the initial (genesis) migration.
///  - `#[migration(from = v)]`: Marks a migration from version v to v+1, where v is
///    a non-negative integer.
#[proc_macro_attribute]
pub fn migration(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// A helper attribute for `#[sdk_derive(...)]`. It doesn't do anything on its own;
/// it only marks functions that represent contract methods.
///
/// The permitted forms are:
///  - `#[evm_method(signature = "...")]`: The method selector is computed from
///    a Solidity method signature, and the method takes the precompute handle
///    and data offset as parameters.
///  - `#[evm_method(signature = "...", convert)]`: The method selector is
///    computed from the signature, the arguments are automatically decoded and
///    passed to the marked method, which must have the appropriate number and
///    type of arguments.
#[proc_macro_attribute]
pub fn evm_method(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// A helper attribute for `#[sdk_derive(...)]`. It doesn't do anything on its own;
/// it only marks the function within a contract implementation that returns its address.
///
/// The method marked with this attribute should take no arguments and return
/// an object of type `primitive_types::H160`.
#[proc_macro_attribute]
pub fn evm_contract_address(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Constructs an `oasis_sdk::core::common::version::Version` from the Cargo.toml version.
#[proc_macro]
pub fn version_from_cargo(_input: TokenStream) -> TokenStream {
    version_from_cargo::version_from_cargo().into()
}
