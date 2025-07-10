//! Module that contains known test keys.

/// Define an ed25519 test key.
macro_rules! test_key_ed25519 {
    ($doc:expr, $name:ident, $seed:expr) => {
        #[doc = " Test key "]
        #[doc=$doc]
        #[doc = "."]
        pub mod $name {
            use crate::{
                core::common::crypto::hash::Hash,
                crypto::signature::{ed25519, PublicKey, Signer},
                types::address::{Address, SignatureAddressSpec},
            };

            #[doc = " Test Ed25519 signer "]
            #[doc=$doc]
            #[doc = "."]
            pub fn signer() -> ed25519::MemorySigner {
                let seed = Hash::digest_bytes($seed.as_bytes());
                ed25519::MemorySigner::new_from_seed(seed.as_ref()).unwrap()
            }

            #[doc = " Test public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk() -> PublicKey {
                signer().public_key()
            }

            #[doc = " Test Ed25519 public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk_ed25519() -> ed25519::PublicKey {
                if let PublicKey::Ed25519(pk) = pk() {
                    pk
                } else {
                    unreachable!()
                }
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
    ($doc:expr, $name:ident, $seed:expr) => {
        #[doc = " Test key "]
        #[doc=$doc]
        #[doc = "."]
        pub mod $name {
            use crate::{
                core::common::crypto::hash::Hash,
                crypto::signature::{secp256k1, PublicKey, Signer},
                types::address::{Address, SignatureAddressSpec},
            };

            #[doc = " Test Secp256k1 signer "]
            #[doc=$doc]
            #[doc = "."]
            pub fn signer() -> secp256k1::MemorySigner {
                let seed = Hash::digest_bytes($seed.as_bytes());
                secp256k1::MemorySigner::new_from_seed(seed.as_ref()).unwrap()
            }

            #[doc = " Test public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk() -> PublicKey {
                signer().public_key()
            }

            #[doc = " Test Secp256k1 public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk_secp256k1() -> secp256k1::PublicKey {
                if let PublicKey::Secp256k1(pk) = pk() {
                    pk
                } else {
                    unreachable!()
                }
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
                core::common::crypto::hash::Hash,
                crypto::signature::{sr25519, PublicKey, Signer},
                types::address::{Address, SignatureAddressSpec},
            };

            #[doc = " Test Sr25519 signer "]
            #[doc=$doc]
            #[doc = "."]
            pub fn signer() -> sr25519::MemorySigner {
                let seed = Hash::digest_bytes($seed.as_bytes());
                sr25519::MemorySigner::new_from_seed(seed.as_ref()).unwrap()
            }

            #[doc = " Test public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk() -> PublicKey {
                signer().public_key()
            }

            #[doc = " Test Sr25519 public key "]
            #[doc=$doc]
            #[doc = "."]
            pub fn pk_sr25519() -> sr25519::PublicKey {
                if let PublicKey::Sr25519(pk) = pk() {
                    pk
                } else {
                    unreachable!()
                }
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

test_key_ed25519!("A", alice, "oasis-runtime-sdk/test-keys: alice");
test_key_ed25519!("B", bob, "oasis-runtime-sdk/test-keys: bob");
test_key_ed25519!("C", charlie, "oasis-runtime-sdk/test-keys: charlie");
test_key_secp256k1!("D", dave, "oasis-runtime-sdk/test-keys: dave");
test_key_secp256k1!("E", erin, "oasis-runtime-sdk/test-keys: erin");
test_key_sr25519!("F", frank, "oasis-runtime-sdk/test-keys: frank");
test_key_sr25519!("G", grace, "oasis-runtime-sdk/test-keys: grace");
