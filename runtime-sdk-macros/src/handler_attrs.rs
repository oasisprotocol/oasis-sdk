use inflector::Inflector as _;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::generators as gen;

pub fn gen_call_items(methods: &syn::ItemTrait, args: &syn::AttributeArgs) -> TokenStream {
    gen_handler_items(methods, args, Handlers::Calls)
}

pub fn gen_query_items(methods: &syn::ItemTrait, args: &syn::AttributeArgs) -> TokenStream {
    gen_handler_items(methods, args, Handlers::Queries)
}

fn gen_handler_items(
    handlers: &syn::ItemTrait,
    args: &syn::AttributeArgs,
    handlers_kind: Handlers,
) -> TokenStream {
    let maybe_module_name = find_meta_key(args, "module_name")
        .ok_or_else(|| {
            format!(
                "missing `module_name` arg to #[oasis_runtime_sdk::{}]",
                handlers_kind,
            )
        })
        .and_then(|meta| match &meta.lit {
            syn::Lit::Str(module_name) => module_name
                .parse::<syn::Path>()
                .map_err(|_| "expected `module_name` to be a valid path".into()),
            _ => Err("expected `module_name` to be a valid path".into()),
        });
    let module_name_path = match maybe_module_name {
        Ok(module_name_path) => module_name_path,
        Err(err_msg) => {
            handlers.ident.span().unwrap().error(&err_msg);
            return quote!();
        }
    };

    let handler_methods: Vec<_> = handlers
        .items
        .iter()
        .filter_map(|itm| match itm {
            syn::TraitItem::Method(m) => Some(m),
            _ => None,
        })
        .collect();

    let handler_names: Vec<HandlerName<'_>> = handler_methods
        .iter()
        .map(|m| {
            let name = find_attr(&m.attrs, &handlers_kind.to_string().to_singular())
                .and_then(|handler_meta| match handler_meta {
                    syn::Meta::List(meta) => {
                        find_meta_key(&meta.nested, "name").and_then(|meta| match &meta.lit {
                            syn::Lit::Str(name) => Some(name.value()),
                            _ => None,
                        })
                    }
                    _ => None,
                })
                .unwrap_or_else(|| m.sig.ident.to_string().to_pascal_case());

            HandlerName {
                ident: &m.sig.ident,
                name,
            }
        })
        .collect();

    let module_items = gen_module_items(
        &handlers,
        &module_name_path,
        &handler_methods,
        &handler_names,
        handlers_kind,
    );
    let client_items = gen_client_items(
        &handlers,
        &module_name_path,
        &handler_methods,
        &handler_names,
        handlers_kind,
    );

    let output = quote! {
        #(#[cfg(feature = "runtime-module")] #module_items)*
        #(#[cfg(feature = "runtime-client")] #client_items)*
    };

    output
}

