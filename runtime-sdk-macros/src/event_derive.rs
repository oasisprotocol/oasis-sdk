use darling::{util::Flag, FromDeriveInput, FromVariant};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident};

#[derive(FromDeriveInput)]
#[darling(supports(enum_any), attributes(event))]
struct Event {
    ident: Ident,

    data: darling::ast::Data<EventVariant, darling::util::Ignored>,

    /// The type ident of the event enum.
    module: syn::Path,

    /// Whether to sequentially autonumber the event codes.
    /// This option exists as a convenience for runtimes that
    /// only append events or release only breaking changes.
    #[darling(default)]
    autonumber: Flag,
}

#[derive(FromVariant)]
#[darling(attributes(event))]
struct EventVariant {
    ident: Ident,

    /// The explicit ID of the event code. Overrides any autonumber set on the event enum.
    #[darling(default)]
    id: Option<u32>,
}

pub fn derive_event(input: DeriveInput) -> TokenStream {
    let event = match Event::from_derive_input(&input) {
        Ok(event) => event,
        Err(e) => return e.write_errors(),
    };

    let event_ty_ident = &event.ident;
    let wrapper_ident = format_ident!("_IMPL_EVENT_FOR_{}", event_ty_ident);
    let module_path = &event.module;

    let variants = event.data.as_ref().take_enum().unwrap();
    let mut next_autonumber = 0u32;
    let mut reserved_numbers = std::collections::BTreeSet::new();
    let code_match_arms = variants.iter().map(|variant| {
        let event_id = match variant.id {
            Some(id) => {
                if reserved_numbers.contains(&id) {
                    variant
                        .ident
                        .span()
                        .unwrap()
                        .error(format!("id {} already used", id))
                        .emit();
                    return quote!({});
                }
                reserved_numbers.insert(id);
                id
            }
            None if event.autonumber.is_some() => {
                let mut reserved_successors = reserved_numbers.range(next_autonumber..);
                while reserved_successors.next() == Some(&next_autonumber) {
                    next_autonumber += 1;
                }
                let i = next_autonumber;
                reserved_numbers.insert(i);
                next_autonumber += 1;
                i
            }
            None => {
                variant
                    .ident
                    .span()
                    .unwrap()
                    .error("missing `id` for variant")
                    .emit();
                return quote!();
            }
        };
        let variant_ident = &variant.ident;
        quote! {
            Self::#variant_ident { .. } => { #event_id }
        }
    });

    quote! {
        const #wrapper_ident: () = {
            use oasis_runtime_sdk::core::common::cbor;

            impl oasis_runtime_sdk::event::Event for #event_ty_ident {
                fn module(&self) -> &str {
                    <#module_path as oasis_runtime_sdk::module::Module>::NAME
                }

                fn code(&self) -> u32 {
                    match self {
                        #(#code_match_arms)*
                    }
                }

                fn value(&self) -> cbor::Value {
                    cbor::to_value(self)
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn generate_event_impl() {
        let expected: syn::Stmt = syn::parse_quote!(
            const _IMPL_EVENT_FOR_MainEvent: () = {
                use oasis_runtime_sdk::core::common::cbor;
                impl oasis_runtime_sdk::event::Event for MainEvent {
                    fn module(&self) -> &str {
                        <module::TheModule as oasis_runtime_sdk::module::Module>::NAME
                    }
                    fn code(&self) -> u32 {
                        match self {
                            Self::Event0 { .. } => 0u32,
                            Self::Event2 { .. } => 2u32,
                            Self::Event1 { .. } => 1u32,
                            Self::Event3 { .. } => 3u32,
                        }
                    }
                    fn value(&self) -> cbor::Value {
                        cbor::to_value(self)
                    }
                }
            };
        );

        let input: syn::DeriveInput = syn::parse_quote!(
            #[derive(Event)]
            #[event(autonumber, module = "module::TheModule")]
            pub enum MainEvent {
                Event0,
                #[event(id = 2)]
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
}
