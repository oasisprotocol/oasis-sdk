use base64;

use oasis_contract_sdk_types::testing::addresses;

use super::*;

#[test]
fn test_initiate_serialization() {
    let tcs = vec![
       (
           "o2RuYW1lYGZzeW1ib2xgaGRlY2ltYWxzAA==",
           Default::default(),
       ),
       (
           "o2RuYW1ldVRlc3Rpbmcgc2VyaWFsaXphdGlvbmZzeW1ib2xoVEVTVF9TRVJoZGVjaW1hbHMB",
           TokenInstantiation{
               name: "Testing serialization".to_string(),
               symbol: "TEST_SER".to_string(),
               decimals: 1,
               ..Default::default()
           },
       ),
       (
           "pGRuYW1lb1RFU1QgdG9rZW4gbmFtZWZzeW1ib2xkVEVTVGhkZWNpbWFscwZwaW5pdGlhbF9iYWxhbmNlc4KiZmFtb3VudEInEGdhZGRyZXNzVQDzj3nsHmz+l7T+BseJi1Ko+ttHg6JmYW1vdW50QQpnYWRkcmVzc1UA6WTLZ/m1vC5bdgxVKoClozN3AHA=",
           TokenInstantiation{
              name: "TEST token name".to_string(),
              symbol: "TEST".to_string(),
              decimals: 6,
              initial_balances: vec![
              (
                InitialBalance{
                  address: addresses::alice::address().into(),
                  amount: 10_000,
                }
	      ),
              (
                InitialBalance{
                  address: addresses::charlie::address().into(),
                  amount: 10,
                }
	      ),
              ],
              minting: None,
            },
          ),
    ];

    for (encoded_base64, tc) in tcs {
        let ser = cbor::to_vec(tc.clone());
        assert_eq!(
            base64::encode(ser),
            encoded_base64,
            "serialization should match"
        );

        let dec: TokenInstantiation = cbor::from_slice(&base64::decode(encoded_base64).unwrap())
            .expect("token instantiation should deserialize correctly");
        assert_eq!(dec, tc, "decoded account should match the expected value");
    }
}
