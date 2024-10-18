package callformat

import (
	"crypto/sha512"
	"encoding/hex"
	"testing"

	"github.com/oasisprotocol/curve25519-voi/primitives/x25519"
	"github.com/oasisprotocol/deoxysii"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

func TestInterop(t *testing.T) {
	clientSK := (x25519.PrivateKey)(sha512.Sum512_256([]byte("callformat test client")))
	clientPK := clientSK.Public()
	runtimeSK := (x25519.PrivateKey)(sha512.Sum512_256([]byte("callformat test runtime")))
	runtimePK := runtimeSK.Public()

	call := types.Call{
		Method: "mock",
		Body:   nil,
	}
	var nonce [deoxysii.NonceSize]byte
	cfg := EncodeConfig{
		PublicKey: &types.SignedPublicKey{PublicKey: *runtimePK},
		Epoch:     1,
	}
	callEnc, metadata := encodeCallEncryptedX25519DeoxysII(&call, clientPK, &clientSK, nonce, &cfg)

	// If these change, update runtime-sdk/src/callformat.rs too.
	require.Equal(t, "a264626f6479f6666d6574686f64646d6f636b", hex.EncodeToString(cbor.Marshal(call)))
	require.Equal(t, "a264626f6479a462706b5820eedc75d3c500fc1b2d321757c383e276ab705c5a02013b3f1966e9caf73cdb0264646174615823c4635f2f9496a033a578e3f1e007be5d6cfa9631fb2fe2c8c76d26b322b6afb2fa5cdf6565706f636801656e6f6e63654f00000000000000000000000000000066666f726d617401", hex.EncodeToString(cbor.Marshal(callEnc)))

	resultCBOR, err := hex.DecodeString("a1626f6bf6")
	require.NoError(t, err)
	var result types.CallResult
	err = cbor.Unmarshal(resultCBOR, &result)
	require.NoError(t, err)
	resultEncCBOR, err := hex.DecodeString("a167756e6b6e6f776ea264646174615528d1c5eedc5e54e1ef140ba905e84e0bea8daf60af656e6f6e63654f000000000000000000000000000000")
	require.NoError(t, err)
	var resultEnc types.CallResult
	err = cbor.Unmarshal(resultEncCBOR, &resultEnc)
	require.NoError(t, err)

	resultOurs, err := DecodeResult(&resultEnc, metadata)
	require.NoError(t, err)
	require.Equal(t, &result, resultOurs)
}
