use crate::{crypto::signature::Signature, testing::keys};

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

#[test]
fn test_config_batch() {
    let config = Config {
        signers: vec![
            Signer {
                public_key: keys::alice::pk(),
                weight: 1,
            },
            Signer {
                public_key: keys::bob::pk(),
                weight: 1,
            },
            Signer {
                public_key: keys::charlie::pk(),
                weight: 2,
            },
        ],
        threshold: 2,
    };
    let dummy_sig_a = Signature::from(vec![97]);
    let dummy_sig_b = Signature::from(vec![98]);
    let dummy_sig_c = Signature::from(vec![99]);
    config
        .batch(&[Some(dummy_sig_a.clone()), None, None])
        .expect_err("insufficient weight");
    assert_eq!(
        config
            .batch(&[Some(dummy_sig_a.clone()), Some(dummy_sig_b.clone()), None])
            .expect("sufficient weight ab"),
        (
            vec![keys::alice::pk(), keys::bob::pk()],
            vec![dummy_sig_a.clone(), dummy_sig_b.clone()]
        )
    );
    assert_eq!(
        config
            .batch(&[None, None, Some(dummy_sig_c.clone())])
            .expect("sufficient weight c"),
        (vec![keys::charlie::pk()], vec![dummy_sig_c.clone()])
    );
    assert_eq!(
        config
            .batch(&[
                Some(dummy_sig_a.clone()),
                Some(dummy_sig_b.clone()),
                Some(dummy_sig_c.clone()),
            ])
            .expect("sufficient weight abc"),
        (
            vec![keys::alice::pk(), keys::bob::pk(), keys::charlie::pk()],
            vec![
                dummy_sig_a.clone(),
                dummy_sig_b.clone(),
                dummy_sig_c.clone(),
            ]
        )
    );
    config
        .batch(&[Some(dummy_sig_a.clone()), Some(dummy_sig_b.clone())])
        .expect_err("too few signature slots");
    config
        .batch(&[
            Some(dummy_sig_a.clone()),
            Some(dummy_sig_b.clone()),
            None,
            None,
        ])
        .expect_err("too many signature slots");
}
