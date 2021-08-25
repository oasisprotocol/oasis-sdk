use darling::FromDeriveInput;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::generators::{self as gen, CodedVariant, EnumCodeConverter};

#[derive(FromDeriveInput)]
#[darling(supports(enum_any), attributes(sdk_event))]
struct Event {
    ident: syn::Ident,

    data: darling::ast::Data<EventVariant, darling::util::Ignored>,

    /// The path to a const set to the module name.
    #[darling(default)]
    module_name: Option<syn::LitStr>,

    /// Whether to sequentially autonumber the event codes.
    /// This option exists as a convenience for runtimes that
    /// only append events or release only breaking changes.
    #[darling(default, rename = "autonumber")]
    autonumber: darling::util::Flag,
}

#[derive(darling::FromVariant)]
#[darling(attributes(sdk_event))]
struct EventVariant {
    ident: syn::Ident,

    /// The explicit ID of the event code. Overrides any autonumber set on the event enum.
    #[darling(default, rename = "code")]
    code: Option<u32>,
}

impl CodedVariant for EventVariant {
    const FIELD_NAME: &'static str = "code";

    fn ident(&self) -> &syn::Ident {
        &self.ident
    }

    fn code(&self) -> Option<u32> {
        self.code
    }
}

pub fn derive_event(input: syn::DeriveInput) -> TokenStream {
    let event = match Event::from_derive_input(&input) {
        Ok(event) => event,
        Err(e) => return e.write_errors(),
    };

    let event_ty_ident = &event.ident;
    let module_name = match gen::module_name(event.module_name.as_ref()) {
        Ok(expr) => expr,
        Err(_) => return quote!(),
    };

    let EnumCodeConverter {
        converter: code_converter,
        used_codes,
    } = match gen::enum_code_converter(
        &format_ident!("self"),
        &event.data.as_ref().take_enum().unwrap(),
        event.autonumber.is_some(),
    ) {
        Ok(cc) => cc,
        Err(_) => return quote!(),
    };

    let sdk_crate = gen::sdk_crate_path();

    gen::wrap_in_const(quote! {
        use #sdk_crate::core::common::cbor;

        impl #sdk_crate::event::Event for #event_ty_ident {
            fn module_name() -> &'static str {
                #module_name
            }

            fn code(&self) -> u32 {
                #code_converter
            }

            fn has_variant_with_code(code: u32) -> bool {
                return false #(|| code == #used_codes)*
            }

            fn value(&self) -> cbor::Value {
                cbor::to_value(self)
            }

            fn from_value(value: cbor::Value) -> Result<Self, cbor::Error> {
                cbor::from_value(value)
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
                use ::oasis_runtime_sdk::core::common::cbor;
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
                    fn has_variant_with_code(code: u32) -> bool {
                        return false
                            || code == 0u32
                            || code == 2u32
                            || code == 1u32
                            || code == 3u32;
                    }
                    fn value(&self) -> cbor::Value {
                        cbor::to_value(self)
                    }
                    fn from_value(value: cbor::Value) -> Result<Self, cbor::Error> {
                        cbor::from_value(value)
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
                use ::oasis_runtime_sdk::core::common::cbor;
                impl ::oasis_runtime_sdk::event::Event for MainEvent {
                    fn module_name() -> &'static str {
                        THE_MODULE_NAME
                    }
                    fn code(&self) -> u32 {
                        0
                    }
                    fn has_variant_with_code(code: u32) -> bool {
                        return false || code == 0u32;
                    }
                    fn value(&self) -> cbor::Value {
                        cbor::to_value(self)
                    }
                    fn from_value(value: cbor::Value) -> Result<Self, cbor::Error> {
                        cbor::from_value(value)
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
