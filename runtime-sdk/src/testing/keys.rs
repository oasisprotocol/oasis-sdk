//! Module that contains known test keys.

/// Define an ed25519 test key.
macro_rules! test_key_ed25519 {
    ($doc:expr, $name:ident, $seed:expr) => {
        #[doc = " Test key "]
        #[doc=$doc]
        #[doc = "."]
        pub mod $name {
            use crate::{
                crypto::signature::{ed25519, PublicKey},
                types::address::{Address, SignatureAddressSpec},
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
                signer().public()
            }

            #[doc = " Test Ed25519 signer "]
            #[doc=$doc]
            #[doc = "."]
            pub fn signer() -> ed25519::MemorySigner {
                ed25519::MemorySigner::new_test($seed)
            }

            #[doc = " Test address derivation information "]
            #[doc=$doc]
            #[doc = "."]
            pub fn sigspec() -> SignatureAddressSpec {
                SignatureAddressSpec::Ed25519(pk_ed25519())
            }

            #[doc = " Test address "]
            #[doc=$doc]
            #[doc = "."]
            pub fn address() -> Address {
                Address::from_sigspec(&sigspec())
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
                types::address::{Address, SignatureAddressSpec},
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

            #[doc = " Test address derivation information "]
            #[doc=$doc]
            #[doc = "."]
            pub fn sigspec() -> SignatureAddressSpec {
                SignatureAddressSpec::Secp256k1Eth(pk_secp256k1())
            }

            #[doc = " Test address "]
            #[doc=$doc]
            #[doc = "."]
            pub fn address() -> Address {
                Address::from_sigspec(&sigspec())
            }
        }
    };
}

/// Define an sr25519 test key.
macro_rules! test_key_sr25519 {
    ($doc:expr, $name:ident, $seed:expr) => {
        #[doc = " Test key "]
        #[doc=$doc]
        #[doc = "."]
        pub mod $name {
            use crate::{
                crypto::signature::{sr25519, PublicKey},
                types::address::{Address, SignatureAddressSpec},
            };

            #[doc = " Test public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk() -> PublicKey {
                PublicKey::Sr25519(pk_sr25519())
            }

            #[doc = " Test Sr25519 public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk_sr25519() -> sr25519::PublicKey {
                signer().public()
            }

            #[doc = " Test Sr25519 signer "]
            #[doc=$doc]
            #[doc = "."]
            pub fn signer() -> sr25519::MemorySigner {
                sr25519::MemorySigner::new_test($seed)
            }

            #[doc = " Test address derivation information "]
            #[doc=$doc]
            #[doc = "."]
            pub fn sigspec() -> SignatureAddressSpec {
                SignatureAddressSpec::Sr25519(pk_sr25519())
            }

            #[doc = " Test address "]
            #[doc=$doc]
            #[doc = "."]
            pub fn address() -> Address {
                Address::from_sigspec(&sigspec())
            }
        }
    };
}

test_key_ed25519!("Alice", alice, "oasis-runtime-sdk/test-keys: alice");
test_key_ed25519!("Bob", bob, "oasis-runtime-sdk/test-keys: bob");
test_key_ed25519!("Charlie", charlie, "oasis-runtime-sdk/test-keys: charlie");
test_key_ed25519!("Cory", cory, "ekiden test entity key seed");
test_key_secp256k1!("Dave", dave, "AwF6GNjbybMzhi3XRj5R1oTiMMkO1nAwB7NZAlH1X4BE");
test_key_secp256k1!("Erin", erin, "A9i0oSK+5sLSONbMYGmaFUA+Fb8zzqYEMUMspacIgO09");
test_key_sr25519!("Frank", frank, "oasis-runtime-sdk/test-keys: frank");
test_key_sr25519!("Grace", grace, "oasis-runtime-sdk/test-keys: grace");
