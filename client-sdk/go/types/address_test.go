package types

import "testing"

func TestAddressEd25519(t *testing.T) {
	// TODO
	/*#[test]
	  fn test_address_ed25519() {
	      let pk = PublicKey::Ed25519(
	          "badadd1e55ffffffffffffffffffffffffffffffffffffffffffffffffffffff".into(),
	      );

	      let addr = Address::from_pk(&pk);
	      assert_eq!(
	          addr.to_bech32(),
	          "oasis1qryqqccycvckcxp453tflalujvlf78xymcdqw4vz"
	      );
	  }*/
}

func TestAddressSecp256k1(t *testing.T) {
	/*
			#[test]
		    fn test_address_secp256k1() {
		        let pk = PublicKey::Secp256k1(
		            "02badadd1e55ffffffffffffffffffffffffffffffffffffffffffffffffffffff".into(),
		        );

		        let addr = Address::from_pk(&pk);
		        assert_eq!(
		            addr.to_bech32(),
		            "oasis1qr4cd0sr32m3xcez37ym7rmjp5g88muu8sdfx8u3"
		        );
		    }
	*/
}
