//! Module that contains definitions useful for testing.

// TODO: Should be derived from seeds once implemented in the Rust version.

/// Define a test key.
macro_rules! test_key {
    ($doc:expr, $name:ident, $pk:expr) => {
        #[doc=" Test key "]
        #[doc=$doc]
        #[doc="."]
        pub mod $name {
            use crate::crypto::signature::PublicKey;
            use crate::types::address::Address;

            #[doc=" Test public key "]
            #[doc=$doc]
            #[doc="."]
            pub fn pk() -> PublicKey {
                PublicKey::Ed25519($pk.into())
            }

            #[doc=" Test address "]
            #[doc=$doc]
            #[doc="."]
            pub fn address() -> Address {
                Address::from_pk(&pk())
            }
        }
    }
}

test_key!("A", alice, "NcPzNW3YU2T+ugNUtUWtoQnRvbOL9dYSaBfbjHLP1pE=");
test_key!("B", bob, "YgkEiVSR4SMQdfXw+ppuFYlqH0seutnCKk8KG8PyAx0=");
test_key!("C", charlie, "8l1AQE+ETOPLckiNJ7NOD+AfZdaPw6wguir/vSF11YI=");
