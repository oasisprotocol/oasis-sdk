use darling::{util::Flag, FromDeriveInput, FromField, FromVariant};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};

use crate::generators::{self as gen, CodedVariant};

#[derive(FromDeriveInput)]
#[darling(supports(enum_any), attributes(sdk_error))]
struct Error {
    ident: syn::Ident,

    data: darling::ast::Data<ErrorVariant, darling::util::Ignored>,

    /// The path to a const set to the module name.
    #[darling(default)]
    module_name: Option<syn::LitStr>,

    /// Whether to sequentially autonumber the error codes.
    /// This option exists as a convenience for runtimes that
    /// only append errors or release only breaking changes.
    #[darling(default, rename = "autonumber")]
    autonumber: Flag,
}

#[derive(FromVariant)]
#[darling(attributes(sdk_error))]
struct ErrorVariant {
    ident: syn::Ident,

    fields: darling::ast::Fields<ErrorField>,

    /// The explicit ID of the error code. Overrides any autonumber set on the error enum.
    #[darling(default, rename = "code")]
    code: Option<u32>,

    #[darling(default, rename = "transparent")]
    transparent: Flag,
}

impl CodedVariant for ErrorVariant {
    const FIELD_NAME: &'static str = "code";

    fn ident(&self) -> &syn::Ident {
        &self.ident
    }

    fn code(&self) -> Option<u32> {
        self.code
    }
}

#[derive(FromField)]
#[darling(forward_attrs(source, from))]
struct ErrorField {
    ident: Option<syn::Ident>,

    attrs: Vec<syn::Attribute>,
}

pub fn derive_error(input: syn::DeriveInput) -> TokenStream {
    let error = match Error::from_derive_input(&input) {
        Ok(error) => error,
        Err(e) => return e.write_errors(),
    };

    let error_ty_ident = &error.ident;

    let module_name = match gen::module_name(error.module_name.as_ref()) {
        Ok(expr) => expr,
        Err(_) => return quote!(),
    };

    let ErrorImpl {
        module_name_body,
        code_body,
        kind_for_code_body,
        kinds_enum_ty,
        kinds_enum,
    } = match convert_variants(
        error_ty_ident,
        &module_name,
        &error.data.as_ref().take_enum().unwrap(),
        error.autonumber.is_some(),
    ) {
        Ok(imp) => imp,
        Err(_) => return quote!(),
    };

    let sdk_crate = gen::sdk_crate_path();

    let error_impls = gen::wrap_in_const(quote! {
        use #sdk_crate::{
            self as sdk, core::types::Error as RuntimeError, error::Error as _,
        };

        impl sdk::error::Error for #error_ty_ident {
            type Kinds = #kinds_enum_ty;

            fn module_name(&self) -> &str {
                #module_name_body
            }

            fn code(&self) -> u32 {
                #code_body
            }

            fn kind_for_code(code: u32) -> Option<Self::Kinds> {
                #kind_for_code_body
            }
        }

        impl From<#error_ty_ident> for RuntimeError {
            fn from(err: #error_ty_ident) -> RuntimeError {
                RuntimeError::new(err.module_name(), err.code(), &err.to_string())
            }
        }
    });

    quote! {
        #error_impls

        #kinds_enum
    }
}

