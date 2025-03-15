use darling::{
    util::{Flag, SpannedValue},
    FromAttributes, FromDeriveInput, FromField, FromVariant,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, Pat, Path};
use tiny_keccak::{Hasher as _, Keccak};

const SELECTOR_LENGTH: usize = 4;

#[derive(FromAttributes)]
#[darling(attributes(evm_method))]
struct EvmMethod {
    signature: SpannedValue<String>,
    convert: Flag,
}

#[derive(Debug, Clone)]
struct Signature {
    selector: [u8; SELECTOR_LENGTH],
    name: Ident,
    convert: bool,
    args: Vec<String>,
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

fn parse_signature(name: Ident, signature: &str) -> Signature {
    let mut hash = [0u8; 32];
    let mut keccak = Keccak::v256();
    keccak.update(signature.as_bytes());
    keccak.finalize(&mut hash);
    let mut selector = [0u8; SELECTOR_LENGTH];
    selector.copy_from_slice(&hash[..SELECTOR_LENGTH]);

    let mut args = Vec::new();
    if let Some((_, after_paren)) = signature.split_once('(') {
        if let Some((arg_list, _)) = after_paren.split_once(')') {
            args = arg_list
                .split(',')
                .map(str::to_string)
                .filter(|s| !s.is_empty())
                .collect();
        }
    }

    Signature {
        selector,
        name,
        convert: false,
        args,
    }
}

fn get_method(f: &ImplItemFn) -> Option<(Signature, EvmMethod, TokenStream)> {
    let attr = match EvmMethod::from_attributes(&f.attrs) {
        Ok(a) => a,
        Err(e) => {
            e.span().unwrap().error(format!("{}", e)).emit();
            return None;
        }
    };

    let mut signature = parse_signature(f.sig.ident.clone(), attr.signature.as_str());
    signature.convert = attr.convert.is_present();
    for inp in f.sig.inputs.iter() {
        match inp {
            FnArg::Receiver(recv) => {
                recv.self_token
                    .span
                    .unwrap()
                    .error("Dispatchable methods can't have a receiver")
                    .emit();
                return None;
            }
            FnArg::Typed(typed) => match *typed.pat {
                Pat::Ident(_) => continue,
                _ => {
                    typed
                        .colon_token
                        .span
                        .unwrap()
                        .error("Unsupported function input type")
                        .emit();
                    return None;
                }
            },
        }
    }
    if signature.convert && signature.args.len() != f.sig.inputs.len() - 1 {
        signature
            .name
            .span()
            .unwrap()
            .error("Number of function arguments doesn't match EVM signature")
            .emit();
        return None;
    }

    let byte_tokens = signature.selector.iter().map(|&b| quote!(#b));
    let byte_list = quote!(#(#byte_tokens ,)*);
    Some((signature, attr, byte_list))
}

fn generate_method_call(attr: &EvmMethod, sig: &Signature) -> Option<TokenStream> {
    let mut decoder_list: Vec<TokenStream> = Vec::new();
    let mut call_args: Vec<Ident> = Vec::new();
    let mut arg_decls: Vec<TokenStream> = Vec::new();
    for (i, evm_ty) in sig.args.iter().enumerate() {
        let arg_name = format_ident!("arg_{}", i);
        call_args.push(arg_name.clone());
        match evm_ty.as_str() {
            "address" => {
                decoder_list.push(quote!(::ethabi::ParamType::Address));
                arg_decls.push(quote! {
                    let #arg_name = decoded_args[#i].clone().into_address().unwrap();
                });
            }
            "uint256" => {
                decoder_list.push(quote!(::ethabi::ParamType::Uint(256)));
                let temp_name = format_ident!("{}_uint", arg_name);
                arg_decls.push(quote! {
                    let #temp_name = decoded_args[#i].clone().into_uint().unwrap();
                    if #temp_name.bits() > (u128::BITS as usize) {
                        return Some(Err(::evm::executor::stack::PrecompileFailure::Error {
                            exit_status: ::evm::ExitError::Other("integer overflow".into()),
                        }));
                    }
                    let #arg_name = #temp_name.as_u128();
                });
            }
            ty => {
                attr.signature
                    .span()
                    .unwrap()
                    .error(format!("Unknown argument type '{}'", ty))
                    .emit();
            }
        }
    }
    let method_name = sig.name.clone();
    Some(quote! { {
        let decoded_args = match ::ethabi::decode(&[#(#decoder_list),*], &handle.input()[#SELECTOR_LENGTH..]) {
            Err(e) => return Some(Err(::evm::executor::stack::PrecompileFailure::Error {
                exit_status: ::evm::ExitError::Other("invalid argument".into()),
            })),
            Ok(tokens) => tokens,
        };
        #(#arg_decls)*
        Some(Self::#method_name(handle, #(#call_args),*))
    } })
}

pub fn derive_evm_contract(input: ItemImpl) -> TokenStream {
    let mut methods: Vec<(Signature, EvmMethod, TokenStream)> = Vec::new();
    let mut address_fn: Option<_> = None;

    for item in input.items.iter() {
        if let ImplItem::Fn(f) = item {
            if f.attrs
                .iter()
                .any(|attr| attr.path().is_ident("evm_method"))
            {
                methods.extend(get_method(f).into_iter());
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

    let method_arms: Vec<TokenStream> = methods
        .into_iter()
        .map(|(sig, method, keccak)| {
            if sig.convert {
                let decode_body = generate_method_call(&method, &sig);
                quote! {
                    &[#keccak] => { #decode_body }
                }
            } else {
                let method_name = sig.name.clone();
                quote! {
                    &[#keccak] => Some(Self::#method_name(handle, #SELECTOR_LENGTH)),
                }
            }
        })
        .collect();

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
            ) -> Option<Result<::evm::executor::stack::PrecompileOutput, ::evm::executor::stack::PrecompileFailure>> {
                if handle.context().address != handle.code_address() {
                    return Some(Err(::evm::executor::stack::PrecompileFailure::Error {
                        exit_status: ::evm::ExitError::Other("invalid call".into()),
                    }));
                }
                let selector = match handle.input().get(..#SELECTOR_LENGTH) {
                    Some(slice) => slice,
                    None => return Some(Err(::evm::executor::stack::PrecompileFailure::Revert {
                        exit_status: ::evm::ExitRevert::Reverted,
                        output: vec![],
                    })),
                };
                match selector {
                    #(#method_arms)*
                    _ => Some(Err(::evm::executor::stack::PrecompileFailure::Revert {
                        exit_status: ::evm::ExitRevert::Reverted,
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
        #[automatically_derived]
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

#[derive(FromDeriveInput)]
#[darling(supports(enum_unit, enum_tuple), attributes(evm_error))]
struct EvmError {
    ident: Ident,
    data: darling::ast::Data<EvmErrorArg, ()>,
}

#[derive(FromVariant)]
#[darling(attributes(evm_error))]
struct EvmErrorArg {
    ident: Ident,
    signature: SpannedValue<String>,
}

pub fn derive_evm_error(input: DeriveInput) -> TokenStream {
    let error = match EvmError::from_derive_input(&input) {
        Ok(error) => error,
        Err(e) => return e.write_errors(),
    };

    let mut variant_encoders: Vec<TokenStream> = Vec::new();
    for variant in error.data.as_ref().take_enum().unwrap().iter() {
        let ident = &variant.ident;
        let signature = parse_signature(variant.ident.clone(), &variant.signature);
        let hash_bytes = signature.selector.map(|b| quote!(#b));
        let mut abi_tokens: Vec<TokenStream> = Vec::new();
        let mut match_fields: Vec<Ident> = Vec::new();
        for (i, evm_ty) in signature.args.iter().enumerate() {
            let field_ident = format_ident!("field_{}", i);
            match_fields.push(field_ident.clone());
            match evm_ty.as_str() {
                "address" => {
                    // The tuple field for this should be a primitive_types::H160.
                    abi_tokens.push(quote! {
                        ::ethabi::Token::Address(*#field_ident)
                    });
                }
                "uint256" => {
                    // The tuple field for this should be a u128.
                    abi_tokens.push(quote! {
                        ::ethabi::Token::Uint((*#field_ident).into())
                    });
                }
                "string" => {
                    // The tuple field for this should be anything that has a to_string() method.
                    abi_tokens.push(quote! {
                        ::ethabi::Token::String(#field_ident.to_string())
                    });
                }
                ty => {
                    variant
                        .signature
                        .span()
                        .unwrap()
                        .error(format!("Unknown type '{}'", ty))
                        .emit();
                    return TokenStream::new();
                }
            }
        }
        let (match_expr, encode_expr) = if match_fields.is_empty() {
            (quote! { Self::#ident }, TokenStream::new())
        } else {
            (
                quote! { Self::#ident(#(#match_fields,)*) },
                quote! { output.extend(::ethabi::encode(&[#(#abi_tokens,)*]).as_slice()); },
            )
        };
        variant_encoders.push(quote! {
            #match_expr => {
                let mut output = Vec::new();
                output.extend(&[#(#hash_bytes,)*]);
                #encode_expr
                ::evm::executor::stack::PrecompileFailure::Revert {
                    exit_status: ::evm::ExitRevert::Reverted,
                    output,
                }
            }
        });
    }

    let evm_crate = evm_crate_path();
    let error_ty = &error.ident;

    quote! {
        #[automatically_derived]
        impl #evm_crate::precompile::contract::EvmError for #error_ty {
            fn encode(&self) -> ::evm::executor::stack::PrecompileFailure {
                match self {
                    #(#variant_encoders)*
                }
            }
        }
    }
}
