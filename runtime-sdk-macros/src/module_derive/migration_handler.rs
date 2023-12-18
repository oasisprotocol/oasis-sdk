use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;

use crate::emit_compile_error;

/// Deriver for the `MigrationHandler` trait.
pub struct DeriveMigrationHandler {
    /// Item defining the `MigrationHandler::Genesis` associated type.
    genesis_ty: Option<syn::ImplItem>,
    /// Migration functions.
    migrate_fns: Vec<MigrateFn>,
}

struct MigrateFn {
    item: syn::ImplItem,
    ident: syn::Ident,
    from_version: u32,
}

impl DeriveMigrationHandler {
    pub fn new() -> Box<Self> {
        Box::new(Self {
            genesis_ty: None,
            migrate_fns: vec![],
        })
    }
}

impl super::Deriver for DeriveMigrationHandler {
    fn preprocess(&mut self, item: syn::ImplItem) -> Option<syn::ImplItem> {
        match item {
            // We are looking for a `type Genesis = ...;` item.
            syn::ImplItem::Type(ref ty) if &ty.ident.to_string() == "Genesis" => {
                self.genesis_ty = Some(item);

                None // Take the item.
            }
            syn::ImplItem::Fn(ref f) => {
                // Check whether a `migration` attribute is set for the method.
                if let Some(attrs) = parse_attrs(&f.attrs) {
                    self.migrate_fns.push(MigrateFn {
                        ident: f.sig.ident.clone(),
                        from_version: attrs.from_version,
                        item,
                    });

                    None // Take the item.
                } else {
                    Some(item) // Return the item.
                }
            }
            _ => Some(item), // Return the item.
        }
    }

    fn derive(&mut self, generics: &syn::Generics, ty: &Box<syn::Type>) -> TokenStream {
        let genesis_ty = if let Some(genesis_ty) = &self.genesis_ty {
            genesis_ty
        } else {
            return quote! {};
        };

        // Sort by version to ensure migrations are processed in the right order.
        self.migrate_fns.sort_by_key(|f| f.from_version);

        let mut seen_versions = HashSet::new();
        let (migrate_fns, mut migrate_arms): (Vec<_>, Vec<_>) = self.migrate_fns.iter().map(|f| {
            let MigrateFn { item, ident, from_version } = f;
            if seen_versions.contains(from_version) {
                emit_compile_error(format!(
                    "Duplicate migration for version: {from_version}"
                ));
            }
            seen_versions.insert(from_version);

            (
                item,
                if from_version == &0 {
                    // Version zero is special as initializing from genesis always gets us latest.
                    quote! { if version == #from_version { Self::#ident(genesis); version = Self::VERSION; } }
                } else {
                    // For other versions, each migration brings us from V to V+1.
                    // TODO: Add a compile-time assert that version < Self::VERSION.
                    quote! { if version == #from_version && version < Self::VERSION { Self::#ident(); version += 1; } }
                }
            )
        }).unzip();

        // Ensure there is a genesis migration, at least an empty one that bumps the version.
        if !seen_versions.contains(&0) {
            migrate_arms.push(quote! {
                if version == 0u32 { version = Self::VERSION; }
            });
        }

        quote! {
            #[automatically_derived]
            impl #generics sdk::module::MigrationHandler for #ty {
                #genesis_ty

                fn init_or_migrate<C: Context>(
                    _ctx: &C,
                    meta: &mut sdk::modules::core::types::Metadata,
                    genesis: Self::Genesis,
                ) -> bool {
                    let mut version = meta.versions.get(Self::NAME).copied().unwrap_or_default();
                    if version == Self::VERSION {
                        return false; // Already the latest version.
                    }

                    #(#migrate_arms)*

                    if version != Self::VERSION {
                        panic!("no migration for module state from version {version} to {}", Self::VERSION)
                    }

                    // Update version information.
                    meta.versions.insert(Self::NAME.to_owned(), Self::VERSION);
                    return true;
                }
            }

            #[automatically_derived]
            impl #generics #ty {
                #(#migrate_fns)*
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct MigrationHandlerAttr {
    /// Version that this handler handles. Zero indicates genesis.
    from_version: u32,
}
impl syn::parse::Parse for MigrationHandlerAttr {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let kind: syn::Ident = input.parse()?;
        let from_version = match kind.to_string().as_str() {
            "init" => 0,
            "from" => {
                let _: syn::token::Eq = input.parse()?;
                let version: syn::LitInt = input.parse()?;

                version.base10_parse()?
            }
            _ => return Err(syn::Error::new(kind.span(), "invalid migration kind")),
        };

        if !input.is_empty() {
            return Err(syn::Error::new(input.span(), "unexpected extra tokens"));
        }
        Ok(Self { from_version })
    }
}

fn parse_attrs(attrs: &[syn::Attribute]) -> Option<MigrationHandlerAttr> {
    let migration_meta = attrs
        .iter()
        .find(|attr| attr.path().is_ident("migration"))?;
    migration_meta
        .parse_args()
        .map_err(|err| {
            emit_compile_error(format!(
                "Unsupported format of #[migration(...)] attribute: {err}"
            ))
        })
        .ok()
}
