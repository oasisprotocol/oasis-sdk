use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;

use crate::emit_compile_error;

/// Deriver for the `MethodHandler` trait.
pub struct DeriveMethodHandler {
    handlers: Vec<ParsedImplItem>,
}

impl DeriveMethodHandler {
    pub fn new() -> Box<Self> {
        Box::new(Self { handlers: vec![] })
    }
}

impl super::Deriver for DeriveMethodHandler {
    fn preprocess(&mut self, item: syn::ImplItem) -> Option<syn::ImplItem> {
        let method = match item {
            syn::ImplItem::Fn(ref f) => f,
            _ => return Some(item),
        };

        let attrs = if let Some(attrs) = parse_attrs(&method.attrs) {
            attrs
        } else {
            return Some(item);
        };

        self.handlers.push(ParsedImplItem {
            handler: Some(HandlerInfo {
                attrs,
                ident: method.sig.ident.clone(),
            }),
            item,
        });

        None // Take the item.
    }

    fn derive(&mut self, generics: &syn::Generics, ty: &Box<syn::Type>) -> TokenStream {
        let handlers = &self.handlers;
        let handler_items = handlers
            .iter()
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
            let (handler_names, handler_idents) = filter_by_kind(handlers, HandlerKind::Prefetch);

            // Find call handlers; for every call handler without a corresponding prefetch handler, we'll
            // generate a dummy prefetch handler.
            let (call_handler_names, _) = filter_by_kind(handlers, HandlerKind::Call);
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
            let (handler_names, handler_fns): (Vec<_>, Vec<_>) = handlers
                .iter()
                .filter_map(|h| h.handler.as_ref())
                .filter(|h| h.attrs.kind == HandlerKind::Call)
                .map(|h| {
                    (h.attrs.rpc_name.clone(), {
                        let ident = &h.ident;

                        if h.attrs.is_internal {
                            quote! {
                                |ctx, body| {
                                    if !sdk::state::CurrentState::with_env(|env| env.is_internal()) {
                                        return Err(sdk::modules::core::Error::Forbidden.into());
                                    }
                                    Self::#ident(ctx, body)
                                }
                            }
                        } else {
                            quote! { Self::#ident }
                        }
                    })
                })
                .unzip();

            if handler_names.is_empty() {
                quote! {}
            } else {
                quote! {
                    fn dispatch_call<C: Context>(
                        ctx: &C,
                        method: &str,
                        body: cbor::Value,
                    ) -> DispatchResult<cbor::Value, CallResult> {
                        match method {
                            #(
                              #handler_names => module::dispatch_call(ctx, body, #handler_fns),
                            )*
                            _ => DispatchResult::Unhandled(body),
                        }
                    }
                }
            }
        };

        let query_parameters_impl = {
            quote! {
                fn query_parameters<C: Context>(_ctx: &C, _args: ()) -> Result<<Self as module::Module>::Parameters, <Self as module::Module>::Error> {
                    Ok(Self::params())
                }
            }
        };

        let dispatch_query_impl = {
            let (handler_names, handler_idents) = filter_by_kind(handlers, HandlerKind::Query);

            if handler_names.is_empty() {
                quote! {
                    fn dispatch_query<C: Context>(
                        ctx: &C,
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
                        ctx: &C,
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
            let (handler_names, handler_idents) =
                filter_by_kind(handlers, HandlerKind::MessageResult);

            if handler_names.is_empty() {
                quote! {}
            } else {
                quote! {
                    fn dispatch_message_result<C: Context>(
                        ctx: &C,
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

        quote! {
            #[automatically_derived]
            impl #generics sdk::module::MethodHandler for #ty {
                #prefetch_impl
                #dispatch_call_impl
                #dispatch_query_impl
                #dispatch_message_result_impl
                #supported_methods_impl
                #expensive_queries_impl
                #allowed_private_km_queries_impl
                #allowed_interactive_calls_impl
            }

            #[automatically_derived]
            impl #generics #ty {
                #query_parameters_impl

                #(#handler_items)*
            }
        }
    }
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
    /// Whether this handler is tagged as internal.
    is_internal: bool,
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
        let mut is_internal = false;
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
            } else if tag == "internal" {
                if kind != HandlerKind::Call {
                    return Err(syn::Error::new(
                        tag.span(),
                        "`internal` tag is only allowed on `call` handlers",
                    ));
                }
                is_internal = true;
            } else {
                return Err(syn::Error::new(
                    tag.span(),
                    "invalid handler tag; supported: `expensive`, `allow_private_km`, `allow_interactive`, `internal`",
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
            is_internal,
        })
    }
}

fn parse_attrs(attrs: &[syn::Attribute]) -> Option<MethodHandlerAttr> {
    let handler_meta = attrs.iter().find(|attr| attr.path().is_ident("handler"))?;
    handler_meta
        .parse_args()
        .map_err(|err| {
            emit_compile_error(format!(
                "Unsupported format of #[handler(...)] attribute: {err}"
            ))
        })
        .ok()
}
