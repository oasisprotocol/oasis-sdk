use crate::testing::keys;

use super::{Config, Signer};

#[test]
fn test_config_verify() {
    Config {
        signers: vec![Signer {
            public_key: keys::alice::pk(),
            weight: 0,
        }],
        threshold: 0,
    }
    .verify()
    .expect_err("zero threshold");
    Config {
        signers: vec![
            Signer {
                public_key: keys::alice::pk(),
                weight: 1,
            },
            Signer {
                public_key: keys::alice::pk(),
                weight: 1,
            },
        ],
        threshold: 1,
    }
    .verify()
    .expect_err("duplicate key");
    Config {
        signers: vec![
            Signer {
                public_key: keys::alice::pk(),
                weight: 1,
            },
            Signer {
                public_key: keys::bob::pk(),
                weight: 0,
            },
        ],
        threshold: 1,
    }
    .verify()
    .expect_err("zero weight key");
    Config {
        signers: vec![
            Signer {
                public_key: keys::alice::pk(),
                weight: 1,
            },
            Signer {
                public_key: keys::bob::pk(),
                weight: u64::max_value(),
            },
        ],
        threshold: 1,
    }
    .verify()
    .expect_err("weight overflow");
    Config {
        signers: vec![
            Signer {
                public_key: keys::alice::pk(),
                weight: 1,
            },
            Signer {
                public_key: keys::bob::pk(),
                weight: 1,
            },
        ],
        threshold: 3,
    }
    .verify()
    .expect_err("impossible threshold");
    Config {
        signers: vec![
            Signer {
                public_key: keys::alice::pk(),
                weight: 1,
            },
            Signer {
                public_key: keys::bob::pk(),
                weight: 1,
            },
        ],
        threshold: 2,
    }
    .verify()
    .expect("this one should be fine");
}
