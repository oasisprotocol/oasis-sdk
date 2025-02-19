use darling::{util::SpannedValue, FromAttributes, FromDeriveInput, FromField};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Ident, ImplItem, ImplItemFn, ItemImpl, Path};
use tiny_keccak::{Hasher as _, Keccak};

const SELECTOR_LENGTH: usize = 4;

#[derive(FromAttributes)]
#[darling(attributes(evm_method))]
struct EvmMethod {
    hash: Option<SpannedValue<String>>,
    signature: Option<SpannedValue<String>>,
}

fn evm_crate_path() -> Path {
    let is_internal = std::env::var("CARGO_PKG_NAME")
        .map(|pkg_name| pkg_name == "oasis-runtime-sdk-evm")
        .unwrap_or_default();
    if is_internal {
        // Doctests are their own crates, but they share the name of the primary crate.
        // Thus, the primary crate needs to refer to itself. Either that or depend on unstable
        // rustdoc env vars.
        syn::parse_quote!(crate::oasis_runtime_sdk_evm)
    } else {
        syn::parse_quote!(::oasis_runtime_sdk_evm)
    }
}

fn get_method(f: &ImplItemFn) -> Option<TokenStream> {
    let attr = match EvmMethod::from_attributes(&f.attrs) {
        Ok(a) => a,
        Err(e) => {
            e.span().unwrap().error(format!("{}", e)).emit();
            return None;
        }
    };

    if attr.hash.is_some() && attr.signature.is_some() {
        attr.hash
            .unwrap()
            .span()
            .unwrap()
            .error("Only one of `signature` and `hash` must be specified")
            .emit();
        return None;
    }

    let selector: Vec<u8> = if let Some(ref hash) = attr.hash {
        match hex::decode(hash.as_ref()) {
            Ok(bytes) => {
                if bytes.len() != SELECTOR_LENGTH {
                    hash.span()
                        .unwrap()
                        .error(format!("Hash must be {} bytes", SELECTOR_LENGTH))
                        .emit();
                    return None;
                }
                bytes
            }
            Err(_) => {
                hash.span()
                    .unwrap()
                    .error("Hash must be a valid hex string")
                    .emit();
                return None;
            }
        }
    } else if let Some(ref sig) = attr.signature {
        let mut bytes = [0u8; 32];
        let mut keccak = Keccak::v256();
        keccak.update(sig.as_bytes());
        keccak.finalize(&mut bytes);
        bytes[..SELECTOR_LENGTH].to_vec()
    } else {
        // If it's none of those, it's either not our attribute at all or there
        // were no attributes.
        return None;
    };

    let byte_tokens = selector.into_iter().map(|b| quote!(#b));
    let method_name = &f.sig.ident;
    Some(quote! {
        &[#(#byte_tokens ,)*] => Some(Self::#method_name(handle, #SELECTOR_LENGTH)),
    })
}

pub fn derive_evm_contract(input: ItemImpl) -> TokenStream {
    let mut method_arms: Vec<TokenStream> = Vec::new();
    let mut address_fn: Option<_> = None;

    for item in input.items.iter() {
        if let ImplItem::Fn(f) = item {
            if f.attrs
                .iter()
                .any(|attr| attr.path().is_ident("evm_method"))
            {
                method_arms.extend(get_method(f).into_iter());
            }
            if f.attrs
                .iter()
                .any(|attr| attr.path().is_ident("evm_contract_address"))
            {
                address_fn = Some(f.sig.ident.clone());
            }
        }
    }
    if address_fn.is_none() {
        input.impl_token
        .span.unwrap()
            .error("Missing implementation for address function (use the `evm_contract_address` attribute)")
            .emit();
        return TokenStream::new();
    }

    let impl_generics = &input.generics;
    let self_ty = input.self_ty.clone();
    let evm_crate = evm_crate_path();
    quote! {
        #input

        #[automatically_derived]
        impl #impl_generics #evm_crate::precompile::contract::StaticContract for #self_ty {
            fn address() -> ::primitive_types::H160 {
                Self::#address_fn()
            }

            fn dispatch_call(
                handle: &mut impl ::evm::executor::stack::PrecompileHandle,
            ) -> Option<Result<#evm_crate::precompile::PrecompileOutput, #evm_crate::precompile::PrecompileFailure>> {
                use #evm_crate::precompile as precompiles;
                let selector = match handle.input().get(..4) {
                    Some(slice) => slice,
                    None => return Some(Err(precompiles::PrecompileFailure::Revert {
                        exit_status: precompiles::ExitRevert::Reverted,
                        output: vec![],
                    })),
                };
                match selector {
                    #(#method_arms)*
                    _ => Some(Err(precompiles::PrecompileFailure::Revert {
                        exit_status: precompiles::ExitRevert::Reverted,
                        output: vec![],
                    })),
                }
            }
        }

        #[automatically_derived]
        impl #impl_generics ::evm::executor::stack::PrecompileSet for #self_ty {
            fn execute(&self, handle: &mut impl ::evm::executor::stack::PrecompileHandle) -> Option<Result<::evm::executor::stack::PrecompileOutput, ::evm::executor::stack::PrecompileFailure>> {
                match self.is_precompile(handle.code_address(), handle.remaining_gas()) {
                    ::evm::executor::stack::IsPrecompileResult::Answer {
                        is_precompile: true,
                        extra_cost,
                    } => {
                        if let Err(e) = handle.record_cost(extra_cost) {
                            return Some(Err(e.into()));
                        }
                    }
                    ::evm::executor::stack::IsPrecompileResult::OutOfGas => {
                        return Some(Err(::evm::ExitError::OutOfGas.into()));
                    }
                    _ => {
                        return None;
                    }
                }

                <Self as #evm_crate::precompile::contract::StaticContract>::dispatch_call(handle)
            }

            fn is_precompile(
                &self,
                address: ::primitive_types::H160,
                _remaining_gas: u64,
            ) -> ::evm::executor::stack::IsPrecompileResult {
                ::evm::executor::stack::IsPrecompileResult::Answer {
                    is_precompile: Self::address() == address,
                    extra_cost: 0,
                }
            }
        }
    }
}

