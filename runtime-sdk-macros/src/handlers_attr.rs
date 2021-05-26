use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::generators as gen;

pub fn gen_method_registration_handler_impl(
    handlers: syn::ItemImpl,
    args: syn::AttributeArgs,
) -> TokenStream {
    let maybe_rpc_namespace = find_meta_key(&args, "rpc_namespace").map(|meta| match &meta.lit {
        syn::Lit::Str(rpc_namespace) => Ok(rpc_namespace.value()),
        _ => Err(()),
    });
    let rpc_namespace = match maybe_rpc_namespace {
        Some(Ok(rpc_namespace)) => rpc_namespace,
        Some(Err(())) => {
            return quote!();
        }
        None => {
            handlers
                .impl_token
                .span
                .unwrap()
                .error("missing `rpc_namespace` arg to #[handlers]");
            return quote!();
        }
    };

    let sdk_crate = gen::sdk_crate_path();

    let module_generics = &handlers.generics;
    let module_ty = &handlers.self_ty;

    let method_registrations = handlers.items.iter().filter_map(|itm| {
        let method = match itm {
            syn::ImplItem::Method(m) => m,
            _ => return None,
        };

        let mut inputs = method.sig.inputs.iter();
        let e_needs_ctx =
            "handler method must have `&mut TxContext` or `&mut DispatchContext` as the first argument";
        let result_ident = format_ident!("result");
        let (registrar, method_info, result_encoder) = match inputs.next() {
            Some(syn::FnArg::Typed(syn::PatType {
                ty:
                    box syn::Type::Reference(syn::TypeReference {
                        elem: box syn::Type::Path(syn::TypePath { path, .. }),
                        ..
                    }),
                ..
            })) => {
                let ty_ident = &path.segments.last().as_ref().unwrap().ident; // path must have one segment
                if ty_ident == "TxContext" {
                    let result_encoder = quote! {
                        match #result_ident {
                            Ok(value) => sdk::types::transaction::CallResult::Ok(value),
                            Err(err) => err.to_call_result(),
                        }
                    };
                    (quote!(register_callable), quote!(CallableMethodInfo), result_encoder)
                } else if ty_ident == "DispatchContext" {
                    let result_encoder = quote!(Ok(cbor::to_value(&#result_ident?)));
                    (quote!(register_query), quote!(QueryMethodInfo), result_encoder)
                } else {
                    method.sig.ident.span().unwrap().error(e_needs_ctx).emit();
                    return Some(quote!());
                }
            }
            _ => {
                method.sig.ident.span().unwrap().error(e_needs_ctx).emit();
                return Some(quote!());
            }
        };

        let handler_ident = &method.sig.ident;

        let rpc_method = find_attr(&method.attrs, "handler")
            .and_then(|handler_meta| match handler_meta {
                syn::Meta::List(meta) => {
                    find_meta_key(&meta.nested, "name").and_then(|meta| match &meta.lit {
                        syn::Lit::Str(name) => Some(name.value()),
                        _ => None,
                    })
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                inflector::cases::pascalcase::to_pascal_case(&handler_ident.to_string())
            });
        let method_name = format!("{}.{}", rpc_namespace, rpc_method);

        let arg_names: Vec<_> = inputs
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

        let method_cfg_attrs = method
            .attrs
            .iter()
            .filter(|attr| attr.path.is_ident("cfg") || attr.path.is_ident("cfg_attr"));

        Some(quote! {
            #(#method_cfg_attrs)*
            {
            methods.#registrar(sdk::module::#method_info {
                name: #method_name,
                handler: |_mi, ctx, body| {
                    let #result_ident = || -> Result<cbor::Value, Error> {
                        let (#(#arg_names),*) = cbor::from_value(body)?;
                        Ok(cbor::to_value(&Self::#handler_ident(ctx, #(#arg_names),*)?))
                    }();
                    #result_encoder
                }
            });
            }
        })
    });

    let method_registration = gen::wrap_in_const(quote! {
        use #sdk_crate::{self as sdk, core::common::cbor, error::Error as _};

        #[allow(warnings)]
        impl#module_generics sdk::module::MethodRegistrationHandler for #module_ty {
            fn register_methods(methods: &mut sdk::module::MethodRegistry) {
                #(#method_registrations)*
            }
        }
    });

    quote! {
        #handlers

        #method_registration
    }
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
