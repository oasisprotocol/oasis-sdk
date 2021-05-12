package types

import (
	"testing"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/stretchr/testify/require"
)

func TestMultisigConfigBatch(t *testing.T) {
	require := require.New(t)

	dummyPKA := PublicKey{PublicKey: ed25519.NewPublicKey("CgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")}
	dummyPKB := PublicKey{PublicKey: ed25519.NewPublicKey("CwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")}
	dummyPKC := PublicKey{PublicKey: ed25519.NewPublicKey("DAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")}
	config := MultisigConfig{
		Signers: []MultisigSigner{
			{
				PublicKey: dummyPKA,
				Weight:    1,
			},
			{
				PublicKey: dummyPKB,
				Weight:    1,
			},
			{
				PublicKey: dummyPKC,
				Weight:    2,
			},
		},
		Threshold: 2,
	}
	dummySigA := []byte("a")
	dummySigB := []byte("b")
	dummySigC := []byte("c")

	_, _, err := config.Batch([][]byte{dummySigA, nil, nil})
	require.Error(err, "insufficient weight")

	pks, sigs, err := config.Batch([][]byte{dummySigA, dummySigB, nil})
	require.NoError(err, "sufficient weight ab")
	require.Equal([]PublicKey{dummyPKA, dummyPKB}, pks)
	require.Equal([][]byte{dummySigA, dummySigB}, sigs)

	pks, sigs, err = config.Batch([][]byte{nil, nil, dummySigC})
	require.NoError(err, "sufficient weight c")
	require.Equal([]PublicKey{dummyPKC}, pks)
	require.Equal([][]byte{dummySigC}, sigs)

	pks, sigs, err = config.Batch([][]byte{dummySigA, dummySigB, dummySigC})
	require.NoError(err, "sufficient weight abc")
	require.Equal([]PublicKey{dummyPKA, dummyPKB, dummyPKC}, pks)
	require.Equal([][]byte{dummySigA, dummySigB, dummySigC}, sigs)

	_, _, err = config.Batch([][]byte{dummySigA, dummySigB})
	require.Error(err, "too few signature slots")

	_, _, err = config.Batch([][]byte{dummySigA, dummySigB, nil, nil})
	require.Error(err, "too many signature slots")
}
