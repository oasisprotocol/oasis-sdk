//! Module that contains known test keys.

// TODO: Should be derived from seeds once implemented in the Rust version.

/// Define an ed25519 test key.
macro_rules! test_key_ed25519 {
    ($doc:expr, $name:ident, $pk:expr) => {
        #[doc = " Test key "]
        #[doc=$doc]
        #[doc = "."]
        pub mod $name {
            use crate::{
                crypto::signature::{ed25519, PublicKey},
                types::address::Address,
            };

            #[doc = " Test public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk() -> PublicKey {
                PublicKey::Ed25519(pk_ed25519())
            }

            #[doc = " Test Ed25519 public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk_ed25519() -> ed25519::PublicKey {
                $pk.into()
            }

            #[doc = " Test address "]
            #[doc=$doc]
            #[doc = "."]
            pub fn address() -> Address {
                Address::from_pk(&pk())
            }
        }
    };
}

/// Define a secp256k1 test key.
macro_rules! test_key_secp256k1 {
    ($doc:expr, $name:ident, $pk:expr) => {
        #[doc = " Test key "]
        #[doc=$doc]
        #[doc = "."]
        pub mod $name {
            use crate::{
                crypto::signature::{secp256k1, PublicKey},
                types::address::Address,
            };

            #[doc = " Test public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk() -> PublicKey {
                PublicKey::Secp256k1(pk_secp256k1())
            }

            #[doc = " Test Secp256k1 public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk_secp256k1() -> secp256k1::PublicKey {
                $pk.into()
            }

            #[doc = " Test address "]
            #[doc=$doc]
            #[doc = "."]
            pub fn address() -> Address {
                Address::from_pk(&pk())
            }
        }
    };
}

test_key_ed25519!("A", alice, "NcPzNW3YU2T+ugNUtUWtoQnRvbOL9dYSaBfbjHLP1pE=");
test_key_ed25519!("B", bob, "YgkEiVSR4SMQdfXw+ppuFYlqH0seutnCKk8KG8PyAx0=");
test_key_ed25519!("C", charlie, "8l1AQE+ETOPLckiNJ7NOD+AfZdaPw6wguir/vSF11YI=");
test_key_secp256k1!("D", dave, "AwF6GNjbybMzhi3XRj5R1oTiMMkO1nAwB7NZAlH1X4BE");
