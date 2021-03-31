use darling::{util::Flag, FromDeriveInput, FromVariant};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident};

use crate::generators::{self as gen, CodedVariant};

#[derive(FromDeriveInput)]
#[darling(supports(enum_any), attributes(sdk_error))]
struct Error {
    ident: Ident,

    data: darling::ast::Data<ErrorVariant, darling::util::Ignored>,

    /// The path to a const set to the module name.
    module_name: syn::Path,

    /// Whether to sequentially autonumber the error codes.
    /// This option exists as a convenience for runtimes that
    /// only append errors or release only breaking changes.
    #[darling(default, rename = "autonumber")]
    autonumber: Flag,
}

#[derive(FromVariant)]
#[darling(attributes(sdk_error))]
struct ErrorVariant {
    ident: Ident,

    /// The explicit ID of the error code. Overrides any autonumber set on the error enum.
    #[darling(default, rename = "code")]
    code: Option<u32>,
}

impl CodedVariant for ErrorVariant {
    const FIELD_NAME: &'static str = "code";

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

    let code_converter = gen::enum_code_converter(
        &format_ident!("self"),
        &error.data.as_ref().take_enum().unwrap(),
        error.autonumber.is_some(),
    );

    let sdk_crate = gen::sdk_crate_path();

    let module_name = error.module_name;

    gen::wrap_in_const(quote! {
        use #sdk_crate::{
            self as sdk, core::types::Error as RuntimeError, error::Error as _,
        };

        impl sdk::error::Error for #error_ty_ident {
            fn module_name() -> &'static str {
                #module_name
            }

            fn code(&self) -> u32 {
                #code_converter
            }
        }

        impl From<#error_ty_ident> for RuntimeError {
            fn from(err: #error_ty_ident) -> RuntimeError {
                RuntimeError::new(#error_ty_ident::module_name(), err.code(), &err.to_string())
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
                use oasis_runtime_sdk::{
                    self as sdk, core::types::Error as RuntimeError, error::Error as _,
                };
                impl sdk::error::Error for Error {
                    fn module_name() -> &'static str {
                        MODULE_NAME
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
                impl From<Error> for RuntimeError {
                    fn from(err: Error) -> RuntimeError {
                        RuntimeError::new(Error::module_name(), err.code(), &err.to_string())
                    }
                }
            };
        );

        let input: syn::DeriveInput = syn::parse_quote!(
            #[derive(Error)]
            #[sdk_error(autonumber, module_name = "MODULE_NAME")]
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
        let actual: syn::Stmt = syn::parse2(error_derivation).unwrap();

        crate::assert_empty_diff!(actual, expected);
    }
}
