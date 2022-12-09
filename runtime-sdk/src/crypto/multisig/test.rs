use crate::{crypto::signature::Signature, testing::keys};

use super::{Config, Signer};

#[test]
fn test_config_validate_basic() {
    Config {
        signers: vec![Signer {
            public_key: keys::alice::pk(),
            weight: 0,
        }],
        threshold: 0,
    }
    .validate_basic()
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
    .validate_basic()
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
    .validate_basic()
    .expect_err("zero weight key");
    Config {
        signers: vec![
            Signer {
                public_key: keys::alice::pk(),
                weight: 1,
            },
            Signer {
                public_key: keys::bob::pk(),
                weight: u64::MAX,
            },
        ],
        threshold: 1,
    }
    .validate_basic()
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
    .validate_basic()
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
    .validate_basic()
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
            Signer {
                public_key: keys::dave::pk(),
                weight: 1,
            },
            Signer {
                public_key: keys::erin::pk(),
                weight: 1,
            },
            Signer {
                public_key: keys::frank::pk(),
                weight: 1,
            },
            Signer {
                public_key: keys::grace::pk(),
                weight: 1,
            },
        ],
        threshold: 2,
    };
    let dummy_sig_a = Signature::from(vec![97]);
    let dummy_sig_b = Signature::from(vec![98]);
    let dummy_sig_c = Signature::from(vec![99]);
    let dummy_sig_d = Signature::from(vec![100]);
    let dummy_sig_e = Signature::from(vec![101]);
    let dummy_sig_f = Signature::from(vec![102]);
    let dummy_sig_g = Signature::from(vec![103]);
    config
        .batch(&[
            Some(dummy_sig_a.clone()),
            None,
            None,
            None,
            None,
            None,
            None,
        ])
        .expect_err("insufficient weight");
    assert_eq!(
        config
            .batch(&[
                Some(dummy_sig_a.clone()),
                Some(dummy_sig_b.clone()),
                None,
                None,
                None,
                None,
                None
            ])
            .expect("sufficient weight ab"),
        (
            vec![keys::alice::pk(), keys::bob::pk()],
            vec![dummy_sig_a.clone(), dummy_sig_b.clone()]
        )
    );
    assert_eq!(
        config
            .batch(&[
                None,
                None,
                Some(dummy_sig_c.clone()),
                None,
                None,
                None,
                None
            ])
            .expect("sufficient weight c"),
        (vec![keys::charlie::pk()], vec![dummy_sig_c.clone()])
    );
    assert_eq!(
        config
            .batch(&[
                None,
                None,
                None,
                Some(dummy_sig_d.clone()),
                Some(dummy_sig_e.clone()),
                None,
                None
            ])
            .expect("sufficient weight de"),
        (
            vec![keys::dave::pk(), keys::erin::pk()],
            vec![dummy_sig_d.clone(), dummy_sig_e.clone()]
        )
    );
    assert_eq!(
        config
            .batch(&[
                None,
                None,
                None,
                None,
                None,
                Some(dummy_sig_f.clone()),
                Some(dummy_sig_g.clone())
            ])
            .expect("sufficient weight fg"),
        (
            vec![keys::frank::pk(), keys::grace::pk()],
            vec![dummy_sig_f.clone(), dummy_sig_g.clone()]
        )
    );
    assert_eq!(
        config
            .batch(&[
                Some(dummy_sig_a.clone()),
                Some(dummy_sig_b.clone()),
                Some(dummy_sig_c.clone()),
                Some(dummy_sig_d.clone()),
                Some(dummy_sig_e.clone()),
                Some(dummy_sig_f.clone()),
                Some(dummy_sig_g.clone()),
            ])
            .expect("sufficient weight abc"),
        (
            vec![
                keys::alice::pk(),
                keys::bob::pk(),
                keys::charlie::pk(),
                keys::dave::pk(),
                keys::erin::pk(),
                keys::frank::pk(),
                keys::grace::pk()
            ],
            vec![
                dummy_sig_a.clone(),
                dummy_sig_b.clone(),
                dummy_sig_c,
                dummy_sig_d,
                dummy_sig_e,
                dummy_sig_f,
                dummy_sig_g,
            ]
        )
    );
    config
        .batch(&[Some(dummy_sig_a.clone()), Some(dummy_sig_b.clone())])
        .expect_err("too few signature slots");
    config
        .batch(&[
            Some(dummy_sig_a),
            Some(dummy_sig_b),
            None,
            None,
            None,
            None,
            None,
            None,
        ])
        .expect_err("too many signature slots");
}
