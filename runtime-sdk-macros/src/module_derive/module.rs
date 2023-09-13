use proc_macro2::TokenStream;
use quote::quote;

/// Deriver for the `Module` trait.
pub struct DeriveModule {
    /// Items specifying the module configuration.
    module_cfg: Vec<syn::ImplItem>,
}

impl DeriveModule {
    pub fn new() -> Box<Self> {
        Box::new(Self { module_cfg: vec![] })
    }
}

impl super::Deriver for DeriveModule {
    fn preprocess(&mut self, item: syn::ImplItem) -> Option<syn::ImplItem> {
        match item {
            syn::ImplItem::Type(ref ty) => {
                match ty.ident.to_string().as_str() {
                    "Error" | "Event" | "Parameters" => {
                        self.module_cfg.push(item);
                        None // Take the item.
                    }
                    _ => Some(item), // Return the item.
                }
            }
            syn::ImplItem::Const(ref cnst) => {
                match cnst.ident.to_string().as_str() {
                    "NAME" | "VERSION" => {
                        self.module_cfg.push(item);
                        None // Take the item.
                    }
                    _ => Some(item), // Return the item.
                }
            }
            _ => Some(item), // Return the item.
        }
    }

    fn derive(&mut self, generics: &syn::Generics, ty: &Box<syn::Type>) -> TokenStream {
        if self.module_cfg.is_empty() {
            return quote! {};
        }
        let module_cfg = &self.module_cfg;

        quote! {
            #[automatically_derived]
            impl #generics sdk::module::Module for #ty {
                #(#module_cfg)*
            }
        }
    }
}
