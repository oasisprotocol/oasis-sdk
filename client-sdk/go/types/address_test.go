package types

import (
	"encoding/binary"
	"encoding/hex"
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
)

func TestAddressEd25519(t *testing.T) {
	require := require.New(t)

	pk := ed25519.NewPublicKey("utrdHlX///////////////////////////////////8=")
	addr := NewAddress(NewSignatureAddressSpecEd25519(pk))

	require.EqualValues("oasis1qryqqccycvckcxp453tflalujvlf78xymcdqw4vz", addr.String())
}

func TestAddressSecp256k1Eth(t *testing.T) {
	require := require.New(t)

	pk := secp256k1.NewPublicKey("Arra3R5V////////////////////////////////////")
	addr := NewAddress(NewSignatureAddressSpecSecp256k1Eth(pk))

	require.EqualValues("oasis1qzd7akz24n6fxfhdhtk977s5857h3c6gf5583mcg", addr.String())
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

func TestAddressRaw(t *testing.T) {
	require := require.New(t)

	ethAddress, _ := hex.DecodeString("dce075e1c39b1ae0b75d554558b6451a226ffe00")
	addr := NewAddressRaw(AddressV0Secp256k1EthContext, ethAddress)
	require.EqualValues("oasis1qrk58a6j2qn065m6p06jgjyt032f7qucy5wqeqpt", addr.String())
}
