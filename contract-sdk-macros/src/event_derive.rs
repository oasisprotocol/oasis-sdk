use darling::{util::Flag, FromDeriveInput, FromVariant};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident};

use crate::util;

#[derive(FromDeriveInput)]
#[darling(supports(enum_any), attributes(sdk_event))]
struct Event {
    ident: Ident,

    data: darling::ast::Data<EventVariant, darling::util::Ignored>,

    /// The optional module name for all events.
    #[darling(default)]
    module_name: String,

    /// Whether to sequentially autonumber the event codes.
    /// This option exists as a convenience for contracts that
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

pub fn derive_event(input: DeriveInput) -> TokenStream {
    let sdk_crate = util::sdk_crate_identifier();
    let event = match Event::from_derive_input(&input) {
        Ok(event) => event,
        Err(e) => return e.write_errors(),
    };

    let event_ty_ident = &event.ident;

    let (module_name_body, code_body) = convert_variants(
        &format_ident!("self"),
        &event.module_name,
        &event.data.as_ref().take_enum().unwrap(),
        event.autonumber.is_present(),
    );

    util::wrap_in_const(quote! {
        use #sdk_crate as __sdk;

        #[automatically_derived]
        impl __sdk::event::Event for #event_ty_ident {
            fn module_name(&self) -> &str {
                #module_name_body
            }

            fn code(&self) -> u32 {
                #code_body
            }
        }
    })
}

fn convert_variants(
    enum_binding: &Ident,
    module_name: &str,
    variants: &[&EventVariant],
    autonumber: bool,
) -> (TokenStream, TokenStream) {
    if variants.is_empty() {
        return (quote!(#module_name), quote!(0));
    }

    let mut next_autonumber = 0u32;
    let mut reserved_numbers = std::collections::BTreeSet::new();

    let (module_name_matches, code_matches): (Vec<_>, Vec<_>) = variants
        .iter()
        .map(|variant| {
            let variant_ident = &variant.ident;

            let code = match variant.code {
                Some(code) => {
                    if reserved_numbers.contains(&code) {
                        variant_ident
                            .span()
                            .unwrap()
                            .error(format!("code {} already used", code))
                            .emit();
                        return (quote!(), quote!());
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
                        .error("missing `code` for variant")
                        .emit();
                    return (quote!(), quote!());
                }
            };

            (
                quote! {
                    Self::#variant_ident { .. } => #module_name,
                },
                quote! {
                    Self::#variant_ident { .. } => #code,
                },
            )
        })
        .unzip();

    (
        quote! {
            match #enum_binding {
                #(#module_name_matches)*
            }
        },
        quote! {
            match #enum_binding {
                #(#code_matches)*
            }
        },
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn generate_event_impl_auto() {
        let expected: syn::Stmt = syn::parse_quote!(
            #[doc(hidden)]
            const _: () = {
                use :: oasis_contract_sdk as __sdk;
                #[automatically_derived]
                impl __sdk::event::Event for MainEvent {
                    fn module_name(&self) -> &str {
                        match self {
                            Self::Event0 { .. } => "",
                            Self::Event2 { .. } => "",
                            Self::Event1 { .. } => "",
                            Self::Event3 { .. } => "",
                        }
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
            #[doc(hidden)]
            const _: () = {
                use :: oasis_contract_sdk as __sdk;
                #[automatically_derived]
                impl __sdk::event::Event for MainEvent {
                    fn module_name(&self) -> &str {
                        "the_module_name"
                    }
                    fn code(&self) -> u32 {
                        0
                    }
                }
            };
        );

        let input: syn::DeriveInput = syn::parse_quote!(
            #[derive(Event)]
            #[sdk_event(autonumber, module_name = "the_module_name")]
            pub enum MainEvent {}
        );
        let event_derivation = super::derive_event(input);
        let actual: syn::Stmt = syn::parse2(event_derivation).unwrap();

        crate::assert_empty_diff!(actual, expected);
    }
}
