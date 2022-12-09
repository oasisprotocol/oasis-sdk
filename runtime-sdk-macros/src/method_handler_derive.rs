use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;

use crate::{emit_compile_error, generators as gen};

/// Given an `impl MyModule` block, produces an `impl MethodHandler for MyModule`.
/// See `sdk_derive()` in lib.rs for details.
pub fn derive_method_handler(impl_block: syn::ItemImpl) -> TokenStream {
    let sdk_crate = gen::sdk_crate_path();
    let module_generics = &impl_block.generics;
    let module_ty = &impl_block.self_ty;

    /// If `item` is a method handler, parses and returns its properties from the attributes.
    fn maybe_parse_handler(item: &syn::ImplItem) -> Option<HandlerInfo> {
        // Consider only fns
        let method = match item {
            syn::ImplItem::Method(m) => m,
            _ => return None,
        };
        Some(HandlerInfo {
            attrs: parse_attrs(&method.attrs)?,
            ident: method.sig.ident.clone(),
        })
    }

    let (handlers, nonhandlers): (Vec<ParsedImplItem>, Vec<ParsedImplItem>) = impl_block
        .items
        .into_iter()
        .map(|item| ParsedImplItem {
            handler: maybe_parse_handler(&item),
            item,
        })
        .partition(|p| p.handler.is_some());

    let handler_items = handlers
        .iter()
        .map(|ParsedImplItem { item, .. }| item)
        .collect::<Vec<_>>();
    let nonhandler_items = nonhandlers
        .into_iter()
        .map(|ParsedImplItem { item, .. }| item)
        .collect::<Vec<_>>();

    /// Generates parallel vectors of rpc names and handler idents for all handlers of kind `kind`.
    fn filter_by_kind(
        handlers: &[ParsedImplItem],
        kind: HandlerKind,
    ) -> (Vec<syn::Expr>, Vec<syn::Ident>) {
        handlers
            .iter()
            .filter_map(|h| h.handler.as_ref())
            .filter(|h| h.attrs.kind == kind)
            .map(|h| (h.attrs.rpc_name.clone(), h.ident.clone()))
            .unzip()
    }

    let prefetch_impl = {
        let (handler_names, handler_idents) = filter_by_kind(&handlers, HandlerKind::Prefetch);

        // Find call handlers; for every call handler without a corresponding prefetch handler, we'll
        // generate a dummy prefetch handler.
        let (call_handler_names, _) = filter_by_kind(&handlers, HandlerKind::Call);
        let handler_names_without_impl: Vec<&syn::Expr> = call_handler_names
            .iter()
            .filter(|n| !handler_names.contains(n))
            .collect();

        if handler_names.is_empty() {
            quote! {}
        } else {
            quote! {
                fn prefetch(
                    prefixes: &mut BTreeSet<Prefix>,
                    method: &str,
                    body: cbor::Value,
                    auth_info: &AuthInfo,
                ) -> module::DispatchResult<cbor::Value, Result<(), sdk::error::RuntimeError>> {
                    let mut add_prefix = |p| {prefixes.insert(p);};
                    match method {
                        // "Real", user-defined prefetch handlers.
                        #(
                          #handler_names => module::DispatchResult::Handled(
                            Self::#handler_idents(&mut add_prefix, body, auth_info)
                          ),
                        )*
                        // No-op prefetch handlers.
                        #(
                          #handler_names_without_impl => module::DispatchResult::Handled(Ok(())),
                        )*
                        _ => module::DispatchResult::Unhandled(body),
                    }
                }
            }
        }
    };

    let dispatch_call_impl = {
        let (handler_names, handler_idents) = filter_by_kind(&handlers, HandlerKind::Call);

        if handler_names.is_empty() {
            quote! {}
        } else {
            quote! {
                fn dispatch_call<C: TxContext>(
                    ctx: &mut C,
                    method: &str,
                    body: cbor::Value,
                ) -> DispatchResult<cbor::Value, CallResult> {
                    match method {
                        #(
                          #handler_names => module::dispatch_call(ctx, body, Self::#handler_idents),
                        )*
                        _ => DispatchResult::Unhandled(body),
                    }
                }
            }
        }
    };

    let query_parameters_impl = {
        quote! {
            fn query_parameters<C: Context>(ctx: &mut C, _args: ()) -> Result<<Self as module::Module>::Parameters, <Self as module::Module>::Error> {
                Ok(Self::params(ctx.runtime_state()))
            }
        }
    };

    let dispatch_query_impl = {
        let (handler_names, handler_idents) = filter_by_kind(&handlers, HandlerKind::Query);

        if handler_names.is_empty() {
            quote! {
                fn dispatch_query<C: Context>(
                    ctx: &mut C,
                    method: &str,
                    args: cbor::Value,
                ) -> DispatchResult<cbor::Value, Result<cbor::Value, sdk::error::RuntimeError>> {
                    match method {
                        q if q == format!("{}.Parameters", Self::NAME) => module::dispatch_query(ctx, args, Self::query_parameters),
                        _ => DispatchResult::Unhandled(args),
                    }
                }
            }
        } else {
            quote! {
                fn dispatch_query<C: Context>(
                    ctx: &mut C,
                    method: &str,
                    args: cbor::Value,
                ) -> DispatchResult<cbor::Value, Result<cbor::Value, sdk::error::RuntimeError>> {
                    match method {
                        #(
                          #handler_names => module::dispatch_query(ctx, args, Self::#handler_idents),
                        )*
                        q if q == format!("{}.Parameters", Self::NAME) => module::dispatch_query(ctx, args, Self::query_parameters),
                        _ => DispatchResult::Unhandled(args),
                    }
                }
            }
        }
    };

    let dispatch_message_result_impl = {
        let (handler_names, handler_idents) = filter_by_kind(&handlers, HandlerKind::MessageResult);

        if handler_names.is_empty() {
            quote! {}
        } else {
            quote! {
                fn dispatch_message_result<C: Context>(
                    ctx: &mut C,
                    handler_name: &str,
                    result: MessageResult,
                ) -> DispatchResult<MessageResult, ()> {
                    match handler_name {
                        #(
                          #handler_names => {
                              Self::#handler_idents(
                                  ctx,
                                  result.event,
                                  cbor::from_value(result.context).expect("invalid message handler context"),
                              );
                              DispatchResult::Handled(())
                          }
                        )*
                        _ => DispatchResult::Unhandled(result),
                    }
                }
            }
        }
    };

    let supported_methods_impl = {
        let (handler_names, handler_kinds): (Vec<syn::Expr>, Vec<syn::Path>) = handlers
            .iter()
            .filter_map(|h| h.handler.as_ref())
            // `prefetch` is an implementation detail of `call` handlers, so we don't list them
            .filter(|h| h.attrs.kind != HandlerKind::Prefetch)
            .map(|h| (h.attrs.rpc_name.clone(), h.attrs.kind.as_sdk_ident()))
            .unzip();
        if handler_names.is_empty() {
            quote! {}
        } else {
            quote! {
                fn supported_methods() -> Vec<core_types::MethodHandlerInfo> {
                    vec![ #(
                        core_types::MethodHandlerInfo {
                            kind: #handler_kinds,
                            name: #handler_names.to_string(),
                        },
                    )* ]
                }
            }
        }
    };

    let expensive_queries_impl = {
        let handler_names: Vec<syn::Expr> = handlers
            .iter()
            .filter_map(|h| h.handler.as_ref())
            .filter(|h| h.attrs.kind == HandlerKind::Query && h.attrs.is_expensive)
            .map(|h| h.attrs.rpc_name.clone())
            .collect();
        if handler_names.is_empty() {
            quote! {}
        } else {
            quote! {
                fn is_expensive_query(method: &str) -> bool {
                    [ #( #handler_names, )* ].contains(&method)
                }
            }
        }
    };

    let allowed_private_km_queries_impl = {
        let handler_names: Vec<syn::Expr> = handlers
            .iter()
            .filter_map(|h| h.handler.as_ref())
            .filter(|h| h.attrs.kind == HandlerKind::Query && h.attrs.allow_private_km)
            .map(|h| h.attrs.rpc_name.clone())
            .collect();
        if handler_names.is_empty() {
            quote! {}
        } else {
            quote! {
                fn is_allowed_private_km_query(method: &str) -> bool {
                    [ #( #handler_names, )* ].contains(&method)
                }
            }
        }
    };

    let allowed_interactive_calls_impl = {
        let handler_names: Vec<syn::Expr> = handlers
            .iter()
            .filter_map(|h| h.handler.as_ref())
            .filter(|h| h.attrs.kind == HandlerKind::Call && h.attrs.allow_interactive)
            .map(|h| h.attrs.rpc_name.clone())
            .collect();
        if handler_names.is_empty() {
            quote! {}
        } else {
            quote! {
                fn is_allowed_interactive_call(method: &str) -> bool {
                    [ #( #handler_names, )* ].contains(&method)
                }
            }
        }
    };

    gen::wrap_in_const(quote! {
        use #sdk_crate::{
          self as sdk,
          cbor,
          error::Error as _,
          module::{DispatchResult, CallResult},
          modules::core::types as core_types,
          types::message::MessageResult
        };

        impl #module_generics sdk::module::MethodHandler for #module_ty {
            #(#nonhandler_items)*

            #prefetch_impl
            #dispatch_call_impl
            #dispatch_query_impl
            #dispatch_message_result_impl
            #supported_methods_impl
            #expensive_queries_impl
            #allowed_private_km_queries_impl
            #allowed_interactive_calls_impl
        }

        impl #module_generics #module_ty {
            #query_parameters_impl

            #(#handler_items)*
        }
    })
}

