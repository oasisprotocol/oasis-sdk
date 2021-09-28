package types

import (
	"encoding/binary"
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

func TestAddressMultisig(t *testing.T) {
	require := require.New(t)

	addr := NewAddressFromMultisig(&MultisigConfig{
		Signers: []MultisigSigner{
			{
				// A snapshot of ../testing Alice.
				PublicKey: PublicKey{PublicKey: ed25519.NewPublicKey("NcPzNW3YU2T+ugNUtUWtoQnRvbOL9dYSaBfbjHLP1pE=")},
				Weight:    1,
			},
			{
				// A snapshot of ../testing Bob.
				PublicKey: PublicKey{PublicKey: ed25519.NewPublicKey("YgkEiVSR4SMQdfXw+ppuFYlqH0seutnCKk8KG8PyAx0=")},
				Weight:    1,
			},
		},
		Threshold: 2,
	})

	require.EqualValues("oasis1qpcprk8jxpsjxw9fadxvzrv9ln7td69yus8rmtux", addr.String())
}

func TestAddressModule(t *testing.T) {
	require := require.New(t)

	id := make([]byte, 8)

	binary.BigEndian.PutUint64(id, uint64(42))
	addr := NewAddressForModule("contracts", id)
	require.EqualValues("oasis1qq398yyk4wt2zxhtt8c66raynelgt6ngh5yq87xg", addr.String())
}