#[derive(FromDeriveInput)]
#[darling(supports(struct_named), attributes(evm_event))]
struct EvmEvent {
    ident: Ident,
    data: darling::ast::Data<(), EvmEventArg>,
    name: String,
}

#[derive(FromField)]
#[darling(attributes(evm_event))]
struct EvmEventArg {
    ident: Option<Ident>,
    indexed: darling::util::Flag,
    arg_type: String,
}

pub fn derive_evm_event(input: DeriveInput) -> TokenStream {
    let event = match EvmEvent::from_derive_input(&input) {
        Ok(event) => event,
        Err(e) => return e.write_errors(),
    };

    let fields = event.data.as_ref().take_struct().unwrap().fields;

    let signature = {
        let mut bytes = [0u8; 32];
        let mut keccak = Keccak::v256();
        keccak.update(event.name.as_bytes());
        keccak.update(b"(");
        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                keccak.update(b",");
            }
            keccak.update(field.arg_type.as_bytes());
        }
        keccak.update(b")");
        keccak.finalize(&mut bytes);
        bytes
    };
    let signature_tokens = signature.into_iter().map(|b| quote!(#b));

    let evm_crate = evm_crate_path();

    let topics: Vec<TokenStream> = fields.iter().filter_map(|f| {
        if f.indexed.is_present() {
            let name = f.ident.as_ref().unwrap();
            Some(quote! { ::primitive_types::H256::from_slice(::ethabi::encode(&[self.#name.clone()]).as_slice()) })
        } else {
            None
        }
    }).collect();

    let data: Vec<TokenStream> = fields
        .iter()
        .filter_map(|f| {
            if !f.indexed.is_present() {
                let name = f.ident.as_ref().unwrap();
                Some(quote! { self.#name.clone() })
            } else {
                None
            }
        })
        .collect();
    let data_tokens = if data.is_empty() {
        quote! { vec![] }
    } else {
        quote! {
            ::ethabi::encode(&[
                #(#data ,)*
            ])
        }
    };

    let event_ty = &event.ident;

    quote! {
        impl #evm_crate::precompile::contract::EvmEvent for #event_ty {
            fn emit<C: #evm_crate::precompile::contract::StaticContract>(&self, handle: &mut impl ::evm::executor::stack::PrecompileHandle) -> Result<(), ::evm::ExitError> {
                let address = C::address();
                let topics = vec![
                    ::primitive_types::H256::from_slice(&[#(#signature_tokens ,)*]),
                    #(#topics ,)*
                ];
                let data = #data_tokens;
                handle.log(address, topics, data)
            }
        }
    }
}
