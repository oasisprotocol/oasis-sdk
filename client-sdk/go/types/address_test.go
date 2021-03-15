package types

import (
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
)

func TestAddressEd25519(t *testing.T) {
	require := require.New(t)

	pk := ed25519.NewPublicKey("utrdHlX///////////////////////////////////8=")
	addr := NewAddress(pk)

	require.EqualValues("oasis1qryqqccycvckcxp453tflalujvlf78xymcdqw4vz", addr.String())
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