fn convert_variants(
    error_ty_ident: &syn::Ident,
    module_name: &syn::Expr,
    variants: &[&ErrorVariant],
    autonumber: bool,
) -> Result<ErrorImpl, ()> {
    if variants.is_empty() {
        return Ok(ErrorImpl {
            module_name_body: quote!(#module_name),
            code_body: quote!(0),
            kind_for_code_body: quote!(None),
            kinds_enum: None,
            kinds_enum_ty: quote!(()),
        });
    }

    let mut next_autonumber = 0u32;
    let mut reserved_numbers = std::collections::BTreeSet::new();

    let mut module_name_matches = Vec::with_capacity(variants.len());
    let mut code_matches = Vec::with_capacity(variants.len());
    let mut coded_variant_idents = Vec::with_capacity(variants.len());
    let mut kind_for_code_matches = Vec::with_capacity(variants.len());

    for variant in variants.iter() {
        let variant_ident = &variant.ident;

        if variant.transparent.is_some() {
            // Transparently forward everything to the source.
            let mut maybe_sources = variant
                .fields
                .iter()
                .enumerate()
                .filter_map(|(i, f)| (!f.attrs.is_empty()).then(|| (i, f.ident.clone())));
            let source = maybe_sources.next();
            if maybe_sources.count() != 0 {
                variant_ident
                    .span()
                    .unwrap()
                    .error("multiple error sources specified for variant")
                    .emit();
                return Err(());
            }
            if source.is_none() {
                variant_ident
                    .span()
                    .unwrap()
                    .error("no source error specified for variant")
                    .emit();
                return Err(());
            }
            let (field_index, field_ident) = source.unwrap();

            let field = match field_ident {
                Some(ident) => syn::Member::Named(ident),
                None => syn::Member::Unnamed(syn::Index {
                    index: field_index as u32,
                    span: variant_ident.span(),
                }),
            };

            let source = quote!(source);
            let module_name = quote_spanned!(variant_ident.span()=> #source.module_name());
            let code = quote_spanned!(variant_ident.span()=> #source.code());

            module_name_matches.push(quote! {
                Self::#variant_ident { #field: #source, .. } => #module_name,
            });
            code_matches.push(quote! {
                Self::#variant_ident { #field: #source, .. } => #code,
            });
        } else {
            // Regular case without forwarding.
            let code = match variant.code {
                Some(code) => {
                    if reserved_numbers.contains(&code) {
                        variant_ident
                            .span()
                            .unwrap()
                            .error(format!("code {} already used", code))
                            .emit();
                        return Err(());
                    }
                    reserved_numbers.insert(code);
                    code
                }
                None if autonumber => {
                    let mut reserved_successors = reserved_numbers.range(next_autonumber..);
                    while reserved_successors.next() == Some(&next_autonumber) {
                        next_autonumber += 1;
                    }
                    let code = next_autonumber;
                    reserved_numbers.insert(code);
                    next_autonumber += 1;
                    code
                }
                None => {
                    variant_ident
                        .span()
                        .unwrap()
                        .error("missing `#[sdk_error(code = _)]` for variant")
                        .emit();
                    return Err(());
                }
            };

            module_name_matches.push(quote!(Self::#variant_ident { .. } => #module_name,));
            code_matches.push(quote!(Self::#variant_ident { .. } => #code,));
            coded_variant_idents.push(variant_ident);
            kind_for_code_matches.push(quote!(#code => Some(Self::Kinds::#variant_ident),));
        }
    }

    let has_coded_variants = !coded_variant_idents.is_empty();
    let kinds_enum_ident = format_ident!("{}Kind", error_ty_ident);

    Ok(ErrorImpl {
        module_name_body: quote! {
            match self {
                #(#module_name_matches)*
            }
        },
        code_body: quote! {
            match self {
                #(#code_matches)*
            }
        },
        kind_for_code_body: has_coded_variants
            .then(|| {
                quote! {
                    match code {
                        #(#kind_for_code_matches)*
                        _ => None
                    }
                }
            })
            .unwrap_or_else(|| quote!(None)),
        kinds_enum_ty: has_coded_variants
            .then(|| quote!(#kinds_enum_ident))
            .unwrap_or_else(|| quote!(())),
        kinds_enum: has_coded_variants.then(|| {
            quote! {
                #[derive(Clone, Copy, Debug, PartialEq, Eq)]
                pub enum #kinds_enum_ident {
                    #(#coded_variant_idents),*
                }
            }
        }),
    })
}

struct ErrorImpl {
    module_name_body: TokenStream,
    code_body: TokenStream,
    kind_for_code_body: TokenStream,
    kinds_enum_ty: TokenStream,
    kinds_enum: Option<TokenStream>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn generate_error_impl_auto() {
        let expected: syn::File = syn::parse_quote!(
            const _: () = {
                use ::oasis_runtime_sdk::{
                    self as sdk, core::types::Error as RuntimeError, error::Error as _,
                };
                impl sdk::error::Error for Error {
                    type Kinds = ErrorKind;
                    fn module_name(&self) -> &str {
                        match self {
                            Self::Error0 { .. } => MODULE_NAME,
                            Self::Error2 { .. } => MODULE_NAME,
                            Self::Error1 { .. } => MODULE_NAME,
                            Self::Error3 { .. } => MODULE_NAME,
                        }
                    }
                    fn code(&self) -> u32 {
                        match self {
                            Self::Error0 { .. } => 0u32,
                            Self::Error2 { .. } => 2u32,
                            Self::Error1 { .. } => 1u32,
                            Self::Error3 { .. } => 3u32,
                        }
                    }
                    fn kind_for_code(code: u32) -> Option<Self::Kinds> {
                        match code {
                            0u32 => Some(Self::Kinds::Error0),
                            2u32 => Some(Self::Kinds::Error2),
                            1u32 => Some(Self::Kinds::Error1),
                            3u32 => Some(Self::Kinds::Error3),
                            _ => None,
                        }
                    }
                }
                impl From<Error> for RuntimeError {
                    fn from(err: Error) -> RuntimeError {
                        RuntimeError::new(err.module_name(), err.code(), &err.to_string())
                    }
                }
            };
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub enum ErrorKind {
                Error0,
                Error2,
                Error1,
                Error3,
            }
        );

        let input: syn::DeriveInput = syn::parse_quote!(
            #[derive(Error)]
            #[sdk_error(autonumber)]
            pub enum Error {
                Error0,
                #[sdk_error(code = 2)]
                Error2 {
                    payload: Vec<u8>,
                },
                Error1(String),
                Error3,
            }
        );
        let error_derivation = super::derive_error(input);
        let actual: syn::File = syn::parse2(error_derivation).unwrap();

        crate::assert_empty_diff!(actual, expected);
    }

    #[test]
    fn generate_error_impl_manual() {
        let expected: syn::Stmt = syn::parse_quote!(
            const _: () = {
                use ::oasis_runtime_sdk::{
                    self as sdk, core::types::Error as RuntimeError, error::Error as _,
                };
                impl sdk::error::Error for Error {
                    type Kinds = ();
                    fn module_name(&self) -> &str {
                        THE_MODULE_NAME
                    }
                    fn code(&self) -> u32 {
                        0
                    }
                    fn kind_for_code(code: u32) -> Option<Self::Kinds> {
                        None
                    }
                }
                impl From<Error> for RuntimeError {
                    fn from(err: Error) -> RuntimeError {
                        RuntimeError::new(err.module_name(), err.code(), &err.to_string())
                    }
                }
            };
        );

        let input: syn::DeriveInput = syn::parse_quote!(
            #[derive(Error)]
            #[sdk_error(autonumber, module_name = "THE_MODULE_NAME")]
            pub enum Error {}
        );
        let error_derivation = super::derive_error(input);
        let actual: syn::Stmt = syn::parse2(error_derivation).unwrap();

        crate::assert_empty_diff!(actual, expected);
    }

    #[test]
    fn generate_error_impl_from() {
        let expected: syn::Stmt = syn::parse_quote!(
            const _: () = {
                use ::oasis_runtime_sdk::{
                    self as sdk, core::types::Error as RuntimeError, error::Error as _,
                };
                impl sdk::error::Error for Error {
                    type Kinds = ();
                    fn module_name(&self) -> &str {
                        match self {
                            Self::Foo { 0: source, .. } => source.module_name(),
                        }
                    }
                    fn code(&self) -> u32 {
                        match self {
                            Self::Foo { 0: source, .. } => source.code(),
                        }
                    }
                    fn kind_for_code(code: u32) -> Option<Self::Kinds> {
                        None
                    }
                }
                impl From<Error> for RuntimeError {
                    fn from(err: Error) -> RuntimeError {
                        RuntimeError::new(err.module_name(), err.code(), &err.to_string())
                    }
                }
            };
        );

        let input: syn::DeriveInput = syn::parse_quote!(
            #[derive(Error)]
            #[sdk_error(module_name = "THE_MODULE_NAME")]
            pub enum Error {
                #[sdk_error(transparent)]
                Foo(#[from] AnotherError),
            }
        );
        let error_derivation = super::derive_error(input);
        let actual: syn::Stmt = syn::parse2(error_derivation).unwrap();

        crate::assert_empty_diff!(actual, expected);
    }
}
