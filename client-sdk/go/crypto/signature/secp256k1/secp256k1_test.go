package secp256k1

import (
	"encoding/hex"
	"testing"

	"github.com/stretchr/testify/require"

	sdkSignature "github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

// A helper method that creates a new test secp256k1 signer.
func newTestSigner(t *testing.T) sdkSignature.Signer {
	require := require.New(t)

	// Use the same test private key as in the btcec examples.
	hexPrivateKey := "22a47fa09a223f2aa079edf85a7c2d4f87" + "20ee63e502ee2869afab7de234b80c"

	rawPrivateKey, err := hex.DecodeString(hexPrivateKey)
	require.NoError(err, "DecodeString")
	require.NotNil(rawPrivateKey, "DecodeString")

	signer := NewSigner(rawPrivateKey)
	require.NotNil(signer.Public(), "signer public key should not be nil")

	return signer
}

func TestSecp256k1SignAndVerify(t *testing.T) {
	require := require.New(t)
	s := newTestSigner(t)

	ctx1 := []byte("ctx1")
	msg1 := []byte("msg1")
	sig1, err := s.ContextSign(ctx1, msg1)
	require.NoError(err, "ContextSign")
	require.NotNil(sig1, "signature should not be nil")

	ver1 := s.Public().Verify(ctx1, msg1, sig1)
	require.True(ver1, "verification should succeed")

	ctx2 := []byte("ctx2")
	msg2 := []byte("msg2")
	sig2, err := s.ContextSign(ctx2, msg2)
	require.NoError(err, "ContextSign")
	require.NotNil(sig2, "signature should not be nil")

	ver2 := s.Public().Verify(ctx2, msg2, sig2)
	require.True(ver2, "verification should succeed")

	require.False(s.Public().Verify(ctx1, msg2, sig1))
	require.False(s.Public().Verify(ctx1, msg2, sig2))
	require.False(s.Public().Verify(ctx2, msg2, sig1))
	require.False(s.Public().Verify(ctx2, msg1, sig2))
	require.False(s.Public().Verify(ctx1, msg1, sig2))
	require.False(s.Public().Verify(ctx2, msg2, sig1))
	require.False(s.Public().Verify(ctx2, msg1, sig1))

	require.False(s.Public().Verify(ctx2, []byte("foo"), sig2))
	require.False(s.Public().Verify([]byte("bar"), msg2, sig2))

	// Try the example from btcec too, the signature should match.
	ctx3 := []byte("")
	msg3 := []byte("test message")
	sig3, err := s.ContextSign(ctx3, msg3)
	require.NoError(err, "ContextSign")
	require.NotNil(sig3, "signature should not be nil")
	require.EqualValues(hex.EncodeToString(sig3), "304502210082fa505a49af65ba0e90450dfb9a03e69947840bf49bba04ee0091fa5173124902205b33fae275be8e59d851568151c95fb15f18e5d23d4bff39e57333df0814bce4")

	ver3 := s.Public().Verify(ctx3, msg3, sig3)
	require.True(ver3, "verification should succeed")

	ver4 := s.Public().Verify(ctx3, msg3, []byte("asdfghjkl"))
	require.False(ver4, "verification should fail")

	ver5 := s.Public().Verify(ctx3, msg3, []byte(""))
	require.False(ver5, "verification should fail")
}

func TestSecp256k1PubKeySerDes(t *testing.T) {
	require := require.New(t)
	s := newTestSigner(t)

	spk := s.Public()
	require.NotNil(spk, "signer public key should not be nil")

	pk, ok := spk.(PublicKey)
	require.True(ok, "signer public key should be a secp256k1 public key")

	mstr := pk.String()
	require.NotNil(mstr, "String")

	sstr := s.String()
	require.NotNil(sstr, "String")
	require.EqualValues(mstr, sstr)

	mbin, err := pk.MarshalBinary()
	require.NoError(err, "MarshalBinary")

	var upk PublicKey
	err = upk.UnmarshalBinary(mbin)
	require.NoError(err, "UnmarshalBinary")
	require.True(pk.Equal(upk))
	require.True(upk.Equal(pk))

	mtxt, err := pk.MarshalText()
	require.NoError(err, "MarshalText")

	var utpk PublicKey
	err = utpk.UnmarshalText(mtxt)
	require.NoError(err, "UnmarshalText")
	require.True(pk.Equal(utpk))
	require.True(utpk.Equal(pk))

	var x PublicKey
	require.Error(x.UnmarshalText([]byte("asdf")))
	require.Error(x.UnmarshalBinary([]byte("ghij")))
}

func TestSecp256k1Reset(t *testing.T) {
	require := require.New(t)
	s := newTestSigner(t)

	ctx1 := []byte("ctx1")
	msg1 := []byte("msg1")
	sig1, err := s.ContextSign(ctx1, msg1)
	require.NoError(err, "ContextSign")
	require.NotNil(sig1, "signature should not be nil")

	ver1 := s.Public().Verify(ctx1, msg1, sig1)
	require.True(ver1, "verification should succeed")

	s.Reset()

	ver2 := s.Public().Verify(ctx1, msg1, sig1)
	require.False(ver2, "verification should fail after reset")
}
