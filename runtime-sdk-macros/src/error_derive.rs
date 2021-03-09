use darling::{util::Flag, FromDeriveInput, FromVariant};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident};

use crate::generators::{self as gen, CodedVariant};

#[derive(FromDeriveInput)]
#[darling(supports(enum_any), attributes(runtime_error))]
struct Error {
    ident: Ident,

    data: darling::ast::Data<ErrorVariant, darling::util::Ignored>,

    /// The type ident of the error enum.
    module: syn::Path,

    /// Whether to sequentially autonumber the error codes.
    /// This option exists as a convenience for runtimes that
    /// only append errors or release only breaking changes.
    #[darling(default)]
    autonumber: Flag,
}

#[derive(FromVariant)]
#[darling(attributes(runtime_error))]
struct ErrorVariant {
    ident: Ident,

    /// The explicit ID of the error code. Overrides any autonumber set on the error enum.
    #[darling(default)]
    code: Option<u32>,
}

impl CodedVariant for ErrorVariant {
    fn ident(&self) -> &Ident {
        &self.ident
    }

    fn code(&self) -> Option<u32> {
        self.code
    }
}

pub fn derive_error(input: DeriveInput) -> TokenStream {
    let error = match Error::from_derive_input(&input) {
        Ok(error) => error,
        Err(e) => return e.write_errors(),
    };

    let error_ty_ident = &error.ident;
    let module_path = &error.module;

    let code_converter = gen::enum_code_converter(
        &format_ident!("self"),
        &error.data.as_ref().take_enum().unwrap(),
        error.autonumber.is_some(),
    );

    gen::wrap_in_const(quote! {
        impl oasis_runtime_sdk::Error for #error_ty_ident {
            fn module(&self) -> &str {
                <#module_path as oasis_runtime_sdk::module::Module>::NAME
            }

            fn code(&self) -> u32 {
                #code_converter
            }
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn generate_error_impl() {
        let expected: syn::Stmt = syn::parse_quote!(
            const _: () = {
                impl oasis_runtime_sdk::Error for Error {
                    fn module(&self) -> &str {
                        <module::TheModule as oasis_runtime_sdk::module::Module>::NAME
                    }
                    fn code(&self) -> u32 {
                        match self {
                            Self::Error0 { .. } => 0u32,
                            Self::Error2 { .. } => 2u32,
                            Self::Error1 { .. } => 1u32,
                            Self::Error3 { .. } => 3u32,
                        }
                    }
                }
            };
        );

        let input: syn::DeriveInput = syn::parse_quote!(
            #[derive(Error)]
            #[runtime_error(autonumber, module = "module::TheModule")]
            pub enum Error {
                Error0,
                #[runtime_error(code = 2)]
                Error2 {
                    payload: Vec<u8>,
                },
                Error1(String),
                Error3,
            }
        );
        let error_derivation = super::derive_error(input);
        let actual: syn::Stmt = syn::parse2(error_derivation).unwrap();

        crate::assert_empty_diff!(actual, expected);
    }
}