fn gen_module_items(
    handlers: &syn::ItemTrait,
    module_name_path: &syn::Path,
    handler_methods: &[&syn::TraitItemMethod],
    handler_names: &[HandlerName<'_>],
    handlers_kind: Handlers,
) -> Vec<TokenStream> {
    let sdk_crate = gen::sdk_crate_path();

    let trait_ident = &handlers.ident;
    let trait_generics = &handlers.generics;
    let supertraits = &handlers.supertraits;

    let handler_ctx_ty = handlers_kind.context_ty();

    let module_handlers = handler_methods.iter().map(|handler| {
        let handler_ident = &handler.sig.ident;
        let attrs = &handler.attrs;
        let inputs = &handler.sig.inputs;
        let generics = &handler.sig.generics.params;
        let output_ty = &handler.sig.output;
        // TODO wrap non-result output in result
        quote! {
            #(#attrs)*
            fn #handler_ident<#generics>(
                ctx: &mut impl #sdk_crate::context::#handler_ctx_ty,
                #inputs
            ) #output_ty;
        }
    });

    let handler_fn_ident = format_ident!("handle_{}", handlers_kind.to_string().to_singular());
    let handler_args_ident = format_ident!("args");
    let handler_err_ty = match handlers_kind {
        Handlers::Calls => quote!(#sdk_crate::types::transaction::CallResult),
        Handlers::Queries => {
            quote!(Result<#sdk_crate::core::common::cbor::Value, #sdk_crate::error::RuntimeError>)
        }
    };
    let dispatch_arms = handler_methods
        .iter()
        .zip(handler_names)
        .map(|(m, handler_name)| {
            let result_ident = format_ident!("result");

            let handler_ident = &handler_name.ident;
            let handler_name = &handler_name.name;

            let handler_cfg_attrs = m
                .attrs
                .iter()
                .filter(|attr| attr.path.is_ident("cfg") || attr.path.is_ident("cfg_attr"));

            let handler_arg_names: Vec<_> = m
                .sig
                .inputs
                .iter()
                .enumerate()
                .map(|(i, inp)| match inp {
                    syn::FnArg::Typed(syn::PatType {
                        pat: box syn::Pat::Ident(syn::PatIdent { ident, .. }),
                        ..
                    }) => {
                        format_ident!("arg_{}", ident)
                    }
                    _ => {
                        format_ident!("arg_{}", i)
                    }
                })
                .collect();

            let result_encoder = match handlers_kind {
                Handlers::Calls => quote! {
                    match #result_ident {
                        Ok(value) => #sdk_crate::types::transaction::CallResult::Ok(value),
                        Err(e) => #sdk_crate::error::Error::to_call_result(&e),
                    }
                },
                Handlers::Queries => quote!(#result_ident.map_err(Into::into)),
            };

            quote! {
                #(#handler_cfg_attrs)*
                Some(#handler_name) => {
                    use #sdk_crate::core::common::cbor;
                    let #result_ident = cbor::from_value(#handler_args_ident)
                        .map_err(Into::into)
                        .and_then(|(#(#handler_arg_names),*)| {
                            Self::#handler_ident(ctx, #(#handler_arg_names),*)
                        })
                        .map(|result| cbor::to_value(&result));
                    #sdk_crate::module::DispatchResult::Handled(#result_encoder)
                }
            }
        });

    let module_trait = quote! {
        pub trait #trait_ident #trait_generics : #supertraits {
            #(#module_handlers)*

            #[allow(warnings)]
            fn #handler_fn_ident<C: #sdk_crate::context::#handler_ctx_ty>(
                ctx: &mut C,
                method: &str,
                #handler_args_ident: #sdk_crate::core::common::cbor::Value,
            ) -> #sdk_crate::module::DispatchResult<
                #sdk_crate::core::common::cbor::Value,
                #handler_err_ty,
            > {
                let mut method_parts = method.splitn(1, '.');
                if method_parts.next().map(|p| p == #module_name_path).unwrap_or_default() {
                    return #sdk_crate::module::DispatchResult::Unhandled(#handler_args_ident);
                }
                match method_parts.next() {
                    #(#dispatch_arms)*
                    _ => #sdk_crate::module::DispatchResult::Unhandled(#handler_args_ident),
                }
            }
        }
    };

    vec![module_trait]
}

fn gen_client_items(
    _handlers: &syn::ItemTrait,
    _module_name_path: &syn::Path,
    _handler_methods: &[&syn::TraitItemMethod],
    _handler_names: &[HandlerName<'_>],
    _handlers_kind: Handlers,
) -> Vec<TokenStream> {
    vec![]
}

/// Returns the parsed attribute with path ending with `name` or `None` if not found.
fn find_attr(attrs: &[syn::Attribute], name: &str) -> Option<syn::Meta> {
    attrs.iter().find_map(|attr| {
        if attr.path.segments.last()?.ident == name {
            attr.parse_meta().ok()
        } else {
            None
        }
    })
}

/// Returns the `MetaNameValue` identified by `key` or `None` if not found.
fn find_meta_key<'a>(
    metas: impl IntoIterator<Item = &'a syn::NestedMeta>,
    key: &str,
) -> Option<&'a syn::MetaNameValue> {
    metas.into_iter().find_map(|meta| match meta {
        syn::NestedMeta::Meta(syn::Meta::NameValue(meta)) if meta.path.is_ident(key) => Some(meta),
        _ => None,
    })
}

#[derive(Clone, Copy)]
enum Handlers {
    Calls,
    Queries,
}

impl Handlers {
    fn context_ty(&self) -> TokenStream {
        match self {
            Self::Calls => quote!(TxContext),
            Self::Queries => quote!(Context),
        }
    }
}

impl std::fmt::Display for Handlers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Calls => "calls",
            Self::Queries => "queries",
        })
    }
}

struct HandlerName<'a> {
    ident: &'a syn::Ident,
    name: String,
}
