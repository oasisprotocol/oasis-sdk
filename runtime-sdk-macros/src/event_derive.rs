use darling::{util::Flag, FromDeriveInput, FromVariant};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident};

use crate::generators::{self as gen, CodedVariant};

#[derive(FromDeriveInput)]
#[darling(supports(enum_any), attributes(sdk_event))]
struct Event {
    ident: Ident,

    data: darling::ast::Data<EventVariant, darling::util::Ignored>,

    /// The path to a const set to the module name.
    module_name: Option<syn::Path>,

    /// Whether to sequentially autonumber the event codes.
    /// This option exists as a convenience for runtimes that
    /// only append events or release only breaking changes.
    #[darling(rename = "autonumber")]
    autonumber: Flag,
}

#[derive(FromVariant)]
#[darling(attributes(sdk_event))]
struct EventVariant {
    ident: Ident,

    /// The explicit ID of the event code. Overrides any autonumber set on the event enum.
    #[darling(rename = "code")]
    code: Option<u32>,
}

impl CodedVariant for EventVariant {
    const FIELD_NAME: &'static str = "code";

    fn ident(&self) -> &Ident {
        &self.ident
    }

    fn code(&self) -> Option<u32> {
        self.code
    }
}

pub fn derive_event(input: DeriveInput) -> TokenStream {
    let event = match Event::from_derive_input(&input) {
        Ok(event) => event,
        Err(e) => return e.write_errors(),
    };

    let event_ty_ident = &event.ident;
    let module_name = event
        .module_name
        .unwrap_or_else(|| syn::parse_quote!(MODULE_NAME));

    let code_converter = gen::enum_code_converter(
        &format_ident!("self"),
        &event.data.as_ref().take_enum().unwrap(),
        event.autonumber.is_present(),
    );

    let sdk_crate = gen::sdk_crate_path();

    gen::wrap_in_const(quote! {
        impl #sdk_crate::event::Event for #event_ty_ident {
            fn module_name() -> &'static str {
                #module_name
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
    fn generate_event_impl_auto() {
        let expected: syn::Stmt = syn::parse_quote!(
            const _: () = {
                impl ::oasis_runtime_sdk::event::Event for MainEvent {
                    fn module_name() -> &'static str {
                        MODULE_NAME
                    }
                    fn code(&self) -> u32 {
                        match self {
                            Self::Event0 { .. } => 0u32,
                            Self::Event2 { .. } => 2u32,
                            Self::Event1 { .. } => 1u32,
                            Self::Event3 { .. } => 3u32,
                        }
                    }
                }
            };
        );

        let input: syn::DeriveInput = syn::parse_quote!(
            #[derive(Event)]
            #[sdk_event(autonumber)]
            pub enum MainEvent {
                Event0,
                #[sdk_event(code = 2)]
                Event2 {
                    payload: Vec<u8>,
                },
                Event1(String),
                Event3,
            }
        );
        let event_derivation = super::derive_event(input);
        let actual: syn::Stmt = syn::parse2(event_derivation).unwrap();

        crate::assert_empty_diff!(actual, expected);
    }

    #[test]
    fn generate_event_impl_manual() {
        let expected: syn::Stmt = syn::parse_quote!(
            const _: () = {
                impl ::oasis_runtime_sdk::event::Event for MainEvent {
                    fn module_name() -> &'static str {
                        THE_MODULE_NAME
                    }
                    fn code(&self) -> u32 {
                        0
                    }
                }
            };
        );

        let input: syn::DeriveInput = syn::parse_quote!(
            #[derive(Event)]
            #[sdk_event(autonumber, module_name = "THE_MODULE_NAME")]
            pub enum MainEvent {}
        );
        let event_derivation = super::derive_event(input);
        let actual: syn::Stmt = syn::parse2(event_derivation).unwrap();

        crate::assert_empty_diff!(actual, expected);
    }
}
