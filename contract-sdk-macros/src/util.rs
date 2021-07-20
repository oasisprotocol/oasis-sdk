use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::Ident;

pub fn wrap_in_const(tokens: TokenStream) -> TokenStream {
    quote! {
        #[doc(hidden)]
        const _: () = {
            #tokens
        };
    }
}

pub fn sdk_crate_identifier() -> TokenStream {
    let found_crate = crate_name("oasis-contract-sdk").unwrap_or(FoundCrate::Itself);

    match found_crate {
        FoundCrate::Itself => quote!(::oasis_contract_sdk),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!( ::#ident )
        }
    }
}
