use proc_macro2::TokenStream;
use quote::quote;

macro_rules! parse_cargo_ver {
    ($ver:literal) => {{
        let cargo_pkg_version_env_var = if cfg!(test) {
            concat!("TEST_CARGO_PKG_VERSION_", $ver)
        } else {
            concat!("CARGO_PKG_VERSION_", $ver)
        };
        std::env::var(cargo_pkg_version_env_var)
            .unwrap()
            .parse::<u16>()
            .unwrap()
    }};
}

/// Constructs an `oasis_sdk::core::common::version::Version` from the Cargo.toml version.
pub fn version_from_cargo() -> TokenStream {
    let major = parse_cargo_ver!("MAJOR");
    let minor = parse_cargo_ver!("MINOR");
    let patch = parse_cargo_ver!("PATCH");
    quote!(oasis_runtime_sdk::core::common::version::Version::new(#major, #minor, #patch))
}

#[cfg(test)]
mod tests {
    #[test]
    fn generates_version() {
        std::env::set_var("TEST_CARGO_PKG_VERSION_MAJOR", "1");
        std::env::set_var("TEST_CARGO_PKG_VERSION_MINOR", "2");
        std::env::set_var("TEST_CARGO_PKG_VERSION_PATCH", "3");

        let expected: syn::Expr = syn::parse_quote!(
            oasis_runtime_sdk::core::common::version::Version::new(1u16, 2u16, 3u16)
        );
        let actual: syn::Expr = syn::parse2(super::version_from_cargo()).unwrap();

        crate::assert_empty_diff!(actual, expected);
    }
}
