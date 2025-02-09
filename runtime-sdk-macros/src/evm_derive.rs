use darling::{util::SpannedValue, FromAttributes};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{ImplItem, ImplItemFn, ItemImpl, Path};
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
        &[#(#byte_tokens ,)*] => Self::#method_name(&input[#SELECTOR_LENGTH..], gas_limit, ctx, is_static),
    })
}

pub fn derive_evm_contract(input: ItemImpl) -> TokenStream {
    let mut method_arms: Vec<TokenStream> = Vec::new();

    for item in input.items.iter() {
        if let ImplItem::Fn(f) = item {
            method_arms.extend(get_method(&f).into_iter());
        }
    }

    let impl_generics = &input.generics;
    let self_ty = input.self_ty.clone();
    let evm_crate = evm_crate_path();
    quote! {
        #input

        #[automatically_derived]
        impl #impl_generics #evm_crate::precompile::contract::StaticContract for #self_ty {
            fn dispatch_call(
                input: &[u8],
                gas_limit: Option<u64>,
                ctx: &#evm_crate::precompile::Context,
                is_static: bool,
            ) -> Result<(#evm_crate::precompile::PrecompileOutput, u64), #evm_crate::precompile::PrecompileFailure> {
                use #evm_crate::precompile as precompiles;
                let selector = match input.get(..4) {
                    Some(slice) => slice,
                    None => return Err(precompiles::PrecompileFailure::Revert {
                        exit_status: precompiles::ExitRevert::Reverted,
                        output: vec![],
                    }),
                };
                match selector {
                    #(#method_arms)*
                    _ => Err(precompiles::PrecompileFailure::Revert {
                        exit_status: precompiles::ExitRevert::Reverted,
                        output: vec![],
                    }),
                }
            }
        }
    }
}