/// An item (in the `syn` sense, i.e. a fn, type, comment, etc) in an `impl` block,
/// plus parsed data about its #[handler] attribute, if any.
#[derive(Clone)]
struct ParsedImplItem {
    item: syn::ImplItem,
    handler: Option<HandlerInfo>,
}

#[derive(Clone, Debug)]
struct HandlerInfo {
    attrs: MethodHandlerAttr,
    /// Name of the handler function.
    ident: syn::Ident,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum HandlerKind {
    Call,
    Query,
    MessageResult,
    Prefetch,
}

impl HandlerKind {
    fn as_sdk_ident(&self) -> syn::Path {
        match self {
            HandlerKind::Call => parse_quote!(core_types::MethodHandlerKind::Call),
            HandlerKind::Query => parse_quote!(core_types::MethodHandlerKind::Query),
            HandlerKind::MessageResult => {
                parse_quote!(core_types::MethodHandlerKind::MessageResult)
            }
            HandlerKind::Prefetch => {
                unimplemented!("prefetch cannot be expressed in core::types::MethodHandlerKind")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct MethodHandlerAttr {
    kind: HandlerKind,
    /// Name of the RPC that this handler handles, e.g. "my_module.MyQuery".
    rpc_name: syn::Expr,
    /// Whether this handler is tagged as expensive. Only applies to query handlers.
    is_expensive: bool,
    /// Whether this handler is tagged as allowing access to private key manager state. Only applies
    /// to query handlers.
    allow_private_km: bool,
    /// Whether this handler is tagged as allowing interactive calls. Only applies to call handlers.
    allow_interactive: bool,
}
impl syn::parse::Parse for MethodHandlerAttr {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let kind: syn::Ident = input.parse()?;
        let kind = match kind.to_string().as_str() {
            "call" => HandlerKind::Call,
            "query" => HandlerKind::Query,
            "message_result" => HandlerKind::MessageResult,
            "prefetch" => HandlerKind::Prefetch,
            _ => return Err(syn::Error::new(kind.span(), "invalid handler kind")),
        };
        let _: syn::token::Eq = input.parse()?;
        let rpc_name: syn::Expr = input.parse()?;

        // Parse optional comma-separated tags.
        let mut is_expensive = false;
        let mut allow_private_km = false;
        let mut allow_interactive = false;
        while input.peek(syn::token::Comma) {
            let _: syn::token::Comma = input.parse()?;
            let tag: syn::Ident = input.parse()?;

            if tag == "expensive" {
                if kind != HandlerKind::Query {
                    return Err(syn::Error::new(
                        tag.span(),
                        "`expensive` tag is only allowed on `query` handlers",
                    ));
                }
                is_expensive = true;
            } else if tag == "allow_private_km" {
                if kind != HandlerKind::Query {
                    return Err(syn::Error::new(
                        tag.span(),
                        "`allow_private_km` tag is only allowed on `query` handlers",
                    ));
                }
                allow_private_km = true;
            } else if tag == "allow_interactive" {
                if kind != HandlerKind::Call {
                    return Err(syn::Error::new(
                        tag.span(),
                        "`allow_interactive` tag is only allowed on `call` handlers",
                    ));
                }
                allow_interactive = true;
            } else {
                return Err(syn::Error::new(
                    tag.span(),
                    "invalid handler tag; supported: `expensive`, `allow_private_km`, `allow_interactive`",
                ));
            }
        }

        if !input.is_empty() {
            return Err(syn::Error::new(input.span(), "unexpected extra tokens"));
        }
        Ok(Self {
            kind,
            rpc_name,
            is_expensive,
            allow_private_km,
            allow_interactive,
        })
    }
}

fn parse_attrs(attrs: &[syn::Attribute]) -> Option<MethodHandlerAttr> {
    let handler_meta = attrs.iter().find(|attr| attr.path.is_ident("handler"))?;
    handler_meta
        .parse_args()
        .map_err(|err| {
            emit_compile_error(format!(
                "Unsupported format of #[handler(...)] attribute: {}",
                err
            ))
        })
        .ok()
}

#[cfg(test)]
mod tests {
    // Helper; asserts that `derive_method_handler` generates the `expected` code from `input`.
    fn expect_method_handler_impl(input: syn::ItemImpl, expected: syn::Stmt) {
        let derivation = super::derive_method_handler(input);
        let actual: syn::Stmt = syn::parse2(derivation).unwrap();

        crate::assert_empty_diff!(actual, expected);
    }

    // The `uses` statement common to all autogenerated impls.
    thread_local! {
      static USES: proc_macro2::TokenStream = quote::quote! {
            use ::oasis_runtime_sdk::{
                  self as sdk, cbor,
                  error::Error as _,
                  module::{CallResult, DispatchResult},
                  modules::core::types as core_types,
                  types::message::MessageResult,
              };
      }
    }

    /// Unannotated functions in the input impl block should be assumed to be
    /// a part of the `MethodHandler` trait that is not implementable (or intentionally
    /// not implemented) via `derive(MethodHandler)`.
    #[test]
    fn generate_method_handler_impl_unannotated_func() {
        let input = syn::parse_quote!(
            impl<C: Cfg> MyModule<C> {
                fn unannotated_fn_should_be_passed_thru(foo: Bar) -> Baz {}
            }
        );

        expect_method_handler_impl(
            input,
            USES.with(|uses| {
                syn::parse_quote!(
                    const _: () = {
                        #uses
                        impl<C: Cfg> sdk::module::MethodHandler for MyModule<C> {
                            fn unannotated_fn_should_be_passed_thru(foo: Bar) -> Baz {}
                            fn dispatch_query<C: Context>(
                                ctx: &mut C,
                                method: &str,
                                args: cbor::Value,
                            ) -> DispatchResult<cbor::Value, Result<cbor::Value, sdk::error::RuntimeError>>
                            {
                                match method {
                                    q if q == format!("{}.Parameters", Self::NAME) => {
                                        module::dispatch_query(ctx, args, Self::query_parameters)
                                    }
                                    _ => DispatchResult::Unhandled(args),
                                }
                            }
                        }
                        impl<C: Cfg> MyModule<C> {
                            fn query_parameters<C: Context>(ctx: &mut C, _args: ()) -> Result<<Self as module::Module>::Parameters, <Self as module::Module>::Error> {
                                Ok(Self::params(ctx.runtime_state()))
                            }
                        }
                    };
                )
            }),
        );
    }

    #[test]
    fn generate_method_handler_impl_calls() {
        let input = syn::parse_quote!(
            impl<C: Cfg> MyModule<C> {
                #[handler(prefetch = "my_module.MyCall")]
                fn prefetch_for_my_call() {}
                #[handler(call = "my_module.MyCall")]
                fn my_call(foo2: Bar2) -> Baz2 {}
                #[handler(call = "my_module.MyOtherCall")]
                fn my_other_call(foo3: Bar3) -> Baz3 {}
            }
        );

        expect_method_handler_impl(
            input,
            USES.with(|uses| {
                syn::parse_quote!(
                    const _: () = {
                        #uses
                        impl<C: Cfg> sdk::module::MethodHandler for MyModule<C> {
                            fn prefetch(
                                prefixes: &mut BTreeSet<Prefix>,
                                method: &str,
                                body: cbor::Value,
                                auth_info: &AuthInfo,
                            ) -> module::DispatchResult<cbor::Value, Result<(), sdk::error::RuntimeError>> {
                                let mut add_prefix = |p| {
                                    prefixes.insert(p);
                                };
                                match method {
                                    "my_module.MyCall" => module::DispatchResult::Handled(
                                        Self::prefetch_for_my_call(&mut add_prefix, body, auth_info),
                                    ),
                                    "my_module.MyOtherCall" => module::DispatchResult::Handled(Ok(())),
                                    _ => module::DispatchResult::Unhandled(body),
                                }
                            }
                            fn dispatch_call<C: TxContext>(
                                ctx: &mut C,
                                method: &str,
                                body: cbor::Value,
                            ) -> DispatchResult<cbor::Value, CallResult> {
                                match method {
                                    "my_module.MyCall" => module::dispatch_call(ctx, body, Self::my_call),
                                    "my_module.MyOtherCall" => {
                                        module::dispatch_call(ctx, body, Self::my_other_call)
                                    }
                                    _ => DispatchResult::Unhandled(body),
                                }
                            }
                            fn dispatch_query<C: Context>(
                                ctx: &mut C,
                                method: &str,
                                args: cbor::Value,
                            ) -> DispatchResult<cbor::Value, Result<cbor::Value, sdk::error::RuntimeError>>
                            {
                                match method {
                                    q if q == format!("{}.Parameters", Self::NAME) => {
                                        module::dispatch_query(ctx, args, Self::query_parameters)
                                    }
                                    _ => DispatchResult::Unhandled(args),
                                }
                            }
                            fn supported_methods() -> Vec<core_types::MethodHandlerInfo> {
                                vec![
                                    core_types::MethodHandlerInfo {
                                        kind: core_types::MethodHandlerKind::Call,
                                        name: "my_module.MyCall".to_string(),
                                    },
                                    core_types::MethodHandlerInfo {
                                        kind: core_types::MethodHandlerKind::Call,
                                        name: "my_module.MyOtherCall".to_string(),
                                    },
                                ]
                            }
                        }
                        impl<C: Cfg> MyModule<C> {
                            fn query_parameters<C: Context>(ctx: &mut C, _args: ()) -> Result<<Self as module::Module>::Parameters, <Self as module::Module>::Error> {
                                Ok(Self::params(ctx.runtime_state()))
                            }
                            #[handler(prefetch = "my_module.MyCall")]
                            fn prefetch_for_my_call() {}
                            #[handler(call = "my_module.MyCall")]
                            fn my_call(foo2: Bar2) -> Baz2 {}
                            #[handler(call = "my_module.MyOtherCall")]
                            fn my_other_call(foo3: Bar3) -> Baz3 {}
                        }
                    };
                )
            }),
        );
    }

    #[test]
    fn generate_method_handler_impl_queries() {
        let input = syn::parse_quote!(
            impl<C: Cfg> MyModule<C> {
                #[handler(query = RPC_NAME_OF_MY_QUERY)]
                fn my_query() -> () {}
                #[handler(query = "module.OtherQuery", expensive)]
                fn my_other_query() -> () {}
                #[handler(query = "module.ConfidentialQuery", expensive, allow_private_km)]
                fn my_confidential_query() -> () {}
            }
        );

        expect_method_handler_impl(
            input,
            USES.with(|uses| {
                syn::parse_quote!(
                    const _: () = {
                        #uses
                        impl<C: Cfg> sdk::module::MethodHandler for MyModule<C> {
                            fn dispatch_query<C: Context>(
                                ctx: &mut C,
                                method: &str,
                                args: cbor::Value,
                            ) -> DispatchResult<cbor::Value, Result<cbor::Value, sdk::error::RuntimeError>>
                            {
                                match method {
                                    RPC_NAME_OF_MY_QUERY => module::dispatch_query(ctx, args, Self::my_query),
                                    "module.OtherQuery" => module::dispatch_query(ctx, args, Self::my_other_query),
                                    "module.ConfidentialQuery" => module::dispatch_query(ctx, args, Self::my_confidential_query),
                                    q if q == format!("{}.Parameters", Self::NAME) => {
                                        module::dispatch_query(ctx, args, Self::query_parameters)
                                    }
                                    _ => DispatchResult::Unhandled(args),
                                }
                            }
                            fn supported_methods() -> Vec<core_types::MethodHandlerInfo> {
                                vec![
                                    core_types::MethodHandlerInfo {
                                        kind: core_types::MethodHandlerKind::Query,
                                        name: RPC_NAME_OF_MY_QUERY.to_string(),
                                    },
                                    core_types::MethodHandlerInfo {
                                        kind: core_types::MethodHandlerKind::Query,
                                        name: "module.OtherQuery".to_string(),
                                    },
                                    core_types::MethodHandlerInfo {
                                        kind: core_types::MethodHandlerKind::Query,
                                        name: "module.ConfidentialQuery".to_string(),
                                    },
                                ]
                            }
                            fn is_expensive_query(method: &str) -> bool {
                                ["module.OtherQuery", "module.ConfidentialQuery"].contains(&method)
                            }
                            fn is_allowed_private_km_query(method: &str) -> bool {
                                ["module.ConfidentialQuery"].contains(&method)
                            }
                        }
                        impl<C: Cfg> MyModule<C> {
                            fn query_parameters<C: Context>(
                                ctx: &mut C,
                                _args: (),
                            ) -> Result<<Self as module::Module>::Parameters, <Self as module::Module>::Error>
                            {
                                Ok(Self::params(ctx.runtime_state()))
                            }
                            #[handler(query = RPC_NAME_OF_MY_QUERY)]
                            fn my_query() -> () {}
                            #[handler(query = "module.OtherQuery", expensive)]
                            fn my_other_query() -> () {}
                            #[handler(query = "module.ConfidentialQuery", expensive, allow_private_km)]
                            fn my_confidential_query() -> () {}
                        }
                    };
                )
            }),
        );
    }

    #[test]
    fn generate_method_handler_impl_method_calls() {
        let input = syn::parse_quote!(
            impl<C: Cfg> MyModule<C> {
                #[handler(query = "my_module.MyMC")]
                fn my_method_call() -> () {}
            }
        );

        expect_method_handler_impl(
            input,
            USES.with(|uses| {
                syn::parse_quote!(
                    const _: () = {
                        #uses
                        impl<C: Cfg> sdk::module::MethodHandler for MyModule<C> {
                            fn dispatch_query<C: Context>(
                                ctx: &mut C,
                                method: &str,
                                args: cbor::Value,
                            ) -> DispatchResult<cbor::Value, Result<cbor::Value, sdk::error::RuntimeError>>
                            {
                                match method {
                                    "my_module.MyMC" => module::dispatch_query(ctx, args, Self::my_method_call),
                                    q if q == format!("{}.Parameters", Self::NAME) => {
                                        module::dispatch_query(ctx, args, Self::query_parameters)
                                    }
                                    _ => DispatchResult::Unhandled(args),
                                }
                            }
                            fn supported_methods() -> Vec<core_types::MethodHandlerInfo> {
                                vec![core_types::MethodHandlerInfo {
                                    kind: core_types::MethodHandlerKind::Query,
                                    name: "my_module.MyMC".to_string(),
                                }]
                            }
                        }
                        impl<C: Cfg> MyModule<C> {
                            fn query_parameters<C: Context>(ctx: &mut C, _args: ()) -> Result<<Self as module::Module>::Parameters, <Self as module::Module>::Error> {
                                Ok(Self::params(ctx.runtime_state()))
                            }
                            #[handler(query = "my_module.MyMC")]
                            fn my_method_call() -> () {}
                        }
                    };
                )
            }),
        );
    }

    #[test]
    #[should_panic(expected = "invalid handler kind")]
    fn generate_method_handler_malformed_bad_kind() {
        let input: syn::ItemImpl = syn::parse_quote!(
            impl<C: Cfg> MyModule<C> {
                #[handler(unsupported_key = "some_value")]
                fn my_method_call() -> () {}
            }
        );
        super::derive_method_handler(input);
    }

    #[test]
    #[should_panic(expected = "only allowed on `query` handlers")]
    fn generate_method_handler_malformed_expensive_nonquery() {
        let input: syn::ItemImpl = syn::parse_quote!(
            impl<C: Cfg> MyModule<C> {
                #[handler(call = "foo", expensive)]
                fn my_method_call() -> () {}
            }
        );
        super::derive_method_handler(input);
    }

    #[test]
    #[should_panic(expected = "only allowed on `query` handlers")]
    fn generate_method_handler_malformed_allow_private_km_nonquery() {
        let input: syn::ItemImpl = syn::parse_quote!(
            impl<C: Cfg> MyModule<C> {
                #[handler(call = "foo", allow_private_km)]
                fn my_method_call() -> () {}
            }
        );
        super::derive_method_handler(input);
    }

    #[test]
    #[should_panic]
    fn generate_method_handler_malformed_multiple_metas() {
        let input: syn::ItemImpl = syn::parse_quote!(
            impl<C: Cfg> MyModule<C> {
                #[handler(call = "foo", query = "bar")]
                fn my_method_call() -> () {}
            }
        );
        super::derive_method_handler(input);
    }
}
