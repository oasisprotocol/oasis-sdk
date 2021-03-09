use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

pub fn wrap_in_const(tokens: TokenStream) -> TokenStream {
    quote! {
        const _: () = {
            #tokens
        };
    }
}

/// Determines what crate name should be used to refer to `oasis_runtime_sdk` types.
/// Required for use within the SDK itself (crates cannot refer to their own names).
pub fn sdk_crate_path() -> syn::Path {
    let is_internal = std::env::var("CARGO_PKG_NAME")
        .map(|pkg_name| pkg_name == "oasis-runtime-sdk")
        .unwrap_or_default();
    if is_internal {
        syn::parse_quote!(crate)
    } else {
        syn::parse_quote!(oasis_runtime_sdk)
    }
}

pub trait CodedVariant {
    fn ident(&self) -> &Ident;

    fn code(&self) -> Option<u32>;
}

pub fn enum_code_converter<V: CodedVariant>(
    enum_binding: &Ident,
    variants: &[&V],
    autonumber: bool,
) -> TokenStream {
    if variants.is_empty() {
        return quote!(0); // Early return with default if there are no variants.
    }

    let mut next_autonumber = 0u32;
    let mut reserved_numbers = std::collections::BTreeSet::new();
    let match_arms = variants.iter().map(|variant| {
        let variant_ident = variant.ident();
        let code = match variant.code() {
            Some(code) => {
                if reserved_numbers.contains(&code) {
                    variant_ident
                        .span()
                        .unwrap()
                        .error(format!("code {} already used", code))
                        .emit();
                    return quote!({});
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
                return quote!();
            }
        };
        quote!(Self::#variant_ident { .. } => { #code })
    });
    quote! {
        match #enum_binding {
            #(#match_arms)*
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_empty_enum_converter() {
        struct DummyVariant {}
        impl CodedVariant for DummyVariant {
            fn ident(&self) -> &Ident {
                unimplemented!()
            }
            fn code(&self) -> Option<u32> {
                unimplemented!()
            }
        }
        let variants: &[&DummyVariant] = &[];

        let expected: syn::Expr = syn::parse_quote!(0);
        let converter = enum_code_converter(&quote::format_ident!("the_enum"), variants, false);
        let actual: syn::Expr = syn::parse2(converter).unwrap();
        assert_eq!(expected, actual);
    }
}
